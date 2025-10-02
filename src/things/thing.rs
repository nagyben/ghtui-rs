use std::{any::Any, cmp::Ordering};

use ratatui::widgets::Row;

use crate::action::Action;

pub trait Thing: Send + Sync {
    fn row(&self) -> Row<'_>;
    fn header(&self) -> Vec<&'static str>;
    fn details(&self) -> Option<Action> {
        None
    }
    fn cmp_by_column_index(&self, other: &dyn Thing, index: usize) -> std::cmp::Ordering;
    fn as_any(&self) -> &dyn Any;
    fn get_uuid(&self) -> uuid::Uuid;
}
