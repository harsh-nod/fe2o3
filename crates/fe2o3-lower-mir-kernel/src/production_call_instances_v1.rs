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
#[path = "production_call_instances_v1_tests.rs"]
mod tests;
