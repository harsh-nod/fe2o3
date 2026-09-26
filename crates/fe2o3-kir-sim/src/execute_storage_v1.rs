//! Private execution of the structurally verified storage family in the existing Engine.

use super::*;
use fe2o3_kernel_ir::{
    StorageLayoutIdV1, StorageLayoutKindV1, StorageLayoutV1, StorageOperationV1,
    StorageProjectionV1, StorageVariantEncodingV1,
};

// Component execution consumes the distinct structural verifier's borrow. It
// does not manufacture an old-profile admission token or canonical identity.
pub(crate) fn simulate_storage_view_v1(
    verified: fe2o3_kernel_ir::VerifiedStorageKernelIrModuleV1<'_>,
    request: &SimulationRequestV1,
    dynamic: Option<DynamicWorkgroupMemoryRequestV1>,
    target: SimulationTargetV1,
    limits: SimulationLimitsV1,
    sink: &mut impl SimulationEventSinkV1,
) -> Result<StorageExecutionCompletionV1, SimulationErrorV1> {
    let retained =
        crate::resident::storage_module_retained_bytes_v1(&verified).ok_or_else(|| {
            SimulationErrorV1::Preflight(SimulationPreflightErrorV1::ResourceLimit {
                resource: "resident bytes",
                actual: u64::MAX,
                limit: limits.max_resident_bytes as u64,
            })
        })?;
    let plan = crate::preflight::preflight_storage_v1(
        &verified, retained, request, dynamic, target, limits,
    )
    .map_err(SimulationErrorV1::Preflight)?;
    let mut debug = NoopSimulationDebugSinkV1;
    execute_module_v1(
        verified.module(),
        Some(&verified),
        None,
        request,
        ExecutionConfiguration {
            target,
            limits,
            policy: request.events,
            plan,
            debug_capture: SimulationDebugCaptureLimitsV1::disabled(),
            schedule: None,
            resident_offset: 0,
        },
        sink,
        &mut debug,
    )
    .map_err(SimulationErrorV1::Execution)
}

pub(super) fn storage_row_v1<'a>(
    engine: &Engine<'a, impl SimulationEventSinkV1>,
    id: StorageLayoutIdV1,
    site: &CompactSite,
) -> Result<&'a StorageLayoutV1, SimulationExecutionErrorV1> {
    engine
        .memory
        .storage_accounting
        .charge(1)
        .map_err(|kind| engine.at(*site, kind))?;
    let verified = engine.storage.ok_or_else(|| {
        engine.at(
            *site,
            storage_violation_v1("storage operation has no storage-verified module borrow"),
        )
    })?;
    if !std::ptr::eq(verified.module(), engine.module) {
        return Err(engine.at(
            *site,
            storage_violation_v1("storage view belongs to a different module"),
        ));
    }
    verified.storage().layouts().row(id).ok_or_else(|| {
        engine.at(
            *site,
            storage_violation_v1("storage row is absent from the current module"),
        )
    })
}

pub(super) fn storage_allocation_element_v1(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    ty: &Type,
    alignment: u32,
    site: &CompactSite,
) -> Result<(ScalarType, usize, Option<StorageLayoutIdV1>), SimulationExecutionErrorV1> {
    match ty {
        Type::Scalar(scalar) => engine
            .target
            .scalar_bytes(*scalar)
            .map(|width| (*scalar, width, None))
            .ok_or_else(|| {
                engine.at(
                    *site,
                    SimulationExecutionErrorKindV1::InternalInvariant(
                        "preflighted scalar allocation element",
                    ),
                )
            }),
        Type::StorageObject(layout) => {
            let row = storage_row_v1(engine, *layout, site)?;
            if alignment < row.alignment {
                return Err(engine.at(
                    *site,
                    storage_violation_v1("storage allocation understates its row alignment"),
                ));
            }
            let width = storage_extent_v1(row).map_err(|kind| engine.at(*site, kind))?;
            Ok((ScalarType::U8, width, Some(*layout)))
        }
        _ => Err(engine.at(
            *site,
            storage_violation_v1("unsupported allocation element representation"),
        )),
    }
}

