//! Checked same-block private Load-to-Load forwarding. The first ordinary load
//! remains observable; no speculation, motion, global-memory or alias rule.

use crate::{
    CanonicalKirInventoryErrorV1, CanonicalKirInventoryV1 as Inventory,
    CanonicalKirMemorySsaErrorV1, CanonicalKirMemorySsaNodeIdV1 as NodeId,
    CanonicalKirMemorySsaNodeV1 as Node, CanonicalKirMemorySsaV1 as MemorySsa,
};
use fe2o3_kernel_ir::{
    AddressSpace, BinaryOp, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirOperationCoordinateV1 as Coordinate, MemoryAccess, Module, Operation,
    OperationKind as Kind, ScalarType, Type, UnaryOp, ValueId,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use std::{fmt, mem::size_of};

#[path = "canonical_kir_load_forwarding_initialization_v1.rs"]
mod initialization;
use initialization::InitializedPrivateLoadsV1;

/// Inert ordered coordinates, not memory equality or rewrite authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirLoadForwardingRowV1 {
    pub first: Coordinate,
    pub load: Coordinate,
}
type Row = CanonicalKirLoadForwardingRowV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirLoadForwardingErrorV1 {
    Resource(Resource),
    Inventory(CanonicalKirInventoryErrorV1),
    MemorySsa(CanonicalKirMemorySsaErrorV1),
    ForeignSubject,
    StaleCandidate,
    Rule(&'static str),
}
type Error = CanonicalKirLoadForwardingErrorV1;
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
        write!(f, "load forwarding: {self:?}")
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Inventory(error) => Some(error),
            Self::MemorySsa(error) => Some(error),
            Self::ForeignSubject | Self::StaleCandidate | Self::Rule(_) => None,
        }
    }
}

/// Live header and actual row capacity; borrowed owners/analyses excluded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirLoadForwardingStorageV1(usize);
impl CanonicalKirLoadForwardingStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}
use self::CanonicalKirLoadForwardingStorageV1 as Storage;

/// Move-only proposal borrowing this exact immutable inventory and MemorySSA.
///
/// ```compile_fail
/// use fe2o3_kernel_analysis::{CanonicalKirLoadForwardingPlanV1, CanonicalKirMemorySsaV1};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn detach(memory: CanonicalKirMemorySsaV1<'_, '_>, budget: &mut Budget<'_>) {
///     let (plan, _) = CanonicalKirLoadForwardingPlanV1::derive(memory.inventory(), &memory, budget).unwrap();
///     drop(memory);
///     let _ = plan.rows();
/// }
/// ```
pub struct CanonicalKirLoadForwardingPlanV1<'m, 'i, 'g> {
    memory: &'m MemorySsa<'i, 'g>,
    rows: Vec<Row>,
    retained: usize,
}

/// Consumed producer execution, not independently checked output authority.
pub struct CanonicalKirAppliedLoadForwardingV1<'g> {
    input: &'g Owner,
    rows: Vec<Row>,
    retained: usize,
}
impl<'g> CanonicalKirAppliedLoadForwardingV1<'g> {
    pub const fn input(&self) -> &'g Owner {
        self.input
    }
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    /// Transfers the existing allocation into explicitly inert claims.
    pub fn into_inert_rows(self) -> Vec<Row> {
        self.rows
    }
    pub fn check_output<'a>(
        &'a self,
        output: &'a Owner,
        budget: &mut Budget<'_>,
    ) -> Result<(CheckedCanonicalKirLoadForwardingV1<'a>, Storage)> {
        check_canonical_kir_load_forwarding_v1(self.input, output, &self.rows, budget)
    }
}

