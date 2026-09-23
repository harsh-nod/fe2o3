use super::*;
pub use general::redundant_store::{
    ProductionCrossBlockForwardingErrorV1, ProductionCrossBlockForwardingOriginV1,
    ProductionCrossBlockForwardingStorageV1, ProductionInductionRefinementErrorV1,
    ProductionInductionRefinementOriginV1, ProductionInductionRefinementStorageV1,
    ProductionOwnedCrossBlockForwardingContinuationV1,
    ProductionOwnedInductionRefinementContinuationV1,
    ProductionOwnedRefinedCrossBlockForwardingContinuationV1,
    ProductionOwnedUnitLocalCrossBlockForwardingContinuationV1,
    ProductionOwnedUnitLocalInductionRefinementContinuationV1,
    ProductionOwnedUnitLocalRefinedCrossBlockForwardingContinuationV1,
    ProductionRefinedCrossBlockForwardingErrorV1, ProductionRefinedCrossBlockForwardingStorageV1,
};
pub use general::redundant_store::{
    ProductionLicmErrorV1, ProductionLicmStorageV1, ProductionOwnedLicmContinuationV1,
    ProductionOwnedUnitLocalLicmContinuationV1,
};
pub use general::redundant_store::{
    ProductionLoopInductionQueryErrorV1, ProductionLoopInductionQueryStorageV1,
    ProductionLoopInductionQueryV1,
};
pub use general::redundant_store::{
    ProductionOwnedSourceLocalOrderContinuationV1, ProductionSourceLocalOrderErrorV1,
    ProductionSourceLocalOrderStorageV1, SourceU32LocalOrderRequestV1,
};

#[path = "production_checked_output_general_policy3_v1.rs"]
mod general;
pub use general::redundant_store::{
    ProductionCommutativeContinuationErrorV1, ProductionCommutativeContinuationStorageV1,
    ProductionOwnedCommutativeContinuationV1, ProductionOwnedPrivateCellPromotionContinuationV1,
    ProductionOwnedRedundantStoreContinuationV1, ProductionOwnedRedundantStoreStorageV1,
    ProductionOwnedUnitLocalCommutativeContinuationV1,
    ProductionOwnedUnitLocalPrivateCellPromotionContinuationV1,
    ProductionOwnedUnitLocalRedundantStoreContinuationV1,
    ProductionPrivateCellPromotionContinuationErrorV1,
    ProductionPrivateCellPromotionContinuationStorageV1, ProductionRedundantStoreAdmissionErrorV1,
    ProductionRedundantStoreAdmissionStorageV1, ProductionRedundantStoreAdmissionV1,
};
pub use general::redundant_store::{
    ProductionLoopPreheaderIncomingOriginV1, ProductionLoopPreheaderOriginV1,
    ProductionLoopPreheaderParameterOriginV1, ProductionLoopPreheadersErrorV1,
    ProductionLoopPreheadersStorageV1, ProductionOwnedLoopPreheadersContinuationV1,
    ProductionOwnedUnitLocalLoopPreheadersContinuationV1,
};

#[path = "production_checked_output_admission_policy4_v1.rs"]
mod policy4;
pub use policy4::{
    ProductionCheckedOutputAdmissionErrorPolicy4V1, ProductionCheckedOutputOwnerPolicy4V1,
    ProductionUnitLocalErasedCheckedOutputOwnerPolicy4V1,
};

#[path = "production_checked_output_admission_policy5_v1.rs"]
mod policy5;
pub use policy5::{
    ProductionCheckedOutputAdmissionErrorPolicy5V1, ProductionCheckedOutputOwnerPolicy5V1,
    ProductionUnitLocalErasedCheckedOutputOwnerPolicy5V1,
};

#[path = "production_checked_output_admission_policy6_v1.rs"]
mod policy6;
pub use policy6::{
    ProductionCheckedOutputAdmissionErrorPolicy6V1, ProductionCheckedOutputOwnerPolicy6V1,
    ProductionUnitLocalErasedCheckedOutputOwnerPolicy6V1,
};

#[derive(Clone, Copy)]
enum OutputAdmissionKindV1 {
    ClosedScalar,
    GuardedGlobal,
}

