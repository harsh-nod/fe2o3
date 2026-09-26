//! Metered containment geometry, before any executable pointer is published.

use super::*;
use crate::storage_inputs_v29::*;
use fe2o3_kernel_ir::{StorageLayoutIdV1, StorageLayoutKindV1, StorageLayoutV1};

#[derive(Clone, Copy)]
pub(super) enum StorageInputRepresentationV29 {
    Scalar(ScalarType),
    Object(StorageLayoutIdV1),
}

#[derive(Clone, Copy)]
pub(super) struct StorageInputAllocationV29 {
    pub(super) origin: SimulationInputOriginV29,
    pub(super) representation: StorageInputRepresentationV29,
}

#[derive(Clone, Copy)]
pub(super) struct StorageInputPositionV29 {
    pub(super) allocation: u64,
    pub(super) layout: StorageLayoutIdV1,
    pub(super) start: usize,
    pub(super) end: usize,
}

#[derive(Clone, Copy)]
pub(super) struct StorageInputSelectorV29 {
    pub(super) parent: StorageInputPositionV29,
    pub(super) variant: u32,
}

#[derive(Clone, Copy)]
pub(super) struct StorageInputReferentV29 {
    pub(super) position: StorageInputPositionV29,
    pub(super) address_space: AddressSpace,
    pub(super) access: AccessMode,
    pub(super) elements: Option<usize>,
}

pub(super) fn storage_input_row_v29<'a>(
    owner: &'a fe2o3_kernel_ir::VerifiedStorageKernelIrModuleV1<'_>,
    accounting: &StorageAccountingV1,
    layout: StorageLayoutIdV1,
) -> Result<&'a StorageLayoutV1, SimulationExecutionErrorKindV1> {
    accounting.charge(1)?;
    owner
        .storage()
        .layouts()
        .row(layout)
        .ok_or(storage_violation_v1(
            "input layout is absent from the exact owner",
        ))
}

fn storage_input_extent_v29(size: u64) -> Result<usize, SimulationExecutionErrorKindV1> {
    usize::try_from(size).map_err(|_| SimulationExecutionErrorKindV1::PointerOffsetOverflow)
}

/// No pointer edges, recursion, array expansion, or inferred active variant.
/// The caller separately discharges every reported selector before publication.
pub(super) fn storage_input_path_v29(
    owner: &fe2o3_kernel_ir::VerifiedStorageKernelIrModuleV1<'_>,
    accounting: &StorageAccountingV1,
    mut position: StorageInputPositionV29,
    path: &[SimulationObjectComponentV29],
    mut selector: impl FnMut(StorageInputSelectorV29) -> Result<(), SimulationExecutionErrorKindV1>,
) -> Result<StorageInputPositionV29, SimulationExecutionErrorKindV1> {
    for component in path {
        accounting.charge(1)?;
        let row = storage_input_row_v29(owner, accounting, position.layout)?;
        let (offset, child) = match (component, &row.kind) {
            (
                SimulationObjectComponentV29::Field(index),
                StorageLayoutKindV1::Record(fields) | StorageLayoutKindV1::Union(fields),
            ) => {
                let field = fields
                    .get(*index as usize)
                    .ok_or(storage_violation_v1("input field ordinal is absent"))?;
                (field.offset, field.layout)
            }
            (
                SimulationObjectComponentV29::Field(index),
                StorageLayoutKindV1::Slice { data, length, .. },
            ) => {
                let field = match index {
                    0 => data,
                    1 => length,
                    _ => return Err(storage_violation_v1("input descriptor field is absent")),
                };
                (field.offset, field.layout)
            }
            (
                SimulationObjectComponentV29::Index(index),
                StorageLayoutKindV1::Array {
                    element,
                    length,
                    stride,
                },
            ) => {
                if index >= length {
                    return Err(storage_violation_v1(
                        "input array index exceeds its exact length",
                    ));
                }
                (
                    index
                        .checked_mul(*stride)
                        .ok_or(SimulationExecutionErrorKindV1::PointerOffsetOverflow)?,
                    *element,
                )
            }
            (
                SimulationObjectComponentV29::Variant(index),
                StorageLayoutKindV1::Variants { variants, .. },
            ) => {
                let variant = variants
                    .get(*index as usize)
                    .filter(|value| !value.uninhabited)
                    .ok_or(storage_violation_v1(
                        "input variant is absent or uninhabited",
                    ))?;
                selector(StorageInputSelectorV29 {
                    parent: position,
                    variant: *index,
                })?;
                (0, variant.layout)
            }
            (SimulationObjectComponentV29::Tag, StorageLayoutKindV1::Variants { encoding, .. }) => {
                let tag = encoding.tag();
                (tag.offset, tag.layout)
            }
            _ => {
                return Err(storage_violation_v1(
                    "input path does not match its physical component",
                ));
            }
        };
        let child_row = storage_input_row_v29(owner, accounting, child)?;
        let start = position
            .start
            .checked_add(storage_input_extent_v29(offset)?)
            .ok_or(SimulationExecutionErrorKindV1::PointerOffsetOverflow)?;
        let end = start
            .checked_add(storage_input_extent_v29(child_row.size)?)
            .ok_or(SimulationExecutionErrorKindV1::PointerOffsetOverflow)?;
        if start < position.start || end > position.end {
            return Err(storage_violation_v1(
                "input child escapes its containing object",
            ));
        }
        position = StorageInputPositionV29 {
            allocation: position.allocation,
            layout: child,
            start,
            end,
        };
    }
    Ok(position)
}

