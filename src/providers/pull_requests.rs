use std::sync::{Arc, Mutex};

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
    pull_requests: Arc<Mutex<Vec<PullRequest>>>,
    username: Option<String>,
    client: GraphQLGithubClient,
    page_size: usize,
}

impl PullRequestProvider {
    pub fn new() -> Self {
        Self { page_size: 20, pull_requests: Arc::new(Mutex::new(Vec::new())), ..Default::default() }
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

    async fn load_all_pull_requests(
        username: String,
        pull_requests: Arc<Mutex<Vec<PullRequest>>>,
        page_size: usize,
        action_tx: Option<UnboundedSender<Action>>,
        event_tx: Option<UnboundedSender<AppEvent>>,
    ) -> Result<()> {
        let mut has_next_page = true;
        let mut end_cursor: Option<String> = None;

        while has_next_page {
            if let Some(tx) = &action_tx {
                let _ = tx.send(Action::Notify(Notification::Info("Loading pull requests...".to_string())));
            }
            match GraphQLGithubClient::get_pull_requests_paginated(
                username.clone(),
                page_size as i32,
                end_cursor.clone(),
            )
            .await
            {
                Ok((new_pull_requests, next_page, cursor)) => {
                    if let Ok(mut prs) = pull_requests.lock() {
                        prs.extend(new_pull_requests);
                    }

                    // Emit event after each page so UI updates incrementally
                    if let Some(tx) = &event_tx {
                        let _ = tx.send(AppEvent::ProviderReturnedResult);
                    }

                    has_next_page = next_page;
                    end_cursor = cursor;
                },
                Err(err) => {
                    error!("Error loading pull requests: {:?}", err);
                    if let Some(tx) = &action_tx {
                        let _ = tx.send(Action::Error(err.to_string()));
                    }
                    break;
                },
            }
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

        if self.username.is_none() {
            self.get_current_user();
        } else if let Some(username) = &self.username {
            let username = username.clone();
            let pull_requests = Arc::clone(&self.pull_requests);
            let page_size = self.page_size;
            let action_tx = self.action_tx.clone();
            let event_tx = self.event_tx.clone();

            tokio::spawn(async move {
                let _ = Self::load_all_pull_requests(username, pull_requests, page_size, action_tx, event_tx).await;
            });
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
        let pull_requests = self.pull_requests.lock().unwrap();
        log::debug!("Getting {} things from PullRequestProvider", pull_requests.len());
        Ok(pull_requests.iter().map(|thing| Box::new(thing.clone()) as Box<dyn Thing>).collect())
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
            _ => {},
        }
        Ok(None)
    }
}
