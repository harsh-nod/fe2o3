//! Checked, block-local forwarding from an ordinary private Store. This rule
//! never deletes a Store, speculates an access, or changes control flow.

use crate::{
    CanonicalKirInventoryV1 as Inventory, CanonicalKirMemorySsaErrorV1,
    CanonicalKirMemorySsaNodeIdV1 as NodeId, CanonicalKirMemorySsaNodeV1 as Node,
    CanonicalKirMemorySsaV1 as MemorySsa,
};
use fe2o3_kernel_ir::{
    AddressSpace, BinaryOp, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirOperationCoordinateV1 as Coordinate, MemoryAccess, Module, Operation,
    OperationKind as Kind, ScalarType, Type, UnaryOp, ValueId,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use std::{fmt, mem::size_of};

/// Inert replay coordinates. A row alone does not establish any equality.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirStoreForwardingRowV1 {
    pub store: Coordinate,
    pub load: Coordinate,
}
type Row = CanonicalKirStoreForwardingRowV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirStoreForwardingErrorV1 {
    Resource(Resource),
    MemorySsa(CanonicalKirMemorySsaErrorV1),
    ForeignSubject,
    StaleCandidate,
    Rule(&'static str),
}
type Error = CanonicalKirStoreForwardingErrorV1;
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl From<CanonicalKirMemorySsaErrorV1> for Error {
    fn from(error: CanonicalKirMemorySsaErrorV1) -> Self {
        Self::MemorySsa(error)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "store forwarding: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Actual row capacity plus the live wrapper header. Borrowed inputs excluded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirStoreForwardingStorageV1(usize);
impl CanonicalKirStoreForwardingStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}
use self::CanonicalKirStoreForwardingStorageV1 as Storage;

/// A move-only proposal tied to the actual immutable inventory and MemorySSA.
/// It is not a general memory-safety, alias, or transformation permission.
///
/// ```compile_fail
/// use fe2o3_kernel_analysis::{CanonicalKirStoreForwardingPlanV1, CanonicalKirMemorySsaV1};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn detach(memory: CanonicalKirMemorySsaV1<'_, '_>, budget: &mut Budget<'_>) {
///     let (plan, _) = CanonicalKirStoreForwardingPlanV1::derive(memory.inventory(), &memory, budget).unwrap();
///     drop(memory);
///     let _ = plan.rows();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_kernel_analysis::CanonicalKirStoreForwardingPlanV1;
/// use fe2o3_kernel_ir::{Module, VerifiedCanonicalKernelIrModuleV12 as Owner,
///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// fn reuse<'g>(plan: CanonicalKirStoreForwardingPlanV1<'_, '_, 'g>, owner: &'g Owner,
///     candidate: &mut Module, budget: &mut Budget<'_>) {
///     let _ = plan.apply(owner, candidate, budget);
///     let _ = plan.apply(owner, candidate, budget);
/// }
/// ```
pub struct CanonicalKirStoreForwardingPlanV1<'m, 'i, 'g> {
    memory: &'m MemorySsa<'i, 'g>,
    rows: Vec<Row>,
    retained: usize,
}

/// Evidence of this consumed mutation session; replay still independently checks
/// the actual output. This cannot be constructed from inert coordinate rows.
pub struct CanonicalKirAppliedStoreForwardingV1<'g> {
    input: &'g Owner,
    rows: Vec<Row>,
    retained: usize,
}
impl<'g> CanonicalKirAppliedStoreForwardingV1<'g> {
    pub const fn input(&self) -> &'g Owner {
        self.input
    }
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    pub fn check_output<'a>(
        &'a self,
        output: &'a Owner,
        budget: &mut Budget<'_>,
    ) -> Result<(CheckedCanonicalKirStoreForwardingV1<'a>, Storage)> {
        check_canonical_kir_store_forwarding_v1(self.input, output, &self.rows, budget)
    }
}

#[derive(Clone, Copy)]
struct Store {
    coordinate: Coordinate,
    pointer: ValueId,
    value: ValueId,
    access: MemoryAccess,
}

