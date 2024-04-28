use color_eyre::{
    eyre::{bail, eyre, Error, Report, Result},
    owo_colors::OwoColorize,
};
use graphql_client::GraphQLQuery;
use octocrab::Octocrab;

use crate::{
    action::Action,
    components::pull_request::{pull_requests_query, PullRequest, PullRequestsQuery},
};

pub trait GithubClient {
    fn get_current_user() -> impl std::future::Future<Output = Result<String>> + Send;
    fn get_pull_requests(username: String) -> impl std::future::Future<Output = Result<Vec<PullRequest>>> + Send;
}

#[derive(Default)]
pub struct GraphQLGithubClient;

impl GithubClient for GraphQLGithubClient {
    async fn get_current_user() -> Result<String> {
        let token = std::env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN must be set");
        let oc = Octocrab::builder().personal_token(token).build().expect("Failed to create Octocrab client");
        let response: serde_json::Value = oc.graphql(&serde_json::json!({ "query": "{ viewer { login }}" })).await?;
        Ok(String::from(response["data"]["viewer"]["login"].as_str().unwrap()))
    }

    async fn get_pull_requests(username: String) -> Result<Vec<PullRequest>> {
        let token = std::env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN must be set");
        let oc = Octocrab::builder().personal_token(token).build().expect("Failed to create Octocrab client");

        let response: graphql_client::Response<pull_requests_query::ResponseData> = oc
            .graphql(&PullRequestsQuery::build_query(pull_requests_query::Variables {
                first: 10,
                query: format!("is:pr involves:{} state:open", username),
            }))
            .await?;

        log::debug!("{:#?}", response);
        let r =
            response.data.ok_or(eyre!("Response data is empty"))?.search.edges.ok_or(eyre!("Search data is empty"))?;
        let pull_requests: Vec<PullRequest> = r
            .iter()
            .map(|v: &Option<pull_requests_query::PullRequestsQuerySearchEdges>| {
                let inner = v.as_ref().unwrap().node.as_ref().unwrap();
                match inner {
                    pull_requests_query::PullRequestsQuerySearchEdgesNode::PullRequest(pr) => pr.into(),
                    _ => panic!("Unexpected node type: {:?}", inner),
                }
            })
            .collect();
        Ok(pull_requests)
    }
}
