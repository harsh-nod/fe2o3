//! Checked local redundant-store deletion. The first Store and every other
//! operation stay live. This is not general DSE, source admission or authority.

use crate::{
    CanonicalKirInventoryErrorV1, CanonicalKirInventoryV1 as Inventory,
    CanonicalKirMemorySsaErrorV1, CanonicalKirMemorySsaNodeIdV1 as NodeId,
    CanonicalKirMemorySsaNodeV1 as Node, CanonicalKirMemorySsaV1 as MemorySsa,
};
use fe2o3_kernel_ir::{
    AddressSpace, BinaryOp, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirOperationCoordinateV1 as Coordinate, KirLocalMemoryEffectRefV1, MemoryAccess,
    Module, Operation, OperationKind as Kind, ScalarType, Type, UnaryOp, ValueId,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use std::{
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

#[path = "canonical_kir_redundant_store_check_v1.rs"]
mod check;
#[path = "canonical_kir_redundant_store_slots_v1.rs"]
mod slots;
pub use check::{CheckedCanonicalKirRedundantStoreV1, check_canonical_kir_redundant_store_v1};

/// Inert original coordinates. The anchor is never deleted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirRedundantStoreRowV1 {
    pub anchor: Coordinate,
    pub removed: Coordinate,
}
type Row = CanonicalKirRedundantStoreRowV1;

/// Complete ordered output-to-input operation correspondence, not authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirRedundantStoreRetainedOperationV1 {
    pub input: Coordinate,
    pub output: Coordinate,
}
type Retained = CanonicalKirRedundantStoreRetainedOperationV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirRedundantStoreErrorV1 {
    Resource(Resource),
    Inventory(CanonicalKirInventoryErrorV1),
    MemorySsa(CanonicalKirMemorySsaErrorV1),
    ForeignSubject,
    StaleCandidate,
    Rule(&'static str),
    Panicked,
}
type Error = CanonicalKirRedundantStoreErrorV1;
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl From<CanonicalKirInventoryErrorV1> for Error {
    fn from(error: CanonicalKirInventoryErrorV1) -> Self {
        Self::Inventory(error)
    }
}
impl From<CanonicalKirMemorySsaErrorV1> for Error {
    fn from(error: CanonicalKirMemorySsaErrorV1) -> Self {
        Self::MemorySsa(error)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "redundant private store: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Actual owned capacities and wrapper header, excluding borrowed owners.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirRedundantStoreStorageV1(usize);
impl CanonicalKirRedundantStoreStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}
type Storage = CanonicalKirRedundantStoreStorageV1;

/// A move-only plan borrowing one exact immutable inventory and MemorySSA.
///
/// ```compile_fail
/// use fe2o3_kernel_analysis::{CanonicalKirRedundantStorePlanV1, CanonicalKirMemorySsaV1};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn detach(memory: CanonicalKirMemorySsaV1<'_, '_>, budget: &mut Budget<'_>) {
///     let (plan, _) = CanonicalKirRedundantStorePlanV1::derive(memory.inventory(), &memory, budget).unwrap();
///     drop(memory);
///     let _ = plan.rows();
/// }
/// ```
pub struct CanonicalKirRedundantStorePlanV1<'m, 'i, 'g> {
    memory: &'m MemorySsa<'i, 'g>,
    rows: Vec<Row>,
    origins: Vec<Retained>,
    retained: usize,
}

/// Consumed mutation evidence; independent checking still uses both real owners.
pub struct CanonicalKirAppliedRedundantStoreV1<'g> {
    input: &'g Owner,
    rows: Vec<Row>,
    origins: Vec<Retained>,
    retained: usize,
}
impl<'g> CanonicalKirAppliedRedundantStoreV1<'g> {
    pub const fn input(&self) -> &'g Owner {
        self.input
    }
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }
    pub fn retained_operations(&self) -> &[Retained] {
        &self.origins
    }
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    pub fn check_output<'a>(
        &'a self,
        output: &'a Owner,
        budget: &mut Budget<'_>,
    ) -> Result<(CheckedCanonicalKirRedundantStoreV1<'a>, Storage)> {
        check_canonical_kir_redundant_store_v1(
            self.input,
            output,
            &self.rows,
            &self.origins,
            budget,
        )
    }
}

