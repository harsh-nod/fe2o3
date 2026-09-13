//! Keep returned data outside the model loan until its ledger commit.

use super::*;
use crate::shared_memory::{
    CoherentAllocationCustodyV1, CoherentInitializationCustodyV1, DeviceInitializationCustodyV1,
};

pub(in crate::queue) struct DetachedInsertionLedgerV1<'a> {
    pub(in crate::queue) identities: &'a mut Vec<Gfx942FixedDispatchStorageIdentityV1>,
    pub(in crate::queue) count: &'a mut usize,
    pub(in crate::queue) next: &'a mut Option<usize>,
}

pub(in crate::queue) enum DataInsertionIndexV1 {
    Explicit(usize),
    HoleOrAppend,
    RequiredHole,
}

pub(in crate::queue) trait DataInsertionRootV1<P> {
    const LEDGER_OPERATION: &'static str;
    fn completed_identity(
        &self,
    ) -> Result<Gfx942FixedDispatchStorageIdentityV1, MemorySessionError>;
    fn take_data(&mut self) -> Result<Gfx942FixedDispatchDataV1, MemorySessionError>;
    fn prepare(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        parameters: P,
    ) -> Result<(), MemorySessionError>;
    fn requires_retention(&self) -> bool;
    fn retain(self, memory: &mut SharedGttMemorySessionV1);
}

impl DataInsertionRootV1<u64> for DeviceInitializationCustodyV1 {
    const LEDGER_OPERATION: &'static str = "detached initialized-device identity ledger";

    fn completed_identity(
        &self,
    ) -> Result<Gfx942FixedDispatchStorageIdentityV1, MemorySessionError> {
        Ok(
            Gfx942FixedDispatchStorageIdentityV1::DeviceInitializedContent(
                self.completed()?.storage_identity(),
            ),
        )
    }

    fn take_data(&mut self) -> Result<Gfx942FixedDispatchDataV1, MemorySessionError> {
        self.take_complete()
            .map(Gfx942FixedDispatchDataV1::initialized)
    }

    fn prepare(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        alignment: u64,
    ) -> Result<(), MemorySessionError> {
        memory.prepare_device_initialization_in_place(self, alignment)
    }

    fn requires_retention(&self) -> bool {
        self.requires_retention()
    }

    fn retain(self, memory: &mut SharedGttMemorySessionV1) {
        memory.retain_device_initialization_failure(self);
    }
}

impl DataInsertionRootV1<&[u8]> for CoherentInitializationCustodyV1 {
    const LEDGER_OPERATION: &'static str = "detached initialized-coherent identity ledger";

    fn completed_identity(
        &self,
    ) -> Result<Gfx942FixedDispatchStorageIdentityV1, MemorySessionError> {
        Ok(
            Gfx942FixedDispatchStorageIdentityV1::HostVisibleInitialized(
                self.completed()?.storage_identity(),
            ),
        )
    }

    fn take_data(&mut self) -> Result<Gfx942FixedDispatchDataV1, MemorySessionError> {
        self.take_complete()
            .map(Gfx942FixedDispatchDataV1::host_visible_initialized)
    }

    fn prepare(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        source: &[u8],
    ) -> Result<(), MemorySessionError> {
        memory.prepare_coherent_initialization_in_place(self, source)
    }

    fn requires_retention(&self) -> bool {
        self.requires_retention()
    }

    fn retain(self, memory: &mut SharedGttMemorySessionV1) {
        memory.retain_coherent_initialization_failure(self);
    }
}

impl DataInsertionRootV1<usize> for CoherentAllocationCustodyV1 {
    const LEDGER_OPERATION: &'static str = "detached uninitialized-coherent identity ledger";

    fn completed_identity(
        &self,
    ) -> Result<Gfx942FixedDispatchStorageIdentityV1, MemorySessionError> {
        Ok(
            Gfx942FixedDispatchStorageIdentityV1::HostVisibleUninitialized(
                self.completed()?.storage_identity(),
            ),
        )
    }

    fn take_data(&mut self) -> Result<Gfx942FixedDispatchDataV1, MemorySessionError> {
        self.take_complete()
            .map(Gfx942FixedDispatchDataV1::host_visible_uninitialized)
    }

    fn prepare(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        requested_bytes: usize,
    ) -> Result<(), MemorySessionError> {
        memory.prepare_coherent_allocation_in_place(self, requested_bytes)
    }

    fn requires_retention(&self) -> bool {
        self.requires_retention()
    }

    fn retain(self, memory: &mut SharedGttMemorySessionV1) {
        memory.retain_coherent_allocation_failure(self);
    }
}

