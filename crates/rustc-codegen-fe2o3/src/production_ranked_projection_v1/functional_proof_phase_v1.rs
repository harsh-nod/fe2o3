//! Linear proof custody across the mandatory final-graph schedule. Phase and
//! roster checks are joins only; none constructs graph or launch authority.

use fe2o3_kernel_ir::{Kernel, VerifiedCanonicalKernelIrV13};
use fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1;
use fe2o3_pliron::{ProductionFinalGraphTargetContractV1, ProductionVerifiedFinalGraphV1};
use fe2o3_verifier::{
    FunctionalRefinementVerusRuntimeLeaseV1,
    ProductionEffectIrDerivedVerusVerifiedMirPlironKernelV3,
    ProductionFinalGraphFunctionalRefinementExecutionV2,
    ProductionPreparedFinalGraphFunctionalExecutionV1,
    ProductionPreparedFinalGraphFunctionalSubjectV1,
};

use super::*;
use crate::production_mir_pliron_verus_join_v1::AuthenticatedMirPlironPerCompilationVerificationV2;

type Failure = ProductionRankedVerificationErrorV1;

pub(super) enum FunctionalProofPhaseV1 {
    Source(AuthenticatedMirPlironPerCompilationVerificationV2),
    Prepared(ProductionPreparedFinalGraphFunctionalExecutionV1),
    Completed(ProductionFinalGraphFunctionalRefinementExecutionV2),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FunctionalProofPhaseKindV1 {
    Source,
    Prepared,
    Completed,
}

impl FunctionalProofPhaseV1 {
    fn kind(&self) -> FunctionalProofPhaseKindV1 {
        match self {
            Self::Source(_) => FunctionalProofPhaseKindV1::Source,
            Self::Prepared(_) => FunctionalProofPhaseKindV1::Prepared,
            Self::Completed(_) => FunctionalProofPhaseKindV1::Completed,
        }
    }

    pub(super) fn verified(&self) -> &ProductionEffectIrDerivedVerusVerifiedMirPlironKernelV3 {
        match self {
            Self::Source(source) => source.verified(),
            Self::Prepared(prepared) => prepared.verified(),
            Self::Completed(completed) => completed.verified(),
        }
    }

    pub(super) fn into_source(
        self,
    ) -> Result<AuthenticatedMirPlironPerCompilationVerificationV2, Failure> {
        match self {
            Self::Source(source) => Ok(source),
            Self::Prepared(_) | Self::Completed(_) => Err(Failure::RosterMetadata(
                "legacy source extraction cannot discard an executed final proof",
            )),
        }
    }
}

impl AuthenticatedRankedVerificationV5 {
    /// Borrowed pending proof only. Final graph verification remains mandatory.
    pub(crate) fn prepared_final_execution(
        &self,
    ) -> Option<&ProductionPreparedFinalGraphFunctionalExecutionV1> {
        match &self.functional.as_ref()?.phase {
            FunctionalProofPhaseV1::Prepared(execution) => Some(execution),
            FunctionalProofPhaseV1::Source(_) | FunctionalProofPhaseV1::Completed(_) => None,
        }
    }
}

impl AuthenticatedRankedVerificationRosterV1 {
    /// Execute each final theorem before the mandatory live graph schedule.
    /// All source owners, roster metadata and theorem recipes are preflighted
    /// before the first execution. Failure returns no partial proof roster.
    pub(crate) fn prepare_final_graph_functional(
        self,
        runtime: &FunctionalRefinementVerusRuntimeLeaseV1,
        semantic_kir: &ProductionSemanticKirOwnerV1,
        final_canonical: &VerifiedCanonicalKernelIrV13,
        final_epoch: u64,
        target_contract: &ProductionFinalGraphTargetContractV1,
        timeout_seconds: u32,
    ) -> Result<Self, Failure> {
        self.require_functional_phase(FunctionalProofPhaseKindV1::Source)?;
        self.require_source_owner(semantic_kir)?;
        let source = semantic_kir
            .canonical_kernel_ir_v13()
            .ok_or(Failure::RosterMetadata(
                "final functional preparation requires source KIR V13",
            ))?;
        let mut subjects = Vec::with_capacity(self.roots.len());
        for _ in &self.roots {
            let subject = ProductionPreparedFinalGraphFunctionalSubjectV1::try_new(
                source,
                final_canonical,
                final_epoch,
                target_contract,
            )
            .map_err(Failure::FinalGraphFunctional)?;
            self.require_kernel_roster(subject.source_kernels())?;
            self.require_kernel_roster(subject.final_kernels())?;
            subjects.push(subject);
        }
        let mut subjects = subjects.into_iter();
        let prepared = self.map_functional(|functional| {
            let subject = subjects.next().ok_or(Failure::RosterMetadata(
                "prepared theorem roster is incomplete",
            ))?;
            let execution = subject
                .execute_effect_ir_derived(
                    runtime,
                    semantic_kir,
                    functional.phase.into_source()?.into_verified(),
                    timeout_seconds,
                )
                .map_err(Failure::FinalGraphFunctional)?;
            Ok(AuthenticatedFunctionalVerificationV1 {
                phase: FunctionalProofPhaseV1::Prepared(execution),
                ..functional
            })
        })?;
        if subjects.next().is_some() {
            return Err(Failure::RosterMetadata("unconsumed prepared theorem"));
        }
        prepared.require_functional_phase(FunctionalProofPhaseKindV1::Prepared)?;
        Ok(prepared)
    }

