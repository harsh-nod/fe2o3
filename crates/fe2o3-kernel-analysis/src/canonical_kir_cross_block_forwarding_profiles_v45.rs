//! Closed owner adapters; both formats use the same independently checked rule.
use super::*;
use crate::{CanonicalKirInventoryStorageV1, CanonicalKirMemorySsaStorageV1};
use fe2o3_kernel_ir::{
    CanonicalKirControlFlowViewV1 as Flow, CanonicalKirFunctionCoordinateV1 as Function,
    with_canonical_kir_control_flow_v1, with_canonical_kir_control_flow_v18,
};

pub(super) trait Profile: Sized {
    fn layout(
        &self,
        id: fe2o3_kernel_ir::StorageLayoutIdV1,
    ) -> Option<&fe2o3_kernel_ir::StorageLayoutV1>;
    fn wire_len(&self) -> usize;
    fn headers(&self, output: &Self) -> Result<()>;
    fn inventory<'g>(
        &'g self,
        budget: &mut Budget<'_>,
    ) -> Result<(Inventory<'g, Self>, CanonicalKirInventoryStorageV1)>;
    fn memory<'i, 'g>(
        inventory: &'i Inventory<'g, Self>,
        limits: MemoryLimits,
        budget: &mut Budget<'_>,
    ) -> Result<(Memory<'i, 'g, Self>, CanonicalKirMemorySsaStorageV1)>;
    fn with_flow<'g, 'w, T>(
        &'g self,
        function: Function,
        limits: ControlFlowLimits,
        budget: &mut Budget<'w>,
        run: impl for<'s> FnOnce(&mut Flow<'s, 'g, Self>, &mut Budget<'w>) -> Result<T>,
    ) -> Result<T>;
}

macro_rules! profile {
    ($owner:ty, $derive:ident, $flow:ident, $headers:ident, $wire:expr) => {
        impl Profile for $owner {
            fn layout(
                &self,
                id: fe2o3_kernel_ir::StorageLayoutIdV1,
            ) -> Option<&fe2o3_kernel_ir::StorageLayoutV1> {
                self.module().storage_layouts.get(id.0 as usize)
            }
            fn wire_len(&self) -> usize {
                ($wire)(self)
            }
            fn headers(&self, output: &Self) -> Result<()> {
                super::$headers(self.module(), output.module())
            }
            fn inventory<'g>(
                &'g self,
                budget: &mut Budget<'_>,
            ) -> Result<(Inventory<'g, Self>, CanonicalKirInventoryStorageV1)> {
                Ok(Inventory::$derive(self, budget)?)
            }
            fn memory<'i, 'g>(
                inventory: &'i Inventory<'g, Self>,
                limits: MemoryLimits,
                budget: &mut Budget<'_>,
            ) -> Result<(Memory<'i, 'g, Self>, CanonicalKirMemorySsaStorageV1)> {
                Ok(Memory::$derive(inventory, limits, budget)?)
            }
            fn with_flow<'g, 'w, T>(
                &'g self,
                function: Function,
                limits: ControlFlowLimits,
                budget: &mut Budget<'w>,
                run: impl for<'s> FnOnce(&mut Flow<'s, 'g, Self>, &mut Budget<'w>) -> Result<T>,
            ) -> Result<T> {
                $flow(self, function, limits, budget, run)
            }
        }
    };
}
profile!(
    Owner,
    derive,
    with_canonical_kir_control_flow_v1,
    headers,
    |owner: &Owner| owner.canonical().canonical_bytes().len()
);
profile!(
    Owner18,
    derive_v18,
    with_canonical_kir_control_flow_v18,
    headers_v18,
    |owner: &Owner18| owner.canonical_bytes().len()
);
