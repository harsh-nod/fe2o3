//! Exact physical private-memory initialization on a borrowed canonical inventory.
//! Source lifetime, optimizer equivalence and native policy admission are separate.
use crate::{CanonicalKirInventoryErrorV1, CanonicalKirInventoryV1};
use fe2o3_kernel_ir::{
    AddressSpace, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1, Constant, OperationKind, Type, ValueId,
};
use std::{
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

#[path = "canonical_kir_private_byte_memory_v38.rs"]
mod bytes;
#[path = "canonical_kir_private_memory_cfg_v1.rs"]
mod physical_cfg;
#[path = "canonical_kir_private_memory_queue_v1.rs"]
mod queue;
#[path = "canonical_kir_private_memory_typed_v18.rs"]
mod typed;
pub use bytes::{
    CanonicalKirPrivateByteAddressV38, CanonicalKirPrivateByteAnalysisV38,
    CanonicalKirPrivateByteLimitsV38, CanonicalKirPrivateByteObligationV38,
    CanonicalKirPrivateByteOperationKindV38, CanonicalKirPrivateByteOperationV38,
    CanonicalKirPrivateByteRangeV38, CanonicalKirPrivateByteStorageV38,
    analyze_canonical_kir_private_bytes_v38,
};
#[doc(hidden)]
pub use queue::CanonicalKirPrivateDataflowQueueV1;
pub use typed::{CheckedCanonicalKirPrivateMemoryV18, check_canonical_kir_private_memory_v18};

/// Existing physical cell ceiling, independent of the live byte ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirPrivateMemoryLimitsV1 {
    /// Maximum sum of fixed allocation elements in the actual inventory.
    pub max_cells: usize,
}

/// No partial physical proof is returned on refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirPrivateMemoryErrorV1 {
    /// The caller's existing work/storage ledger refused.
    Resource(Resource),
    /// An actual inventory query refused.
    Inventory(CanonicalKirInventoryErrorV1),
    /// The existing physical profile refused this exact requirement.
    Unsupported {
        phase: &'static str,
        detail: &'static str,
    },
    /// A construction panic was caught after discarding local backing.
    Panicked,
}
impl From<Resource> for CanonicalKirPrivateMemoryErrorV1 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for CanonicalKirPrivateMemoryErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Inventory(error) => error.fmt(f),
            Self::Unsupported { phase, detail } => write!(f, "{phase}: {detail}"),
            Self::Panicked => f.write_str("physical private-memory construction panicked"),
        }
    }
}
impl std::error::Error for CanonicalKirPrivateMemoryErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Inventory(error) => Some(error),
            Self::Unsupported { .. } | Self::Panicked => None,
        }
    }
}
type Error = CanonicalKirPrivateMemoryErrorV1;
type R<T> = Result<T, Error>;

/// Read-only address diagnostics derived by the physical checker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirPrivateMemoryAddressV1 {
    allocation: usize,
    start: usize,
    length: usize,
    offset: usize,
    alignment: u32,
    stride: usize,
}
impl CanonicalKirPrivateMemoryAddressV1 {
    pub const fn allocation(&self) -> usize {
        self.allocation
    }
    pub const fn start(&self) -> usize {
        self.start
    }
    pub const fn length(&self) -> usize {
        self.length
    }
    pub const fn offset(&self) -> usize {
        self.offset
    }
    pub const fn alignment(&self) -> u32 {
        self.alignment
    }
    pub const fn stride(&self) -> usize {
        self.stride
    }
}
type Address = CanonicalKirPrivateMemoryAddressV1;

/// Exact borrowed inventory plus immutable physical backing, not source proof.
/// Queries are allocation-free O(1) borrows; consumers pay their own row scans.
///
/// ```compile_fail
/// use fe2o3_kernel_analysis::CheckedCanonicalKirPrivateMemoryV1;
/// fn forge() { let _ = CheckedCanonicalKirPrivateMemoryV1 {}; }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_analysis::CheckedCanonicalKirPrivateMemoryV1;
/// fn copy(value: CheckedCanonicalKirPrivateMemoryV1<'_, '_>) {
///     let _ = value.clone();
/// }
/// ```
pub struct CheckedCanonicalKirPrivateMemoryV1<
    'a,
    'g,
    O = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
