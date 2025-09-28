use ratatui::widgets::Row;

use crate::action::Action;

pub trait Thing: Send + Sync {
    fn row(&self) -> Row<'_>;
    fn header(&self) -> Vec<&'static str>;
    fn details(&self) -> Option<Action> {
        None
    }
}
