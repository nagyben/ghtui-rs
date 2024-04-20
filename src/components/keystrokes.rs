use std::time::Instant;

use color_eyre::eyre::Result;
use crossterm::event::KeyEvent;
use ratatui::{prelude::*, widgets::*};

use super::Component;
use crate::{action::Action, config::key_event_to_string, tui::Frame};

const COOLDOWN: f64 = 1.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Keystrokes {
    timer: Instant,
    key_history: Vec<KeyEvent>,
}

impl Default for Keystrokes {
    fn default() -> Self {
        Self::new()
    }
}

impl Keystrokes {
    pub fn new() -> Self {
        Self { timer: Instant::now(), key_history: Vec::new() }
    }

    fn app_tick(&mut self) -> Result<()> {
        let now = Instant::now();
        let elapsed = (now - self.timer).as_secs_f64();
        if elapsed >= COOLDOWN {
            self.key_history.clear();
        }
        Ok(())
    }

    fn format_key_events(key_events: &[KeyEvent]) -> String {
        key_events.iter().map(key_event_to_string).collect()
    }
}

impl Component for Keystrokes {
    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        if let Action::Tick = action {
            self.app_tick()?
        };
        Ok(None)
    }

    fn draw(&mut self, f: &mut Frame<'_>, rect: Rect) -> Result<()> {
        let s = Keystrokes::format_key_events(&self.key_history);
        let block = Block::default().title(block::Title::from(s.dim()).alignment(Alignment::Right));
        let rect = Rect::new(rect.x, rect.height - 1, rect.width, 1);
        f.render_widget(block, rect);
        Ok(())
    }

    fn handle_key_events(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        self.key_history.push(key);
        self.timer = Instant::now();
        Ok(Some(Action::Render))
    }
}
