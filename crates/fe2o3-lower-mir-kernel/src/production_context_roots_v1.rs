//! Borrowed context-root agreement, not producer authentication or scope admission.

use std::fmt;

use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as ResourceError,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::{ProductionSemanticSsaFunctionPlanV1, ProductionSemanticSsaOwnerV1};

use crate::{ProductionSourceLaunchRootV1, ProductionSourceLaunchRosterV1};

/// Detached, untrusted coordinates of one ordinary direct call in a physical root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionContextCallBoundaryV29 {
    /// Source basic block containing the call terminator.
    pub block: SemanticBlockIdV1,
    /// Exact number of statements preceding that terminator.
    pub statement_count: usize,
    /// Whole destination local; projected destinations are not this protocol.
    pub destination: SemanticLocalIdV1,
    /// Exact nominal destination type.
    pub destination_type: SemanticTypeIdV1,
    /// Normal call-return continuation.
    pub target: SemanticBlockIdV1,
    /// Retained unwind action, without reinterpreting it as scope closure.
    pub unwind: SemanticUnwindActionV1,
}

/// Untrusted facts projected from the backend's separately retained context receipt.
///
/// Constructing this input, including from a matching digest, establishes no
/// source authority. Production must retain and check its private originating
/// receipt beside the source owner. This protocol describes a physical root's
/// issuance and logical-helper call, not a workgroup callback or its captures.
#[derive(Clone, Copy, Debug)]
pub struct ProductionContextRootInputV29<'a> {
    /// Exact admitted source identity retained by the producer.
    pub semantic_sha256: &'a [u8; 32],
    /// Physical kernel root, not the logical helper substituted as a new entry.
    pub root: SemanticFunctionIdV1,
    /// Retained physical root identity.
    pub root_identity: SemanticFunctionIdentityV1,
    /// Logical helper called by the physical root.
    pub helper: SemanticFunctionIdV1,
    /// Retained logical helper identity.
    pub helper_identity: SemanticFunctionIdentityV1,
    /// Context issuer in the callable table, not the function table.
    pub issuer: SemanticCallableIdV1,
    /// Retained issuer binding identity.
    pub issuer_identity: SemanticFunctionIdentityV1,
    /// Nominal kernel context type.
    pub context_type: SemanticTypeIdV1,
    /// Retained context type identity.
    pub context_identity: SemanticTypeIdentityV1,
    /// Exact issuance boundary in the root.
    pub issuance: ProductionContextCallBoundaryV29,
    /// Exact logical-helper call boundary in the root.
    pub helper_call: ProductionContextCallBoundaryV29,
    /// Root local moved into the first logical-helper argument.
    pub helper_context_local: SemanticLocalIdV1,
    /// Retained ordered arguments: whole-local moves/copies or zero-sized constants.
    pub helper_arguments: &'a [SemanticOperandV1],
}

/// Root-handoff disagreement or exhaustion of the shared cumulative budget.
#[derive(Debug, Eq, PartialEq)]
pub enum ProductionContextRootErrorV29 {
    /// Wrong source version or source identity.
    Source,
    /// Wrong physical root, role, entry or identity.
    Root,
    /// Wrong logical helper, role or identity.
    Helper,
    /// Wrong context issuer callable, operation or binding identity.
    Issuer,
    /// Wrong nominal context type or identity.
    ContextType,
    /// Changed call kind, destination, ordinal or continuation.
    CallBoundary,
    /// Changed argument order, context move or unsupported root transport.
    Arguments,
    /// Source declarations disagree with retained SSA plans.
    Ssa,
    /// Source and launch roster disagree.
    Launch,
    /// Missing, duplicated, reordered or extra physical context issuance.
    RootCensus,
    /// Incomplete callable classifications or mismatched provider/derive identity.
    CallableCensus,
    /// Missing, extra or changed scope events in the complete source census.
    ScopeEventCensus,
    /// The shared ledger refused further work.
    Resource(ResourceError),
}

