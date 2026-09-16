//! Owner-held formal memory admission for verified target-neutral Kernel IR.

use std::{error::Error, fmt};

use fe2o3_kernel_ir::{
    ExplicitLaunchExtent, FormalIndexWidth, FormalMemoryIncompleteReason,
    FormalMemoryObligationAnalysis, FormalMemoryObligationError, FormalMemoryObligations,
    InterInvocationConflictRequirement, LaunchDomain, LaunchExtent,
    derive_kernel_memory_obligations_for_launch,
};

use crate::{
    ProductionMemoryDischargeFailureV1, ProductionSemanticKirErrorV1, ProductionSemanticKirOwnerV1,
};

/// The per-active-axis extent of the smallest structural witness launch.
pub const PRODUCTION_FORMAL_MEMORY_WITNESS_EXTENT_V1: u64 = 2;
const FORMAL_REASON_DIAGNOSTIC_LIMIT_V1: usize = 16;

fn format_formal_reasons(
    formatter: &mut fmt::Formatter<'_>,
    reasons: &[FormalMemoryIncompleteReason],
) -> fmt::Result {
    formatter
        .debug_list()
        .entries(reasons.iter().take(FORMAL_REASON_DIAGNOSTIC_LIMIT_V1))
        .finish()?;
    if reasons.len() > FORMAL_REASON_DIAGNOSTIC_LIMIT_V1 {
        write!(
            formatter,
            " ({} more)",
            reasons.len() - FORMAL_REASON_DIAGNOSTIC_LIMIT_V1
        )?;
    }
    Ok(())
}

/// Fail-closed diagnostics from production formal-memory admission.
#[derive(Debug)]
pub enum ProductionFormalMemoryErrorV1 {
    /// The retained semantic-to-Kernel-IR owner no longer verifies.
    SemanticKir(ProductionSemanticKirErrorV1),
    /// Formal extraction requires a nonempty selected-kernel roster.
    KernelCount {
        /// Number of kernels present in the verified module.
        actual: usize,
    },
    /// Formal extraction rejected the verified module or selected kernel.
    Analysis(FormalMemoryObligationError),
    /// At least one memory effect has no complete formal derivation.
    Incomplete {
        /// Canonically ordered reasons formal extraction was incomplete.
        reasons: Box<[FormalMemoryIncompleteReason]>,
    },
    /// Compiler-owned LDS effects no longer match their exact semantic lowering spans.
    CompilerOwnedWorkgroupDischarge {
        /// Canonically ordered internal workgroup-memory reasons.
        reasons: Box<[FormalMemoryIncompleteReason]>,
        /// Stable failure at the semantic/KIR composition boundary.
        detail: ProductionMemoryDischargeFailureV1,
    },
    /// Ranked checks could not correlate a dynamic index to its exact access.
    UnsupportedIndexDischarge {
        /// Canonically ordered formal reasons that required ranked discharge.
        reasons: Box<[FormalMemoryIncompleteReason]>,
        /// Stable failure at the ranked/Kernel-IR composition boundary.
        detail: ProductionMemoryDischargeFailureV1,
    },
    /// Ranked checks could not discharge structurally guarded accesses.
    GuardedAccessDischarge {
        /// Canonically ordered formal reasons that required ranked discharge.
        reasons: Box<[FormalMemoryIncompleteReason]>,
        /// Stable failure at the ranked/Kernel-IR composition boundary.
        detail: ProductionMemoryDischargeFailureV1,
    },
    /// The modeled memory accesses contain an inherent cross-invocation conflict.
    InterInvocationConflicts {
        /// Canonically ordered conflicts that prevent race-free admission.
        conflicts: Box<[InterInvocationConflictRequirement]>,
    },
    /// Re-derived obligations no longer match the retained admission witness.
    ObligationMismatch,
}

impl fmt::Display for ProductionFormalMemoryErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SemanticKir(error) => write!(formatter, "verified semantic KIR failed: {error}"),
            Self::KernelCount { actual } => write!(
                formatter,
                "formal memory admission requires a nonempty kernel roster; found {actual}",
            ),
            Self::Analysis(error) => write!(formatter, "formal memory extraction failed: {error}"),
            Self::Incomplete { reasons } => {
                write!(
                    formatter,
                    "formal memory extraction is incomplete for {} reason(s): ",
                    reasons.len(),
                )?;
                format_formal_reasons(formatter, reasons)
            }
            Self::CompilerOwnedWorkgroupDischarge { reasons, detail } => {
                write!(
                    formatter,
                    "exact compiler-owned workgroup lowering could not discharge {} internal reason(s): {detail}; locations: ",
                    reasons.len(),
                )?;
                format_formal_reasons(formatter, reasons)
            }
            Self::GuardedAccessDischarge { reasons, detail } => {
                write!(
                    formatter,
                    "ranked checks could not discharge {} guarded access reason(s): {detail}; locations: ",
                    reasons.len(),
                )?;
                format_formal_reasons(formatter, reasons)
            }
            Self::UnsupportedIndexDischarge { reasons, detail } => {
                write!(
                    formatter,
                    "ranked checks could not discharge {} unsupported index reason(s): {detail}; locations: ",
                    reasons.len(),
                )?;
                format_formal_reasons(formatter, reasons)
            }
            Self::InterInvocationConflicts { conflicts } => write!(
                formatter,
                "formal memory admission found {} inter-invocation conflict(s)",
                conflicts.len(),
            ),
            Self::ObligationMismatch => formatter.write_str(
                "re-derived formal memory obligations differ from the retained admission witness",
            ),
        }
    }
}

