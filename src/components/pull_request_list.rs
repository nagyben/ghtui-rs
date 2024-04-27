use std::{collections::HashMap, time::Duration};

use color_eyre::{eyre::Result, owo_colors::OwoColorize};
use crossterm::event::{KeyCode, KeyEvent};
use derivative::Derivative;
use graphql_client::GraphQLQuery;
use oauth2::{
    basic::BasicClient, reqwest::async_http_client, AuthUrl, ClientId, DeviceAuthorizationUrl, Scope,
    StandardDeviceAuthorizationResponse, TokenUrl,
};
use octocrab::Octocrab;
use ratatui::{
    prelude::*,
    widgets::{
        block::{Position, Title},
        *,
    },
};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, info};

use super::{
    info_overlay::InfoOverlay,
    pull_request::{self, pull_requests_query::PullRequestState},
};
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
pub struct PullRequestList {
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
    selected_row: usize,
    pull_requests: Vec<PullRequest>,
    username: String,
    show_info_overlay: bool,
    info_overlay: InfoOverlay,
}

impl PullRequestList {
    pub fn new() -> Self {
        Self::default()
    }

    fn get_current_user(&mut self) -> Result<()> {
        let tx = self.command_tx.clone().unwrap();
        tokio::spawn(async move {
            let token = std::env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN must be set");
            let oc = Octocrab::builder().personal_token(token).build().expect("Failed to create Octocrab client");

            let response: serde_json::Value = oc
                .graphql(&serde_json::json!({ "query": "{ viewer { login }}" }))
                .await
                .unwrap_or(serde_json::Value::Null);

            if let serde_json::Value::Null = response {
                return tx.send(Action::Error("Failed to get current user".to_string()));
            }

            let username = String::from(response["data"]["viewer"]["login"].as_str().unwrap());
            tx.send(Action::GetCurrentUserResult(username))
        });
        Ok(())
    }

    fn fetch_repos(&mut self) -> Result<()> {
        let tx = self.command_tx.clone().unwrap();
        let username = self.username.clone();
        tokio::spawn(async move {
            let token = std::env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN must be set");
            let oc = Octocrab::builder().personal_token(token).build().expect("Failed to create Octocrab client");

            if username.is_empty() {
                tx.send(Action::GetCurrentUser);
                tx.send(Action::GetRepos);
            }

            let response: octocrab::Result<graphql_client::Response<pull_requests_query::ResponseData>> = oc
                .graphql(&PullRequestsQuery::build_query(pull_requests_query::Variables {
                    first: 10,
                    query: format!("is:pr involves:{} state:open", username),
                }))
                .await;
            match response {
                Ok(response) => {
                    log::info!("{:#?}", response);
                    let r = response.data.unwrap().search.edges.unwrap();
                    let pull_requests: Vec<PullRequest> = r
                        .iter()
                        .map(|v: &Option<pull_requests_query::PullRequestsQuerySearchEdges>| {
                            let inner = v.as_ref().unwrap().node.as_ref().unwrap();
                            match inner {
                                pull_requests_query::PullRequestsQuerySearchEdgesNode::PullRequest(pr) => pr.into(),
                                _ => panic!("Unexpected node type: {:?}", inner),
                            }
                        })
                        .collect();
                    pull_requests.iter().for_each(|pr| {});
                    tx.send(Action::GetReposResult(pull_requests))
                },
                Err(e) => tx.send(Action::Error(e.to_string())),
            }
        });
        Ok(())
    }