fn store(operation: &Operation, coordinate: Coordinate) -> Option<Store> {
    match operation.kind {
        Kind::Store {
            pointer,
            value,
            access,
        } if access.address_space == AddressSpace::Private
            && !access.volatile
            && operation.results.is_empty() =>
        {
            Some(Store {
                coordinate,
                pointer,
                value,
                access,
            })
        }
        _ => None,
    }
}
fn integer(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Scalar(
            ScalarType::I8
                | ScalarType::I16
                | ScalarType::I32
                | ScalarType::I64
                | ScalarType::U8
                | ScalarType::U16
                | ScalarType::U32
                | ScalarType::U64
        )
    )
}
fn matching_load(operation: &Operation, prior: Store) -> bool {
    matches!(operation.kind, Kind::Load { pointer, access }
        if pointer == prior.pointer && access == prior.access)
        && operation.results.len() == 1
        && integer(&operation.results[0].ty)
}
// Closed total recipes only. In particular no division, intrinsic, pointer
// arithmetic, allocation, call, contract, guard, or convergence operation.
fn transparent(operation: &Operation) -> bool {
    if operation.results.len() != 1 {
        return false;
    }
    let fixed = integer(&operation.results[0].ty) || operation.results[0].ty == Type::BOOL;
    fixed
        && matches!(
            operation.kind,
            Kind::Constant(_)
                | Kind::Unary {
                    op: UnaryOp::Not,
                    ..
                }
                | Kind::Binary {
                    op: BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor,
                    ..
                }
                | Kind::Select { .. }
        )
}
fn replacement(prior: Store) -> Kind {
    Kind::Binary {
        op: BinaryOp::BitOr,
        lhs: prior.value,
        rhs: prior.value,
    }
}