impl Error for ProductionFormalMemoryErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::SemanticKir(error) => Some(error),
            Self::Analysis(error) => Some(error),
            Self::KernelCount { .. }
            | Self::Incomplete { .. }
            | Self::CompilerOwnedWorkgroupDischarge { .. }
            | Self::UnsupportedIndexDischarge { .. }
            | Self::GuardedAccessDischarge { .. }
            | Self::InterInvocationConflicts { .. }
            | Self::ObligationMismatch => None,
        }
    }
}

/// Move-only owner of exact semantic KIR and composed memory-safety evidence.
///
/// Admission uses extent two on every active launch axis so cross-invocation
/// affine overlap is observable in each dimension. Affine effects retain
/// complete formal obligations. Dynamic index expressions may instead be
/// discharged by the exact owner-held ranked bounds/race receipt; no other
/// incomplete formal reason is admitted. Retained bounds and alias records are
/// runtime obligations, not evidence about any concrete launch or allocation.
#[must_use = "dropping formal admission abandons the target-neutral safety witness"]
pub struct ProductionFormalMemoryOwnerV1 {
    semantic_kir: ProductionSemanticKirOwnerV1,
    kernels: Box<[ProductionFormalMemoryKernelV1]>,
}

/// Exact formal-memory admission retained for one canonical module kernel.
#[derive(Debug, Eq, PartialEq)]
pub struct ProductionFormalMemoryKernelV1 {
    obligations: FormalMemoryObligations,
    ranked_discharged_reasons: Box<[FormalMemoryIncompleteReason]>,
    compiler_discharged_reasons: Box<[FormalMemoryIncompleteReason]>,
}

impl fmt::Debug for ProductionFormalMemoryOwnerV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionFormalMemoryOwnerV1")
            .field("kernels", &self.kernels)
            .finish_non_exhaustive()
    }
}

impl ProductionFormalMemoryOwnerV1 {
    /// Consumes verified semantic KIR and requires complete, conflict-free
    /// formal extraction for the production witness extent.
    pub fn try_admit(
        semantic_kir: ProductionSemanticKirOwnerV1,
    ) -> Result<Self, ProductionFormalMemoryErrorV1> {
        semantic_kir
            .verify_equivalence()
            .map_err(ProductionFormalMemoryErrorV1::SemanticKir)?;
        let kernels = derive_admitted_obligations(&semantic_kir)?;
        let owner = Self {
            semantic_kir,
            kernels,
        };
        owner.verify_equivalence()?;
        Ok(owner)
    }

    /// Re-verifies exact semantic KIR and deterministically re-derives the
    /// retained formal obligations.
    pub fn verify_equivalence(&self) -> Result<(), ProductionFormalMemoryErrorV1> {
        self.semantic_kir
            .verify_equivalence()
            .map_err(ProductionFormalMemoryErrorV1::SemanticKir)?;
        let kernels = derive_admitted_obligations(&self.semantic_kir)?;
        if kernels != self.kernels {
            return Err(ProductionFormalMemoryErrorV1::ObligationMismatch);
        }
        Ok(())
    }

    /// Borrows the exact semantic-to-Kernel-IR owner.
    pub const fn semantic_kir(&self) -> &ProductionSemanticKirOwnerV1 {
        &self.semantic_kir
    }

    /// Borrows complete compiler-derived obligations for the witness extent.
    pub fn obligations(&self) -> Option<&FormalMemoryObligations> {
        let [kernel] = self.kernels.as_ref() else {
            return None;
        };
        Some(&kernel.obligations)
    }

    /// Borrows the complete canonical per-kernel formal roster.
    pub fn kernels(&self) -> &[ProductionFormalMemoryKernelV1] {
        &self.kernels
    }

    /// Resolves exact formal obligations for one kernel identity.
    pub fn obligations_for_kernel(&self, kernel: &str) -> Option<&FormalMemoryObligations> {
        self.kernels
            .iter()
            .find(|evidence| evidence.obligations.kernel().as_str() == kernel)
            .map(|evidence| &evidence.obligations)
    }

    /// Returns dynamic index derivations discharged by the retained, exact
    /// ranked bounds/race receipt rather than the affine formal engine.
    pub fn ranked_discharged_reasons(&self) -> Option<&[FormalMemoryIncompleteReason]> {
        let [kernel] = self.kernels.as_ref() else {
            return None;
        };
        Some(&kernel.ranked_discharged_reasons)
    }

