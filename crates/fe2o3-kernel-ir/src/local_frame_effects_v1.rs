//! A closed, scoped local-memory classification, separate from raw effects.

use std::{
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
};

use crate::{
    AddressSpace, BasicBlock, BlockId, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as ResourceError, CastKind, Constant, Function,
    KirLocalMemoryEffectRefV1, Module, Operation, OperationKind, ScalarType, Terminator, Type,
    ValueId, VerifiedKernelIrModuleV1, valid_scalar_cast, verification_bounded_sort_by_v1,
    verification_find_last_by_v1,
};

mod chain_v1;
pub use chain_v1::{
    CheckedLocalFrameChainV1, LocalFrameControlKindV1, LocalFrameControlV1,
    LocalFrameEdgeBindingV1, with_checked_local_frame_chain_function_v1,
};

/// Closed leaf rules exclude live calls, loops and unresolved control choices.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalFrameRefusalReasonV1 {
    Function,
    Signature,
    ControlFlow,
    Operation,
    Effects,
    Definition,
    PointerUse,
    Allocation,
    Index,
    Alignment,
    UninitializedRead,
}

/// A closed-rule refusal or a failure of the caller's existing resource ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalFrameErrorV1 {
    Unsupported {
        function_ordinal: usize,
        operation: Option<usize>,
        reason: LocalFrameRefusalReasonV1,
    },
    Resource(ResourceError),
}

impl From<ResourceError> for LocalFrameErrorV1 {
    fn from(value: ResourceError) -> Self {
        Self::Resource(value)
    }
}

impl fmt::Display for LocalFrameErrorV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported {
                function_ordinal,
                operation,
                reason,
            } => write!(
                out,
                "local-frame function {function_ordinal}, operation {operation:?}: {reason:?}"
            ),
            Self::Resource(error) => error.fmt(out),
        }
    }
}
impl std::error::Error for LocalFrameErrorV1 {}

/// A location is qualified by the exact module borrowed by its checked result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalFrameLocationV1 {
    function_ordinal: usize,
    block: BlockId,
    operation: usize,
}
impl LocalFrameLocationV1 {
    pub const fn function_ordinal(self) -> usize {
        self.function_ordinal
    }
    pub const fn block(self) -> BlockId {
        self.block
    }
    pub const fn operation(self) -> usize {
        self.operation
    }
}