fn storage_address_v1(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    values: &HashMap<ValueId, RuntimeValue>,
    id: ValueId,
    site: &CompactSite,
) -> Result<StorageAddressV1, SimulationExecutionErrorV1> {
    match runtime_value(engine, values, id, site)? {
        RuntimeValue::StoragePointer(address) => Ok(address.clone()),
        _ => Err(engine.at(
            *site,
            SimulationExecutionErrorKindV1::RuntimeType {
                value: Some(id),
                expected: "pointer to current module storage object",
            },
        )),
    }
}

pub(super) fn storage_invocation_v1(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    site: &CompactSite,
) -> Result<SimulationInvocationV1, SimulationExecutionErrorV1> {
    engine.invocation.ok_or_else(|| {
        engine.at(
            *site,
            storage_violation_v1("storage access requires an active invocation"),
        )
    })
}

pub(super) fn storage_observe_v1(
    engine: &mut Engine<'_, impl SimulationEventSinkV1>,
    address: &StorageAddressV1,
    width: usize,
    write: bool,
    site: &CompactSite,
) -> Result<(), SimulationExecutionErrorV1> {
    if address.pointer.address_space == AddressSpace::Global {
        engine.record_access(
            site,
            address.pointer.allocation,
            address.pointer.byte_offset,
            width,
            write,
            false,
        )?;
    }
    let kind = if write {
        SimulationEventKindV1::MemoryWrite {
            allocation: address.pointer.allocation,
            offset: address.pointer.byte_offset,
            bytes: width,
        }
    } else {
        SimulationEventKindV1::MemoryRead {
            allocation: address.pointer.allocation,
            offset: address.pointer.byte_offset,
            bytes: width,
        }
    };
    engine.event(site, kind)
}

pub(super) fn execute_storage_v1(
    engine: &mut Engine<'_, impl SimulationEventSinkV1>,
    values: &HashMap<ValueId, RuntimeValue>,
    operation: &Operation,
    site: &CompactSite,
) -> Result<SmallResults<RuntimeValue>, SimulationExecutionErrorV1> {
    let OperationKind::Storage(storage) = &operation.kind else {
        return Err(engine.at(
            *site,
            storage_violation_v1("storage dispatch kind mismatch"),
        ));
    };
    let invocation = storage_invocation_v1(engine, site)?;
    match *storage {
        StorageOperationV1::Project { base, step } => {
            let base = storage_address_v1(engine, values, base, site)?;
            let address =
                storage_project_v1(engine, values, operation, &base, step, invocation, site)?;
            Ok(SmallResults::One(RuntimeValue::StoragePointer(address)))
        }
        StorageOperationV1::ReadValue { address, access } => {
            let address = storage_address_v1(engine, values, address, site)?;
            let row = storage_row_v1(engine, address.layout, site)?;
            let result = operation
                .results
                .first()
                .filter(|_| operation.results.len() == 1)
                .ok_or_else(|| {
                    engine.at(*site, storage_violation_v1("storage read result arity"))
                })?;
            let value =
                storage_read_value_v1(engine, &address, row, &result.ty, access, invocation, site)?;
            Ok(SmallResults::One(value))
        }
        StorageOperationV1::ReadDiscriminant { address, access } => {
            if operation.results.len() != 1
                || operation.results[0].ty != Type::Scalar(ScalarType::U128)
                || access.volatile
            {
                return Err(engine.at(*site, storage_violation_v1("discriminant read requires nonvolatile U128 result")));
            }
            let address = storage_address_v1(engine, values, address, site)?;
            let row = storage_row_v1(engine, address.layout, site)?;
            let StorageLayoutKindV1::Variants { encoding, variants } = &row.kind else {
                return Err(engine.at(*site, storage_violation_v1("discriminant read requires variant layout")));
            };
            let (active, _, _) = storage_active_variant_v1(engine, &address, *encoding, variants, access, invocation, site)?;
            let variant = active.and_then(|index| variants.get(index)).filter(|variant| !variant.uninhabited)
                .ok_or_else(|| engine.at(*site, storage_violation_v1("discriminant read has an invalid active tag")))?;
            let value = ScalarBitsV1::new(ScalarType::U128, variant.discriminant, engine.target)
                .map_err(|_| engine.at(*site, storage_violation_v1("discriminant read result width is unsupported")))?;
            Ok(SmallResults::One(RuntimeValue::Scalar(value)))
        }
        StorageOperationV1::WriteValue {
            address,
            value,
            access,
        } => {
            let address = storage_address_v1(engine, values, address, site)?;
            let row = storage_row_v1(engine, address.layout, site)?;
            let value = runtime_value(engine, values, value, site)?;
            storage_write_value_v1(engine, &address, row, value, access, invocation, site)?;
            Ok(SmallResults::None)
        }
        StorageOperationV1::SetDiscriminant {
            address,
            variant,
            access,
        } => {
            if !operation.results.is_empty() {
                return Err(engine.at(*site, storage_violation_v1("set discriminant result arity")));
            }
            let address = storage_address_v1(engine, values, address, site)?;
            storage_set_discriminant_v1(engine, &address, variant, access, invocation, site)?;
            Ok(SmallResults::None)
        }
        StorageOperationV1::CopyObject {
            source,
            destination,
            source_access,
            destination_access,
            overlap,
        } => {
            let source = storage_address_v1(engine, values, source, site)?;
            let destination = storage_address_v1(engine, values, destination, site)?;
            storage_copy_object_v1(
                engine,
                &source,
                &destination,
                source_access,
                destination_access,
                overlap,
                invocation,
                site,
            )?;
            Ok(SmallResults::None)
        }
    }
}

