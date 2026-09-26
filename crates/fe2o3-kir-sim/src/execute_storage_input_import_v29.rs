//! Transactional input preparation. No Engine or user sink exists here.

use super::*;
use crate::storage_inputs_v29::*;
use crate::storage_request_view_v29::{SimulationArgumentRefV29, SimulationBackingRefV29};
use fe2o3_kernel_ir::{StorageLayoutKindV1, StoragePointerV1, StorageVariantEncodingV1};

pub(super) struct StorageInputContextV29<'a> {
    pub(super) memory: &'a mut Memory,
    pub(super) target: SimulationTargetV1,
    pub(super) limits: SimulationLimitsV1,
}

impl StorageInputContextV29<'_> {
    pub(super) fn fail(&self, kind: SimulationExecutionErrorKindV1) -> SimulationExecutionErrorV1 {
        top_level_error(kind)
    }
}

mod sealed {
    pub trait Sealed {}
}

pub(super) trait StorageInputProfileV29: sealed::Sealed {
    type Argument;
    type Backing;
    fn request(&self) -> SimulationRequestRefV29<'_>;
    fn initialize(
        &self,
        context: &mut StorageInputContextV29<'_>,
        owner: Option<&fe2o3_kernel_ir::VerifiedStorageKernelIrModuleV1<'_>>,
        entry: &Function,
    ) -> Result<Vec<RuntimeValue>, SimulationExecutionErrorV1>;
    fn copy_back(
        &self,
        engine: &Engine<'_, impl SimulationEventSinkV1>,
        parameters: &[RuntimeValue],
    ) -> Result<(Vec<Self::Argument>, Vec<Self::Backing>), SimulationExecutionErrorV1>;
}

pub(super) struct LegacyStorageInputsV29<'a>(pub(super) &'a SimulationRequestV1);
pub(super) struct ExplicitStorageInputsV29<'a>(pub(super) &'a SimulationStorageRequestV29);
impl sealed::Sealed for LegacyStorageInputsV29<'_> {}
impl sealed::Sealed for ExplicitStorageInputsV29<'_> {}

impl StorageInputProfileV29 for LegacyStorageInputsV29<'_> {
    type Argument = SimulationArgumentV1;
    type Backing = SharedBufferV1;
    fn request(&self) -> SimulationRequestRefV29<'_> {
        SimulationRequestRefV29::Legacy(self.0)
    }
    fn initialize(
        &self,
        context: &mut StorageInputContextV29<'_>,
        _: Option<&fe2o3_kernel_ir::VerifiedStorageKernelIrModuleV1<'_>>,
        entry: &Function,
    ) -> Result<Vec<RuntimeValue>, SimulationExecutionErrorV1> {
        initialize_shared_buffers(context, self.0)?;
        initialize_arguments(context, entry, self.0)
    }
    fn copy_back(
        &self,
        engine: &Engine<'_, impl SimulationEventSinkV1>,
        _: &[RuntimeValue],
    ) -> Result<(Vec<Self::Argument>, Vec<Self::Backing>), SimulationExecutionErrorV1> {
        Ok((
            copy_back_arguments(&engine.memory, &self.0.arguments)?,
            copy_back_shared_buffers(&engine.memory, &self.0.shared_buffers)?,
        ))
    }
}

impl StorageInputProfileV29 for ExplicitStorageInputsV29<'_> {
    type Argument = SimulationStorageArgumentObservationV29;
    type Backing = SimulationSharedStorageObservationV29;
    fn request(&self) -> SimulationRequestRefV29<'_> {
        SimulationRequestRefV29::Storage(self.0)
    }
    fn initialize(
        &self,
        context: &mut StorageInputContextV29<'_>,
        owner: Option<&fe2o3_kernel_ir::VerifiedStorageKernelIrModuleV1<'_>>,
        entry: &Function,
    ) -> Result<Vec<RuntimeValue>, SimulationExecutionErrorV1> {
        let owner = owner.ok_or_else(|| {
            context.fail(storage_violation_v1(
                "explicit storage inputs require the exact V18 owner",
            ))
        })?;
        storage_import_inputs_v29(context, owner, self.0, entry).map_err(top_level_error)
    }
    fn copy_back(
        &self,
        engine: &Engine<'_, impl SimulationEventSinkV1>,
        parameters: &[RuntimeValue],
    ) -> Result<(Vec<Self::Argument>, Vec<Self::Backing>), SimulationExecutionErrorV1> {
        storage_export_inputs_v29(engine, self.0, parameters)
    }
}