/// A fresh private allocation per activation, not an allocation-elision license.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalFrameAllocationV1 {
    location: LocalFrameLocationV1,
    pointer: ValueId,
    element: ScalarType,
    element_bytes: u64,
    count: u64,
    byte_extent: u64,
    alignment: u32,
}
impl LocalFrameAllocationV1 {
    pub const fn location(self) -> LocalFrameLocationV1 {
        self.location
    }
    pub const fn pointer(self) -> ValueId {
        self.pointer
    }
    pub const fn element(self) -> ScalarType {
        self.element
    }
    pub const fn element_bytes(self) -> u64 {
        self.element_bytes
    }
    pub const fn count(self) -> u64 {
        self.count
    }
    pub const fn byte_extent(self) -> u64 {
        self.byte_extent
    }
    pub const fn alignment(self) -> u32 {
        self.alignment
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalFrameAccessKindV1 {
    Read,
    Write,
}

/// Every physical access remains an obligation, with its exact scalar SSA value.
/// A read names its preceding same-cell Store, not a scalar-equivalence theorem.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocalFrameAccessV1 {
    location: LocalFrameLocationV1,
    allocation: usize,
    cell: u64,
    pointer: ValueId,
    value: ValueId,
    kind: LocalFrameAccessKindV1,
    initializing_store: Option<LocalFrameLocationV1>,
}
impl LocalFrameAccessV1 {
    pub const fn location(self) -> LocalFrameLocationV1 {
        self.location
    }
    /// Ordinal in this result's allocation roster, never a cross-function ID.
    pub const fn allocation(self) -> usize {
        self.allocation
    }
    pub const fn cell(self) -> u64 {
        self.cell
    }
    pub const fn pointer(self) -> ValueId {
        self.pointer
    }
    pub const fn value(self) -> ValueId {
        self.value
    }
    pub const fn kind(self) -> LocalFrameAccessKindV1 {
        self.kind
    }
    pub const fn initializing_store(self) -> Option<LocalFrameLocationV1> {
        self.initializing_store
    }
}

/// A scoped classification of retained local-memory obligations only.
///
/// This is not purity, source equivalence, termination, target allocation
/// feasibility, or artifact authority. No interprocedural caller consumes it.
///
/// The checked owner cannot be forged outside this module.
/// ```compile_fail
/// use fe2o3_kernel_ir::CheckedLocalFrameV1;
/// let fabricated = CheckedLocalFrameV1 {};
/// ```
/// Its borrow cannot escape the callback, even while the module stays alive.
/// ```compile_fail
/// use fe2o3_kernel_ir::*;
/// fn escape(verified: VerifiedKernelIrModuleV1<'_>,
///           budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let mut escaped = None;
///     with_checked_local_frame_function_v1(verified, 0, budget, |checked, _| {
///         escaped = Some(checked);
///         Ok(())
///     }).unwrap();
///     drop(escaped);
/// }
/// ```
pub struct CheckedLocalFrameV1<'scope, 'module> {
    module: &'module Module,
    function: &'module Function,
    function_ordinal: usize,
    allocations: &'scope [LocalFrameAllocationV1],
    accesses: &'scope [LocalFrameAccessV1],
    ledger: usize,
    work_ledger: crate::CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
}

impl<'scope, 'module> CheckedLocalFrameV1<'scope, 'module> {
    /// Exact borrowed graph subject. A raw borrow does not grant proof authority.
    pub const fn module(&self) -> &'module Module {
        self.module
    }
    pub const fn function(&self) -> &'module Function {
        self.function
    }
    pub const fn function_ordinal(&self) -> usize {
        self.function_ordinal
    }

    fn require_live(&self, budget: &mut Budget<'_>) -> Result<(), LocalFrameErrorV1> {
        budget.charge_work(5)?;
        if self.ledger != budget as *const Budget<'_> as usize
            || self.work_ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.floor
        {
            return Err(ResourceError::Accounting.into());
        }
        Ok(())
    }

    /// Pays the exposed row census on the same still-live ledger.
    pub fn allocations(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<&[LocalFrameAllocationV1], LocalFrameErrorV1> {
        self.require_live(budget)?;
        budget.charge_work(self.allocations.len())?;
        Ok(self.allocations)
    }

    pub fn accesses(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<&[LocalFrameAccessV1], LocalFrameErrorV1> {
        self.require_live(budget)?;
        budget.charge_work(self.accesses.len())?;
        Ok(self.accesses)
    }
}

#[derive(Clone, Copy)]
struct PointerCell {
    allocation: usize,
    cell: u64,
    direct_allocation: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct KnownUnsigned {
    ty: ScalarType,
    bits: u64,
}

fn unsigned_width(ty: ScalarType) -> Option<u32> {
    match ty {
        ScalarType::Bool => Some(1),
        ScalarType::U8 => Some(8),
        ScalarType::U16 => Some(16),
        ScalarType::U32 => Some(32),
        // Kernel IR explicitly defines the U64 <-> Index representation bridge.
        // This is not inferred from the analysis host's usize width.
        ScalarType::U64 | ScalarType::Index => Some(64),
        _ => None,
    }
}

fn width_mask(width: u32) -> u64 {
    if width == 64 {
        u64::MAX
    } else {
        (1_u64 << width) - 1
    }
}

fn validate_fact(fact: KnownUnsigned, ty: ScalarType) -> Result<KnownUnsigned, LocalFrameErrorV1> {
    let width = unsigned_width(ty).ok_or(ResourceError::Accounting)?;
    if fact.ty != ty || fact.bits > width_mask(width) {
        return Err(ResourceError::Accounting.into());
    }
    Ok(fact)
}

fn literal_fact(
    constant: &Constant,
    ty: &Type,
) -> Result<Option<KnownUnsigned>, LocalFrameErrorV1> {
    if &constant.ty() != ty {
        return Err(ResourceError::Accounting.into());
    }
    let bits = match constant {
        Constant::Bool(value) => Some(u64::from(*value)),
        Constant::U8(value) => Some(u64::from(*value)),
        Constant::U16(value) => Some(u64::from(*value)),
        Constant::U32(value) => Some(u64::from(*value)),
        Constant::U64(value) | Constant::Index(value) => Some(*value),
        _ => None,
    };
    bits.map(|bits| {
        let ty = ty.as_scalar().ok_or(ResourceError::Accounting)?;
        validate_fact(KnownUnsigned { ty, bits }, ty)
    })
    .transpose()
}

fn checked_cast_fact(
    kind: CastKind,
    from: ScalarType,
    to: ScalarType,
    fact: Option<KnownUnsigned>,
    location: LocalFrameLocationV1,
    budget: &mut Budget<'_>,
) -> Result<Option<KnownUnsigned>, LocalFrameErrorV1> {
    budget.charge_work(8)?;
    let allowed = unsigned_width(from).is_some()
        && unsigned_width(to).is_some()
        && to != ScalarType::Bool
        && valid_scalar_cast(kind, from, to)
        && match kind {
            CastKind::ZeroExtend | CastKind::Truncate => true,
            CastKind::Bitcast => matches!(
                (from, to),
                (ScalarType::U64, ScalarType::Index) | (ScalarType::Index, ScalarType::U64)
            ),
            _ => false,
        };
    if !allowed {
        return Err(refusal(
            location.function_ordinal,
            Some(location.operation),
            LocalFrameRefusalReasonV1::Operation,
        ));
    }
    fact.map(|fact| {
        let fact = validate_fact(fact, from)?;
        let bits = if kind == CastKind::Truncate {
            fact.bits & width_mask(unsigned_width(to).ok_or(ResourceError::Accounting)?)
        } else {
            fact.bits
        };
        validate_fact(KnownUnsigned { ty: to, bits }, to)
    })
    .transpose()
}

struct Definition<'module> {
    value: ValueId,
    ty: &'module Type,
    operation: Option<usize>,
    pointer: Option<PointerCell>,
    constant: Option<KnownUnsigned>,
    scalar_slot: Option<KnownUnsigned>,
}

struct CellVisit {
    allocation: usize,
    cell: u64,
    access: usize,
}

struct Workspace<'module> {
    definitions: Vec<Definition<'module>>,
    allocations: Vec<LocalFrameAllocationV1>,
    accesses: Vec<LocalFrameAccessV1>,
    cells: Vec<CellVisit>,
}