> {
    inventory: &'a CanonicalKirInventoryV1<'g, O>,
    definitions: Vec<Option<Address>>,
    operations: Vec<bool>,
    latest_stores: Vec<Option<usize>>,
}
type PrivateMemory<'a, 'g, O = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12> =
    CheckedCanonicalKirPrivateMemoryV1<'a, 'g, O>;
impl<'a, 'g, O> CheckedCanonicalKirPrivateMemoryV1<'a, 'g, O> {
    pub const fn inventory(&self) -> &'a CanonicalKirInventoryV1<'g, O> {
        self.inventory
    }
    pub fn definition(&self, index: usize) -> bool {
        self.definitions.get(index).is_some_and(Option::is_some)
    }
    pub fn operation(&self, index: usize) -> bool {
        self.operations.get(index).copied().unwrap_or(false)
    }
    pub fn is_for(&self, inventory: &CanonicalKirInventoryV1<'_, O>) -> bool {
        std::ptr::eq(self.inventory, inventory)
    }
    pub fn address(&self, index: usize) -> Option<&CanonicalKirPrivateMemoryAddressV1> {
        self.definitions.get(index).and_then(Option::as_ref)
    }
    pub fn latest_stores(&self) -> &[Option<usize>] {
        &self.latest_stores
    }
    /// Physical initialization does not confer source, native or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
    fn retained_storage(&self) -> R<usize> {
        size_of::<Self>()
            .checked_add(capacity_bytes(&self.definitions)?)
            .and_then(|bytes| bytes.checked_add(capacity_bytes(&self.operations).ok()?))
            .and_then(|bytes| bytes.checked_add(capacity_bytes(&self.latest_stores).ok()?))
            .ok_or_else(arithmetic)
    }
}

/// Unreserved transfer of only the returned proof header and actual capacities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirPrivateMemoryStorageV1(usize);
impl CanonicalKirPrivateMemoryStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Scoped physical proof. Prepay owner/inventory separately and reserve the
/// returned receipt immediately before further work. Temporary vectors and
/// panic payloads drop before valid-ledger cleanup. No callback or raw parts
/// are accepted, and the exact borrowed inventory remains the subject.
///
/// ```compile_fail
/// use fe2o3_kernel_analysis::{CanonicalKirInventoryV1,
///     CanonicalKirPrivateMemoryLimitsV1, check_canonical_kir_private_memory_v1};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// fn escape(inventory: CanonicalKirInventoryV1<'_>,
///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let (proof, _) = check_canonical_kir_private_memory_v1(
///         &inventory, CanonicalKirPrivateMemoryLimitsV1 { max_cells: 8 }, budget).unwrap();
///     drop(inventory);
///     let _ = proof.inventory();
/// }
/// ```
pub fn check_canonical_kir_private_memory_v1<'a, 'g>(
    inventory: &'a CanonicalKirInventoryV1<'g>,
    limits: CanonicalKirPrivateMemoryLimitsV1,
    budget: &mut Budget<'_>,
) -> R<(
    CheckedCanonicalKirPrivateMemoryV1<'a, 'g>,
    CanonicalKirPrivateMemoryStorageV1,
)> {
    scoped(budget, |budget| {
        let proof = build(inventory, limits.max_cells, budget)?;
        charge(budget, 4)?;
        let receipt = CanonicalKirPrivateMemoryStorageV1(proof.retained_storage()?);
        Ok((proof, receipt))
    })
}

/// Compatibility only: retains all paid construction credit until the caller's
/// enclosing scratch transaction drops the proof and releases the delta.
/// This is NOT the scoped transfer contract above. The sole production caller
/// is the lowerer's legacy private-memory check adapter, pending its retirement.
/// No duplicate checker, mode flag or transferable source authority is involved.
#[doc(hidden)]
pub fn check_canonical_kir_private_memory_retaining_scratch_v1<'a, 'g>(
    inventory: &'a CanonicalKirInventoryV1<'g>,
    max_cells: usize,
    budget: &mut Budget<'_>,
) -> R<CheckedCanonicalKirPrivateMemoryV1<'a, 'g>> {
    build(inventory, max_cells, budget)
}

