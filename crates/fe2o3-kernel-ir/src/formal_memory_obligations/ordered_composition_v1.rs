//! Exact composition-scoped memory interpretation. No generic effect promotion.
use super::*;
use crate::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, VerifiedOrderedProgramCompositionV1,
};

/// Closed analysis-input bounds, not additional executable or source authority.
pub const ORDERED_COMPOSITION_FORMAL_RETAINED_BYTES_V1: usize = 64 * 1024;
const WORK: usize = 131_072;

/// Typed memory-analysis failure, preserving the existing analyzer's result.
#[derive(Debug)]
pub enum OrderedCompositionFormalErrorV1 {
    Resource(Resource),
    Profile(&'static str),
    Formal(FormalMemoryObligationError),
}
impl From<Resource> for OrderedCompositionFormalErrorV1 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for OrderedCompositionFormalErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ordered composition formal memory: {self:?}")
    }
}
impl Error for OrderedCompositionFormalErrorV1 {}

/// Added retained logical report envelope. It is not an allocator/RSS bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderedCompositionFormalStorageV1 {
    retained: usize,
}
impl OrderedCompositionFormalStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
}

/// Extracts the real root memory effects from the one retained executable.
///
/// Only the typed structural composition proves that all reachable helper
/// definitions and authored regions are memory-free. Their compiler ordering
/// remains unchanged. The ordinary root load/store/pointer/guard analysis runs
/// unchanged, including every incomplete reason and unresolved runtime bounds,
/// launch and alias condition. No expression is substituted for a helper call.
///
/// The caller keeps the canonical/structural owner's existing receipts live.
/// This entry prepays a bounded retained report envelope and work before invoking
/// the existing independently bounded analyzer. The analyzer's own auxiliary
/// allocation domain is not represented as aggregate heap/RSS here. On every
/// return the incoming floor is preserved, work/denials remain cumulative; on
/// success reserve the returned receipt while retaining the report.
pub fn derive_ordered_composition_memory_obligations_v1(
    owner: &VerifiedOrderedProgramCompositionV1,
    kernel_id: &KernelId,
    launch: ExplicitLaunchExtent,
    index_width: FormalIndexWidth,
    budget: &mut Budget<'_>,
) -> Result<
    (
        FormalMemoryObligationAnalysis,
        OrderedCompositionFormalStorageV1,
    ),
    OrderedCompositionFormalErrorV1,
> {
    let floor = budget.storage();
    budget.with_prepaid_scope(
        floor,
        1,
        WORK,
        ORDERED_COMPOSITION_FORMAL_RETAINED_BYTES_V1,
        |budget| {
            bounded_input(owner, kernel_id, budget)?;
            let report = derive_kernel_memory_obligations_with_composition_context(
                owner.canonical().verified_module_ref_v1(),
                kernel_id,
                launch,
                index_width,
                None,
                Some(owner),
            )
            .map_err(OrderedCompositionFormalErrorV1::Formal)?;
            report_payload(&report)?;
            Ok((
                report,
                OrderedCompositionFormalStorageV1 {
                    retained: ORDERED_COMPOSITION_FORMAL_RETAINED_BYTES_V1,
                },
            ))
        },
    )
}

/// Exact typed-context activation, inaccessible as a public caller boolean.
pub(super) fn contains(
    owner: &VerifiedOrderedProgramCompositionV1,
    module: &Module,
    kernel_id: &KernelId,
) -> bool {
    std::ptr::eq(module, owner.canonical().module())
        && module.kernels.as_slice().first().is_some_and(|kernel| {
            module.kernels.len() == 1
                && &kernel.id == kernel_id
                && module
                    .functions
                    .get(owner.root_function_ordinal() as usize)
                    .is_some_and(|function| function.id == kernel.entry)
        })
}

