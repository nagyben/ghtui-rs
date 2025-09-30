use async_trait::async_trait;
use color_eyre::{eyre::eyre, Result};
use ratatui::{
    layout::{Alignment, Constraint, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Cell, Paragraph, Row, Table},
};
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, error, trace};

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
    action_tx: Option<UnboundedSender<Action>>,
    event_tx: Option<UnboundedSender<AppEvent>>,
    config: Config,
    pull_requests: Vec<PullRequest>,
    username: Option<String>,
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
        let action_tx = self.action_tx.clone().unwrap();
        let event_tx = self.event_tx.clone().unwrap();
        action_tx.send(Action::Notify(Notification::Info(String::from("Getting current user..."))))?;
        tokio::spawn(async move {
            match GraphQLGithubClient::get_current_user().await {
                Ok(username) => {
                    let _ = action_tx.send(Action::Notify(Notification::Info(format!("Got user {username}"))));
                    let _ = event_tx.send(AppEvent::UserIdentified(username));
                },
                Err(err) => {
                    error!("Error getting current user: {:?}", err);
                    let _ = action_tx.send(Action::Error(format!("{:#}", err)));
                },
            }
        });
        Ok(())
    }

    fn load_more_pull_requests(&mut self) -> Result<()> {
        if !self.has_next_page || self.is_loading_more {
            return Ok(());
        }

        if let Some(username) = &self.username {
            debug!("Loading more pull requests...");
            let command_tx = self.action_tx.clone().unwrap();
            command_tx.send(Action::Notify(Notification::Info(String::from("Loading more pull requests..."))))?;
            let event_tx = self.event_tx.clone().unwrap();
            let username = username.clone();
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
        } else {
            return Err(eyre!("Username is not set"));
        }
        Ok(())
    }

    fn selected_column(columns: Vec<&'static str>, selected_column: usize) -> Vec<Cell<'static>> {
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
        trace!("Refreshing pull requests...");
        let action_tx = self.action_tx.clone().unwrap();
        let event_tx = self.event_tx.clone().unwrap();
        if self.username.is_none() {
            self.get_current_user();
        } else {
            self.load_more_pull_requests();
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

    pub fn with_action_handler(mut self, tx: UnboundedSender<Action>) -> Self {
        self.action_tx = Some(tx);
        self
    }

    pub fn with_event_handler(mut self, tx: UnboundedSender<AppEvent>) -> Self {
        self.event_tx = Some(tx);
        self
    }
}

#[async_trait]
impl Provider for PullRequestProvider {
    async fn provide(&mut self) -> Result<()> {
        self.refresh();
        Ok(())
    }

    fn get_things(&self) -> Result<Vec<Box<dyn Thing>>> {
        log::debug!("Getting {} things from PullRequestProvider", self.pull_requests.len());
        Ok(self.pull_requests.iter().map(|thing| Box::new(thing.clone()) as Box<dyn Thing>).collect())
    }

    fn commands(&self) -> Vec<&'static str> {
        vec!["pr", "pullrequest", "pull-request", "pull_request"]
    }

    fn handle_app_event(&mut self, event: AppEvent) -> Result<Option<Action>> {
        match event {
            AppEvent::UserIdentified(user) => self.username = Some(user.clone()),
            _ => {},
        }
        Ok(None)
    }

    fn handle_action(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::LoadMorePullRequests => {
                self.load_more_pull_requests()?;
            },
            Action::LoadMorePullRequestsResult(pull_requests, has_next_page, end_cursor) => {
                debug!("Loaded {} more pull requests", pull_requests.len());
                self.pull_requests.extend(pull_requests);
                self.has_next_page = has_next_page;
                self.end_cursor = end_cursor;
                self.is_loading_more = false;
                if let Some(tx) = &self.event_tx {
                    let _ = tx.send(AppEvent::ProviderReturnedResult);
                }
            },
            _ => {},
        }
        Ok(None)
    }
}
