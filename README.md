# ghtui-rs

[![CI](https://github.com//ghtui-rs/workflows/CI/badge.svg)](https://github.com//ghtui-rs/actions)

A GitHub TUI

## Getting started

1. Clone the repo
2. Download the Github GraphQL schema to `src/github/schema.graphql` by running the following command:

    ```sh
    curl -L https://docs.github.com/public/fpt/schema.docs.graphql -o src/github/schema.graphql
    ```

3. Generate a PAT token with read access to repos and pull requests
4. Run with `GITHUB_TOKEN=<your_pat> cargo run`
5. Press 'R' to refresh the list of repos
6. Navigate with arrow keys, or J and K
7. Press 'Enter' to view the selected Pull Request
