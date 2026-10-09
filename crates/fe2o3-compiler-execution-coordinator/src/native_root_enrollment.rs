//! Original-attempt enrollment query; no wire change or mapped-consumer activation.
use super::*;
use crate::proof_helper_backing::ProofHelperBackingError;
use crate::proof_helper_launch::{ManagedProofHelper as Helper, ProofHelperLaunchError as Failure};

/// Inert coordinates obtained together through original attempt custody. These
/// may describe verification expectations, never substitute for their live owner.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct OriginalCompilerEnrollment {
    pub(crate) rustc_invocation_sha256: [u8; 32],
    pub(crate) native_policy_sha256: [u8; 32],
    pub(crate) policy_generation: u64,
    pub(crate) binding_count: Option<u32>,
}

const SCRATCH: usize =
    size_of::<crate::compiler_invocation_backing::OriginalInvocationEnrollment>()
        + size_of::<([u8; 32], u64)>()
        + size_of::<(OriginalCompilerEnrollment, Storage)>();

impl NativeAttempt<'_, Helper> {
    /// All inputs come from this same attempt: original trace/root association,
    /// retained issuer policy and the helper's original sealed compiler backing.
    /// Existing live-issuer/helper and original-account checks remain mandatory.
    /// Nested work is charged on that account; reserve returned output storage
    /// before retaining it. No complete dynamic compiler/PREPARSE qualification,
    /// transport authority or permission to activate mapped recovery is returned.
    pub(crate) fn original_enrollment(
        &self,
        b: &mut Budget<'_>,
    ) -> std::result::Result<(OriginalCompilerEnrollment, Storage), Failure> {
        b.check_prior_denials_v1()?;
        b.with_prepaid_scope(
            self.retained,
            8,
            8,
            SCRATCH,
            |b| {
                self.validate_original(b)?;
                self.validate_ready(b)?;
                let (native_policy_sha256, policy_generation) =
                    self.original_policy_coordinates(b)?;
                let invocation = self.trace.with_backing(b, |helper, b| {
                    helper.with_compiler(b, |compiler, b| {
                        compiler
                            .original_enrollment(b)
                            .map_err(ProofHelperBackingError::from)
                            .map_err(Failure::from)
                    })
                })?;
                self.validate_original(b)?;
                Ok((
                    OriginalCompilerEnrollment {
                        rustc_invocation_sha256: invocation.rustc_invocation_sha256,
                        native_policy_sha256,
                        policy_generation,
                        binding_count: invocation.binding_count,
                    },
                    Storage(size_of::<OriginalCompilerEnrollment>()),
                ))
            },
        )
    }
}