    /// Returns internal LDS effects discharged only by replaying the exact,
    /// compiler-owned collective lowering and its source correspondence.
    pub fn compiler_discharged_reasons(&self) -> Option<&[FormalMemoryIncompleteReason]> {
        let [kernel] = self.kernels.as_ref() else {
            return None;
        };
        Some(&kernel.compiler_discharged_reasons)
    }

    /// Returns the structural fallback extent used for every dynamic axis.
    pub const fn witness_extent(&self) -> u64 {
        PRODUCTION_FORMAL_MEMORY_WITNESS_EXTENT_V1
    }

    /// Returns exact per-axis extents of the admitted structural witness.
    pub fn witness_extents(&self) -> Option<[u64; 3]> {
        let [kernel] = self.semantic_kir.module().kernels.as_slice() else {
            return None;
        };
        Some(witness_extents(&kernel.domain))
    }

    /// Returns the exact flattened invocation count in the structural witness.
    pub fn witness_invocation_count(&self) -> Option<u64> {
        self.obligations().map(|obligations| {
            obligations.invocations().map_or(0, |invocations| {
                invocations.end_exclusive() - invocations.start()
            })
        })
    }

    /// Formal admission alone never grants artifact or launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

impl ProductionFormalMemoryKernelV1 {
    /// Borrows this kernel's complete formal obligations.
    pub const fn obligations(&self) -> &FormalMemoryObligations {
        &self.obligations
    }

    /// Returns dynamic-index reasons discharged by this kernel's ranked proof.
    pub fn ranked_discharged_reasons(&self) -> &[FormalMemoryIncompleteReason] {
        &self.ranked_discharged_reasons
    }

    /// Returns compiler-owned LDS reasons discharged for this kernel.
    pub fn compiler_discharged_reasons(&self) -> &[FormalMemoryIncompleteReason] {
        &self.compiler_discharged_reasons
    }

    /// Returns exact structural witness extents for this kernel.
    pub fn witness_extents(&self, module: &fe2o3_kernel_ir::Module) -> Option<[u64; 3]> {
        module
            .kernels
            .iter()
            .find(|kernel| kernel.id == *self.obligations.kernel())
            .map(|kernel| witness_extents(&kernel.domain))
    }

    /// Returns the flattened structural witness invocation count.
    pub fn witness_invocation_count(&self) -> u64 {
        self.obligations.invocations().map_or(0, |invocations| {
            invocations.end_exclusive() - invocations.start()
        })
    }
}

/// Fresh complete-only memory analysis borrowing the actual checked output.
///
/// This is not final source/ranked admission. The fixed structural witness does
/// not authenticate a runtime launch, Private memory is outside the formal
/// engine's modeled accesses, and a memory-complete graph may still trap. No
/// historical ranked or compiler discharge is imported into these results.
///
/// The existing formal engine's work, scratch, and obligation payload are not
/// charged to the canonical optimizer ledger or covered by its storage receipt.
/// The report borrows the checked owner; it does not copy an executable graph.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{
///     CheckedOutputFormalMemoryAnalysisV1, analyze_checked_output_formal_memory_v1,
/// };
/// use fe2o3_pliron::CheckedNeutralKernelIrOwnerV1;
/// fn escape(owner: CheckedNeutralKernelIrOwnerV1) -> CheckedOutputFormalMemoryAnalysisV1<'static> {
///     analyze_checked_output_formal_memory_v1(&owner).unwrap()
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{
///     CheckedOutputFormalMemoryAnalysisV1, ProductionFormalMemoryOwnerV1,
/// };
/// fn admit(report: CheckedOutputFormalMemoryAnalysisV1<'_>) -> ProductionFormalMemoryOwnerV1 {
///     report
/// }
/// ```
pub struct CheckedOutputFormalMemoryAnalysisV1<'o> {
    checked: &'o fe2o3_pliron::CheckedNeutralKernelIrOwnerV1,
    kernels: Box<[FormalMemoryObligations]>,
}

impl CheckedOutputFormalMemoryAnalysisV1<'_> {
    /// The actual checked output borrowed by this analysis.
    pub fn output(&self) -> &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12 {
        self.checked.owner()
    }

    /// Fresh complete obligations in the output module's kernel order.
    pub fn kernels(&self) -> &[FormalMemoryObligations] {
        &self.kernels
    }

    /// Releases the already derived rows and ends this report's output borrow.
    /// These owned diagnostics do not retain or identify an executable owner,
    /// authenticate a source/ranked attachment, or grant formal admission. A
    /// private consuming stage must retain the same checked owner separately;
    /// this method neither copies a graph nor reinterprets an N-only owner.
    /// The existing formal engine's separate accounting policy is unchanged.
    ///
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::{CheckedOutputFormalMemoryAnalysisV1, ProductionFormalMemoryOwnerV1};
    /// fn admit(report: CheckedOutputFormalMemoryAnalysisV1<'_>) -> ProductionFormalMemoryOwnerV1 {
    ///     report.into_kernel_obligations()
    /// }
    /// ```
    pub fn into_kernel_obligations(self) -> Box<[FormalMemoryObligations]> {
        self.kernels
    }
}