impl fmt::Display for ProductionContextRootErrorV29 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(formatter),
            other => write!(formatter, "context-root handoff mismatch: {other:?}"),
        }
    }
}

impl std::error::Error for ProductionContextRootErrorV29 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ResourceError> for ProductionContextRootErrorV29 {
    fn from(error: ResourceError) -> Self {
        Self::Resource(error)
    }
}

/// Scoped view of actual source objects after root-handoff consistency checks.
///
/// No field borrows the detached input. This is not producer authentication,
/// scope/borrow correctness, a complete issuance census, or execution authority.
/// The original physical root remains the ABI entry. No capability is erased.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionCheckedContextRootV29;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<ProductionCheckedContextRootV29<'static>>();
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{ProductionCheckedContextRootV29,
///     ProductionContextRootInputV29, ProductionContextRootErrorV29,
///     ProductionSourceLaunchRosterV1, with_checked_context_root_v29};
/// use fe2o3_pliron::ProductionSemanticSsaOwnerV1;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// fn escape<'a>(ssa: &'a ProductionSemanticSsaOwnerV1,
///     launch: &'a ProductionSourceLaunchRosterV1,
///     input: ProductionContextRootInputV29<'_>,
///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
/// ) -> Result<ProductionCheckedContextRootV29<'a>, ProductionContextRootErrorV29> {
///     with_checked_context_root_v29(ssa, launch, input, budget, |checked, _| Ok(checked))
/// }
/// ```
pub struct ProductionCheckedContextRootV29<'a> {
    semantic_ssa: &'a ProductionSemanticSsaOwnerV1,
    root: &'a SemanticFunctionDeclV1,
    helper: &'a SemanticFunctionDeclV1,
    issuance: &'a SemanticDirectCallV1,
    helper_call: &'a SemanticDirectCallV1,
    issuance_boundary: ProductionContextCallBoundaryV29,
    helper_call_boundary: ProductionContextCallBoundaryV29,
    root_plan: &'a ProductionSemanticSsaFunctionPlanV1,
    helper_plan: &'a ProductionSemanticSsaFunctionPlanV1,
    launch: &'a ProductionSourceLaunchRootV1,
    context_type: SemanticTypeIdV1,
}

impl<'a> ProductionCheckedContextRootV29<'a> {
    /// Exact immutable source/SSA owner used by the check.
    pub const fn semantic_ssa(&self) -> &'a ProductionSemanticSsaOwnerV1 {
        self.semantic_ssa
    }
    /// Original physical root ID.
    pub const fn root_id(&self) -> SemanticFunctionIdV1 {
        self.root_plan.function()
    }
    /// Logical helper ID resolved through the actual callable table.
    pub const fn helper_id(&self) -> SemanticFunctionIdV1 {
        self.helper_plan.function()
    }
    /// Exact nominal context type, not its ordinary zero-sized representation.
    pub const fn context_type(&self) -> SemanticTypeIdV1 {
        self.context_type
    }
    /// Actual physical root declaration.
    pub const fn root(&self) -> &'a SemanticFunctionDeclV1 {
        self.root
    }
    /// Actual logical helper declaration.
    pub const fn helper(&self) -> &'a SemanticFunctionDeclV1 {
        self.helper
    }
    /// Actual context issuance call.
    pub const fn issuance(&self) -> &'a SemanticDirectCallV1 {
        self.issuance
    }
    /// Actual logical-helper call and ordered arguments.
    pub const fn helper_call(&self) -> &'a SemanticDirectCallV1 {
        self.helper_call
    }
    /// Checked coordinates of the actual context issuance call.
    pub const fn issuance_boundary(&self) -> ProductionContextCallBoundaryV29 {
        self.issuance_boundary
    }
    /// Checked coordinates of the actual logical-helper call.
    pub const fn helper_call_boundary(&self) -> ProductionContextCallBoundaryV29 {
        self.helper_call_boundary
    }
    /// Retained SSA plan for the physical root.
    pub const fn root_plan(&self) -> &'a ProductionSemanticSsaFunctionPlanV1 {
        self.root_plan
    }
    /// Retained SSA plan for the logical helper.
    pub const fn helper_plan(&self) -> &'a ProductionSemanticSsaFunctionPlanV1 {
        self.helper_plan
    }
    /// Exact source-launch association for the physical root.
    pub const fn launch(&self) -> &'a ProductionSourceLaunchRootV1 {
        self.launch
    }
}

