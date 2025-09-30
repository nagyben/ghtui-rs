use std::any::Any;

use color_eyre::{eyre::Result, owo_colors::OwoColorize};
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
use tracing::{debug, info};

use crate::{
    action::Action,
    colors::{BASE, TEXT},
    components::{Component, Frame},
    config::{Config, KeyBindings},
    github::{client::GraphQLGithubClient, traits::GithubClient},
    things::pull_request::{PullRequest, PullRequestState},
};

#[derive(Default)]
pub struct PullRequestInfoOverlay {
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
    pull_request: Option<PullRequest>,
    detailed_pull_request: Option<PullRequest>,
    scroll_offset: u16,
    is_loading_details: bool,
}

impl PullRequestInfoOverlay {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_pull_request(mut self, pull_request: PullRequest) -> Self {
        self.pull_request = Some(pull_request);
        self.detailed_pull_request = None;
        self.is_loading_details = false; // Will be set to true when we start loading
        self
    }

    fn load_pull_request_details(&mut self) {
        if let (Some(pr), Some(tx)) = (&self.pull_request, &self.command_tx) {
            if self.is_loading_details {
                return; // Already loading
            }

            self.is_loading_details = true;
            let pr = pr.clone();
            let tx_clone = tx.clone();

            tokio::spawn(async move {
                if let Some(repo_parts) = pr.repository.split_once('/') {
                    let (owner, repo) = repo_parts;
                    match GraphQLGithubClient::get_pull_request_details(owner.to_string(), repo.to_string(), pr.number)
                        .await
                    {
                        Ok(detailed_pr) => {
                            let _ = tx_clone.send(Action::PullRequestDetailsLoaded(Box::new(detailed_pr)));
                        },
                        Err(e) => {
                            debug!("Failed to load PR details: {}", e);
                            let _ = tx_clone.send(Action::PullRequestDetailsLoadError);
                        },
                    }
                }
            });
        }
    }
}

impl Component for PullRequestInfoOverlay {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);

        // Start loading detailed data if we have a PR
        if self.pull_request.is_some() {
            self.load_pull_request_details();
        }

        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
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
                if let Some(pr) = &self.pull_request {
                    let url = pr.url.clone();
                    let _ = open::that(url);
                }
            },
            Action::PullRequestDetailsLoaded(detailed_pr) => {
                self.detailed_pull_request = Some(*detailed_pr);
                self.is_loading_details = false;
            },
            Action::PullRequestDetailsLoadError => {
                self.is_loading_details = false;
            },
            _ => {},
        }
        Ok(None)
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(TEXT).bg(BASE))
            .bg(BASE);
        f.render_widget(Clear, area);
        f.render_widget(block, area);

        let layout = layout::Layout::new(Direction::Vertical, [
            Constraint::Length(5),       // Header
            Constraint::Length(1),       // separator
            Constraint::Percentage(100), // body
        ])
        .split(area.inner(&Margin { horizontal: 1, vertical: 1 }));

        if let Some(pr) = &self.pull_request {
            // Use detailed PR if available, otherwise use summary PR
            let display_pr = self.detailed_pull_request.as_ref().unwrap_or(pr);

            let header = Paragraph::new(vec![
                Span::styled(
                    format!("#{} in {}", display_pr.number, display_pr.repository),
                    Style::default().fg(Color::Gray),
                )
                .into(),
                (*display_pr.title).to_string().into(),
                format!("Opened by: {} on {}", display_pr.author, display_pr.created_at).into(),
                format!("State: {}", match &display_pr.state {
                    PullRequestState::Closed => "CLOSED".to_string(),
                    PullRequestState::Merged => "MERGED".to_string(),
                    PullRequestState::Open => "OPEN".to_string(),
                })
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
        }
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
    }
}