/// Analyze every actual output kernel using the existing fixed witness policy.
/// Any incomplete extraction or inter-invocation conflict rejects; this path
/// neither consults nor constructs a final source/ranked admission owner.
pub fn analyze_checked_output_formal_memory_v1(
    checked: &fe2o3_pliron::CheckedNeutralKernelIrOwnerV1,
) -> Result<CheckedOutputFormalMemoryAnalysisV1<'_>, ProductionFormalMemoryErrorV1> {
    let kernels = derive_checked_output_formal_obligation_rows_v1(checked)?;
    Ok(CheckedOutputFormalMemoryAnalysisV1 { checked, kernels })
}

fn derive_checked_output_formal_obligation_rows_v1(
    checked: &fe2o3_pliron::CheckedNeutralKernelIrOwnerV1,
) -> Result<Box<[FormalMemoryObligations]>, ProductionFormalMemoryErrorV1> {
    let module = checked.owner().module();
    if module.kernels.is_empty() {
        return Err(ProductionFormalMemoryErrorV1::KernelCount { actual: 0 });
    }
    let mut kernels = Vec::with_capacity(module.kernels.len());
    for (index, _) in module.kernels.iter().enumerate() {
        let obligations = derive_complete_output_formal_kernel_v1(checked.owner(), index)?
            .ok_or(ProductionFormalMemoryErrorV1::ObligationMismatch)?;
        kernels.push(obligations);
    }
    Ok(kernels.into_boxed_slice())
}

/// Fresh inert data for one actual output kernel; an absent index returns None.
/// The kernel and witness domain are borrowed from the same owner in O(1).
/// This neither binds a source/ranked result nor grants formal admission.
/// Formal work, scratch and obligation storage retain the engine's existing
/// separate resource domain; no canonical-ledger coverage is implied.
pub(crate) fn derive_complete_output_formal_kernel_v1(
    output: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    kernel_index: usize,
) -> Result<Option<FormalMemoryObligations>, ProductionFormalMemoryErrorV1> {
    let module = output.module();
    let Some(kernel) = module.kernels.get(kernel_index) else {
        return Ok(None);
    };
    let analysis = derive_kernel_memory_obligations_for_launch(
        module,
        &kernel.id,
        ExplicitLaunchExtent::Exact {
            rank: kernel.domain.rank(),
            extents: witness_extents(&kernel.domain),
        },
        FormalIndexWidth::Bits64,
    )
    .map_err(ProductionFormalMemoryErrorV1::Analysis)?;
    let obligations = match analysis {
        FormalMemoryObligationAnalysis::Complete(obligations) => obligations,
        FormalMemoryObligationAnalysis::Incomplete { reasons, .. } => {
            return Err(ProductionFormalMemoryErrorV1::Incomplete {
                reasons: reasons.into_boxed_slice(),
            });
        }
    };
    if !obligations.inter_invocation_conflicts().is_empty() {
        return Err(ProductionFormalMemoryErrorV1::InterInvocationConflicts {
            conflicts: obligations
                .inter_invocation_conflicts()
                .to_vec()
                .into_boxed_slice(),
        });
    }
    Ok(Some(obligations))
}

fn derive_admitted_obligations(
    semantic_kir: &ProductionSemanticKirOwnerV1,
) -> Result<Box<[ProductionFormalMemoryKernelV1]>, ProductionFormalMemoryErrorV1> {
    let module = semantic_kir.module();
    if module.kernels.is_empty() {
        return Err(ProductionFormalMemoryErrorV1::KernelCount {
            actual: module.kernels.len(),
        });
    }
    let mut admitted = Vec::with_capacity(module.kernels.len());
    for kernel in &module.kernels {
        admitted.push(derive_admitted_obligations_for_kernel(
            semantic_kir,
            kernel,
        )?);
    }
    Ok(admitted.into_boxed_slice())
}

