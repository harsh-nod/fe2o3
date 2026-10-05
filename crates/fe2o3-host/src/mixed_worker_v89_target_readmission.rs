//! Exact target association, independent of proof or dispatch authority.
use fe2o3_verifier::{
    MixedTargetSelectionSubjectV89 as TargetSubject,
    check_mixed_target_selection_v89 as check_target,
};
include!("mixed_worker_target_readmission_family.rs");
#[cfg(test)]
#[path = "mixed_worker_v89_target_readmission_tests.rs"]
mod tests;
