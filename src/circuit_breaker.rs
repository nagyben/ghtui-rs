use std::{
    collections::{HashMap, HashSet, VecDeque},
    time::{Duration, Instant},
};

use color_eyre::Result;
use ratatui::layout::Rect;

use crate::{action::Action, components::Component, tui::Frame};

const MAX_ACTIONS_PER_WINDOW: usize = 50;
const WINDOW_DURATION: Duration = Duration::from_secs(2);

pub struct CircuitBreaker {
    action_timestamps: HashMap<String, VecDeque<Instant>>,
    excluded_actions: HashSet<String>,
}

impl CircuitBreaker {
    pub fn new() -> Self {
        let excluded_actions = HashSet::from([
            Action::Render.to_string(),
            Action::Tick.to_string(),
            Action::Up.to_string(),
            Action::Down.to_string(),
            Action::Left.to_string(),
            Action::Right.to_string(),
        ]);

        Self { action_timestamps: HashMap::new(), excluded_actions }
    }

    pub fn exclude_action(&mut self, action: Action) {
        self.excluded_actions.insert(action.to_string());
    }

    pub fn include_action(&mut self, action: Action) {
        self.excluded_actions.remove(&action.to_string());
    }

    pub fn check_action(&mut self, action: &Action) {
        let action_key: String = action.to_string();

        // Skip checking if this action is excluded
        if self.excluded_actions.contains(&action_key) {
            return;
        }

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
    fn handle_action(&mut self, action: Action) -> Result<()> {
        self.check_action(&action);
        Ok(())
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

    fn is_active(&self) -> bool {
        true
    }
}