    /// Consume retained executions after the actual schedule, without opening
    /// a runtime, rerunning Verus, or constructing a verified graph. Original
    /// source executions and lineage evidence remain borrowable in this roster.
    pub(crate) fn complete_final_graph_functional(
        self,
        verified_graph: &mut ProductionVerifiedFinalGraphV1,
    ) -> Result<Self, Failure> {
        self.require_functional_phase(FunctionalProofPhaseKindV1::Prepared)?;
        revalidate_graph(verified_graph)?;
        self.require_kernel_roster(&verified_graph.module().kernels)?;
        for root in &self.roots {
            let pending =
                root.verification
                    .prepared_final_execution()
                    .ok_or(Failure::RosterMetadata(
                        "final functional completion requires a prepared root",
                    ))?;
            pending
                .require_exact_subject(
                    pending.source_canonical(),
                    verified_graph.canonical(),
                    verified_graph.final_epoch(),
                    verified_graph.target_contract(),
                )
                .map_err(Failure::FinalGraphFunctional)?;
        }
        let completed = self.map_functional(|functional| {
            let FunctionalProofPhaseV1::Prepared(pending) = functional.phase else {
                return Err(Failure::RosterMetadata(
                    "final functional completion is out of phase",
                ));
            };
            let execution = pending
                .complete_with_verified_graph(verified_graph)
                .map_err(Failure::FinalGraphFunctional)?;
            Ok(AuthenticatedFunctionalVerificationV1 {
                phase: FunctionalProofPhaseV1::Completed(execution),
                ..functional
            })
        })?;
        completed.require_functional_phase(FunctionalProofPhaseKindV1::Completed)?;
        revalidate_graph(verified_graph)?;
        Ok(completed)
    }

    /// Terminal extraction, after lineage has borrowed this roster. The caller
    /// must still perform its exact protected-reference roster and final-owner
    /// validation; these roots confer no additional authority.
    pub(crate) fn into_completed_final_graph_functional_roster(
        self,
    ) -> Result<AuthenticatedCompletedFinalGraphFunctionalRosterV1, Failure> {
        self.require_functional_phase(FunctionalProofPhaseKindV1::Completed)?;
        let roots = self
            .roots
            .into_vec()
            .into_iter()
            .map(|root| {
                let AuthenticatedRankedVerificationV5 {
                    middle_end_evidence,
                    semantic_u32_induction,
                    functional,
                } = root.verification;
                let functional =
                    functional.ok_or(Failure::RosterMetadata("unproved completed root"))?;
                let FunctionalProofPhaseV1::Completed(execution) = functional.phase else {
                    return Err(Failure::RosterMetadata(
                        "completed extraction is out of phase",
                    ));
                };
                Ok(AuthenticatedCompletedFinalGraphFunctionalRootV1 {
                    logical_name: root.logical_name,
                    export_symbol: root.export_symbol,
                    semantic_root: root.semantic_root,
                    semantic_root_identity: root.semantic_root_identity,
                    kernel_binding: root.kernel_binding,
                    source_rank: root.source_rank,
                    _middle_end_evidence: middle_end_evidence,
                    _semantic_u32_induction: semantic_u32_induction,
                    _semantics: functional.semantics,
                    _parallel_contract: functional.parallel_contract,
                    _parallel_report: functional.parallel_report,
                    execution,
                })
            })
            .collect::<Result<Vec<_>, Failure>>()?;
        Ok(AuthenticatedCompletedFinalGraphFunctionalRosterV1 {
            roots: roots.into_boxed_slice(),
            canonical_roster_identity: self.canonical_roster_identity,
            canonical_kernel_order: self.canonical_kernel_order,
        })
    }