#[derive(Clone, Copy)]
struct Store {
    coordinate: Coordinate,
    pointer: ValueId,
    value: ValueId,
    access: MemoryAccess,
}
impl Store {
    fn identical(self, other: Self) -> bool {
        self.pointer == other.pointer && self.value == other.value && self.access == other.access
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

// Closed total scalar recipes, not an effect-free-implies-nontrapping claim.
fn transparent(operation: &Operation) -> bool {
    let [result] = operation.results.as_slice() else {
        return false;
    };
    (integer(&result.ty) || result.ty == Type::BOOL)
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

impl<'m, 'i, 'g> CanonicalKirRedundantStorePlanV1<'m, 'i, 'g> {
    /// Linear complete direct-slot census and block scan. Identical runs retain
    /// their first Store. Each subsequent original Def must follow the last Def,
    /// even when that last original Store will also be removed.
    pub fn derive(
        inventory: &'i Inventory<'g>,
        memory: &'m MemorySsa<'i, 'g>,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        scoped(budget, |budget| {
            budget.charge_work(3)?;
            if !memory.belongs_to(inventory) {
                return Err(Error::ForeignSubject);
            }
            budget.reserve_storage(size_of::<Self>())?;
            let (mut rows, row_bytes) = reserve::<Row>(inventory.operations().len(), budget)?;
            let (mut origins, origin_bytes) =
                reserve::<Retained>(inventory.operations().len(), budget)?;
            let (slots, slot_bytes) = slots::Slots::derive(inventory, budget)?;
            budget.reserve_storage(slot_bytes)?;
            for block in inventory.blocks() {
                budget.charge_work(1)?;
                let mut seed: Option<(Store, NodeId)> = None;
                let mut output = 0_u32;
                for ordinal in block.operations.clone() {
                    budget.charge_work(5)?;
                    let row = &inventory.operations()[ordinal];
                    let mut deleted = false;
                    if let Some(store) = slots.store(ordinal, budget)? {
                        let node = memory
                            .operation(row.coordinate, budget)?
                            .ok_or(Error::Rule("Store lacks MemorySSA Def"))?;
                        let incoming = match memory.node(node, budget)? {
                            Node::Def {
                                operation,
                                incoming,
                            } if *operation == row.coordinate => *incoming,
                            _ => return Err(Error::Rule("exact Store MemorySSA identity")),
                        };
                        if let Some((anchor, last)) =
                            seed.filter(|(anchor, _)| anchor.identical(store))
                        {
                            if incoming != last {
                                return Err(Error::Rule("exact redundant Store Def chain"));
                            }
                            push(
                                &mut rows,
                                Row {
                                    anchor: anchor.coordinate,
                                    removed: row.coordinate,
                                },
                                budget,
                            )?;
                            seed = Some((anchor, node));
                            deleted = true;
                        } else {
                            seed = Some((store, node));
                        }
                    } else if !transparent(row.operation) {
                        seed = None;
                    }
                    if !deleted {
                        push(
                            &mut origins,
                            Retained {
                                input: row.coordinate,
                                output: Coordinate {
                                    block: block.coordinate,
                                    operation: output,
                                },
                            },
                            budget,
                        )?;
                        output = output.checked_add(1).ok_or(Resource::Arithmetic)?;
                    }
                }
            }
            let retained = size_of::<Self>()
                .checked_add(row_bytes)
                .and_then(|n| n.checked_add(origin_bytes))
                .ok_or(Resource::Arithmetic)?;
            drop(slots);
            budget.release_storage(slot_bytes)?;
            Ok((
                Self {
                    memory,
                    rows,
                    origins,
                    retained,
                },
                CanonicalKirRedundantStoreStorageV1(retained),
            ))
        })
    }
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }
    pub fn retained_operations(&self) -> &[Retained] {
        &self.origins
    }

    /// Consumes the plan once. All fallible work and allocation precedes mutation.
    /// Linear retain preserves paid vector capacity and never shifts via remove.
    pub fn apply(
        self,
        input: &'g Owner,
        candidate: &mut Module,
        budget: &mut Budget<'_>,
    ) -> Result<CanonicalKirAppliedRedundantStoreV1<'g>> {
        budget.charge_work(3)?;
        if !std::ptr::eq(input, self.memory.inventory().owner()) {
            return Err(Error::ForeignSubject);
        }
        budget.charge_work(input.canonical().canonical_bytes().len())?;
        if &*candidate != input.module() {
            return Err(Error::StaleCandidate);
        }
        if size_of::<Self>() != size_of::<CanonicalKirAppliedRedundantStoreV1<'_>>() {
            return Err(Resource::Accounting.into());
        }
        let inventory = self.memory.inventory();
        let work = inventory
            .functions()
            .len()
            .checked_add(inventory.blocks().len())
            .and_then(|n| n.checked_add(inventory.operations().len()))
            .ok_or(Resource::Arithmetic)?;
        budget.charge_work(work)?;
        let mut next = 0;
        for (f, function) in candidate.functions.iter_mut().enumerate() {
            if let Some(body) = &mut function.body {
                for (b, block) in body.blocks.iter_mut().enumerate() {
                    let mut old = 0_u32;
                    block.operations.retain(|_| {
                        let coordinate = Coordinate {
                            block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
                                function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
                                    f as u32,
                                ),
                                block: b as u32,
                            },
                            operation: old,
                        };
                        old += 1;
                        if self
                            .rows
                            .get(next)
                            .is_some_and(|row| row.removed == coordinate)
                        {
                            next += 1;
                            false
                        } else {
                            true
                        }
                    });
                }
            }
        }
        Ok(CanonicalKirAppliedRedundantStoreV1 {
            input,
            rows: self.rows,
            origins: self.origins,
            retained: self.retained,
        })
    }
}

fn reserve<T>(count: usize, budget: &mut Budget<'_>) -> Result<(Vec<T>, usize)> {
    budget.charge_work(5)?;
    let requested = count
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(requested)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    let actual = result
        .capacity()
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(actual.checked_sub(requested).ok_or(Resource::Accounting)?)?;
    Ok((result, actual))
}
fn push<T>(rows: &mut Vec<T>, item: T, budget: &mut Budget<'_>) -> Result<()> {
    budget.charge_work(1)?;
    if rows.len() == rows.capacity() {
        return Err(Resource::Accounting.into());
    }
    rows.push(item);
    Ok(())
}
fn scoped<'work, T>(
    budget: &mut Budget<'work>,
    run: impl FnOnce(&mut Budget<'work>) -> Result<T>,
) -> Result<T> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let result = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(Error::Panicked)
        }
    };
    if ledger != budget.work_ledger_identity_v1() || budget.storage() < floor {
        drop(result);
        return Err(Resource::Accounting.into());
    }
    if let Err(error) = budget.release_storage(budget.storage() - floor) {
        drop(result);
        return Err(error.into());
    }
    result
}

#[cfg(test)]
#[path = "canonical_kir_redundant_store_v1_tests.rs"]
mod tests;