fn bounded_input(
    owner: &VerifiedOrderedProgramCompositionV1,
    kernel: &KernelId,
    budget: &mut Budget<'_>,
) -> Result<(), OrderedCompositionFormalErrorV1> {
    let module = owner.canonical().module();
    if !contains(owner, module, kernel) {
        return Err(OrderedCompositionFormalErrorV1::Profile(
            "selected actual composition root",
        ));
    }
    let mut blocks = 0usize;
    let mut operations = 0usize;
    let mut memory = 0usize;
    let mut values = 0usize;
    for function in &module.functions {
        budget.charge_work(1)?;
        let body = function
            .body
            .as_ref()
            .ok_or(OrderedCompositionFormalErrorV1::Profile("defined function"))?;
        blocks = blocks
            .checked_add(body.blocks.len())
            .ok_or(Resource::Arithmetic)?;
        if blocks > 128
            || function.id.as_str().len() > 256
            || function.signature.parameters.len() > 8
        {
            return Err(OrderedCompositionFormalErrorV1::Profile(
                "formal function/block bound",
            ));
        }
        for block in &body.blocks {
            budget.charge_work(1)?;
            values = values
                .checked_add(block.parameters.len())
                .ok_or(Resource::Arithmetic)?;
            if values > 1024 {
                return Err(OrderedCompositionFormalErrorV1::Profile("formal SSA bound"));
            }
            let edge_values = match block.terminator.as_ref() {
                Some(crate::Terminator::Branch { arguments, .. }) => arguments.len(),
                Some(crate::Terminator::ConditionalBranch {
                    then_arguments,
                    else_arguments,
                    ..
                }) => then_arguments
                    .len()
                    .checked_add(else_arguments.len())
                    .ok_or(Resource::Arithmetic)?,
                Some(crate::Terminator::Switch {
                    selector,
                    cases,
                    default_target,
                    default_arguments,
                }) => bounded_boolean_switch(
                    function,
                    *selector,
                    cases,
                    *default_target,
                    default_arguments,
                    budget,
                )?,
                Some(crate::Terminator::Return { values }) => values.len(),
                Some(crate::Terminator::Unreachable) => 0,
                _ => {
                    return Err(OrderedCompositionFormalErrorV1::Profile(
                        "formal bounded branch profile",
                    ));
                }
            };
            if edge_values > 32 {
                return Err(OrderedCompositionFormalErrorV1::Profile(
                    "formal edge bound",
                ));
            }
            budget.charge_work(edge_values)?;
            operations = operations
                .checked_add(block.operations.len())
                .ok_or(Resource::Arithmetic)?;
            if operations > 512 {
                return Err(OrderedCompositionFormalErrorV1::Profile(
                    "formal operation bound",
                ));
            }
            for operation in &block.operations {
                budget.charge_work(1)?;
                values = values
                    .checked_add(operation.results.len())
                    .ok_or(Resource::Arithmetic)?;
                if values > 1024 {
                    return Err(OrderedCompositionFormalErrorV1::Profile("formal SSA bound"));
                }
                if matches!(
                    operation.kind,
                    OperationKind::Load { .. }
                        | OperationKind::Store { .. }
                        | OperationKind::GuardedLoad { .. }
                        | OperationKind::GuardedStore { .. }
                ) {
                    memory += 1;
                }
            }
        }
    }
    if memory > 16 || kernel.as_str().len() > 256 {
        return Err(OrderedCompositionFormalErrorV1::Profile(
            "formal access/name bound",
        ));
    }
    Ok(())
}

