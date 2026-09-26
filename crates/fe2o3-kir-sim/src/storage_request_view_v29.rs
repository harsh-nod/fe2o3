//! Closed borrowed request profiles. Neither branch reconstructs the other.

use crate::resident::{ResidentLedger, reserved_bool_vec_bytes, reserved_vec_bytes};
use crate::storage_inputs_v29::*;
use crate::{
    BufferArgumentV1, BufferBackingIdV1, EventPolicyV1, GridShapeV1, SharedBufferV1,
    SimulationArgumentV1, SimulationRequestV1, WorkgroupShapeV1,
};
use fe2o3_kernel_ir::{AccessMode, KernelId};

#[derive(Clone, Copy)]
pub(crate) enum SimulationRequestRefV29<'a> {
    Legacy(&'a SimulationRequestV1),
    Storage(&'a SimulationStorageRequestV29),
}

#[derive(Clone, Copy)]
pub(crate) enum SimulationArgumentRefV29<'a> {
    Existing(&'a SimulationArgumentV1),
    InlineObject(&'a SimulationObjectImageV29),
    ObjectView(&'a SimulationObjectViewV29),
}

#[derive(Clone, Copy)]
pub(crate) enum SimulationBackingRefV29<'a> {
    Scalar(&'a BufferArgumentV1),
    Object {
        image: &'a SimulationObjectImageV29,
        access: AccessMode,
    },
}

impl SimulationBackingRefV29<'_> {
    pub(crate) fn bytes(self) -> usize {
        match self {
            Self::Scalar(buffer) => buffer.bytes().len(),
            Self::Object { image, .. } => image.bytes().len(),
        }
    }

    pub(crate) fn payload_capacity(self) -> Option<usize> {
        match self {
            Self::Scalar(buffer) => buffer.retained_payload_capacity_bytes(),
            Self::Object { image, .. } => image.retained_payload_capacity_bytes(),
        }
    }
}

impl<'a> SimulationRequestRefV29<'a> {
    pub(crate) fn legacy(self) -> Option<&'a SimulationRequestV1> {
        match self {
            Self::Legacy(request) => Some(request),
            Self::Storage(_) => None,
        }
    }

    pub(crate) fn kernel(self) -> &'a KernelId {
        match self {
            Self::Legacy(r) => &r.kernel,
            Self::Storage(r) => &r.kernel,
        }
    }

    pub(crate) fn grid(self) -> GridShapeV1 {
        match self {
            Self::Legacy(r) => r.grid,
            Self::Storage(r) => r.grid,
        }
    }

    pub(crate) fn workgroup(self) -> WorkgroupShapeV1 {
        match self {
            Self::Legacy(r) => r.workgroup,
            Self::Storage(r) => r.workgroup,
        }
    }

    pub(crate) fn events(self) -> EventPolicyV1 {
        match self {
            Self::Legacy(r) => r.events,
            Self::Storage(r) => r.events,
        }
    }

    pub(crate) fn argument_count(self) -> usize {
        match self {
            Self::Legacy(r) => r.arguments.len(),
            Self::Storage(r) => r.arguments.len(),
        }
    }

    pub(crate) fn backing_count(self) -> usize {
        match self {
            Self::Legacy(r) => r.shared_buffers.len(),
            Self::Storage(r) => r.shared_storage.len(),
        }
    }

    pub(crate) fn argument(self, index: usize) -> Option<SimulationArgumentRefV29<'a>> {
        match self {
            Self::Legacy(r) => r
                .arguments
                .get(index)
                .map(SimulationArgumentRefV29::Existing),
            Self::Storage(r) => r.arguments.get(index).map(|argument| match argument {
                SimulationStorageArgumentV29::Existing(value) => {
                    SimulationArgumentRefV29::Existing(value)
                }
                SimulationStorageArgumentV29::InlineObject(image) => {
                    SimulationArgumentRefV29::InlineObject(image)
                }
                SimulationStorageArgumentV29::ObjectView(view) => {
                    SimulationArgumentRefV29::ObjectView(view)
                }
            }),
        }
    }

    pub(crate) fn backing(
        self,
        index: usize,
    ) -> Option<(BufferBackingIdV1, SimulationBackingRefV29<'a>)> {
        match self {
            Self::Legacy(r) => r
                .shared_buffers
                .get(index)
                .map(|shared| (shared.id, SimulationBackingRefV29::Scalar(&shared.buffer))),
            Self::Storage(r) => r.shared_storage.get(index).map(|shared| {
                (
                    shared.id,
                    match &shared.storage {
                        SimulationStorageBackingV29::Scalar(buffer) => {
                            SimulationBackingRefV29::Scalar(buffer)
                        }
                        SimulationStorageBackingV29::Object { image, access } => {
                            SimulationBackingRefV29::Object {
                                image,
                                access: *access,
                            }
                        }
                    },
                )
            }),
        }
    }

    pub(crate) fn arguments(self) -> impl Iterator<Item = SimulationArgumentRefV29<'a>> {
        (0..self.argument_count()).filter_map(move |index| self.argument(index))
    }

    pub(crate) fn backings(
        self,
    ) -> impl Iterator<Item = (BufferBackingIdV1, SimulationBackingRefV29<'a>)> {
        (0..self.backing_count()).filter_map(move |index| self.backing(index))
    }

    pub(crate) fn allocations(self) -> impl Iterator<Item = SimulationBackingRefV29<'a>> {
        self.arguments()
            .filter_map(|argument| match argument {
                SimulationArgumentRefV29::Existing(SimulationArgumentV1::Buffer(buffer)) => {
                    Some(SimulationBackingRefV29::Scalar(buffer))
                }
                SimulationArgumentRefV29::InlineObject(image) => {
                    Some(SimulationBackingRefV29::Object {
                        image,
                        access: AccessMode::ReadOnly,
                    })
                }
                _ => None,
            })
            .chain(self.backings().map(|(_, backing)| backing))
    }

    /// Counts request-owned capacities only; interpreter tables are separate.
    pub(crate) fn retain_input(self, resident: &mut ResidentLedger) -> Option<()> {
        match self {
            Self::Legacy(r) => {
                resident.add_bytes(size_of::<SimulationRequestV1>())?;
                resident.add_vec::<SimulationArgumentV1>(r.arguments.capacity())?;
                resident.add_vec::<SharedBufferV1>(r.shared_buffers.capacity())?;
            }
            Self::Storage(r) => {
                resident.add_bytes(size_of::<SimulationStorageRequestV29>())?;
                resident.add_vec::<SimulationStorageArgumentV29>(r.arguments.capacity())?;
                resident.add_vec::<SimulationSharedStorageV29>(r.shared_storage.capacity())?;
                for argument in &r.arguments {
                    if let SimulationStorageArgumentV29::ObjectView(view) = argument {
                        resident.add_vec::<SimulationObjectComponentV29>(view.path.capacity())?;
                    }
                }
            }
        }
        resident.add_bytes(self.kernel().retained_capacity_bytes())?;
        for allocation in self.allocations() {
            resident.add_bytes(allocation.payload_capacity()?)?;
        }
        Some(())
    }

    pub(crate) fn retain_output(self, resident: &mut ResidentLedger) -> Option<()> {
        match self {
            Self::Legacy(_) => {
                resident.add_bytes(reserved_vec_bytes::<SimulationArgumentV1>(
                    self.argument_count(),
                )?)?;
                resident.add_bytes(reserved_vec_bytes::<SharedBufferV1>(self.backing_count())?)?;
            }
            Self::Storage(_) => {
                resident.add_bytes(
                    reserved_vec_bytes::<SimulationStorageArgumentObservationV29>(
                        self.argument_count(),
                    )?,
                )?;
                resident.add_bytes(reserved_vec_bytes::<SimulationSharedStorageObservationV29>(
                    self.backing_count(),
                )?)?;
            }
        }
        for allocation in self.allocations() {
            resident.add_bytes(reserved_vec_bytes::<u8>(allocation.bytes())?)?;
            resident.add_bytes(reserved_bool_vec_bytes(allocation.bytes())?)?;
        }
        // Runtime relocation observations are charged by the consumed storage
        // ledger at export, including newly stored pointer values.
        Some(())
    }
}
