//! Borrowed call-instance coordinates for checked expansion, not scope authority.

use std::{fmt, mem::size_of, ops::Range};

use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as ResourceError,
};
use fe2o3_mir_model::{SsaBlockIdV1, semantic_mir_v1::*};
use fe2o3_pliron::{ProductionSemanticSsaFunctionPlanV1, ProductionSemanticSsaOwnerV1};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProductionCallInstanceIdV1(usize);

impl ProductionCallInstanceIdV1 {
    pub(crate) const fn index(self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProductionCallOccurrenceV1 {
    pub(crate) caller: ProductionCallInstanceIdV1,
    pub(crate) block: SemanticBlockIdV1,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum ProductionCallInstanceErrorV1 {
    Source,
    OccurrencesUnavailable,
    UnknownInstance,
    RecursiveCall {
        function: SemanticFunctionIdV1,
    },
    UnsupportedTerminator {
        function: SemanticFunctionIdV1,
        block: SemanticBlockIdV1,
    },
    UnsupportedCallAbi {
        function: SemanticFunctionIdV1,
        block: SemanticBlockIdV1,
    },
    InvalidParameter,
    NotBorrow,
    Resource(ResourceError),
}

impl fmt::Display for ProductionCallInstanceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(formatter),
            other => write!(formatter, "call-instance planning refused: {other:?}"),
        }
    }
}

impl std::error::Error for ProductionCallInstanceErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ResourceError> for ProductionCallInstanceErrorV1 {
    fn from(value: ResourceError) -> Self {
        Self::Resource(value)
    }
}

type Error = ProductionCallInstanceErrorV1;

pub(crate) struct ProductionCallInstanceV1<'s> {
    function: SemanticFunctionIdV1,
    declaration: &'s SemanticFunctionDeclV1,
    ssa: &'s ProductionSemanticSsaFunctionPlanV1,
    incoming: Option<usize>,
    calls: Range<usize>,
    exits: Range<usize>,
}

impl<'s> ProductionCallInstanceV1<'s> {
    pub(crate) const fn function(&self) -> SemanticFunctionIdV1 {
        self.function
    }
    pub(crate) const fn declaration(&self) -> &'s SemanticFunctionDeclV1 {
        self.declaration
    }
    pub(crate) const fn ssa(&self) -> &'s ProductionSemanticSsaFunctionPlanV1 {
        self.ssa
    }
}

pub(crate) struct ProductionInstanceCallV1<'s> {
    occurrence: ProductionCallOccurrenceV1,
    source: &'s SemanticDirectCallV1,
    callable: &'s SemanticCallableDeclV1,
    child: Option<ProductionCallInstanceIdV1>,
}

impl<'s> ProductionInstanceCallV1<'s> {
    pub(crate) const fn occurrence(&self) -> ProductionCallOccurrenceV1 {
        self.occurrence
    }
    pub(crate) const fn source(&self) -> &'s SemanticDirectCallV1 {
        self.source
    }
    pub(crate) const fn callable(&self) -> &'s SemanticCallableDeclV1 {
        self.callable
    }
    pub(crate) const fn child(&self) -> Option<ProductionCallInstanceIdV1> {
        self.child
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProductionInstanceExitKindV1 {
    Return { local: SemanticLocalIdV1 },
    DivergingCall,
    UnwindResume,
    UnwindTerminate,
    Abort,
    Unreachable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProductionInstanceExitV1 {
    pub(crate) instance: ProductionCallInstanceIdV1,
    pub(crate) block: SemanticBlockIdV1,
    pub(crate) kind: ProductionInstanceExitKindV1,
}

/// A source argument selector, not a flattened physical argument or authority.
pub(crate) struct ProductionInstanceParameterV1<'s> {
    pub(crate) operand: &'s SemanticOperandV1,
    pub(crate) tuple_field: Option<u32>,
    pub(crate) ty: SemanticTypeIdV1,
}

/// An actual Borrow statement. Uniqueness, extent and scope remain separate checks.
pub(crate) struct ProductionInstanceBorrowV1<'s> {
    pub(crate) instance: ProductionCallInstanceIdV1,
    pub(crate) block: SemanticBlockIdV1,
    pub(crate) statement: usize,
    pub(crate) destination: &'s SemanticPlaceV1,
    pub(crate) kind: SemanticBorrowKindV1,
    pub(crate) source: &'s SemanticPlaceV1,
}

// Logical capacities are explicit: an allocator's spare capacity never grants
// additional unmetered rows. Moving an existing vector is prepaid as work.
struct Rows<T> {
    values: Vec<T>,
    capacity: usize,
}

impl<T> Rows<T> {
    fn new() -> Self {
        Self {
            values: Vec::new(),
            capacity: 0,
        }
    }

