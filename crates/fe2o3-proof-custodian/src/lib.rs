//! Fixed keyless proof execution under independently installed root configuration.
//!
//! This component retains process and original proof custody, not application
//! registration, compiler currentness or GPU authority. The root launcher must run
//! outside the filtered compiler coordinator. Application handover is separate.

#![deny(unsafe_code)]

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
compile_error!("fe2o3-proof-custodian requires Linux x86-64");

mod application;
mod cgroup;
mod deployment;
mod launch;
mod manager;
mod wire;
#[allow(unsafe_code)]
mod worker;

pub use application::run_inherited_application_proof_controller_v1;
pub use deployment::{
    ProductionApplicationProofCustodianDeploymentV1, ProductionProofCustodianDeploymentV1,
    ProofCustodianDeploymentV1,
};
pub use launch::{
    PendingRootApplicationProofControllerV1, PendingRootConditionalFillProofV1,
    PendingRootProofControllerLaunchV1, RootManagedProofControllerV1,
    RootRetainedConditionalFillProofV1, RootStagedApplicationProofControllerV1,
};
pub use manager::run_fixed_proof_manager_v1;
pub use worker::run_inherited_conditional_fill_proof_controller_v1;

use std::io;

fn require(value: bool, reason: &'static str) -> io::Result<()> {
    if value {
        Ok(())
    } else {
        Err(io::Error::other(reason))
    }
}

fn other(error: impl std::error::Error + Send + Sync + 'static) -> io::Error {
    io::Error::other(error)
}
