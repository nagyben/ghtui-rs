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
            .border_style(Style::default().fg(Color::White).bg(Color::Black));
        f.render_widget(Clear, area);
        f.render_widget(block, area);

        let layout = layout::Layout::new(Direction::Vertical, [
            Constraint::Min(1),          // Title
            Constraint::Length(2),       // Opened by X on Y
            Constraint::Percentage(100), // body
        ])
        .split(area.inner(&Margin { horizontal: 1, vertical: 1 }));

        if let Some(pr) = &self.pull_request {
            let leader = Paragraph::new(format!("#{} in {}", pr.number, pr.repository))
                .style(Style::default().fg(Color::White))
                .alignment(Alignment::Center);
            let title = Paragraph::new(&*pr.title).style(Style::default().fg(Color::White)).alignment(Alignment::Left);

            let opened_by = Paragraph::new(format!("Opened by: {} on {}", pr.author, pr.created_at))
                .style(Style::default().fg(Color::White))
                .alignment(Alignment::Left);

            let body = Paragraph::new(&*pr.body)
                .style(Style::default().fg(Color::White))
                .alignment(Alignment::Left)
                .scroll((self.scroll_offset, 0));

            f.render_widget(title, layout[0]);
            f.render_widget(opened_by, layout[1]);
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