    fn push(
        &mut self,
        value: T,
        budget: &mut Budget<'_>,
        storage: &mut usize,
    ) -> Result<(), Error> {
        budget.charge_work(1)?;
        if self.values.len() == self.capacity {
            let capacity = self
                .capacity
                .max(2)
                .checked_mul(2)
                .ok_or(ResourceError::Arithmetic)?;
            let additional = (capacity - self.capacity)
                .checked_mul(size_of::<T>())
                .ok_or(ResourceError::Arithmetic)?;
            let total = storage
                .checked_add(additional)
                .ok_or(ResourceError::Arithmetic)?;
            budget.charge_work(self.values.len())?;
            budget.reserve_storage(additional)?;
            *storage = total;
            self.values
                .try_reserve_exact(capacity - self.values.len())
                .map_err(|_| ResourceError::Allocation)?;
            self.capacity = capacity;
        }
        self.values.push(value);
        Ok(())
    }
}

pub(crate) struct ProductionCallInstancePlanV1<'s> {
    owner: &'s ProductionSemanticSsaOwnerV1,
    instances: Rows<ProductionCallInstanceV1<'s>>,
    calls: Rows<ProductionInstanceCallV1<'s>>,
    exits: Rows<ProductionInstanceExitV1>,
}

impl<'s> ProductionCallInstancePlanV1<'s> {
    pub(crate) const fn owner(&self) -> &'s ProductionSemanticSsaOwnerV1 {
        self.owner
    }
    pub(crate) const fn root(&self) -> ProductionCallInstanceIdV1 {
        ProductionCallInstanceIdV1(0)
    }
    pub(crate) fn instances(&self) -> &[ProductionCallInstanceV1<'s>] {
        &self.instances.values
    }
    pub(crate) fn id_at(&self, index: usize) -> Option<ProductionCallInstanceIdV1> {
        (index < self.instances.values.len()).then_some(ProductionCallInstanceIdV1(index))
    }
    pub(crate) fn instance(
        &self,
        id: ProductionCallInstanceIdV1,
    ) -> Option<&ProductionCallInstanceV1<'s>> {
        self.instances.values.get(id.0)
    }
    pub(crate) fn occurrences(
        &self,
        id: ProductionCallInstanceIdV1,
    ) -> Option<fe2o3_pliron::ProductionSemanticSsaFunctionOccurrencesV1<'s>> {
        self.owner
            .occurrences_v1()?
            .function(self.instance(id)?.function)
    }
    pub(crate) fn incoming(
        &self,
        id: ProductionCallInstanceIdV1,
    ) -> Option<&ProductionInstanceCallV1<'s>> {
        self.calls.values.get(self.instance(id)?.incoming?)
    }
    pub(crate) fn calls(
        &self,
        id: ProductionCallInstanceIdV1,
    ) -> Option<&[ProductionInstanceCallV1<'s>]> {
        self.calls.values.get(self.instance(id)?.calls.clone())
    }
    pub(crate) fn exits(
        &self,
        id: ProductionCallInstanceIdV1,
    ) -> Option<&[ProductionInstanceExitV1]> {
        self.exits.values.get(self.instance(id)?.exits.clone())
    }
    pub(crate) fn returns(
        &self,
        id: ProductionCallInstanceIdV1,
    ) -> Option<impl Iterator<Item = &ProductionInstanceExitV1>> {
        Some(
            self.exits(id)?
                .iter()
                .filter(|exit| matches!(exit.kind, ProductionInstanceExitKindV1::Return { .. })),
        )
    }

    /// Resolves the callee's original local role without conflating a RustCall
    /// tuple field with an ordinary argument ordinal or native register.
    pub(crate) fn parameter_source(
        &self,
        instance: ProductionCallInstanceIdV1,
        local: SemanticLocalIdV1,
        budget: &mut Budget<'_>,
    ) -> Result<ProductionInstanceParameterV1<'s>, Error> {
        budget.charge_work(12)?;
        let instance_row = self.instance(instance).ok_or(Error::UnknownInstance)?;
        let declaration = instance_row
            .declaration
            .locals()
            .get(local.index() as usize)
            .ok_or(Error::InvalidParameter)?;
        let call = self
            .incoming(instance)
            .ok_or(Error::InvalidParameter)?
            .source;
        let (argument, tuple_field) = match declaration.role() {
            SemanticLocalRoleV1::Argument(argument) => (argument, None),
            SemanticLocalRoleV1::RustCallTupleField { argument, field } => (argument, Some(field)),
            _ => return Err(Error::InvalidParameter),
        };
        let operand = call
            .arguments()
            .get(argument as usize)
            .ok_or(Error::InvalidParameter)?;
        let ty = if let Some(field) = tuple_field {
            if instance_row.declaration.abi().extern_abi() != SemanticExternAbiV1::RustCall {
                return Err(Error::InvalidParameter);
            }
            let tuple = self
                .owner
                .source_semantic()
                .types()
                .get(operand.ty().index() as usize)
                .ok_or(Error::InvalidParameter)?;
            let SemanticTypeShapeV1::Tuple(tuple) = tuple.shape() else {
                return Err(Error::InvalidParameter);
            };
            *tuple
                .fields()
                .get(field as usize)
                .ok_or(Error::InvalidParameter)?
        } else {
            operand.ty()
        };
        if ty != declaration.ty() {
            return Err(Error::InvalidParameter);
        }
        Ok(ProductionInstanceParameterV1 {
            operand,
            tuple_field,
            ty,
        })
    }

    /// Borrows the retained syntax even when the actual SSA adapter did not
    /// promote its reference local. This does not fabricate an SSA value.
    pub(crate) fn borrow_at(
        &self,
        instance: ProductionCallInstanceIdV1,
        block: SemanticBlockIdV1,
        statement: usize,
        budget: &mut Budget<'_>,
    ) -> Result<ProductionInstanceBorrowV1<'s>, Error> {
        budget.charge_work(12)?;
        let row = self.instance(instance).ok_or(Error::UnknownInstance)?;
        if !row
            .ssa
            .plan()
            .is_reachable(SsaBlockIdV1::new(block.index()))
        {
            return Err(Error::NotBorrow);
        }
        let statement_row = row
            .declaration
            .blocks()
            .get(block.index() as usize)
            .and_then(|block| block.statements().get(statement))
            .ok_or(Error::NotBorrow)?;
        let SemanticStatementKindV1::Assign(assignment) = statement_row.kind() else {
            return Err(Error::NotBorrow);
        };
        let SemanticRvalueKindV1::Borrow { kind, place } = assignment.value().kind() else {
            return Err(Error::NotBorrow);
        };
        let mutability = match kind {
            SemanticBorrowKindV1::Mutable => SemanticMutabilityV1::Mutable,
            SemanticBorrowKindV1::Shared => SemanticMutabilityV1::Immutable,
            SemanticBorrowKindV1::Fake => return Err(Error::NotBorrow),
        };
        let destination = assignment.destination();
        let ty = self
            .owner
            .source_semantic()
            .types()
            .get(destination.ty().index() as usize)
            .ok_or(Error::NotBorrow)?;
        if !destination.projections().is_empty()
            || !matches!(ty.shape(), SemanticTypeShapeV1::Pointer(pointer)
                if pointer.kind() == SemanticPointerKindV1::Reference
                    && pointer.mutability() == mutability && pointer.pointee() == place.ty())
        {
            return Err(Error::NotBorrow);
        }
        Ok(ProductionInstanceBorrowV1 {
            instance,
            block,
            statement,
            destination,
            kind: *kind,
            source: place,
        })
    }
}