/// Checks one root's detached anchors, then visits only the actual retained data.
///
/// The checker allocates nothing and charges all its lookups, comparisons and
/// roster scans before performing them. Root operands are deliberately limited
/// to whole-local moves/copies and zero-sized constants, matching the producer
/// protocol; callback argument transport is a separate future contract.
/// The visitor receives the same cumulative ledger and owns its additional work
/// and storage charges. Neither failure nor success rolls back that ledger.
/// Existing V29 executable-admission gates are unchanged by this check.
pub fn with_checked_context_root_v29<R>(
    semantic_ssa: &ProductionSemanticSsaOwnerV1,
    launch: &ProductionSourceLaunchRosterV1,
    input: ProductionContextRootInputV29<'_>,
    budget: &mut Budget<'_>,
    use_root: impl for<'a> FnOnce(
        ProductionCheckedContextRootV29<'a>,
        &mut Budget<'_>,
    ) -> Result<R, ProductionContextRootErrorV29>,
) -> Result<R, ProductionContextRootErrorV29> {
    use ProductionContextRootErrorV29 as Error;

    budget.charge_work(32)?;
    let semantic = semantic_ssa.source_semantic();
    if semantic.wire_version() != SemanticMirWireVersionV1::V29
        || input.semantic_sha256 != semantic_ssa.source_semantic_sha256()
    {
        return Err(Error::Source);
    }
    if launch.semantic_sha256() != input.semantic_sha256
        || launch.roots().len() != semantic.roots().len()
    {
        return Err(Error::Launch);
    }
    let root = semantic
        .functions()
        .get(input.root.index() as usize)
        .ok_or(Error::Root)?;
    if root.identity() != input.root_identity
        || root.role() != SemanticFunctionRoleV1::KernelRoot
        || root.kernel_entry().is_none()
    {
        return Err(Error::Root);
    }
    let helper = semantic
        .functions()
        .get(input.helper.index() as usize)
        .ok_or(Error::Helper)?;
    if input.root == input.helper
        || helper.identity() != input.helper_identity
        || helper.role() != SemanticFunctionRoleV1::InternalHelper
    {
        return Err(Error::Helper);
    }
    let context = semantic
        .types()
        .get(input.context_type.index() as usize)
        .ok_or(Error::ContextType)?;
    if context.identity() != input.context_identity
        || context.rust_type_kind()
            != SemanticRustTypeKindV1::Execution(SemanticExecutionRoleV29::KernelContext)
    {
        return Err(Error::ContextType);
    }
    let issuance = input.issuance.observe(root, budget)?;
    let helper_call = input.helper_call.observe(root, budget)?;
    if issuance.callee() != input.issuer
        || !issuance.arguments().is_empty()
        || input.issuance.destination_type != input.context_type
        || !matches!(semantic.callables().get(input.issuer.index() as usize),
            Some(SemanticCallableDeclV1::CompilerIntrinsic {
                binding,
                operation: SemanticCompilerIntrinsicOperationV1::Execution(
                    SemanticExecutionOperationV29::ContextIssue { context }), ..
            }) if binding.identity() == input.issuer_identity && *context == input.context_type)
    {
        return Err(Error::Issuer);
    }
    if !matches!(semantic.callables().get(helper_call.callee().index() as usize),
        Some(SemanticCallableDeclV1::Defined { function }) if *function == input.helper)
    {
        return Err(Error::Helper);
    }
    check_arguments(input.helper_arguments, helper_call.arguments(), budget)?;
    budget.charge_work(4)?;
    if !matches!(helper_call.arguments().first(), Some(SemanticOperandV1::Move(place))
        if place.local() == input.helper_context_local
            && place.ty() == input.context_type && place.projections().is_empty())
    {
        return Err(Error::Arguments);
    }
    let root_plan = semantic_ssa
        .plan_for_function(input.root)
        .ok_or(Error::Ssa)?;
    let helper_plan = semantic_ssa
        .plan_for_function(input.helper)
        .ok_or(Error::Ssa)?;
    if root_plan.function_identity() != root.identity()
        || helper_plan.function_identity() != helper.identity()
    {
        return Err(Error::Ssa);
    }
    let mut root_ordinal = None;
    for (ordinal, candidate) in semantic.roots().iter().enumerate() {
        budget.charge_work(1)?;
        if *candidate == input.root {
            root_ordinal = Some(ordinal);
            break;
        }
    }
    let launch_root = launch
        .roots()
        .get(root_ordinal.ok_or(Error::Root)?)
        .ok_or(Error::Launch)?;
    budget.charge_work(3)?;
    if launch_root.selected_root() != input.root
        || launch_root.semantic_root_identity() != input.root_identity
        || root
            .kernel_entry()
            .map(|entry| *entry.kernel_binding_identity().as_bytes())
            != Some(launch_root.kernel_binding())
    {
        return Err(Error::Launch);
    }
    use_root(
        ProductionCheckedContextRootV29 {
            semantic_ssa,
            root,
            helper,
            issuance,
            helper_call,
            issuance_boundary: input.issuance,
            helper_call_boundary: input.helper_call,
            root_plan,
            helper_plan,
            launch: launch_root,
            context_type: input.context_type,
        },
        budget,
    )
}