    pub(super) fn require_functional_phase(
        &self,
        expected: FunctionalProofPhaseKindV1,
    ) -> Result<(), Failure> {
        self.require_roster_identity()?;
        require_phase_roster(
            self.roots.iter().map(|root| {
                root.verification
                    .functional
                    .as_ref()
                    .map(|functional| functional.phase.kind())
            }),
            expected,
        )?;
        for root in &self.roots {
            require_source_proof_joins(root)?;
        }
        // Final receipts may differ by source root, but never by final subject.
        if let Some(first) = self.roots.first() {
            match first
                .verification
                .functional
                .as_ref()
                .map(|functional| &functional.phase)
            {
                Some(FunctionalProofPhaseV1::Prepared(first)) => {
                    for root in &self.roots {
                        let pending = root
                            .verification
                            .prepared_final_execution()
                            .ok_or(Failure::RosterMetadata("incomplete prepared roster"))?;
                        if !pending.retains_strictly_imported_final_graph_receipt() {
                            return Err(Failure::RosterMetadata("unimported prepared final proof"));
                        }
                        pending
                            .require_exact_subject(
                                first.source_canonical(),
                                first.final_canonical(),
                                first.final_epoch(),
                                first.target_contract(),
                            )
                            .map_err(Failure::FinalGraphFunctional)?;
                    }
                }
                Some(FunctionalProofPhaseV1::Completed(first)) => {
                    for root in &self.roots {
                        let Some(AuthenticatedFunctionalVerificationV1 {
                            phase: FunctionalProofPhaseV1::Completed(completed),
                            ..
                        }) = &root.verification.functional
                        else {
                            return Err(Failure::RosterMetadata("incomplete completed roster"));
                        };
                        let actual = completed.report();
                        let expected = first.report();
                        if actual.source_graph() != expected.source_graph()
                            || actual.final_graph() != expected.final_graph()
                            || actual.final_epoch() != expected.final_epoch()
                            || actual.target_closure() != expected.target_closure()
                            || !completed.retains_strictly_imported_final_graph_receipt()
                        {
                            return Err(Failure::RosterMetadata(
                                "cross-wired completed final subjects",
                            ));
                        }
                    }
                }
                Some(FunctionalProofPhaseV1::Source(_)) => {}
                None => return Err(Failure::RosterMetadata("unproved functional root")),
            }
        }
        Ok(())
    }

    fn require_roster_identity(&self) -> Result<(), Failure> {
        let records = self
            .roots
            .iter()
            .map(authenticated_identity_record)
            .collect::<Vec<_>>();
        require_exact_ranked_kernel_roster_identity_v1(
            &records,
            self.canonical_roster_identity,
            &self.canonical_kernel_order,
        )
    }

    fn require_kernel_roster(&self, kernels: &[Kernel]) -> Result<(), Failure> {
        if kernels.len() != self.roots.len() {
            return Err(Failure::RosterMetadata(
                "incomplete canonical functional kernel roster",
            ));
        }
        for root in &self.roots {
            let mut matches = kernels
                .iter()
                .filter(|kernel| kernel.id.as_str().as_bytes() == root.export_symbol());
            let kernel = matches
                .next()
                .ok_or(Failure::RosterMetadata("missing canonical functional root"))?;
            if matches.next().is_some()
                || kernel.entry.as_str().as_bytes() != root.export_symbol()
                || kernel.domain.rank() != root.source_rank()
            {
                return Err(Failure::RosterMetadata(
                    "cross-wired canonical functional root",
                ));
            }
        }
        Ok(())
    }

