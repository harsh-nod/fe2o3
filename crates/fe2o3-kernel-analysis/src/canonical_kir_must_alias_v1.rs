//! Owner-bound, intraprocedural origins of unmodified workgroup-storage pointers.

use std::{error::Error, fmt, mem::size_of};

use fe2o3_kernel_ir::{
    AddressSpace, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirDefinitionCoordinateV1 as Definition, OperationKind, Type,
};

use crate::{CanonicalKirDefinitionRefV1, CanonicalKirInventoryV1 as Inventory};

/// Independent caps on actual dense rosters, never on sparse SSA identifiers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirMustAliasLimitsV1 {
    pub definitions: usize,
    pub operations: usize,
    pub edge_arguments: usize,
    pub dependencies: usize,
}

impl Default for CanonicalKirMustAliasLimitsV1 {
    fn default() -> Self {
        Self {
            definitions: 65_536,
            operations: 65_536,
            edge_arguments: 262_144,
            dependencies: 393_216,
        }
    }
}

/// Fail-closed resource or actual-inventory admission failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirMustAliasErrorV1 {
    Resource(Resource),
    InputLimit {
        resource: &'static str,
        actual: usize,
        limit: usize,
    },
    InconsistentInventory,
    InvalidDefinition {
        definition: usize,
    },
}

impl From<Resource> for CanonicalKirMustAliasErrorV1 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for CanonicalKirMustAliasErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(formatter),
            Self::InputLimit {
                resource,
                actual,
                limit,
            } => {
                write!(
                    formatter,
                    "native KIR must-alias {resource} count {actual} exceeds {limit}"
                )
            }
            Self::InconsistentInventory => formatter.write_str("inconsistent native KIR inventory"),
            Self::InvalidDefinition { definition } => {
                write!(
                    formatter,
                    "invalid native KIR definition index {definition}"
                )
            }
        }
    }
}
impl Error for CanonicalKirMustAliasErrorV1 {}
type Result<T> = std::result::Result<T, CanonicalKirMustAliasErrorV1>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Origin {
    Pending,
    Allocation(usize),
    Unknown,
}

impl Origin {
    fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            (Self::Pending, value) | (value, Self::Pending) => value,
            (Self::Allocation(a), Self::Allocation(b)) if a == b => Self::Allocation(a),
            (Self::Allocation(_), Self::Allocation(_)) => Self::Unknown,
        }
    }
}

/// A completed report over one exact immutable owner-bound inventory.
///
/// Only WorkgroupMemory results ground origins. Nonentry block parameters join
/// all syntactic incoming edges; pointer selects join both alternatives. A
/// grounded cycle can retain one allocation. Ungrounded cycles, distinct
/// allocations, function/entry arguments, calls, casts, offsets and unsupported
/// producers remain unknown. No reachability, callee summary, alias-disjointness,
/// source-authentication or lifecycle-safety authority is implied.
///
/// This is allocation-definition identity for explicit LDS storage, not a
/// general memory-location/offset analysis or a claim about all Rust pointers.
#[derive(Debug)]
pub struct CanonicalKirMustAliasV1<'i, 'g> {
    inventory: &'i Inventory<'g>,
    origins: Vec<Origin>,
    retained: usize,
}

/// Retained report header and requested payload; caller owns graph/inventory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirMustAliasStorageV1(usize);

impl CanonicalKirMustAliasStorageV1 {
    /// Reserve this before further ledger work while the report remains live.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

impl<'i, 'g> CanonicalKirMustAliasV1<'i, 'g> {
    /// Work/storage O(V + O + E), with E counting edge-argument occurrences and
    /// two dependencies per supported pointer select. After seeding, each lattice
    /// value changes at most twice. A fixed-capacity queue deduplicates pending definitions.
    ///
    /// All variable storage and initialization are charged before allocation.
    /// Scratch is dropped before restoring the incoming storage floor, on both
    /// success and failure. The report receipt transfers on success; accepted
    /// work, peak storage and failure history remain in the caller's ledger.
    pub fn derive(
        inventory: &'i Inventory<'g>,
        limits: CanonicalKirMustAliasLimitsV1,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, CanonicalKirMustAliasStorageV1)> {
        let floor = budget.storage();
        let result = Engine::build(inventory, limits, budget).and_then(|engine| engine.run(budget));
        let release = budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)?;
        budget.release_storage(release)?;
        result.map(|report| {
            let retained = report.retained;
            (report, CanonicalKirMustAliasStorageV1(retained))
        })
    }

