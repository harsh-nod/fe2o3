//! Content-only admission to the existing nominal V3 finalizer. This module
//! cannot establish the compiler's actual Worker join from a caller callback.

use crate::{
    InertProtectedFirstBuildWorkerV3EvidenceV1 as Source,
    NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V3 as DESCRIPTOR_SCRATCH, NominalWorkerFinalizationErrorV3,
    PreparedFinalizedNominalWorkerHsacoV3 as Artifact, finalize_protected_worker_nominal_hsaco_v3,
};
use fe2o3_compiler_lineage::MixedMiddleEndIdentityV50 as Identity;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use fe2o3_verifier::{
    ExecutedTypedSourceTailV50 as Executed, MixedOptimizerRelocationErrorV28 as Refinement,
};
use std::mem::{align_of, size_of};

/// Exact content, original resource, or existing finalizer refusal.
#[derive(Debug)]
pub enum TypedWorkerLineageErrorV50 {
    /// Shared account refusal.
    Resource(Resource),
    /// Exact retained runtime/request content differs or is no longer current.
    Refinement(Refinement),
    /// Existing strict nominal finalizer failed.
    Finalization(NominalWorkerFinalizationErrorV3<Resource>),
    /// Same-ledger retained content identity changed.
    Binding,
}
type Error = TypedWorkerLineageErrorV50;
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl From<Refinement> for Error {
    fn from(error: Refinement) -> Self {
        Self::Refinement(error)
    }
}
impl From<NominalWorkerFinalizationErrorV3<Resource>> for Error {
    fn from(error: NominalWorkerFinalizationErrorV3<Resource>) -> Self {
        Self::Finalization(error)
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Refinement(e) => e.fmt(f),
            Self::Finalization(e) => e.fmt(f),
            Self::Binding => f.write_str("mixed finalizer lineage binding differs"),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Refinement(e) => Some(e),
            Self::Finalization(e) => Some(e),
            Self::Binding => None,
        }
    }
}

/// Content-only finalized artifact plus the real composed execution owner.
/// This type does not establish an original compiler/Worker join. Neither it
/// nor the old artifact grants publication or launch authority. There is no
/// decoded-receipt constructor and no caller callback can upgrade its meaning.
///
/// ```compile_fail
/// use fe2o3_hsaco_finalize::PreparedFinalizedTypedContentV50 as Owner;
/// fn duplicate(owner: Owner<'_, '_, '_, '_, '_, '_, '_>) { let _ = owner.clone(); }
/// ```
pub struct PreparedFinalizedTypedContentV50<'e, 'r, 'h, 'n, 'p, 'v, 's> {
    finalized: Artifact,
    executed: &'e Executed<'r, 'h, 'n, 'p, 'v, 's>,
    content: Identity,
    retained: usize,
    required: usize,
    ledger: Ledger,
}
impl PreparedFinalizedTypedContentV50<'_, '_, '_, '_, '_, '_, '_> {
    fn custody(&self, budget: &Budget<'_>) -> Result<(), Error> {
        if self.ledger != budget.work_ledger_identity_v1() || budget.storage() < self.required {
            return Err(Resource::Accounting.into());
        }
        Ok(())
    }
    /// Exact existing nominal finalizer owner; no extraction drops execution custody.
    pub fn finalized(&self, budget: &Budget<'_>) -> Result<&Artifact, Error> {
        self.custody(budget)?;
        Ok(&self.finalized)
    }
    /// Replays every transported byte against the same request and current runtime.
    pub fn replay(&self, budget: &mut Budget<'_>) -> Result<(), Error> {
        self.custody(budget)?;
        let content = self.executed.replay_lineage_capsule_v50(
            self.finalized.raw().outer_handoff().capsule().receipts(),
            budget,
        )?;
        if content != self.content {
            return Err(Error::Binding);
        }
        Ok(())
    }
    /// Additional wrapper storage on the shared KIR ledger. Existing bounded
    /// Worker/artifact backing retains its own finalizer accounting domain.
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    /// Drops the finalized artifact before settling this wrapper's own credit.
    /// An earlier query refusal cannot hide intact same-ledger custody.
    pub fn discard(self, budget: &mut Budget<'_>) -> Result<(), Error> {
        let custody = self.custody(budget);
        let retained = self.retained;
        drop(self);
        custody?;
        budget.release_storage(retained)?;
        Ok(())
    }
    /// Conditional execution and artifact inspection do not prove original MIR lowering.
    pub const fn proves_mir_to_native_lowering(&self) -> bool {
        false
    }
    /// Independent compiler/current-record/policy/runtime gates remain required.
    pub const fn grants_publication_or_launch_authority(&self) -> bool {
        false
    }
    /// Only the compiler's retained owner can establish its exact Worker join.
    pub const fn establishes_original_compiler_worker_join(&self) -> bool {
        false
    }
}
type StaticOwner =
    PreparedFinalizedTypedContentV50<'static, 'static, 'static, 'static, 'static, 'static, 'static>;