/// Builds instance/return coordinates over the actual source, then drops its own
/// rows before releasing exactly their reservations. Consumer storage is not
/// rolled back; the consumer must manage any additional retained allocation.
/// This route does not replace or widen ordinary call lowering.
pub(crate) fn with_production_call_instances_v1<R, E>(
    owner: &ProductionSemanticSsaOwnerV1,
    root: SemanticFunctionIdV1,
    budget: &mut Budget<'_>,
    consumer: impl FnOnce(&ProductionCallInstancePlanV1<'_>, &mut Budget<'_>) -> Result<R, E>,
) -> Result<R, E>
where
    E: From<Error>,
{
    let mut plan = ProductionCallInstancePlanV1 {
        owner,
        instances: Rows::new(),
        calls: Rows::new(),
        exits: Rows::new(),
    };
    let mut storage = 0;
    let outcome = build(&mut plan, root, budget, &mut storage)
        .map_err(E::from)
        .and_then(|()| consumer(&plan, budget));
    drop(plan);
    budget
        .release_storage(storage)
        .map_err(Error::from)
        .map_err(E::from)?;
    outcome
}

fn instance_row<'s>(
    owner: &'s ProductionSemanticSsaOwnerV1,
    function: SemanticFunctionIdV1,
    incoming: Option<usize>,
    budget: &mut Budget<'_>,
) -> Result<ProductionCallInstanceV1<'s>, Error> {
    budget.charge_work(8)?;
    let declaration = owner
        .source_semantic()
        .functions()
        .get(function.index() as usize)
        .ok_or(Error::Source)?;
    let ssa = owner.plan_for_function(function).ok_or(Error::Source)?;
    let role = if incoming.is_none() {
        SemanticFunctionRoleV1::KernelRoot
    } else {
        SemanticFunctionRoleV1::InternalHelper
    };
    if ssa.function_identity() != declaration.identity() || declaration.role() != role {
        return Err(Error::Source);
    }
    Ok(ProductionCallInstanceV1 {
        function,
        declaration,
        ssa,
        incoming,
        calls: 0..0,
        exits: 0..0,
    })
}