impl Workspace<'_> {
    fn new() -> Self {
        Self {
            definitions: Vec::new(),
            allocations: Vec::new(),
            accesses: Vec::new(),
            cells: Vec::new(),
        }
    }
}

fn refusal(
    function_ordinal: usize,
    operation: Option<usize>,
    reason: LocalFrameRefusalReasonV1,
) -> LocalFrameErrorV1 {
    LocalFrameErrorV1::Unsupported {
        function_ordinal,
        operation,
        reason,
    }
}

fn reserve<T>(
    rows: &mut Vec<T>,
    count: usize,
    budget: &mut Budget<'_>,
) -> Result<(), LocalFrameErrorV1> {
    budget.charge_work(2)?;
    let bytes = count
        .checked_mul(size_of::<T>())
        .ok_or(ResourceError::Arithmetic)?;
    budget.reserve_storage(bytes)?;
    rows.try_reserve_exact(count)
        .map_err(|_| ResourceError::Allocation)?;
    let actual = rows
        .capacity()
        .checked_mul(size_of::<T>())
        .ok_or(ResourceError::Arithmetic)?;
    budget.reserve_storage(actual.checked_sub(bytes).ok_or(ResourceError::Accounting)?)?;
    Ok(())
}

fn find(
    definitions: &[Definition<'_>],
    value: ValueId,
    budget: &mut Budget<'_>,
) -> Result<usize, LocalFrameErrorV1> {
    verification_find_last_by_v1(definitions, 1, budget, |row| row.value.cmp(&value))?
        .ok_or_else(|| ResourceError::Accounting.into())
}

fn constant_index(
    definitions: &[Definition<'_>],
    operations: &[Operation],
    value: ValueId,
    function: usize,
    at: usize,
    sequence: usize,
    budget: &mut Budget<'_>,
) -> Result<u64, LocalFrameErrorV1> {
    let row = &definitions[find(definitions, value, budget)?];
    budget.charge_work(3)?;
    let Some(ordinal) = row.operation.filter(|ordinal| *ordinal < sequence) else {
        return Err(refusal(
            function,
            Some(at),
            LocalFrameRefusalReasonV1::Index,
        ));
    };
    budget.charge_work(2)?;
    if let Some(fact) = row.constant {
        let ty = row.ty.as_scalar().ok_or(ResourceError::Accounting)?;
        let fact = validate_fact(fact, ty)?;
        if ty.is_integer() {
            return Ok(fact.bits);
        }
    }
    let value = match operations.get(ordinal).map(|operation| &operation.kind) {
        Some(OperationKind::Constant(Constant::U8(value))) => Some(u64::from(*value)),
        Some(OperationKind::Constant(Constant::U16(value))) => Some(u64::from(*value)),
        Some(OperationKind::Constant(Constant::U32(value))) => Some(u64::from(*value)),
        Some(OperationKind::Constant(Constant::U64(value) | Constant::Index(value))) => {
            Some(*value)
        }
        Some(OperationKind::Constant(Constant::I8(value))) => u64::try_from(*value).ok(),
        Some(OperationKind::Constant(Constant::I16(value))) => u64::try_from(*value).ok(),
        Some(OperationKind::Constant(Constant::I32(value))) => u64::try_from(*value).ok(),
        Some(OperationKind::Constant(Constant::I64(value))) => u64::try_from(*value).ok(),
        _ => None,
    };
    value.ok_or_else(|| refusal(function, Some(at), LocalFrameRefusalReasonV1::Index))
}

fn audit_effects(
    operation: &Operation,
    function: usize,
    at: usize,
    budget: &mut Budget<'_>,
) -> Result<(), LocalFrameErrorV1> {
    budget.charge_work(3)?;
    if !operation.compiler_ordering_effects_v12().is_empty() {
        return Err(refusal(
            function,
            Some(at),
            LocalFrameRefusalReasonV1::Effects,
        ));
    }
    let expected = match operation.kind {
        OperationKind::Alloca { .. } | OperationKind::Load { .. } | OperationKind::Store { .. } => {
            1
        }
        _ => 0,
    };
    let mut count = 0;
    operation.try_visit_local_memory_effects_v1(|effect| {
        budget.charge_work(1)?;
        let exact = matches!(
            (&operation.kind, effect),
            (
                OperationKind::Alloca { .. },
                KirLocalMemoryEffectRefV1::Allocate(AddressSpace::Private)
            ) | (
                OperationKind::Load { .. },
                KirLocalMemoryEffectRefV1::Read(AddressSpace::Private)
            ) | (
                OperationKind::Store { .. },
                KirLocalMemoryEffectRefV1::Write(AddressSpace::Private)
            )
        );
        if !exact {
            return Err(refusal(
                function,
                Some(at),
                LocalFrameRefusalReasonV1::Effects,
            ));
        }
        count += 1;
        Ok(())
    })?;
    if count != expected {
        return Err(refusal(
            function,
            Some(at),
            LocalFrameRefusalReasonV1::Effects,
        ));
    }
    Ok(())
}

fn derive<'module>(
    module: &'module Module,
    function_ordinal: usize,
    workspace: &mut Workspace<'module>,
    budget: &mut Budget<'_>,
) -> Result<&'module Function, LocalFrameErrorV1> {
    use LocalFrameRefusalReasonV1 as Reason;
    budget.charge_work(8)?;
    let function = module
        .functions
        .get(function_ordinal)
        .ok_or_else(|| refusal(function_ordinal, None, Reason::Function))?;
    let body = function
        .body
        .as_ref()
        .ok_or_else(|| refusal(function_ordinal, None, Reason::Function))?;
    if body.blocks.len() != 1 || !body.blocks[0].parameters.is_empty() {
        return Err(refusal(function_ordinal, None, Reason::ControlFlow));
    }
    let block = &body.blocks[0];
    if !matches!(block.terminator, Some(Terminator::Return { .. })) {
        return Err(refusal(function_ordinal, None, Reason::ControlFlow));
    }
    for ty in function
        .signature
        .parameters
        .iter()
        .chain(&function.signature.results)
    {
        budget.charge_work(1)?;
        if !matches!(ty, Type::Scalar(_) | Type::Unit) {
            return Err(refusal(function_ordinal, None, Reason::Signature));
        }
    }

    let mut definitions = body.parameters.len();
    let mut allocations = 0_usize;
    let mut accesses = 0_usize;
    for operation in &block.operations {
        budget.charge_work(1)?;
        definitions = definitions
            .checked_add(operation.results.len())
            .ok_or(ResourceError::Arithmetic)?;
        allocations = allocations
            .checked_add(usize::from(matches!(
                operation.kind,
                OperationKind::Alloca { .. }
            )))
            .ok_or(ResourceError::Arithmetic)?;
        accesses = accesses
            .checked_add(usize::from(matches!(
                operation.kind,
                OperationKind::Load { .. } | OperationKind::Store { .. }
            )))
            .ok_or(ResourceError::Arithmetic)?;
    }
    reserve(&mut workspace.definitions, definitions, budget)?;
    reserve(&mut workspace.allocations, allocations, budget)?;
    reserve(&mut workspace.accesses, accesses, budget)?;
    reserve(&mut workspace.cells, accesses, budget)?;
    for (value, ty) in body.parameters.iter().zip(&function.signature.parameters) {
        budget.charge_work(3)?;
        workspace.definitions.push(Definition {
            value: *value,
            ty,
            operation: None,
            pointer: None,
            constant: None,
            scalar_slot: None,
        });
    }
    for (ordinal, operation) in block.operations.iter().enumerate() {
        budget.charge_work(1)?;
        for result in &operation.results {
            budget.charge_work(3)?;
            workspace.definitions.push(Definition {
                value: result.id,
                ty: &result.ty,
                operation: Some(ordinal),
                pointer: None,
                constant: None,
                scalar_slot: None,
            });
        }
    }
    verification_bounded_sort_by_v1(&mut workspace.definitions, 1, budget, |left, right| {
        left.value.cmp(&right.value)
    })?;
    for pair in workspace.definitions.windows(2) {
        budget.charge_work(1)?;
        if pair[0].value >= pair[1].value {
            return Err(refusal(function_ordinal, None, Reason::Definition));
        }
    }

    derive_block(function_ordinal, block, 0, false, workspace, budget)?;
    audit_return(function_ordinal, block, workspace, budget)?;
    finish_cells(function_ordinal, workspace, budget)?;
    Ok(function)
}