impl<'m, 'i, 'g> CanonicalKirStoreForwardingPlanV1<'m, 'i, 'g> {
    /// Linear census, one bounded row buffer, no sorting or alias search. Every
    /// private store resets the seed; every unrecognized operation clears it.
    /// A candidate additionally requires the exact Store Def as its MemorySSA
    /// incoming version. Each block/function begins without a seed.
    ///
    /// Reserve the returned receipt while the plan lives. All Result exits
    /// restore the incoming floor without rewinding work/peak/failure history.
    pub fn derive(
        inventory: &'i Inventory<'g>,
        memory: &'m MemorySsa<'i, 'g>,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        let floor = budget.storage();
        let result = Self::build(inventory, memory, budget);
        budget.release_storage(
            budget
                .storage()
                .checked_sub(floor)
                .ok_or(Resource::Accounting)?,
        )?;
        result.map(|plan| {
            let storage = Storage(plan.retained);
            (plan, storage)
        })
    }
    fn build(
        inventory: &'i Inventory<'g>,
        memory: &'m MemorySsa<'i, 'g>,
        budget: &mut Budget<'_>,
    ) -> Result<Self> {
        budget.charge_work(3)?;
        if !memory.belongs_to(inventory) {
            return Err(Error::ForeignSubject);
        }
        budget.reserve_storage(size_of::<Self>())?;
        budget.charge_work(3)?;
        let requested = inventory
            .operations()
            .len()
            .checked_mul(size_of::<Row>())
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(requested)?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(inventory.operations().len())
            .map_err(|_| Resource::Allocation)?;
        let actual = rows
            .capacity()
            .checked_mul(size_of::<Row>())
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(actual.checked_sub(requested).ok_or(Resource::Accounting)?)?;
        for block in inventory.blocks() {
            budget.charge_work(1)?;
            let mut prior: Option<(Store, NodeId)> = None;
            for index in block.operations.clone() {
                budget.charge_work(5)?;
                let operation = &inventory.operations()[index];
                if let Some(seed) = store(operation.operation, operation.coordinate) {
                    let node = memory
                        .operation(operation.coordinate, budget)?
                        .ok_or(Error::Rule("Store lacks MemorySSA Def"))?;
                    if !matches!(memory.node(node, budget)?, Node::Def { operation: coordinate, .. }
                        if *coordinate == operation.coordinate)
                    {
                        return Err(Error::Rule("Store MemorySSA identity"));
                    }
                    prior = Some((seed, node));
                } else if let Some((seed, state)) =
                    prior.filter(|(seed, _)| matching_load(operation.operation, *seed))
                {
                    let node = memory
                        .operation(operation.coordinate, budget)?
                        .ok_or(Error::Rule("Load lacks MemorySSA Use"))?;
                    if !matches!(memory.node(node, budget)?, Node::Use { operation: coordinate, incoming }
                        if *coordinate == operation.coordinate && *incoming == state)
                    {
                        return Err(Error::Rule("Load MemorySSA version"));
                    }
                    budget.charge_work(1)?;
                    if rows.len() == rows.capacity() {
                        return Err(Resource::Accounting.into());
                    }
                    rows.push(Row {
                        store: seed.coordinate,
                        load: operation.coordinate,
                    });
                } else if !transparent(operation.operation) {
                    prior = None;
                }
            }
        }
        budget.charge_work(2)?;
        let retained = size_of::<Self>()
            .checked_add(actual)
            .ok_or(Resource::Arithmetic)?;
        Ok(Self {
            memory,
            rows,
            retained,
        })
    }
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// Consumes the exact plan. A foreign owner or changed candidate refuses
    /// before mutation; all mutation work is prepaid before the first write.
    /// No allocation or fallible operation occurs after that boundary. The
    /// caller retains its candidate and plan reservations until they are dropped.
    pub fn apply(
        self,
        input: &'g Owner,
        candidate: &mut Module,
        budget: &mut Budget<'_>,
    ) -> Result<CanonicalKirAppliedStoreForwardingV1<'g>> {
        budget.charge_work(3)?;
        if !std::ptr::eq(input, self.memory.inventory().owner()) {
            return Err(Error::ForeignSubject);
        }
        budget.charge_work(input.canonical().canonical_bytes().len())?;
        if &*candidate != input.module() {
            return Err(Error::StaleCandidate);
        }
        if size_of::<Self>() != size_of::<CanonicalKirAppliedStoreForwardingV1<'_>>() {
            return Err(Resource::Accounting.into());
        }
        budget.charge_work(self.rows.len())?;
        for row in &self.rows {
            let block = &mut candidate.functions[row.load.block.function.0 as usize]
                .body
                .as_mut()
                .expect("verified source body")
                .blocks[row.load.block.block as usize];
            let seed = store(&block.operations[row.store.operation as usize], row.store)
                .expect("immutable planned Store is never rewritten");
            block.operations[row.load.operation as usize].kind = replacement(seed);
        }
        Ok(CanonicalKirAppliedStoreForwardingV1 {
            input,
            rows: self.rows,
            retained: self.retained,
        })
    }
}