/// Failure to close an explicitly supported Policy3 admission grammar.
#[derive(Debug)]
pub enum ProductionCheckedOutputAdmissionErrorPolicy3V1 {
    /// The canonical work or storage contract was not satisfied.
    Resource(AssertOriginResourceV1),
    /// Original source/ranked admission failed.
    Source(ProductionSemanticKirErrorV1),
    /// Exact source, bound input, and actual output custody failed.
    SourceOutput(ProductionSourceOutputErrorV1),
    /// The target-binding coordinate relation failed.
    Coordinates(fe2o3_kernel_analysis::CanonicalKirCoordinatePreservationErrorV1),
    /// Fresh actual-output formal analysis failed.
    Formal(crate::ProductionFormalMemoryErrorV1),
    /// Private address safety is not admitted by this version.
    PrivateAddressR2,
    /// A complete census encountered a form outside the closed subset.
    Unsupported {
        /// The exact graph or source phase being checked.
        phase: &'static str,
        /// The refused contract, not a detached permission flag.
        detail: &'static str,
    },
}

impl fmt::Display for ProductionCheckedOutputAdmissionErrorPolicy3V1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "closed Policy3 output admission failed: {self:?}"
        )
    }
}

impl Error for ProductionCheckedOutputAdmissionErrorPolicy3V1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Source(error) => Some(error),
            Self::SourceOutput(error) => Some(error),
            Self::Coordinates(error) => Some(error),
            Self::Formal(error) => Some(error),
            Self::PrivateAddressR2 | Self::Unsupported { .. } => None,
        }
    }
}

/// Move-only final safety custody for an explicitly checked Policy3 subset.
///
/// `try_admit` retains the closed scalar contract: roots have one returning source and
/// executable block, total scalar recipes and no calls, memory, private
/// addresses, assertions, conditional control, compiler ordering or borrowed
/// interfaces. Ranked projection may use a finite unconditional chain covering
/// every block, including its synthetic entry prologue. Source, ranked, N, B
/// and O are independently censused; formal completeness alone does not
/// establish these restrictions. The source owner remains historical N
/// custody, while `output()` always returns the actual checked Policy3 O.
/// `try_admit_general_v1` additionally admits its documented guarded-global
/// grammar, retaining runtime assertions, bounds and alias requirements.
///
/// This is not artifact, target, descriptor, runtime-launch or publication
/// authority. Fixed formal witnesses do not authenticate dynamic launches.
/// Neither legacy final admission nor borrowed structural reports can replace
/// this consuming construction.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{ProductionCheckedOutputOwnerPolicy3V1,
///     CheckedOutputFormalMemoryAnalysisPolicy3V1};
/// fn substitute(report: CheckedOutputFormalMemoryAnalysisPolicy3V1<'_>)
///     -> ProductionCheckedOutputOwnerPolicy3V1 { report }
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{ProductionCheckedOutputOwnerPolicy3V1,
///     ProductionFormalMemoryOwnerV1};
/// fn substitute(owner: ProductionCheckedOutputOwnerPolicy3V1)
///     -> ProductionFormalMemoryOwnerV1 { owner }
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionCheckedOutputOwnerPolicy3V1;
/// use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12;
/// fn escape(owner: ProductionCheckedOutputOwnerPolicy3V1)
///     -> &'static VerifiedCanonicalKernelIrModuleV12 { owner.output() }
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionCheckedOutputOwnerPolicy3V1;
/// fn duplicate(owner: ProductionCheckedOutputOwnerPolicy3V1) {
///     let first = owner;
///     let second = owner;
///     drop((first, second));
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionCheckedOutputOwnerPolicy3V1;
/// fn clone_owner(owner: &ProductionCheckedOutputOwnerPolicy3V1)
///     -> ProductionCheckedOutputOwnerPolicy3V1 { owner.clone() }
/// ```
#[must_use = "dropping this owner abandons exact checked-output safety custody"]
pub struct ProductionCheckedOutputOwnerPolicy3V1 {
    source: ProductionSemanticKirOwnerV1,
    bound: fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    checked: fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1,
    kernels: Box<[FormalMemoryObligations]>,
    source_storage_floor: usize,
    admission: OutputAdmissionKindV1,
}

impl fmt::Debug for ProductionCheckedOutputOwnerPolicy3V1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionCheckedOutputOwnerPolicy3V1")
            .field("output", &self.output().canonical().identity())
            .field("kernels", &self.kernels.len())
            .finish_non_exhaustive()
    }
}