fn storage_project_v1(
    engine: &mut Engine<'_, impl SimulationEventSinkV1>,
    values: &HashMap<ValueId, RuntimeValue>,
    operation: &Operation,
    base: &StorageAddressV1,
    step: StorageProjectionV1,
    invocation: SimulationInvocationV1,
    site: &CompactSite,
) -> Result<StorageAddressV1, SimulationExecutionErrorV1> {
    let row = storage_row_v1(engine, base.layout, site)?;
    let allocation = engine
        .memory
        .allocation(&base.pointer)
        .map_err(|kind| engine.at(*site, kind))?;
    allocation
        .storage
        .scope
        .validate(invocation)
        .map_err(|kind| engine.at(*site, kind))?;
    let Type::Pointer(result) = &operation
        .results
        .first()
        .filter(|_| operation.results.len() == 1)
        .ok_or_else(|| {
            engine.at(
                *site,
                storage_violation_v1("storage projection result arity"),
            )
        })?
        .ty
    else {
        return Err(engine.at(
            *site,
            storage_violation_v1("storage projection result is not a pointer"),
        ));
    };
    let (offset, layout, guard) = match step {
        StorageProjectionV1::Field(index) => {
            let (offset, layout) =
                storage_field_v1(row, index).map_err(|kind| engine.at(*site, kind))?;
            (offset, layout, base.pointer.storage_guard)
        }
        StorageProjectionV1::ArrayIndex(index) => {
            let StorageLayoutKindV1::Array {
                element,
                length,
                stride,
            } = row.kind
            else {
                return Err(engine.at(
                    *site,
                    storage_violation_v1("array projection requires array layout"),
                ));
            };
            let index = scalar_value(engine, values, index, site)?.bits();
            if index >= u128::from(length) {
                return Err(engine.at(
                    *site,
                    storage_violation_v1("storage array index is out of bounds"),
                ));
            }
            let offset = u64::try_from(index)
                .ok()
                .and_then(|index| index.checked_mul(stride))
                .ok_or_else(|| {
                    engine.at(*site, SimulationExecutionErrorKindV1::PointerOffsetOverflow)
                })?;
            (offset, element, base.pointer.storage_guard)
        }
        StorageProjectionV1::VariantForWrite { index } => {
            let StorageLayoutKindV1::Variants { variants, .. } = &row.kind else {
                return Err(engine.at(
                    *site,
                    storage_violation_v1("construction projection requires variant layout"),
                ));
            };
            if result.access != AccessMode::WriteOnly
                || base.pointer.access == AccessMode::ReadOnly
                || base.pointer.address_space == AddressSpace::Constant
            {
                return Err(engine.at(
                    *site,
                    storage_violation_v1(
                        "construction projection requires strictly write-only rights",
                    ),
                ));
            }
            let variant = variants
                .get(index as usize)
                .filter(|variant| !variant.uninhabited)
                .ok_or_else(|| {
                    engine.at(
                        *site,
                        storage_violation_v1("construction variant is absent or uninhabited"),
                    )
                })?;
            (0, variant.layout, base.pointer.storage_guard)
        }
        StorageProjectionV1::Variant { index, access } => {
            let StorageLayoutKindV1::Variants { encoding, variants } = &row.kind else {
                return Err(engine.at(
                    *site,
                    storage_violation_v1("variant projection requires variant layout"),
                ));
            };
            let (active, tag_address, width) = storage_active_variant_v1(
                engine, base, *encoding, variants, access, invocation, site,
            )?;
            let variant = variants
                .get(index as usize)
                .filter(|variant| !variant.uninhabited)
                .filter(|_| active == Some(index as usize))
                .ok_or_else(|| {
                    engine.at(
                        *site,
                        storage_violation_v1(
                            "variant projection does not match the actual active tag",
                        ),
                    )
                })?;
            let start = tag_address.pointer.byte_offset;
            let end = start.checked_add(width).ok_or_else(|| {
                engine.at(*site, SimulationExecutionErrorKindV1::PointerOffsetOverflow)
            })?;
            let guard = engine
                .memory
                .storage_register_guard_v1(base, start, end)
                .map_err(|kind| engine.at(*site, kind))?;
            (0, variant.layout, Some(guard))
        }
    };
    if result.address_space != base.pointer.visible_address_space()
        || result.pointee.as_ref() != &Type::StorageObject(layout)
    {
        return Err(engine.at(
            *site,
            storage_violation_v1("projection changes holder space or local row"),
        ));
    }
    let child_row = storage_row_v1(engine, layout, site)?;
    let mut child = engine
        .memory
        .storage_child_v1(base, offset, layout, child_row, result.access)
        .map_err(|kind| engine.at(*site, kind))?;
    child.pointer.storage_guard = guard;
    Ok(child)
}

