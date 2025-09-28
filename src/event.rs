#[derive(Debug)]
pub enum AppEvent {
    ProviderReturnedResult,
    ProviderError(String),
    CommandPaletteClosed,
    CommandPaletteOpened,
}
