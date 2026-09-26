//! Current observations retain original input identities, never replay paths.

use super::*;
use crate::storage_inputs_v29::*;

fn storage_export_headers_v29() -> usize {
    2 * size_of::<Vec<SimulationStorageArgumentObservationV29>>()
        + 2 * size_of::<Vec<SimulationSharedStorageObservationV29>>()
        + 2 * size_of::<SimulationObjectObservationV29>()
        + 2 * size_of::<Result<SimulationObjectObservationV29, SimulationExecutionErrorKindV1>>()
        + 2 * size_of::<SimulationRelocationObservationV29>()
        + 2 * size_of::<Result<SimulationPointerObservationV29, SimulationExecutionErrorKindV1>>()
        + 2 * size_of::<Result<Result<(), SimulationExecutionErrorV1>, Box<dyn std::any::Any + Send>>>(
        )
}

fn output_relocation_credit_v29(
    arguments: &[SimulationStorageArgumentObservationV29],
    backings: &[SimulationSharedStorageObservationV29],
) -> usize {
    let argument_rows = arguments.iter().filter_map(|argument| match argument {
        SimulationStorageArgumentObservationV29::InlineObject(image) => {
            Some(image.relocations.capacity())
        }
        _ => None,
    });
    let backing_rows = backings
        .iter()
        .filter_map(|backing| match &backing.storage {
            SimulationStorageBackingObservationV29::Object { image, .. } => {
                Some(image.relocations.capacity())
            }
            _ => None,
        });
    argument_rows.chain(backing_rows).sum::<usize>()
        * size_of::<SimulationRelocationObservationV29>()
}

fn observe_input_pointer_v29(
    memory: &Memory,
    pointer: &PointerValue,
    pointee: SimulationObservedPointeeV29,
) -> Result<SimulationPointerObservationV29, SimulationExecutionErrorKindV1> {
    memory.storage_accounting.charge(1)?;
    let allocation = memory.allocation(pointer)?;
    let input = allocation.input.ok_or(storage_violation_v1(
        "output pointer escapes the original input allocations",
    ))?;
    if allocation.storage.scope != StorageScopeV1::Unscoped
        || !matches!(
            allocation.address_space,
            AddressSpace::Global | AddressSpace::Constant
        )
        || pointer.address_space != allocation.address_space
        || (allocation.access != AccessMode::ReadWrite && allocation.access != pointer.access)
        || pointer.byte_offset < pointer.lower_bound
        || pointer.byte_offset > pointer.upper_bound
        || pointer.upper_bound > allocation.bytes.len()
    {
        return Err(storage_violation_v1(
            "output pointer changes original scope, rights or bounds",
        ));
    }
    allocation
        .storage
        .guard(pointer.storage_guard, &memory.storage_accounting)?;
    // This is an identity join, not a reverse search for a representable path.
    let (id, _, _) = memory.storage_input_allocation_v29(input.origin)?;
    if id != pointer.allocation {
        return Err(storage_violation_v1(
            "output pointer substituted an original allocation",
        ));
    }
    Ok(SimulationPointerObservationV29 {
        origin: input.origin,
        pointee,
        address_space: pointer.visible_address_space(),
        access: pointer.access,
        byte_offset: pointer.byte_offset,
        lower_bound: pointer.lower_bound,
        upper_bound: pointer.upper_bound,
    })
}

fn observe_input_object_v29(
    memory: &Memory,
    origin: SimulationInputOriginV29,
    image: &SimulationObjectImageV29,
) -> Result<SimulationObjectObservationV29, SimulationExecutionErrorKindV1> {
    let (_, allocation, input) = memory.storage_input_allocation_v29(origin)?;
    if !matches!(input.representation, StorageInputRepresentationV29::Object(id) if id == image.layout())
        || allocation.bytes.len() != image.bytes().len()
        || allocation.alignment != image.alignment()
    {
        return Err(storage_violation_v1(
            "output object changes its original registered schema",
        ));
    }
    memory.storage_accounting.charge(
        allocation
            .bytes
            .len()
            .checked_mul(2)
            .and_then(|work| work.checked_add(allocation.storage.relocations.len()))
            .ok_or(storage_violation_v1("object observation work overflow"))?,
    )?;
    let bytes = try_clone_slice(&allocation.bytes)?;
    let initialized = try_clone_slice(&allocation.initialized)?;
    let mut relocations = Vec::new();
    // Keep observation growth in the same consumed ledger; it is not an input
    // relocation census and may include pointers written during execution.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        storage_reserve_v1(
            &mut relocations,
            allocation.storage.relocations.len(),
            &memory.storage_accounting,
        )?;
        for relocation in &allocation.storage.relocations {
            memory
                .storage_validate_input_pointer_v29(&relocation.value, relocation.representation)?;
            let pointee = match &relocation.value {
                StoragePointerPayloadV1::Scalar(pointer) => {
                    SimulationObservedPointeeV29::Scalar(pointer.element)
                }
                StoragePointerPayloadV1::Object(address) => {
                    SimulationObservedPointeeV29::Object(address.layout)
                }
            };
            if relocation.start >= relocation.end
                || relocation.end > initialized.len()
                || relocation.end - relocation.start
                    != usize::from(relocation.representation.stored_bits / 8)
            {
                return Err(storage_violation_v1(
                    "output relocation is not a complete original object region",
                ));
            }
            memory
                .storage_accounting
                .charge(relocation.end - relocation.start)?;
            if !initialized[relocation.start..relocation.end]
                .iter()
                .all(|bit| *bit)
            {
                return Err(storage_violation_v1(
                    "output relocation occupies uninitialized bytes",
                ));
            }
            relocations.push(SimulationRelocationObservationV29 {
                byte_offset: relocation.start,
                byte_width: relocation.end - relocation.start,
                representation: relocation.representation,
                pointer: observe_input_pointer_v29(memory, relocation.value.pointer(), pointee)?,
            });
        }
        Ok(())
    }));
    match result {
        Ok(Ok(())) => Ok(SimulationObjectObservationV29 {
            layout: image.layout(),
            alignment: image.alignment(),
            bytes,
            initialized,
            relocations,
        }),
        failed => {
            let credit = relocations.capacity() * size_of::<SimulationRelocationObservationV29>();
            drop(relocations);
            memory.storage_accounting.release(credit);
            match failed {
                Ok(Err(error)) => Err(error),
                Err(panic) => std::panic::resume_unwind(panic),
                Ok(Ok(())) => unreachable!(),
            }
        }
    }
}

