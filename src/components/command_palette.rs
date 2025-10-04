use std::any::Any;

use color_eyre::eyre::Result;
use crossterm::event::{self, KeyCode};
use ratatui::{
    prelude::Rect,
    style::{Color, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph},
};
use tokio::sync::mpsc::UnboundedSender;
use tracing::trace;

use crate::{action::Action, components::Component, event::AppEvent, mode::Mode, tui::Frame};

#[derive(Default)]
pub struct CommandPalette {
    buffer: String,
    cursor_position: usize,
    action_tx: Option<UnboundedSender<Action>>,
    event_tx: Option<UnboundedSender<AppEvent>>,
    mode: Mode,
    active: bool,
}

impl CommandPalette {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
            cursor_position: 0,
            active: false,
            ..Default::default()
        }
    }

    pub fn show(&mut self) {
        trace!("Showing command palette");
        self.active = true;
        if let Some(tx) = &self.event_tx {
            let _ = tx.send(AppEvent::CommandPaletteOpened);
        }
        if let Some(tx) = &self.action_tx {
            let _ = tx.send(Action::Render);
        }
    }

    pub fn hide(&mut self) {
        trace!("Hiding command palette");
        self.active = false;
        if let Some(tx) = &self.event_tx {
            let _ = tx.send(AppEvent::CommandPaletteClosed);
        }
    }

    pub fn with_action_handler(mut self, tx: UnboundedSender<Action>) -> Self {
        self.action_tx = Some(tx);
        self
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

    fn register_event_handler(
        &mut self,
        tx: tokio::sync::mpsc::UnboundedSender<AppEvent>,
    ) -> Result<()> {
        self.event_tx = Some(tx);
        Ok(())
    }

    fn handle_key_events(&mut self, key: event::KeyEvent) -> Result<Option<Action>> {
        if !self.is_active() {
            return Ok(None);
        }
        match key.code {
            KeyCode::Char(c) => {
                self.buffer.insert(self.cursor_position, c);
                self.cursor_position += 1;
                trace!("Buffer after insert: {}", self.buffer);
                let command = self.buffer.trim().to_string();
                if let Some(tx) = &self.action_tx {
                    tx.send(Action::Render)?;
                    if self.mode == Mode::Search {
                        tx.send(Action::ExecuteSearch(command))?;
                    }
                }
            }
            KeyCode::Esc => {
                trace!("Command palette cancelled");
                self.buffer.clear();
                self.cursor_position = 0;
                if let Some(tx) = &self.action_tx {
                    tx.send(Action::ChangeMode(Mode::Normal))?;
                    tx.send(Action::Render)?;
                }
            }
            KeyCode::Enter => {
                let command = self.buffer.trim().to_string();
                self.buffer.clear();
                self.cursor_position = 0;
                self.hide();
                if let Some(tx) = &self.action_tx {
                    match self.mode {
                        Mode::Command => tx.send(Action::ExecuteCommand(command))?,
                        Mode::Search => tx.send(Action::ExecuteSearch(command))?,
                        _ => {}
                    }
                    tx.send(Action::ChangeMode(Mode::Normal))?;
                    tx.send(Action::Render)?;
                }
            }
            KeyCode::Backspace => {
                if self.cursor_position > 0 {
                    self.buffer.remove(self.cursor_position - 1);
                    self.cursor_position -= 1;
                    let command = self.buffer.trim().to_string();
                    if let Some(tx) = &self.action_tx {
                        tx.send(Action::Render)?;
                        if self.mode == Mode::Search {
                            tx.send(Action::ExecuteSearch(command))?;
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(None)
    }

    fn handle_action(&mut self, action: Action) -> Result<()> {
        match action {
            Action::ChangeMode(mode) => {
                self.mode = mode;
                match mode {
                    Mode::Command | Mode::Search => {
                        self.show();
                    }
                    _ => self.hide(),
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn draw(&mut self, f: &mut Frame<'_>, area: Rect) -> Result<()> {
        if self.is_active() {
            let block = Block::default()
                .title("Command Palette")
                .borders(Borders::ALL);
            let paragraph = Paragraph::new(Text::from(Line::from(vec![
                match self.mode {
                    Mode::Command => Span::styled(":", Style::from(Color::Cyan)),
                    Mode::Search => Span::styled("/", Style::from(Color::Green)),
                    _ => Span::default(),
                },
                Span::styled(self.buffer.clone(), Style::default()),
            ])))
            .block(block)
            .scroll((0, 0));
            let palette_area = Rect {
                x: area.x + 2,
                y: area.y + area.height / 4,
                width: area.width - 4,
                height: 3,
            };
            f.render_widget(paragraph, palette_area);
            f.set_cursor(
                palette_area.x + self.cursor_position as u16 + 2,
                palette_area.y + 1,
            );
        }
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn is_active(&self) -> bool {
        self.active
    }
}
