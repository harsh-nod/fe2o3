//! Keep initialized data outside the model loan until its ledger commit.

use super::*;
use crate::shared_memory::DeviceInitializationCustodyV1;

pub(in crate::queue) struct DetachedInsertionLedgerV1<'a> {
    pub(in crate::queue) identities: &'a mut Vec<Gfx942FixedDispatchStorageIdentityV1>,
    pub(in crate::queue) count: &'a mut usize,
    pub(in crate::queue) next: &'a mut Option<usize>,
}

pub(in crate::queue) trait DeviceInsertionContextV1 {
    fn require_unbound(&self) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn ledger(&mut self) -> DetachedInsertionLedgerV1<'_>;
    fn reserve(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        reserve_identity_capacity_v1(self.ledger().identities)
    }
    fn prepare(
        &mut self,
        root: &mut DeviceInitializationCustodyV1,
        alignment: u64,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn fail(&mut self, root: DeviceInitializationCustodyV1, panicked: bool);
}

pub(in crate::queue) fn reserve_identity_capacity_v1(
    identities: &mut Vec<Gfx942FixedDispatchStorageIdentityV1>,
) -> Result<(), ComputeAqlQueueSessionErrorV1> {
    let additional = super::super::dispatch_binding::MAX_DISPATCH_DATA_LEASES_V1
        .checked_sub(identities.len())
        .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
            "detached dispatch-data ledger bound",
        ))?;
    identities.try_reserve_exact(additional).map_err(|_| {
        Gfx942DispatchBindingErrorV1::HostAllocationCapacity {
            operation: "detached initialized-device identity ledger",
        }
        .into()
    })
}

pub(in crate::queue) struct SettledDeviceInsertionV1 {
    pub(in crate::queue) result:
        std::thread::Result<Result<Gfx942FixedDispatchDataV1, ComputeAqlQueueSessionErrorV1>>,
    pub(in crate::queue) transport: bool,
}

impl SettledDeviceInsertionV1 {
    pub(super) fn into_result(
        self,
    ) -> Result<Gfx942FixedDispatchDataV1, ComputeAqlQueueSessionErrorV1> {
        match self.result {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
}

pub(in crate::queue) fn settle_device_insertion_v1<C: DeviceInsertionContextV1>(
    context: &mut C,
    mut root: DeviceInitializationCustodyV1,
    data_index: Option<usize>,
    alignment: u64,
) -> SettledDeviceInsertionV1 {
    let mut entered = false;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        context.require_unbound()?;
        let ledger = context.ledger();
        let next_count = *ledger.count + 1;
        if *ledger.count >= super::super::dispatch_binding::MAX_DISPATCH_DATA_LEASES_V1 {
            return Err(Gfx942DispatchBindingErrorV1::DataLeaseCount {
                requested: next_count,
                maximum: super::super::dispatch_binding::MAX_DISPATCH_DATA_LEASES_V1,
            }
            .into());
        }
        let index = data_index.unwrap_or_else(|| ledger.next.unwrap_or(ledger.identities.len()));
        validate_new_detached_data_index(*ledger.count, index)?;
        context.reserve()?;
        entered = true;
        context.prepare(&mut root, alignment)?;
        let identity = Gfx942FixedDispatchStorageIdentityV1::DeviceInitializedContent(
            root.completed()?.storage_identity(),
        );
        let ledger = context.ledger();
        insert_detached_identity_at(ledger.identities, ledger.next, identity, index);
        *ledger.count = next_count;
        let memory = root
            .take_complete()
            .expect("borrowed Complete remains rooted through ledger commit");
        Ok(Gfx942FixedDispatchDataV1::initialized(memory))
    }));
    let transport = result.is_err() || entered && !matches!(result, Ok(Ok(_)));
    if transport {
        context.fail(root, result.is_err());
    }
    SettledDeviceInsertionV1 { result, transport }
}

impl DeviceInsertionContextV1 for ComputeAqlQueueSessionV1 {
    fn require_unbound(&self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.require_unbound_fixed_dispatch()
    }

    fn ledger(&mut self) -> DetachedInsertionLedgerV1<'_> {
        DetachedInsertionLedgerV1 {
            identities: &mut self.detached_data_identities,
            count: &mut self.detached_data_count,
            next: &mut self.detached_next_insertion_index,
        }
    }

    fn prepare(
        &mut self,
        root: &mut DeviceInitializationCustodyV1,
        alignment: u64,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.with_live_queue_memory_model(|memory| {
            memory
                .prepare_device_initialization_in_place(root, alignment)
                .map_err(Into::into)
        })
    }

    fn fail(&mut self, root: DeviceInitializationCustodyV1, panicked: bool) {
        self.poison_terminal();
        if panicked {
            poison_process_global_after_dispatch_terminal_v1();
        }
        if root.requires_retention() {
            if let Some(engine) = self.engine.as_mut() {
                engine
                    .backend
                    .session
                    .retain_device_initialization_failure(root);
            } else {
                // Admitted initialization cannot remove its engine. Preserve
                // custody rather than replacing the original failure if it does.
                core::mem::forget(root);
            }
        }
    }
}

impl ComputeAqlQueueSessionV1 {
    pub(super) fn initialize_device_data_settled_v1(
        &mut self,
        data_index: Option<usize>,
        bytes: Box<[u8]>,
        alignment: u64,
        content: Gfx942DeviceContentDescriptorV1,
    ) -> SettledDeviceInsertionV1 {
        settle_device_insertion_v1(
            self,
            DeviceInitializationCustodyV1::new(bytes, content),
            data_index,
            alignment,
        )
    }
}