pub(super) fn storage_export_inputs_v29(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    request: &SimulationStorageRequestV29,
    parameters: &[RuntimeValue],
) -> Result<
    (
        Vec<SimulationStorageArgumentObservationV29>,
        Vec<SimulationSharedStorageObservationV29>,
    ),
    SimulationExecutionErrorV1,
> {
    if parameters.len() != request.arguments.len() {
        return Err(engine.fail(storage_violation_v1(
            "output lost the original parameter census",
        )));
    }
    let _headers = engine
        .memory
        .storage_accounting
        .temporary(storage_export_headers_v29())
        .map_err(top_level_error)?;
    let mut arguments = Vec::new();
    let mut backings = Vec::new();
    arguments
        .try_reserve_exact(request.arguments.len())
        .map_err(|_| engine.fail(SimulationExecutionErrorKindV1::AllocationFailure))?;
    backings
        .try_reserve_exact(request.shared_storage.len())
        .map_err(|_| engine.fail(SimulationExecutionErrorKindV1::AllocationFailure))?;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for (index, (argument, parameter)) in request.arguments.iter().zip(parameters).enumerate() {
            let output = match argument {
                SimulationStorageArgumentV29::Existing(argument) => {
                    SimulationStorageArgumentObservationV29::Existing(copy_back_argument_v29(
                        &engine.memory,
                        index,
                        argument,
                    )?)
                }
                SimulationStorageArgumentV29::InlineObject(image) => {
                    SimulationStorageArgumentObservationV29::InlineObject(
                        observe_input_object_v29(
                            &engine.memory,
                            SimulationInputOriginV29::Argument(index as u32),
                            image,
                        )
                        .map_err(top_level_error)?,
                    )
                }
                SimulationStorageArgumentV29::ObjectView(view) => {
                    let (address, elements) = match parameter {
                        RuntimeValue::StoragePointer(address) => (address, None),
                        RuntimeValue::StorageSlice(slice) => {
                            (&slice.address, Some(slice.elements as u64))
                        }
                        _ => {
                            return Err(engine.fail(storage_violation_v1(
                                "output object view lost its original typed parameter",
                            )));
                        }
                    };
                    if address.layout != view.layout {
                        return Err(engine.fail(storage_violation_v1(
                            "output object view changed its original layout",
                        )));
                    }
                    SimulationStorageArgumentObservationV29::ObjectView {
                        pointer: observe_input_pointer_v29(
                            &engine.memory,
                            &address.pointer,
                            SimulationObservedPointeeV29::Object(address.layout),
                        )
                        .map_err(top_level_error)?,
                        elements,
                    }
                }
            };
            arguments.push(output);
        }
        for shared in &request.shared_storage {
            let storage = match &shared.storage {
                SimulationStorageBackingV29::Scalar(buffer) => {
                    let (_, allocation, input) = engine
                        .memory
                        .storage_input_allocation_v29(SimulationInputOriginV29::Backing(shared.id))
                        .map_err(top_level_error)?;
                    if !matches!(input.representation, StorageInputRepresentationV29::Scalar(scalar) if scalar == buffer.element())
                        || !allocation.storage.relocations.is_empty()
                    {
                        return Err(engine.fail(storage_violation_v1(
                            "raw shared output cannot export symbolic pointer representations",
                        )));
                    }
                    SimulationStorageBackingObservationV29::Scalar(buffer.with_contents(
                        try_clone_slice(&allocation.bytes).map_err(top_level_error)?,
                        try_clone_slice(&allocation.initialized).map_err(top_level_error)?,
                    ))
                }
                SimulationStorageBackingV29::Object { image, access } => {
                    SimulationStorageBackingObservationV29::Object {
                        image: observe_input_object_v29(
                            &engine.memory,
                            SimulationInputOriginV29::Backing(shared.id),
                            image,
                        )
                        .map_err(top_level_error)?,
                        access: *access,
                    }
                }
            };
            backings.push(SimulationSharedStorageObservationV29 {
                id: shared.id,
                storage,
            });
        }
        Ok::<(), SimulationExecutionErrorV1>(())
    }));
    match result {
        Ok(Ok(())) => Ok((arguments, backings)),
        failed => {
            let credit = output_relocation_credit_v29(&arguments, &backings);
            drop(arguments);
            drop(backings);
            engine.memory.storage_accounting.release(credit);
            match failed {
                Ok(Err(error)) => Err(error),
                Err(panic) => std::panic::resume_unwind(panic),
                Ok(Ok(())) => unreachable!(),
            }
        }
    }
}