fn storage_active_variant_v1(
    engine: &mut Engine<'_, impl SimulationEventSinkV1>,
    base: &StorageAddressV1,
    encoding: StorageVariantEncodingV1,
    variants: &[fe2o3_kernel_ir::StorageVariantV1],
    access: MemoryAccess,
    invocation: SimulationInvocationV1,
    site: &CompactSite,
) -> Result<(Option<usize>, StorageAddressV1, usize), SimulationExecutionErrorV1> {
    let tag = encoding.tag();
    let tag_row = storage_row_v1(engine, tag.layout, site)?;
    let mut tag_address = engine.memory
        .storage_child_v1(base, tag.offset, tag.layout, tag_row, base.pointer.access)
        .map_err(|kind| engine.at(*site, kind))?;
    let width = storage_extent_v1(tag_row).map_err(|kind| engine.at(*site, kind))?;
    engine.memory.storage_validate_v1(&tag_address, access, width, false, invocation)
        .map_err(|kind| engine.at(*site, kind))?;
    let symbolic = match tag_row.kind {
        StorageLayoutKindV1::Scalar(scalar) => {
            if (!scalar.is_integer() && scalar != ScalarType::Bool)
                || engine.target.scalar_bytes(scalar) != Some(width)
            {
                return Err(engine.at(*site, storage_violation_v1("variant tag differs from target integer or bool width")));
            }
            tag_address.pointer.element = scalar;
            None
        }
        StorageLayoutKindV1::Pointer(pointer) if matches!(encoding, StorageVariantEncodingV1::Niche { .. }) => {
            let recipe = engine.target.storage_pointer_encoding(pointer)
                .filter(|recipe| usize::from(recipe.bits() / 8) == width
                    && tag_row.alignment == u32::from(recipe.bits() / 8))
                .ok_or_else(|| engine.at(*site, storage_violation_v1("pointer niche requires an explicit exact AMDGPU encoding profile")))?;
            storage_pointer_tag_relocated_v1(engine, &tag_address, pointer, access, width, invocation, site)?
                .then_some(recipe)
        }
        _ => return Err(engine.at(*site, storage_violation_v1("variant tag is neither an integer nor a qualified pointer niche"))),
    };
    if let Some(recipe) = symbolic {
        let active = storage_nonnull_pointer_variant_v1(recipe, encoding, variants.len(), &engine.memory.storage_accounting)
            .map_err(|kind| engine.at(*site, kind))?;
        storage_observe_v1(engine, &tag_address, width, false, site)?;
        return Ok((Some(active), tag_address, width));
    }
    let tag_bits = storage_tag_bits_v1(engine, &tag_address, access, width, invocation, site)?;
    let active = storage_raw_variant_v29(tag_bits, width, encoding, variants, &engine.memory.storage_accounting)
        .map_err(|kind| engine.at(*site, kind))?;
    Ok((active, tag_address, width))
}

