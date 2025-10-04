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
        Component,
    },
    config::Config,
    event::AppEvent,
    mode::Mode,
    tui,
};

pub struct App {
    pub config: Config,
    pub tick_rate: f64,
    pub frame_rate: f64,
    pub components: Vec<Box<dyn Component>>,
    pub should_quit: bool,
    pub should_suspend: bool,
    pub mode: Mode,
    pub last_tick_key_events: Vec<KeyEvent>,
    action_tx: UnboundedSender<Action>,
    action_rx: UnboundedReceiver<Action>,
    event_tx: UnboundedSender<AppEvent>,
    event_rx: UnboundedReceiver<AppEvent>,
}

impl App {
    pub fn new(tick_rate: f64, frame_rate: f64) -> Result<Self> {
        let config = Config::new()?;
        let mode = Mode::Normal;
        let (action_tx, action_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        let components: Vec<Box<dyn Component>> = vec![
            Box::new(
                PullRequestList::new().with_event_handler(event_tx.clone()).with_action_handler(action_tx.clone()),
            ),
            Box::new(CommandPalette::new().with_event_handler(event_tx.clone()).with_action_handler(action_tx.clone())),
            Box::new(CircuitBreaker::new()),
            Box::new(Notifications::default()),
            Box::new(Keystrokes::default()),
        ];
        Ok(Self {
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
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        let action_tx = self.action_tx.clone();
        let action_rx = &mut self.action_rx;
        let event_tx = self.event_tx.clone();
        let event_rx = &mut self.event_rx;

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
            tokio::select! {
                biased;
                Some(e) = tui.next()=> {
                    match e {
                        tui::Event::Quit => action_tx.send(Action::Quit)?,
                        tui::Event::Tick => action_tx.send(Action::Tick)?,
                        tui::Event::Render => action_tx.send(Action::Render)?,
                        tui::Event::Resize(x, y) => action_tx.send(Action::Resize(x, y))?,
                        tui::Event::Key(key) => {
                            if self.mode == Mode::Command {
                                for component in self.components.iter_mut() {
                                    if let Some(cmd_palette) = component.as_any_mut().downcast_mut::<CommandPalette>() {
                                        cmd_palette.handle_key_events(key)?;
                                    }
                                }
                                continue;
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

                Some(action) = action_rx.recv() => {
                    if action != Action::Tick && action != Action::Render {
                        trace!("Action::{action:?}");
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
                        Action::Error(ref err) => {
                            action_tx.send(Action::Notify(Notification::Error(format!("Error: {err}"))))?;
                        },
                        Action::ChangeMode(mode) => {
                            trace!("Changing mode to: {mode:?}");
                            self.mode = mode;
                            event_tx.send(AppEvent::ModeChanged(mode))?;
                        },
                        Action::OpenCommandPalette => {
                            action_tx.send(Action::ChangeMode(Mode::Command))?;
                            action_tx.send(Action::Render)?;
                        },

                        Action::OpenSearchPalette => {
                            action_tx.send(Action::ChangeMode(Mode::Search))?;
                            action_tx.send(Action::Render)?;
                        },
                        Action::ExecuteCommand(ref command) => {}
                        _ => {},
                    }
                    for component in self.components.iter_mut() {
                        component.handle_action(action.clone())?;
                    }
                }

                Some(event) = self.event_rx.recv() =>{
                    trace!("AppEvent::{event:?}");
                    match event {
                        AppEvent::ModeChanged(new_mode) => {
                            info!("Changing mode to: {new_mode:?}");
                            self.mode = new_mode;
                        },
                        AppEvent::Quit => {
                            self.should_quit = true;
                        },
                        _ => {},
                    }

                    for component in self.components.iter_mut() {
                        component.handle_app_event(event.clone())?;
                    }
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
}
