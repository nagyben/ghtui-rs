use std::{
    collections::{HashMap, VecDeque},
    time::{Duration, Instant},
};

use color_eyre::Result;
use ratatui::layout::Rect;

use crate::{action::Action, components::Component, tui::Frame};

const MAX_ACTIONS_PER_WINDOW: usize = 50;
const WINDOW_DURATION: Duration = Duration::from_secs(2);

pub struct CircuitBreaker {
    action_timestamps: HashMap<String, VecDeque<Instant>>,
}

impl CircuitBreaker {
    pub fn new() -> Self {
        Self { action_timestamps: HashMap::new() }
    }

    pub fn check_action(&mut self, action: &Action) {
        let action_key: String = action.to_string();
        let now = Instant::now();

        // Clean up old timestamps outside the window
        self.cleanup_old_timestamps(now);

        // Record this action
        let timestamps = self.action_timestamps.entry(action_key.clone()).or_insert_with(VecDeque::new);
        timestamps.push_back(now);

        // Check if this action has exceeded the threshold
        if timestamps.len() > MAX_ACTIONS_PER_WINDOW {
            panic!(
                "INFINITE ACTION LOOP DETECTED!\n\
                Action '{}' was triggered {} times in the last {} seconds.\n\
                This indicates a critical bug in the application logic.\n\
                Please check your component event handlers and action dispatching code.",
                action_key,
                timestamps.len(),
                WINDOW_DURATION.as_secs()
            );
        }
    }

    fn cleanup_old_timestamps(&mut self, now: Instant) {
        let cutoff = now - WINDOW_DURATION;

        for timestamps in self.action_timestamps.values_mut() {
            while let Some(&front_time) = timestamps.front() {
                if front_time < cutoff {
                    timestamps.pop_front();
                } else {
                    break;
                }
            }
        }

        // Remove empty entries
        self.action_timestamps.retain(|_, timestamps| !timestamps.is_empty());
    }
}

impl Component for CircuitBreaker {
    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        self.check_action(&action);
        Ok(None)
    }

    fn draw(&mut self, _f: &mut Frame<'_>, _rect: Rect) -> Result<()> {
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