fn derive_admitted_obligations_for_kernel(
    semantic_kir: &ProductionSemanticKirOwnerV1,
    kernel: &fe2o3_kernel_ir::Kernel,
) -> Result<ProductionFormalMemoryKernelV1, ProductionFormalMemoryErrorV1> {
    let module = semantic_kir.module();
    let domain = &kernel.domain;
    let rank = domain.rank();
    let witness = ExplicitLaunchExtent::Exact {
        rank,
        extents: witness_extents(domain),
    };
    let analysis = derive_kernel_memory_obligations_for_launch(
        module,
        &kernel.id,
        witness,
        FormalIndexWidth::Bits64,
    )
    .map_err(ProductionFormalMemoryErrorV1::Analysis)?;
    let (obligations, ranked_discharged_reasons, compiler_discharged_reasons) = match analysis {
        FormalMemoryObligationAnalysis::Complete(obligations) => (
            obligations,
            Vec::new().into_boxed_slice(),
            Vec::new().into_boxed_slice(),
        ),
        FormalMemoryObligationAnalysis::Incomplete { partial, reasons } => {
            let mut compiler_reasons = Vec::new();
            let mut remaining_reasons = Vec::new();
            for reason in reasons {
                if matches!(
                    reason,
                    FormalMemoryIncompleteReason::UnsupportedMemoryEffect { .. }
                        | FormalMemoryIncompleteReason::UnsupportedPointerDerivation { .. }
                ) {
                    compiler_reasons.push(reason);
                } else {
                    remaining_reasons.push(reason);
                }
            }
            if !compiler_reasons.is_empty()
                && let Err(detail) = semantic_kir
                    .retained_collective_lowering_discharges_workgroup_memory(
                        kernel.id.as_str(),
                        &compiler_reasons,
                    )
            {
                return Err(
                    ProductionFormalMemoryErrorV1::CompilerOwnedWorkgroupDischarge {
                        reasons: compiler_reasons.into_boxed_slice(),
                        detail,
                    },
                );
            }
            let mut guarded_reasons = Vec::new();
            let mut guarded_locations = Vec::new();
            let mut unsupported_indices = Vec::new();
            let mut reasons_are_ranked_dischargeable = true;
            for reason in &remaining_reasons {
                match reason {
                    FormalMemoryIncompleteReason::GuardedAccessRequiresRankedProof { location } => {
                        guarded_reasons.push(reason.clone());
                        guarded_locations.push(*location);
                    }
                    FormalMemoryIncompleteReason::UnsupportedIndexExpression { .. } => {
                        unsupported_indices.push(reason.clone());
                    }
                    _ => reasons_are_ranked_dischargeable = false,
                }
            }
            if !reasons_are_ranked_dischargeable {
                return Err(ProductionFormalMemoryErrorV1::Incomplete {
                    reasons: remaining_reasons.into_boxed_slice(),
                });
            }
            if !unsupported_indices.is_empty()
                && let Err(detail) = semantic_kir
                    .retained_generic_checks_discharge_unsupported_indices(
                        kernel.id.as_str(),
                        &unsupported_indices,
                    )
            {
                return Err(ProductionFormalMemoryErrorV1::UnsupportedIndexDischarge {
                    reasons: unsupported_indices.into_boxed_slice(),
                    detail,
                });
            }
            if !guarded_locations.is_empty()
                && let Err(detail) = semantic_kir
                    .retained_generic_checks_discharge_guarded_accesses(
                        kernel.id.as_str(),
                        &guarded_locations,
                    )
            {
                return Err(ProductionFormalMemoryErrorV1::GuardedAccessDischarge {
                    reasons: guarded_reasons.into_boxed_slice(),
                    detail,
                });
            }
            (
                partial,
                remaining_reasons.into_boxed_slice(),
                compiler_reasons.into_boxed_slice(),
            )
        }
    };
    if !obligations.inter_invocation_conflicts().is_empty() {
        return Err(ProductionFormalMemoryErrorV1::InterInvocationConflicts {
            conflicts: obligations
                .inter_invocation_conflicts()
                .to_vec()
                .into_boxed_slice(),
        });
    }
    Ok(ProductionFormalMemoryKernelV1 {
        obligations,
        ranked_discharged_reasons,
        compiler_discharged_reasons,
    })
}

fn witness_extents(domain: &LaunchDomain) -> [u64; 3] {
    let mut witness = [1_u64; 3];
    for (axis, extent) in domain.extents().enumerate() {
        witness[axis] = match extent {
            LaunchExtent::Static(extent) => u64::from(extent),
            LaunchExtent::Dynamic => PRODUCTION_FORMAL_MEMORY_WITNESS_EXTENT_V1,
        };
    }
    witness
}

#[cfg(test)]
mod tests {
    use fe2o3_kernel_ir::{
        AccessMode, AddressSpace, BasicBlock, BlockId,
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work, Constant, Function, FunctionId,
        FunctionOperationLocation, Kernel, MemoryAccess, Module, Operation, OperationKind,
        Signature, Terminator, Type, ValueDef, ValueId,
        VerifiedCanonicalKernelIrModuleV12 as Output,
    };
    use fe2o3_pliron::{CheckedNeutralKernelIrOwnerV1, KirPlironGraphV12};

    use super::*;

    fn guarded_reason(operation_index: usize) -> FormalMemoryIncompleteReason {
        FormalMemoryIncompleteReason::GuardedAccessRequiresRankedProof {
            location: FunctionOperationLocation::new(BlockId(2), operation_index),
        }
    }

    #[test]
    fn discharge_diagnostics_name_exact_access_bound_and_bounded_reason_set() {
        let reasons = (0..17).map(guarded_reason).collect::<Vec<_>>();
        let error = ProductionFormalMemoryErrorV1::GuardedAccessDischarge {
            reasons: reasons.into_boxed_slice(),
            detail: ProductionMemoryDischargeFailureV1::GuardedBound {
                location: FunctionOperationLocation::new(BlockId(2), 3),
                index: ValueId(41),
                slice: ValueId(7),
                detail: "guard predicate does not prove the selected index is in bounds",
            },
        };

        let diagnostic = error.to_string();
        assert!(diagnostic.contains("17 guarded access reason(s)"));
        assert!(diagnostic.contains("operation_index: 3"));
        assert!(diagnostic.contains("index ValueId(41)"));
        assert!(diagnostic.contains("slice ValueId(7)"));
        assert!(diagnostic.contains("(1 more)"));
    }