fn derive_block(
    function_ordinal: usize,
    block: &BasicBlock,
    start: usize,
    checked_control: bool,
    workspace: &mut Workspace<'_>,
    budget: &mut Budget<'_>,
) -> Result<(), LocalFrameErrorV1> {
    use LocalFrameRefusalReasonV1 as Reason;
    for (at, operation) in block.operations.iter().enumerate() {
        budget.charge_work(4)?;
        let sequence = start.checked_add(at).ok_or(ResourceError::Arithmetic)?;
        match operation.kind {
            OperationKind::Constant(_)
            | OperationKind::Unary { .. }
            | OperationKind::Binary { .. }
            | OperationKind::Compare { .. }
            | OperationKind::Select { .. }
            | OperationKind::Alloca { .. }
            | OperationKind::GetElementPointer { .. }
            | OperationKind::Load { .. }
            | OperationKind::Store { .. } => {}
            OperationKind::Cast {
                kind: CastKind::ZeroExtend | CastKind::Truncate | CastKind::Bitcast,
                ..
            } => {}
            _ => return Err(refusal(function_ordinal, Some(at), Reason::Operation)),
        }
        audit_effects(operation, function_ordinal, at, budget)?;
        let mut position = 0;
        operation.kind.try_visit_operands(|value| {
            budget.charge_work(2)?;
            let row = &workspace.definitions[find(&workspace.definitions, value, budget)?];
            if row
                .operation
                .is_some_and(|definition| definition >= sequence)
            {
                return Err(refusal(function_ordinal, Some(at), Reason::Definition));
            }
            if matches!(row.ty, Type::Pointer(_)) {
                let allowed = position == 0
                    && matches!(
                        operation.kind,
                        OperationKind::GetElementPointer { .. }
                            | OperationKind::Load { .. }
                            | OperationKind::Store { .. }
                    );
                if !allowed || row.pointer.is_none() {
                    return Err(refusal(function_ordinal, Some(at), Reason::PointerUse));
                }
            } else if !matches!(row.ty, Type::Scalar(_) | Type::Unit) {
                return Err(refusal(function_ordinal, Some(at), Reason::PointerUse));
            }
            position += 1;
            Ok(())
        })?;
        for result in &operation.results {
            budget.charge_work(1)?;
            let allowed_pointer = matches!(
                operation.kind,
                OperationKind::Alloca { .. } | OperationKind::GetElementPointer { .. }
            );
            if !matches!(result.ty, Type::Scalar(_) | Type::Unit)
                && !(allowed_pointer && matches!(result.ty, Type::Pointer(_)))
            {
                return Err(refusal(function_ordinal, Some(at), Reason::PointerUse));
            }
        }
        let location = LocalFrameLocationV1 {
            function_ordinal,
            block: block.id,
            operation: at,
        };
        match &operation.kind {
            OperationKind::Constant(constant) => {
                budget.charge_work(3)?;
                let definition = find(&workspace.definitions, operation.results[0].id, budget)?;
                workspace.definitions[definition].constant =
                    literal_fact(constant, &operation.results[0].ty)?;
            }
            OperationKind::Cast { kind, value, to } => {
                budget.charge_work(2)?;
                let source = &workspace.definitions[find(&workspace.definitions, *value, budget)?];
                let (Some(from), Some(target)) = (source.ty.as_scalar(), to.as_scalar()) else {
                    return Err(refusal(function_ordinal, Some(at), Reason::Operation));
                };
                if &operation.results[0].ty != to {
                    return Err(ResourceError::Accounting.into());
                }
                let fact =
                    checked_cast_fact(*kind, from, target, source.constant, location, budget)?;
                let result = find(&workspace.definitions, operation.results[0].id, budget)?;
                workspace.definitions[result].constant = fact;
            }
            OperationKind::Compare {
                predicate,
                lhs,
                rhs,
            } if checked_control => {
                budget.charge_work(12)?;
                let lhs = find(&workspace.definitions, *lhs, budget)?;
                let rhs = find(&workspace.definitions, *rhs, budget)?;
                let fact = chain_v1::checked_compare_fact(
                    *predicate,
                    &workspace.definitions[lhs],
                    &workspace.definitions[rhs],
                    &operation.results[0].ty,
                )?;
                let result = find(&workspace.definitions, operation.results[0].id, budget)?;
                workspace.definitions[result].constant = fact;
            }
            OperationKind::Alloca {
                element,
                count,
                alignment,
                address_space,
            } => {
                budget.charge_work(8)?;
                let Type::Scalar(element) = element else {
                    return Err(refusal(function_ordinal, Some(at), Reason::Allocation));
                };
                let width = element
                    .bit_width()
                    .map(|bits| u64::from(bits.div_ceil(8)))
                    .ok_or_else(|| refusal(function_ordinal, Some(at), Reason::Allocation))?;
                let count = match count {
                    Some(count) => constant_index(
                        &workspace.definitions,
                        if checked_control {
                            &[]
                        } else {
                            &block.operations
                        },
                        *count,
                        function_ordinal,
                        at,
                        sequence,
                        budget,
                    )?,
                    None => 1,
                };
                let byte_extent = count
                    .checked_mul(width)
                    .filter(|_| count != 0)
                    .ok_or_else(|| refusal(function_ordinal, Some(at), Reason::Allocation))?;
                if *address_space != AddressSpace::Private || u64::from(*alignment) < width {
                    return Err(refusal(function_ordinal, Some(at), Reason::Allocation));
                }
                let pointer = operation.results[0].id;
                let definition = find(&workspace.definitions, pointer, budget)?;
                let allocation = workspace.allocations.len();
                workspace.allocations.push(LocalFrameAllocationV1 {
                    location,
                    pointer,
                    element: *element,
                    element_bytes: width,
                    count,
                    byte_extent,
                    alignment: *alignment,
                });
                workspace.definitions[definition].pointer = Some(PointerCell {
                    allocation,
                    cell: 0,
                    direct_allocation: true,
                });
            }
            OperationKind::GetElementPointer { base, offset } => {
                budget.charge_work(5)?;
                let origin = workspace.definitions[find(&workspace.definitions, *base, budget)?]
                    .pointer
                    .ok_or_else(|| refusal(function_ordinal, Some(at), Reason::PointerUse))?;
                if !origin.direct_allocation {
                    return Err(refusal(function_ordinal, Some(at), Reason::PointerUse));
                }
                let cell = constant_index(
                    &workspace.definitions,
                    if checked_control {
                        &[]
                    } else {
                        &block.operations
                    },
                    *offset,
                    function_ordinal,
                    at,
                    sequence,
                    budget,
                )?;
                if cell >= workspace.allocations[origin.allocation].count {
                    return Err(refusal(function_ordinal, Some(at), Reason::Index));
                }
                let result = find(&workspace.definitions, operation.results[0].id, budget)?;
                workspace.definitions[result].pointer = Some(PointerCell {
                    allocation: origin.allocation,
                    cell,
                    direct_allocation: false,
                });
            }
            OperationKind::Load { pointer, access }
            | OperationKind::Store {
                pointer, access, ..
            } => {
                budget.charge_work(8)?;
                let origin = workspace.definitions[find(&workspace.definitions, *pointer, budget)?]
                    .pointer
                    .ok_or_else(|| refusal(function_ordinal, Some(at), Reason::PointerUse))?;
                let allocation = &workspace.allocations[origin.allocation];
                let byte_offset = origin
                    .cell
                    .checked_mul(allocation.element_bytes)
                    .ok_or(ResourceError::Arithmetic)?;
                if access.volatile || access.address_space != AddressSpace::Private {
                    return Err(refusal(function_ordinal, Some(at), Reason::Effects));
                }
                if access.alignment > allocation.alignment
                    || byte_offset % u64::from(access.alignment) != 0
                {
                    return Err(refusal(function_ordinal, Some(at), Reason::Alignment));
                }
                let (kind, value) = match operation.kind {
                    OperationKind::Load { .. } => {
                        (LocalFrameAccessKindV1::Read, operation.results[0].id)
                    }
                    OperationKind::Store { value, .. } => (LocalFrameAccessKindV1::Write, value),
                    _ => unreachable!(),
                };
                // Only a one-element allocation has a scalar-value transport
                // slot. Every Store, including an unknown value, replaces it.
                budget.charge_work(4)?;
                if allocation.count == 1 && origin.cell == 0 {
                    let allocation_pointer = allocation.pointer;
                    let element = allocation.element;
                    let scalar = find(&workspace.definitions, value, budget)?;
                    let slot = find(&workspace.definitions, allocation_pointer, budget)?;
                    budget.charge_work(2)?;
                    if workspace.definitions[scalar].ty.as_scalar() != Some(element) {
                        return Err(ResourceError::Accounting.into());
                    }
                    match kind {
                        LocalFrameAccessKindV1::Write => {
                            let fact = workspace.definitions[scalar]
                                .constant
                                .map(|fact| validate_fact(fact, element))
                                .transpose()?;
                            workspace.definitions[slot].scalar_slot = fact;
                        }
                        LocalFrameAccessKindV1::Read => {
                            let fact = workspace.definitions[slot]
                                .scalar_slot
                                .map(|fact| validate_fact(fact, element))
                                .transpose()?;
                            workspace.definitions[scalar].constant = fact;
                        }
                    }
                }
                let ordinal = workspace.accesses.len();
                workspace.accesses.push(LocalFrameAccessV1 {
                    location,
                    allocation: origin.allocation,
                    cell: origin.cell,
                    pointer: *pointer,
                    value,
                    kind,
                    initializing_store: None,
                });
                workspace.cells.push(CellVisit {
                    allocation: origin.allocation,
                    cell: origin.cell,
                    access: ordinal,
                });
            }
            _ => {}
        }
    }
    Ok(())
}

