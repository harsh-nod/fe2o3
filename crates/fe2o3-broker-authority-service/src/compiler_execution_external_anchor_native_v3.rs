//! Native custody over the shared seqpacket exchange mechanics, not a V1 owner.
use super::*;
use crate::{
    ProtectedCompilerExecutionIssuerServiceErrorV3 as Error,
    ProtectedExternalAnchorServiceAdmissionV2 as Admission,
};
use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicyV3 as Policy;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use std::mem::size_of;
use std::os::fd::{AsFd, OwnedFd};
type Result<T> = std::result::Result<T, Error>;
type WireError = ProtectedCompilerExecutionExternalAnchorErrorV1;

include!("compiler_execution_external_anchor_native_body.rs");
