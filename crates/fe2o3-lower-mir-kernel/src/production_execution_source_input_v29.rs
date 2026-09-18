//! Complete detached source census for the scoped materializer.
//! Matching these rows does not authenticate their originating Rust producer.

use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::ProductionSemanticSsaOwnerV1;

use crate::{
    ProductionCheckedContextRootV29, ProductionContextRootErrorV29 as Error,
    ProductionContextRootInputV29, ProductionSourceLaunchRosterV1, with_checked_context_root_v29,
};

/// Untrusted classification of one entry in the complete source callable table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionScopeCallableCandidateV29 {
    /// No provider or workgroup-derivation classification is claimed.
    Ordinary,
    /// A source helper whose normal return closes its derived workgroup scope.
    /// Only the backend's retained receipt authenticates this classification.
    Provider {
        /// Exact source function, also its defined-callable table index.
        function: SemanticFunctionIdV1,
        /// Retained source function identity.
        identity: SemanticFunctionIdentityV1,
    },
    /// A workgroup-derivation intrinsic with exact retained binding and types.
    Derive {
        /// Source binding identity.
        binding: SemanticFunctionIdentityV1,
        /// Intrinsic operation identity.
        operation: SemanticCompilerIntrinsicIdentityV1,
        /// Nominal input context type.
        context: SemanticTypeIdV1,
        /// Nominal output workgroup type.
        workgroup: SemanticTypeIdV1,
    },
}

/// Source call classification, not an expansion or lifecycle proof.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionScopeCallKindV29 {
    /// An ordinary call inside a provider.
    Ordinary,
    /// A call to a classified provider.
    Provider,
    /// Workgroup derivation inside a provider.
    Derive,
}

/// One retained source event; exceptional exits are not normal scope closure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionScopeEventKindV29 {
    /// A direct call, retaining its exact callable and classification.
    Call {
        /// Exact source callable table index.
        callee: SemanticCallableIdV1,
        /// Claimed source call classification.
        kind: ProductionScopeCallKindV29,
    },
    /// A provider's normal return.
    Return,
    /// A provider's assertion, including its exceptional path.
    Assert,
    /// An unreachable terminator in a provider.
    Unreachable,
    /// A provider's unwind-resume terminator.
    UnwindResume,
    /// A provider's unwind-termination terminator.
    UnwindTerminate,
    /// A provider's abort terminator.
    Abort,
}

/// Detached event coordinates in canonical function/block order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionScopeEventCandidateV29 {
    /// Source function containing the event, including unreachable blocks.
    pub function: SemanticFunctionIdV1,
    /// Source basic block containing the terminator.
    pub block: SemanticBlockIdV1,
    /// Exact number of statements before the terminator.
    pub statement_count: usize,
    /// Retained event classification.
    pub kind: ProductionScopeEventKindV29,
}

/// Complete candidate projection of the backend's separately retained receipt.
///
/// All slices are borrowed, including helper operands inside root rows. A
/// matching census supplies neither producer authentication nor executable
/// authority. The backend must keep its move-only originating receipt beside
/// the one source owner until source-to-output correspondence is established.
#[derive(Clone, Copy, Debug)]
pub struct ProductionExecutionSourceInputV29<'a> {
    /// Exact source identity retained by the producer.
    pub semantic_sha256: &'a [u8; 32],
    /// Context-enabled subset of physical roots, strictly ordered by root ID.
    pub roots: &'a [ProductionContextRootInputV29<'a>],
    /// One classification for every source callable, including unused entries.
    pub classes: &'a [ProductionScopeCallableCandidateV29],
    /// Complete source event census, including unreachable blocks.
    pub events: &'a [ProductionScopeEventCandidateV29],
}

