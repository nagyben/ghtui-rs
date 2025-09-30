use color_eyre::{
    eyre::{bail, eyre, Error, Report, Result},
    owo_colors::OwoColorize,
};

use crate::things::pull_request::PullRequest;

pub trait GithubClient {
    fn get_current_user() -> impl std::future::Future<Output = Result<String>> + Send;
    fn get_pull_requests(username: String) -> impl std::future::Future<Output = Result<Vec<PullRequest>>> + Send;
    fn get_pull_requests_paginated(
        username: String,
        first: i32,
        after: Option<String>,
    ) -> impl std::future::Future<Output = Result<(Vec<PullRequest>, bool, Option<String>)>> + Send;
    fn get_pull_request_details(
        owner: String,
        repo: String,
        number: usize,
    ) -> impl std::future::Future<Output = Result<PullRequest>> + Send;
    fn approve_pull_request(pull_request: &PullRequest) -> impl std::future::Future<Output = Result<()>> + Send;
}
