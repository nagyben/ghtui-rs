use std::sync::OnceLock;

use cached::proc_macro::cached;
use color_eyre::{
    eyre::{bail, eyre, Error, Report, Result},
    owo_colors::OwoColorize,
};
use graphql_client::GraphQLQuery;
use log::debug;
use octocrab::Octocrab;

use crate::{
    action::Action,
    components::pull_request::{
        pull_request_detail_query, pull_requests_summary_query, PullRequest, PullRequestComment,
        PullRequestDetailQuery, PullRequestReview, PullRequestReviewState, PullRequestState,
        PullRequestsSummaryQuery,
    },
    github::traits::GithubClient,
};

static CACHED_USERNAME: OnceLock<String> = OnceLock::new();

#[derive(Default)]
pub struct GraphQLGithubClient;

impl GithubClient for GraphQLGithubClient {
    async fn get_current_user() -> Result<String> {
        // use the inner trick to cache as described in
        // https://github.com/jaemk/cached/issues/16#issuecomment-431941432
        #[cached(result = true)]
        async fn inner() -> Result<String> {
            // Check cache first
            if let Some(cached_username) = CACHED_USERNAME.get() {
                debug!("Using cached username: {}", cached_username);
                return Ok(cached_username.clone());
            }

            let token = std::env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN must be set");
            debug!("Getting current user profile from GITHUB_TOKEN");
            let oc = Octocrab::builder()
                .personal_token(token)
                .build()
                .expect("Failed to create Octocrab client");
            let response: serde_json::Value = oc
                .graphql(&serde_json::json!({ "query": "{ viewer { login }}" }))
                .await?;
            let username = String::from(response["data"]["viewer"]["login"].as_str().unwrap());

            // Cache the username
            let _ = CACHED_USERNAME.set(username.clone());
            debug!("Cached username: {}", username);

            Ok(username)
        }
        inner().await
    }

