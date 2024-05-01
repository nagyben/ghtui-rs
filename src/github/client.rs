use color_eyre::{
    eyre::{bail, eyre, Error, Report, Result},
    owo_colors::OwoColorize,
};
use graphql_client::GraphQLQuery;
use octocrab::Octocrab;
use tracing::debug;

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
        let username2 = username.clone();
        let t1 = tokio::spawn(async move {
            let token = std::env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN must be set");
            let oc = Octocrab::builder().personal_token(token).build().expect("Failed to create Octocrab client");
            let response: graphql_client::Response<pull_requests_query::ResponseData> = oc
                .graphql(&PullRequestsQuery::build_query(pull_requests_query::Variables {
                    first: 20,
                    query: format!("is:pr involves:{} state:open", username),
                }))
                .await
                .expect("Failed to get pull requests (involves)");
            response
        });

        let t2 = tokio::spawn(async move {
            let token = std::env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN must be set");
            let oc = Octocrab::builder().personal_token(token).build().expect("Failed to create Octocrab client");
            let response: graphql_client::Response<pull_requests_query::ResponseData> = oc
                .graphql(&PullRequestsQuery::build_query(pull_requests_query::Variables {
                    first: 20,
                    query: format!("is:pr review-requested:{} state:open", username2),
                }))
                .await
                .expect("Failed to get pull requests (review-requested)");
            response
        });

        let pr_involves = t1.await?;
        let pr_review_requested = t2.await?;

        let r1 = pr_involves
            .data
            .ok_or(eyre!("Response data is empty"))?
            .search
            .edges
            .ok_or(eyre!("Search data is empty"))?;
        let r2 = pr_review_requested
            .data
            .ok_or(eyre!("Response data is empty"))?
            .search
            .edges
            .ok_or(eyre!("Search data is empty"))?;

        let pull_requests_involves: Vec<PullRequest> = r1
            .iter()
            .map(|v: &Option<pull_requests_query::PullRequestsQuerySearchEdges>| {
                let inner = v.as_ref().unwrap().node.as_ref().unwrap();
                match inner {
                    pull_requests_query::PullRequestsQuerySearchEdgesNode::PullRequest(pr) => pr.into(),
                    _ => panic!("Unexpected node type: {:?}", inner),
                }
            })
            .collect();

        let pull_requests_review_requested: Vec<PullRequest> = r2
            .iter()
            .map(|v: &Option<pull_requests_query::PullRequestsQuerySearchEdges>| {
                let inner = v.as_ref().unwrap().node.as_ref().unwrap();
                match inner {
                    pull_requests_query::PullRequestsQuerySearchEdgesNode::PullRequest(pr) => pr.into(),
                    _ => panic!("Unexpected node type: {:?}", inner),
                }
            })
            .collect();

        let pull_requests =
            pull_requests_involves.into_iter().chain(pull_requests_review_requested.into_iter()).collect();
        Ok((pull_requests))
    }
}
