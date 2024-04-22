type URI = String;
type DateTime = chrono::DateTime<chrono::Utc>;

use graphql_client::GraphQLQuery;
use serde::{Deserialize, Serialize};

use self::pull_requests_query::{PullRequestsQuerySearchEdgesNode, PullRequestsQuerySearchEdgesNodeOnPullRequest};
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "src/github/schema.graphql",
    query_path = "src/github/queries/pull_requests.graphql",
    variables_derives = "Clone, Debug",
    response_derives = "Clone, Debug"
)]
pub struct PullRequestsQuery;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
}

impl From<&PullRequestsQuerySearchEdgesNodeOnPullRequest> for PullRequest {
    fn from(value: &PullRequestsQuerySearchEdgesNodeOnPullRequest) -> Self {
        Self {
            number: value.number as usize,
            title: value.title.clone(),
            repository: value.repository.name_with_owner.clone(),
            created_at: value.created_at,
            updated_at: value.updated_at,
            url: value.url.clone(),
            changed_files: value.changed_files as usize,
            additions: value.additions as usize,
            deletions: value.deletions as usize,
        }
    }
}
