use async_trait::async_trait;
use color_eyre::Result;
use ratatui::{
    layout::{Alignment, Constraint, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Table},
};
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, error};

use crate::{
    action::Action,
    colors::{BLUE, GREEN, PEACH, RED, TEXT},
    components::{notifications::Notification, utils::centered_rect},
    config::Config,
    event::AppEvent,
    github::{client::GraphQLGithubClient, traits::GithubClient},
    providers::provider::Provider,
    things::{
        pull_request::{PullRequest, PullRequestReviewState, PullRequestState},
        thing::Thing,
    },
};

#[derive(Default, Debug)]
pub struct PullRequestProvider {
    command_tx: Option<UnboundedSender<Action>>,
    event_tx: Option<UnboundedSender<AppEvent>>,
    config: Config,
    pull_requests: Vec<PullRequest>,
    username: String,
    client: GraphQLGithubClient,
    // Pagination state
    has_next_page: bool,
    end_cursor: Option<String>,
    is_loading_more: bool,
    page_size: usize,
}

impl PullRequestProvider {
    pub fn new() -> Self {
        Self { page_size: 20, has_next_page: true, pull_requests: Vec::new(), ..Default::default() }
    }

    fn get_current_user(&mut self) -> Result<()> {
        let tx = self.command_tx.clone().unwrap();
        tx.send(Action::Notify(Notification::Info(String::from("Getting current user..."))))?;
        tokio::spawn(async move {
            match GraphQLGithubClient::get_current_user().await {
                Ok(username) => {
                    tx.send(Action::Notify(Notification::Info(format!("Got user {username}"))))?;
                    tx.send(Action::GetCurrentUserResult(username))
                },
                Err(err) => {
                    error!("Error getting current user: {:?}", err);
                    tx.send(Action::Error(format!("{:#}", err)))
                },
            }
        });
        Ok(())
    }

    fn fetch_repos(&mut self) -> Result<()> {
        let tx = self.command_tx.clone().unwrap();
        tx.send(Action::Notify(Notification::Info(String::from("Fetching pull requests..."))))?;
        let username = self.username.clone();
        let initial_load_size = 10;

        // Reset pagination state for fresh fetch
        self.has_next_page = true;
        self.end_cursor = None;
        self.is_loading_more = false;

        tokio::spawn(async move {
            match GraphQLGithubClient::get_pull_requests_paginated(username, initial_load_size, None).await {
                Ok((pull_requests, has_next_page, end_cursor)) => {
                    let _ = tx.send(Action::Notify(Notification::Info(format!(
                        "Got pull requests: {}",
                        pull_requests.len()
                    ))));
                    let _ = tx.send(Action::LoadMorePullRequestsResult(pull_requests, has_next_page, end_cursor));
                },
                Err(err) => {
                    error!("Error getting pull requests: {:?}", err);
                    let _ = tx.send(Action::Error(err.to_string()));
                },
            }
        });
        Ok(())
    }

    fn load_more_pull_requests(&mut self) -> Result<()> {
        if !self.has_next_page || self.is_loading_more {
            return Ok(());
        }

        debug!("Loading more pull requests...");
        let command_tx = self.command_tx.clone().unwrap();
        command_tx.send(Action::Notify(Notification::Info(String::from("Loading more pull requests..."))))?;
        let event_tx = self.event_tx.clone().unwrap();
        let username = self.username.clone();
        let page_size = self.page_size as i32;
        let after = self.end_cursor.clone();

        self.is_loading_more = true;

        tokio::spawn(async move {
            match GraphQLGithubClient::get_pull_requests_paginated(username, page_size, after).await {
                Ok((pull_requests, has_next_page, end_cursor)) => {
                    let _ = event_tx.send(AppEvent::ProviderReturnedResult);
                },
                Err(err) => {
                    error!("Error loading more pull requests: {:?}", err);
                    let _ = command_tx.send(Action::Error(err.to_string()));
                },
            }
        });
        Ok(())
    }

    fn selected_column(columns: Vec<&'static str>, selected_column: usize) -> Vec<Cell> {
        columns
            .iter()
            .enumerate()
            .map(|(i, &column)| {
                if i == selected_column {
                    Cell::from(column).style(Style::new().fg(PEACH))
                } else {
                    Cell::from(column).style(Style::new().fg(TEXT))
                }
            })
            .collect()
    }

    // fn sort(&mut self, ) {
    //     if !self.pull_requests.is_empty() {
    //         let pull_requests = &mut self.pull_requests;
    //         pull_requests.sort_by(|a, b| {
    //             match self.selected_column {
    //                 0 => a.number.cmp(&b.number),
    //                 1 => a.repository.cmp(&b.repository),
    //                 2 => a.title.cmp(&b.title),
    //                 3 => a.author.cmp(&b.author),
    //                 4 => a.created_at.cmp(&b.created_at),
    //                 5 => a.updated_at.cmp(&b.updated_at),
    //                 6 => (a.additions + a.deletions).cmp(&(b.additions + b.deletions)),
    //                 7 => a.state.cmp(&b.state),
    //                 _ => a.title.cmp(&b.title),
    //             }
    //         });
    //     }
    // }

    fn refresh(&mut self) {
        let tx = self.command_tx.clone().unwrap();
        if self.username.is_empty() {
            // Get username and then immediately fetch repos
            let tx_clone = tx.clone();
            tokio::spawn(async move {
                match GraphQLGithubClient::get_current_user().await {
                    Ok(username) => {
                        if let Err(e) = tx.send(Action::GetCurrentUserResult(username.clone())) {
                            tracing::error!("Failed to send user result: {}", e);
                            return;
                        }
                        // Immediately fetch repos after getting username
                        match GraphQLGithubClient::get_pull_requests_paginated(username, 10, None).await {
                            Ok((pull_requests, has_next_page, end_cursor)) => {
                                let _ = tx.send(Action::LoadMorePullRequestsResult(
                                    pull_requests,
                                    has_next_page,
                                    end_cursor,
                                ));
                            },
                            Err(err) => {
                                error!("Error getting pull requests: {:?}", err);
                                let _ = tx.send(Action::Error(err.to_string()));
                            },
                        }
                    },
                    Err(err) => {
                        error!("Error getting current user: {:?}", err);
                        let _ = tx.send(Action::Error(format!("{:#}", err)));
                    },
                }
            });
        } else {
            let _ = self.fetch_repos();
        }
    }

    fn render_token_error(&self, f: &mut ratatui::prelude::Frame<'_>, area: Rect) {
        let text = Paragraph::new(vec![
            Line::from("Error: GITHUB_TOKEN is not set!"),
            Line::from("Create a Personal Access Token in the GitHub UI and set the GITHUB_TOKEN environment variable to its value before running ghtui"),
            Line::from("Press 'q' or 'ctrl-c' to quit"),
        ]).style(Style::default().fg(RED))
        .alignment(Alignment::Center);
        f.render_widget(text, centered_rect(area, 100, 10));
    }
}

#[async_trait]
impl Provider for PullRequestProvider {
    async fn provide(&mut self) -> Result<()> {
        todo!()
    }

    fn get_things(&self) -> Result<Vec<Box<dyn Thing>>> {
        todo!()
    }

    fn commands(&self) -> Vec<&'static str> {
        vec!["pr", "pullrequest", "pull-request", "pull_request"]
    }
}
