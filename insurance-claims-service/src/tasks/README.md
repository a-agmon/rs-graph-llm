# Insurance Claims Workflow - Tasks Module

This directory contains the modular task implementations for the insurance claims processing workflow.

## Structure

- **`types.rs`** - Shared data structures used across tasks (`ClaimDetails`, `ClaimValidation`, `ClaimDecision`)
- **`utils.rs`** - Shared utility functions (`get_llm_agent`, `validate_claim`, `extract_cost_from_text`)
- **`initial_claim_query.rs`** - Initial welcome and claim information gathering
- **`insurance_type_classifier.rs`** - Determines car vs apartment insurance type
- **`car_insurance_details.rs`** - Collects car-specific claim details
- **`apartment_insurance_details.rs`** - Collects apartment-specific claim details
- **`smart_claim_validator.rs`** - **Unified validator, approval handler, and decision processor**
- **`final_summary.rs`** - **Single endpoint for all claim outcomes (approved/rejected)**
- **`mod.rs`** - Module organization and re-exports

## Simplified Workflow Architecture

```
Initial Claim Query
       ↓
Insurance Type Classifier
       ↓
    [Car | Apartment] Insurance Details
       ↓
Smart Claim Validator & Approval Handler
  (Auto-approve <$1000 OR Wait for manual approval ≥$1000)
       ↓
   Final Summary
  (Approved OR Rejected)
```

### Smart Validator Logic:
- **< $1000**: Auto-approve and proceed to Final Summary
- **≥ $1000**: Present approval request and stay in task until decision
- **After approval/rejection**: Proceed to Final Summary
- **Status messages**: Act as comprehensive logging system

## Key Features

- **Conditional Routing**: Uses graph conditional edges to route based on insurance type and claim amount
- **LLM Integration**: Each interactive task uses LLM agents for natural conversation
- **Session Management**: Maintains claim state across multiple interactions
- **JSON Parsing**: Extracts structured data from LLM responses
- **Chat History**: Preserves conversation context throughout the workflow

## Adding New Tasks

To add a new task:

1. Create a new `.rs` file in this directory
2. Implement the `Task` trait from `graph_flow`
3. Add the module to `mod.rs`
4. Re-export the task struct in `mod.rs`
5. Update `main.rs` to include the task in the graph with appropriate edges

## Dependencies

Each task module can import:
- Shared types from `super::types`
- Shared utilities from `super::utils`
- External dependencies as needed