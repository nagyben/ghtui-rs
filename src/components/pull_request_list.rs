use std::sync::{Arc, RwLock};

use color_eyre::{eyre::Result, owo_colors::OwoColorize};
use crossterm::event::{KeyCode, KeyEvent};
use derivative::Derivative;
use graphql_client::GraphQLQuery;
use oauth2::{
    basic::BasicClient, reqwest::async_http_client, AuthUrl, ClientId, DeviceAuthorizationUrl,
    Scope, StandardDeviceAuthorizationResponse, TokenUrl,
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
use tracing::{debug, error, error_span, info, trace};

use super::{
    notifications::Notification, pull_request_info_overlay::PullRequestInfoOverlay,
    utils::centered_rect,
};
use crate::{
    action::Action,
    colors::{
        BASE, BLUE, GREEN, LAVENDER, OVERLAY0, PEACH, PINK, RED, ROSEWATER, SURFACE0, TEXT, YELLOW,
    },
    components::{
        pull_request::{PullRequest, PullRequestReviewState, PullRequestState},
        Component, Frame,
    },
    config::{get_keybinding_for_action, key_event_to_string, Config, KeyBindings},
    event::AppEvent,
    github::{client::GraphQLGithubClient, traits::GithubClient},
    mode::Mode,
};

#[derive(Default)]
pub struct PullRequestList {
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
    selected_row: usize,
    pull_requests: Arc<RwLock<Vec<PullRequest>>>,
    pull_requests_view: Vec<PullRequest>,
    username: Option<String>,
    show_info_overlay: bool,
    info_overlay: PullRequestInfoOverlay,
    client: GraphQLGithubClient,
    selected_column: usize,
    initial_load_size: usize,
    page_size: usize,
    table_state: TableState,
    action_tx: Option<UnboundedSender<Action>>,
    event_tx: Option<UnboundedSender<AppEvent>>,
    active: bool,
}

impl PullRequestList {
    pub fn new() -> Self {
        Self {
            initial_load_size: 10,
            page_size: 20,
            selected_column: 1, // sort by repo by default
            ..Default::default()
        }
    }

    fn get_current_user(&mut self) -> Result<()> {
        let action_tx = self.action_tx.clone().unwrap();
        let event_tx = self.event_tx.clone().unwrap();
        action_tx.send(Action::Notify(Notification::Info(String::from(
            "Getting current user...",
        ))))?;
        tokio::spawn(async move {
            match GraphQLGithubClient::get_current_user().await {
                Ok(username) => {
                    trace!("herpyderp");
                    let _ = action_tx.send(Action::Notify(Notification::Info(format!(
                        "Got user {username}"
                    ))));
                    let _ = event_tx.send(AppEvent::UserIdentified(username));
                }
                Err(err) => {
                    error!("Error getting current user: {:?}", err);
                    let _ = action_tx.send(Action::Error(format!("{:#}", err)));
                }
            }
        });
        Ok(())
    }

    fn refresh(&mut self) {
        trace!("Refreshing pull requests...");

        if self.username.is_none() {
            let _ = self.get_current_user();
        } else if let Some(username) = &self.username {
            let username = username.clone();
            let pull_requests = Arc::clone(&self.pull_requests);
            let page_size = self.page_size;
            let action_tx = self.action_tx.clone();
            let event_tx = self.event_tx.clone();

            tokio::spawn(async move {
                let _ = Self::load_all_pull_requests(
                    username,
                    pull_requests,
                    page_size,
                    action_tx,
                    event_tx,
                )
                .await;
            });
        }
    }

    async fn load_all_pull_requests(
        username: String,
        pull_requests: Arc<RwLock<Vec<PullRequest>>>,
        page_size: usize,
        action_tx: Option<UnboundedSender<Action>>,
        event_tx: Option<UnboundedSender<AppEvent>>,
    ) -> Result<()> {
        let mut has_next_page = true;
        let mut end_cursor: Option<String> = None;

        while has_next_page {
            if let Some(tx) = &action_tx {
                trace!("Notifying loading pull requests...");
                tx.send(Action::Notify(Notification::Info(
                    "Loading pull requests...".to_string(),
                )))?;
            }
            match GraphQLGithubClient::get_pull_requests_paginated(
                username.clone(),
                page_size as i32,
                end_cursor.clone(),
            )
            .await
            {
                Ok((new_pull_requests, next_page, cursor)) => {
                    if let Ok(mut prs) = pull_requests.write() {
                        prs.extend(new_pull_requests);

                        // dedup needs a sorted vec
                        prs.sort();
                        prs.dedup();
                    }

                    // Emit event after each page so UI updates incrementally
                    if let Some(tx) = &event_tx {
                        let _ = tx.send(AppEvent::ProviderReturnedResult);
                    }

                    has_next_page = next_page;
                    end_cursor = cursor;
                }
                Err(err) => {
                    error!("Error loading pull requests: {:?}", err);
                    if let Some(tx) = &action_tx {
                        let _ = tx.send(Action::Error(err.to_string()));
                    }
                    break;
                }
            }
        }

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
        let rows = self
            .pull_requests_view
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
                        Span::styled(
                            format!("{:+}", (0 - pr.deletions as isize)),
                            Style::new().fg(RED),
                        ),
                    ])),
                    Cell::from(match pr.state {
                        PullRequestState::Open => {
                            if pr.is_draft {
                                "DRAFT"
                            } else {
                                "OPEN"
                            }
                        }
                        PullRequestState::Closed => "CLOSED",
                        PullRequestState::Merged => "MERGED",
                    }),
                    Cell::from(Line::from(
                        pr.reviews
                            .iter()
                            .flat_map(|prr| {
                                vec![
                                    Span::styled(
                                        prr.author.clone(),
                                        match prr.state {
                                            PullRequestReviewState::Commented => {
                                                Style::new().fg(BLUE)
                                            }
                                            PullRequestReviewState::Approved => {
                                                Style::new().fg(GREEN)
                                            }
                                            PullRequestReviewState::ChangesRequested => {
                                                Style::new().fg(YELLOW)
                                            }
                                            _ => Style::new().fg(Color::Gray),
                                        },
                                    ),
                                    Span::raw(" "),
                                ]
                            })
                            .collect::<Vec<Span>>(),
                    )),
                ])
            })
            .collect::<Vec<_>>();
        self.table_state.select(Some(self.selected_row));
        let table = Table::default()
            .widths(Constraint::from_lengths([4, 40, 80, 10, 12, 12, 6, 6, 50]))
            .rows(rows)
            .column_spacing(1)
            .header(
                Row::new(PullRequestList::selected_column(
                    vec![
                        "#",
                        "Repository",
                        "Title",
                        "Author",
                        "Created",
                        "Updated",
                        "Changes",
                        "State",
                        "Reviews",
                    ],
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
            .highlight_style(
                Style::new()
                    .bg(SURFACE0)
                    .fg(PEACH)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        f.render_stateful_widget(table, area, &mut self.table_state);
    }

    fn render_placeholder(&self, f: &mut ratatui::prelude::Frame<'_>, area: Rect) {
        // TODO: get the key bindings from the config
        let text = Paragraph::new(
            if let Some(refresh_key) =
                get_keybinding_for_action(&self.config.keybindings, Mode::Normal, &Action::Refresh)
            {
                format!(
                    "Press '{}' to refresh",
                    key_event_to_string(&refresh_key[0])
                )
            } else {
                String::from("Error: refresh key not bound in the config!")
            },
        )
        .style(Style::default().fg(TEXT))
        .alignment(Alignment::Center);
        f.render_widget(text, centered_rect(area, 100, 10))
    }

    fn sort_pull_requests(&mut self) {
        let mut pull_requests = self.pull_requests.write().unwrap();
        pull_requests.sort_by(|a, b| match self.selected_column {
            0 => a.number.cmp(&b.number),
            1 => a.repository.cmp(&b.repository),
            2 => a.title.cmp(&b.title),
            3 => a.author.cmp(&b.author),
            4 => a.created_at.cmp(&b.created_at),
            5 => a.updated_at.cmp(&b.updated_at),
            6 => (a.additions + a.deletions).cmp(&(b.additions + b.deletions)),
            7 => a.state.cmp(&b.state),
            _ => a.title.cmp(&b.title),
        });
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

    fn filter_pull_requests(&mut self, search_string: Option<&str>) {
        match search_string {
            Some(search_string) => {
                trace!(
                    "Filtering pull requests with search string: {}",
                    search_string
                );
                let all_prs = self.pull_requests.read().unwrap();
                self.pull_requests_view = if search_string.is_empty() {
                    all_prs.iter().cloned().collect()
                } else {
                    all_prs
                        .iter()
                        .filter(|pr| pr.filter(search_string))
                        .cloned()
                        .collect()
                };
            }
            None => self.pull_requests_view = self.pull_requests.read().unwrap().clone(),
        }
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

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn handle_action(&mut self, action: Action) -> Result<()> {
        // Always pass certain actions to the overlay if it exists
        match &action {
            Action::PullRequestDetailsLoaded(_) | Action::PullRequestDetailsLoadError => {
                self.info_overlay.handle_action(action.clone())?;
            }
            _ => {}
        }

        if self.show_info_overlay {
            self.info_overlay.handle_action(action.clone())?;
        } else {
            match &action {
                Action::Tick => {}
                Action::Up => {
                    if !self.pull_requests_view.is_empty() {
                        self.selected_row = self.selected_row.saturating_sub(1);
                    }
                }
                Action::Down => {
                    if !self.pull_requests_view.is_empty() {
                        self.selected_row =
                            std::cmp::min(self.selected_row + 1, self.pull_requests_view.len() - 1);
                    }
                }
                Action::Left => {
                    self.selected_column = self.selected_column.saturating_sub(1);
                    self.sort_pull_requests();
                }
                Action::Right => {
                    self.selected_column = self.selected_column.saturating_add(1);
                    self.sort_pull_requests();
                }
                Action::Refresh => {
                    self.refresh();
                }
                Action::Open => {
                    if let Some(pr) = self.pull_requests_view.get(self.selected_row) {
                        let url = pr.url.clone();
                        trace!("Opening URL: {}", url);
                        let _ = open::that(url);
                    }
                }
                Action::Sort(column) => todo!(),
                Action::ExecuteSearch(search_string) => {
                    self.filter_pull_requests(Some(search_string))
                }
                _ => {}
            }
        }

        match action {
            Action::Info | Action::Enter => {
                let pull_requests = self.pull_requests.read().unwrap();
                let action_tx = self.action_tx.clone().unwrap();
                if let Some(pr) = pull_requests.get(self.selected_row) {
                    self.info_overlay = PullRequestInfoOverlay::new(pr.clone(), action_tx);
                    self.info_overlay.load();

                    // Register the action handler for the overlay
                    if let Some(tx) = &self.command_tx {
                        let _ = self.info_overlay.register_action_handler(tx.clone());
                        let _ = self
                            .info_overlay
                            .register_config_handler(self.config.clone());
                    }

                    self.show_info_overlay = !self.show_info_overlay;
                }
            }
            Action::Escape | Action::Back => {
                self.show_info_overlay = false;
                self.filter_pull_requests(None);
            }
            _ => {}
        }

        Ok(())
    }

    fn handle_app_event(&mut self, event: AppEvent) -> Result<()> {
        match event {
            AppEvent::UserIdentified(username) => {
                self.username = Some(username);
                self.refresh();
            }
            AppEvent::ProviderReturnedResult => {
                {
                    let pull_requests = self.pull_requests.read().unwrap();
                    self.pull_requests_view = pull_requests.clone();
                }
                self.sort_pull_requests();
            }
            _ => {}
        }
        Ok(())
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        self.render_pull_requests_table(f, area);
        // if GITHUB_TOKEN is not set, display a placeholder
        if std::env::var("GITHUB_TOKEN").is_err() {
            self.render_token_error(f, area);
            return Ok(());
        }

        {
            let pull_requests = self.pull_requests.read().unwrap();
            if pull_requests.is_empty() {
                self.render_placeholder(f, area);
            }
        }

        if self.show_info_overlay {
            self.info_overlay.draw(f, area.inner(&Margin::new(4, 4)))?;
        }

        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn is_active(&self) -> bool {
        self.active
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

    #[rstest]
    #[case(Action::Info)]
    #[case(Action::Escape)]
    #[case(Action::Back)]
    fn test_dismiss_info_overlay_actions(#[case] action: Action) {
        let mut item_list = PullRequestList::default();
        // simulate opening the info overlay
        assert_eq!(item_list.handle_action(Action::Info).unwrap(), ());
        assert!(item_list.show_info_overlay);

        // simulate dismissing the info overlay
        assert_eq!(item_list.handle_action(action).unwrap(), ());
        assert!(!item_list.show_info_overlay)
    }
}