fn storage_pointer_tag_relocated_v1(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    address: &StorageAddressV1,
    representation: fe2o3_kernel_ir::StoragePointerV1,
    access: MemoryAccess,
    width: usize,
    invocation: SimulationInvocationV1,
    site: &CompactSite,
) -> Result<bool, SimulationExecutionErrorV1> {
    engine.memory.storage_value_ready_v1(address, access, width, invocation)
        .map_err(|kind| engine.at(*site, kind))?;
    let allocation = engine.memory.allocation(&address.pointer)
        .map_err(|kind| engine.at(*site, kind))?;
    let start = address.pointer.byte_offset;
    let end = start.checked_add(width)
        .ok_or_else(|| engine.at(*site, SimulationExecutionErrorKindV1::PointerOffsetOverflow))?;
    engine.memory.storage_accounting.charge(allocation.storage.relocations.len())
        .map_err(|kind| engine.at(*site, kind))?;
    let mut found = false;
    for relocation in &allocation.storage.relocations {
        if !storage_overlap_v1(start, end, relocation.start, relocation.end) { continue; }
        if found || relocation.start != start || relocation.end != end
            || relocation.representation != representation
        {
            return Err(engine.at(*site, storage_violation_v1("pointer tag has a partial or incompatible relocation")));
        }
        engine.memory.storage_validate_pointer_v1(&relocation.value, representation, invocation)
            .map_err(|kind| engine.at(*site, kind))?;
        found = true;
    }
    Ok(found)
}

// Called only after a complete current relocation proves a live bounded pointer.
// The full roster is prepaid atomically. Placeholder bytes are never read.
fn storage_nonnull_pointer_variant_v1(
    recipe: fe2o3_amd_target::AmdPointerEncodingV1,
    encoding: StorageVariantEncodingV1,
    count: usize,
    accounting: &StorageAccountingV1,
) -> Result<usize, SimulationExecutionErrorKindV1> {
    storage_nonnull_pointer_variant_v29(recipe, encoding, count, accounting)
}

#[cfg(test)]
mod pointer_niche_domain_tests {
    use super::*;

