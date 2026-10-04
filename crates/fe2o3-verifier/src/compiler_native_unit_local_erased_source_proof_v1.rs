//! In-process typed source/N/ranked/N-to-E reconstruction, without artifact authority.

use super::ranked_source::{RecompiledNativeRankedRootsV1, recompile_native_ranked_roots_v1};
use super::*;
use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12;
use fe2o3_lower_mir_kernel::{
    ReplayedUnitLocalErasedNativeSourceV1, attach_replayed_native_unit_local_erasure_v1,
};

/// Original source packet and signed rosters commit to N. The separate actual E
/// is a checked candidate, never relabeled as N or as a RawEmpty source owner.
#[derive(Clone, Copy)]
pub struct NativeCompilerUnitLocalErasedSourceProofInputsV1<'a> {
    /// Original semantic source, N, catalog and all signed N-roster subjects.
    pub original: NativeCompilerSourceProofInputsV1<'a>,
    /// Complete ordered original typed ranked recipes and effect signatures.
    pub ranked_roots: &'a [NativeCompilerRankedRootV1<'a>],
    /// Actual erased graph, separately owned and reserved by the caller.
    pub erased: &'a VerifiedCanonicalKernelIrModuleV12,
}

/// Retains freshly reconstructed source/N, recompiled ranked roots, independently
/// admitted E and checked imported signatures. No conversion to a legacy source
/// owner, protected origin receipt or final artifact is provided.
pub struct ValidatedNativeCompilerUnitLocalErasedSourceProofV1 {
    source: ReplayedUnitLocalErasedNativeSourceV1,
    middle: Roster,
    correspondence: Roster,
    verus: Roster,
    roots: Vec<CheckedRoot>,
}

