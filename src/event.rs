#[derive(Debug, Clone)]
pub enum AppEvent {
    ProviderReturnedResult,
    ProviderError(String),
    CommandPaletteClosed,
    CommandPaletteOpened,
    UserIdentified(String),
}