#[derive(Clone, Copy)]
struct Load {
    coordinate: Coordinate,
    pointer: ValueId,
    value: ValueId,
    ty: ScalarType,
    access: MemoryAccess,
}
fn integer(ty: ScalarType) -> bool {
    matches!(
        ty,
        ScalarType::I8
            | ScalarType::I16
            | ScalarType::I32
            | ScalarType::I64
            | ScalarType::U8
            | ScalarType::U16
            | ScalarType::U32
            | ScalarType::U64
    )
}
fn load(operation: &Operation, coordinate: Coordinate) -> Option<Load> {
    let Kind::Load { pointer, access } = operation.kind else {
        return None;
    };
    let [result] = operation.results.as_slice() else {
        return None;
    };
    let Type::Scalar(ty) = result.ty else {
        return None;
    };
    if access.address_space != AddressSpace::Private || access.volatile || !integer(ty) {
        return None;
    }
    Some(Load {
        coordinate,
        pointer,
        value: result.id,
        ty,
        access,
    })
}
fn matching_load(operation: &Operation, prior: Load) -> bool {
    load(operation, prior.coordinate).is_some_and(|current| {
        current.pointer == prior.pointer && current.access == prior.access && current.ty == prior.ty
    })
}
// Closed total recipes only. All other loads, stores, atomics, calls, assembly,
// casts, arithmetic with partial semantics, contracts and convergence are fences.
fn transparent(operation: &Operation) -> bool {
    let [result] = operation.results.as_slice() else {
        return false;
    };
    let Type::Scalar(ty) = result.ty else {
        return false;
    };
    (integer(ty) || ty == ScalarType::Bool)
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
fn replacement(prior: Load) -> Kind {
    Kind::Binary {
        op: BinaryOp::BitOr,
        lhs: prior.value,
        rhs: prior.value,
    }
}
fn incoming(
    memory: &MemorySsa<'_, '_>,
    coordinate: Coordinate,
    budget: &mut Budget<'_>,
) -> Result<NodeId> {
    let node = memory
        .operation(coordinate, budget)?
        .ok_or(Error::Rule("Load lacks MemorySSA Use"))?;
    match memory.node(node, budget)? {
        Node::Use {
            operation,
            incoming,
        } if *operation == coordinate => Ok(*incoming),
        _ => Err(Error::Rule("exact Load MemorySSA identity")),
    }
}

impl<'m, 'i, 'g> CanonicalKirLoadForwardingPlanV1<'m, 'i, 'g> {
    /// Linear census and one actual-capacity row buffer. A block begins without
    /// a seed. An eligible different Load replaces, rather than crosses, it.
    /// A separate complete use census requires a direct, nonescaping single
    /// integer Alloca and an aligned same-block initializing Store. No arbitrary
    /// private parameter or uninitialized first read can establish a seed.
    /// A candidate requires the same incoming MemorySSA version as its first
    /// preserved Load. No allocation identity is inferred from pointer equality.
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
            let receipt = Storage(plan.retained);
            (plan, receipt)
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
        let (mut initialized, initialized_storage) =
            InitializedPrivateLoadsV1::derive(inventory, budget)?;
        budget.reserve_storage(initialized_storage)?;
        for block in inventory.blocks() {
            budget.charge_work(1)?;
            initialized.enter_block();
            let mut prior: Option<(Load, NodeId)> = None;
            for index in block.operations.clone() {
                budget.charge_work(5)?;
                let row = &inventory.operations()[index];
                if let Some(current) = initialized.observe(index, budget)? {
                    let state = incoming(memory, row.coordinate, budget)?;
                    if let Some((first, version)) =
                        prior.filter(|(first, _)| matching_load(row.operation, *first))
                    {
                        if state != version {
                            return Err(Error::Rule("Load MemorySSA version"));
                        }
                        budget.charge_work(1)?;
                        if rows.len() == rows.capacity() {
                            return Err(Resource::Accounting.into());
                        }
                        rows.push(Row {
                            first: first.coordinate,
                            load: row.coordinate,
                        });
                    } else {
                        prior = Some((current, state));
                    }
                } else if !transparent(row.operation) {
                    prior = None;
                }
            }
        }
        budget.charge_work(2)?;
        let retained = size_of::<Self>()
            .checked_add(actual)
            .ok_or(Resource::Arithmetic)?;
        drop(initialized);
        budget.release_storage(initialized_storage)?;
        Ok(Self {
            memory,
            rows,
            retained,
        })
    }
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// Consumes this exact plan. All validation/work precedes the first write.
    /// The first Load is never replaced, so its access and failure stay live.
    pub fn apply(
        self,
        input: &'g Owner,
        candidate: &mut Module,
        budget: &mut Budget<'_>,
    ) -> Result<CanonicalKirAppliedLoadForwardingV1<'g>> {
        budget.charge_work(3)?;
        if !std::ptr::eq(input, self.memory.inventory().owner()) {
            return Err(Error::ForeignSubject);
        }
        budget.charge_work(input.canonical().canonical_bytes().len())?;
        if &*candidate != input.module() {
            return Err(Error::StaleCandidate);
        }
        if size_of::<Self>() != size_of::<CanonicalKirAppliedLoadForwardingV1<'_>>() {
            return Err(Resource::Accounting.into());
        }
        budget.charge_work(self.rows.len())?;
        for row in &self.rows {
            let block = &mut candidate.functions[row.load.block.function.0 as usize]
                .body
                .as_mut()
                .expect("verified body")
                .blocks[row.load.block.block as usize];
            let first = load(&block.operations[row.first.operation as usize], row.first)
                .expect("first planned Load is never rewritten");
            block.operations[row.load.operation as usize].kind = replacement(first);
        }
        Ok(CanonicalKirAppliedLoadForwardingV1 {
            input,
            rows: self.rows,
            retained: self.retained,
        })
    }
}