struct PreparedInputRelocationV29<'a> {
    holder: StorageInputPositionV29,
    source: &'a SimulationObjectRelocationV29,
    root: StorageInputPositionV29,
    representation: StoragePointerV1,
    referent: StorageInputReferentV29,
    guard: Option<usize>,
}

struct PreparedInputGuardV29 {
    parent: StorageInputPositionV29,
    start: usize,
    end: usize,
    previous: Option<usize>,
    published: Option<usize>,
}

#[derive(Clone, Copy)]
struct PreparedInputArgumentV29 {
    referent: StorageInputReferentV29,
    guard: Option<usize>,
}

#[derive(Default)]
struct InputScratchV29<'a> {
    relocations: Vec<PreparedInputRelocationV29<'a>>,
    guards: Vec<PreparedInputGuardV29>,
    arguments: Vec<Option<PreparedInputArgumentV29>>,
}

fn storage_input_headers_v29() -> usize {
    size_of::<InputScratchV29<'_>>()
        + 2 * size_of::<PreparedInputRelocationV29<'_>>()
        + 2 * size_of::<PreparedInputGuardV29>()
        + 2 * size_of::<PreparedInputArgumentV29>()
        + 4 * size_of::<Result<StorageInputPositionV29, SimulationExecutionErrorKindV1>>()
        + 4 * size_of::<Result<StorageInputReferentV29, SimulationExecutionErrorKindV1>>()
        + 2 * size_of::<Result<Vec<RuntimeValue>, SimulationExecutionErrorKindV1>>()
        + size_of::<
            Result<
                Result<Vec<RuntimeValue>, SimulationExecutionErrorKindV1>,
                Box<dyn std::any::Any + Send>,
            >,
        >()
}

impl InputScratchV29<'_> {
    fn release(self, accounting: &StorageAccountingV1) {
        let bytes = self.relocations.capacity() * size_of::<PreparedInputRelocationV29<'_>>()
            + self.guards.capacity() * size_of::<PreparedInputGuardV29>()
            + self.arguments.capacity() * size_of::<Option<PreparedInputArgumentV29>>();
        drop(self);
        accounting.release(bytes);
    }
}

fn storage_register_input_v29(
    context: &mut StorageInputContextV29<'_>,
    origin: SimulationInputOriginV29,
    backing: SimulationBackingRefV29<'_>,
    space: AddressSpace,
) -> Result<u64, SimulationExecutionErrorKindV1> {
    let (bytes, initialized, access, alignment, representation) = match backing {
        SimulationBackingRefV29::Scalar(buffer) => (
            buffer.bytes(),
            buffer.initialized(),
            buffer.access(),
            buffer.alignment(),
            StorageInputRepresentationV29::Scalar(buffer.element()),
        ),
        SimulationBackingRefV29::Object { image, access } => (
            image.bytes(),
            image.initialized(),
            access,
            image.alignment(),
            StorageInputRepresentationV29::Object(image.layout()),
        ),
    };
    context.memory.storage_accounting.charge(
        bytes
            .len()
            .checked_mul(2)
            .and_then(|work| work.checked_add(1))
            .ok_or(storage_violation_v1("input registration work overflow"))?,
    )?;
    context
        .memory
        .validate_allocation(bytes.len(), context.limits)?;
    let id = context.memory.allocate(
        space,
        access,
        alignment,
        try_clone_slice(bytes)?,
        try_clone_slice(initialized)?,
        context.limits,
    )?;
    context
        .memory
        .allocations
        .get_mut(&id)
        .ok_or(storage_violation_v1("new input allocation disappeared"))?
        .input = Some(StorageInputAllocationV29 {
        origin,
        representation,
    });
    match origin {
        SimulationInputOriginV29::Argument(index) => {
            let slot = context
                .memory
                .argument_allocations
                .get_mut(index as usize)
                .ok_or(storage_violation_v1("original input argument is absent"))?;
            if slot.replace(id).is_some() {
                return Err(storage_violation_v1(
                    "input argument allocation is duplicated",
                ));
            }
        }
        SimulationInputOriginV29::Backing(key) => {
            if context.memory.shared_allocations.insert(key, id).is_some() {
                return Err(storage_violation_v1(
                    "input backing allocation is duplicated",
                ));
            }
        }
    }
    Ok(id)
}

