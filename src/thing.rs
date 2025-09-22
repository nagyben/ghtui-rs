use ratatui::widgets::Row;

pub trait Thing {
    fn render_row(&self) -> Row<'_>;
}
