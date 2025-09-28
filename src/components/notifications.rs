use std::{any::Any, time::Instant};

use color_eyre::{
    eyre::Result,
    owo_colors::{colors::CustomColor, OwoColorize},
};
use ratatui::{prelude::*, widgets::*};
use serde::{Deserialize, Serialize};
use strum::Display;
use tracing::debug;

use super::Component;
use crate::{
    action::Action,
    colors::{BASE, BLUE, PINK, YELLOW},
    tui::Frame,
};

#[derive(Debug, Clone, Default)]
pub struct Notifications {
    notifications: Vec<NotificationWithTimestamp>,
}

type NotificationWithTimestamp = (Notification, Instant);

const NOTIFICATION_DURATION: u64 = 5;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Display, Deserialize)]
pub enum Notification {
    Info(String),
    Warning(String),
    Error(String),
}

impl Notifications {
    fn app_tick(&mut self) -> Result<()> {
        let now = Instant::now();
        self.notifications.retain(|(_, timestamp)| timestamp.elapsed().as_secs() < NOTIFICATION_DURATION);
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
            _ => (),
        };
        Ok(None)
    }

    fn draw(&mut self, f: &mut Frame<'_>, rect: Rect) -> Result<()> {
        if !self.notifications.is_empty() {
            for (i, notification) in self.notifications.iter().enumerate() {
                let notification_text = Text::from(
                    match &notification.0 {
                        Notification::Info(s) => s,
                        Notification::Warning(s) => s,
                        Notification::Error(s) => s,
                    }
                    .clone(),
                )
                .style(
                    Style::default()
                        .fg(match &notification.0 {
                            Notification::Info(_) => BLUE,
                            Notification::Warning(_) => YELLOW,
                            Notification::Error(_) => PINK,
                        })
                        .bg(BASE),
                )
                .alignment(Alignment::Right);

                let rect = Rect::new(
                    rect.width - notification_text.width() as u16,
                    rect.y + i as u16 + 1,
                    notification_text.width() as u16,
                    1,
                );

                f.render_widget(Clear, rect);
                f.render_widget(notification_text, rect);
            }
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