fn storage_input_images_v29(
    request: &SimulationStorageRequestV29,
) -> impl Iterator<Item = (SimulationInputOriginV29, &SimulationObjectImageV29)> {
    request
        .shared_storage
        .iter()
        .filter_map(|shared| match &shared.storage {
            SimulationStorageBackingV29::Object { image, .. } => {
                Some((SimulationInputOriginV29::Backing(shared.id), image))
            }
            _ => None,
        })
        .chain(request.arguments.iter().enumerate().filter_map(
            |(index, argument)| match argument {
                SimulationStorageArgumentV29::InlineObject(image) => {
                    Some((SimulationInputOriginV29::Argument(index as u32), image))
                }
                _ => None,
            },
        ))
}

fn prepare_input_relocations_v29<'a>(
    context: &StorageInputContextV29<'_>,
    owner: &fe2o3_kernel_ir::VerifiedStorageKernelIrModuleV1<'_>,
    request: &'a SimulationStorageRequestV29,
    scratch: &mut InputScratchV29<'a>,
) -> Result<(), SimulationExecutionErrorKindV1> {
    for (origin, image) in storage_input_images_v29(request) {
        let (id, allocation, _) = context.memory.storage_input_allocation_v29(origin)?;
        let root = StorageInputPositionV29 {
            allocation: id,
            layout: image.layout(),
            start: 0,
            end: allocation.bytes.len(),
        };
        let mut last_end = 0;
        for source in image.relocations() {
            context.memory.storage_accounting.charge(1)?;
            let holder = storage_input_path_v29(
                owner,
                &context.memory.storage_accounting,
                root,
                &source.path,
                |_| Ok(()),
            )?;
            let row =
                storage_input_row_v29(owner, &context.memory.storage_accounting, holder.layout)?;
            let StorageLayoutKindV1::Pointer(representation) = row.kind else {
                return Err(storage_violation_v1(
                    "input relocation path is not a pointer row",
                ));
            };
            let recipe = context
                .target
                .storage_pointer_encoding(representation)
                .ok_or(storage_violation_v1(
                    "input pointer requires an exact target encoding profile",
                ))?;
            if holder.layout != source.pointer_layout
                || holder.start < last_end
                || holder.end - holder.start != usize::from(recipe.bits() / 8)
                || row.alignment != u32::from(recipe.bits() / 8)
            {
                return Err(storage_violation_v1(
                    "input relocations are partial, overlapping, unordered or misrepresented",
                ));
            }
            context
                .memory
                .storage_accounting
                .charge(holder.end - holder.start)?;
            if !allocation.initialized[holder.start..holder.end]
                .iter()
                .all(|bit| *bit)
            {
                return Err(storage_violation_v1(
                    "input pointer representation is not initialized",
                ));
            }
            let referent = context.memory.storage_input_referent_v29(
                owner,
                &source.referent,
                context.target,
                |_| Ok(()),
            )?;
            if representation.pointee != referent.position.layout
                || representation.access != referent.access
                || (representation.value_space != referent.address_space
                    && representation.value_space != AddressSpace::Generic)
            {
                return Err(storage_violation_v1(
                    "input relocation changes its actual referent representation",
                ));
            }
            let count = scratch
                .relocations
                .len()
                .checked_add(1)
                .ok_or(storage_violation_v1("input relocation count overflow"))?;
            storage_reserve_v1(
                &mut scratch.relocations,
                count,
                &context.memory.storage_accounting,
            )?;
            scratch.relocations.push(PreparedInputRelocationV29 {
                holder,
                source,
                root,
                representation,
                referent,
                guard: None,
            });
            last_end = holder.end;
        }
    }
    Ok(())
}