    pub const fn inventory(&self) -> &'i Inventory<'g> {
        self.inventory
    }

    pub fn belongs_to(&self, inventory: &Inventory<'_>) -> bool {
        std::ptr::eq(self.inventory, inventory)
    }

    /// Resolves an actual dense definition index in this report's inventory.
    /// Returns the physical allocation row, never a block-parameter binding.
    /// The index is an inert locator, not proof of another owner's identity.
    /// Exactly two work units are charged before this allocation-free query.
    pub fn allocation_for_definition(
        &self,
        definition: usize,
        budget: &mut Budget<'_>,
    ) -> Result<Option<&'i CanonicalKirDefinitionRefV1<'g>>> {
        budget.charge_work(2)?;
        match self.origins.get(definition) {
            Some(Origin::Allocation(origin)) => self
                .inventory
                .definitions()
                .get(*origin)
                .map(Some)
                .ok_or(CanonicalKirMustAliasErrorV1::InconsistentInventory),
            Some(Origin::Unknown) => Ok(None),
            Some(Origin::Pending) => Err(CanonicalKirMustAliasErrorV1::InconsistentInventory),
            None => Err(CanonicalKirMustAliasErrorV1::InvalidDefinition { definition }),
        }
    }
}

#[derive(Clone, Copy)]
struct Link {
    target: usize,
    next: usize,
}

const NONE: usize = usize::MAX;

struct Engine<'i, 'g> {
    report: CanonicalKirMustAliasV1<'i, 'g>,
    heads: Vec<usize>,
    incoming: Vec<u8>,
    queued: Vec<u8>,
    queue: Vec<usize>,
    links: Vec<Link>,
    head: usize,
    tail: usize,
    len: usize,
}

impl<'i, 'g> Engine<'i, 'g> {
    fn build(
        inventory: &'i Inventory<'g>,
        limits: CanonicalKirMustAliasLimitsV1,
        budget: &mut Budget<'_>,
    ) -> Result<Self> {
        budget.charge_work(4)?;
        let definitions = inventory.definitions().len();
        limit("definitions", definitions, limits.definitions)?;
        limit(
            "operations",
            inventory.operations().len(),
            limits.operations,
        )?;
        limit(
            "edge arguments",
            inventory.edge_arguments().len(),
            limits.edge_arguments,
        )?;
        let mut dependencies = inventory.edge_arguments().len();
        for operation in inventory.operations() {
            budget.charge_work(1)?;
            if matches!(operation.operation.kind, OperationKind::Select { .. })
                && operation.results.len() == 1
                && is_storage(inventory.definitions()[operation.results.start].ty)
            {
                dependencies = dependencies.checked_add(2).ok_or(Resource::Arithmetic)?;
            }
        }
        limit("dependencies", dependencies, limits.dependencies)?;
        budget.reserve_storage(size_of::<Self>())?;
        let retained = size_of::<CanonicalKirMustAliasV1<'_, '_>>()
            .checked_add(bytes::<Origin>(definitions)?)
            .ok_or(Resource::Arithmetic)?;
        Ok(Self {
            report: CanonicalKirMustAliasV1 {
                inventory,
                origins: allocate(definitions, Origin::Unknown, budget)?,
                retained,
            },
            heads: allocate(definitions, NONE, budget)?,
            incoming: allocate(definitions, 0, budget)?,
            queued: allocate(definitions, 0, budget)?,
            queue: allocate(definitions, 0, budget)?,
            links: allocate(
                dependencies,
                Link {
                    target: NONE,
                    next: NONE,
                },
                budget,
            )?,
            head: 0,
            tail: 0,
            len: 0,
        })
    }

