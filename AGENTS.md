# AGENTS.md - Coding Agent Guidelines for ghtui-rs

## Build & Test Commands
- `cargo build` - Build the project
- `cargo run` - Run the application (requires GITHUB_TOKEN env var)
- `cargo test` - Run all tests
- `cargo test <test_name>` - Run a specific test
- `cargo fmt` - Format code using rustfmt
- `cargo clippy` - Run linter

## Code Style Guidelines
- Use rustfmt with max_width=120, 4-space tabs
- Import granularity: `Crate` level with `StdExternalCrate` grouping
- Prefer `use_field_init_shorthand` and `use_try_shorthand`
- Follow snake_case for variables/functions, PascalCase for types
- Use `color_eyre::eyre::Result` for error handling
- Prefer async/await with tokio runtime
- Use tracing macros (debug!, info!) instead of println!
- Allow #![allow(dead_code, unused_imports, unused_variables)] at module level
- Use graphql_client for GraphQL queries with derives for Clone, Debug

## Architecture Patterns
- Components implement the Component trait
- Use tokio::sync::mpsc for async communication
- State managed through App struct with Vec<Box<dyn Component>>
- Configuration via Config struct loaded from config files