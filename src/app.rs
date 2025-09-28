use color_eyre::eyre::Result;
use crossterm::event::KeyEvent;
use ratatui::prelude::Rect;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tracing::{debug, info, trace};

use crate::{
    action::Action,
    circuit_breaker::CircuitBreaker,
    components::{
        command_palette::CommandPalette,
        keystrokes::Keystrokes,
        notifications::{Notification, Notifications},
        pull_request_info_overlay::PullRequestInfoOverlay,
        pull_request_list::PullRequestList,
        thing_list::ThingList,
        Component,
    },
    config::Config,
    event::AppEvent,
    mode::Mode,
    providers::{provider::Provider, pull_requests::PullRequestProvider, random::RandomProvider},
    things::thing::Thing,
    tui,
};

pub struct App {
    pub config: Config,
    pub tick_rate: f64,
    pub frame_rate: f64,
    pub should_quit: bool,
    pub should_suspend: bool,
    mode: Mode,
    pub last_tick_key_events: Vec<KeyEvent>,
    action_tx: UnboundedSender<Action>,
    action_rx: UnboundedReceiver<Action>,
    event_tx: UnboundedSender<AppEvent>,
    event_rx: UnboundedReceiver<AppEvent>,
    providers: Vec<Box<dyn Provider>>,
    active_provider: usize,
    components: Vec<Box<dyn Component>>,
}