    async fn get_pull_requests(username: String) -> Result<Vec<PullRequest>> {
        debug!("Getting pull requests for {}", username);
        let username2 = username.clone();

        // Use the lighter summary query instead of the full query
        let t1 = tokio::spawn(async move {
            let token = std::env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN must be set");
            let oc = Octocrab::builder()
                .personal_token(token)
                .build()
                .expect("Failed to create Octocrab client");
            let response: graphql_client::Response<pull_requests_summary_query::ResponseData> = oc
                .graphql(&PullRequestsSummaryQuery::build_query(
                    pull_requests_summary_query::Variables {
                        first: 30,
                        after: None,
                        query: format!("is:pr involves:{} state:open", username),
                    },
                ))
                .await
                .expect("Failed to get pull requests");
            response
        });

        let t2 = tokio::spawn(async move {
            let token = std::env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN must be set");
            let oc = Octocrab::builder()
                .personal_token(token)
                .build()
                .expect("Failed to create Octocrab client");
            let response: graphql_client::Response<pull_requests_summary_query::ResponseData> = oc
                .graphql(&PullRequestsSummaryQuery::build_query(
                    pull_requests_summary_query::Variables {
                        first: 30,
                        after: None,
                        query: format!("is:pr review-requested:{} state:open", username2),
                    },
                ))
                .await
                .expect("Failed to get pull requests");
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
            .filter_map(|v| v.as_ref())
            .filter_map(|edge| edge.node.as_ref())
            .filter_map(|node| {
                match node {
                    pull_requests_summary_query::PullRequestsSummaryQuerySearchEdgesNode::PullRequest(pr) => {
                        Some(pr.into())
                    },
                    _ => None,
                }
            })
            .collect();

        let pull_requests_review_requested: Vec<PullRequest> = r2
            .iter()
            .filter_map(|v| v.as_ref())
            .filter_map(|edge| edge.node.as_ref())
            .filter_map(|node| {
                match node {
                    pull_requests_summary_query::PullRequestsSummaryQuerySearchEdgesNode::PullRequest(pr) => {
                        Some(pr.into())
                    },
                    _ => None,
                }
            })
            .collect();

        let mut pull_requests: Vec<PullRequest> = pull_requests_involves
            .into_iter()
            .chain(pull_requests_review_requested.into_iter())
            .collect();

        // Optimized deduplication - sort first, then dedup
        pull_requests.sort_by(|a, b| {
            a.repository
                .cmp(&b.repository)
                .then_with(|| a.number.cmp(&b.number))
        });
        pull_requests.dedup();

        debug!("Found {} pull requests", pull_requests.len());
        Ok(pull_requests)
    }

    async fn get_pull_requests_paginated(
        username: String,
        first: i32,
        after: Option<String>,
    ) -> Result<(Vec<PullRequest>, bool, Option<String>)> {
        debug!(
            "Getting paginated pull requests for {} (first: {}, after: {:?})",
            username, first, after
        );
        let username2 = username.clone();
        let after1 = after.clone();
        let after2 = after.clone();

        let t1 = tokio::spawn(async move {
            let token = std::env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN must be set");
            let oc = Octocrab::builder()
                .personal_token(token)
                .build()
                .expect("Failed to create Octocrab client");
            let response: graphql_client::Response<pull_requests_summary_query::ResponseData> = oc
                .graphql(&PullRequestsSummaryQuery::build_query(
                    pull_requests_summary_query::Variables {
                        first: first.into(),
                        after: after1,
                        query: format!("is:pr involves:{} state:open", username),
                    },
                ))
                .await
                .expect("Failed to get pull requests");
            response
        });

        let t2 = tokio::spawn(async move {
            let token = std::env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN must be set");
            let oc = Octocrab::builder()
                .personal_token(token)
                .build()
                .expect("Failed to create Octocrab client");
            let response: graphql_client::Response<pull_requests_summary_query::ResponseData> = oc
                .graphql(&PullRequestsSummaryQuery::build_query(
                    pull_requests_summary_query::Variables {
                        first: first.into(),
                        after: after2,
                        query: format!("is:pr review-requested:{} state:open", username2),
                    },
                ))
                .await
                .expect("Failed to get pull requests");
            response
        });

        let pr_involves = t1.await?;
        let pr_review_requested = t2.await?;

        let r1_data = pr_involves
            .data
            .ok_or(eyre!("Response data is empty"))?
            .search;
        let r2_data = pr_review_requested
            .data
            .ok_or(eyre!("Response data is empty"))?
            .search;

        let r1 = r1_data.edges.ok_or(eyre!("Search data is empty"))?;
        let r2 = r2_data.edges.ok_or(eyre!("Search data is empty"))?;

        let pull_requests_involves: Vec<PullRequest> = r1
            .iter()
            .filter_map(|v| v.as_ref())
            .filter_map(|edge| edge.node.as_ref())
            .filter_map(|node| {
                match node {
                    pull_requests_summary_query::PullRequestsSummaryQuerySearchEdgesNode::PullRequest(pr) => {
                        Some(pr.into())
                    },
                    _ => None,
                }
            })
            .collect();

        let pull_requests_review_requested: Vec<PullRequest> = r2
            .iter()
            .filter_map(|v| v.as_ref())
            .filter_map(|edge| edge.node.as_ref())
            .filter_map(|node| {
                match node {
                    pull_requests_summary_query::PullRequestsSummaryQuerySearchEdgesNode::PullRequest(pr) => {
                        Some(pr.into())
                    },
                    _ => None,
                }
            })
            .collect();

        let mut pull_requests: Vec<PullRequest> = pull_requests_involves
            .into_iter()
            .chain(pull_requests_review_requested.into_iter())
            .collect();

        pull_requests.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        pull_requests.dedup();

        // Get pagination info from the first query (assumes both have similar pagination)
        let has_next_page = r1_data.page_info.has_next_page || r2_data.page_info.has_next_page;
        let end_cursor = r1_data
            .page_info
            .end_cursor
            .clone()
            .or_else(|| r2_data.page_info.end_cursor.clone());

        debug!(
            "Found {} pull requests (has_next_page: {}, end_cursor: {:?})",
            pull_requests.len(),
            has_next_page,
            end_cursor
        );
        Ok((pull_requests, has_next_page, end_cursor))
    }

    async fn get_pull_request_details(
        owner: String,
        repo: String,
        number: usize,
    ) -> Result<PullRequest> {
        #[cached(result = true)]
        async fn inner(owner: String, repo: String, number: usize) -> Result<PullRequest> {
            debug!(
                "Getting detailed PR info for {}/{} #{}",
                owner, repo, number
            );
            let token = std::env::var("GITHUB_TOKEN").expect("GITHUB_TOKEN must be set");
            let oc = Octocrab::builder()
                .personal_token(token)
                .build()
                .expect("Failed to create Octocrab client");

            let response: graphql_client::Response<pull_request_detail_query::ResponseData> = oc
                .graphql(&PullRequestDetailQuery::build_query(
                    pull_request_detail_query::Variables {
                        owner,
                        repo,
                        number: number as i64,
                    },
                ))
                .await
                .expect("Failed to get pull request details");

            let pr_data = response
                .data
                .ok_or(eyre!("Response data is empty"))?
                .repository
                .ok_or(eyre!("Repository not found"))?
                .pull_request
                .ok_or(eyre!("Pull request not found"))?;

            // Convert the detailed PR to our internal format
            let pull_request = PullRequest {
                number: pr_data.number as usize,
                title: pr_data.title,
                repository: pr_data.repository.name_with_owner,
                created_at: pr_data.created_at,
                updated_at: pr_data.updated_at,
                url: pr_data.url,
                changed_files: pr_data.changed_files as usize,
                additions: pr_data.additions as usize,
                deletions: pr_data.deletions as usize,
                state: match pr_data.state {
                    pull_request_detail_query::PullRequestState::OPEN => PullRequestState::Open,
                    pull_request_detail_query::PullRequestState::CLOSED => PullRequestState::Closed,
                    pull_request_detail_query::PullRequestState::MERGED => PullRequestState::Merged,
                    _ => PullRequestState::Open,
                },
                is_draft: pr_data.is_draft,
                reviews: pr_data
                    .latest_reviews
                    .map(|reviews| reviews.edges.unwrap_or_default())
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|edge| edge.as_ref())
                    .filter_map(|edge| edge.node.as_ref())
                    .map(|review| {
                        PullRequestReview {
                    author: review
                        .author
                        .as_ref()
                        .map(|a| a.login.clone())
                        .unwrap_or_default(),
                    state: match review.state {
                        pull_request_detail_query::PullRequestReviewState::APPROVED => {
                            PullRequestReviewState::Approved
                        }
                        pull_request_detail_query::PullRequestReviewState::CHANGES_REQUESTED => {
                            PullRequestReviewState::ChangesRequested
                        }
                        pull_request_detail_query::PullRequestReviewState::COMMENTED => {
                            PullRequestReviewState::Commented
                        }
                        pull_request_detail_query::PullRequestReviewState::DISMISSED => {
                            PullRequestReviewState::Dismissed
                        }
                        pull_request_detail_query::PullRequestReviewState::PENDING => {
                            PullRequestReviewState::Pending
                        }
                        _ => PullRequestReviewState::Commented,
                    },
                }
                    })
                    .collect(),
                author: pr_data
                    .author
                    .as_ref()
                    .map(|a| a.login.clone())
                    .unwrap_or_default(),
                base_branch: pr_data.base_ref_name,
                body: pr_data.body,
                comments: pr_data
                    .comments
                    .nodes
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|comment| comment.as_ref())
                    .map(|comment| PullRequestComment {
                        author: comment
                            .author
                            .as_ref()
                            .map(|a| a.login.clone())
                            .unwrap_or_default(),
                        body: comment.body.clone(),
                        created_at: comment.created_at,
                    })
                    .collect(),
            };

            Ok(pull_request)
        }
        inner(owner, repo, number).await
    }

    async fn approve_pull_request(pull_request: &PullRequest) -> Result<()> {
        // TODO: Implement PR approval
        todo!("PR approval not yet implemented")
    }
}