fn check_class(
    class: ProductionScopeCallableCandidateV29,
    index: usize,
    semantic: &AdmittedInertSemanticMirV1,
) -> Result<(), Error> {
    use ProductionScopeCallableCandidateV29 as Class;
    let actual = &semantic.callables()[index];
    match class {
        Class::Ordinary => {
            if matches!(
                actual,
                SemanticCallableDeclV1::CompilerIntrinsic {
                    operation: SemanticCompilerIntrinsicOperationV1::Execution(
                        SemanticExecutionOperationV29::WorkgroupDerive { .. }
                    ),
                    ..
                }
            ) {
                return Err(Error::CallableCensus);
            }
        }
        Class::Provider { function, identity } => {
            let body = semantic
                .functions()
                .get(function.index() as usize)
                .ok_or(Error::CallableCensus)?;
            if function.index() as usize != index
                || body.identity() != identity
                || body.role() != SemanticFunctionRoleV1::InternalHelper
                || body.kernel_entry().is_some()
                || !matches!(actual, SemanticCallableDeclV1::Defined { function: found }
                    if *found == function)
            {
                return Err(Error::CallableCensus);
            }
        }
        Class::Derive {
            binding,
            operation,
            context,
            workgroup,
        } => {
            if !matches!(actual, SemanticCallableDeclV1::CompilerIntrinsic {
                binding: found_binding, operation_identity,
                operation: SemanticCompilerIntrinsicOperationV1::Execution(
                    SemanticExecutionOperationV29::WorkgroupDerive {
                        context: found_context, workgroup: found_workgroup }),
            } if found_binding.identity() == binding && *operation_identity == operation
                && *found_context == context && *found_workgroup == workgroup)
            {
                return Err(Error::CallableCensus);
            }
        }
    }
    Ok(())
}

fn event_kind(
    classes: &[ProductionScopeCallableCandidateV29],
    function: usize,
    terminator: &SemanticTerminatorKindV1,
) -> Result<Option<ProductionScopeEventKindV29>, Error> {
    use ProductionScopeCallKindV29 as Call;
    use ProductionScopeCallableCandidateV29 as Class;
    use ProductionScopeEventKindV29 as Event;
    let provider = matches!(classes.get(function), Some(Class::Provider { .. }));
    let (callee, tail) = match terminator {
        SemanticTerminatorKindV1::Call(call) => (call.callee(), false),
        SemanticTerminatorKindV1::TailCall(call) => (call.callee(), true),
        SemanticTerminatorKindV1::Return if provider => return Ok(Some(Event::Return)),
        SemanticTerminatorKindV1::Assert { .. } if provider => return Ok(Some(Event::Assert)),
        SemanticTerminatorKindV1::Unreachable if provider => return Ok(Some(Event::Unreachable)),
        SemanticTerminatorKindV1::UnwindResume if provider => return Ok(Some(Event::UnwindResume)),
        SemanticTerminatorKindV1::UnwindTerminate if provider => {
            return Ok(Some(Event::UnwindTerminate));
        }
        SemanticTerminatorKindV1::Abort if provider => return Ok(Some(Event::Abort)),
        SemanticTerminatorKindV1::Drop { .. } | SemanticTerminatorKindV1::FalseEdge { .. }
            if provider =>
        {
            return Err(Error::ScopeEventCensus);
        }
        _ => return Ok(None),
    };
    let kind = match classes
        .get(callee.index() as usize)
        .ok_or(Error::CallableCensus)?
    {
        Class::Provider { .. } => Call::Provider,
        Class::Derive { .. } if provider => Call::Derive,
        Class::Derive { .. } => return Err(Error::ScopeEventCensus),
        Class::Ordinary if provider => Call::Ordinary,
        Class::Ordinary => return Ok(None),
    };
    if tail {
        return Err(Error::ScopeEventCensus);
    }
    Ok(Some(Event::Call { callee, kind }))
}

