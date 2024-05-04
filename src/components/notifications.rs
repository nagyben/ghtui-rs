use std::time::Instant;

use color_eyre::eyre::Result;
use ratatui::{prelude::*, widgets::*};
use serde::{Deserialize, Serialize};
use strum::Display;
use tracing::debug;

use super::Component;
use crate::{action::Action, tui::Frame};

#[derive(Debug, Clone, Default)]
pub struct Notifications {
    notifications: Vec<NotificationWithTimestamp>,
}

type NotificationWithTimestamp = (Notification, Instant);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Display, Deserialize)]
pub enum Notification {
    Info(String),
    Warning(String),
    Error(String),
}

impl Notifications {
    fn app_tick(&mut self) -> Result<()> {
        Ok(())
    }

    fn render_tick(&mut self) -> Result<()> {
        Ok(())
    }
}

impl Component for Notifications {
    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::Tick => self.app_tick()?,
            Action::Render => self.render_tick()?,
            Action::Notify(notification) => {
                self.notifications.push((notification, Instant::now()));
            },
            Action::Up | Action::Down => {
                self.notifications.push((Notification::Info(action.to_string()), Instant::now()));
            },
            _ => (),
        };
        Ok(None)
    }

    fn draw(&mut self, f: &mut Frame<'_>, rect: Rect) -> Result<()> {
        let notifications = List::new(
            self.notifications
                .iter()
                .map(|n| {
                    match &n.0 {
                        Notification::Info(s) => s,
                        Notification::Warning(s) => s,
                        Notification::Error(s) => s,
                    }
                })
                .map(|s| ListItem::new(Text::from(s.clone()).style(Style::default().red()).alignment(Alignment::Right)))
                .collect::<Vec<_>>(),
        )
        .block(Block::default());

        debug!("Drawing notifications: {:?}", self.notifications);

        let rect = Rect::new(rect.width.saturating_sub(40), 0, 40, 10);

        f.render_widget(notifications, rect);
        Ok(())
    }
}
