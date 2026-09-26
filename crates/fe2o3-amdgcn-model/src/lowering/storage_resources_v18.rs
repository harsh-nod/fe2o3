//! Metering for reached storage work. The shared legacy emitter's allocation
//! policy remains separate; this is not a whole-entry M9 resource receipt.

use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::cell::{Cell, RefCell};
use std::mem::size_of;

pub(super) trait StorageMeterV18 {
    fn charge(&self, amount: usize) -> Result<(), Resource>;
}

pub(super) struct MeterV18<'b, 'w> {
    budget: RefCell<&'b mut Budget<'w>>,
    first: Cell<Option<Resource>>,
    held: usize,
}

pub(super) fn headers_v18() -> usize {
    use storage_v1::{
        StoragePointerEmissionErrorV1 as PointerError, StoragePointerRecipeV1 as Recipe,
        StoragePointerShapeV1 as Shape, StoragePointerTextOperandV1 as Operand,
        StorageTargetContextV1 as Target,
    };
    size_of::<MeterV18<'_, '_>>()
        + 2 * size_of::<Result<MeterV18<'_, '_>, Resource>>()
        + size_of::<storage_native_v18::StorageEmissionContextV18<'_>>()
        + 2 * size_of::<Result<String, LoweringErrors>>()
        + 2 * size_of::<Result<String, storage_native_v18::StorageLoweringErrorV18>>()
        + size_of::<CapacityLimitedText<'_>>()
        + 2 * size_of::<fe2o3_kernel_ir::VerifiedStorageKernelIrModuleV1<'_>>()
        // Eleven statically named local sites, including the two constructors.
        // Loop iterations do not retain names; this sums mutually exclusive sites.
        + 11 * size_of::<NameV18>()
        + 2 * size_of::<Result<NameV18, fmt::Error>>()
        + 2 * size_of::<Result<NameV18, LoweringErrors>>()
        + 2 * size_of::<Target<'_, '_>>()
        + 2 * size_of::<Result<Target<'_, '_>, PointerError>>()
        + 2 * size_of::<Recipe<'_, '_, '_>>()
        + 2 * size_of::<Result<Recipe<'_, '_, '_>, PointerError>>()
        + size_of::<Shape>()
        + size_of::<Operand<'_>>()
        + 2 * size_of::<Result<Operand<'_>, PointerError>>()
        + 2 * size_of::<storage_values_v18::ResultTypeV18<'_>>()
        + size_of::<[[[bool; 5]; 5]; 2]>()
        + size_of::<[KernelAddressSpace; 5]>()
}

impl<'b, 'w> MeterV18<'b, 'w> {
    pub(super) fn new(budget: &'b mut Budget<'w>) -> Result<Self, Resource> {
        let held = headers_v18();
        budget.reserve_storage(held)?;
        Ok(Self {
            budget: RefCell::new(budget),
            first: Cell::new(None),
            held,
        })
    }

    pub(super) fn failure(&self) -> Option<Resource> {
        self.first.get()
    }
}

impl StorageMeterV18 for MeterV18<'_, '_> {
    fn charge(&self, amount: usize) -> Result<(), Resource> {
        if let Some(first) = self.first.get() {
            return Err(first);
        }
        let result = self.budget.borrow_mut().charge_work(amount);
        if let Err(error) = result {
            self.first.set(Some(error));
        }
        result
    }
}

impl Drop for MeterV18<'_, '_> {
    fn drop(&mut self) {
        // No callback or replacement budget is exposed by this scope.
        let _ = self.budget.get_mut().release_storage(self.held);
    }
}

/// Named SSA operands are bounded by numeric KIR coordinates and fixed suffixes.
/// This prevents per-operation heap name allocation outside the shared bindings.
pub(super) struct NameV18 {
    bytes: [u8; 128],
    length: usize,
}
impl NameV18 {
    pub(super) fn new(args: fmt::Arguments<'_>) -> Result<Self, fmt::Error> {
        let mut value = Self {
            bytes: [0; 128],
            length: 0,
        };
        fmt::write(&mut value, args)?;
        Ok(value)
    }
    pub(super) fn as_str(&self) -> &str {
        std::str::from_utf8(&self.bytes[..self.length]).expect("only UTF-8 formatting")
    }
}
impl fmt::Write for NameV18 {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let end = self
            .length
            .checked_add(text.len())
            .filter(|n| *n <= self.bytes.len())
            .ok_or(fmt::Error)?;
        self.bytes[self.length..end].copy_from_slice(text.as_bytes());
        self.length = end;
        Ok(())
    }
}
impl fmt::Display for NameV18 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
#[path = "storage_resources_v18_tests.rs"]
mod tests;