fn build(
    plan: &mut ProductionCallInstancePlanV1<'_>,
    root: SemanticFunctionIdV1,
    budget: &mut Budget<'_>,
    storage: &mut usize,
) -> Result<(), Error> {
    budget.charge_work(1)?;
    let captured = plan
        .owner
        .occurrences_v1()
        .ok_or(Error::OccurrencesUnavailable)?;
    let root_row = instance_row(plan.owner, root, None, budget)?;
    plan.instances.push(root_row, budget, storage)?;
    let mut cursor = 0;
    while cursor < plan.instances.values.len() {
        budget.charge_work(1)?;
        let id = ProductionCallInstanceIdV1(cursor);
        let function = plan.instances.values[cursor].function;
        let declaration = plan.instances.values[cursor].declaration;
        let ssa = plan.instances.values[cursor].ssa;
        captured.function(function).ok_or(Error::Source)?;
        budget.charge_work(declaration.locals().len())?;
        let mut return_locals = declaration
            .locals()
            .iter()
            .enumerate()
            .filter(|(_, local)| local.role() == SemanticLocalRoleV1::Return);
        let (return_local, return_declaration) = return_locals.next().ok_or(Error::Source)?;
        if return_locals.next().is_some()
            || return_declaration.ty() != declaration.abi().source_output_type()
        {
            return Err(Error::Source);
        }
        let return_local = SemanticLocalIdV1::from_index(
            u32::try_from(return_local).map_err(|_| ResourceError::Arithmetic)?,
        );
        let calls_start = plan.calls.values.len();
        let exits_start = plan.exits.values.len();
        for source_block in ssa.plan().reverse_postorder() {
            budget.charge_work(4)?;
            let block = SemanticBlockIdV1::from_index(source_block.get());
            let body = declaration
                .blocks()
                .get(block.index() as usize)
                .ok_or(Error::Source)?;
            let kind = body.terminator().kind();
            let exit = match kind {
                SemanticTerminatorKindV1::Call(call) => {
                    budget.charge_work(4)?;
                    let callable = plan
                        .owner
                        .source_semantic()
                        .callables()
                        .get(call.callee().index() as usize)
                        .ok_or(Error::Source)?;
                    let child = if let SemanticCallableDeclV1::Defined { function: callee } =
                        callable
                    {
                        let mut ancestor = Some(id);
                        while let Some(current) = ancestor {
                            budget.charge_work(2)?;
                            let current_row = plan.instance(current).ok_or(Error::Source)?;
                            if current_row.function == *callee {
                                return Err(Error::RecursiveCall { function: *callee });
                            }
                            ancestor = plan.incoming(current).map(|call| call.occurrence.caller);
                        }
                        let child = ProductionCallInstanceIdV1(plan.instances.values.len());
                        let child_row = instance_row(
                            plan.owner,
                            *callee,
                            Some(plan.calls.values.len()),
                            budget,
                        )?;
                        if child_row.declaration.abi().c_variadic()
                            || !call.variadic_argument_abis().is_empty()
                        {
                            return Err(Error::UnsupportedCallAbi { function, block });
                        }
                        plan.instances.push(child_row, budget, storage)?;
                        Some(child)
                    } else {
                        None
                    };
                    plan.calls.push(
                        ProductionInstanceCallV1 {
                            occurrence: ProductionCallOccurrenceV1 { caller: id, block },
                            source: call,
                            callable,
                            child,
                        },
                        budget,
                        storage,
                    )?;
                    call.destination()
                        .is_none()
                        .then_some(ProductionInstanceExitKindV1::DivergingCall)
                }
                SemanticTerminatorKindV1::Return => Some(ProductionInstanceExitKindV1::Return {
                    local: return_local,
                }),
                SemanticTerminatorKindV1::UnwindResume => {
                    Some(ProductionInstanceExitKindV1::UnwindResume)
                }
                SemanticTerminatorKindV1::UnwindTerminate => {
                    Some(ProductionInstanceExitKindV1::UnwindTerminate)
                }
                SemanticTerminatorKindV1::Abort => Some(ProductionInstanceExitKindV1::Abort),
                SemanticTerminatorKindV1::Unreachable => {
                    Some(ProductionInstanceExitKindV1::Unreachable)
                }
                SemanticTerminatorKindV1::TailCall(_) | SemanticTerminatorKindV1::Drop { .. } => {
                    return Err(Error::UnsupportedTerminator { function, block });
                }
                SemanticTerminatorKindV1::Goto(_)
                | SemanticTerminatorKindV1::SwitchInt { .. }
                | SemanticTerminatorKindV1::Assert { .. }
                | SemanticTerminatorKindV1::FalseEdge { .. } => None,
            };
            if let Some(kind) = exit {
                plan.exits.push(
                    ProductionInstanceExitV1 {
                        instance: id,
                        block,
                        kind,
                    },
                    budget,
                    storage,
                )?;
            }
        }
        plan.instances.values[cursor].calls = calls_start..plan.calls.values.len();
        plan.instances.values[cursor].exits = exits_start..plan.exits.values.len();
        cursor += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;
    use fe2o3_pliron::{
        ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1, ProductionSemanticSsaLimitsV1,
    };

    const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
    const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
    const TUPLE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
    const REFERENCE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
    const ROOT: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(0);
    const FLOOR: usize = 37;

    #[derive(Clone, Copy)]
    enum Case {
        Ordinary,
        RustCall,
        Borrow,
        Recursive,
        Loop,
        Abort,
    }

    fn source() -> SemanticSourceProvenanceV1 {
        SemanticSourceProvenanceV1::unavailable()
    }
    fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
    }
    fn scalar(value: u32) -> SemanticOperandV1 {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            U32,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value.into(), 4).unwrap()),
        ))
    }
    fn ignored(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
        SemanticAbiValueV1::new(ty, SemanticAbiPassModeV1::Ignore)
    }
    fn direct(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
        SemanticAbiValueV1::new(
            ty,
            SemanticAbiPassModeV1::Direct(
                SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(
                        false,
                        None,
                        ty == REFERENCE,
                        false,
                        false,
                        true,
                    ),
                    SemanticAbiExtensionV1::None,
                    if ty == REFERENCE { 4 } else { 0 },
                    (ty == REFERENCE).then_some(4),
                )
                .unwrap(),
            ),
        )
    }
    fn abi(tag: u8, kernel: bool, arguments: &[SemanticTypeIdV1]) -> SemanticFunctionAbiV1 {
        SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([250; 32]),
            if kernel {
                SemanticCanonAbiV1::GpuKernel
            } else {
                SemanticCanonAbiV1::Rust
            },
            if kernel {
                SemanticExternAbiV1::GpuKernel
            } else {
                SemanticExternAbiV1::Rust
            },
            false,
            false,
            arguments.len() as u32,
            arguments
                .iter()
                .map(|ty| SemanticAbiArgumentV1::source(direct(*ty)))
                .collect(),
            ignored(UNIT),
        )
        .unwrap()
    }
    fn local(tag: u8, ty: SemanticTypeIdV1, role: SemanticLocalRoleV1) -> SemanticLocalDeclV1 {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([tag; 32]),
            ty,
            role,
            source(),
        )
    }
    fn block(
        tag: u8,
        statements: Vec<SemanticStatementV1>,
        kind: SemanticTerminatorKindV1,
    ) -> SemanticBasicBlockV1 {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([tag; 32]),
            source(),
            statements,
            SemanticTerminatorV1::new(source(), kind),
        )
        .unwrap()
    }
    fn assign(destination: SemanticPlaceV1, kind: SemanticRvalueKindV1) -> SemanticStatementV1 {
        let ty = destination.ty();
        SemanticStatementV1::new(
            source(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                destination,
                SemanticRvalueV1::new(ty, kind),
            )),
        )
    }
    fn call(
        callee: u32,
        arguments: Vec<SemanticOperandV1>,
        target: u32,
    ) -> SemanticTerminatorKindV1 {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(callee),
                arguments,
                Some(SemanticCallDestinationV1::new(
                    place(0, UNIT),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(target),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    }
    fn function(
        tag: u8,
        role: SemanticFunctionRoleV1,
        abi: SemanticFunctionAbiV1,
        locals: Vec<SemanticLocalDeclV1>,
        blocks: Vec<SemanticBasicBlockV1>,
    ) -> SemanticFunctionDeclV1 {
        SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([tag; 32]),
            role,
            SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
            source(),
            abi,
            locals,
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap()
    }

    fn fixture(case: Case, capture: bool) -> ProductionSemanticSsaOwnerV1 {
        let unit = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([4; 32]),
            SemanticLayoutIdentityV1::from_sha256([4; 32]),
            SemanticTypeLayoutV1::new(Some(0), 1).unwrap(),
            SemanticTypeShapeV1::Unit,
        );
        let scalar_type = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([5; 32]),
            SemanticLayoutIdentityV1::from_sha256([5; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(4),
                4,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, 32, 4),
                    SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            }),
        );
        let tuple = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([6; 32]),
            SemanticLayoutIdentityV1::from_sha256([6; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(4),
                4,
                SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![U32]).unwrap()),
        );
        let reference = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([7; 32]),
            SemanticLayoutIdentityV1::from_sha256([7; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                    SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    U32,
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Mutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        )
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(
                    SemanticAbiPointeeInfoV1::new(
                        SemanticAbiPointeeKindV1::MutableReference { unpin: true },
                        4,
                        4,
                    )
                    .unwrap(),
                ),
                None,
            ),
        );
        let mut root_blocks = Vec::new();
        for (ordinal, value) in [9, 11].into_iter().enumerate() {
            let (statements, arguments) = match case {
                Case::RustCall => (
                    vec![assign(
                        place(2, TUPLE),
                        SemanticRvalueKindV1::Aggregate(
                            SemanticAggregateRvalueV1::new(
                                SemanticAggregateKindV1::Tuple,
                                vec![scalar(value)],
                            )
                            .unwrap(),
                        ),
                    )],
                    vec![
                        SemanticOperandV1::Constant(SemanticConstantV1::new(
                            UNIT,
                            SemanticConstantValueV1::ZeroSized,
                        )),
                        SemanticOperandV1::Move(place(2, TUPLE)),
                    ],
                ),
                Case::Borrow => (
                    vec![assign(
                        place(3, REFERENCE),
                        SemanticRvalueKindV1::Borrow {
                            kind: SemanticBorrowKindV1::Mutable,
                            place: place(1, U32),
                        },
                    )],
                    vec![SemanticOperandV1::Move(place(3, REFERENCE))],
                ),
                _ => (vec![], vec![scalar(value)]),
            };
            root_blocks.push(block(
                40 + ordinal as u8,
                statements,
                call(1, arguments, ordinal as u32 + 1),
            ));
        }
        root_blocks.push(block(42, vec![], SemanticTerminatorKindV1::Return));
        let root = function(
            20,
            SemanticFunctionRoleV1::KernelRoot,
            abi(20, true, &[U32]),
            vec![
                local(30, UNIT, SemanticLocalRoleV1::Return),
                local(31, U32, SemanticLocalRoleV1::Argument(0)),
                local(32, TUPLE, SemanticLocalRoleV1::Temporary),
                local(33, REFERENCE, SemanticLocalRoleV1::Temporary),
            ],
            root_blocks,
        )
        .with_kernel_entry(SemanticKernelEntryV1::new(
            SemanticLinkSymbolV1::new(b"call_instance_fixture".to_vec()).unwrap(),
            SemanticKernelBindingIdentityV1::from_sha256([19; 32]),
            SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
        ));
        let mut helper_abi = abi(
            50,
            false,
            &[if matches!(case, Case::Borrow) {
                REFERENCE
            } else {
                U32
            }],
        );
        let mut helper_locals = vec![
            local(60, UNIT, SemanticLocalRoleV1::Return),
            local(
                61,
                if matches!(case, Case::Borrow) {
                    REFERENCE
                } else {
                    U32
                },
                SemanticLocalRoleV1::Argument(0),
            ),
        ];
        if matches!(case, Case::RustCall) {
            helper_abi = SemanticFunctionAbiV1::from_rustc_with_source_signature(
                SemanticAbiIdentityV1::from_sha256([50; 32]),
                SemanticLayoutIdentityV1::from_sha256([250; 32]),
                SemanticCanonAbiV1::Rust,
                SemanticExternAbiV1::RustCall,
                false,
                false,
                1,
                vec![UNIT, TUPLE],
                UNIT,
                vec![
                    SemanticAbiArgumentV1::source(ignored(UNIT)),
                    SemanticAbiArgumentV1::rust_call_tuple_field(0, direct(U32)),
                ],
                ignored(UNIT),
            )
            .unwrap();
            helper_locals = vec![
                local(60, UNIT, SemanticLocalRoleV1::Return),
                local(61, UNIT, SemanticLocalRoleV1::Argument(0)),
                local(
                    62,
                    U32,
                    SemanticLocalRoleV1::RustCallTupleField {
                        argument: 1,
                        field: 0,
                    },
                ),
            ];
        } else if matches!(case, Case::Borrow) {
            helper_abi = helper_abi
                .with_source_argument_ownership(vec![
                    SemanticSourceArgumentOwnershipV1::UniqueBorrow,
                ])
                .unwrap();
        }
        let helper_blocks = match case {
            Case::Recursive => vec![
                block(70, vec![], call(1, vec![scalar(1)], 1)),
                block(71, vec![], SemanticTerminatorKindV1::Return),
            ],
            Case::Loop => vec![
                block(70, vec![], call(2, vec![scalar(1)], 1)),
                block(
                    71,
                    vec![],
                    SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::Goto,
                        SemanticBlockIdV1::from_index(0),
                    )),
                ),
            ],
            Case::Abort => vec![block(70, vec![], SemanticTerminatorKindV1::Abort)],
            _ => vec![block(70, vec![], SemanticTerminatorKindV1::Return)],
        };
        let helper = function(
            50,
            SemanticFunctionRoleV1::InternalHelper,
            helper_abi,
            helper_locals,
            helper_blocks,
        );
        let mut functions = vec![root, helper];
        if matches!(case, Case::Loop) {
            functions.push(function(
                80,
                SemanticFunctionRoleV1::InternalHelper,
                abi(80, false, &[U32]),
                vec![
                    local(90, UNIT, SemanticLocalRoleV1::Return),
                    local(91, U32, SemanticLocalRoleV1::Argument(0)),
                ],
                vec![block(100, vec![], SemanticTerminatorKindV1::Return)],
            ));
        }
        let callables = (0..functions.len())
            .map(|index| {
                SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(index as u32))
            })
            .collect();
        let admitted = InertSemanticMirRequestV1::new_with_callables(
            SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
            vec![unit, scalar_type, tuple, reference],
            vec![],
            vec![],
            vec![],
            functions,
            callables,
            vec![ROOT],
        )
        .unwrap()
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
        let semantic = ProductionSemanticMirOwnerV1::try_new(
            admitted,
            ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap();
        let mut owner = ProductionSemanticSsaOwnerV1::try_new(
            semantic,
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
        if capture {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = Budget::new(&mut work, usize::MAX);
            owner
                .try_capture_occurrences_with_budget_v1(&mut budget)
                .unwrap();
        }
        owner
    }

    #[test]
    fn repeated_helpers_have_distinct_instances_and_exact_returns() {
        let owner = fixture(Case::Ordinary, true);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        with_production_call_instances_v1(
            &owner,
            ROOT,
            &mut budget,
            |plan, budget| -> Result<(), Error> {
                assert!(std::ptr::eq(plan.owner(), &owner));
                assert_eq!(plan.instances().len(), 3);
                let calls = plan.calls(plan.root()).unwrap();
                assert_eq!(calls.len(), 2);
                assert_ne!(calls[0].child(), calls[1].child());
                for (ordinal, call) in calls.iter().enumerate() {
                    let child = call.child().unwrap();
                    assert_eq!(call.occurrence().block.index(), ordinal as u32);
                    assert_eq!(plan.instance(child).unwrap().function().index(), 1);
                    assert!(std::ptr::eq(
                        plan.incoming(child).unwrap().source(),
                        call.source()
                    ));
                    assert_eq!(plan.returns(child).unwrap().count(), 1);
                    let parameter =
                        plan.parameter_source(child, SemanticLocalIdV1::from_index(1), budget)?;
                    assert_eq!(
                        parameter.operand,
                        &scalar(if ordinal == 0 { 9 } else { 11 })
                    );
                    assert_eq!((parameter.tuple_field, parameter.ty), (None, U32));
                    assert!(plan.occurrences(child).is_some());
                }
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(budget.storage(), 0);
    }

    #[test]
    fn rust_call_parameter_retains_outer_tuple_field_identity() {
        let owner = fixture(Case::RustCall, true);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        with_production_call_instances_v1(
            &owner,
            ROOT,
            &mut budget,
            |plan, budget| -> Result<(), Error> {
                for call in plan.calls(plan.root()).unwrap() {
                    let child = call.child().unwrap();
                    let receiver =
                        plan.parameter_source(child, SemanticLocalIdV1::from_index(1), budget)?;
                    assert_eq!((receiver.tuple_field, receiver.ty), (None, UNIT));
                    let field =
                        plan.parameter_source(child, SemanticLocalIdV1::from_index(2), budget)?;
                    assert_eq!((field.tuple_field, field.ty), (Some(0), U32));
                    assert!(std::ptr::eq(field.operand, &call.source().arguments()[1]));
                    assert!(matches!(
                        plan.parameter_source(child, SemanticLocalIdV1::from_index(0), budget),
                        Err(Error::InvalidParameter)
                    ));
                }
                Ok(())
            },
        )
        .unwrap();
    }

    #[test]
    fn retained_borrow_is_syntax_not_a_manufactured_ssa_value() {
        let owner = fixture(Case::Borrow, true);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        with_production_call_instances_v1(
            &owner,
            ROOT,
            &mut budget,
            |plan, budget| -> Result<(), Error> {
                let borrow =
                    plan.borrow_at(plan.root(), SemanticBlockIdV1::from_index(0), 0, budget)?;
                assert_eq!(
                    (borrow.instance, borrow.block.index(), borrow.statement),
                    (plan.root(), 0, 0)
                );
                assert_eq!(borrow.kind, SemanticBorrowKindV1::Mutable);
                assert_eq!(
                    (borrow.source.local().index(), borrow.source.ty()),
                    (1, U32)
                );
                assert_eq!(
                    (borrow.destination.local().index(), borrow.destination.ty()),
                    (3, REFERENCE)
                );
                let source = plan.instance(plan.root()).unwrap().declaration();
                let SemanticStatementKindV1::Assign(assignment) =
                    source.blocks()[0].statements()[0].kind()
                else {
                    unreachable!()
                };
                assert!(std::ptr::eq(borrow.destination, assignment.destination()));
                assert!(matches!(
                    plan.borrow_at(plan.root(), SemanticBlockIdV1::from_index(0), 1, budget),
                    Err(Error::NotBorrow)
                ));
                Ok(())
            },
        )
        .unwrap();
    }

    #[test]
    fn cfg_loops_are_finite_static_instances_but_recursive_calls_refuse() {
        for (case, recursive) in [(Case::Loop, false), (Case::Recursive, true)] {
            let owner = fixture(case, true);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = Budget::new(&mut work, usize::MAX);
            budget.reserve_storage(FLOOR).unwrap();
            let result = with_production_call_instances_v1(
                &owner,
                ROOT,
                &mut budget,
                |plan, _| -> Result<(), Error> {
                    assert_eq!(plan.instances().len(), 5);
                    for call in plan.calls(plan.root()).unwrap() {
                        let loop_instance = call.child().unwrap();
                        assert_eq!(plan.calls(loop_instance).unwrap().len(), 1);
                        assert_eq!(plan.returns(loop_instance).unwrap().count(), 0);
                    }
                    Ok(())
                },
            );
            if recursive {
                assert!(
                    matches!(result, Err(Error::RecursiveCall { function }) if function.index() == 1)
                );
            } else {
                result.unwrap();
            }
            assert_eq!(budget.storage(), FLOOR);
        }
    }

    #[test]
    fn missing_occurrences_and_abnormal_exits_are_not_successful_returns() {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let uncaptured = fixture(Case::Ordinary, false);
        let missing = with_production_call_instances_v1(
            &uncaptured,
            ROOT,
            &mut budget,
            |_, _| -> Result<(), Error> { panic!("missing capture must not visit") },
        );
        assert_eq!(missing, Err(Error::OccurrencesUnavailable));
        let owner = fixture(Case::Abort, true);
        with_production_call_instances_v1(
            &owner,
            ROOT,
            &mut budget,
            |plan, _| -> Result<(), Error> {
                for call in plan.calls(plan.root()).unwrap() {
                    let child = call.child().unwrap();
                    assert_eq!(plan.returns(child).unwrap().count(), 0);
                    assert!(matches!(
                        plan.exits(child).unwrap(),
                        [ProductionInstanceExitV1 {
                            kind: ProductionInstanceExitKindV1::Abort,
                            ..
                        }]
                    ));
                }
                Ok(())
            },
        )
        .unwrap();
    }

    #[test]
    fn exact_resource_limits_and_consumer_storage_preserve_the_owner_floor() {
        let owner = fixture(Case::Ordinary, true);
        let measure = |work_limit, storage_limit| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(FLOOR).unwrap();
            let result = with_production_call_instances_v1(&owner, ROOT, &mut budget, |_, _| {
                Ok::<_, Error>(())
            });
            assert_eq!(budget.storage(), FLOOR);
            (result, budget.work(), budget.peak_storage())
        };
        let (result, work, storage) = measure(usize::MAX, usize::MAX);
        result.unwrap();
        measure(work, storage).0.unwrap();
        assert!(matches!(
            measure(work - 1, storage).0,
            Err(Error::Resource(ResourceError::Work(_)))
        ));
        assert!(matches!(
            measure(work, storage - 1).0,
            Err(Error::Resource(ResourceError::Storage(_)))
        ));
        for cap in 0..work {
            assert!(matches!(
                measure(cap, usize::MAX).0,
                Err(Error::Resource(ResourceError::Work(_)))
            ));
        }
        for fail in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = Budget::new(&mut work, usize::MAX);
            budget.reserve_storage(FLOOR).unwrap();
            let result = with_production_call_instances_v1(
                &owner,
                ROOT,
                &mut budget,
                |_, budget| -> Result<(), Error> {
                    budget.reserve_storage(7)?;
                    if fail {
                        Err(Error::InvalidParameter)
                    } else {
                        Ok(())
                    }
                },
            );
            assert_eq!(
                result,
                if fail {
                    Err(Error::InvalidParameter)
                } else {
                    Ok(())
                }
            );
            assert_eq!(budget.storage(), FLOOR + 7);
            budget.release_storage(7).unwrap();
        }
    }
}