/// Exact actual input/output relation for this local rule, not Policy3 execution,
/// general refinement, formal-memory completeness, or target admission.
pub struct CheckedCanonicalKirStoreForwardingV1<'a> {
    input: &'a Owner,
    output: &'a Owner,
    rows: &'a [Row],
}
impl<'a> CheckedCanonicalKirStoreForwardingV1<'a> {
    pub const fn input(&self) -> &'a Owner {
        self.input
    }
    pub const fn output(&self) -> &'a Owner {
        self.output
    }
    pub const fn rows(&self) -> &'a [Row] {
        self.rows
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Independent linear replay over both actual verified owners. It does not
/// trust MemorySSA or producer decisions: each row re-establishes an earlier
/// unguarded Store to the identical private pointer/access, a fixed integer
/// pointee, and a closed total interval. The core verifier establishes the full
/// Store-value/pointee/Load-result type equality and SSA availability. Every
/// unmentioned operation, declaration, result, terminator and CFG edge is exact.
/// The successful Store is preserved, so forwarding neither introduces an
/// access nor hides a possible first-access failure or uninitialized read.
pub fn check_canonical_kir_store_forwarding_v1<'a>(
    input: &'a Owner,
    output: &'a Owner,
    rows: &'a [Row],
    budget: &mut Budget<'_>,
) -> Result<(CheckedCanonicalKirStoreForwardingV1<'a>, Storage)> {
    let floor = budget.storage();
    let result = (|| {
        budget.charge_work(3)?;
        let bytes = input
            .canonical()
            .canonical_bytes()
            .len()
            .checked_add(output.canonical().canonical_bytes().len())
            .ok_or(Resource::Arithmetic)?;
        budget.charge_work(bytes)?;
        budget.reserve_storage(size_of::<CheckedCanonicalKirStoreForwardingV1<'_>>())?;
        check_modules(input.module(), output.module(), rows, budget)?;
        Ok(CheckedCanonicalKirStoreForwardingV1 {
            input,
            output,
            rows,
        })
    })();
    budget.release_storage(
        budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)?,
    )?;
    result.map(|checked| {
        (
            checked,
            Storage(size_of::<CheckedCanonicalKirStoreForwardingV1<'_>>()),
        )
    })
}

fn check_modules(
    input: &Module,
    output: &Module,
    rows: &[Row],
    budget: &mut Budget<'_>,
) -> Result<()> {
    // Exhaustive destructuring makes additions to semantic headers a compiler
    // error rather than silently excluding them from the independent replay.
    let Module {
        id,
        functions,
        kernels,
        required_capabilities,
    } = input;
    if id != &output.id
        || kernels != &output.kernels
        || required_capabilities != &output.required_capabilities
        || functions.len() != output.functions.len()
    {
        return Err(Error::Rule("module payload"));
    }
    let mut next = 0usize;
    for (f, (a, b)) in functions.iter().zip(&output.functions).enumerate() {
        budget.charge_work(1)?;
        let fe2o3_kernel_ir::Function {
            id,
            signature,
            role,
            body,
            required_capabilities,
        } = a;
        if id != &b.id
            || signature != &b.signature
            || role != &b.role
            || required_capabilities != &b.required_capabilities
        {
            return Err(Error::Rule("function payload"));
        }
        let (a, b) = match (body, &b.body) {
            (Some(a), Some(b)) => (a, b),
            (None, None) => continue,
            _ => return Err(Error::Rule("function body")),
        };
        let fe2o3_kernel_ir::FunctionBody { parameters, blocks } = a;
        if parameters != &b.parameters || blocks.len() != b.blocks.len() {
            return Err(Error::Rule("function body payload"));
        }
        for (k, (a, b)) in blocks.iter().zip(&b.blocks).enumerate() {
            budget.charge_work(1)?;
            let fe2o3_kernel_ir::BasicBlock {
                id,
                parameters,
                operations,
                terminator,
            } = a;
            if id != &b.id
                || parameters != &b.parameters
                || terminator != &b.terminator
                || operations.len() != b.operations.len()
            {
                return Err(Error::Rule("block payload"));
            }
            let mut prior: Option<Store> = None;
            for (o, (a, b)) in operations.iter().zip(&b.operations).enumerate() {
                budget.charge_work(5)?;
                let coordinate = Coordinate {
                    block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
                        function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
                            u32::try_from(f).map_err(|_| Resource::Arithmetic)?,
                        ),
                        block: u32::try_from(k).map_err(|_| Resource::Arithmetic)?,
                    },
                    operation: u32::try_from(o).map_err(|_| Resource::Arithmetic)?,
                };
                if let Some(row) = rows.get(next).filter(|row| row.load == coordinate) {
                    budget.charge_work(1)?;
                    let seed = prior
                        .filter(|seed| seed.coordinate == row.store && matching_load(a, *seed))
                        .ok_or(Error::Rule("Store/Load provenance or barrier"))?;
                    if a.results != b.results || b.kind != replacement(seed) {
                        return Err(Error::Rule("forwarded value or type"));
                    }
                    next += 1;
                } else if a != b {
                    return Err(Error::Rule("unrecorded mutation"));
                }
                if let Some(seed) = store(a, coordinate) {
                    prior = Some(seed);
                } else if prior.is_some_and(|seed| matching_load(a, seed)) {
                } else if !transparent(a) {
                    prior = None;
                }
            }
        }
    }
    if next != rows.len() {
        return Err(Error::Rule("unordered, duplicate, or absent row"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "canonical_kir_store_forwarding_v1_tests.rs"]
mod tests;