    fn run(mut self, budget: &mut Budget<'_>) -> Result<CanonicalKirMustAliasV1<'i, 'g>> {
        let inventory = self.report.inventory;
        for (index, definition) in inventory.definitions().iter().enumerate() {
            budget.charge_work(1)?;
            // The first stored block is the admitted function entry. Its
            // parameters have an implicit unknown entry, even with backedges.
            if matches!(definition.coordinate, Definition::BlockArgument { block, .. } if block.block != 0)
                && is_storage(definition.ty)
            {
                self.report.origins[index] = Origin::Pending;
            }
        }
        let mut link = 0;
        for operation in inventory.operations() {
            budget.charge_work(1)?;
            if operation.results.len() != 1 {
                continue;
            }
            let result = operation.results.start;
            if !is_storage(inventory.definitions()[result].ty) {
                continue;
            }
            match operation.operation.kind {
                OperationKind::WorkgroupMemory(_) => {
                    self.report.origins[result] = Origin::Allocation(result);
                }
                OperationKind::Select { .. } => {
                    if operation.operands.len() != 3 {
                        return Err(CanonicalKirMustAliasErrorV1::InconsistentInventory);
                    }
                    self.report.origins[result] = Origin::Pending;
                    for operand in (operation.operands.start + 1)..operation.operands.end {
                        self.link(link, inventory.uses()[operand].definition, result, budget)?;
                        link += 1;
                    }
                }
                _ => {}
            }
        }
        for argument in inventory.edge_arguments() {
            budget.charge_work(1)?;
            self.link(
                link,
                argument.incoming_definition,
                argument.target_definition,
                budget,
            )?;
            link += 1;
        }
        if link != self.links.len() {
            return Err(CanonicalKirMustAliasErrorV1::InconsistentInventory);
        }
        for definition in 0..self.report.origins.len() {
            budget.charge_work(1)?;
            if self.incoming[definition] == 0 && self.report.origins[definition] == Origin::Pending
            {
                self.report.origins[definition] = Origin::Unknown;
            }
            if self.report.origins[definition] != Origin::Pending {
                self.enqueue(definition, budget)?;
            }
        }
        self.drain(budget)?;
        // Bottom is not evidence: close every still-ungrounded cycle to unknown
        // and propagate it into even already-grounded downstream joins.
        for definition in 0..self.report.origins.len() {
            budget.charge_work(1)?;
            if self.report.origins[definition] == Origin::Pending {
                self.report.origins[definition] = Origin::Unknown;
                self.enqueue(definition, budget)?;
            }
        }
        self.drain(budget)?;
        Ok(self.report)
    }

    fn link(
        &mut self,
        index: usize,
        source: usize,
        target: usize,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        budget.charge_work(1)?;
        if source >= self.heads.len() || target >= self.heads.len() || index >= self.links.len() {
            return Err(CanonicalKirMustAliasErrorV1::InconsistentInventory);
        }
        self.links[index] = Link {
            target,
            next: self.heads[source],
        };
        self.heads[source] = index;
        self.incoming[target] = 1;
        Ok(())
    }

    fn enqueue(&mut self, definition: usize, budget: &mut Budget<'_>) -> Result<()> {
        budget.charge_work(1)?;
        if self.queued[definition] != 0 {
            return Ok(());
        }
        if self.len >= self.queue.len() {
            return Err(CanonicalKirMustAliasErrorV1::InconsistentInventory);
        }
        self.queue[self.tail] = definition;
        self.tail = (self.tail + 1) % self.queue.len();
        self.len += 1;
        self.queued[definition] = 1;
        Ok(())
    }

    fn drain(&mut self, budget: &mut Budget<'_>) -> Result<()> {
        while self.len != 0 {
            budget.charge_work(1)?;
            let definition = self.queue[self.head];
            self.head = (self.head + 1) % self.queue.len();
            self.len -= 1;
            self.queued[definition] = 0;
            let value = self.report.origins[definition];
            let mut index = self.heads[definition];
            while index != NONE {
                budget.charge_work(1)?;
                let Link { target, next } = self.links[index];
                let old = self.report.origins[target];
                let joined = old.join(value);
                if joined != old {
                    self.report.origins[target] = joined;
                    self.enqueue(target, budget)?;
                }
                index = next;
            }
        }
        Ok(())
    }
}

fn is_storage(ty: &Type) -> bool {
    matches!(ty, Type::Pointer(pointer) if pointer.address_space == AddressSpace::Workgroup)
}

fn limit(resource: &'static str, actual: usize, limit: usize) -> Result<()> {
    if actual > limit {
        Err(CanonicalKirMustAliasErrorV1::InputLimit {
            resource,
            actual,
            limit,
        })
    } else {
        Ok(())
    }
}

fn bytes<T>(count: usize) -> Result<usize> {
    count
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic.into())
}

fn allocate<T: Copy>(count: usize, value: T, budget: &mut Budget<'_>) -> Result<Vec<T>> {
    if count == 0 {
        return Ok(Vec::new());
    }
    budget.charge_work(count.checked_add(1).ok_or(Resource::Arithmetic)?)?;
    budget.reserve_storage(bytes::<T>(count)?)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    values.resize(count, value);
    Ok(values)
}

#[cfg(test)]
#[path = "canonical_kir_must_alias_v1_tests.rs"]
mod tests;
