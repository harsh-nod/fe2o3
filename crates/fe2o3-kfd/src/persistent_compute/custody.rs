//! Persistent compute retained-input and terminal-custody accessors.

use super::*;

impl Gfx942PersistentComputeInputV1 {
    pub const fn is_fully_initialized(&self) -> bool {
        !matches!(self, Self::Uninitialized(_))
    }

    pub(crate) fn belongs_to(&self, queue: QueueKeyV1) -> bool {
        match self {
            Self::Uninitialized(allocation) => allocation.attachment.queue == queue,
            Self::Initialized(ready) => ready.allocation.attachment.queue == queue,
            Self::InitializedAfterDispatch(ready) => ready.allocation.attachment.queue == queue,
            Self::InitializedStorage(ready) => ready.allocation.attachment.queue == queue,
        }
    }

    pub const fn byte_len(&self) -> u64 {
        match self {
            Self::Uninitialized(allocation) => allocation.byte_len(),
            Self::Initialized(ready) => ready.byte_len(),
            Self::InitializedAfterDispatch(allocation) => allocation.byte_len(),
            Self::InitializedStorage(allocation) => allocation.byte_len(),
        }
    }

    pub const fn physical_byte_len(&self) -> u64 {
        match self {
            Self::Uninitialized(allocation) => allocation.physical_byte_len(),
            Self::Initialized(ready) => ready.physical_byte_len(),
            Self::InitializedAfterDispatch(allocation) => allocation.physical_byte_len(),
            Self::InitializedStorage(allocation) => allocation.physical_byte_len(),
        }
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        Gfx942DirectionalQueuePersistentAllocationV1,
        PersistentComputeInitializationV1,
    ) {
        match self {
            Self::Uninitialized(allocation) => {
                (allocation, PersistentComputeInitializationV1::Uninitialized)
            }
            Self::Initialized(ready) => (
                ready.allocation,
                PersistentComputeInitializationV1::AuthenticatedH2d(ready.authenticated_sha256),
            ),
            Self::InitializedAfterDispatch(ready) => (
                ready.allocation,
                PersistentComputeInitializationV1::AfterDispatch,
            ),
            Self::InitializedStorage(ready) => (
                ready.allocation,
                PersistentComputeInitializationV1::InitializedStorage,
            ),
        }
    }

    pub(crate) fn from_parts(
        allocation: Gfx942DirectionalQueuePersistentAllocationV1,
        initialization: PersistentComputeInitializationV1,
    ) -> Self {
        match initialization {
            PersistentComputeInitializationV1::AuthenticatedH2d(authenticated_sha256) => {
                Self::Initialized(Gfx942PersistentComputeReadyV1 {
                    allocation,
                    authenticated_sha256,
                })
            }
            PersistentComputeInitializationV1::AfterDispatch => {
                Self::InitializedAfterDispatch(Gfx942PersistentComputeInitializedAfterDispatchV1 {
                    allocation,
                })
            }
            PersistentComputeInitializationV1::Uninitialized => Self::Uninitialized(allocation),
            PersistentComputeInitializationV1::InitializedStorage => {
                Self::InitializedStorage(Gfx942PersistentComputeInitializedStorageV1 { allocation })
            }
        }
    }

    pub fn into_allocation(self) -> Gfx942DirectionalQueuePersistentAllocationV1 {
        match self {
            Self::Uninitialized(allocation) => allocation,
            Self::InitializedAfterDispatch(ready) => ready.allocation,
            Self::InitializedStorage(ready) => ready.allocation,
            Self::Initialized(ready) => ready.allocation,
        }
    }
}

impl Gfx942PersistentComputeCompletedV1 {
    pub const fn effect(&self) -> Gfx942PersistentComputeEffectV1 {
        self.effect
    }

    pub fn into_parts(
        self,
    ) -> (
        Gfx942DirectionalQueuePersistentAllocationV1,
        Gfx942PersistentDependencyFrontierV1,
        Gfx942PersistentComputeEffectV1,
    ) {
        (self.allocation, self.frontier, self.effect)
    }

    /// Retires the exact completed frontier and preserves the strongest valid
    /// initialization proof for an exact persistent-control replay.
    #[allow(clippy::result_large_err)]
    pub fn retire_settled_frontier_for_replay_v1(
        self,
    ) -> Result<
        (
            Gfx942PersistentComputeInputV1,
            Gfx942PersistentComputeEffectV1,
        ),
        Gfx942DirectionalPersistentSdmaFrontierRetirementFailureV1,
    > {
        let allocation = self.allocation.retire_settled_frontier_v1(self.frontier)?;
        let authenticated_sha256 =
            replay_authenticated_sha256_v1(self.effect, self.authenticated_sha256);
        let input = match (authenticated_sha256, self.fully_initialized) {
            (Some(authenticated_sha256), true) => {
                Gfx942PersistentComputeInputV1::Initialized(Gfx942PersistentComputeReadyV1 {
                    allocation,
                    authenticated_sha256,
                })
            }
            (None, true) => Gfx942PersistentComputeInputV1::InitializedAfterDispatch(
                Gfx942PersistentComputeInitializedAfterDispatchV1 { allocation },
            ),
            (_, false) => Gfx942PersistentComputeInputV1::Uninitialized(allocation),
        };
        Ok((input, self.effect))
    }
}

