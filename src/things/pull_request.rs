#![allow(clippy::upper_case_acronyms)]
type URI = String;
type DateTime = chrono::DateTime<chrono::Utc>;

use std::fmt::Debug;

use graphql_client::GraphQLQuery;
use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Cell, Row},
};
use serde::{Deserialize, Serialize};
use tracing::debug;

use self::{
    pull_requests_query::{
        PullRequestReviewState as PrQueryReviewState, PullRequestState as PrQueryState,
        PullRequestsQuerySearchEdgesNodeOnPullRequest,
    },
    pull_requests_summary_query::{
        PullRequestReviewState as PrSummaryReviewState, PullRequestState as PrSummaryState,
        PullRequestsSummaryQuerySearchEdgesNodeOnPullRequest,
    },
};
use crate::{
    action::Action,
    colors::{BLUE, GREEN, RED, YELLOW},
    things::thing::Thing,
};

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/github/schema.graphql",
    query_path = "src/github/queries/pull_requests.graphql",
    variables_derives = "Clone, Debug, Eq, PartialEq, Ord, PartialOrd",
    response_derives = "Clone, Debug"
)]
pub struct PullRequestsQuery;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/github/schema.graphql",
    query_path = "src/github/queries/pull_requests_summary.graphql",
    variables_derives = "Clone, Debug, Eq, PartialEq, Ord, PartialOrd",
    response_derives = "Clone, Debug"
)]
pub struct PullRequestsSummaryQuery;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/github/schema.graphql",
    query_path = "src/github/queries/pull_request_detail.graphql",
    variables_derives = "Clone, Debug, Eq, PartialEq, Ord, PartialOrd",
    response_derives = "Clone, Debug"
)]
pub struct PullRequestDetailQuery;

#[derive(Clone, Serialize, Deserialize, Eq)]
pub struct PullRequest {
    pub number: usize,
    pub title: String,
    pub repository: String,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub url: URI,
    pub changed_files: usize,
    pub additions: usize,
    pub deletions: usize,
    pub state: PullRequestState,
    pub is_draft: bool,
    pub reviews: Vec<PullRequestReview>,
    pub author: String,
    pub base_branch: String,
    pub body: String,
    pub comments: Vec<PullRequestComment>,
    pub uuid: uuid::Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, strum::Display)]
pub enum PullRequestState {
    Open,
    Closed,
    Merged,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, strum::Display)]
pub enum PullRequestReviewState {
    Approved,
    ChangesRequested,
    Commented,
    Dismissed,
    Pending,
}

impl From<PrQueryState> for PullRequestState {
    fn from(state: PrQueryState) -> Self {
        match state {
            PrQueryState::OPEN => PullRequestState::Open,
            PrQueryState::CLOSED => PullRequestState::Closed,
            PrQueryState::MERGED => PullRequestState::Merged,
            _ => PullRequestState::Open,
        }
    }
}

impl From<PrSummaryState> for PullRequestState {
    fn from(state: PrSummaryState) -> Self {
        match state {
            PrSummaryState::OPEN => PullRequestState::Open,
            PrSummaryState::CLOSED => PullRequestState::Closed,
            PrSummaryState::MERGED => PullRequestState::Merged,
            _ => PullRequestState::Open,
        }
    }
}

impl From<PrQueryReviewState> for PullRequestReviewState {
    fn from(state: PrQueryReviewState) -> Self {
        match state {
            PrQueryReviewState::APPROVED => PullRequestReviewState::Approved,
            PrQueryReviewState::CHANGES_REQUESTED => PullRequestReviewState::ChangesRequested,
            PrQueryReviewState::COMMENTED => PullRequestReviewState::Commented,
            PrQueryReviewState::DISMISSED => PullRequestReviewState::Dismissed,
            PrQueryReviewState::PENDING => PullRequestReviewState::Pending,
            _ => PullRequestReviewState::Commented,
        }
    }
}

impl From<PrSummaryReviewState> for PullRequestReviewState {
    fn from(state: PrSummaryReviewState) -> Self {
        match state {
            PrSummaryReviewState::APPROVED => PullRequestReviewState::Approved,
            PrSummaryReviewState::CHANGES_REQUESTED => PullRequestReviewState::ChangesRequested,
            PrSummaryReviewState::COMMENTED => PullRequestReviewState::Commented,
            PrSummaryReviewState::DISMISSED => PullRequestReviewState::Dismissed,
            PrSummaryReviewState::PENDING => PullRequestReviewState::Pending,
            _ => PullRequestReviewState::Commented,
        }
    }
}

impl PartialEq for PullRequest {
    fn eq(&self, other: &Self) -> bool {
        debug!("{:?} == {:?}", self.number, other.number);
        debug!("{:?} == {:?}", self.repository, other.repository);
        self.number == other.number && self.repository == other.repository
    }
}

impl std::cmp::Ord for PullRequest {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let repo_ord = self.repository.cmp(&other.repository);
        match repo_ord {
            std::cmp::Ordering::Equal => self.number.cmp(&other.number),
            _ => repo_ord,
        }
    }
}

