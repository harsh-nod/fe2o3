/// Source-call obligations are not collapsed into an empty-effect decision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionCanonicalAssertionCallKindV1 {
    /// Kernel roots are checked as roots, never granted a callable summary.
    Root,
    /// Independently complete empty effects without a determinism claim.
    EmptyOnly,
    /// The actual neutral source closure also classified deterministic scalars.
    DeterministicEmpty,
    /// Real private effects require the retained source/local-frame reader.
    PrivateFrame,
}
/// One original function association and its independently checked call classification.
pub struct ProductionCanonicalAssertionCallableV1 {
    association: usize,
    function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    source: SemanticFunctionIdV1,
    decision: fe2o3_mir_model::SemanticCallableDecisionV1,
    kind: ProductionCanonicalAssertionCallKindV1,
}
impl ProductionCanonicalAssertionCallableV1 {
    /// Original root-qualified function association ordinal.
    pub const fn association(&self) -> usize {
        self.association
    }
    /// Function coordinate in the unchanged canonical graph.
    pub const fn function(&self) -> fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1 {
        self.function
    }
    /// Function identity in the retained semantic source.
    pub const fn source_function(&self) -> SemanticFunctionIdV1 {
        self.source
    }
    /// Source-summary decision, including any retained rejection.
    pub const fn source_decision(&self) -> fe2o3_mir_model::SemanticCallableDecisionV1 {
        self.decision
    }
    /// Graph/source classification; private frames do not grant empty effects.
    pub const fn kind(&self) -> ProductionCanonicalAssertionCallKindV1 {
        self.kind
    }
}

fn callable_rows(
    source: &ProductionCanonicalRankedMetadataV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> R<Vec<ProductionCanonicalAssertionCallableV1>> {
    use ProductionCanonicalAssertionCallKindV1 as Kind;
    use fe2o3_kernel_analysis::{
        CanonicalKirCallEffectDecisionV1 as Effect, CanonicalKirCallEffectsV1,
    };
    use fe2o3_mir_model::{
        SemanticCallableDecisionV1 as Decision, SemanticDefinedCallableSummariesV1,
    };
    budget.reserve_storage(std::mem::size_of::<
        Vec<ProductionCanonicalAssertionCallableV1>,
    >())?;
    let mut result = rows(source.calls.groups.len(), budget)?;
    scoped(budget, |budget| {
        let semantic = source.owner.semantic_ssa.source_semantic();
        let summaries = SemanticDefinedCallableSummariesV1::new_metered(
            semantic.types(),
            semantic.functions(),
            semantic.callables(),
            SemanticAssertionLimitsV1::new(usize::MAX, usize::MAX),
            &mut CallableMeter(budget),
        )
        .map_err(callable_error)?;
        budget.charge_work(3)?;
        if !std::ptr::eq(summaries.types(), semantic.types())
            || !std::ptr::eq(summaries.functions(), semantic.functions())
            || !std::ptr::eq(summaries.callables(), semantic.callables())
        {
            return Err(binding(None, "callable summary source owner"));
        }
        let (effects, receipt) = CanonicalKirCallEffectsV1::derive(source.inventory, budget)
            .map_err(Failure::CallEffects)?;
        budget.reserve_storage(receipt.retained_storage())?;
        for callable in semantic.callables() {
            budget.charge_work(1)?;
            let SemanticCallableDeclV1::Defined { function } = callable else {
                return Err(binding(None, "non-defined source callable"));
            };
            budget.charge_work(source.calls.groups.len())?;
            if !source
                .calls
                .groups
                .iter()
                .any(|group| group.function.source().semantic_function() == *function)
            {
                return Err(binding(None, "callable has no original graph association"));
            }
        }
        for (association, group) in source.calls.groups.iter().enumerate() {
            budget.charge_work(4)?;
            let source_function = group.function.source().semantic_function();
            let decision = summaries
                .decision(source_function)
                .ok_or_else(|| binding(None, "callable source function"))?;
            let function = group.function.canonical.coordinate;
            let effect = effects
                .decision(function, budget)
                .map_err(Failure::CallEffects)?;
            let kind = if group.function.source().role()
                != SemanticKirFunctionRoleV1::InternalHelper
            {
                Kind::Root
            } else if effect == Effect::CompleteEmpty {
                match decision {
                    Decision::ExactEmptyDeterministicScalar => Kind::DeterministicEmpty,
                    Decision::ExactEmptyOnly => Kind::EmptyOnly,
                    Decision::Rejected => {
                        return Err(binding(
                            None,
                            "rejected callable cannot acquire an empty summary",
                        ));
                    }
                }
            } else {
                if effect != Effect::CompleteNonempty {
                    return Err(binding(None, "incomplete helper effect closure"));
                }
                let frame = source
                    .arguments
                    .frames
                    .get(association)
                    .and_then(Option::as_ref)
                    .ok_or_else(|| binding(None, "private helper has no retained source frame"))?;
                budget.charge_work(argument_sum_v1(&[
                    frame.allocations().len(),
                    frame.accesses().len(),
                    3,
                ])?)?;
                if source.owner.helper_source_policy_v1()
                    != ProductionHelperSourcePolicyV1::UnitLocal
                    || !std::ptr::eq(frame.source(), group.function.source())
                    || !std::ptr::eq(frame.function(), group.function.canonical.function)
                    || frame.allocations().is_empty()
                {
                    return Err(binding(None, "private helper source/frame identity"));
                }
                // Rejected stays Rejected. The separate private proof supplies no
                // empty-effect, scalar-return-value or determinism authority.
                Kind::PrivateFrame
            };
            if result.len() == result.capacity() {
                return Err(binding(None, "prepaid callable row capacity"));
            }
            result.push(ProductionCanonicalAssertionCallableV1 {
                association,
                function,
                source: source_function,
                decision,
                kind,
            });
        }
        Ok(())
    })?;
    Ok(result)
}