// Check actual retained collection capacities against the prepaid logical
// envelope, including incomplete reasons/callee strings and all pair reports.
fn report_payload(
    report: &FormalMemoryObligationAnalysis,
) -> Result<(), OrderedCompositionFormalErrorV1> {
    let obligations = report.obligations();
    let mut bytes = std::mem::size_of::<FormalMemoryObligationAnalysis>();
    let mut add = |value: usize| -> Result<(), Resource> {
        bytes = bytes.checked_add(value).ok_or(Resource::Arithmetic)?;
        Ok(())
    };
    add(obligations.kernel.retained_capacity_bytes())?;
    add(obligations.entry.retained_capacity_bytes())?;
    macro_rules! rows {
        ($field:ident, $ty:ty) => {
            add(obligations
                .$field
                .capacity()
                .checked_mul(std::mem::size_of::<$ty>())
                .ok_or(Resource::Arithmetic)?)?;
        };
    }
    rows!(allocations, FormalAllocationParameter);
    rows!(accesses, FormalMemoryAccess);
    rows!(bounds_requirements, FormalBoundsRequirement);
    rows!(runtime_alias_requirements, RuntimeAliasRequirement);
    rows!(
        inter_invocation_conflicts,
        InterInvocationConflictRequirement
    );
    if let FormalMemoryObligationAnalysis::Incomplete { reasons, .. } = report {
        add(reasons
            .capacity()
            .checked_mul(std::mem::size_of::<FormalMemoryIncompleteReason>())
            .ok_or(Resource::Arithmetic)?)?;
        for reason in reasons {
            if let FormalMemoryIncompleteReason::CallEffectsUnavailable { callee, .. } = reason {
                add(callee.retained_capacity_bytes())?;
            }
        }
    }
    if bytes > ORDERED_COMPOSITION_FORMAL_RETAINED_BYTES_V1 {
        return Err(Resource::Accounting.into());
    }
    Ok(())
}

// Admit only the source tail's Bool -> i64, two-case switch. This is a bounded
// input check, not a proof that the predicate discharges any memory condition.
// The existing formal analyzer receives the original CFG unchanged.
fn bounded_boolean_switch(
    function: &Function,
    selector: ValueId,
    cases: &[crate::SwitchCase],
    default: BlockId,
    default_arguments: &[ValueId],
    budget: &mut Budget<'_>,
) -> Result<usize, OrderedCompositionFormalErrorV1> {
    let refusal = || OrderedCompositionFormalErrorV1::Profile("formal bounded branch profile");
    if cases.len() != 2
        || cases[0].value != 0
        || cases[1].value != 1
        || cases[0].target == cases[1].target
        || cases
            .iter()
            .any(|case| case.target == default || !case.arguments.is_empty())
        || !default_arguments.is_empty()
    {
        return Err(refusal());
    }
    budget.charge_work(3)?;
    let body = function.body.as_ref().ok_or_else(refusal)?;
    let mut unreachable = false;
    for block in &body.blocks {
        budget.charge_work(1)?;
        if block.id == default {
            unreachable = block.parameters.is_empty()
                && block.operations.is_empty()
                && matches!(block.terminator, Some(crate::Terminator::Unreachable));
        }
    }
    if !unreachable {
        return Err(refusal());
    }
    let find = |id: ValueId, budget: &mut Budget<'_>| {
        let mut scanned = 0usize;
        for block in &body.blocks {
            budget.charge_work(1)?;
            for operation in &block.operations {
                scanned = scanned.checked_add(1).ok_or(Resource::Arithmetic)?;
                if scanned > 512 {
                    return Err(OrderedCompositionFormalErrorV1::Profile(
                        "formal operation bound",
                    ));
                }
                budget.charge_work(1)?;
                if matches!(operation.results.as_slice(), [result] if result.id == id) {
                    return Ok(Some(operation));
                }
            }
        }
        Ok(None)
    };
    let cast = find(selector, budget)?.ok_or_else(refusal)?;
    let OperationKind::Cast {
        kind: crate::CastKind::ZeroExtend,
        value,
        to: Type::Scalar(ScalarType::I64),
    } = &cast.kind
    else {
        return Err(refusal());
    };
    if cast.results[0].ty != Type::Scalar(ScalarType::I64)
        || find(*value, budget)?.is_none_or(|operation| operation.results[0].ty != Type::BOOL)
    {
        return Err(refusal());
    }
    Ok(0) // All three actual successor argument lists are explicitly empty.
}