pub(in crate::queue) trait DataInsertionContextV1<R: DataInsertionRootV1<P>, P> {
    fn require_unbound(&self) -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn ledger(&mut self) -> DetachedInsertionLedgerV1<'_>;
    fn reserve(&mut self) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        reserve_identity_capacity_v1(self.ledger().identities, R::LEDGER_OPERATION)
    }
    fn prepare(&mut self, root: &mut R, parameters: P)
    -> Result<(), ComputeAqlQueueSessionErrorV1>;
    fn fail(&mut self, root: R, panicked: bool);
}

pub(in crate::queue) fn reserve_identity_capacity_v1(
    identities: &mut Vec<Gfx942FixedDispatchStorageIdentityV1>,
    operation: &'static str,
) -> Result<(), ComputeAqlQueueSessionErrorV1> {
    let additional = super::super::dispatch_binding::MAX_DISPATCH_DATA_LEASES_V1
        .checked_sub(identities.len())
        .ok_or(ComputeAqlQueueSessionErrorV1::Contract(
            "detached dispatch-data ledger bound",
        ))?;
    identities
        .try_reserve_exact(additional)
        .map_err(|_| Gfx942DispatchBindingErrorV1::HostAllocationCapacity { operation }.into())
}

pub(in crate::queue) struct SettledDataInsertionV1 {
    pub(in crate::queue) result:
        std::thread::Result<Result<Gfx942FixedDispatchDataV1, ComputeAqlQueueSessionErrorV1>>,
    pub(in crate::queue) transport: bool,
}

impl SettledDataInsertionV1 {
    pub(super) fn into_result(
        self,
    ) -> Result<Gfx942FixedDispatchDataV1, ComputeAqlQueueSessionErrorV1> {
        match self.result {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
}

pub(in crate::queue) fn settle_data_insertion_v1<C, R, P>(
    context: &mut C,
    mut root: R,
    data_index: DataInsertionIndexV1,
    parameters: P,
) -> SettledDataInsertionV1
where
    R: DataInsertionRootV1<P>,
    C: DataInsertionContextV1<R, P>,
{
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
        let index = match data_index {
            DataInsertionIndexV1::Explicit(index) => index,
            DataInsertionIndexV1::HoleOrAppend => ledger.next.unwrap_or(ledger.identities.len()),
            DataInsertionIndexV1::RequiredHole => ledger
                .next
                .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?,
        };
        validate_new_detached_data_index(*ledger.count, index)?;
        context.reserve()?;
        entered = true;
        context.prepare(&mut root, parameters)?;
        let identity = root.completed_identity()?;
        let ledger = context.ledger();
        insert_detached_identity_at(ledger.identities, ledger.next, identity, index);
        *ledger.count = next_count;
        let data = root
            .take_data()
            .expect("borrowed Complete remains rooted through ledger commit");
        Ok(data)
    }));
    let transport = result.is_err() || entered && !matches!(result, Ok(Ok(_)));
    if transport {
        context.fail(root, result.is_err());
    }
    SettledDataInsertionV1 { result, transport }
}

impl<R: DataInsertionRootV1<P>, P> DataInsertionContextV1<R, P> for ComputeAqlQueueSessionV1 {
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
        root: &mut R,
        parameters: P,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        self.with_live_queue_memory_model(|memory| {
            root.prepare(memory, parameters).map_err(Into::into)
        })
    }

    fn fail(&mut self, root: R, panicked: bool) {
        self.poison_terminal();
        if panicked {
            poison_process_global_after_dispatch_terminal_v1();
        }
        if root.requires_retention() {
            if let Some(engine) = self.engine.as_mut() {
                root.retain(&mut engine.backend.session);
            } else {
                // Admitted preparation cannot remove its engine. Preserve
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
    ) -> SettledDataInsertionV1 {
        settle_data_insertion_v1(
            self,
            DeviceInitializationCustodyV1::new(bytes, content),
            data_index.map_or(
                DataInsertionIndexV1::HoleOrAppend,
                DataInsertionIndexV1::Explicit,
            ),
            alignment,
        )
    }

    pub(super) fn initialize_coherent_data_settled_v1(
        &mut self,
        data_index: Option<usize>,
        bytes: &[u8],
    ) -> SettledDataInsertionV1 {
        settle_data_insertion_v1(
            self,
            CoherentInitializationCustodyV1::new(),
            data_index.map_or(
                DataInsertionIndexV1::RequiredHole,
                DataInsertionIndexV1::Explicit,
            ),
            bytes,
        )
    }

    pub(super) fn allocate_coherent_data_settled_v1(
        &mut self,
        data_index: Option<usize>,
        requested_bytes: usize,
    ) -> SettledDataInsertionV1 {
        settle_data_insertion_v1(
            self,
            CoherentAllocationCustodyV1::new(),
            data_index.map_or(
                DataInsertionIndexV1::RequiredHole,
                DataInsertionIndexV1::Explicit,
            ),
            requested_bytes,
        )
    }
}