    fn render_pull_requests_table(&mut self, f: &mut ratatui::prelude::Frame<'_>, area: Rect) {
        let rows = self
            .pull_requests
            .iter()
            .map(|pr: &PullRequest| {
                Row::new(vec![
                    Cell::from(format!("{:}", pr.number)),
                    Cell::from(pr.repository.clone()),
                    Cell::from(pr.title.clone()),
                    Cell::from(pr.author.clone()),
                    Cell::from(format!("{}", pr.created_at.format("%Y-%m-%d %H:%M"))),
                    Cell::from(format!("{}", pr.updated_at.format("%Y-%m-%d %H:%M"))),
                    Cell::from(Line::from(vec![
                        Span::styled(format!("{:+}", pr.additions), Style::new().fg(Color::LightGreen)),
                        Span::styled(format!("{:+}", (0 - pr.deletions as isize)), Style::new().fg(Color::LightRed)),
                    ])),
                    Cell::from(match pr.state {
                        pull_requests_query::PullRequestState::OPEN => {
                            if pr.is_draft {
                                "DRAFT"
                            } else {
                                "OPEN"
                            }
                        },
                        _ => "Unknown",
                    }),
                    Cell::from(Line::from(
                        pr.reviews
                            .iter()
                            .flat_map(|prr| {
                                vec![
                                    Span::styled(prr.author.clone(), match prr.state {
                                        PullRequestReviewState::COMMENTED => Style::new().fg(Color::LightBlue),
                                        PullRequestReviewState::APPROVED => Style::new().fg(Color::LightGreen),
                                        PullRequestReviewState::CHANGES_REQUESTED => {
                                            Style::new().fg(Color::LightYellow)
                                        },
                                        _ => Style::new().fg(Color::Gray),
                                    }),
                                    Span::raw(" "),
                                ]
                            })
                            .collect::<Vec<Span>>(),
                    )),
                ])
            })
            .collect::<Vec<_>>();
        let mut table_state = TableState::default();
        table_state.select(Some(self.selected_row));
        let table = Table::default()
            .widths(Constraint::from_lengths([4, 40, 80, 10, 20, 20, 6, 6, 50]))
            .rows(rows)
            .column_spacing(1)
            .header(
                Row::new(vec![
                    "#",
                    "Repository",
                    "Title",
                    "Author",
                    "Created",
                    "Updated",
                    "Changes",
                    "State",
                    "Reviews",
                ])
                .bottom_margin(1),
            )
            .block(
                Block::default()
                    .title(Title::from("Pull Requests"))
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded),
            )
            .highlight_style(Style::new().reversed().add_modifier(Modifier::BOLD))
            .highlight_symbol(">> ");

        f.render_stateful_widget(table, area, &mut table_state);
    }
}

impl Component for PullRequestList {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        if !self.show_info_overlay {
            match &action {
                Action::Tick => {},
                Action::Up => {
                    self.selected_row = self.selected_row.saturating_sub(1);
                    return Ok(Some(Action::Render));
                },
                Action::Down => {
                    self.selected_row = std::cmp::min(self.selected_row + 1, self.pull_requests.len() - 1);
                    return Ok(Some(Action::Render));
                },
                Action::GetRepos => {
                    self.fetch_repos()?;
                },
                Action::GetReposResult(pull_requests) => {
                    self.pull_requests = pull_requests.clone();
                },
                Action::Enter => {
                    if let Some(pr) = self.pull_requests.get(self.selected_row) {
                        let url = pr.url.clone();
                        let _ = open::that(url);
                    }
                },
                Action::GetCurrentUser => {
                    self.get_current_user();
                },
                Action::GetCurrentUserResult(user) => self.username = user.clone(),
                _ => {},
            }
        }
        match action {
            Action::Info => {
                if let Some(pr) = self.pull_requests.get(self.selected_row) {
                    self.info_overlay = InfoOverlay::new().with_pull_request(pr.clone());
                    self.show_info_overlay = !self.show_info_overlay;
                }
            },
            _ => {},
        }
        Ok(None)
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        self.render_pull_requests_table(f, area);

        if self.show_info_overlay {
            self.info_overlay.draw(f, area.inner(&Margin::new(4, 4)))?;
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let item_list = PullRequestList::new();
        assert_eq!(item_list.selected_row, 0);
    }

    #[test]
    fn test_up_down_actions() {
        let mut item_list = PullRequestList::new();
        assert_eq!(item_list.update(Action::Up).unwrap(), Some(Action::Render));
        assert_eq!(item_list.update(Action::Down).unwrap(), Some(Action::Render));
    }

    #[test]
    fn test_dismiss_info_overlay_actions() {
        let mut item_list = PullRequestList::new();
        assert_eq!(item_list.update(Action::Info).unwrap(), None);
        assert!(item_list.show_info_overlay);
        assert_eq!(item_list.update(Action::Info).unwrap(), None);
        assert!(!item_list.show_info_overlay)
    }
}