impl ValidatedNativeCompilerUnitLocalErasedSourceProofV1 {
    /// Actual owning original/N/ranked/E reconstruction and exact N/E relation.
    pub fn source(&self) -> &ReplayedUnitLocalErasedNativeSourceV1 {
        &self.source
    }
    /// Number of imported, recompiled original ranked roots.
    pub fn root_count(&self) -> usize {
        self.roots.len()
    }
    /// The complete original N middle-end roster, unchanged by erasure.
    pub fn middle_end_roster(&self) -> &Roster {
        &self.middle
    }
    /// The complete original N correspondence roster, unchanged by erasure.
    pub fn correspondence_roster(&self) -> &Roster {
        &self.correspondence
    }
    /// The complete original N aggregate-signature roster, unchanged by erasure.
    pub fn verus_roster(&self) -> &Roster {
        &self.verus
    }
    /// Imported aggregate proof for an original typed root, not origin authority.
    pub fn signed_ranked_proof(&self, root: usize) -> Option<&Signed> {
        self.roots.get(root).map(|root| &root.signed)
    }
    /// Original source/ranked correspondence was reconstructed from typed input.
    pub const fn replays_ranked_source_relation(&self) -> bool {
        true
    }
    /// Complete indexed-address and whole-operational equivalence are outside
    /// the inherited ranked translation domain.
    pub const fn proves_whole_operational_or_indexed_address_equivalence(&self) -> bool {
        false
    }
    /// Embedded signature keys do not establish compiler or launch provenance.
    pub const fn authenticates_compiler_or_launch_origin(&self) -> bool {
        false
    }
    /// This in-process boundary does not approve a final artifact or launch.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

/// Retained logical reconstruction payload transferred to the caller. Existing
/// semantic, PLIRON and proof engines retain their own bounded resource domains.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeCompilerUnitLocalErasedSourceProofStorageV1(usize);
impl NativeCompilerUnitLocalErasedSourceProofStorageV1 {
    /// Additional retained payload to reserve after this scope restores its floor.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Reconstructs original source/N, imports exact ordered effect signatures,
/// invokes the real staged ranked compiler and rebuilds V5/aggregate subjects.
/// Actual E then passes fresh V12 admission and independent checked silent-call
/// deletion against original N. No erasure producer or serialized deletion map
/// supplies authority. This stops before E/B/C/O and final descriptor validation.
///
/// Entry8; exact vectors, text and E bytes are paid before use. Success, error and
/// unwind restore the incoming canonical storage floor. The result storage
/// receipt must be reserved before subsequent allocation.
pub fn validate_native_compiler_unit_local_erased_source_proof_v1(
    inputs: NativeCompilerUnitLocalErasedSourceProofInputsV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<
    (
        ValidatedNativeCompilerUnitLocalErasedSourceProofV1,
        NativeCompilerUnitLocalErasedSourceProofStorageV1,
    ),
    E,
> {
    budget.charge_work(8)?;
    let floor = budget.storage();
    let token = budget.work_ledger_identity_v1();
    let slot = budget as *const Budget<'_> as usize;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let (checked, packet_storage) = validate_native_compiler_source_packet_v1(
            inputs.original,
            SourceReplayRoute::UnitLocal,
            budget,
        )?;
        budget.reserve_storage(packet_storage.retained_storage())?;
        complete_unit_local_erased_source_proof_v1(
            checked,
            packet_storage,
            inputs.ranked_roots,
            inputs.erased,
            budget,
        )
    }));
    if token != budget.work_ledger_identity_v1() || slot != budget as *const Budget<'_> as usize {
        drop(result);
        return Err(Resource::Accounting.into());
    }
    let Some(release) = budget.storage().checked_sub(floor) else {
        drop(result);
        return Err(Resource::Accounting.into());
    };
    if let Err(error) = budget.release_storage(release) {
        drop(result);
        return Err(error.into());
    }
    match result {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

/// Caller retains the packet reservation and supplies the outer cleanup scope.
pub(super) fn complete_unit_local_erased_source_proof_v1(
    checked: CheckedNativeCompilerSourcePacketV1,
    packet_storage: NativeCompilerSourceProofStorageV1,
    ranked_roots: &[NativeCompilerRankedRootV1<'_>],
    erased: &VerifiedCanonicalKernelIrModuleV12,
    budget: &mut Budget<'_>,
) -> Result<
    (
        ValidatedNativeCompilerUnitLocalErasedSourceProofV1,
        NativeCompilerUnitLocalErasedSourceProofStorageV1,
    ),
    E,
> {
    budget.charge_work(1)?;
    if ranked_roots.len() != checked.roots.len() {
        return Err(E::Mismatch("complete typed ranked root roster"));
    }
    let wrapper = std::mem::size_of::<ValidatedNativeCompilerUnitLocalErasedSourceProofV1>();
    budget.reserve_storage(wrapper)?;
    let RecompiledNativeRankedRootsV1 {
        candidates,
        lowerings,
        candidate_storage,
        lowering_vector_storage,
        lowering_storage,
    } = recompile_native_ranked_roots_v1(
        checked.source.source(),
        &checked.middle,
        &checked.roots,
        ranked_roots,
        budget,
    )?;
    let CheckedNativeCompilerSourcePacketV1 {
        source,
        middle,
        correspondence,
        verus,
        roots,
    } = checked;
    let ReconstructedNativeSource::UnitLocal(source) = source else {
        return Err(E::Mismatch("UnitLocal reconstruction route"));
    };
    let (source, attachment_storage) = attach_replayed_native_unit_local_erasure_v1(
        source,
        &candidates,
        lowerings,
        erased,
        budget,
    )
    .map_err(E::Source)?;
    drop(candidates);
    budget.release_storage(
        candidate_storage
            .checked_add(lowering_vector_storage)
            .ok_or(Resource::Arithmetic)?,
    )?;
    let retained = packet_storage
        .retained_storage()
        .checked_add(wrapper)
        .and_then(|value| value.checked_add(lowering_storage))
        .and_then(|value| value.checked_add(attachment_storage.retained_storage()))
        .ok_or(Resource::Arithmetic)?;
    Ok((
        ValidatedNativeCompilerUnitLocalErasedSourceProofV1 {
            source,
            middle,
            correspondence,
            verus,
            roots,
        },
        NativeCompilerUnitLocalErasedSourceProofStorageV1(retained),
    ))
}
