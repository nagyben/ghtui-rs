use std::sync::{Arc, RwLock};

use color_eyre::{
    eyre::{eyre, Result},
    owo_colors::OwoColorize,
};
use crossterm::event::{KeyCode, KeyEvent};
use graphql_client::GraphQLQuery;
use oauth2::{
    basic::BasicClient, reqwest::async_http_client, AuthUrl, ClientId, DeviceAuthorizationUrl, Scope,
    StandardDeviceAuthorizationResponse, TokenUrl,
};
use octocrab::Octocrab;
use ratatui::{prelude::*, widgets::*};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, error, info, trace};

use super::pull_request::PullRequestState;
use crate::{
    action::{self, Action},
    colors::{BASE, TEXT},
    components::{
        notifications::Notification::Info,
        pull_request::{PullRequest, PullRequestReviewState},
        Component, Frame,
    },
    config::{Config, KeyBindings},
    event::AppEvent,
    github::{client::GraphQLGithubClient, traits::GithubClient},
};

#[derive(Default)]
pub struct PullRequestInfoOverlay {
    action_tx: Option<UnboundedSender<Action>>,
    config: Config,
    pull_request: Arc<RwLock<PullRequest>>,
    scroll_offset: u16,
    is_loading_details: bool,
}

impl PullRequestInfoOverlay {
    pub fn new(pull_request: PullRequest, action_tx: UnboundedSender<Action>) -> Self {
        Self { pull_request: Arc::new(RwLock::new(pull_request)), action_tx: Some(action_tx), ..Default::default() }
    }

    async fn load_pull_request_details(
        pull_request: Arc<RwLock<PullRequest>>,
        action_tx: UnboundedSender<Action>,
        // event_tx: UnboundedSender<AppEvent>,
    ) -> Result<()> {
        let pull_request = pull_request.clone();
        let action_tx = action_tx.clone();

        let owner: String;
        let repo: String;
        let number: usize;
        {
            let pr = pull_request.read().unwrap();
            if let Some((pr_owner, pr_repo)) = pr.repository.split_once('/') {
                owner = pr_owner.to_string();
                repo = pr_repo.to_string();
                number = pr.number;
            } else {
                return Err(eyre!("Invalid repository format: {}", pr.repository));
            }
        }

        trace!("Loading detailed PR info...");
        match GraphQLGithubClient::get_pull_request_details(owner.to_string(), repo.to_string(), number).await {
            Ok(detailed_pr) => {
                trace!("Loaded detailed PR info");
                {
                    let mut pr = pull_request.write().unwrap();
                    *pr = detailed_pr.clone();
                }
                let _ = action_tx.send(Action::Render);
            },
            Err(e) => {
                error!("Failed to load PR details: {}", e);
                let _ = action_tx.send(Action::Notify(Info("Failed to load PR details".to_string())));
            },
        }
        Ok(())
    }

    pub fn load(&self) {
        trace!("Loading pull request details");
        self.action_tx.as_ref().unwrap().send(Action::Notify(Info("Loading pull request details...".to_string())));

        let pull_request = self.pull_request.clone();
        let action_tx = self.action_tx.clone().unwrap();
        tokio::spawn(async move {
            Self::load_pull_request_details(pull_request, action_tx).await;
        });
    }
}

impl Component for PullRequestInfoOverlay {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.action_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn handle_action(&mut self, action: Action) -> Result<()> {
        match action {
            Action::Tick => {},
            Action::Up => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
            },
            Action::Down => {
                self.scroll_offset = self.scroll_offset.saturating_add(1);
            },
            Action::PageUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(10);
            },
            Action::PageDn => {
                self.scroll_offset = self.scroll_offset.saturating_add(10);
            },
            Action::Enter => {},
            Action::Open => {
                let pr = &self.pull_request.read().unwrap();
                let url = pr.url.clone();
                let _ = open::that(url);
            },
            _ => {},
        }
        Ok(())
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(TEXT).bg(BASE))
            .bg(BASE);
        f.render_widget(Clear, area);
        f.render_widget(block, area);

        let layout = layout::Layout::new(
            Direction::Vertical,
            [
                Constraint::Length(5),       // Header
                Constraint::Length(1),       // separator
                Constraint::Percentage(100), // body
            ],
        )
        .split(area.inner(&Margin { horizontal: 1, vertical: 1 }));

        let pr = &self.pull_request.read().unwrap();
        // Use detailed PR if available, otherwise use summary PR
        let display_pr = pr;

        let header = Paragraph::new(vec![
            Span::styled(
                format!("#{} in {}", display_pr.number, display_pr.repository),
                Style::default().fg(Color::Gray),
            )
            .into(),
            (*display_pr.title).to_string().into(),
            format!("Opened by: {} on {}", display_pr.author, display_pr.created_at).into(),
            format!(
                "State: {}",
                match &display_pr.state {
                    PullRequestState::Closed => "CLOSED".to_string(),
                    PullRequestState::Merged => "MERGED".to_string(),
                    PullRequestState::Open => "OPEN".to_string(),
                }
            )
            .into(),
        ])
        .style(Style::default().fg(TEXT).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Left);

        let horizontal_separator = Paragraph::new("─".repeat(area.width as usize)).style(Style::default().fg(TEXT));

        let body_text = if self.is_loading_details {
            "Loading detailed information...".to_string()
        } else if display_pr.body.is_empty() {
            "No description provided.".to_string()
        } else {
            display_pr.body.clone()
        };

        let body = Paragraph::new(body_text)
            .style(Style::default().fg(TEXT))
            .alignment(Alignment::Left)
            .scroll((self.scroll_offset, 0));

        f.render_widget(header, layout[0]);
        f.render_widget(horizontal_separator, layout[1]);
        f.render_widget(body, layout[2]);
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        todo!()
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        todo!()
    }

    fn is_active(&self) -> bool {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {}
}