type StaticExecution = Executed<'static, 'static, 'static, 'static, 'static, 'static>;
type Capture<'a, E> = (Source, &'a E);
type Built = (Artifact, Identity);
const HEADER: usize = size_of::<StaticOwner>();
fn frames() -> Result<usize, Resource> {
    let base = HEADER + align_of::<StaticOwner>() + DESCRIPTOR_SCRATCH;
    [
        base,
        size_of::<Capture<'static, StaticExecution>>(),
        align_of::<Capture<'static, StaticExecution>>(),
        size_of::<std::panic::AssertUnwindSafe<Capture<'static, StaticExecution>>>(),
        size_of::<Result<Built, Error>>(),
        size_of::<std::thread::Result<Result<Built, Error>>>(),
        size_of::<Result<StaticOwner, Error>>(),
        2 * size_of::<Result<(), Error>>(),
        size_of::<&mut Budget<'_>>(),
        5 * size_of::<usize>(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, n| {
        sum.checked_add(n).ok_or(Resource::Arithmetic)
    })
}

/// Requires mixed content on the exact existing first-build capsule, then calls
/// the unchanged nominal V3 finalizer. Historical schemas are never retried.
///
/// The complete original request/runtime and capsule backing must already be
/// prepaid; caller retains that input credit until this wrapper is discarded.
/// The result is explicitly unjoined content. The compiler must independently
/// retain and check its exact protected Worker owner before consuming this
/// evidence and keep that owner through subsequent artifact use.
/// Shared Worker inspection/artifact allocation keeps the existing bounded
/// transaction accounting domain, exactly as the nominal finalizer documents.
/// This method accounts its extra typed frames and retained wrapper, not total
/// process RSS or the legacy finalizer's artifact heap. It neither manufactures
/// missing capsule proof fields nor admits sealed publication/currentness.
pub fn finalize_protected_worker_typed_content_v50<'e, 'r, 'h, 'n, 'p, 'v, 's>(
    source: Source,
    executed: &'e Executed<'r, 'h, 'n, 'p, 'v, 's>,
    budget: &mut Budget<'_>,
) -> Result<PreparedFinalizedTypedContentV50<'e, 'r, 'h, 'n, 'p, 'v, 's>, Error> {
    let floor = budget.storage();
    let scratch = frames()?;
    let capture: Capture<'_, _> = (source, executed);
    let operation = move |budget: &mut Budget<'_>| -> Result<Built, Error> {
        let (source, executed) = std::convert::identity(capture);
        let content =
            executed.replay_lineage_capsule_v50(source.handoff().capsule().receipts(), budget)?;
        let finalized =
            finalize_protected_worker_nominal_hsaco_v3(source, DESCRIPTOR_SCRATCH, &mut |n| {
                budget.charge_work(n)
            })?;
        let actual = executed.replay_lineage_capsule_v50(
            finalized.raw().outer_handoff().capsule().receipts(),
            budget,
        )?;
        if actual != content {
            return Err(Error::Binding);
        }
        Ok((finalized, content))
    };
    #[cfg(test)]
    {
        assert_eq!(
            std::mem::size_of_val(&operation),
            size_of::<Capture<'_, StaticExecution>>()
        );
        assert_eq!(
            std::mem::align_of_val(&operation),
            align_of::<Capture<'_, StaticExecution>>()
        );
    }
    let (finalized, content) = budget.with_prepaid_scope(floor, 0, 0, scratch, operation)?;
    // The scope already admitted this exact header at peak before the owner existed.
    // The shared scope returns it unreserved; adopt only that header now.
    budget.reserve_storage(HEADER)?;
    Ok(PreparedFinalizedTypedContentV50 {
        finalized,
        executed,
        content,
        retained: HEADER,
        required: budget.storage(),
        ledger: budget.work_ledger_identity_v1(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typed_finalizer_lineage_uses_existing_artifact_and_explicit_capture_frames() {
        let expected = size_of::<StaticOwner>()
            + align_of::<StaticOwner>()
            + size_of::<(Source, &StaticExecution)>()
            + align_of::<(Source, &StaticExecution)>()
            + size_of::<std::panic::AssertUnwindSafe<(Source, &StaticExecution)>>()
            + size_of::<Result<(Artifact, Identity), Error>>()
            + size_of::<std::thread::Result<Result<(Artifact, Identity), Error>>>()
            + size_of::<Result<StaticOwner, Error>>()
            + 2 * size_of::<Result<(), Error>>()
            + size_of::<&mut Budget<'_>>()
            + 5 * size_of::<usize>()
            + DESCRIPTOR_SCRATCH;
        assert_eq!(frames().unwrap(), expected);
        assert_ne!(
            std::any::TypeId::of::<StaticOwner>(),
            std::any::TypeId::of::<Artifact>()
        );
    }
}