impl App {
    pub fn new(tick_rate: f64, frame_rate: f64) -> Result<Self> {
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        let config = Config::new()?;
        let mode = Mode::Normal;

        let providers: Vec<Box<dyn Provider>> = vec![
            // Box::new(PullRequestProvider::new()),
            Box::new(RandomProvider::new().with_event_handler(event_tx.clone())),
        ];

        let components: Vec<Box<dyn Component>> = vec![
            Box::new(ThingList::new()),
            Box::new(CommandPalette::new().with_event_handler(event_tx.clone())),
            Box::new(CircuitBreaker::new()),
            Box::new(Notifications::default()),
            Box::new(Keystrokes::default()),
        ];

        Ok(Self {
            active_provider: 0,
            tick_rate,
            frame_rate,
            components,
            should_quit: false,
            should_suspend: false,
            config,
            mode,
            last_tick_key_events: Vec::new(),
            action_tx,
            action_rx,
            event_tx,
            event_rx,
            providers,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        let action_tx = self.action_tx.clone();
        let event_tx = self.event_tx.clone();

        let mut tui = tui::Tui::new()?.tick_rate(self.tick_rate).frame_rate(self.frame_rate);
        // tui.mouse(true);
        tui.enter()?;

        for component in self.components.iter_mut() {
            component.register_action_handler(action_tx.clone())?;
        }

        for component in self.components.iter_mut() {
            component.register_config_handler(self.config.clone())?;
        }

        for component in self.components.iter_mut() {
            component.init(tui.size()?)?;
        }

        loop {
            if let Some(e) = tui.next().await {
                match e {
                    tui::Event::Quit => action_tx.send(Action::Quit)?,
                    tui::Event::Tick => action_tx.send(Action::Tick)?,
                    tui::Event::Render => action_tx.send(Action::Render)?,
                    tui::Event::Resize(x, y) => action_tx.send(Action::Resize(x, y))?,
                    tui::Event::Key(key) => {
                        if self.mode == Mode::Command {
                        } else if let Some(keymap) = self.config.keybindings.get(&self.mode) {
                            if let Some(action) = keymap.get(&vec![key]) {
                                action_tx.send(action.clone())?;
                            } else {
                                // If the key was not handled as a single key action,
                                // then consider it for multi-key combinations.
                                self.last_tick_key_events.push(key);

                                // Check for multi-key combinations
                                if let Some(action) = keymap.get(&self.last_tick_key_events) {
                                    action_tx.send(action.clone())?;
                                }
                            }
                        };
                    },
                    _ => {},
                }
                for component in self.components.iter_mut() {
                    if let Some(action) = component.handle_events(Some(e.clone()))? {
                        action_tx.send(action)?;
                    }
                }
            }

            while let Ok(event) = self.event_rx.try_recv() {
                trace!("{event:?}");
                match event {
                    AppEvent::ProviderReturnedResult => {
                        let things = self.providers[self.active_provider].get_things()?;
                        self.get_thing_list().set_things(things)?;
                        action_tx.send(Action::Render)?;
                    },
                    AppEvent::ProviderError(err) => action_tx.send(Action::Error(err))?,
                    _ => {},
                }
            }

            while let Ok(action) = self.action_rx.try_recv() {
                if action != Action::Tick && action != Action::Render {
                    log::debug!("{action:?}");
                }
                match action {
                    Action::Tick => {
                        self.last_tick_key_events.drain(..);
                    },
                    Action::Quit => self.should_quit = true,
                    Action::Suspend => self.should_suspend = true,
                    Action::Resume => self.should_suspend = false,
                    Action::Resize(w, h) => {
                        tui.resize(Rect::new(0, 0, w, h))?;
                        tui.draw(|f| {
                            for component in self.components.iter_mut() {
                                let r = component.draw(f, f.size());
                                if let Err(e) = r {
                                    action_tx.send(Action::Error(format!("Failed to draw: {:?}", e))).unwrap();
                                }
                            }
                        })?;
                    },
                    Action::Render => {
                        tui.draw(|f| {
                            for component in self.components.iter_mut() {
                                let r = component.draw(f, f.size());
                                if let Err(e) = r {
                                    action_tx.send(Action::Error(format!("Failed to draw: {:?}", e))).unwrap();
                                }
                            }
                        })?;
                    },
                    Action::Refresh => {
                        debug!("Refreshing data from provider {}...", self.active_provider);
                        self.providers[self.active_provider].provide().await?;
                    },
                    Action::EnterCommandMode => {
                        self.action_tx.send(Action::ChangeMode(Mode::Command))?;
                    },
                    Action::ChangeMode(mode) => {
                        self.mode = mode;
                    },
                    Action::ExecuteCommand(ref cmd) => {
                        match cmd.as_str() {
                            "q" | "quit" | "exit" => self.should_quit = true,
                            cmd => {
                                if let Some(provider) = self.providers.iter().position(|p| p.commands().contains(&cmd))
                                {
                                    self.action_tx.send(Action::ChangeProvider(provider))?;
                                } else {
                                    action_tx.send(Action::Notify(Notification::Error(format!(
                                        "Unknown command: '{}'",
                                        cmd
                                    ))))?;
                                }
                            },
                        }
                    },
                    Action::ChangeProvider(idx) => {
                        if idx < self.providers.len() {
                            self.active_provider = idx;
                            info!("Switched to provider {:?}", self.providers[idx]);
                            for component in self.components.iter_mut() {
                                if let Some(action) = component.update(Action::ChangeProvider(idx))? {
                                    action_tx.send(action)?;
                                }
                            }
                        } else {
                            action_tx.send(Action::Notify(Notification::Error(format!(
                                "Provider index {} out of bounds",
                                idx
                            ))))?;
                        }
                    },
                    Action::Error(ref err) => {
                        action_tx.send(Action::Notify(Notification::Error(format!("Error: {err}"))))?;
                    },
                    _ => {},
                }
                for component in self.components.iter_mut() {
                    if let Some(action) = component.update(action.clone())? {
                        action_tx.send(action)?
                    };
                }
            }

            if self.should_suspend {
                tui.suspend()?;
                action_tx.send(Action::Resume)?;
                tui = tui::Tui::new()?.tick_rate(self.tick_rate).frame_rate(self.frame_rate);
                // tui.mouse(true);
                tui.enter()?;
            } else if self.should_quit {
                tui.stop()?;
                break;
            }
        }
        tui.exit()?;
        Ok(())
    }

    fn get_command_palette(&mut self) -> &mut CommandPalette {
        let idx = self
            .components
            .iter()
            .position(|c| c.as_any().is::<CommandPalette>())
            .expect("CommandPalette not found in components");

        self.components[idx].as_any_mut().downcast_mut::<CommandPalette>().unwrap()
    }

    fn get_thing_list(&mut self) -> &mut ThingList {
        let idx = self
            .components
            .iter()
            .position(|c| c.as_any().is::<ThingList>())
            .expect("ThingList not found in components");

        self.components[idx].as_any_mut().downcast_mut::<ThingList>().unwrap()
    }
}
