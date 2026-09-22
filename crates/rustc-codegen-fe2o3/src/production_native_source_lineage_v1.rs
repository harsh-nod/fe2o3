//! Native source lineage from genuine attached source and consumed ranked custody.

#[path = "production_native_source_packet_v1.rs"]
mod packet;
use packet::{NativeSourceRefV1, prepare_native_source_packet_v1};

#[path = "production_native_owned_source_packet_v1.rs"]
mod owned_packet;
pub(crate) use owned_packet::{
    PreparedNativeSourceProofPacketV1, prepare_borrowed_erased_native_source_packet_v1,
    prepare_borrowed_native_source_packet_v1,
};

#[path = "production_native_erased_source_lineage_v1.rs"]
mod erased;
pub(crate) use erased::{
    ErasedNativeSourceLineageStorageV1, PreparedErasedNativeSourceLineageV1,
    try_prepare_erased_native_source_lineage_v1,
};

use crate::production_ranked_projection_v1::AuthenticatedRankedVerificationRosterV1;
use fe2o3_compiler_lineage::{
    InertNativeNeutralSubjectV1, MultiRootProofRosterInputsV3, MultiRootProofRosterKindV3 as Kind,
    MultiRootProofRosterRootInputV3 as RootInput, MultiRootProofRosterTranscriptV3 as Roster,
    encode_native_neutral_module_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    InertCanonicalKernelIrContractCatalogV1 as Catalog,
};
use fe2o3_lower_mir_kernel::{
    ProductionSemanticKirOwnerV1, ProductionSourceLaunchRootInputV1,
    check_native_source_ranked_roster_v1, native_source_ranked_candidates_v1,
    native_source_ranked_staging_commitments_v1,
};
use fe2o3_mir_model::InertCanonicalSemanticU32InductionEvidenceV1 as Induction;
use fe2o3_verifier::{
    CanonicalProductionMirPlironVerusExecutionEvidenceV1 as Signed,
    NativeCompilerRankedRecipeRootV1, NativeCompilerRankedRecipeSourceProofInputsV1,
    NativeCompilerRootStagingV1, NativeCompilerSourceProofInputsV1,
    NativeCompilerStagingCommitmentV1, ValidatedNativeCompilerRankedSourceProofV1,
    encode_native_compiler_source_packet_v1, validate_native_compiler_ranked_source_packet_v1,
};

#[derive(Debug)]
pub(crate) enum NativeSourceLineageErrorV1 {
    Resource(Resource),
    Source(fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1),
    SourceJoin(fe2o3_lower_mir_kernel::NativeSourceReplayErrorV1),
    Catalog(fe2o3_kernel_ir::KernelIrContractCatalogErrorV1),
    Subject(fe2o3_compiler_lineage::NativeNeutralSubjectErrorV1),
    Native(fe2o3_compiler_lineage::NativeNeutralModuleErrorV1),
    Roster(fe2o3_compiler_lineage::MultiRootProofRosterErrorV3),
    Signed(fe2o3_verifier::ProductionMirPlironVerusExecutionEvidenceErrorV1),
    Induction(fe2o3_mir_model::SemanticU32InductionEvidenceErrorV1),
    Replay(fe2o3_verifier::NativeCompilerSourceProofErrorV1),
    MissingSignedRankedReceipt { root: u32 },
    Mismatch(&'static str),
}
impl std::fmt::Display for NativeSourceLineageErrorV1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Source(error) => error.fmt(f),
            Self::SourceJoin(error) => error.fmt(f),
            Self::Catalog(error) => error.fmt(f),
            Self::Subject(error) => error.fmt(f),
            Self::Native(error) => error.fmt(f),
            Self::Roster(error) => error.fmt(f),
            Self::Signed(error) => error.fmt(f),
            Self::Induction(error) => error.fmt(f),
            Self::Replay(error) => error.fmt(f),
            Self::MissingSignedRankedReceipt { root } => {
                write!(f, "native source root {root} has no signed ranked receipt")
            }
            Self::Mismatch(detail) => write!(f, "native source lineage mismatch: {detail}"),
        }
    }
}
impl std::error::Error for NativeSourceLineageErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Source(error) => Some(error),
            Self::SourceJoin(error) => Some(error),
            Self::Catalog(error) => Some(error),
            Self::Subject(error) => Some(error),
            Self::Native(error) => Some(error),
            Self::Roster(error) => Some(error),
            Self::Signed(error) => Some(error),
            Self::Induction(error) => Some(error),
            Self::Replay(error) => Some(error),
            Self::MissingSignedRankedReceipt { .. } | Self::Mismatch(_) => None,
        }
    }
}
impl From<Resource> for NativeSourceLineageErrorV1 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
type E = NativeSourceLineageErrorV1;