fn audit_return(
    function_ordinal: usize,
    block: &BasicBlock,
    workspace: &Workspace<'_>,
    budget: &mut Budget<'_>,
) -> Result<(), LocalFrameErrorV1> {
    budget.charge_work(1)?;
    block
        .terminator
        .as_ref()
        .ok_or(ResourceError::Accounting)?
        .try_visit_operands(|value| {
            budget.charge_work(1)?;
            let row = &workspace.definitions[find(&workspace.definitions, value, budget)?];
            if !matches!(row.ty, Type::Scalar(_) | Type::Unit) {
                return Err(refusal(
                    function_ordinal,
                    None,
                    LocalFrameRefusalReasonV1::PointerUse,
                ));
            }
            Ok(())
        })?;
    Ok(())
}

fn finish_cells(
    function_ordinal: usize,
    workspace: &mut Workspace<'_>,
    budget: &mut Budget<'_>,
) -> Result<(), LocalFrameErrorV1> {
    verification_bounded_sort_by_v1(&mut workspace.cells, 3, budget, |left, right| {
        (left.allocation, left.cell, left.access).cmp(&(right.allocation, right.cell, right.access))
    })?;
    let mut previous = None;
    let mut last_store = None;
    for cell in &workspace.cells {
        budget.charge_work(5)?;
        let key = (cell.allocation, cell.cell);
        if previous != Some(key) {
            last_store = None;
            previous = Some(key);
        }
        let access = &mut workspace.accesses[cell.access];
        match access.kind {
            LocalFrameAccessKindV1::Write => last_store = Some(access.location),
            LocalFrameAccessKindV1::Read => {
                if last_store.is_none() {
                    return Err(refusal(
                        function_ordinal,
                        Some(access.location.operation),
                        LocalFrameRefusalReasonV1::UninitializedRead,
                    ));
                }
                access.initializing_store = last_store;
            }
        }
    }
    Ok(())
}