    #[test]
    fn symbolic_domain_decode_has_independent_exact_and_one_short_roster_work() {
        for limit in [2, 1] {
            let accounting = StorageAccountingV1::new(SimulationLimitsV1 {
                max_steps: limit, ..SimulationLimitsV1::default()
            });
            accounting.hold(17).unwrap();
            let recipe = fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950
                .pointer_encoding(5).unwrap();
            let encoding = StorageVariantEncodingV1::Niche {
                tag: fe2o3_kernel_ir::StorageFieldV1 { offset: 0, layout: StorageLayoutIdV1(1) },
                untagged_variant: 0, first_niche_variant: 1, last_niche_variant: 1,
                niche_start: u32::MAX as u128,
            };
            let result = storage_nonnull_pointer_variant_v1(recipe, encoding, 2, &accounting);
            if limit == 2 {
                assert_eq!(result.unwrap(), 0);
                assert_eq!(accounting.steps(), 2);
            } else {
                assert!(matches!(result, Err(SimulationExecutionErrorKindV1::StepLimit { limit: 1 })));
                assert_eq!(accounting.steps(), 0);
            }
            assert_eq!(accounting.held(), 17);
            accounting.release(17);
        }
    }

    #[test]
    fn symbolic_domain_considers_every_niche_value_not_just_relocation_presence() {
        for space in [0, 1, 3, 4, 5] {
            let recipe = fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942
                .pointer_encoding(space).unwrap();
            let accounting = StorageAccountingV1::new(SimulationLimitsV1::default());
            let encoding = StorageVariantEncodingV1::Niche {
                tag: fe2o3_kernel_ir::StorageFieldV1 { offset: 0, layout: StorageLayoutIdV1(1) },
                untagged_variant: 0, first_niche_variant: 1, last_niche_variant: 2,
                niche_start: recipe.null_bits(),
            };
            assert!(matches!(storage_nonnull_pointer_variant_v1(recipe, encoding, 3, &accounting),
                Err(SimulationExecutionErrorKindV1::StorageViolation {
                    reason: "symbolic pointer domain crosses possible logical variants",
                })));
            assert_eq!(accounting.steps(), 3);
            assert_eq!(accounting.held(), 0);
        }
    }
}

fn storage_tag_bits_v1(
    engine: &mut Engine<'_, impl SimulationEventSinkV1>,
    address: &StorageAddressV1,
    access: MemoryAccess,
    width: usize,
    invocation: SimulationInvocationV1,
    site: &CompactSite,
) -> Result<u128, SimulationExecutionErrorV1> {
    engine
        .memory
        .storage_value_ready_v1(address, access, width, invocation)
        .map_err(|kind| engine.at(*site, kind))?;
    let allocation = engine
        .memory
        .allocation(&address.pointer)
        .map_err(|kind| engine.at(*site, kind))?;
    let start = address.pointer.byte_offset;
    allocation
        .storage
        .raw_read(start, start + width, &engine.memory.storage_accounting)
        .map_err(|kind| engine.at(*site, kind))?;
    engine
        .memory
        .storage_accounting
        .charge(width)
        .map_err(|kind| engine.at(*site, kind))?;
    let mut bytes = [0_u8; 16];
    bytes[..width].copy_from_slice(&allocation.bytes[start..start + width]);
    let bits = u128::from_le_bytes(bytes);
    storage_observe_v1(engine, address, width, false, site)?;
    Ok(bits)
}

