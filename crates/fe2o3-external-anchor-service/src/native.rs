//! Closed native custody adapters over the same durable state engine as V1.
use crate::ExternalAnchorServiceErrorV1;
use fe2o3_compiler_closure_capability::CompilerExecutionCapabilityErrorV2;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1;
use std::{error::Error, fmt};

/// Logical returned ownership charge, shared by the native service families.
/// Reserve before retaining a result; retire its full charge only after drop.
#[derive(Debug)]
pub struct NativeExternalAnchorStorageV2(pub(crate) usize);
impl NativeExternalAnchorStorageV2 {
    pub const fn additional_storage(&self) -> usize {
        self.0
    }
}

/// Fixed native custody/accounting failures plus the shared durable-state errors.
#[derive(Debug)]
pub enum NativeExternalAnchorErrorV2 {
    Resource(CanonicalKernelIrVerificationResourceErrorV1),
    Capability(CompilerExecutionCapabilityErrorV2),
    State(ExternalAnchorServiceErrorV1),
    ServiceCredentials,
}
impl fmt::Display for NativeExternalAnchorErrorV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Capability(e) => e.fmt(f),
            Self::State(e) => e.fmt(f),
            Self::ServiceCredentials => {
                f.write_str("native anchor requires the deployment's exact nonroot UID/GID")
            }
        }
    }
}
impl Error for NativeExternalAnchorErrorV2 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Capability(e) => Some(e),
            Self::State(e) => Some(e),
            Self::ServiceCredentials => None,
        }
    }
}
impl From<CanonicalKernelIrVerificationResourceErrorV1> for NativeExternalAnchorErrorV2 {
    fn from(e: CanonicalKernelIrVerificationResourceErrorV1) -> Self {
        Self::Resource(e)
    }
}
impl From<CompilerExecutionCapabilityErrorV2> for NativeExternalAnchorErrorV2 {
    fn from(e: CompilerExecutionCapabilityErrorV2) -> Self {
        Self::Capability(e)
    }
}
impl From<ExternalAnchorServiceErrorV1> for NativeExternalAnchorErrorV2 {
    fn from(e: ExternalAnchorServiceErrorV1) -> Self {
        Self::State(e)
    }
}

pub(crate) enum OpenMode {
    Initialize,
    Existing,
    OpenOrInitialize,
}