    #[test]
    fn unsupported_index_diagnostic_names_the_exact_consumer() {
        let location = FunctionOperationLocation::new(BlockId(5), 8);
        let error = ProductionFormalMemoryErrorV1::UnsupportedIndexDischarge {
            reasons: vec![guarded_reason(0)].into_boxed_slice(),
            detail: ProductionMemoryDischargeFailureV1::Access {
                location,
                detail: "semantic memory access site has no ranked access receipt",
            },
        };

        let diagnostic = error.to_string();
        assert!(diagnostic.contains("unsupported index reason(s)"));
        assert!(diagnostic.contains("semantic memory access site has no ranked access receipt"));
        assert!(diagnostic.contains("block: BlockId(5)"));
        assert!(diagnostic.contains("operation_index: 8"));
    }

    // Canonical KIR components, not admitted source/Store/control capabilities.
    fn formal_component(parameters: Vec<Type>, operations: Vec<Operation>) -> Module {
        let values = (0..parameters.len())
            .map(|index| ValueId(u32::try_from(index).unwrap()))
            .collect();
        let mut block = BasicBlock::new(BlockId(0));
        block.operations = operations;
        block.terminator = Some(Terminator::Return { values: vec![] });
        let mut module = Module::new("formal-helper-component");
        module.functions.push(Function::kernel_entry(
            "entry",
            Signature::new(parameters, vec![]),
            values,
            vec![block],
        ));
        module.kernels.push(Kernel::new(
            "kernel",
            "entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        ));
        module
    }

    fn global_pointer(writable: bool) -> Type {
        Type::pointer(
            Type::F32,
            AddressSpace::Global,
            if writable {
                AccessMode::ReadWrite
            } else {
                AccessMode::ReadOnly
            },
        )
    }

    fn with_formal_component(module: &Module, test: impl FnOnce(&Output, &mut Budget<'_>)) {
        let mut work = Work::new(1_000_000_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000_000);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(29).unwrap();
        let (output, storage) =
            Output::from_module_ref_with_verification_budget_v12(module, &mut budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let floor = budget.storage();
        let before = budget.work();
        test(&output, &mut budget);
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.work(), before);
        assert_eq!(budget.failed_storage(), None);
        drop(output);
        budget.release_storage(storage.retained_storage()).unwrap();
        assert_eq!(budget.storage(), 29);
        assert_eq!(work.failed_work(), None);
    }

    #[test]
    fn complete_helper_retains_exact_runtime_obligations_and_rejects_absent_indices() {
        let mut module = formal_component(
            vec![global_pointer(true), global_pointer(false), Type::F32],
            vec![
                Operation::effect_free(
                    ValueDef::new(ValueId(3), Type::F32),
                    OperationKind::Load {
                        pointer: ValueId(1),
                        access: MemoryAccess::new(AddressSpace::Global, 4),
                    },
                ),
                Operation::new(
                    vec![],
                    OperationKind::Store {
                        pointer: ValueId(0),
                        value: ValueId(2),
                        access: MemoryAccess::new(AddressSpace::Global, 4),
                    },
                ),
            ],
        );
        module.kernels[0].domain = LaunchDomain::D1 {
            x: LaunchExtent::Static(1),
        };
        with_formal_component(&module, |output, _| {
            let actual = derive_complete_output_formal_kernel_v1(output, 0)
                .unwrap()
                .unwrap();
            let fresh = derive_kernel_memory_obligations_for_launch(
                output.module(),
                &output.module().kernels[0].id,
                ExplicitLaunchExtent::Exact {
                    rank: 1,
                    extents: [1, 1, 1],
                },
                FormalIndexWidth::Bits64,
            )
            .unwrap();
            assert!(matches!(fresh, FormalMemoryObligationAnalysis::Complete(_)));
            assert_eq!(&actual, fresh.obligations());
            assert_eq!(actual.accesses().len(), 2);
            assert_eq!(actual.bounds_requirements().len(), 2);
            assert_eq!(actual.runtime_alias_requirements().len(), 1);
            assert!(actual.inter_invocation_conflicts().is_empty());
            for absent in [1, usize::MAX] {
                assert!(
                    derive_complete_output_formal_kernel_v1(output, absent)
                        .unwrap()
                        .is_none()
                );
            }
        });
        with_formal_component(&Module::new("empty-formal-component"), |output, _| {
            assert!(
                derive_complete_output_formal_kernel_v1(output, 0)
                    .unwrap()
                    .is_none()
            );
        });
    }

    fn conflict_component(external: bool) -> Module {
        let mut module = formal_component(
            vec![global_pointer(true), Type::F32],
            vec![Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(0),
                    value: ValueId(1),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            )],
        );
        if external {
            module.functions[0].body.as_mut().unwrap().blocks[0]
                .operations
                .push(Operation::new(
                    vec![],
                    OperationKind::Call {
                        callee: FunctionId::new("external"),
                        arguments: vec![],
                    },
                ));
            module.functions.push(Function::declaration(
                "external",
                Signature::new(vec![], vec![]),
            ));
        }
        module
    }

    #[test]
    fn complete_helper_preserves_incomplete_before_partial_conflict_error_order() {
        for external in [false, true] {
            with_formal_component(&conflict_component(external), |output, _| {
                let fresh = derive_kernel_memory_obligations_for_launch(
                    output.module(),
                    &output.module().kernels[0].id,
                    ExplicitLaunchExtent::Exact {
                        rank: 1,
                        extents: [2, 1, 1],
                    },
                    FormalIndexWidth::Bits64,
                )
                .unwrap();
                assert_eq!(fresh.obligations().inter_invocation_conflicts().len(), 1);
                let error = derive_complete_output_formal_kernel_v1(output, 0).unwrap_err();
                if external {
                    let FormalMemoryObligationAnalysis::Incomplete {
                        reasons: expected, ..
                    } = fresh
                    else {
                        panic!("external memory effects must remain incomplete")
                    };
                    let ProductionFormalMemoryErrorV1::Incomplete { reasons } = error else {
                        panic!("incompleteness must precede the partial conflict")
                    };
                    assert_eq!(reasons.as_ref(), expected.as_slice());
                    assert!(
                        matches!(reasons.as_ref(), [FormalMemoryIncompleteReason::CallEffectsUnavailable { location, callee }]
                        if location.operation_index == 1 && callee.as_str() == "external")
                    );
                } else {
                    assert!(matches!(fresh, FormalMemoryObligationAnalysis::Complete(_)));
                    let ProductionFormalMemoryErrorV1::InterInvocationConflicts { conflicts } =
                        error
                    else {
                        panic!("complete extraction must still reject inherent conflicts")
                    };
                    assert_eq!(
                        conflicts.as_ref(),
                        fresh.obligations().inter_invocation_conflicts()
                    );
                }
            });
        }
    }

    #[test]
    fn complete_helper_does_not_discharge_an_unsupported_physical_index() {
        let pointer = global_pointer(false);
        let module = formal_component(
            vec![pointer.clone(), Type::INDEX],
            vec![
                Operation::effect_free(
                    ValueDef::new(ValueId(2), pointer),
                    OperationKind::GetElementPointer {
                        base: ValueId(0),
                        offset: ValueId(1),
                    },
                ),
                Operation::effect_free(
                    ValueDef::new(ValueId(3), Type::F32),
                    OperationKind::Load {
                        pointer: ValueId(2),
                        access: MemoryAccess::new(AddressSpace::Global, 4),
                    },
                ),
            ],
        );
        with_formal_component(&module, |output, _| {
            let ProductionFormalMemoryErrorV1::Incomplete { reasons } =
                derive_complete_output_formal_kernel_v1(output, 0).unwrap_err()
            else {
                panic!("an unknown actual offset has no discharge")
            };
            assert!(
                matches!(reasons.as_ref(), [FormalMemoryIncompleteReason::UnsupportedIndexExpression { location, index, allocation }]
                if location.block == BlockId(0) && location.operation_index == 0
                    && *index == ValueId(1) && allocation.parameter_index() == 0)
            );
        });
    }

    fn ordered_read_roots() -> Module {
        let mut module = Module::new("ordered-formal-roots");
        for (kernel, entry, domain) in [
            (
                "z_root",
                "z_entry",
                LaunchDomain::D1 {
                    x: LaunchExtent::Dynamic,
                },
            ),
            (
                "a_root",
                "a_entry",
                LaunchDomain::D2 {
                    x: LaunchExtent::Static(3),
                    y: LaunchExtent::Dynamic,
                },
            ),
            (
                "m_root",
                "m_entry",
                LaunchDomain::D3 {
                    x: LaunchExtent::Dynamic,
                    y: LaunchExtent::Static(3),
                    z: LaunchExtent::Static(5),
                },
            ),
        ] {
            let mut block = BasicBlock::new(BlockId(40));
            block.operations = vec![
                Operation::effect_free(
                    ValueDef::new(ValueId(17), Type::F32),
                    OperationKind::Constant(Constant::F32Bits(0)),
                ),
                Operation::effect_free(
                    ValueDef::new(ValueId(18), Type::F32),
                    OperationKind::Load {
                        pointer: ValueId(0),
                        access: MemoryAccess::new(AddressSpace::Global, 4),
                    },
                ),
            ];
            block.terminator = Some(Terminator::Return { values: vec![] });
            module.functions.push(Function::kernel_entry(
                entry,
                Signature::new(vec![global_pointer(false)], vec![]),
                vec![ValueId(0)],
                vec![block],
            ));
            module.kernels.push(Kernel::new(kernel, entry, domain));
        }
        module
    }

    fn with_checked_formal_component(
        module: &Module,
        test: impl FnOnce(&CheckedNeutralKernelIrOwnerV1),
    ) {
        let mut work = Work::new(1_000_000_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000_000);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(29).unwrap();
        let (input, input_storage) =
            Output::from_module_ref_with_verification_budget_v12(module, &mut budget).unwrap();
        budget
            .reserve_storage(input_storage.retained_storage())
            .unwrap();
        let (mut graph, graph_storage) = KirPlironGraphV12::import(&input, &mut budget).unwrap();
        budget
            .reserve_storage(graph_storage.retained_storage())
            .unwrap();
        let observed = graph
            .execute_production_neutral_optimization_v1(&mut budget)
            .unwrap()
            .extract()
            .unwrap();
        budget
            .reserve_storage(observed.storage().retained_storage())
            .unwrap();
        let graph_storage = graph.retained_storage();
        drop(graph);
        budget.release_storage(graph_storage).unwrap();
        let checked = observed.try_check_and_finish_v1(&mut budget).unwrap();
        budget
            .reserve_storage(checked.storage().retained_storage())
            .unwrap();
        let floor = budget.storage();
        let before = budget.work();
        test(&checked);
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.work(), before);
        assert_eq!(budget.failed_storage(), None);
        let retained = checked.storage().retained_storage();
        drop(checked);
        budget.release_storage(retained).unwrap();
        drop(input);
        budget
            .release_storage(input_storage.retained_storage())
            .unwrap();
        assert_eq!(budget.storage(), 29);
        assert_eq!(work.failed_work(), None);
    }

    #[test]
    fn legacy_collector_keeps_actual_changed_output_order_and_per_axis_witness() {
        let source = ordered_read_roots();
        with_checked_formal_component(&source, |checked| {
            let output = checked.owner();
            assert_ne!(
                checked.native_input_audit_bytes(),
                output.canonical().canonical_bytes()
            );
            let rows = derive_checked_output_formal_obligation_rows_v1(checked).unwrap();
            assert_eq!(rows.len(), 3);
            for (index, (name, rank, extents, count)) in [
                ("z_root", 1, [2, 1, 1], 2),
                ("a_root", 2, [3, 2, 1], 6),
                ("m_root", 3, [2, 3, 5], 30),
            ]
            .into_iter()
            .enumerate()
            {
                assert_eq!(rows[index].kernel().as_str(), name);
                assert_eq!(rows[index].accesses().len(), 1);
                let invocations = rows[index].invocations().unwrap();
                assert_eq!(invocations.end_exclusive() - invocations.start(), count);
                let direct = derive_kernel_memory_obligations_for_launch(
                    output.module(),
                    &output.module().kernels[index].id,
                    ExplicitLaunchExtent::Exact { rank, extents },
                    FormalIndexWidth::Bits64,
                )
                .unwrap();
                assert!(matches!(
                    direct,
                    FormalMemoryObligationAnalysis::Complete(_)
                ));
                assert_eq!(&rows[index], direct.obligations());
                assert_eq!(
                    derive_complete_output_formal_kernel_v1(output, index)
                        .unwrap()
                        .as_ref(),
                    Some(&rows[index])
                );
            }
        });
    }

    #[test]
    fn legacy_collector_preserves_empty_roster_and_first_kernel_failure() {
        with_checked_formal_component(&Module::new("empty-formal-roster"), |checked| {
            assert!(matches!(
                derive_checked_output_formal_obligation_rows_v1(checked),
                Err(ProductionFormalMemoryErrorV1::KernelCount { actual: 0 })
            ));
        });
        let mut source = conflict_component(true);
        let mut conflict = source.functions[0].clone();
        conflict.id = FunctionId::new("conflict_entry");
        assert!(matches!(
            conflict.body.as_mut().unwrap().blocks[0]
                .operations
                .pop()
                .unwrap()
                .kind,
            OperationKind::Call { .. }
        ));
        source.functions.push(conflict);
        source.kernels.push(Kernel::new(
            "conflict_kernel",
            "conflict_entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        ));
        for conflict_first in [false, true] {
            if conflict_first {
                source.kernels.swap(0, 1);
            }
            with_checked_formal_component(&source, |checked| {
                let error = derive_checked_output_formal_obligation_rows_v1(checked).unwrap_err();
                if conflict_first {
                    assert_eq!(
                        checked.owner().module().kernels[0].id.as_str(),
                        "conflict_kernel"
                    );
                    assert!(matches!(
                        error,
                        ProductionFormalMemoryErrorV1::InterInvocationConflicts { .. }
                    ));
                } else {
                    assert_eq!(checked.owner().module().kernels[0].id.as_str(), "kernel");
                    assert!(matches!(
                        error,
                        ProductionFormalMemoryErrorV1::Incomplete { .. }
                    ));
                }
            });
        }
    }
}