fn check_census(
    ssa: &ProductionSemanticSsaOwnerV1,
    launch: &ProductionSourceLaunchRosterV1,
    input: ProductionExecutionSourceInputV29<'_>,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    budget.charge_work(32)?;
    let semantic = ssa.source_semantic();
    if semantic.wire_version() != SemanticMirWireVersionV1::V29
        || input.semantic_sha256 != ssa.source_semantic_sha256()
    {
        return Err(Error::Source);
    }
    if input.classes.len() != semantic.callables().len() {
        return Err(Error::CallableCensus);
    }
    if launch.semantic_sha256() != input.semantic_sha256
        || launch.roots().len() != semantic.roots().len()
    {
        return Err(Error::Launch);
    }
    // Include ordinary physical roots, not only those with a context callback.
    for (root, launch) in semantic.roots().iter().zip(launch.roots()) {
        budget.charge_work(8)?;
        let body = &semantic.functions()[root.index() as usize];
        if launch.selected_root() != *root
            || launch.semantic_root_identity() != body.identity()
            || body
                .kernel_entry()
                .map(|entry| *entry.kernel_binding_identity().as_bytes())
                != Some(launch.kernel_binding())
        {
            return Err(Error::Launch);
        }
    }
    let mut previous = None;
    for root in input.roots {
        budget.charge_work(5)?;
        if root.semantic_sha256 != input.semantic_sha256
            || previous.is_some_and(|id| id >= root.root)
        {
            return Err(Error::RootCensus);
        }
        previous = Some(root.root);
    }
    for (index, class) in input.classes.iter().copied().enumerate() {
        budget.charge_work(12)?;
        check_class(class, index, semantic)?;
    }
    let mut next_root = 0;
    let mut issued = 0_usize;
    let mut next_event = 0;
    for (function, body) in semantic.functions().iter().enumerate() {
        budget.charge_work(3)?;
        let root = input
            .roots
            .get(next_root)
            .filter(|root| root.root.index() as usize == function);
        if root.is_some() {
            next_root += 1;
        }
        for (block, data) in body.blocks().iter().enumerate() {
            budget.charge_work(16)?;
            if let Some(kind) = event_kind(input.classes, function, data.terminator().kind())? {
                let expected = ProductionScopeEventCandidateV29 {
                    function: SemanticFunctionIdV1::from_index(
                        u32::try_from(function).map_err(|_| Resource::Arithmetic)?,
                    ),
                    block: SemanticBlockIdV1::from_index(
                        u32::try_from(block).map_err(|_| Resource::Arithmetic)?,
                    ),
                    statement_count: data.statements().len(),
                    kind,
                };
                if input.events.get(next_event) != Some(&expected) {
                    return Err(Error::ScopeEventCensus);
                }
                next_event += 1;
            }
            let (callee, tail) = match data.terminator().kind() {
                SemanticTerminatorKindV1::Call(call) => (call.callee(), false),
                SemanticTerminatorKindV1::TailCall(call) => (call.callee(), true),
                _ => continue,
            };
            if matches!(
                semantic.callables().get(callee.index() as usize),
                Some(SemanticCallableDeclV1::CompilerIntrinsic {
                    operation: SemanticCompilerIntrinsicOperationV1::Execution(
                        SemanticExecutionOperationV29::ContextIssue { .. }
                    ),
                    ..
                })
            ) {
                if tail
                    || root.is_none_or(|root| {
                        root.issuance.block.index() as usize != block || root.issuer != callee
                    })
                {
                    return Err(Error::RootCensus);
                }
                issued = issued.checked_add(1).ok_or(Resource::Arithmetic)?;
            }
        }
    }
    if next_root != input.roots.len() || issued != input.roots.len() {
        return Err(Error::RootCensus);
    }
    if next_event != input.events.len() {
        return Err(Error::ScopeEventCensus);
    }
    Ok(())
}

/// Checks the complete source census before visiting context-enabled roots.
///
/// This allocates nothing. All source blocks, including unreachable ones, enter
/// the census; only later instance expansion selects reachable emission sites.
/// Ordinary roots remain in the checked launch roster. The visitor shares the
/// cumulative ledger and must discard a partial attempt on any error. Neither
/// source consistency nor a provider label grants lifecycle or launch authority.
pub fn with_checked_execution_source_v29(
    ssa: &ProductionSemanticSsaOwnerV1,
    launch: &ProductionSourceLaunchRosterV1,
    input: ProductionExecutionSourceInputV29<'_>,
    budget: &mut Budget<'_>,
    mut use_root: impl for<'a> FnMut(
        ProductionCheckedContextRootV29<'a>,
        &mut Budget<'_>,
    ) -> Result<(), Error>,
) -> Result<(), Error> {
    let ledger = budget.work_ledger_identity_v1();
    check_census(ssa, launch, input, budget)?;
    for root in input.roots {
        budget.charge_work(1)?;
        with_checked_context_root_v29(ssa, launch, *root, budget, |_, _| Ok(()))?;
    }
    for root in input.roots {
        budget.charge_work(1)?;
        let result = with_checked_context_root_v29(ssa, launch, *root, budget, &mut use_root);
        if budget.work_ledger_identity_v1() != ledger {
            return Err(Resource::Accounting.into());
        }
        result?;
    }
    Ok(())
}