/// Exact checked pair, not generic alias/memory safety or publication authority.
pub struct CheckedCanonicalKirLoadForwardingV1<'a> {
    input: &'a Owner,
    output: &'a Owner,
    rows: &'a [Row],
}
impl<'a> CheckedCanonicalKirLoadForwardingV1<'a> {
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

/// Rebuilds Inventory/MemorySSA from the actual input, not the producer's
/// report. Replays every ordered row and complete surviving module/CFG payload
/// against the independently verified output. Both exact ordinary loads have
/// identical private pointer/access/type and memory version; the first remains.
/// The fresh direct-slot census also establishes allocation, alignment,
/// initialization and nonescape for every seed. It does not infer equality of
/// arbitrary uninitialized or nondeterministic memory reads.
/// The closed interval independently excludes every possible memory, trap or
/// convergence barrier even when MemorySSA alone would report the same version.
/// All Result exits restore the incoming floor; reserve the returned header.
pub fn check_canonical_kir_load_forwarding_v1<'a>(
    input: &'a Owner,
    output: &'a Owner,
    rows: &'a [Row],
    budget: &mut Budget<'_>,
) -> Result<(CheckedCanonicalKirLoadForwardingV1<'a>, Storage)> {
    let floor = budget.storage();
    let result = (|| {
        budget.charge_work(3)?;
        budget.charge_work(
            input
                .canonical()
                .canonical_bytes()
                .len()
                .checked_add(output.canonical().canonical_bytes().len())
                .ok_or(Resource::Arithmetic)?,
        )?;
        budget.reserve_storage(size_of::<CheckedCanonicalKirLoadForwardingV1<'_>>())?;
        let (inventory, inventory_storage) =
            Inventory::derive(input, budget).map_err(Error::Inventory)?;
        budget.reserve_storage(inventory_storage.retained_storage())?;
        let (memory, memory_storage) = MemorySsa::derive(&inventory, Default::default(), budget)?;
        budget.reserve_storage(memory_storage.retained_storage())?;
        check_modules(input.module(), output.module(), rows, &memory, budget)?;
        drop(memory);
        budget.release_storage(memory_storage.retained_storage())?;
        drop(inventory);
        budget.release_storage(inventory_storage.retained_storage())?;
        Ok(CheckedCanonicalKirLoadForwardingV1 {
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
            Storage(size_of::<CheckedCanonicalKirLoadForwardingV1<'_>>()),
        )
    })
}

fn check_modules(
    input: &Module,
    output: &Module,
    rows: &[Row],
    memory: &MemorySsa<'_, '_>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    let inventory = memory.inventory();
    let (mut initialized, initialized_storage) =
        InitializedPrivateLoadsV1::derive(inventory, budget)?;
    budget.reserve_storage(initialized_storage)?;
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
    let mut operation_ordinal = 0usize;
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
            initialized.enter_block();
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
            let mut prior: Option<(Load, NodeId)> = None;
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
                budget.charge_work(3)?;
                let _actual = inventory
                    .operations()
                    .get(operation_ordinal)
                    .filter(|row| row.coordinate == coordinate && std::ptr::eq(row.operation, a))
                    .ok_or(Error::Rule("exact input operation census"))?;
                let current = initialized.observe(operation_ordinal, budget)?;
                operation_ordinal = operation_ordinal
                    .checked_add(1)
                    .ok_or(Resource::Arithmetic)?;
                let version = current
                    .map(|_| incoming(memory, coordinate, budget))
                    .transpose()?;
                if let Some(row) = rows.get(next).filter(|row| row.load == coordinate) {
                    budget.charge_work(1)?;
                    let (first, state) = prior
                        .filter(|(first, _)| {
                            first.coordinate == row.first && matching_load(a, *first)
                        })
                        .ok_or(Error::Rule("Load provenance or closed barrier"))?;
                    if Some(state) != version {
                        return Err(Error::Rule("actual Load MemorySSA version"));
                    }
                    let Operation { results, kind } = b;
                    if &a.results != results || *kind != replacement(first) {
                        return Err(Error::Rule("forwarded value or type"));
                    }
                    next += 1;
                } else if a != b {
                    return Err(Error::Rule("unrecorded mutation"));
                }
                if let Some(current) = current {
                    if !prior.is_some_and(|(first, _)| matching_load(a, first)) {
                        prior = Some((current, version.expect("actual ordinary Load version")));
                    }
                } else if !transparent(a) {
                    prior = None;
                }
            }
        }
    }
    if next != rows.len() {
        return Err(Error::Rule("unordered, duplicate, or absent row"));
    }
    if operation_ordinal != inventory.operations().len() {
        return Err(Error::Rule("complete input operation census"));
    }
    drop(initialized);
    budget.release_storage(initialized_storage)?;
    Ok(())
}

#[cfg(test)]
#[path = "canonical_kir_load_forwarding_v1_tests.rs"]
mod tests;
