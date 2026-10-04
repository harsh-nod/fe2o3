//! In-process allocator observations. Numeric identities are scoped to one run.
//! These DTOs are not serialized authority and cannot be used as simulated pointers.
use fe2o3_kernel_ir::{AccessMode, AddressSpace};

use crate::{SimulationEventSiteV1, SimulationInvocationV1};

/// Fresh semantic allocation and its actual reusable backing-storage incarnation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SimulationAllocationStorageIdentityV1 {
    allocation: u64,
    storage_slot: u64,
    generation: u64,
}

impl SimulationAllocationStorageIdentityV1 {
    pub const fn allocation(self) -> u64 {
        self.allocation
    }
    pub const fn storage_slot(self) -> u64 {
        self.storage_slot
    }
    pub const fn generation(self) -> u64 {
        self.generation
    }

    pub(crate) const fn new(allocation: u64, storage_slot: u64, generation: u64) -> Self {
        Self {
            allocation,
            storage_slot,
            generation,
        }
    }
}

/// Actual creation scope; an invocation does not assert a frame activation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationAllocationScopeV1 {
    Dispatch,
    Workgroup {
        coordinate: [u64; 3],
        size: [u32; 3],
        count: [u64; 3],
        launch: [u64; 3],
    },
    Invocation(SimulationInvocationV1),
}

/// Immutable creation facts. Release retains the original creation site.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimulationAllocationDescriptorV1 {
    identity: SimulationAllocationStorageIdentityV1,
    address_space: AddressSpace,
    access: AccessMode,
    alignment: u32,
    byte_len: u64,
    scope: SimulationAllocationScopeV1,
    creation_site: Option<SimulationEventSiteV1>,
}

impl SimulationAllocationDescriptorV1 {
    pub const fn identity(self) -> SimulationAllocationStorageIdentityV1 {
        self.identity
    }
    pub const fn address_space(self) -> AddressSpace {
        self.address_space
    }
    pub const fn access(self) -> AccessMode {
        self.access
    }
    pub const fn alignment(self) -> u32 {
        self.alignment
    }
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
    pub const fn scope(self) -> SimulationAllocationScopeV1 {
        self.scope
    }
    pub const fn creation_site(self) -> Option<SimulationEventSiteV1> {
        self.creation_site
    }

    pub(crate) fn new(
        identity: SimulationAllocationStorageIdentityV1,
        shape: (AddressSpace, AccessMode, u32, u64),
        scope: SimulationAllocationScopeV1,
        creation_site: Option<SimulationEventSiteV1>,
    ) -> Self {
        Self {
            identity,
            address_space: shape.0,
            access: shape.1,
            alignment: shape.2,
            byte_len: shape.3,
            scope,
            creation_site,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationAllocationTransitionKindV1 {
    Preexisting,
    /// A predecessor exists only when the allocator moves the actual cached vectors.
    Create {
        previous_allocation: Option<u64>,
    },
    Release,
}

/// One successfully committed mutation, in a contiguous run-local sequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimulationAllocationTransitionV1 {
    sequence: u64,
    descriptor: SimulationAllocationDescriptorV1,
    kind: SimulationAllocationTransitionKindV1,
}

impl SimulationAllocationTransitionV1 {
    pub const fn sequence(self) -> u64 {
        self.sequence
    }
    pub const fn descriptor(self) -> SimulationAllocationDescriptorV1 {
        self.descriptor
    }
    pub const fn kind(self) -> SimulationAllocationTransitionKindV1 {
        self.kind
    }

    pub(crate) const fn new(
        sequence: u64,
        descriptor: SimulationAllocationDescriptorV1,
        kind: SimulationAllocationTransitionKindV1,
    ) -> Self {
        Self {
            sequence,
            descriptor,
            kind,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationAllocationObservationUnavailableV1 {
    IdentityInvariant,
    SequenceOverflow,
    ObservationStopped,
}

/// Last allocator transition before this exact debug-record delivery.
/// An unavailable watermark must not be treated as an observed complete prefix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationAllocationWatermarkV1 {
    NotRequested,
    PolicyDisabled,
    Available {
        through_sequence: u64,
    },
    Unavailable {
        reason: SimulationAllocationObservationUnavailableV1,
    },
}