/// Move-only original ranked custody plus independently consumed native packet.
/// No protected compiler, O, object, publication or launch authority is minted.
#[allow(
    dead_code,
    reason = "retained custody for the pending protected-native consumer; no publication authority"
)]
pub(crate) struct PreparedNativeSourceLineageV1 {
    ranked: AuthenticatedRankedVerificationRosterV1,
    packet: PreparedNativeSourceProofPacketV1<ValidatedNativeCompilerRankedSourceProofV1>,
}
#[allow(
    dead_code,
    reason = "read-only handoff getters for the pending protected-native consumer"
)]
impl PreparedNativeSourceLineageV1 {
    pub(crate) fn ranked(&self) -> &AuthenticatedRankedVerificationRosterV1 {
        &self.ranked
    }
    pub(crate) fn proof(&self) -> &ValidatedNativeCompilerRankedSourceProofV1 {
        self.packet.proof()
    }
    pub(crate) fn native_module(&self) -> &[u8] {
        self.packet.original_native_module()
    }
    pub(crate) fn source_packet(&self) -> &[u8] {
        self.packet.source_packet()
    }
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NativeSourceLineageStorageV1(usize);
impl NativeSourceLineageStorageV1 {
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}

fn reserved_vec<T>(count: usize, budget: &mut Budget<'_>) -> Result<Vec<T>, E> {
    budget.charge_work(3)?;
    let requested = count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(requested)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    let actual = rows
        .capacity()
        .checked_mul(std::mem::size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(actual.checked_sub(requested).ok_or(Resource::Accounting)?)?;
    Ok(rows)
}

fn encode_roster(
    kind: Kind,
    semantic: [u8; 32],
    subject: InertNativeNeutralSubjectV1,
    identity: [u8; 32],
    order: &[u32],
    roots: &[RootInput<'_>],
    budget: &mut Budget<'_>,
) -> Result<Roster, E> {
    // Exact V3 wire: header176, two counts8, order4 each, fixed root84,
    // four field lengths16, then the actual three names and payload.
    let mut bytes = 184usize
        .checked_add(order.len().checked_mul(4).ok_or(Resource::Arithmetic)?)
        .ok_or(Resource::Arithmetic)?;
    for root in roots {
        budget.charge_work(5)?;
        bytes = bytes
            .checked_add(100)
            .and_then(|n| n.checked_add(root.logical_name.len()))
            .and_then(|n| n.checked_add(root.export_symbol.len()))
            .and_then(|n| n.checked_add(root.kernel_id.len()))
            .and_then(|n| n.checked_add(root.payload.len()))
            .ok_or(Resource::Arithmetic)?;
    }
    // The existing bounded codec's private metadata allocations are a separate
    // domain. These are explicit logical wire/header reservations, not RSS.
    budget.reserve_storage(
        bytes
            .checked_mul(2)
            .and_then(|n| n.checked_add(std::mem::size_of::<Roster>()))
            .ok_or(Resource::Arithmetic)?,
    )?;
    budget.charge_work(bytes)?;
    Roster::new(MultiRootProofRosterInputsV3 {
        kind,
        semantic_mir_sha256: semantic,
        native_neutral_subject: subject,
        roster_identity: identity,
        canonical_kernel_order: order,
        roots,
    })
    .map_err(E::Roster)
}

/// Requires the caller's live source analysis receipt (and any separately owned
/// capture) and consumes the original authenticated ranked roster. Retained
/// legacy semantic/signature/roster engines keep their existing bounded domains;
/// this ledger counts native vectors, wire reservations and independent replay.
/// Result/unwind cleanup preserves the incoming ledger and storage floor.
/// No missing-signature fallback exists. Reserve the returned additional receipt
/// before another allocation; drop this owner before releasing that receipt.
pub(crate) fn try_prepare_native_source_lineage_v1(
    source: &ProductionSemanticKirOwnerV1,
    ranked: AuthenticatedRankedVerificationRosterV1,
    budget: &mut Budget<'_>,
) -> Result<(PreparedNativeSourceLineageV1, NativeSourceLineageStorageV1), E> {
    packet::with_native_lineage_transfer_v1(budget, move |budget| {
        let packet = prepare_borrowed_native_source_packet_v1(source, &ranked, budget)?;
        let retained =
            owned_packet::roster_wrapper_storage::<_, PreparedNativeSourceLineageV1>(&packet)?;
        budget.reserve_storage(retained)?;
        Ok((
            PreparedNativeSourceLineageV1 { ranked, packet },
            NativeSourceLineageStorageV1(retained),
        ))
    })
}

#[cfg(test)]
#[path = "production_native_source_lineage_v1_tests.rs"]
mod tests;
