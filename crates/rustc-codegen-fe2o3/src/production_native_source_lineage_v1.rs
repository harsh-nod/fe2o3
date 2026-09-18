//! Native source lineage from genuine attached source and consumed ranked custody.

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
    check_native_source_ranked_roster_v1, native_source_ranked_staging_commitments_v1,
};
use fe2o3_mir_model::InertCanonicalSemanticU32InductionEvidenceV1 as Induction;
use fe2o3_verifier::{
    CanonicalProductionMirPlironVerusExecutionEvidenceV1 as Signed, NativeCompilerRootStagingV1,
    NativeCompilerSourceProofInputsV1, NativeCompilerStagingCommitmentV1,
    ValidatedNativeCompilerSourceProofV1, validate_native_compiler_source_proof_v1,
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
    proof: ValidatedNativeCompilerSourceProofV1,
    native_module: Vec<u8>,
}
#[allow(
    dead_code,
    reason = "read-only handoff getters for the pending protected-native consumer"
)]
impl PreparedNativeSourceLineageV1 {
    pub(crate) fn ranked(&self) -> &AuthenticatedRankedVerificationRosterV1 {
        &self.ranked
    }
    pub(crate) fn proof(&self) -> &ValidatedNativeCompilerSourceProofV1 {
        &self.proof
    }
    pub(crate) fn native_module(&self) -> &[u8] {
        &self.native_module
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
    budget.charge_work(6)?;
    let floor = budget.storage();
    let token = budget.work_ledger_identity_v1();
    let slot = budget as *const Budget<'_> as usize;
    let minimum = source
        .pre_ranked_retained_analysis_storage_v1()
        .ok_or(E::Mismatch("missing native source owner"))?;
    if floor < minimum {
        return Err(Resource::Accounting.into());
    }
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        source.verify_equivalence().map_err(E::Source)?;
        let native = source
            .pre_ranked_executable()
            .ok_or(E::Mismatch("missing native source owner"))?;
        let launch = source
            .source_launch_roster()
            .ok_or(E::Mismatch("missing source launch roster"))?;
        let semantic = source.semantic().semantic();
        let count = semantic.roots().len();
        budget.charge_work(3)?;
        if ranked.root_count() != count
            || launch.roots().len() != count
            || !ranked.every_functional_verification_is_coherent()
        {
            return Err(E::Mismatch("complete coherent source/ranked roster"));
        }
        let mut joins = reserved_vec(count, budget)?;
        for root in ranked.roots() {
            budget.charge_work(2 + root.export_symbol().len())?;
            let export = std::str::from_utf8(root.export_symbol())
                .map_err(|_| E::Mismatch("ranked export encoding"))?;
            joins.push((
                root.semantic_root().index(),
                export,
                root.verification().middle_end_evidence().ranked_ir(),
            ));
        }
        check_native_source_ranked_roster_v1(source, &joins, budget).map_err(E::SourceJoin)?;
        // Check required signed custody before allocating any native envelopes.
        for root in ranked.roots() {
            budget.charge_work(1)?;
            if root.verification().aggregate_verus_execution().is_none() {
                return Err(E::MissingSignedRankedReceipt {
                    root: root.semantic_root().index(),
                });
            }
        }
        let mut payloads = reserved_vec(count, budget)?;
        let mut launches = reserved_vec(count, budget)?;
        let mut order = reserved_vec(count, budget)?;
        for &index in ranked.canonical_kernel_order() {
            budget.charge_work(1)?;
            order.push(u32::try_from(index).map_err(|_| Resource::Arithmetic)?);
        }
        for (ordinal, root) in ranked.roots().iter().enumerate() {
            budget.charge_work(80)?;
            let launch_root = launch.roots()[ordinal];
            if root.semantic_root() != semantic.roots()[ordinal]
                || root.semantic_root() != launch_root.selected_root()
                || root.semantic_root_identity() != launch_root.semantic_root_identity()
                || *root.kernel_binding() != launch_root.kernel_binding()
                || root.source_rank() != launch_root.source_rank()
            {
                return Err(E::Mismatch("source launch/ranked root"));
            }
            let induction = Induction::from_report(root.verification().semantic_u32_induction())
                .map_err(E::Induction)?;
            let execution = root.verification().aggregate_verus_execution().ok_or(
                E::MissingSignedRankedReceipt {
                    root: root.semantic_root().index(),
                },
            )?;
            let signed = Signed::from_execution(execution).map_err(E::Signed)?;
            budget.reserve_storage(
                induction
                    .canonical_bytes()
                    .len()
                    .checked_add(signed.canonical_bytes().len())
                    .and_then(|n| n.checked_mul(2))
                    .ok_or(Resource::Arithmetic)?,
            )?;
            let (source_rows, staging_receipt) = native_source_ranked_staging_commitments_v1(
                source,
                ordinal,
                root.semantic_root().index(),
                budget,
            )
            .map_err(E::SourceJoin)?;
            budget.reserve_storage(staging_receipt.retained_storage())?;
            let mut staging = reserved_vec(source_rows.len(), budget)?;
            for row in source_rows {
                budget.charge_work(289)?;
                staging.push(NativeCompilerStagingCommitmentV1 {
                    receipt: row.receipt(),
                    effect: row.effect(),
                    signer: row.signer(),
                    execution: row.execution(),
                    toolchain: row.toolchain(),
                });
            }
            budget.release_storage(staging_receipt.retained_storage())?;
            payloads.push((induction, signed, staging));
            launches.push(ProductionSourceLaunchRootInputV1::new(
                root.logical_name(),
                *root.kernel_binding(),
                launch_root.source_launch(),
            ));
        }
        let semantic_identity = *semantic.semantic_sha256().as_bytes();
        let (catalog, catalog_storage) =
            Catalog::from_rows_with_budget(semantic_identity, &[], &[], budget)
                .map_err(E::Catalog)?;
        budget.reserve_storage(catalog_storage.retained_storage())?;
        let graph_bytes = native.canonical().canonical_bytes();
        let catalog_bytes = catalog.canonical_bytes();
        let subject = InertNativeNeutralSubjectV1::new(
            *native.canonical().identity().digest(),
            u64::try_from(graph_bytes.len()).map_err(|_| Resource::Arithmetic)?,
            *catalog.digest(),
            u64::try_from(catalog_bytes.len()).map_err(|_| Resource::Arithmetic)?,
        )
        .map_err(E::Subject)?;
        let native_len = 112usize
            .checked_add(graph_bytes.len())
            .and_then(|n| n.checked_add(catalog_bytes.len()))
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(native_len)?;
        budget.charge_work(native_len)?;
        let native_module = encode_native_neutral_module_v1(&subject, graph_bytes, catalog_bytes)
            .map_err(E::Native)?;
        budget.reserve_storage(
            native_module
                .capacity()
                .checked_sub(native_len)
                .ok_or(Resource::Accounting)?,
        )?;
        let mut rows = reserved_vec(count, budget)?;
        for (ordinal, root) in ranked.roots().iter().enumerate() {
            budget.charge_work(4)?;
            let workgroup = launch.roots()[ordinal]
                .source_launch()
                .exact_workgroup()
                .ok_or(E::Mismatch("missing exact source workgroup"))?;
            rows.push(RootInput {
                semantic_root: root.semantic_root().index(),
                semantic_root_identity: *root.semantic_root_identity().as_bytes(),
                kernel_binding: *root.kernel_binding(),
                source_rank: root.source_rank(),
                workgroup,
                logical_name: root.logical_name(),
                export_symbol: joins[ordinal].1,
                kernel_id: joins[ordinal].1,
                payload: root.verification().middle_end_evidence().canonical_bytes(),
            });
        }
        let identity = *ranked.canonical_roster_identity().as_bytes();
        let middle = encode_roster(
            Kind::MiddleEnd,
            semantic_identity,
            subject,
            identity,
            &order,
            &rows,
            budget,
        )?;
        for (row, (induction, _, _)) in rows.iter_mut().zip(&payloads) {
            budget.charge_work(1)?;
            row.payload = induction.canonical_bytes();
        }
        let correspondence = encode_roster(
            Kind::Correspondence,
            semantic_identity,
            subject,
            identity,
            &order,
            &rows,
            budget,
        )?;
        for (row, (_, signed, _)) in rows.iter_mut().zip(&payloads) {
            budget.charge_work(1)?;
            row.payload = signed.canonical_bytes();
        }
        let verus = encode_roster(
            Kind::VerusExecution,
            semantic_identity,
            subject,
            identity,
            &order,
            &rows,
            budget,
        )?;
        let mut staging_roots = reserved_vec(count, budget)?;
        for (root, (_, _, staging)) in ranked.roots().iter().zip(&payloads) {
            budget.charge_work(1)?;
            staging_roots.push(NativeCompilerRootStagingV1 {
                semantic_root: root.semantic_root().index(),
                commitments: staging,
            });
        }
        let (proof, receipt) = validate_native_compiler_source_proof_v1(
            NativeCompilerSourceProofInputsV1 {
                semantic_mir: semantic.canonical_encoding(),
                native_module: &native_module,
                middle_end_roster: middle.canonical_bytes(),
                correspondence_roster: correspondence.canonical_bytes(),
                verus_roster: verus.canonical_bytes(),
                launch_inputs: &launches,
                staging_roots: &staging_roots,
            },
            budget,
        )
        .map_err(E::Replay)?;
        budget.reserve_storage(receipt.retained_storage())?;
        let header = std::mem::size_of::<PreparedNativeSourceLineageV1>()
            .checked_sub(std::mem::size_of::<ValidatedNativeCompilerSourceProofV1>())
            .and_then(|n| {
                n.checked_sub(std::mem::size_of::<AuthenticatedRankedVerificationRosterV1>())
            })
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(header)?;
        let retained = header
            .checked_add(native_module.capacity())
            .and_then(|n| n.checked_add(receipt.retained_storage()))
            .ok_or(Resource::Arithmetic)?;
        drop(rows);
        drop(staging_roots);
        drop(launches);
        drop(joins);
        Ok((
            PreparedNativeSourceLineageV1 {
                ranked,
                proof,
                native_module,
            },
            NativeSourceLineageStorageV1(retained),
        ))
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

#[cfg(test)]
#[path = "production_native_source_lineage_v1_tests.rs"]
mod tests;
