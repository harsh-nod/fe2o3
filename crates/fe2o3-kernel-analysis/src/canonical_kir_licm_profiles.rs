//! Closed adapters for the two admitted graph formats; no caller-defined profile.
use super::*;
use crate::{CanonicalKirInventoryStorageV1, CanonicalKirLoopStorageV1};
use fe2o3_kernel_ir::{
    CanonicalKirControlFlowViewV1 as Flow, CanonicalKirFunctionCoordinateV1 as Function,
    with_canonical_kir_control_flow_v1, with_canonical_kir_control_flow_v18,
};

pub(super) trait Profile: Sized {
    fn wire_len(&self) -> usize;
    fn headers(&self, output: &Self) -> Result<()>;
    fn inventory<'g>(
        &'g self,
        budget: &mut Budget<'_>,
    ) -> Result<(Inventory<'g, Self>, CanonicalKirInventoryStorageV1)>;
    fn loops<'i, 'g>(
        inventory: &'i Inventory<'g, Self>,
        limits: Limits,
        budget: &mut Budget<'_>,
    ) -> Result<(Loops<'i, 'g, Self>, CanonicalKirLoopStorageV1)>;
    fn with_flow<'g, 'w, T>(
        &'g self,
        function: Function,
        budget: &mut Budget<'w>,
        run: impl for<'s> FnOnce(&mut Flow<'s, 'g, Self>, &mut Budget<'w>) -> Result<T>,
    ) -> Result<T>;
}

macro_rules! profile {
    ($owner:ty, $inventory:ident, $loops:ident, $flow:ident, $headers:ident, $wire:expr) => {
        impl Profile for $owner {
            fn wire_len(&self) -> usize {
                ($wire)(self)
            }
            fn headers(&self, output: &Self) -> Result<()> {
                crate::canonical_kir_same_cfg_payload_v1::$headers(self.module(), output.module())
                    .map_err(Error::Mismatch)
            }
            fn inventory<'g>(
                &'g self,
                budget: &mut Budget<'_>,
            ) -> Result<(Inventory<'g, Self>, CanonicalKirInventoryStorageV1)> {
                Ok(Inventory::$inventory(self, budget)?)
            }
            fn loops<'i, 'g>(
                inventory: &'i Inventory<'g, Self>,
                limits: Limits,
                budget: &mut Budget<'_>,
            ) -> Result<(Loops<'i, 'g, Self>, CanonicalKirLoopStorageV1)> {
                Ok(Loops::$loops(inventory, limits, budget)?)
            }
            fn with_flow<'g, 'w, T>(
                &'g self,
                function: Function,
                budget: &mut Budget<'w>,
                run: impl for<'s> FnOnce(&mut Flow<'s, 'g, Self>, &mut Budget<'w>) -> Result<T>,
            ) -> Result<T> {
                $flow(self, function, Default::default(), budget, run)
            }
        }
    };
}
profile!(
    Owner,
    derive,
    derive,
    with_canonical_kir_control_flow_v1,
    check,
    |owner: &Owner| owner.canonical().canonical_bytes().len()
);
profile!(
    Owner18,
    derive_v18,
    derive_v18,
    with_canonical_kir_control_flow_v18,
    check_v18,
    |owner: &Owner18| owner.canonical_bytes().len()
);
