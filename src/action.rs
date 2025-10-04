use std::{fmt, string::ToString};

use serde::{
    de::{self, Deserializer, Visitor},
    Deserialize, Serialize,
};
use strum::Display;

use crate::{
    components::{notifications::Notification, pull_request::PullRequest},
    mode::Mode,
};

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

    OpenCommandPalette,
    OpenSearchPalette,

    // custom actions for fetching data
    GetRepos,
    GetReposResult(Vec<PullRequest>),
    GetCurrentUserResult(String),
    GetCurrentUser,
    PullRequestDetailsLoaded(Box<PullRequest>),
    PullRequestDetailsLoadError,
    LoadMorePullRequests,
    LoadMorePullRequestsResult(Vec<PullRequest>, bool, Option<String>),
    Left,
    Right,
    ChangeMode(Mode),
    ExecuteCommand(String),
    ExecuteSearch(String),
}
