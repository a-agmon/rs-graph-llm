# Tasks Module

This directory contains the modular task implementations for the graph-service.

## Structure

- **`types.rs`** - Shared data structures used across tasks (`UserDetails`, `AccountDetails`)
- **`utils.rs`** - Shared utility functions (`get_llm_agent`, `fetch_account_details`)
- **`collect_user_details.rs`** - Task for collecting user credentials
- **`fetch_account_details.rs`** - Task for fetching account information
- **`answer_user_requests.rs`** - Task for answering user queries about their account
- **`mod.rs`** - Module organization and re-exports

## Adding New Tasks

To add a new task:

1. Create a new `.rs` file in this directory
2. Implement the `Task` trait from `graph_flow`
3. Add the module to `mod.rs`
4. Re-export the task struct in `mod.rs`
5. Update `main.rs` to use the new task in the graph

## Dependencies

Each task module can import:
- Shared types from `super::types`
- Shared utilities from `super::utils`
- External dependencies as needed 