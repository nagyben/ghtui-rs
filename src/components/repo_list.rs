use std::{collections::HashMap, time::Duration};

use color_eyre::eyre::Result;
use crossterm::event::{KeyCode, KeyEvent};
use octocrab::Octocrab;
use ratatui::{prelude::*, widgets::*};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

use super::{Component, Frame};
use crate::{
    action::Action,
    config::{Config, KeyBindings},
};

#[derive(Default)]
pub struct RepoList {
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
    selected_row: usize,
}

impl RepoList {
    pub fn new() -> Self {
        Self::default()
    }

    fn fetch_repos(&mut self) -> Result<()> {
        let token = std::env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN must be set");
        let oc = Octocrab::builder().personal_token(token).build().expect("Failed to create Octocrab client");
        let repos = oc.graphql({}).await?;
        Ok(())
    }
}

impl Component for RepoList {
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
                self.selected_row = self.selected_row.saturating_sub(1);
                return Ok(Some(Action::Render));
            },
            Action::Down => {
                self.selected_row = self.selected_row.saturating_add(1);
                return Ok(Some(Action::Render));
            },
            _ => {},
        }
        Ok(None)
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        let rows = vec![Row::new(vec!["Cell11", "Cell12"]); 10];
        let mut table_state = TableState::default();
        table_state.select(Some(self.selected_row));
        let table = Table::default()
            .widths(Constraint::from_lengths([5, 5]))
            .rows(rows)
            .column_spacing(1)
            .header(Row::new(vec!["Col1", "Col2"]).bottom_margin(1))
            .footer(Row::new(vec!["Footer1", "Footer2"]))
            .block(Block::default().title("Title"))
            .highlight_style(Style::new().reversed().add_modifier(Modifier::BOLD))
            .highlight_symbol(">> ");

        f.render_stateful_widget(table, area, &mut table_state);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let item_list = RepoList::new();
        assert_eq!(item_list.selected_row, 0);
    }

    #[test]
    fn test_up_down_actions() {
        let mut item_list = RepoList::new();
        assert_eq!(item_list.update(Action::Up).unwrap(), Some(Action::Render));
        assert_eq!(item_list.update(Action::Down).unwrap(), Some(Action::Render));
    }
}