impl ProductionCheckedOutputOwnerPolicy3V1 {
    /// Consumes source/ranked custody for the checked guarded-global grammar.
    ///
    /// This accepts scalar roots and ordinary nonvolatile global scalar accesses,
    /// with exact checked runtime assertion success/failure control. Fixed-cell
    /// private scalar memory requires a separate complete source/address and
    /// initialization census. Local helpers, collective/ordered operations and
    /// unsupported scalar recipes remain refused. Bounds and alias requirements
    /// are retained obligations, not authenticated runtime bindings or launches.
    ///
    /// Source/capture, B's separate replay receipt and checked O must remain
    /// caller-reserved as for `try_admit`. New inventories, transport and census
    /// scratch use the supplied ledger and restore its incoming floor on every
    /// result. Existing source/ranked reconstruction, formal analysis/results
    /// and wrapper bookkeeping retain their non-canonical accounting exclusion.
    pub fn try_admit_general_v1(
        receipt: ProductionMaterializedRankedModuleReceiptV1,
        bound: fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
        checked: fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<Self, ProductionCheckedOutputAdmissionErrorPolicy3V1> {
        use ProductionCheckedOutputAdmissionErrorPolicy3V1 as E;
        output_admission_charge_v1(budget, 8)?;
        let incoming = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let source_storage_floor = receipt
            .materialized
            .unit_local_source_storage_floor_v1()
            .map_err(E::Source)?;
        let minimum = source_storage_floor
            .checked_add(checked.storage().retained_storage())
            .ok_or(E::Resource(AssertOriginResourceV1::Arithmetic))?;
        if incoming < minimum {
            return Err(E::Resource(AssertOriginResourceV1::Accounting));
        }
        receipt
            .materialized
            .require_legacy_helper_policy_v1("Policy3 general output admission")
            .map_err(E::Source)?;
        validate_source_ranked_roster_v1(
            &receipt.materialized.semantic_ssa,
            &receipt.materialized.source_launch,
            &receipt.roots,
        )
        .map_err(E::Source)?;
        output_admission_charge_v1(budget, 2)?;
        if receipt.roots.is_empty()
            || receipt.roots.len() != receipt.materialized.executable().module().kernels.len()
        {
            return Err(output_admission_unsupported_v1(
                "ranked",
                "complete nonempty root roster",
            ));
        }
        let source =
            ProductionSemanticKirOwnerV1::try_attach_materialized_ranked_checks_with_budget_v1(
                receipt, budget,
            )
            .map_err(E::Source)?;
        let kernels = general::check_general_output_v1(&source, &bound, &checked, budget)?;
        if ledger != budget.work_ledger_identity_v1() || incoming != budget.storage() {
            return Err(E::Resource(AssertOriginResourceV1::Accounting));
        }
        Ok(Self {
            source,
            bound,
            checked,
            kernels,
            source_storage_floor,
            admission: OutputAdmissionKindV1::GuardedGlobal,
        })
    }

    /// Consumes actual ranked/source custody, canonical B, and checked Policy3 O.
    ///
    /// Reserve the source's retained analysis/capture payload, B's separate
    /// replay receipt, and the checked owner's receipt before entry, keeping
    /// them reserved until this owner (or failed input transfer) is dropped.
    /// B's receipt remains a caller contract: a numeric floor does not establish
    /// allocation custody. Exact N/B coordinates and B/history bytes establish
    /// endpoint identity independently; no allocation epoch is invented.
    ///
    /// New census work and the existing source/output workspace use this ledger;
    /// no new census buffers are allocated. Existing source/ranked attachment,
    /// reconstruction, formal scratch/results and this wrapper's bookkeeping
    /// retain their non-canonical-ledger accounting boundary. Input reservations
    /// are not released on success or failure; the caller owns those transfers.
    pub fn try_admit(
        receipt: ProductionMaterializedRankedModuleReceiptV1,
        bound: fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
        checked: fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<Self, ProductionCheckedOutputAdmissionErrorPolicy3V1> {
        use ProductionCheckedOutputAdmissionErrorPolicy3V1 as E;
        output_admission_charge_v1(budget, 8)?;
        let incoming = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let source_storage_floor = receipt
            .materialized
            .unit_local_source_storage_floor_v1()
            .map_err(E::Source)?;
        let minimum = source_storage_floor
            .checked_add(checked.storage().retained_storage())
            .ok_or(E::Resource(AssertOriginResourceV1::Arithmetic))?;
        if incoming < minimum {
            return Err(E::Resource(AssertOriginResourceV1::Accounting));
        }
        receipt
            .materialized
            .require_legacy_helper_policy_v1("Policy3 output admission")
            .map_err(E::Source)?;
        check_output_source_v1(receipt.materialized.semantic_ssa.source_semantic(), budget)?;
        validate_source_ranked_roster_v1(
            &receipt.materialized.semantic_ssa,
            &receipt.materialized.source_launch,
            &receipt.roots,
        )
        .map_err(E::Source)?;
        output_admission_charge_v1(budget, 2)?;
        if receipt.roots.is_empty()
            || receipt.roots.len() != receipt.materialized.executable().module().kernels.len()
        {
            return Err(output_admission_unsupported_v1(
                "ranked",
                "nonempty complete root roster",
            ));
        }
        for root in &receipt.roots {
            check_output_ranked_v1(
                &root.lowering,
                &root.access_sources,
                &root.executable_effect_sources,
                budget,
            )?;
        }
        check_output_module_v1(receipt.materialized.executable().module(), "N", budget)?;
        check_output_module_v1(bound.module(), "B", budget)?;
        check_output_module_v1(checked.owner().module(), "O", budget)?;
        let facts = receipt
            .with_checked_private_array_output_policy3_v1(
                &bound,
                &checked,
                budget,
                |scope, budget| scope.len(budget),
            )
            .map_err(E::SourceOutput)?;
        if facts != 0 {
            return Err(E::PrivateAddressR2);
        }
        let source =
            ProductionSemanticKirOwnerV1::try_attach_materialized_ranked_checks_with_budget_v1(
                receipt, budget,
            )
            .map_err(E::Source)?;
        source
            .verify_equivalence_with_budget_v1(budget)
            .map_err(E::Source)?;
        let kernels = crate::production_formal_memory_v1::derive_complete_output_obligations_v1(
            checked.owner().module(),
        )
        .map_err(E::Formal)?;
        check_output_obligations_v1(&kernels, budget)?;
        if ledger != budget.work_ledger_identity_v1() || incoming != budget.storage() {
            return Err(E::Resource(AssertOriginResourceV1::Accounting));
        }
        Ok(Self {
            source,
            bound,
            checked,
            kernels,
            source_storage_floor,
            admission: OutputAdmissionKindV1::ClosedScalar,
        })
    }

    /// Historical N's source/ranked owner, never the executable O accessor.
    pub const fn source_semantic_kir(&self) -> &ProductionSemanticKirOwnerV1 {
        &self.source
    }

    /// The exact canonical B whose bytes the checked optimizer history names.
    pub const fn bound(&self) -> &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12 {
        &self.bound
    }

    /// The retained actual Policy3 execution and semantic-check owner.
    pub const fn checked_output(&self) -> &fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1 {
        &self.checked
    }

    /// The only final executable endpoint: actual independently checked O.
    pub fn output(&self) -> &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12 {
        self.checked.owner()
    }

    /// Fresh O obligations in its exact nonempty ordered kernel roster.
    pub fn kernels(&self) -> &[FormalMemoryObligations] {
        &self.kernels
    }

    /// Final safety custody alone does not authorize artifacts or launches.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }

    /// Source/capture plus checked-output floor, excluding caller-reserved B.
    /// This numeric requirement does not authenticate allocation custody.
    pub fn retained_input_storage_floor_v1(
        &self,
    ) -> Result<usize, ProductionCheckedOutputAdmissionErrorPolicy3V1> {
        self.source_storage_floor
            .checked_add(self.checked.storage().retained_storage())
            .ok_or(ProductionCheckedOutputAdmissionErrorPolicy3V1::Resource(
                AssertOriginResourceV1::Arithmetic,
            ))
    }

    /// Rechecks the closed subset, canonical endpoint relation and fresh O facts.
    /// This preserves the construction method's explicit accounting exclusions.
    pub fn verify_equivalence(
        &self,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> Result<(), ProductionCheckedOutputAdmissionErrorPolicy3V1> {
        use ProductionCheckedOutputAdmissionErrorPolicy3V1 as E;
        output_admission_charge_v1(budget, 8)?;
        let incoming = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let minimum = self
            .source_storage_floor
            .checked_add(self.checked.storage().retained_storage())
            .ok_or(E::Resource(AssertOriginResourceV1::Arithmetic))?;
        if incoming < minimum {
            return Err(E::Resource(AssertOriginResourceV1::Accounting));
        }
        if matches!(self.admission, OutputAdmissionKindV1::GuardedGlobal) {
            let fresh =
                general::check_general_output_v1(&self.source, &self.bound, &self.checked, budget)?;
            if fresh != self.kernels {
                return Err(E::Formal(
                    crate::ProductionFormalMemoryErrorV1::ObligationMismatch,
                ));
            }
            if ledger != budget.work_ledger_identity_v1() || incoming != budget.storage() {
                return Err(E::Resource(AssertOriginResourceV1::Accounting));
            }
            return Ok(());
        }
        self.source
            .verify_equivalence_with_budget_v1(budget)
            .map_err(E::Source)?;
        check_output_source_v1(self.source.semantic().semantic(), budget)?;
        for root in &self.source.generic_checks {
            check_output_ranked_v1(
                &root.lowering,
                &root.access_sources,
                &root.executable_effect_sources,
                budget,
            )?;
        }
        let original = self
            .source
            .pre_ranked_executable()
            .ok_or_else(|| output_admission_unsupported_v1("N", "connected source custody"))?;
        check_output_module_v1(original.module(), "N", budget)?;
        check_output_module_v1(self.bound.module(), "B", budget)?;
        check_output_module_v1(self.output().module(), "O", budget)?;
        let coordinate_storage = {
            let (_coordinates, storage) =
                fe2o3_kernel_analysis::check_canonical_kir_coordinate_preservation_v1(
                    original,
                    &self.bound,
                    budget,
                )
                .map_err(E::Coordinates)?;
            budget
                .reserve_storage(storage.retained_storage())
                .map_err(E::Resource)?;
            storage
        };
        budget
            .release_storage(coordinate_storage.retained_storage())
            .map_err(E::Resource)?;
        let bytes = self.bound.canonical().canonical_bytes();
        output_admission_charge_v1(
            budget,
            bytes
                .len()
                .checked_add(1)
                .ok_or(E::Resource(AssertOriginResourceV1::Arithmetic))?,
        )?;
        if bytes != self.checked.native_input_audit_bytes() {
            return Err(E::SourceOutput(ProductionSourceOutputErrorV1::InputCustody));
        }
        let fresh = crate::production_formal_memory_v1::derive_complete_output_obligations_v1(
            self.output().module(),
        )
        .map_err(E::Formal)?;
        check_output_obligations_v1(&fresh, budget)?;
        if fresh != self.kernels {
            return Err(E::Formal(
                crate::ProductionFormalMemoryErrorV1::ObligationMismatch,
            ));
        }
        if ledger != budget.work_ledger_identity_v1() || incoming != budget.storage() {
            return Err(E::Resource(AssertOriginResourceV1::Accounting));
        }
        Ok(())
    }
}

fn output_admission_charge_v1(
    budget: &mut AssertOriginBudgetV1<'_>,
    amount: usize,
) -> Result<(), ProductionCheckedOutputAdmissionErrorPolicy3V1> {
    budget
        .charge_work(amount)
        .map_err(ProductionCheckedOutputAdmissionErrorPolicy3V1::Resource)
}

fn output_admission_unsupported_v1(
    phase: &'static str,
    detail: &'static str,
) -> ProductionCheckedOutputAdmissionErrorPolicy3V1 {
    ProductionCheckedOutputAdmissionErrorPolicy3V1::Unsupported { phase, detail }
}

fn check_output_obligations_v1(
    kernels: &[FormalMemoryObligations],
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionCheckedOutputAdmissionErrorPolicy3V1> {
    output_admission_charge_v1(budget, 1)?;
    if kernels.is_empty() {
        return Err(output_admission_unsupported_v1(
            "formal O",
            "nonempty kernel roster",
        ));
    }
    for kernel in kernels {
        output_admission_charge_v1(budget, 6)?;
        if !kernel.allocations().is_empty()
            || !kernel.accesses().is_empty()
            || !kernel.bounds_requirements().is_empty()
            || !kernel.runtime_alias_requirements().is_empty()
            || !kernel.inter_invocation_conflicts().is_empty()
        {
            return Err(output_admission_unsupported_v1(
                "formal O",
                "memory-free obligations",
            ));
        }
    }
    Ok(())
}

fn check_output_ranked_v1(
    lowering: &ProductionRankedKernelLoweringInputV1,
    accesses: &[ProductionRankedAccessSourceV1],
    effects: &[ProductionRankedExecutableEffectSourceV1],
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionCheckedOutputAdmissionErrorPolicy3V1> {
    output_admission_charge_v1(budget, 7)?;
    let blocks = lowering.kernel().blocks();
    if !lowering.all_mandatory_reports_are_clean()
        || !accesses.is_empty()
        || !effects.is_empty()
        || blocks.is_empty()
    {
        return Err(output_admission_unsupported_v1(
            "ranked",
            "closed scalar effects and returning control",
        ));
    }
    // Charge every stored block, including blocks not on the entry path. The
    // successful schedule is 7 + 6 * blocks + operations, with no scratch.
    for block in blocks {
        output_admission_charge_v1(budget, 3)?;
        if block.index_argument_count() != 0 {
            return Err(output_admission_unsupported_v1(
                "ranked",
                "no block arguments",
            ));
        }
        for operation in block.operations() {
            output_admission_charge_v1(budget, 1)?;
            match operation {
                ProductionRankedOperationV1::ExecutionLayout { .. }
                | ProductionRankedOperationV1::IndexConstant { .. } => {}
                _ => {
                    return Err(output_admission_unsupported_v1(
                        "ranked",
                        "closed scalar operation",
                    ));
                }
            }
        }
    }
    // A deterministic sole-successor walk reaching Return on exactly the Bth
    // visit cannot repeat a block, so it covers all B blocks without a bitmap.
    let mut current = 0usize;
    for visit in 0..blocks.len() {
        output_admission_charge_v1(budget, 3)?;
        let block = blocks.get(current).ok_or_else(|| {
            output_admission_unsupported_v1("ranked", "linear target outside block roster")
        })?;
        match block.terminator() {
            fe2o3_pliron::ProductionRankedTerminatorV1::Return if visit == blocks.len() - 1 => {
                return Ok(());
            }
            fe2o3_pliron::ProductionRankedTerminatorV1::Branch { target } => {
                current = usize::try_from(*target).map_err(|_| {
                    ProductionCheckedOutputAdmissionErrorPolicy3V1::Resource(
                        AssertOriginResourceV1::Arithmetic,
                    )
                })?;
            }
            _ => {
                return Err(output_admission_unsupported_v1(
                    "ranked",
                    "complete finite unconditional chain to Return",
                ));
            }
        }
    }
    Err(output_admission_unsupported_v1(
        "ranked",
        "complete finite unconditional chain to Return",
    ))
}

fn output_source_type_v1(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<bool, ProductionCheckedOutputAdmissionErrorPolicy3V1> {
    output_admission_charge_v1(budget, 2)?;
    match types
        .get(ty.index() as usize)
        .map(SemanticTypeDeclV1::shape)
    {
        Some(SemanticTypeShapeV1::Unit) => Ok(true),
        Some(SemanticTypeShapeV1::Scalar(
            SemanticScalarTypeV1::Bool
            | SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 8 | 16 | 32 | 64,
            },
        )) => Ok(false),
        Some(SemanticTypeShapeV1::Array { .. }) => {
            Err(ProductionCheckedOutputAdmissionErrorPolicy3V1::PrivateAddressR2)
        }
        _ => Err(output_admission_unsupported_v1(
            "source",
            "unsigned scalar/Unit types only; no borrowed interface",
        )),
    }
}

fn output_source_place_v1(
    types: &[SemanticTypeDeclV1],
    place: &SemanticPlaceV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionCheckedOutputAdmissionErrorPolicy3V1> {
    output_admission_charge_v1(budget, 1)?;
    if !place.projections().is_empty() {
        return Err(ProductionCheckedOutputAdmissionErrorPolicy3V1::PrivateAddressR2);
    }
    output_source_type_v1(types, place.ty(), budget)?;
    Ok(())
}

fn check_output_source_v1(
    source: &AdmittedInertSemanticMirV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionCheckedOutputAdmissionErrorPolicy3V1> {
    output_admission_charge_v1(budget, 4)?;
    if source.roots().is_empty()
        || source.functions().len() != source.roots().len()
        || !source.statics().is_empty()
        || !source.allocations().is_empty()
    {
        return Err(output_admission_unsupported_v1(
            "source",
            "complete scalar root-only module",
        ));
    }
    let types = source.types();
    for function in source.functions() {
        output_admission_charge_v1(budget, 10)?;
        let abi = function.abi();
        if function.role() != SemanticFunctionRoleV1::KernelRoot
            || function.blocks().len() != 1
            || function.entry().index() != 0
            || abi.can_unwind()
            || abi.c_variadic()
            || !abi.hidden_arguments().is_empty()
            || !matches!(abi.return_value().mode(), SemanticAbiPassModeV1::Ignore)
            || abi.return_value().adjusted().is_some()
            || abi.return_value().pointee_override().is_some()
            || !output_source_type_v1(types, abi.return_type(), budget)?
        {
            return Err(output_admission_unsupported_v1(
                "source",
                "scalar root ABI and single block",
            ));
        }
        for ty in abi.source_input_types() {
            output_source_type_v1(types, *ty, budget)?;
        }
        for ownership in abi.source_argument_ownership() {
            output_admission_charge_v1(budget, 1)?;
            if *ownership != SemanticSourceArgumentOwnershipV1::ByValue {
                return Err(output_admission_unsupported_v1(
                    "source",
                    "borrowed argument",
                ));
            }
        }
        for argument in abi.arguments() {
            output_admission_charge_v1(budget, 4)?;
            if !argument.is_source()
                || !matches!(argument.mode(), SemanticAbiPassModeV1::Direct(_))
                || argument.value().adjusted().is_some()
                || argument.value().pointee_override().is_some()
            {
                return Err(output_admission_unsupported_v1(
                    "source",
                    "direct scalar arguments",
                ));
            }
            output_source_type_v1(types, argument.ty(), budget)?;
        }
        for local in function.locals() {
            output_source_type_v1(types, local.ty(), budget)?;
        }
        let block = &function.blocks()[0];
        output_admission_charge_v1(budget, 1)?;
        if !matches!(block.terminator().kind(), SemanticTerminatorKindV1::Return) {
            return Err(output_admission_unsupported_v1(
                "source",
                "Return only; no assertions or branches",
            ));
        }
        for statement in block.statements() {
            output_admission_charge_v1(budget, 1)?;
            let assignment = match statement.kind() {
                SemanticStatementKindV1::Nop
                | SemanticStatementKindV1::StorageLive(_)
                | SemanticStatementKindV1::StorageDead(_) => continue,
                SemanticStatementKindV1::Assign(assignment) => assignment,
                _ => {
                    return Err(output_admission_unsupported_v1(
                        "source",
                        "total scalar assignment",
                    ));
                }
            };
            output_source_place_v1(types, assignment.destination(), budget)?;
            output_source_type_v1(types, assignment.value().result_type(), budget)?;
            let kind = assignment.value().kind();
            output_admission_charge_v1(budget, 1)?;
            match kind {
                SemanticRvalueKindV1::Use(_)
                | SemanticRvalueKindV1::Unary {
                    operation: SemanticUnaryOpV1::Not,
                    ..
                }
                | SemanticRvalueKindV1::Binary {
                    operation:
                        SemanticBinaryOpV1::BitAnd
                        | SemanticBinaryOpV1::BitOr
                        | SemanticBinaryOpV1::BitXor
                        | SemanticBinaryOpV1::Equal
                        | SemanticBinaryOpV1::NotEqual
                        | SemanticBinaryOpV1::LessThan
                        | SemanticBinaryOpV1::LessOrEqual
                        | SemanticBinaryOpV1::GreaterThan
                        | SemanticBinaryOpV1::GreaterOrEqual,
                    ..
                } => {}
                _ => {
                    return Err(output_admission_unsupported_v1(
                        "source",
                        "total scalar recipe",
                    ));
                }
            }
            kind.try_visit_operands(|operand| {
                output_admission_charge_v1(budget, 1)?;
                match operand {
                    SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
                        output_source_place_v1(types, place, budget)
                    }
                    SemanticOperandV1::Constant(constant) => {
                        output_source_type_v1(types, constant.ty(), budget)?;
                        if !matches!(constant.value(), SemanticConstantValueV1::Scalar(_)) {
                            return Err(output_admission_unsupported_v1(
                                "source",
                                "literal scalar constant",
                            ));
                        }
                        Ok(())
                    }
                }
            })?;
        }
    }
    Ok(())
}

fn output_scalar_type_v1(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Scalar(
            ScalarType::Bool | ScalarType::U8 | ScalarType::U16 | ScalarType::U32 | ScalarType::U64
        )
    )
}

fn check_output_module_v1(
    module: &Module,
    phase: &'static str,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<(), ProductionCheckedOutputAdmissionErrorPolicy3V1> {
    output_admission_charge_v1(budget, 2)?;
    if module.kernels.is_empty() || module.functions.len() != module.kernels.len() {
        return Err(output_admission_unsupported_v1(
            phase,
            "complete root-only module",
        ));
    }
    for function in &module.functions {
        output_admission_charge_v1(budget, 4)?;
        if function.role != fe2o3_kernel_ir::FunctionRole::KernelEntry
            || !function.signature.results.is_empty()
        {
            return Err(output_admission_unsupported_v1(
                phase,
                "scalar kernel signature",
            ));
        }
        for ty in &function.signature.parameters {
            output_admission_charge_v1(budget, 1)?;
            if !output_scalar_type_v1(ty) {
                return Err(output_admission_unsupported_v1(
                    phase,
                    "unsigned scalar parameters",
                ));
            }
        }
        let body = function
            .body
            .as_ref()
            .ok_or_else(|| output_admission_unsupported_v1(phase, "no declarations"))?;
        if body.blocks.len() != 1 {
            return Err(output_admission_unsupported_v1(
                phase,
                "one block, including unreachable bodies",
            ));
        }
        let block = &body.blocks[0];
        output_admission_charge_v1(budget, 2)?;
        if !block.parameters.is_empty()
            || !matches!(&block.terminator, Some(Terminator::Return { values }) if values.is_empty())
        {
            return Err(output_admission_unsupported_v1(phase, "empty Return only"));
        }
        for operation in &block.operations {
            output_admission_charge_v1(budget, 3)?;
            if !operation.compiler_ordering_effects_v12().is_empty() {
                return Err(output_admission_unsupported_v1(
                    phase,
                    "no compiler ordering",
                ));
            }
            operation.try_visit_local_memory_effects_v1(|effect| {
                output_admission_charge_v1(budget, 1)?;
                match effect {
                    fe2o3_kernel_ir::KirLocalMemoryEffectRefV1::Allocate(AddressSpace::Private)
                    | fe2o3_kernel_ir::KirLocalMemoryEffectRefV1::Read(AddressSpace::Private)
                    | fe2o3_kernel_ir::KirLocalMemoryEffectRefV1::Write(AddressSpace::Private) => {
                        Err(ProductionCheckedOutputAdmissionErrorPolicy3V1::PrivateAddressR2)
                    }
                    _ => Err(output_admission_unsupported_v1(phase, "no memory effects")),
                }
            })?;
            match &operation.kind {
                OperationKind::Constant(_)
                | OperationKind::Unary {
                    op: UnaryOp::Not, ..
                }
                | OperationKind::Binary {
                    op: BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor,
                    ..
                }
                | OperationKind::Compare { .. } => {}
                _ => {
                    return Err(output_admission_unsupported_v1(
                        phase,
                        "closed total scalar opcode",
                    ));
                }
            }
            for value in &operation.results {
                output_admission_charge_v1(budget, 1)?;
                if !output_scalar_type_v1(&value.ty) {
                    return Err(output_admission_unsupported_v1(
                        phase,
                        "unsigned scalar results",
                    ));
                }
            }
        }
    }
    Ok(())
}
