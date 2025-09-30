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
    github::{client::GraphQLGithubClient, traits::GithubClient},
    mode::Mode,
    things::thing::Thing,
};

#[derive(Default)]
pub struct ThingList {
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
    selected_row: usize,
    things: Option<Vec<Box<dyn Thing>>>,
    username: String,
    show_info_overlay: bool,
    selected_column: usize,
    // Pagination state
    table_state: TableState,
}

impl ThingList {
    pub fn new() -> Self {
        Self {
            selected_column: 1, // sort by repo by default
            ..Default::default()
        }
    }

    fn selected_column(columns: Vec<&'_ str>, selected_column: usize) -> Vec<Cell<'_>> {
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

    pub fn set_things(&mut self, things: Vec<Box<dyn Thing>>) -> Result<()> {
        self.things = Some(things);
        Ok(())
    }

    fn sort_things(&mut self) {
        todo!()
    }

    fn render_placeholder(&self, f: &mut ratatui::prelude::Frame<'_>, area: Rect) {
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

    fn render_things(&mut self, f: &mut ratatui::Frame<'_>, area: Rect) {
        let mut rows: Vec<Row> = vec![];
        if let Some(things) = &self.things {
            rows = things
                .iter()
                .map(|pr| {
                    let h = pr.as_ref().row();
                    h
                })
                .collect::<Vec<_>>();
        }
        self.table_state.select(Some(self.selected_row));
        let mut table = Table::default()
            .widths(Constraint::from_lengths([4, 40, 80, 10, 12, 12, 6, 6, 50]))
            .rows(rows)
            .column_spacing(1)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(ROSEWATER)
                    .style(Style::default().bg(BASE).fg(TEXT)),
            )
            .highlight_style(Style::new().bg(SURFACE0).fg(TEXT).add_modifier(Modifier::BOLD))
            .highlight_symbol(">> ");

        if let Some(things) = &self.things {
            if let Some(first_thing) = things.first() {
                table = table.header(
                    Row::new(Self::selected_column(first_thing.header(), self.selected_column))
                        .style(Style::new().fg(LAVENDER))
                        .bottom_margin(1)
                        .bottom_margin(1),
                );
            }
        }

        f.render_stateful_widget(table, area, &mut self.table_state);
    }
}

impl Component for ThingList {
    fn init(&mut self, area: Rect) -> Result<()> {
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

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match &action {
            Action::Tick => {},
            Action::Up => {
                self.selected_row = self.selected_row.saturating_sub(1);
                log::debug!("Selected row: {}", self.selected_row);
                return Ok(Some(Action::Render));
            },
            Action::Down => {
                if let Some(things) = &self.things {
                    self.selected_row = std::cmp::min(self.selected_row + 1, things.len() - 1);
                }
                log::debug!("Selected row: {}", self.selected_row);
                return Ok(Some(Action::Render));
            },
            Action::Left => {
                self.selected_column = self.selected_column.saturating_sub(1);
                self.sort_things();
            },
            Action::Right => {
                self.selected_column = self.selected_column.saturating_add(1);
                self.sort_things();
            },
            Action::Refresh => {},
            Action::Open => {},
            _ => {},
        }

        Ok(None)
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        self.render_things(f, area);
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
        let item_list = ThingList::new();
        assert_eq!(item_list.selected_row, 0);
    }

    #[test]
    fn test_up_down_actions() {
        let mut item_list = ThingList::new();
        assert_eq!(item_list.update(Action::Up).unwrap(), Some(Action::Render));
        assert_eq!(item_list.update(Action::Down).unwrap(), Some(Action::Render));
    }

    #[rstest]
    #[case(Action::Info)]
    #[case(Action::Escape)]
    #[case(Action::Back)]
    fn test_dismiss_info_overlay_actions(#[case] action: Action) {
        let mut item_list = ThingList::default();
        // simulate opening the info overlay
        assert_eq!(item_list.update(Action::Info).unwrap(), None);
        assert!(item_list.show_info_overlay);

        // simulate dismissing the info overlay
        assert_eq!(item_list.update(action).unwrap(), None);
        assert!(!item_list.show_info_overlay)
    }
}
