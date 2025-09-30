use std::any::Any;

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
use tracing::{debug, error, error_span, info};

use super::{notifications::Notification, pull_request_info_overlay::PullRequestInfoOverlay, utils::centered_rect};
use crate::{
    action::Action,
    colors::{BASE, BLUE, GREEN, LAVENDER, OVERLAY0, PEACH, PINK, RED, ROSEWATER, SURFACE0, TEXT, YELLOW},
    components::{Component, Frame},
    config::{get_keybinding_for_action, key_event_to_string, Config, KeyBindings},
    event::AppEvent,
    github::{client::GraphQLGithubClient, traits::GithubClient},
    mode::Mode,
    things::pull_request::{PullRequest, PullRequestReviewState, PullRequestState},
};

#[derive(Default)]
pub struct PullRequestList {
    command_tx: Option<UnboundedSender<Action>>,
    event_tx: Option<UnboundedSender<AppEvent>>,
    config: Config,
    selected_row: usize,
    pull_requests: Option<Vec<PullRequest>>,
    username: String,
    show_info_overlay: bool,
    info_overlay: PullRequestInfoOverlay,
    client: GraphQLGithubClient,
    selected_column: usize,
    // Pagination state
    has_next_page: bool,
    end_cursor: Option<String>,
    is_loading_more: bool,
    initial_load_size: usize,
    page_size: usize,
    table_state: TableState,
}