    fn require_source_owner(
        &self,
        semantic_kir: &ProductionSemanticKirOwnerV1,
    ) -> Result<(), Failure> {
        semantic_kir
            .verify_equivalence()
            .map_err(Failure::Custody)?;
        let bindings = self
            .roots
            .iter()
            .map(|root| {
                Ok(RankedRootSemanticBindingRecordV1 {
                    export_symbol: root.export_symbol(),
                    semantic_root: root.semantic_root(),
                    semantic_root_identity: root.semantic_root_identity(),
                    kernel_binding: *root.kernel_binding(),
                    function_name: std::str::from_utf8(root.export_symbol()).map_err(|_| {
                        Failure::RosterMetadata("non-UTF8 canonical function binding")
                    })?,
                })
            })
            .collect::<Result<Vec<_>, Failure>>()?;
        validate_ranked_roster_semantic_bindings_v1(semantic_kir.semantic(), &bindings)?;
        self.require_kernel_roster(&semantic_kir.module().kernels)?;
        for root in &self.roots {
            let lowering = semantic_kir
                .ranked_lowering_for_root(root.semantic_root())
                .ok_or(Failure::RosterMetadata(
                    "source owner has no unique retained ranked root",
                ))?;
            let evidence = root.verification.middle_end_evidence();
            let replay = fe2o3_pliron::ProductionMiddleEndEvidenceV5::try_new(
                semantic_kir.semantic(),
                lowering,
                evidence.ranked_ir(),
            )
            .map_err(Failure::MiddleEndEvidence)?;
            let induction =
                fe2o3_mir_model::analyze_expanded_semantic_u32_induction_no_overflow_v1(
                    semantic_kir.semantic_ssa().source_semantic(),
                    semantic_kir.semantic_ssa().execution_expansion(),
                    root.semantic_root(),
                )
                .map_err(Failure::SemanticU32Induction)?;
            if replay.canonical_bytes() != evidence.canonical_bytes()
                || &induction != root.verification.semantic_u32_induction()
                || root
                    .verification
                    .has_authenticated_functional_verification()
                    != lowering.has_retained_policy_checked_refinement_staging()
            {
                return Err(Failure::RosterMetadata(
                    "substituted source ranked or induction evidence",
                ));
            }
        }
        Ok(())
    }

    fn map_functional(
        self,
        mut transform: impl FnMut(
            AuthenticatedFunctionalVerificationV1,
        ) -> Result<AuthenticatedFunctionalVerificationV1, Failure>,
    ) -> Result<Self, Failure> {
        let roots = self
            .roots
            .into_vec()
            .into_iter()
            .map(|root| {
                let verification = root.verification;
                let functional = verification
                    .functional
                    .ok_or(Failure::RosterMetadata("unproved functional root"))?;
                Ok(AuthenticatedRankedVerificationRootV1 {
                    verification: AuthenticatedRankedVerificationV5 {
                        functional: Some(transform(functional)?),
                        ..verification
                    },
                    ..root
                })
            })
            .collect::<Result<Vec<_>, Failure>>()?;
        Ok(Self {
            roots: roots.into_boxed_slice(),
            ..self
        })
    }
}

fn require_phase_roster(
    phases: impl IntoIterator<Item = Option<FunctionalProofPhaseKindV1>>,
    expected: FunctionalProofPhaseKindV1,
) -> Result<(), Failure> {
    let mut nonempty = false;
    for phase in phases {
        nonempty = true;
        if phase != Some(expected) {
            return Err(Failure::RosterMetadata(
                "missing or out-of-phase functional proof",
            ));
        }
    }
    if !nonempty {
        return Err(Failure::RosterMetadata("empty functional proof roster"));
    }
    Ok(())
}

fn require_source_proof_joins(root: &AuthenticatedRankedVerificationRootV1) -> Result<(), Failure> {
    let verification = &root.verification;
    let functional = verification
        .functional
        .as_ref()
        .ok_or(Failure::RosterMetadata("unproved functional root"))?;
    let verified = functional.phase.verified();
    let execution = verified.per_compilation_verus_execution();
    let report = execution.report();
    let derivation = verified.derivation();
    let binding = report.binding();
    let evidence = verification.middle_end_evidence();
    let induction = verification.semantic_u32_induction();
    if !verification.retained_functional_verification_is_coherent()
        || !execution.retains_strictly_imported_signed_receipt()
        || report.pliron_evidence_identity().as_bytes() != evidence.identity().sha256()
        || derivation.kernel_mir().as_bytes() != evidence.source_semantic_identity()
        || derivation.ranked_kernel().as_bytes() != evidence.ranked_kernel_identity()
        || derivation.semantic_contract() != report.contract_identity()
        || derivation.parallel_contract() != report.parallel_contract_identity()
        || report.parallel_contract_identity() != functional.parallel_contract.canonical_sha256()
        || binding.kernel_mir_hash() != derivation.kernel_mir()
        || binding.safe_reference_mir_hash() != derivation.safe_reference_mir()
        || binding.normalized_obligation_effect_ir_hash() != report.obligation_identity()
        || induction.semantic_mir_sha256().as_bytes() != evidence.source_semantic_identity()
        || induction.function() != root.semantic_root()
        || induction.function_identity() != root.semantic_root_identity()
        || induction.grants_authority()
        || induction.authorizes_compiler_transform()
    {
        return Err(Failure::RosterMetadata(
            "cross-wired source proof or lineage evidence",
        ));
    }
    Ok(())
}

fn authenticated_identity_record(
    root: &AuthenticatedRankedVerificationRootV1,
) -> RankedRosterIdentityRecordV1<'_> {
    let middle_end_identity = root.verification.middle_end_evidence().identity();
    let induction = root.verification.semantic_u32_induction();
    RankedRosterIdentityRecordV1 {
        logical_name: root.logical_name(),
        export_symbol: root.export_symbol(),
        semantic_root: root.semantic_root(),
        semantic_root_identity: root.semantic_root_identity(),
        kernel_binding: *root.kernel_binding(),
        source_rank: root.source_rank(),
        middle_end_identity_sha256: *middle_end_identity.sha256(),
        middle_end_identity_byte_len: middle_end_identity.byte_len(),
        induction_semantic_mir_sha256: *induction.semantic_mir_sha256().as_bytes(),
        induction_function: induction.function(),
        induction_function_identity: induction.function_identity(),
        induction_execution_view_identity: induction.execution_view_identity().copied(),
        induction_checked_additions_examined: u64::try_from(induction.checked_additions_examined())
            .unwrap_or(u64::MAX),
        induction_certificate_count: u64::try_from(induction.certificates().len())
            .unwrap_or(u64::MAX),
        induction_work_units: u64::try_from(induction.work_units()).unwrap_or(u64::MAX),
    }
}

