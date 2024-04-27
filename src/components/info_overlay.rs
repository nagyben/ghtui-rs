use std::{alloc::Layout, collections::HashMap, time::Duration};

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
pub struct InfoOverlay {
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
    pull_request: Option<PullRequest>,
}

impl InfoOverlay {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_pull_request(mut self, pull_request: PullRequest) -> Self {
        self.pull_request = Some(pull_request);
        self
    }
}

impl Component for InfoOverlay {
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
            Action::Up => {},
            Action::Down => {},
            Action::Enter => {},
            _ => {},
        }
        Ok(None)
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::LightCyan).bg(Color::Black));
        f.render_widget(Clear, area);
        f.render_widget(block, area);

        let layout =
            layout::Layout::new(Direction::Vertical, [Constraint::Min(1), Constraint::Length(2), Constraint::Min(1)])
                .split(area.inner(&Margin { horizontal: 1, vertical: 1 }));

        if let Some(pr) = &self.pull_request {
            let leader = Paragraph::new(format!("#{} in {}", pr.number, pr.repository))
                .style(Style::default().fg(Color::LightCyan))
                .alignment(Alignment::Center);
            let title =
                Paragraph::new(&*pr.title).style(Style::default().fg(Color::LightCyan)).alignment(Alignment::Left);

            let opened_by = Paragraph::new(format!("Opened by: {} on {}", pr.author, pr.created_at))
                .style(Style::default().fg(Color::LightCyan))
                .alignment(Alignment::Left);

            f.render_widget(title, layout[0]);
            f.render_widget(opened_by, layout[1]);
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
