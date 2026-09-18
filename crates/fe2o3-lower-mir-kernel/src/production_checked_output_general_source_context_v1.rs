// This closed context selects independently replayed source subjects; it does
// not construct a SemanticKir owner for erased E or change public direct APIs.
#[derive(Clone, Copy)]
enum GeneralSourceContextV1<'a> {
    Direct(&'a ProductionSemanticKirOwnerV1),
    Erased(&'a ProductionUnitLocalErasedSourceOwnerV1),
}

impl<'a> GeneralSourceContextV1<'a> {
    fn semantic(self) -> &'a AdmittedInertSemanticMirV1 {
        match self {
            Self::Direct(source) => source.semantic().semantic(),
            Self::Erased(source) => source.original_source().semantic_ssa.source_semantic(),
        }
    }

    fn limits(self) -> ProductionSemanticKirLimitsV1 {
        match self {
            Self::Direct(source) => source.limits,
            Self::Erased(source) => source.original_source().limits,
        }
    }

    fn neutral(self) -> R<&'a fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12> {
        match self {
            Self::Direct(source) => source
                .pre_ranked_executable()
                .ok_or_else(|| refused("N", "consumed connected source custody")),
            Self::Erased(source) => Ok(source.erased()),
        }
    }

    fn origins(self) -> R<SemanticKirAssertOriginsV1<'a>> {
        match self {
            Self::Direct(source) => source
                .pre_ranked_assert_origins()
                .ok_or_else(|| refused("N", "sealed assertion custody")),
            Self::Erased(source) => Ok(source.original_source().assert_origins()),
        }
    }

    fn replay_and_census(self, budget: &mut AssertOriginBudgetV1<'_>) -> R<()> {
        match self {
            Self::Direct(source) => {
                source.verify_equivalence().map_err(E::Source)?;
                census::source(source, budget)
            }
            Self::Erased(source) => {
                source.verify_equivalence(budget).map_err(E::Source)?;
                // No removed helper is excused from the full original grammar
                // or selected-body/retained-helper ABI census.
                census::source_parts(
                    self.semantic(),
                    &source.original_source().correspondence,
                    budget,
                )
            }
        }
    }

    fn ranked(self, budget: &mut AssertOriginBudgetV1<'_>) -> R<()> {
        let (count, original_kernels) = match self {
            Self::Direct(source) => (
                source.generic_checks.len(),
                self.neutral()?.module().kernels.len(),
            ),
            Self::Erased(source) => (
                source.roots.len(),
                source.original_source().executable().module().kernels.len(),
            ),
        };
        if count == 0 || count != original_kernels {
            return Err(refused("ranked", "complete nonempty root roster"));
        }
        for ordinal in 0..count {
            charge(budget, 1)?;
            let lowering = match self {
                Self::Direct(source) => &source.generic_checks[ordinal].lowering,
                Self::Erased(source) => &source.roots[ordinal].lowering,
            };
            if !lowering.all_mandatory_reports_are_clean() {
                return Err(refused("ranked", "mandatory source/ranked reports"));
            }
            census::ranked(lowering, budget)?;
        }
        Ok(())
    }
}

// The erased occurrence callback requires scratch to be gone on return. This
// nested transaction preserves the already-live E/B, B/C and source indexes.
fn erased_general_scratch_v1<'w, T>(
    budget: &mut AssertOriginBudgetV1<'w>,
    body: impl FnOnce(&mut AssertOriginBudgetV1<'w>) -> R<T>,
) -> R<T> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| body(budget)));
    if ledger != budget.work_ledger_identity_v1() || budget.storage() < floor {
        return Err(E::Resource(AssertOriginResourceV1::Accounting));
    }
    budget
        .release_storage(budget.storage() - floor)
        .map_err(E::Resource)?;
    match result {
        Ok(result) => result,
        Err(_) => Err(E::SourceOutput(ProductionSourceOutputErrorV1::Panicked)),
    }
}

pub(super) fn check_erased_general_output_v1(
    source: &ProductionUnitLocalErasedSourceOwnerV1,
    bound: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    checked: &fe2o3_pliron::CheckedNeutralKernelIrOwnerPolicy3V1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<Box<[FormalMemoryObligations]>> {
    check_general_context_v1(
        GeneralSourceContextV1::Erased(source),
        bound,
        checked,
        budget,
    )
}

pub(super) fn check_erased_forwarded_output_v1(
    source: &ProductionUnitLocalErasedSourceOwnerV1,
    qualified_intermediate: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    output: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<Box<[FormalMemoryObligations]>> {
    check_forwarded_context_v1(
        GeneralSourceContextV1::Erased(source),
        qualified_intermediate,
        output,
        budget,
    )
}