fn inventory_error(error: CanonicalKirInventoryErrorV1) -> Error {
    Error::Inventory(error)
}
fn arithmetic() -> Error {
    Error::Resource(Resource::Arithmetic)
}
fn refused(phase: &'static str, detail: &'static str) -> Error {
    Error::Unsupported { phase, detail }
}
fn charge(budget: &mut Budget<'_>, amount: usize) -> R<()> {
    budget.charge_work(amount).map_err(Error::Resource)
}
fn capacity_bytes<T>(rows: &Vec<T>) -> R<usize> {
    rows.capacity()
        .checked_mul(size_of::<T>())
        .ok_or_else(arithmetic)
}

// This is the existing lowerer allocator's exact six-work, header and actual
// capacity contract. Its construction credits stay with the enclosing scope.
fn scratch<T>(count: usize, budget: &mut Budget<'_>) -> R<Vec<T>> {
    charge(budget, 6)?;
    let bytes = count.checked_mul(size_of::<T>()).ok_or_else(arithmetic)?;
    let reserve = bytes
        .checked_add(size_of::<Vec<T>>())
        .ok_or_else(arithmetic)?;
    budget.reserve_storage(reserve)?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    let actual = capacity_bytes(&rows)?;
    budget.reserve_storage(actual.checked_sub(bytes).ok_or(Resource::Accounting)?)?;
    Ok(rows)
}

struct Cleanup<'a, 'w> {
    budget: &'a mut Budget<'w>,
    slot: usize,
    ledger: CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
    active: bool,
}
impl Cleanup<'_, '_> {
    fn valid(&self) -> bool {
        self.slot == std::ptr::from_ref(&*self.budget) as usize
            && self.ledger == self.budget.work_ledger_identity_v1()
            && self.budget.storage() >= self.floor
    }
    fn finish(&mut self) -> R<()> {
        if !self.active {
            return Ok(());
        }
        self.active = false;
        if !self.valid() {
            return Err(Resource::Accounting.into());
        }
        #[cfg(test)]
        resource_tests::observe_refund(self.budget.storage(), self.floor);
        self.budget
            .release_storage(self.budget.storage() - self.floor)?;
        Ok(())
    }
}
impl Drop for Cleanup<'_, '_> {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}
// Constant-space containment: a payload's destructor can itself panic. Drain
// every resulting payload before releasing backing credit. Semantic work does
// not bound arbitrary external Rust destructor wall-clock cost.
fn discard<T>(value: T) {
    let mut pending = catch_unwind(AssertUnwindSafe(|| drop(value)));
    while let Err(payload) = pending {
        pending = catch_unwind(AssertUnwindSafe(|| drop(payload)));
    }
}
fn scoped<'w, T>(budget: &mut Budget<'w>, run: impl FnOnce(&mut Budget<'w>) -> R<T>) -> R<T> {
    // Prepay constant setup, custody/postflight checks and cleanup bookkeeping.
    charge(budget, 4)?;
    let mut cleanup = Cleanup {
        slot: std::ptr::from_ref(&*budget) as usize,
        ledger: budget.work_ledger_identity_v1(),
        floor: budget.storage(),
        budget,
        active: true,
    };
    let returned = catch_unwind(AssertUnwindSafe(|| {
        cleanup
            .budget
            .reserve_storage(size_of::<Cleanup<'_, '_>>())?;
        cleanup
            .budget
            .reserve_storage(size_of::<std::thread::Result<R<T>>>())?;
        cleanup
            .budget
            .reserve_storage(size_of::<std::thread::Result<()>>())?;
        run(cleanup.budget)
    }));
    if !cleanup.valid() {
        discard(returned);
        return Err(Resource::Accounting.into());
    }
    let result = match returned {
        Ok(result) => result,
        Err(payload) => {
            discard(payload);
            Err(Error::Panicked)
        }
    };
    match cleanup.finish() {
        Ok(()) => result,
        Err(error) => {
            discard(result);
            Err(error)
        }
    }
}

fn is_private(ty: &Type) -> bool {
    matches!(ty, Type::Pointer(pointer) if pointer.address_space == AddressSpace::Private)
}