fn storage_set_discriminant_v1(
    engine: &mut Engine<'_, impl SimulationEventSinkV1>,
    address: &StorageAddressV1,
    index: u32,
    access: MemoryAccess,
    invocation: SimulationInvocationV1,
    site: &CompactSite,
) -> Result<(), SimulationExecutionErrorV1> {
    let row = storage_row_v1(engine, address.layout, site)?;
    let StorageLayoutKindV1::Variants { encoding, variants } = &row.kind else {
        return Err(engine.at(
            *site,
            storage_violation_v1("set discriminant requires variant layout"),
        ));
    };
    if access.volatile || address.pointer.address_space == AddressSpace::Constant {
        return Err(engine.at(
            *site,
            storage_violation_v1("set discriminant requires nonvolatile writable storage"),
        ));
    }
    let variant = variants
        .get(index as usize)
        .filter(|variant| !variant.uninhabited)
        .ok_or_else(|| {
            engine.at(
                *site,
                storage_violation_v1("set discriminant variant is absent or uninhabited"),
            )
        })?;
    engine
        .memory
        .storage_validate_v1(
            address,
            MemoryAccess {
                alignment: 1,
                ..access
            },
            storage_extent_v1(row).map_err(|kind| engine.at(*site, kind))?,
            true,
            invocation,
        )
        .map_err(|kind| engine.at(*site, kind))?;
    if matches!(encoding, StorageVariantEncodingV1::Niche { untagged_variant, .. } if *untagged_variant == index)
    {
        return Ok(());
    }
    let tag = encoding.tag();
    let tag_row = storage_row_v1(engine, tag.layout, site)?;
    let width = storage_extent_v1(tag_row).map_err(|kind| engine.at(*site, kind))?;
    if ![1, 2, 4, 8, 16].contains(&width) {
        return Err(engine.at(
            *site,
            storage_violation_v1("set discriminant physical width is unsupported"),
        ));
    }
    if let StorageLayoutKindV1::Scalar(scalar) = tag_row.kind {
        if engine.target.scalar_bytes(scalar) != Some(width) {
            return Err(engine.at(
                *site,
                storage_violation_v1(
                    "set discriminant tag layout differs from target scalar width",
                ),
            ));
        }
    }
    let physical_bits = (width * 8) as u16;
    let bits = match *encoding {
        StorageVariantEncodingV1::Direct { .. } => variant
            .direct_tag_bits
            .filter(|bits| *bits & mask(physical_bits) == *bits)
            .ok_or_else(|| {
                engine.at(
                    *site,
                    storage_violation_v1("direct discriminant has no exact tag bits"),
                )
            })?,
        StorageVariantEncodingV1::Niche {
            first_niche_variant,
            last_niche_variant,
            niche_start,
            ..
        } => {
            if !(first_niche_variant..=last_niche_variant).contains(&index) {
                return Err(engine.at(
                    *site,
                    storage_violation_v1("discriminant is outside the encoded niche range"),
                ));
            }
            niche_start.wrapping_add(u128::from(index - first_niche_variant)) & mask(physical_bits)
        }
    };
    let tag_address = engine
        .memory
        .storage_child_v1(
            address,
            tag.offset,
            tag.layout,
            tag_row,
            address.pointer.access,
        )
        .map_err(|kind| engine.at(*site, kind))?;
    engine
        .memory
        .storage_validate_v1(&tag_address, access, width, true, invocation)
        .map_err(|kind| engine.at(*site, kind))?;
    let mut snapshot = StorageSnapshotV1::new(&engine.memory.storage_accounting)
        .map_err(|kind| engine.at(*site, kind))?;
    let result = (|| {
        storage_reserve_v1(
            &mut snapshot.bytes,
            width,
            &engine.memory.storage_accounting,
        )
        .map_err(|kind| engine.at(*site, kind))?;
        storage_reserve_v1(
            &mut snapshot.initialized,
            width,
            &engine.memory.storage_accounting,
        )
        .map_err(|kind| engine.at(*site, kind))?;
        engine.charge_steps(site, 2 * width)?;
        snapshot
            .bytes
            .extend_from_slice(&bits.to_le_bytes()[..width]);
        snapshot.initialized.resize(width, true);
        engine
            .memory
            .storage_prepare_copy_v1(&tag_address, &snapshot)
            .map_err(|kind| engine.at(*site, kind))?;
        let writer = if address.pointer.address_space == AddressSpace::Workgroup {
            Some(
                invocation_local_ordinal(invocation)
                    .and_then(|value| value.checked_add(1))
                    .ok_or_else(|| {
                        engine.at(
                            *site,
                            storage_violation_v1("storage workgroup writer ordinal overflow"),
                        )
                    })?,
            )
        } else {
            None
        };
        storage_observe_v1(engine, &tag_address, width, true, site)?;
        engine
            .memory
            .storage_commit_copy_v1(&tag_address, &snapshot, writer);
        Ok(())
    })();
    snapshot.release(&engine.memory.storage_accounting);
    result
}
