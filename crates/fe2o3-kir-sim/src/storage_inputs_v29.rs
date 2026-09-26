//! Explicit images are simulation inputs, not source or host ABI authority.

use crate::{
    BufferArgumentV1, BufferBackingIdV1, EventPolicyV1, GridShapeV1, SimulationArgumentV1,
    WorkgroupShapeV1,
};
use fe2o3_kernel_ir::{AccessMode, AddressSpace, KernelId, ScalarType, StorageLayoutIdV1};

/// An allocation in this request. Numeric machine addresses are not accepted.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SimulationInputOriginV29 {
    Argument(u32),
    Backing(BufferBackingIdV1),
}

/// A containment edge in the exact owner's physical layout table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationObjectComponentV29 {
    Field(u32),
    Index(u64),
    Variant(u32),
    Tag,
}

/// A checked array or scalar-buffer interval, measured in elements.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimulationObjectRangeV29 {
    pub start: u64,
    pub elements: u64,
}

/// A referent recipe. Execution checks its layout, bounds and every selector.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulationObjectViewV29 {
    pub origin: SimulationInputOriginV29,
    pub path: Vec<SimulationObjectComponentV29>,
    pub layout: StorageLayoutIdV1,
    pub range: Option<SimulationObjectRangeV29>,
    pub access: AccessMode,
}

/// One complete stored pointer, never a partial byte patch or raw address.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulationObjectRelocationV29 {
    pub path: Vec<SimulationObjectComponentV29>,
    pub pointer_layout: StorageLayoutIdV1,
    pub referent: SimulationObjectViewV29,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationObjectImageErrorV29 {
    InvalidAlignment,
    InitializationLength,
}

/// Bytes and initialized-byte mask for one exact current-owner layout.
/// Padding and inactive payload bytes need not be initialized. Relocations
/// must be ordered by their resolved byte offset; admission checks the order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulationObjectImageV29 {
    layout: StorageLayoutIdV1,
    alignment: u32,
    bytes: Vec<u8>,
    initialized: Vec<bool>,
    relocations: Vec<SimulationObjectRelocationV29>,
}

impl SimulationObjectImageV29 {
    pub fn new(
        layout: StorageLayoutIdV1,
        alignment: u32,
        bytes: Vec<u8>,
        initialized: Vec<bool>,
        relocations: Vec<SimulationObjectRelocationV29>,
    ) -> Result<Self, SimulationObjectImageErrorV29> {
        if !alignment.is_power_of_two() {
            return Err(SimulationObjectImageErrorV29::InvalidAlignment);
        }
        if bytes.len() != initialized.len() {
            return Err(SimulationObjectImageErrorV29::InitializationLength);
        }
        Ok(Self {
            layout,
            alignment,
            bytes,
            initialized,
            relocations,
        })
    }

    pub const fn layout(&self) -> StorageLayoutIdV1 {
        self.layout
    }
    pub const fn alignment(&self) -> u32 {
        self.alignment
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn initialized(&self) -> &[bool] {
        &self.initialized
    }
    pub fn relocations(&self) -> &[SimulationObjectRelocationV29] {
        &self.relocations
    }

    pub(crate) fn retained_payload_capacity_bytes(&self) -> Option<usize> {
        let mut bytes = self
            .bytes
            .capacity()
            .checked_add(self.initialized.capacity())?
            .checked_add(
                self.relocations
                    .capacity()
                    .checked_mul(size_of::<SimulationObjectRelocationV29>())?,
            )?;
        for relocation in &self.relocations {
            bytes = bytes
                .checked_add(
                    relocation
                        .path
                        .capacity()
                        .checked_mul(size_of::<SimulationObjectComponentV29>())?,
                )?
                .checked_add(
                    relocation
                        .referent
                        .path
                        .capacity()
                        .checked_mul(size_of::<SimulationObjectComponentV29>())?,
                )?;
        }
        Some(bytes)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SimulationStorageArgumentV29 {
    Existing(SimulationArgumentV1),
    /// A Constant, read-only by-value root region, not a Private host pointer.
    InlineObject(SimulationObjectImageV29),
    ObjectView(SimulationObjectViewV29),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SimulationStorageBackingV29 {
    Scalar(BufferArgumentV1),
    Object {
        image: SimulationObjectImageV29,
        access: AccessMode,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulationSharedStorageV29 {
    pub id: BufferBackingIdV1,
    pub storage: SimulationStorageBackingV29,
}

/// An explicit simulator request. This is not a host launch or replay profile.
///
/// ```compile_fail
/// use fe2o3_kir_sim::{SimulationRequestV1, SimulationStorageRequestV29};
/// fn legacy(input: SimulationStorageRequestV29) -> SimulationRequestV1 { input }
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulationStorageRequestV29 {
    pub kernel: KernelId,
    pub grid: GridShapeV1,
    pub workgroup: WorkgroupShapeV1,
    pub arguments: Vec<SimulationStorageArgumentV29>,
    pub shared_storage: Vec<SimulationSharedStorageV29>,
    pub events: EventPolicyV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SimulationObservedPointeeV29 {
    Scalar(ScalarType),
    Object(StorageLayoutIdV1),
}

/// An observed symbolic pointer. It intentionally carries no input path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulationPointerObservationV29 {
    pub origin: SimulationInputOriginV29,
    pub pointee: SimulationObservedPointeeV29,
    pub address_space: AddressSpace,
    pub access: AccessMode,
    pub byte_offset: usize,
    pub lower_bound: usize,
    pub upper_bound: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulationRelocationObservationV29 {
    pub byte_offset: usize,
    pub byte_width: usize,
    pub representation: fe2o3_kernel_ir::StoragePointerV1,
    pub pointer: SimulationPointerObservationV29,
}

/// A CPU observation, deliberately not a replayable object image.
///
/// ```compile_fail
/// use fe2o3_kir_sim::{SimulationObjectImageV29, SimulationObjectObservationV29};
/// fn replay(output: SimulationObjectObservationV29) -> SimulationObjectImageV29 { output }
/// ```
#[derive(Debug)]
pub struct SimulationObjectObservationV29 {
    pub layout: StorageLayoutIdV1,
    pub alignment: u32,
    pub bytes: Vec<u8>,
    pub initialized: Vec<bool>,
    pub relocations: Vec<SimulationRelocationObservationV29>,
}

#[derive(Debug)]
pub enum SimulationStorageArgumentObservationV29 {
    Existing(SimulationArgumentV1),
    InlineObject(SimulationObjectObservationV29),
    ObjectView {
        pointer: SimulationPointerObservationV29,
        elements: Option<u64>,
    },
}

#[derive(Debug)]
pub enum SimulationStorageBackingObservationV29 {
    Scalar(BufferArgumentV1),
    Object {
        image: SimulationObjectObservationV29,
        access: AccessMode,
    },
}

#[derive(Debug)]
pub struct SimulationSharedStorageObservationV29 {
    pub id: BufferBackingIdV1,
    pub storage: SimulationStorageBackingObservationV29,
}