fn revalidate_graph(graph: &mut ProductionVerifiedFinalGraphV1) -> Result<(), Failure> {
    graph.revalidate_live().map_err(|error| {
        Failure::FinalGraphFunctional(
            fe2o3_verifier::ProductionFinalGraphFunctionalRefinementErrorV1::FinalGraph(error),
        )
    })
}

/// A completed root still retains its original evidence until terminal receipt
/// extraction. Metadata accessors do not authorize graph or launch operations.
#[must_use = "retain completed functional custody through the exact final handoff"]
pub(crate) struct AuthenticatedCompletedFinalGraphFunctionalRootV1 {
    logical_name: String,
    export_symbol: Box<[u8]>,
    semantic_root: SemanticFunctionIdV1,
    semantic_root_identity: SemanticFunctionIdentityV1,
    kernel_binding: [u8; 32],
    source_rank: u8,
    _middle_end_evidence: fe2o3_pliron::ProductionMiddleEndEvidenceV5,
    _semantic_u32_induction: fe2o3_mir_model::SemanticU32InductionNoOverflowReportV1,
    _semantics: fe2o3_pliron::ProductionReconciledMirPlironSemanticContractV1,
    _parallel_contract: fe2o3_functional_proof::ParallelReferenceContractV1,
    _parallel_report: fe2o3_pliron::ProductionParallelReferenceContractReportV1,
    execution: ProductionFinalGraphFunctionalRefinementExecutionV2,
}

#[must_use = "retain the exact completed root roster through final handoff"]
pub(crate) struct AuthenticatedCompletedFinalGraphFunctionalRosterV1 {
    roots: Box<[AuthenticatedCompletedFinalGraphFunctionalRootV1]>,
    canonical_roster_identity: ProductionRankedKernelRosterIdentityV1,
    canonical_kernel_order: Box<[usize]>,
}

impl AuthenticatedCompletedFinalGraphFunctionalRosterV1 {
    pub(crate) fn into_parts(
        self,
    ) -> (
        Box<[AuthenticatedCompletedFinalGraphFunctionalRootV1]>,
        ProductionRankedKernelRosterIdentityV1,
        Box<[usize]>,
    ) {
        (
            self.roots,
            self.canonical_roster_identity,
            self.canonical_kernel_order,
        )
    }
}

impl AuthenticatedCompletedFinalGraphFunctionalRootV1 {
    pub(crate) fn logical_name(&self) -> &str {
        &self.logical_name
    }
    pub(crate) fn export_symbol(&self) -> &[u8] {
        &self.export_symbol
    }
    pub(crate) const fn semantic_root(&self) -> SemanticFunctionIdV1 {
        self.semantic_root
    }
    pub(crate) const fn semantic_root_identity(&self) -> SemanticFunctionIdentityV1 {
        self.semantic_root_identity
    }
    pub(crate) const fn kernel_binding(&self) -> &[u8; 32] {
        &self.kernel_binding
    }
    pub(crate) const fn source_rank(&self) -> u8 {
        self.source_rank
    }
    pub(crate) fn verified(&self) -> &ProductionEffectIrDerivedVerusVerifiedMirPlironKernelV3 {
        self.execution.verified()
    }
    pub(crate) fn execution(&self) -> &ProductionFinalGraphFunctionalRefinementExecutionV2 {
        &self.execution
    }
    pub(crate) fn into_execution(self) -> ProductionFinalGraphFunctionalRefinementExecutionV2 {
        self.execution
    }
}

#[cfg(test)]
mod tests;
