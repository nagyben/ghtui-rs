use async_trait::async_trait;
use color_eyre::Result;
use ratatui::{
    text::Line,
    widgets::{Cell, Row},
};
use tokio::sync::mpsc::UnboundedSender;

use crate::{event::AppEvent, providers::provider::Provider, things::thing::Thing};

#[derive(Clone, Debug, Default)]
struct Random {
    pub uuid: uuid::Uuid,
}

impl Thing for Random {
    fn row(&self) -> ratatui::widgets::Row<'_> {
        Row::new(vec![Cell::from("herp".to_string()), Cell::from("derp".to_string()), Cell::from("flerp".to_string())])
    }

    fn header(&self) -> Vec<&'static str> {
        vec!["Header 1", "Header 2", "Header 3"]
    }

    fn details(&self) -> Option<crate::action::Action> {
        None
    }

    fn cmp_by_column_index(&self, other: &dyn Thing, index: usize) -> std::cmp::Ordering {
        todo!()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn get_uuid(&self) -> uuid::Uuid {
        self.uuid
    }
}

#[derive(Default, Debug)]
pub struct RandomProvider {
    things: Vec<Random>,
    tx: Option<UnboundedSender<AppEvent>>,
}
impl RandomProvider {
    pub fn new() -> Self {
        Self { ..Default::default() }
    }

    pub fn with_event_handler(mut self, tx: UnboundedSender<AppEvent>) -> Self {
        self.tx = Some(tx);
        self
    }
}

#[async_trait]
impl Provider for RandomProvider {
    async fn provide(&mut self) -> Result<()> {
        log::debug!("Refreshing RandomProvider");
        self.things = vec![Random::default(), Random::default(), Random::default()];

        if let Some(tx) = &self.tx {
            let _ = tx.send(AppEvent::ProviderReturnedResult);
        }

        log::debug!("RandomProvider has provided {} things", self.things.len());

        Ok(())
    }

    fn register_event_handler(&mut self, tx: UnboundedSender<AppEvent>) -> Result<()> {
        self.tx = Some(tx);
        Ok(())
    }

    fn get_things(&self) -> Result<Vec<Box<dyn Thing>>> {
        log::debug!("Getting {} things from RandomProvider", self.things.len());
        Ok(self.things.iter().map(|thing| Box::new(thing.clone()) as Box<dyn Thing>).collect())
    }

    fn commands(&self) -> Vec<&'static str> {
        vec!["random"]
    }
}
