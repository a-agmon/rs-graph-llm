// Simplified Insurance Claims Workflow Tasks
pub mod initial_claim_query;
pub mod insurance_type_classifier;
pub mod car_insurance_details;
pub mod apartment_insurance_details;
pub mod smart_claim_validator;
pub mod final_summary;

// Shared modules
pub mod types;
pub mod utils;

// Re-export task implementations
pub use initial_claim_query::InitialClaimQueryTask;
pub use insurance_type_classifier::InsuranceTypeClassifierTask;
pub use car_insurance_details::CarInsuranceDetailsTask;
pub use apartment_insurance_details::ApartmentInsuranceDetailsTask;
pub use smart_claim_validator::SmartClaimValidatorTask;
pub use final_summary::FinalSummaryTask;

// Re-export session keys
pub use types::session_keys;