impl PartialOrd for PullRequest {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl std::fmt::Debug for PullRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PullRequest")
            .field("number", &self.number)
            .field("title", &self.title)
            .field("repository", &self.repository)
            .field("state", &self.state)
            .field("author", &self.author)
            .field("base_branch", &self.base_branch)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequestReview {
    pub author: String,
    pub state: PullRequestReviewState,
}

impl From<&PullRequestsQuerySearchEdgesNodeOnPullRequest> for PullRequest {
    fn from(value: &PullRequestsQuerySearchEdgesNodeOnPullRequest) -> Self {
        Self {
            number: value.number as usize,
            title: value.title.clone(),
            author: value.author.as_ref().unwrap().login.clone(),
            repository: value.repository.name_with_owner.clone(),
            created_at: value.created_at,
            updated_at: value.updated_at,
            url: value.url.clone(),
            changed_files: value.changed_files as usize,
            additions: value.additions as usize,
            deletions: value.deletions as usize,
            state: value.state.clone().into(),
            is_draft: value.is_draft,
            reviews: value
                .latest_reviews
                .as_ref()
                .unwrap()
                .edges
                .as_ref()
                .unwrap()
                .iter()
                .map(|v| {
                    PullRequestReview {
                        author: v.as_ref().unwrap().node.as_ref().unwrap().author.as_ref().unwrap().login.clone(),
                        state: v.as_ref().unwrap().node.as_ref().unwrap().state.clone().into(),
                    }
                })
                .collect(),

            base_branch: value.base_ref_name.clone(),
            body: value.body.clone(),
            comments: vec![],
            uuid: uuid::Uuid::new_v4(),
        }
    }
}

impl From<&PullRequestsSummaryQuerySearchEdgesNodeOnPullRequest> for PullRequest {
    fn from(value: &PullRequestsSummaryQuerySearchEdgesNodeOnPullRequest) -> Self {
        Self {
            number: value.number as usize,
            title: value.title.clone(),
            author: value.author.as_ref().unwrap().login.clone(),
            repository: value.repository.name_with_owner.clone(),
            created_at: value.created_at,
            updated_at: value.updated_at,
            url: String::new(), // Will be loaded on-demand
            changed_files: 0,   // Will be loaded on-demand
            additions: value.additions as usize,
            deletions: value.deletions as usize,
            state: value.state.clone().into(),
            is_draft: value.is_draft,
            reviews: value
                .latest_reviews
                .as_ref()
                .unwrap()
                .edges
                .as_ref()
                .unwrap()
                .iter()
                .map(|v| {
                    PullRequestReview {
                        author: v.as_ref().unwrap().node.as_ref().unwrap().author.as_ref().unwrap().login.clone(),
                        state: v.as_ref().unwrap().node.as_ref().unwrap().state.clone().into(),
                    }
                })
                .collect(),
            base_branch: String::new(), // Will be loaded on-demand
            body: String::new(),        // Will be loaded on-demand
            comments: vec![],           // Will be loaded on-demand
            uuid: uuid::Uuid::new_v4(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequestComment {
    pub author: String,
    pub body: String,
    pub created_at: DateTime,
}

impl Thing for PullRequest {
    fn row(&self) -> Row<'_> {
        Row::new(vec![
            Cell::from(format!("{:}", self.number)),
            Cell::from(self.repository.clone()),
            Cell::from(self.title.clone()),
            Cell::from(self.author.clone()),
            Cell::from(format!("{}", self.created_at.format("%Y-%m-%d"))),
            Cell::from(format!("{}", self.updated_at.format("%Y-%m-%d"))),
            Cell::from(Line::from(vec![
                Span::styled(format!("{:+}", self.additions), Style::new().fg(GREEN)),
                Span::styled(format!("{:+}", (0 - self.deletions as isize)), Style::new().fg(RED)),
            ])),
            Cell::from(match self.state {
                PullRequestState::Open => {
                    if self.is_draft {
                        "DRAFT"
                    } else {
                        "OPEN"
                    }
                },
                PullRequestState::Closed => "CLOSED",
                PullRequestState::Merged => "MERGED",
            }),
            Cell::from(Line::from(
                self.reviews
                    .iter()
                    .flat_map(|prr| {
                        vec![
                            Span::styled(prr.author.clone(), match prr.state {
                                PullRequestReviewState::Commented => Style::new().fg(BLUE),
                                PullRequestReviewState::Approved => Style::new().fg(GREEN),
                                PullRequestReviewState::ChangesRequested => Style::new().fg(YELLOW),
                                _ => Style::new().fg(Color::Gray),
                            }),
                            Span::raw(" "),
                        ]
                    })
                    .collect::<Vec<Span>>(),
            )),
        ])
    }

    fn details(&self) -> Option<Action> {
        todo!()
    }

    fn header(&self) -> Vec<&'static str> {
        vec!["#", "Repository", "Title", "Author", "Created", "Updated", "Changes", "State", "Reviews"]
    }

    fn cmp_by_column_index(&self, other: &dyn Thing, index: usize) -> std::cmp::Ordering {
        if let Some(other) = other.as_any().downcast_ref::<PullRequest>() {
            match index {
                0 => self.number.cmp(&other.number),
                1 => self.repository.cmp(&other.repository),
                2 => self.title.cmp(&other.title),
                3 => self.author.cmp(&other.author),
                4 => self.created_at.cmp(&other.created_at),
                5 => self.updated_at.cmp(&other.updated_at),
                6 => (self.additions + self.deletions).cmp(&(other.additions + other.deletions)),
                7 => self.state.cmp(&other.state),
                _ => self.title.cmp(&other.title),
            }
        } else {
            panic!("Cannot compare PullRequest with different Thing type")
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn get_uuid(&self) -> uuid::Uuid {
        self.uuid
    }
}
