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

use super::pull_request::{self, pull_requests_query::PullRequestState};
use crate::{
    action::Action,
    colors::{BASE, TEXT},
    components::{
        pull_request::{
            pull_requests_query::{self, PullRequestReviewState},
            PullRequest, PullRequestsQuery,
        },
        Component, Frame,
    },
    config::{Config, KeyBindings},
};

#[derive(Default)]
pub struct PullRequestInfoOverlay {
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
    pull_request: Option<PullRequest>,
    scroll_offset: u16,
}

impl PullRequestInfoOverlay {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_pull_request(mut self, pull_request: PullRequest) -> Self {
        self.pull_request = Some(pull_request);
        self
    }

    fn get_pull_request_details() {
    }
}

impl Component for PullRequestInfoOverlay {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
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
            let header = Paragraph::new(vec![
                Span::styled(format!("#{} in {}", pr.number, pr.repository), Style::default().fg(Color::Gray)).into(),
                (*pr.title).to_string().into(),
                format!("Opened by: {} on {}", pr.author, pr.created_at).into(),
                format!("State: {}", match &pr.state {
                    PullRequestState::CLOSED => "CLOSED".to_string(),
                    PullRequestState::MERGED => "MERGED".to_string(),
                    PullRequestState::OPEN => "OPEN".to_string(),
                    PullRequestState::Other(state) => state.clone(),
                })
                .into(),
            ])
            .style(Style::default().fg(TEXT).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Left);

            let horizontal_separator = Paragraph::new("─".repeat(area.width as usize)).style(Style::default().fg(TEXT));

            let body = Paragraph::new(&*pr.body)
                .style(Style::default().fg(TEXT))
                .alignment(Alignment::Left)
                .scroll((self.scroll_offset, 0));

            f.render_widget(header, layout[0]);
            f.render_widget(horizontal_separator, layout[1]);
            f.render_widget(body, layout[2]);
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
    }
}