impl PullRequestList {
    pub fn new() -> Self {
        Self {
            initial_load_size: 10,
            page_size: 20,
            has_next_page: true,
            selected_column: 1, // sort by repo by default
            ..Default::default()
        }
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
        let initial_load_size = self.initial_load_size as i32;

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

    fn render_pull_requests_table(&mut self, f: &mut ratatui::prelude::Frame<'_>, area: Rect) {
        let mut rows: Vec<Row<'static>> = vec![];
        if let Some(pull_requests) = &self.pull_requests {
            rows = pull_requests
                .iter()
                .map(|pr: &PullRequest| {
                    Row::new(vec![
                        Cell::from(format!("{:}", pr.number)),
                        Cell::from(pr.repository.clone()),
                        Cell::from(pr.title.clone()),
                        Cell::from(pr.author.clone()),
                        Cell::from(format!("{}", pr.created_at.format("%Y-%m-%d"))),
                        Cell::from(format!("{}", pr.updated_at.format("%Y-%m-%d"))),
                        Cell::from(Line::from(vec![
                            Span::styled(format!("{:+}", pr.additions), Style::new().fg(GREEN)),
                            Span::styled(format!("{:+}", (0 - pr.deletions as isize)), Style::new().fg(RED)),
                        ])),
                        Cell::from(match pr.state {
                            PullRequestState::Open => {
                                if pr.is_draft {
                                    "DRAFT"
                                } else {
                                    "OPEN"
                                }
                            },
                            PullRequestState::Closed => "CLOSED",
                            PullRequestState::Merged => "MERGED",
                        }),
                        Cell::from(Line::from(
                            pr.reviews
                                .iter()
                                .flat_map(|prr| {
                                    vec![
                                        Span::styled(prr.author.clone(), match prr.state {
                                            PullRequestReviewState::Commented => Style::new().fg(BLUE),
                                            PullRequestReviewState::Approved => Style::new().fg(GREEN),
                                            PullRequestReviewState::ChangesRequested => Style::new().fg(YELLOW),
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

            // Add loading indicator if we're loading more PRs
            if self.is_loading_more {
                rows.push(Row::new(vec![
                    Cell::from(""),
                    Cell::from("Loading more..."),
                    Cell::from(""),
                    Cell::from(""),
                    Cell::from(""),
                    Cell::from(""),
                    Cell::from(""),
                    Cell::from(""),
                    Cell::from(""),
                ]));
            }
        }
        self.table_state.select(Some(self.selected_row));
        let table = Table::default()
            .widths(Constraint::from_lengths([4, 40, 80, 10, 12, 12, 6, 6, 50]))
            .rows(rows)
            .column_spacing(1)
            .header(
                Row::new(PullRequestList::selected_column(
                    vec!["#", "Repository", "Title", "Author", "Created", "Updated", "Changes", "State", "Reviews"],
                    self.selected_column,
                ))
                .bottom_margin(1),
            )
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(ROSEWATER)
                    .style(Style::default().bg(BASE).fg(TEXT)),
            )
            .highlight_style(Style::new().bg(SURFACE0).fg(TEXT).add_modifier(Modifier::BOLD))
            .highlight_symbol(">> ");

        f.render_stateful_widget(table, area, &mut self.table_state);
    }

    fn render_placeholder(&self, f: &mut ratatui::prelude::Frame<'_>, area: Rect) {
        // TODO: get the key bindings from the config
        let text = Paragraph::new(
            if let Some(refresh_key) =
                get_keybinding_for_action(&self.config.keybindings, Mode::Normal, &Action::Refresh)
            {
                format!("Press '{}' to refresh", key_event_to_string(&refresh_key[0]))
            } else {
                String::from("Error: refresh key not bound in the config!")
            },
        )
        .style(Style::default().fg(TEXT))
        .alignment(Alignment::Center);
        f.render_widget(text, centered_rect(area, 100, 10))
    }

    fn sort_pull_requests(&mut self) {
        if let Some(ref mut pull_requests) = self.pull_requests {
            pull_requests.sort_by(|a, b| {
                match self.selected_column {
                    0 => a.number.cmp(&b.number),
                    1 => a.repository.cmp(&b.repository),
                    2 => a.title.cmp(&b.title),
                    3 => a.author.cmp(&b.author),
                    4 => a.created_at.cmp(&b.created_at),
                    5 => a.updated_at.cmp(&b.updated_at),
                    6 => (a.additions + a.deletions).cmp(&(b.additions + b.deletions)),
                    7 => a.state.cmp(&b.state),
                    _ => a.title.cmp(&b.title),
                }
            });
        }
    }

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

impl Component for PullRequestList {
    fn init(&mut self, area: Rect) -> Result<()> {
        self.get_current_user()?;
        Ok(())
    }

    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_event_handler(&mut self, tx: UnboundedSender<AppEvent>) -> Result<()> {
        todo!();
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        // Always pass certain actions to the overlay if it exists
        match &action {
            Action::PullRequestDetailsLoaded(_) | Action::PullRequestDetailsLoadError => {
                self.info_overlay.update(action.clone())?;
            },
            _ => {},
        }

        if self.show_info_overlay {
            self.info_overlay.update(action.clone())?;
        } else {
            match &action {
                Action::Tick => {},
                Action::Up => {
                    self.selected_row = self.selected_row.saturating_sub(1);
                    return Ok(Some(Action::Render));
                },
                Action::Down => {
                    if let Some(pull_requests) = &self.pull_requests {
                        self.selected_row = std::cmp::min(self.selected_row + 1, pull_requests.len() - 1);
                    }
                    return Ok(Some(Action::Render));
                },
                Action::Left => {
                    self.selected_column = self.selected_column.saturating_sub(1);
                    self.sort_pull_requests();
                },
                Action::Right => {
                    self.selected_column = self.selected_column.saturating_add(1);
                    self.sort_pull_requests();
                },
                Action::Refresh => {
                    self.refresh();
                },
                Action::GetReposResult(pull_requests) => {
                    // Legacy action - convert to new format
                    self.pull_requests = Some(pull_requests.clone());
                    self.has_next_page = false; // Legacy mode has no pagination
                    self.end_cursor = None;
                    self.is_loading_more = false;
                },
                Action::LoadMorePullRequestsResult(new_pull_requests, has_next_page, end_cursor) => {
                    if let Some(ref mut existing_prs) = self.pull_requests {
                        // Append new PRs to existing ones
                        existing_prs.extend(new_pull_requests.clone());
                        existing_prs.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
                        existing_prs.dedup();
                    } else {
                        // First load
                        self.pull_requests = Some(new_pull_requests.clone());
                    }

                    self.has_next_page = *has_next_page;
                    self.end_cursor = end_cursor.clone();
                    self.is_loading_more = false;

                    if self.has_next_page {
                        let _ = self.load_more_pull_requests();
                    }

                    self.sort_pull_requests();
                },
                Action::Open => {
                    if let Some(pull_requests) = &self.pull_requests {
                        if let Some(pr) = pull_requests.get(self.selected_row) {
                            let url = pr.url.clone();
                            let _ = open::that(url);
                        }
                    }
                },
                Action::GetCurrentUser => {
                    let _ = self.get_current_user();
                },
                Action::GetCurrentUserResult(user) => self.username.clone_from(user),
                Action::Sort(column) => todo!(),
                _ => {},
            }
        }

        match action {
            Action::Info | Action::Enter => {
                if let Some(pull_requests) = &self.pull_requests {
                    if let Some(pr) = pull_requests.get(self.selected_row) {
                        self.info_overlay = PullRequestInfoOverlay::new().with_pull_request(pr.clone());

                        // Register the action handler for the overlay
                        if let Some(tx) = &self.command_tx {
                            let _ = self.info_overlay.register_action_handler(tx.clone());
                            let _ = self.info_overlay.register_config_handler(self.config.clone());
                        }

                        self.show_info_overlay = !self.show_info_overlay;
                    }
                }
            },
            Action::Escape | Action::Back => self.show_info_overlay = false,
            _ => (),
        }

        Ok(None)
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        self.render_pull_requests_table(f, area);
        // if GITHUB_TOKEN is not set, display a placeholder
        if std::env::var("GITHUB_TOKEN").is_err() {
            self.render_token_error(f, area);
            return Ok(());
        }

        if self.pull_requests.is_none() {
            self.render_placeholder(f, area);
        }
        if self.show_info_overlay {
            self.info_overlay.draw(f, area.inner(&Margin::new(4, 4)))?;
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
    use rstest::rstest;
    use sealed_test::prelude::*;
    use tokio::sync::mpsc;

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

    #[rstest]
    #[case(Action::Info)]
    #[case(Action::Escape)]
    #[case(Action::Back)]
    fn test_dismiss_info_overlay_actions(#[case] action: Action) {
        let mut item_list = PullRequestList::default();
        // simulate opening the info overlay
        assert_eq!(item_list.update(Action::Info).unwrap(), None);
        assert!(item_list.show_info_overlay);

        // simulate dismissing the info overlay
        assert_eq!(item_list.update(action).unwrap(), None);
        assert!(!item_list.show_info_overlay)
    }
}
