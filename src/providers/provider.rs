use std::fmt::Debug;

use async_trait::async_trait;
use color_eyre::Result;
use tokio::sync::mpsc::UnboundedSender;

use crate::{action::Action, event::AppEvent, things::thing::Thing};

#[async_trait]
pub trait Provider: Debug {
    async fn provide(&mut self) -> Result<()>;

    fn get_things(&self) -> Result<Vec<Box<dyn Thing>>>;

    #[allow(unused_variables)]
    fn register_event_handler(&mut self, tx: UnboundedSender<AppEvent>) -> Result<()> {
        Ok(())
    }

    fn commands(&self) -> Vec<&'static str>;
}
