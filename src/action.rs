use std::{fmt, string::ToString};

use serde::{
    de::{self, Deserializer, Visitor},
    Deserialize, Serialize,
};
use strum::Display;

use crate::components::{notifications::Notification, pull_request::PullRequest};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Display, Deserialize)]
pub enum Action {
    Tick,
    Render,
    Resize(u16, u16),
    Suspend,
    Resume,
    Quit,
    Refresh,
    Error(String),
    Info,
    Help,

    // custom actions
    Up,
    Down,
    Enter,
    Open,
    Escape,
    Back,
    PageDn,
    PageUp,
    Sort(usize),
    Notify(Notification),

    // custom actions for fetching data
    GetRepos,
    GetReposResult(Vec<PullRequest>),
    GetCurrentUserResult(String),
    GetCurrentUser,
    Left,
    Right,
}