fn index<O>(
    inventory: &CanonicalKirInventoryV1<'_, O>,
    function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    value: ValueId,
    budget: &mut Budget<'_>,
) -> R<usize> {
    inventory
        .definition_index_for_value(function, value, budget)
        .map_err(inventory_error)?
        .ok_or_else(|| refused("private", "exact function-local definition"))
}

fn build<'a, 'g, O: typed::PrivateMemoryOwner>(
    inventory: &'a CanonicalKirInventoryV1<'g, O>,
    max_cells: usize,
    budget: &mut Budget<'_>,
) -> R<PrivateMemory<'a, 'g, O>> {
    charge(budget, 2)?;
    budget
        .reserve_storage(std::mem::size_of::<&CanonicalKirInventoryV1<'_, O>>())
        .map_err(Error::Resource)?;
    let mut constants = scratch::<Option<u64>>(inventory.definitions().len(), budget)?;
    let mut addresses = scratch::<Option<Address>>(inventory.definitions().len(), budget)?;
    let mut operations = scratch::<bool>(inventory.operations().len(), budget)?;
    let mut latest_stores = scratch::<Option<usize>>(inventory.operations().len(), budget)?;
    charge(
        budget,
        inventory
            .definitions()
            .len()
            .checked_mul(2)
            .and_then(|n| {
                inventory
                    .operations()
                    .len()
                    .checked_mul(2)
                    .and_then(|m| n.checked_add(m))
            })
            .ok_or_else(arithmetic)?,
    )?;
    constants.resize(inventory.definitions().len(), None);
    addresses.resize(inventory.definitions().len(), None);
    operations.resize(inventory.operations().len(), false);
    latest_stores.resize(inventory.operations().len(), None);
    for row in inventory.operations() {
        charge(budget, 2)?;
        if let OperationKind::Constant(Constant::Index(value)) = row.operation.kind
            && row.results.len() == 1
        {
            constants[row.results.start] = Some(value);
        } else if O::TYPED {
            charge(budget, 1)?;
            if let OperationKind::Constant(value) = &row.operation.kind
                && row.results.len() == 1
            {
                constants[row.results.start] = typed::count_literal(value);
            }
        }
    }
    let mut cells = 0usize;
    for (ordinal, row) in inventory.operations().iter().enumerate() {
        charge(budget, 3)?;
        let OperationKind::Alloca {
            element,
            count,
            address_space,
            alignment,
        } = &row.operation.kind
        else {
            continue;
        };
        if *address_space != AddressSpace::Private
            || (!matches!(element, Type::Scalar(_))
                && !(O::TYPED && matches!(element, Type::StorageObject(_))))
            || *alignment == 0
            || row.results.len() != 1
            || row.effects.len() != 1
        {
            return Err(refused("private", "one exact scalar private allocation"));
        }
        charge(budget, 3)?;
        let stride = match element {
            Type::Scalar(scalar) => usize::from(
                scalar
                    .bit_width()
                    .ok_or_else(|| refused("private", "fixed-width scalar allocation layout"))?
                    .div_ceil(8),
            ),
            Type::StorageObject(layout) if O::TYPED => {
                typed::scalar_stride(inventory.owner().layouts(), *layout, *alignment, budget)?
            }
            _ => return Err(refused("private", "one exact scalar private allocation")),
        };
        let length = match count {
            None => 1,
            Some(count) => {
                let definition = index(inventory, row.coordinate.block.function, *count, budget)?;
                usize::try_from(
                    constants[definition]
                        .ok_or_else(|| refused("private", "constant allocation extent"))?,
                )
                .map_err(|_| arithmetic())?
            }
        };
        charge(budget, 5)?;
        let end = cells.checked_add(length).ok_or_else(arithmetic)?;
        if length == 0 || end > max_cells {
            return Err(refused("private", "bounded nonzero allocation extent"));
        }
        charge(budget, 1)?;
        length.checked_mul(stride).ok_or_else(arithmetic)?;
        if !matches!(
            inventory.effects()[row.effects.start].effect,
            fe2o3_kernel_ir::KirLocalMemoryEffectRefV1::Allocate(AddressSpace::Private)
        ) {
            return Err(refused("private", "exact allocation effect"));
        }
        addresses[row.results.start] = Some(Address {
            allocation: ordinal,
            start: cells,
            length,
            offset: 0,
            alignment: *alignment,
            stride,
        });
        operations[ordinal] = true;
        cells = end;
    }
    // Only direct constant element addresses and the V18 typed restriction
    // below acquire lineage. Equal pointer bits/types are never provenance.
    for (ordinal, row) in inventory.operations().iter().enumerate() {
        charge(
            budget,
            row.results.len().checked_add(2).ok_or_else(arithmetic)?,
        )?;
        if !row
            .results
            .clone()
            .any(|i| is_private(inventory.definitions()[i].ty))
        {
            continue;
        }
        if matches!(row.operation.kind, OperationKind::Alloca { .. }) {
            continue;
        }
        if O::TYPED
            && matches!(
                row.operation.kind,
                OperationKind::Cast {
                    kind: fe2o3_kernel_ir::CastKind::RestrictPointerAccess,
                    ..
                }
            )
        {
            let address = typed::restrict_address(inventory, row, &addresses, budget)?;
            addresses[row.results.start] = Some(address);
            // Restriction is pure address transport, not a physical Memory row.
            continue;
        }
        let OperationKind::GetElementPointer { base, offset } = row.operation.kind else {
            return Err(refused(
                "private",
                "direct allocation or constant element address",
            ));
        };
        let base_index = index(inventory, row.coordinate.block.function, base, budget)?;
        let offset_index = index(inventory, row.coordinate.block.function, offset, budget)?;
        charge(budget, 5)?;
        let base =
            addresses[base_index].ok_or_else(|| refused("private", "known allocation base"))?;
        let allocation = &inventory.operations()[base.allocation];
        if O::TYPED {
            charge(budget, 1)?;
            if matches!(
                allocation.operation.kind,
                OperationKind::Alloca {
                    element: Type::StorageObject(_),
                    ..
                }
            ) {
                return Err(refused("private", "whole typed allocation address only"));
            }
        }
        if base.offset != 0 || allocation.results.start != base_index || row.results.len() != 1 {
            return Err(refused("private", "direct allocation base only"));
        }
        let offset = usize::try_from(
            constants[offset_index]
                .ok_or_else(|| refused("private", "constant exact element offset"))?,
        )
        .map_err(|_| arithmetic())?;
        if offset >= base.length {
            return Err(refused("private", "element offset within allocation"));
        }
        addresses[row.results.start] = Some(Address { offset, ..base });
        operations[ordinal] = true;
    }
    for (ordinal, definition) in inventory.definitions().iter().enumerate() {
        charge(budget, 2)?;
        if is_private(definition.ty) && addresses[ordinal].is_none() {
            return Err(refused(
                "private",
                "no private parameters or transported unknown pointers",
            ));
        }
    }
    let mut latest = scratch::<Option<usize>>(cells, budget)?;
    charge(budget, cells)?;
    latest.resize(cells, None);
    let mut cross_block = false;
    for block in inventory.blocks() {
        charge(budget, cells.checked_add(1).ok_or_else(arithmetic)?)?;
        latest.fill(None);
        for ordinal in block.operations.clone() {
            let row = &inventory.operations()[ordinal];
            charge(budget, 3)?;
            let memory = memory_access::<O>(&row.operation.kind);
            if let Some((pointer, access, write)) = memory {
                let definition = index(inventory, row.coordinate.block.function, pointer, budget)?;
                charge(budget, 6)?;
                let address = addresses[definition]
                    .ok_or_else(|| refused("private", "known memory address"))?;
                if O::TYPED && matches!(row.operation.kind, OperationKind::Storage(_)) {
                    typed::check_access(inventory, row, definition, address, budget)?;
                }
                if access.volatile || access.alignment == 0 || row.effects.len() != 1 {
                    return Err(refused("private", "one ordinary nonvolatile memory effect"));
                }
                charge(budget, 4)?;
                let byte_offset = address
                    .offset
                    .checked_mul(address.stride)
                    .ok_or_else(arithmetic)?;
                if access.alignment > address.alignment
                    || byte_offset % access.alignment as usize != 0
                {
                    return Err(refused(
                        "private",
                        "access alignment follows allocation and element offset",
                    ));
                }
                if !matches!(
                    (write, inventory.effects()[row.effects.start].effect),
                    (
                        true,
                        fe2o3_kernel_ir::KirLocalMemoryEffectRefV1::Write(AddressSpace::Private)
                    ) | (
                        false,
                        fe2o3_kernel_ir::KirLocalMemoryEffectRefV1::Read(AddressSpace::Private)
                    )
                ) {
                    return Err(refused("private", "exact memory effect"));
                }
                let cell = address
                    .start
                    .checked_add(address.offset)
                    .ok_or_else(arithmetic)?;
                if write {
                    latest[cell] = Some(ordinal);
                } else if latest[cell].is_none() {
                    cross_block = true;
                } else {
                    latest_stores[ordinal] = latest[cell];
                }
                operations[ordinal] = true;
            }
            for operand in &inventory.uses()[row.operands.clone()] {
                charge(budget, 3)?;
                if addresses[operand.definition].is_none() {
                    continue;
                }
                let permitted = match row.operation.kind {
                    OperationKind::GetElementPointer { base, .. } => {
                        base == operand.value && operations[ordinal]
                    }
                    OperationKind::Load { pointer, .. } => {
                        pointer == operand.value && operations[ordinal]
                    }
                    OperationKind::Store { pointer, value, .. } => {
                        pointer == operand.value && value != operand.value && operations[ordinal]
                    }
                    OperationKind::Storage(fe2o3_kernel_ir::StorageOperationV1::ReadValue {
                        address,
                        ..
                    }) if O::TYPED => address == operand.value && operations[ordinal],
                    OperationKind::Storage(fe2o3_kernel_ir::StorageOperationV1::WriteValue {
                        address,
                        value,
                        ..
                    }) if O::TYPED => {
                        address == operand.value && value != operand.value && operations[ordinal]
                    }
                    OperationKind::Cast {
                        kind: fe2o3_kernel_ir::CastKind::RestrictPointerAccess,
                        value,
                        ..
                    } if O::TYPED => {
                        charge(budget, 3)?;
                        // The producer pass authenticated this exact result and
                        // copied only the direct typed allocation's address.
                        value == operand.value
                            && row.results.len() == 1
                            && addresses[row.results.start] == addresses[operand.definition]
                    }
                    _ => false,
                };
                if !permitted {
                    return Err(refused("private", "private pointer does not escape"));
                }
            }
        }
        for operand in &inventory.uses()[block.terminator_uses.clone()] {
            charge(budget, 2)?;
            if addresses[operand.definition].is_some() {
                return Err(refused("private", "no private pointer control transport"));
            }
        }
    }
    if cross_block {
        physical_cfg::check(inventory, &addresses, &mut latest_stores, budget)?;
    }
    Ok(PrivateMemory {
        inventory,
        definitions: addresses,
        operations,
        latest_stores,
    })
}

fn memory_access<O: typed::PrivateMemoryOwner>(
    kind: &OperationKind,
) -> Option<(ValueId, fe2o3_kernel_ir::MemoryAccess, bool)> {
    match *kind {
        OperationKind::Load { pointer, access }
            if access.address_space == AddressSpace::Private =>
        {
            Some((pointer, access, false))
        }
        OperationKind::Store {
            pointer, access, ..
        } if access.address_space == AddressSpace::Private => Some((pointer, access, true)),
        OperationKind::Storage(fe2o3_kernel_ir::StorageOperationV1::ReadValue {
            address,
            access,
        }) if O::TYPED && access.address_space == AddressSpace::Private => {
            Some((address, access, false))
        }
        OperationKind::Storage(fe2o3_kernel_ir::StorageOperationV1::WriteValue {
            address,
            access,
            ..
        }) if O::TYPED && access.address_space == AddressSpace::Private => {
            Some((address, access, true))
        }
        _ => None,
    }
}

#[cfg(test)]
#[path = "canonical_kir_private_memory_resources_v1_tests.rs"]
mod resource_tests;
#[cfg(test)]
#[path = "canonical_kir_private_memory_v1_tests.rs"]
mod tests;
