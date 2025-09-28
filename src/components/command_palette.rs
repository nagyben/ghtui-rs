use std::any::Any;

use color_eyre::eyre::Result;
use crossterm::event::{self, KeyCode};
use ratatui::{
    prelude::Rect,
    widgets::{Block, Borders, Paragraph},
};
use tokio::sync::mpsc::UnboundedSender;
use tracing::trace;

use crate::{action::Action, components::Component, event::AppEvent, mode::Mode, tui::Frame};

#[derive(Default)]
pub struct CommandPalette {
    buffer: String,
    cursor_position: usize,
    visible: bool,
    action_tx: Option<UnboundedSender<Action>>,
    event_tx: Option<UnboundedSender<AppEvent>>,
}

impl CommandPalette {
    pub fn new() -> Self {
        Self { buffer: String::new(), cursor_position: 0, visible: false, ..Default::default() }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn show(&mut self) {
        trace!("Showing command palette");
        self.visible = true;
        if let Some(tx) = &self.event_tx {
            let _ = tx.send(AppEvent::CommandPaletteOpened);
        }
        if let Some(tx) = &self.action_tx {
            let _ = tx.send(Action::Render);
        }
    }

    pub fn hide(&mut self) {
        trace!("Hiding command palette");
        self.visible = false;
        if let Some(tx) = &self.event_tx {
            let _ = tx.send(AppEvent::CommandPaletteClosed);
        }
    }

    pub fn with_event_handler(mut self, tx: UnboundedSender<AppEvent>) -> Self {
        self.event_tx = Some(tx);
        self
    }
}

impl Component for CommandPalette {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.action_tx = Some(tx);
        Ok(())
    }

    fn register_event_handler(&mut self, tx: tokio::sync::mpsc::UnboundedSender<AppEvent>) -> Result<()> {
        self.event_tx = Some(tx);
        Ok(())
    }

    fn handle_key_events(&mut self, key: event::KeyEvent) -> Result<Option<Action>> {
        if !self.is_visible() {
            return Ok(None);
        }
        match key.code {
            KeyCode::Char(c) => {
                self.buffer.insert(self.cursor_position, c);
                self.cursor_position += 1;
                trace!("Buffer after insert: {}", self.buffer);
                Ok(None)
            },
            KeyCode::Esc => {
                self.buffer.clear();
                self.cursor_position = 0;
                if let Some(tx) = &self.action_tx {
                    let _ = tx.send(Action::ChangeMode(Mode::Normal));
                    let _ = tx.send(Action::Render);
                }
                Ok(None)
            },
            KeyCode::Enter => {
                let command = self.buffer.trim().to_string();
                self.buffer.clear();
                self.cursor_position = 0;
                self.hide();
                if let Some(tx) = &self.action_tx {
                    let _ = tx.send(Action::ExecuteCommand(command));
                    let _ = tx.send(Action::ChangeMode(Mode::Normal));
                    let _ = tx.send(Action::Render);
                }
                Ok(None)
            },
            KeyCode::Backspace => {
                if self.cursor_position > 0 {
                    self.buffer.remove(self.cursor_position - 1);
                    self.cursor_position -= 1;
                }
                Ok(None)
            },
            _ => Ok(None),
        }
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::ChangeMode(mode) => {
                match mode {
                    Mode::Command | Mode::Search => self.show(),
                    _ => self.hide(),
                }
                Ok(None)
            },
            _ => Ok(None),
        }
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        if self.is_visible() {
            let block = Block::default().title("Command Palette").borders(Borders::ALL);
            let paragraph = Paragraph::new(self.buffer.clone()).block(block).scroll((0, 0));
            let palette_area = Rect { x: area.x + 2, y: area.y + area.height / 4, width: area.width - 4, height: 3 };
            f.render_widget(paragraph, palette_area);
            f.set_cursor(palette_area.x + self.cursor_position as u16 + 1, palette_area.y + 1);
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