impl ProductionContextCallBoundaryV29 {
    fn observe<'a>(
        self,
        function: &'a SemanticFunctionDeclV1,
        budget: &mut Budget<'_>,
    ) -> Result<&'a SemanticDirectCallV1, ProductionContextRootErrorV29> {
        use ProductionContextRootErrorV29::CallBoundary;

        budget.charge_work(12)?;
        let block = function
            .blocks()
            .get(self.block.index() as usize)
            .ok_or(CallBoundary)?;
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            return Err(CallBoundary);
        };
        let destination = call.destination().ok_or(CallBoundary)?;
        if block.statements().len() != self.statement_count
            || destination.place().local() != self.destination
            || destination.place().ty() != self.destination_type
            || !destination.place().projections().is_empty()
            || destination.edge().target() != self.target
            || destination.edge().role() != SemanticEdgeRoleV1::CallReturn
            || call.unwind() != self.unwind
            || !call.variadic_argument_abis().is_empty()
        {
            return Err(CallBoundary);
        }
        Ok(call)
    }
}

fn check_arguments(
    expected: &[SemanticOperandV1],
    actual: &[SemanticOperandV1],
    budget: &mut Budget<'_>,
) -> Result<(), ProductionContextRootErrorV29> {
    use ProductionContextRootErrorV29::Arguments;

    budget.charge_work(1)?;
    if expected.len() != actual.len() {
        return Err(Arguments);
    }
    let flat = |operand: &SemanticOperandV1| match operand {
        SemanticOperandV1::Move(place) | SemanticOperandV1::Copy(place) => {
            place.projections().is_empty()
        }
        SemanticOperandV1::Constant(value) => {
            matches!(value.value(), SemanticConstantValueV1::ZeroSized)
        }
    };
    for (expected, actual) in expected.iter().zip(actual) {
        budget.charge_work(8)?;
        // Reject variable-size payloads before deriving equality's work bound.
        if !flat(expected) || !flat(actual) || expected != actual {
            return Err(Arguments);
        }
    }
    Ok(())
}