/// The binary search inspects the prepared domain only. It cannot dereference
/// the target, publish a pointer, or discharge the target's own selectors.
fn prepared_tag_relocation_v29<'a, 'input>(
    prepared: &'a [PreparedInputRelocationV29<'input>],
    allocation: u64,
    start: usize,
    end: usize,
    accounting: &StorageAccountingV1,
) -> Result<Option<&'a PreparedInputRelocationV29<'input>>, SimulationExecutionErrorKindV1> {
    let mut low = 0;
    let mut high = prepared.len();
    while low < high {
        accounting.charge(1)?;
        let middle = low + (high - low) / 2;
        let row = &prepared[middle];
        if (row.holder.allocation, row.holder.start) < (allocation, start) {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    for index in [low.checked_sub(1), (low < prepared.len()).then_some(low)]
        .into_iter()
        .flatten()
    {
        accounting.charge(1)?;
        let row = &prepared[index];
        if row.holder.allocation == allocation
            && storage_overlap_v1(start, end, row.holder.start, row.holder.end)
        {
            if row.holder.start != start || row.holder.end != end {
                return Err(storage_violation_v1(
                    "input tag has a partial symbolic relocation",
                ));
            }
            return Ok(Some(row));
        }
    }
    Ok(None)
}

fn validate_input_selector_v29(
    memory: &Memory,
    owner: &fe2o3_kernel_ir::VerifiedStorageKernelIrModuleV1<'_>,
    target: SimulationTargetV1,
    prepared: &[PreparedInputRelocationV29<'_>],
    selector: StorageInputSelectorV29,
) -> Result<(usize, usize), SimulationExecutionErrorKindV1> {
    let row = storage_input_row_v29(owner, &memory.storage_accounting, selector.parent.layout)?;
    let StorageLayoutKindV1::Variants { encoding, variants } = &row.kind else {
        return Err(storage_violation_v1(
            "input selector lost its original variant row",
        ));
    };
    let tag = encoding.tag();
    let tag_row = storage_input_row_v29(owner, &memory.storage_accounting, tag.layout)?;
    let width = usize::try_from(tag_row.size)
        .ok()
        .filter(|width| *width > 0 && *width <= 16)
        .ok_or(storage_violation_v1(
            "input variant tag width is not representable",
        ))?;
    let start = usize::try_from(tag.offset)
        .ok()
        .and_then(|offset| selector.parent.start.checked_add(offset))
        .ok_or(SimulationExecutionErrorKindV1::PointerOffsetOverflow)?;
    let end = start
        .checked_add(width)
        .filter(|end| *end <= selector.parent.end)
        .ok_or(storage_violation_v1(
            "input tag escapes its exact containing object",
        ))?;
    let allocation = memory.allocations.get(&selector.parent.allocation).ok_or(
        SimulationExecutionErrorKindV1::DanglingPointer {
            allocation: selector.parent.allocation,
        },
    )?;
    memory.storage_accounting.charge(width)?;
    if !allocation
        .initialized
        .get(start..end)
        .is_some_and(|bits| bits.iter().all(|bit| *bit))
    {
        return Err(storage_violation_v1("input variant tag is not initialized"));
    }
    let relocation = prepared_tag_relocation_v29(
        prepared,
        selector.parent.allocation,
        start,
        end,
        &memory.storage_accounting,
    )?;
    let active = match (&tag_row.kind, relocation) {
        (StorageLayoutKindV1::Pointer(representation), Some(relocation))
            if matches!(encoding, StorageVariantEncodingV1::Niche { .. }) =>
        {
            let recipe = target
                .storage_pointer_encoding(*representation)
                .filter(|recipe| {
                    usize::from(recipe.bits() / 8) == width
                        && tag_row.alignment == u32::from(recipe.bits() / 8)
                })
                .ok_or(storage_violation_v1(
                    "input pointer tag differs from its exact target encoding",
                ))?;
            if relocation.representation != *representation {
                return Err(storage_violation_v1(
                    "input pointer tag relocation has a foreign representation",
                ));
            }
            Some(storage_nonnull_pointer_variant_v29(
                recipe,
                *encoding,
                variants.len(),
                &memory.storage_accounting,
            )?)
        }
        (StorageLayoutKindV1::Scalar(scalar), None)
            if (scalar.is_integer() || *scalar == ScalarType::Bool)
                && target.scalar_bytes(*scalar) == Some(width) =>
        {
            memory.storage_accounting.charge(width)?;
            let mut bytes = [0; 16];
            bytes[..width].copy_from_slice(&allocation.bytes[start..end]);
            storage_raw_variant_v29(
                u128::from_le_bytes(bytes),
                width,
                *encoding,
                variants,
                &memory.storage_accounting,
            )?
        }
        (StorageLayoutKindV1::Pointer(representation), None)
            if matches!(encoding, StorageVariantEncodingV1::Niche { .. }) =>
        {
            let recipe = target
                .storage_pointer_encoding(*representation)
                .filter(|recipe| {
                    usize::from(recipe.bits() / 8) == width
                        && tag_row.alignment == u32::from(recipe.bits() / 8)
                })
                .ok_or(storage_violation_v1(
                    "input pointer tag lacks its exact target encoding",
                ))?;
            memory.storage_accounting.charge(width)?;
            let mut bytes = [0; 16];
            bytes[..width].copy_from_slice(&allocation.bytes[start..end]);
            let _ = recipe;
            storage_raw_variant_v29(
                u128::from_le_bytes(bytes),
                width,
                *encoding,
                variants,
                &memory.storage_accounting,
            )?
        }
        _ => {
            return Err(storage_violation_v1(
                "input tag has an unsupported or incompatible symbolic domain",
            ));
        }
    };
    if active != Some(selector.variant as usize)
        || variants
            .get(selector.variant as usize)
            .is_none_or(|variant| variant.uninhabited)
    {
        return Err(storage_violation_v1(
            "input projection names an inactive or uninhabited variant",
        ));
    }
    Ok((start, end))
}

fn prepare_referent_guards_v29(
    context: &StorageInputContextV29<'_>,
    owner: &fe2o3_kernel_ir::VerifiedStorageKernelIrModuleV1<'_>,
    view: &SimulationObjectViewV29,
    prepared: &[PreparedInputRelocationV29<'_>],
    guards: &mut Vec<PreparedInputGuardV29>,
) -> Result<PreparedInputArgumentV29, SimulationExecutionErrorKindV1> {
    let mut previous = None;
    let referent =
        context
            .memory
            .storage_input_referent_v29(owner, view, context.target, |selector| {
                let (start, end) = validate_input_selector_v29(
                    context.memory,
                    owner,
                    context.target,
                    prepared,
                    selector,
                )?;
                let index = guards.len();
                storage_reserve_v1(
                    guards,
                    index
                        .checked_add(1)
                        .ok_or(storage_violation_v1("input guard count overflow"))?,
                    &context.memory.storage_accounting,
                )?;
                guards.push(PreparedInputGuardV29 {
                    parent: selector.parent,
                    start,
                    end,
                    previous,
                    published: None,
                });
                previous = Some(index);
                Ok(())
            })?;
    Ok(PreparedInputArgumentV29 {
        referent,
        guard: previous,
    })
}

fn prepare_input_guards_v29(
    context: &StorageInputContextV29<'_>,
    owner: &fe2o3_kernel_ir::VerifiedStorageKernelIrModuleV1<'_>,
    request: &SimulationStorageRequestV29,
    scratch: &mut InputScratchV29<'_>,
) -> Result<(), SimulationExecutionErrorKindV1> {
    for index in 0..scratch.relocations.len() {
        let row = &scratch.relocations[index];
        storage_input_path_v29(
            owner,
            &context.memory.storage_accounting,
            row.root,
            &row.source.path,
            |selector| {
                validate_input_selector_v29(
                    context.memory,
                    owner,
                    context.target,
                    &scratch.relocations,
                    selector,
                )
                .map(|_| ())
            },
        )?;
        let target = prepare_referent_guards_v29(
            context,
            owner,
            &row.source.referent,
            &scratch.relocations,
            &mut scratch.guards,
        )?;
        scratch.relocations[index].guard = target.guard;
    }
    storage_reserve_v1(
        &mut scratch.arguments,
        request.arguments.len(),
        &context.memory.storage_accounting,
    )?;
    for argument in &request.arguments {
        context.memory.storage_accounting.charge(1)?;
        scratch.arguments.push(match argument {
            SimulationStorageArgumentV29::ObjectView(view) => Some(prepare_referent_guards_v29(
                context,
                owner,
                view,
                &scratch.relocations,
                &mut scratch.guards,
            )?),
            _ => None,
        });
    }
    Ok(())
}

fn input_pointer_v29(
    referent: StorageInputReferentV29,
    access: AccessMode,
    visible: AddressSpace,
    ordinal: u32,
    guard: Option<usize>,
) -> PointerValue {
    PointerValue {
        allocation: referent.position.allocation,
        byte_offset: referent.position.start,
        element: ScalarType::U8,
        address_space: referent.address_space,
        access,
        lower_bound: referent.position.start,
        upper_bound: referent.position.end,
        abi_argument_ordinal: ordinal,
        storage_guard: guard,
        generic_exposed: visible == AddressSpace::Generic,
    }
}

fn publish_input_guards_v29(
    memory: &mut Memory,
    guards: &mut [PreparedInputGuardV29],
) -> Result<(), SimulationExecutionErrorKindV1> {
    for index in 0..guards.len() {
        let row = &guards[index];
        let parent = row
            .previous
            .map(|previous| {
                guards
                    .get(previous)
                    .and_then(|row| row.published)
                    .ok_or(storage_violation_v1(
                        "input guard parent was not published first",
                    ))
            })
            .transpose()?;
        let allocation = memory.allocations.get(&row.parent.allocation).ok_or(
            SimulationExecutionErrorKindV1::DanglingPointer {
                allocation: row.parent.allocation,
            },
        )?;
        let referent = StorageInputReferentV29 {
            position: row.parent,
            address_space: allocation.address_space,
            access: allocation.access,
            elements: None,
        };
        let address = StorageAddressV1 {
            pointer: input_pointer_v29(
                referent,
                allocation.access,
                allocation.address_space,
                u32::MAX,
                parent,
            ),
            layout: row.parent.layout,
        };
        let published = memory.storage_register_guard_v1(&address, row.start, row.end)?;
        guards[index].published = Some(published);
    }
    Ok(())
}

fn published_guard_v29(
    guards: &[PreparedInputGuardV29],
    index: Option<usize>,
) -> Result<Option<usize>, SimulationExecutionErrorKindV1> {
    index
        .map(|index| {
            guards
                .get(index)
                .and_then(|guard| guard.published)
                .ok_or(storage_violation_v1(
                    "input pointer's selector guard is not published",
                ))
        })
        .transpose()
}

fn publish_input_relocations_v29(
    context: &mut StorageInputContextV29<'_>,
    scratch: &InputScratchV29<'_>,
) -> Result<(), SimulationExecutionErrorKindV1> {
    for row in &scratch.relocations {
        let guard = published_guard_v29(&scratch.guards, row.guard)?;
        let pointer = input_pointer_v29(
            row.referent,
            row.representation.access,
            row.representation.value_space,
            u32::MAX,
            guard,
        );
        let value = StoragePointerPayloadV1::Object(StorageAddressV1 {
            pointer,
            layout: row.referent.position.layout,
        });
        context
            .memory
            .storage_validate_input_pointer_v29(&value, row.representation)?;
        let allocation = context
            .memory
            .allocations
            .get_mut(&row.holder.allocation)
            .ok_or(SimulationExecutionErrorKindV1::DanglingPointer {
                allocation: row.holder.allocation,
            })?;
        let count =
            allocation
                .storage
                .relocations
                .len()
                .checked_add(1)
                .ok_or(storage_violation_v1(
                    "published input relocation count overflow",
                ))?;
        storage_reserve_v1(
            &mut allocation.storage.relocations,
            count,
            &context.memory.storage_accounting,
        )?;
        allocation.storage.relocation_bytes = allocation
            .storage
            .relocation_bytes
            .checked_add(row.holder.end - row.holder.start)
            .ok_or(storage_violation_v1(
                "published input relocation byte count overflow",
            ))?;
        allocation.storage.relocations.push(StorageRelocationV1 {
            start: row.holder.start,
            end: row.holder.end,
            representation: row.representation,
            value,
        });
    }
    Ok(())
}

fn publish_input_parameters_v29(
    context: &StorageInputContextV29<'_>,
    request: &SimulationStorageRequestV29,
    entry: &Function,
    scratch: &InputScratchV29<'_>,
) -> Result<Vec<RuntimeValue>, SimulationExecutionErrorKindV1> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(request.arguments.len())
        .map_err(|_| SimulationExecutionErrorKindV1::AllocationFailure)?;
    for (index, (argument, ty)) in request
        .arguments
        .iter()
        .zip(&entry.signature.parameters)
        .enumerate()
    {
        context.memory.storage_accounting.charge(1)?;
        let ordinal = u32::try_from(index)
            .map_err(|_| storage_violation_v1("input argument ordinal overflow"))?;
        let parameter =
            match argument {
                SimulationStorageArgumentV29::Existing(argument) => {
                    initialize_registered_argument_v29(context, index, argument, ty)
                        .map_err(|error| error.kind)?
                }
                SimulationStorageArgumentV29::InlineObject(image) => {
                    let (id, allocation, _) = context.memory.storage_input_allocation_v29(
                        SimulationInputOriginV29::Argument(ordinal),
                    )?;
                    let referent = StorageInputReferentV29 {
                        position: StorageInputPositionV29 {
                            allocation: id,
                            layout: image.layout(),
                            start: 0,
                            end: allocation.bytes.len(),
                        },
                        address_space: AddressSpace::Constant,
                        access: AccessMode::ReadOnly,
                        elements: None,
                    };
                    RuntimeValue::StoragePointer(StorageAddressV1 {
                        pointer: input_pointer_v29(
                            referent,
                            AccessMode::ReadOnly,
                            AddressSpace::Constant,
                            ordinal,
                            None,
                        ),
                        layout: image.layout(),
                    })
                }
                SimulationStorageArgumentV29::ObjectView(_) => {
                    let prepared = scratch.arguments.get(index).copied().flatten().ok_or(
                        storage_violation_v1("input object parameter was not prepared"),
                    )?;
                    let (space, access, slice) = match ty {
                        Type::Pointer(pointer) => (pointer.address_space, pointer.access, false),
                        Type::Slice(slice) => (slice.address_space, slice.access, true),
                        _ => {
                            return Err(storage_violation_v1(
                                "input object parameter changed physical type",
                            ));
                        }
                    };
                    if space != prepared.referent.address_space && space != AddressSpace::Generic {
                        return Err(storage_violation_v1(
                            "input object parameter changes its actual address space",
                        ));
                    }
                    let pointer = input_pointer_v29(
                        prepared.referent,
                        access,
                        space,
                        ordinal,
                        published_guard_v29(&scratch.guards, prepared.guard)?,
                    );
                    let address = StorageAddressV1 {
                        pointer,
                        layout: prepared.referent.position.layout,
                    };
                    if slice {
                        RuntimeValue::StorageSlice(StorageSliceV1 {
                            address,
                            elements: prepared.referent.elements.ok_or(storage_violation_v1(
                                "input slice has no exact element extent",
                            ))?,
                        })
                    } else {
                        RuntimeValue::StoragePointer(address)
                    }
                }
            };
        result.push(parameter);
    }
    Ok(result)
}

fn storage_import_inputs_v29(
    context: &mut StorageInputContextV29<'_>,
    owner: &fe2o3_kernel_ir::VerifiedStorageKernelIrModuleV1<'_>,
    request: &SimulationStorageRequestV29,
    entry: &Function,
) -> Result<Vec<RuntimeValue>, SimulationExecutionErrorKindV1> {
    let headers = storage_input_headers_v29();
    context.memory.storage_accounting.hold(headers)?;
    let mut scratch = InputScratchV29::default();
    // The only callback is this private implementation closure. No client
    // operation or Engine exists while input obligations are incomplete.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let view = SimulationRequestRefV29::Storage(request);
        for (id, backing) in view.backings() {
            storage_register_input_v29(
                context,
                SimulationInputOriginV29::Backing(id),
                backing,
                AddressSpace::Global,
            )?;
        }
        for (index, argument) in view.arguments().enumerate() {
            let ordinal = u32::try_from(index)
                .map_err(|_| storage_violation_v1("input argument ordinal overflow"))?;
            match argument {
                SimulationArgumentRefV29::Existing(SimulationArgumentV1::Buffer(buffer)) => {
                    storage_register_input_v29(
                        context,
                        SimulationInputOriginV29::Argument(ordinal),
                        SimulationBackingRefV29::Scalar(buffer),
                        AddressSpace::Global,
                    )?;
                }
                SimulationArgumentRefV29::InlineObject(image) => {
                    storage_register_input_v29(
                        context,
                        SimulationInputOriginV29::Argument(ordinal),
                        SimulationBackingRefV29::Object {
                            image,
                            access: AccessMode::ReadOnly,
                        },
                        AddressSpace::Constant,
                    )?;
                }
                _ => {}
            }
        }
        // These temporary rows borrow the request but never escape. Preserve their
        // exact credit on both ordinary errors and unwinding, before the unexposed
        // Memory transaction is dropped by the caller.
        prepare_input_relocations_v29(context, owner, request, &mut scratch)?;
        prepare_input_guards_v29(context, owner, request, &mut scratch)?;
        publish_input_guards_v29(context.memory, &mut scratch.guards)?;
        publish_input_relocations_v29(context, &scratch)?;
        publish_input_parameters_v29(context, request, entry, &scratch)
    }));
    scratch.release(&context.memory.storage_accounting);
    context.memory.storage_accounting.release(headers);
    match result {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

#[cfg(test)]
#[path = "execute_storage_input_resources_v29_tests.rs"]
mod tests;