/// Classifies one verified function without changing raw summaries or callers.
///
/// The caller retains its graph-owner reservation. Scratch and result rows use
/// this same ledger. Storage units are bytes of these local headers and actual
/// Vec capacities, not RSS, IR graph bytes, or target private-memory capacity.
/// Scope cleanup overrides callback errors/panics only on accounting failure.
pub fn with_checked_local_frame_function_v1<'module>(
    verified: VerifiedKernelIrModuleV1<'module>,
    function_ordinal: usize,
    budget: &mut Budget<'_>,
    next: impl for<'scope> FnOnce(
        CheckedLocalFrameV1<'scope, 'module>,
        &mut Budget<'_>,
    ) -> Result<(), LocalFrameErrorV1>,
) -> Result<(), LocalFrameErrorV1> {
    let incoming = budget.storage();
    let ledger = budget as *const Budget<'_> as usize;
    let headers = size_of::<Workspace<'_>>()
        .checked_add(size_of::<CheckedLocalFrameV1<'_, '_>>())
        .ok_or(ResourceError::Arithmetic)?;
    budget.charge_work(4)?;
    let work_ledger = budget.work_ledger_identity_v1();
    budget.reserve_storage(headers)?;
    let mut workspace = Workspace::new();
    let mut retained_floor = None;
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let function = derive(verified.module(), function_ordinal, &mut workspace, budget)?;
        retained_floor = Some(budget.storage());
        let checked = CheckedLocalFrameV1 {
            module: verified.module(),
            function,
            function_ordinal,
            allocations: &workspace.allocations,
            accesses: &workspace.accesses,
            ledger,
            work_ledger,
            floor: budget.storage(),
        };
        next(checked, budget)
    }));
    if budget.work_ledger_identity_v1() != work_ledger {
        drop(workspace);
        return Err(ResourceError::Accounting.into());
    }
    let accounting = budget.storage() < retained_floor.unwrap_or(incoming + headers);
    drop(workspace);
    let cleanup = budget.rollback_storage(incoming);
    if accounting {
        return Err(ResourceError::Accounting.into());
    }
    cleanup?;
    match outcome {
        Ok(result) => result,
        Err(payload) => resume_unwind(payload),
    }
}

#[cfg(test)]
#[path = "local_frame_effects_v1_tests.rs"]
mod tests;