impl Memory {
    pub(super) fn storage_input_allocation_v29(
        &self,
        origin: SimulationInputOriginV29,
    ) -> Result<(u64, &Allocation, StorageInputAllocationV29), SimulationExecutionErrorKindV1> {
        self.storage_accounting.charge(1)?;
        let id = match origin {
            SimulationInputOriginV29::Argument(index) => self
                .argument_allocations
                .get(index as usize)
                .copied()
                .flatten(),
            SimulationInputOriginV29::Backing(id) => self.shared_allocations.get(&id).copied(),
        }
        .ok_or(storage_violation_v1(
            "input referent has no original allocation",
        ))?;
        let allocation = self
            .allocations
            .get(&id)
            .ok_or(SimulationExecutionErrorKindV1::DanglingPointer { allocation: id })?;
        let input = allocation.input.ok_or(storage_violation_v1(
            "input referent is not an original allocation",
        ))?;
        if input.origin != origin
            || allocation.storage.scope != StorageScopeV1::Unscoped
            || !matches!(
                allocation.address_space,
                AddressSpace::Global | AddressSpace::Constant
            )
        {
            return Err(storage_violation_v1(
                "input referent changes original allocation identity or scope",
            ));
        }
        Ok((id, allocation, input))
    }

    pub(super) fn storage_input_referent_v29(
        &self,
        owner: &fe2o3_kernel_ir::VerifiedStorageKernelIrModuleV1<'_>,
        view: &SimulationObjectViewV29,
        target: SimulationTargetV1,
        selector: impl FnMut(StorageInputSelectorV29) -> Result<(), SimulationExecutionErrorKindV1>,
    ) -> Result<StorageInputReferentV29, SimulationExecutionErrorKindV1> {
        let (id, allocation, input) = self.storage_input_allocation_v29(view.origin)?;
        if (allocation.access != AccessMode::ReadWrite && allocation.access != view.access)
            || allocation.address_space == AddressSpace::Constant
                && view.access != AccessMode::ReadOnly
        {
            return Err(storage_violation_v1(
                "input referent strengthens backing permissions",
            ));
        }
        let (position, sequence) = match input.representation {
            StorageInputRepresentationV29::Scalar(scalar) => {
                let row = storage_input_row_v29(owner, &self.storage_accounting, view.layout)?;
                let width = target
                    .scalar_bytes(scalar)
                    .filter(|width| *width != 0)
                    .ok_or(storage_violation_v1(
                        "input scalar referent has no target width",
                    ))?;
                if !view.path.is_empty()
                    || row.kind != StorageLayoutKindV1::Scalar(scalar)
                    || row.size != width as u64
                    || allocation.bytes.len() % width != 0
                {
                    return Err(storage_violation_v1(
                        "input scalar referent changes exact element layout",
                    ));
                }
                (
                    StorageInputPositionV29 {
                        allocation: id,
                        layout: view.layout,
                        start: 0,
                        end: allocation.bytes.len(),
                    },
                    Some((view.layout, allocation.bytes.len() / width, width)),
                )
            }
            StorageInputRepresentationV29::Object(layout) => {
                let root = StorageInputPositionV29 {
                    allocation: id,
                    layout,
                    start: 0,
                    end: allocation.bytes.len(),
                };
                let position = storage_input_path_v29(
                    owner,
                    &self.storage_accounting,
                    root,
                    &view.path,
                    selector,
                )?;
                let row = storage_input_row_v29(owner, &self.storage_accounting, position.layout)?;
                let sequence = if let StorageLayoutKindV1::Array {
                    element,
                    length,
                    stride,
                } = row.kind
                {
                    Some((
                        element,
                        storage_input_extent_v29(length)?,
                        storage_input_extent_v29(stride)?,
                    ))
                } else {
                    None
                };
                (position, sequence)
            }
        };
        let (position, elements) = if let Some(range) = view.range {
            let (element, length, stride) = sequence.ok_or(storage_violation_v1(
                "input range requires an actual array or scalar buffer",
            ))?;
            let row = storage_input_row_v29(owner, &self.storage_accounting, element)?;
            let start = storage_input_extent_v29(range.start)?;
            let elements = storage_input_extent_v29(range.elements)?;
            if element != view.layout
                || row.size != stride as u64
                || start > length
                || elements > length - start
            {
                return Err(storage_violation_v1(
                    "input range changes element stride or exceeds bounds",
                ));
            }
            let start = start
                .checked_mul(stride)
                .and_then(|offset| position.start.checked_add(offset))
                .ok_or(SimulationExecutionErrorKindV1::PointerOffsetOverflow)?;
            let end = elements
                .checked_mul(stride)
                .and_then(|width| start.checked_add(width))
                .ok_or(SimulationExecutionErrorKindV1::PointerOffsetOverflow)?;
            if end > position.end {
                return Err(storage_violation_v1("input range exceeds original storage"));
            }
            (
                StorageInputPositionV29 {
                    allocation: id,
                    layout: element,
                    start,
                    end,
                },
                Some(elements),
            )
        } else {
            if position.layout != view.layout {
                return Err(storage_violation_v1(
                    "input view changes exact layout identity",
                ));
            }
            (position, None)
        };
        self.storage_accounting.charge(1)?;
        if position.start > position.end || position.end > allocation.bytes.len() {
            return Err(storage_violation_v1(
                "input referent exceeds its original allocation",
            ));
        }
        // Packed projections can be valid pointer values. Each actual access
        // still checks its declared alignment against the allocation and offset.
        Ok(StorageInputReferentV29 {
            position,
            address_space: allocation.address_space,
            access: view.access,
            elements,
        })
    }
}