impl PersistentComputeTerminalDataV1 {
    pub(crate) fn from_vec(data: Vec<Gfx942FixedDispatchDataV1>) -> Self {
        if data.len() > MAX_DISPATCH_DATA_LEASES_V1 {
            std::process::abort();
        }
        let mut inline = ArrayVec::new();
        for owner in data {
            if inline.try_push(owner).is_err() {
                std::process::abort();
            }
        }
        Self { data: inline }
    }

    pub(crate) fn from_one(data: Gfx942FixedDispatchDataV1) -> Self {
        let mut inline = ArrayVec::new();
        inline.push(data);
        Self { data: inline }
    }

    pub(crate) fn from_three(data: [Gfx942FixedDispatchDataV1; 3]) -> Self {
        let mut inline = ArrayVec::new();
        inline.extend(data);
        Self { data: inline }
    }

    pub(crate) const fn len(&self) -> usize {
        self.data.len()
    }

    #[cfg(test)]
    pub(crate) fn into_data_for_test(self) -> impl Iterator<Item = Gfx942FixedDispatchDataV1> {
        self.data.into_iter()
    }
}

impl PersistentComputeTerminalNativeCustodyV1 {
    pub(crate) fn stage(&self) -> Option<Gfx942PersistentComputeTerminalStageV1> {
        Some(match self {
            Self::Preparation(preparation) => {
                let _ = core::mem::size_of_val(preparation);
                Gfx942PersistentComputeTerminalStageV1::Preparing
            }
            Self::Attached => Gfx942PersistentComputeTerminalStageV1::Attached,
            Self::Published(batch) => {
                let _ = core::mem::size_of_val(batch);
                Gfx942PersistentComputeTerminalStageV1::Published
            }
            Self::Completed(completed) => {
                let _ = core::mem::size_of_val(completed);
                Gfx942PersistentComputeTerminalStageV1::Completed
            }
            Self::Recycled(observation) => {
                let _ = observation.packet_count();
                Gfx942PersistentComputeTerminalStageV1::Recycled
            }
            Self::Data(data) => {
                let _ = data.len();
                Gfx942PersistentComputeTerminalStageV1::DataDetached
            }
            Self::Storage(storage) => {
                let _ = core::mem::size_of_val(storage);
                Gfx942PersistentComputeTerminalStageV1::StorageDetached
            }
            Self::Restored => Gfx942PersistentComputeTerminalStageV1::Restored,
            Self::Cancellation(custody) => return custody.stage(),
        })
    }
}

impl BoundedPersistentComputeAttachmentV1 {
    pub(crate) fn from_single(attachment: PersistentComputeAttachmentV1) -> Self {
        let mut entries = ArrayVec::new();
        entries.push(PersistentComputeAttachmentEntryV1 {
            allocation: attachment.allocation,
            initialization: attachment.initialization,
            state: attachment.state,
            storage_identity: Some(attachment.storage_identity),
            effect: attachment.effect,
        });
        Self {
            entries,
            binding: attachment.binding,
            predecessor_dispatch_generation: attachment.predecessor_dispatch_generation,
            terminal_custody: attachment.terminal_custody,
        }
    }

    pub(crate) fn from_three(attachment: ThreeBindingPersistentComputeAttachmentV1) -> Self {
        let mut entries = ArrayVec::new();
        entries.extend(attachment.entries);
        Self {
            entries,
            binding: attachment.binding,
            predecessor_dispatch_generation: attachment.predecessor_dispatch_generation,
            terminal_custody: attachment.terminal_custody,
        }
    }

    pub(crate) const fn is_single(&self) -> bool {
        self.entries.len() == 1
    }

    pub(crate) const fn is_three(&self) -> bool {
        self.entries.len() == 3
    }

    pub(crate) fn single_entry(&self) -> Option<&PersistentComputeAttachmentEntryV1> {
        self.is_single().then(|| &self.entries[0])
    }

    #[cfg(test)]
    pub(crate) fn single_entry_mut(&mut self) -> Option<&mut PersistentComputeAttachmentEntryV1> {
        self.is_single().then(|| &mut self.entries[0])
    }

    #[allow(clippy::result_large_err)]
    pub(crate) fn into_single(mut self) -> Result<PersistentComputeAttachmentV1, Self> {
        if !self.is_single() {
            return Err(self);
        }
        let entry = self.entries.pop().expect("validated single entry");
        Ok(PersistentComputeAttachmentV1 {
            allocation: entry.allocation,
            initialization: entry.initialization,
            state: entry.state,
            binding: self.binding,
            storage_identity: entry
                .storage_identity
                .expect("single attachment storage identity"),
            effect: entry.effect,
            predecessor_dispatch_generation: self.predecessor_dispatch_generation,
            terminal_custody: self.terminal_custody,
        })
    }

    #[allow(clippy::result_large_err)]
    pub(crate) fn into_three(mut self) -> Result<ThreeBindingPersistentComputeAttachmentV1, Self> {
        if !self.is_three() {
            return Err(self);
        }
        let c = self.entries.pop().expect("validated C entry");
        let b = self.entries.pop().expect("validated B entry");
        let a = self.entries.pop().expect("validated A entry");
        Ok(ThreeBindingPersistentComputeAttachmentV1 {
            entries: [a, b, c],
            binding: self.binding,
            predecessor_dispatch_generation: self.predecessor_dispatch_generation,
            terminal_custody: self.terminal_custody,
        })
    }

    pub(crate) const fn terminal_custody(
        &self,
    ) -> Option<&PersistentComputeTerminalNativeCustodyV1> {
        self.terminal_custody.as_ref()
    }
}
