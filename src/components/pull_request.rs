type URI = String;
type DateTime = chrono::DateTime<chrono::Utc>;

use std::fmt::Debug;

use graphql_client::GraphQLQuery;
use serde::{Deserialize, Serialize};
use tracing::debug;

use self::pull_requests_query::{
    PullRequestReviewState, PullRequestState, PullRequestsQuerySearchEdgesNodeOnPullRequest,
};

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/github/schema.graphql",
    query_path = "src/github/queries/pull_requests.graphql",
    variables_derives = "Clone, Debug, Eq, PartialEq, Ord, PartialOrd",
    response_derives = "Clone, Debug"
)]
pub struct PullRequestsQuery;

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
            std::cmp::Ordering::Equal => return self.number.cmp(&other.number),
            _ => return repo_ord,
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
            state: value.state.clone(),
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
                        state: v.as_ref().unwrap().node.as_ref().unwrap().state.clone(),
                    }
                })
                .collect(),

            base_branch: value.base_ref_name.clone(),
            body: value.body.clone(),
            comments: vec![],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequestComment {
    author: String,
    body: String,
    created_at: DateTime,
}
