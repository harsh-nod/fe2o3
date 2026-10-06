use super::*;

// These exercise the inert recorder contract, not source/physical admission.
fn endpoint(local: u32, count: usize) -> ScopedObjectEndpointV29 {
    let local = SemanticLocalIdV1::from_index(local);
    ScopedObjectEndpointV29 {
        object: ScopedObjectIdentityV29::Local {
            instance: ProductionCallInstanceIdV1(0),
            local,
            generation: 0,
        },
        source: ScopedObjectSourceV29::EntryComponent {
            local,
            argument: 0,
            abi_piece: 0,
            byte_offset: 0,
            bit_width: 32,
        },
        root_type: SemanticTypeIdV1::from_index(0),
        projected_type: SemanticTypeIdV1::from_index(0),
        root_schema: fe2o3_kernel_ir::StorageLayoutIdV1(0),
        projected_schema: fe2o3_kernel_ir::StorageLayoutIdV1(0),
        source_path: ScopedObjectPathV29 { first: 0, count: 0 },
        path: ScopedObjectPathV29 { first: 0, count },
    }
}

fn payload_cases() -> Vec<(ScopedObjectPayloadV29, Operation)> {
    use fe2o3_kernel_ir::{
        StorageCopyOverlapV1, StorageOperationV1 as O, StorageProjectionV1 as P,
    };
    let source = endpoint(0, 0);
    let destination = endpoint(1, 0);
    let access = MemoryAccess::new(AddressSpace::Private, 4);
    let mut rows = Vec::new();
    for step in [
        P::Field(2),
        P::ArrayIndex(ValueId(8)),
        P::Variant { index: 3, access },
        P::VariantForWrite { index: 3 },
    ] {
        let operation = O::Project {
            base: ValueId(7),
            step,
        };
        let ty = Type::Pointer(fe2o3_kernel_ir::PointerType::new(
            Type::StorageObject(source.projected_schema),
            AddressSpace::Private,
            if matches!(step, P::VariantForWrite { .. }) {
                AccessMode::WriteOnly
            } else {
                AccessMode::ReadWrite
            },
        ));
        let payload = ScopedObjectPayloadV29 {
            operation,
            result: Some(ValueId(30)),
            role: ScopedObjectRoleV29::Project {
                source,
                projected: endpoint(0, 1),
            },
        };
        rows.push((
            payload,
            Operation::new(
                vec![ValueDef::new(ValueId(30), ty)],
                OperationKind::Storage(operation),
            ),
        ));
    }
    for (operation, role, ty) in [
        (
            O::ReadValue {
                address: ValueId(7),
                access,
            },
            ScopedObjectRoleV29::ReadValue {
                source,
                read: ScopedObjectReadOriginV29::EntryComponent,
            },
            Some(Type::Scalar(ScalarType::U32)),
        ),
        (
            O::ReadDiscriminant {
                address: ValueId(7),
                access,
            },
            ScopedObjectRoleV29::ReadDiscriminant {
                source,
                origin: ScopedObjectTagOriginV29::EntryComponent,
            },
            Some(Type::Scalar(ScalarType::U128)),
        ),
        (
            O::WriteValue {
                address: ValueId(7),
                value: ValueId(8),
                access,
            },
            ScopedObjectRoleV29::WriteValue {
                destination,
                value: ScopedObjectValueOriginV29::EntryComponent {
                    local: SemanticLocalIdV1::from_index(0),
                    argument: 0,
                    abi_piece: 0,
                    byte_offset: 0,
                    bit_width: 32,
                },
            },
            None,
        ),
        (
            O::CopyObject {
                source: ValueId(7),
                destination: ValueId(8),
                source_access: access,
                destination_access: MemoryAccess::new(AddressSpace::Private, 8),
                overlap: StorageCopyOverlapV1::MayOverlap,
            },
            ScopedObjectRoleV29::CopyObject {
                source,
                destination,
            },
            None,
        ),
        (
            O::SetDiscriminant {
                address: ValueId(7),
                variant: 3,
                access,
            },
            ScopedObjectRoleV29::SetDiscriminant {
                destination,
                origin: ScopedObjectTagOriginV29::EntryComponent,
                variant: 3,
            },
            None,
        ),
    ] {
        let result = ty.as_ref().map(|_| ValueId(30));
        let results = ty
            .into_iter()
            .map(|ty| ValueDef::new(ValueId(30), ty))
            .collect();
        rows.push((
            ScopedObjectPayloadV29 {
                operation,
                role,
                result,
            },
            Operation::new(results, OperationKind::Storage(operation)),
        ));
    }
    rows
}

#[test]
fn inert_payloads_preserve_all_nine_storage_shapes_and_exact_result_arity() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    let cases = payload_cases();
    assert_eq!(cases.len(), 9);
    for (payload, operation) in cases {
        payload.check_operation(&operation, &mut budget).unwrap();
        let mut operands = Vec::new();
        payload
            .operation
            .try_visit_operands(|value| {
                operands.push(value);
                Ok::<_, ()>(())
            })
            .unwrap();
        assert_eq!(
            payload.operands().into_iter().flatten().collect::<Vec<_>>(),
            operands
        );
        let mut changed = operation.clone();
        if changed.results.is_empty() {
            changed
                .results
                .push(ValueDef::new(ValueId(90), Type::Scalar(ScalarType::U32)));
        } else {
            changed.results.clear();
        }
        assert!(payload.check_operation(&changed, &mut budget).is_err());
        if payload.result.is_some() {
            let mut changed = operation.clone();
            changed.results[0].id = ValueId(91);
            assert!(payload.check_operation(&changed, &mut budget).is_err());
        }
    }
}

#[test]
fn inert_payload_relocation_is_exhaustive_atomic_and_preserves_original_roles() {
    for (original, _) in payload_cases() {
        let expected = original
            .operands()
            .into_iter()
            .flatten()
            .chain(original.result)
            .collect::<Vec<_>>();
        for fail in 0..expected.len() {
            let mut payload = original;
            let mut seen = Vec::new();
            assert!(
                payload
                    .try_map_values(|value| {
                        seen.push(value);
                        if seen.len() - 1 == fail {
                            return Err(scoped_object_error_v29());
                        }
                        Ok(ValueId(value.0 + 100))
                    })
                    .is_err()
            );
            assert_eq!(payload, original);
            assert_eq!(seen, expected[..=fail]);
        }
        let mut payload = original;
        let mut seen = Vec::new();
        payload
            .try_map_values(|value| {
                seen.push(value);
                Ok(ValueId(value.0 + 100))
            })
            .unwrap();
        assert_eq!(seen, expected);
        assert_eq!(payload.role, original.role);
        assert_eq!(
            payload
                .operands()
                .into_iter()
                .flatten()
                .chain(payload.result)
                .collect::<Vec<_>>(),
            expected
                .iter()
                .map(|value| ValueId(value.0 + 100))
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn copy_endpoints_accesses_overlap_and_tag_bits_cannot_be_substituted() {
    use fe2o3_kernel_ir::{StorageCopyOverlapV1, StorageOperationV1 as O};
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    let (copy, original) = payload_cases()
        .into_iter()
        .find(|(row, _)| matches!(row.operation, O::CopyObject { .. }))
        .unwrap();
    let O::CopyObject {
        source,
        destination,
        source_access,
        destination_access,
        overlap,
    } = copy.operation
    else {
        unreachable!()
    };
    for changed in [
        O::CopyObject {
            source: destination,
            destination: source,
            source_access,
            destination_access,
            overlap,
        },
        O::CopyObject {
            source,
            destination,
            source_access: destination_access,
            destination_access: source_access,
            overlap,
        },
        O::CopyObject {
            source,
            destination,
            source_access,
            destination_access,
            overlap: StorageCopyOverlapV1::NonOverlapping,
        },
    ] {
        let mut actual = original.clone();
        actual.kind = OperationKind::Storage(changed);
        assert!(copy.check_operation(&actual, &mut budget).is_err());
    }
    let mut payload = copy;
    let ScopedObjectRoleV29::CopyObject { destination, .. } = &mut payload.role else {
        unreachable!()
    };
    destination.projected_schema = fe2o3_kernel_ir::StorageLayoutIdV1(1);
    assert!(payload.check_operation(&original, &mut budget).is_err());
    let (tag, mut actual) = payload_cases()
        .into_iter()
        .find(|(row, _)| matches!(row.operation, O::ReadDiscriminant { .. }))
        .unwrap();
    actual.results[0].ty = Type::Scalar(ScalarType::U64);
    assert!(tag.check_operation(&actual, &mut budget).is_err());
    let (mut tag, actual) = payload_cases()
        .into_iter()
        .find(|(row, _)| matches!(row.operation, O::SetDiscriminant { .. }))
        .unwrap();
    let ScopedObjectRoleV29::SetDiscriminant { variant, .. } = &mut tag.role else {
        unreachable!()
    };
    *variant = 2;
    assert!(tag.check_operation(&actual, &mut budget).is_err());
}

fn fresh_anchors(
    subject: ScopedInitializationSubjectV29,
    placement: SemanticEmissionPlacementV1,
) -> ScopedMemoryAnchorsV29 {
    ScopedMemoryAnchorsV29 {
        subject,
        placement,
        rows: Vec::new(),
        objects: Vec::new(),
        object_components: Vec::new(),
        zero_objects: Vec::new(),
        compiler_enum: Vec::new(),
    }
}

fn inspect_inert_object_append(
    lifecycle: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let original = emitted
        .iter()
        .flatten()
        .find_map(|row| row.scoped_memory_anchors.as_ref())
        .unwrap();
    let mut anchors = fresh_anchors(original.subject, original.placement);
    let copy = payload_cases()
        .into_iter()
        .find(|(row, _)| matches!(row.operation, ScopedObjectOperationV29::CopyObject { .. }))
        .unwrap()
        .0;
    with_canonical_call_scratch_v1(budget, |budget| {
        anchors.append_object(BlockId(1), 2, None, copy, budget)?;
        assert_eq!((anchors.rows.len(), anchors.objects.len()), (1, 1));
        assert_eq!(anchors.rows[0].kind, ScopedMemoryAnchorKindV29::Object(0));
        assert_eq!(*anchors.object_payload(&anchors.rows[0], budget)?, copy);
        let mut wrong = anchors.rows[0];
        wrong.kind = ScopedMemoryAnchorKindV29::Object(1);
        assert!(anchors.object_payload(&wrong, budget).is_err());
        let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, usize::MAX);
        assert!(
            anchors
                .object_payload(&anchors.rows[0], &mut foreign)
                .is_err()
        );
        drop(anchors);
        Ok(())
    })?;
    observe(lifecycle, instances, emitted, slots, budget)
}

#[test]
fn inert_object_rows_use_the_existing_authenticated_recorder_subject() {
    let (result, _, _) = run(
        false,
        ScopedFixture::CheckedSlot,
        inspect_inert_object_append,
        LIMIT,
        LIMIT,
    );
    assert!(is_stopped(&result), "{result:?}");
}

fn inspect_original_source_recipe(
    lifecycle: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut checked = 0;
    for lowered in emitted.iter().flatten() {
        let Some(original) = &lowered.scoped_memory_anchors else {
            continue;
        };
        let instance = original.subject.instance;
        let source = instances.instance(instance).unwrap().declaration();
        let occurrences = instances.occurrences(instance).unwrap();
        for row in &original.rows {
            let ScopedMemoryAnchorKindV29::Access {
                pointer,
                payload: Some(ScopedMemoryPayloadV29::Load { result, read }),
            } = row.kind
            else {
                continue;
            };
            if read.prefix != 0 {
                continue;
            }
            let place = scoped_payload_place_v29(source, read.site, read.role).unwrap();
            let actual = &lowered
                .function
                .body
                .as_ref()
                .unwrap()
                .blocks
                .iter()
                .find(|block| block.id == row.block)
                .unwrap()
                .operations[row.position];
            let access = match actual.kind {
                OperationKind::Load { access, .. } | OperationKind::GuardedLoad { access, .. } => {
                    access
                }
                _ => unreachable!(),
            };
            let mut endpoint = endpoint(place.local().index(), 0);
            endpoint.object = ScopedObjectIdentityV29::Local {
                instance,
                local: place.local(),
                generation: 0,
            };
            endpoint.root_type = source.locals()[place.local().index() as usize].ty();
            endpoint.projected_type = read.ty;
            endpoint.source = ScopedObjectSourceV29::Place {
                site: read.site,
                role: read.role,
                local: place.local(),
                prefix: read.prefix,
            };
            let payload = ScopedObjectPayloadV29 {
                operation: ScopedObjectOperationV29::ReadValue {
                    address: pointer,
                    access,
                },
                result: Some(result),
                role: ScopedObjectRoleV29::ReadValue {
                    source: endpoint,
                    read: ScopedObjectReadOriginV29::Original(read),
                },
            };
            let anchor = ScopedMemoryAnchorV29 {
                kind: ScopedMemoryAnchorKindV29::Object(0),
                ..*row
            };
            let anchors = fresh_anchors(original.subject, original.placement);
            anchors.check_object_source(source, &occurrences, 0, &anchor, &payload, budget)?;
            for mutation in 0..7 {
                let mut forged = payload;
                let ScopedObjectRoleV29::ReadValue {
                    source: endpoint,
                    read,
                } = &mut forged.role
                else {
                    unreachable!()
                };
                match mutation {
                    0 => {
                        endpoint.object = ScopedObjectIdentityV29::Local {
                            instance: ProductionCallInstanceIdV1(usize::MAX),
                            local: place.local(),
                            generation: 0,
                        }
                    }
                    1 => endpoint.root_type = SemanticTypeIdV1::from_index(u32::MAX),
                    2 => endpoint.projected_type = SemanticTypeIdV1::from_index(u32::MAX),
                    3 => endpoint.path = ScopedObjectPathV29 { first: 1, count: 0 },
                    4 => {
                        let ScopedObjectReadOriginV29::Original(read) = read else {
                            unreachable!()
                        };
                        read.occurrence = ScopedMemoryOccurrenceV29::Retained { event: usize::MAX };
                    }
                    5 => {
                        let ScopedObjectReadOriginV29::Original(read) = read else {
                            unreachable!()
                        };
                        read.ty = SemanticTypeIdV1::from_index(u32::MAX);
                    }
                    _ => endpoint.source_path = ScopedObjectPathV29 { first: 1, count: 0 },
                }
                assert!(
                    anchors
                        .check_object_source(source, &occurrences, 0, &anchor, &forged, budget)
                        .is_err()
                );
            }
            checked += 1;
        }
    }
    assert!(
        checked >= 2,
        "the original source fixture must exercise shared helper instances"
    );
    observe(lifecycle, instances, emitted, slots, budget)
}

#[test]
fn typed_recipe_source_queries_reuse_original_instance_and_occurrence_coordinates() {
    // Changing the physical opcode is not an admission test; this only checks
    // the typed recorder's inert recipe against actual original source capture.
    let (result, _, _) = run(
        false,
        ScopedFixture::CheckedSlot,
        inspect_original_source_recipe,
        LIMIT,
        LIMIT,
    );
    assert!(is_stopped(&result), "{result:?}");
}

fn inspect_inert_object_census(
    lifecycle: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut checked = 0;
    for item in &slots.instances {
        let lowered = emitted[item.instance.index()].as_mut().unwrap();
        let declaration = instances.instance(item.instance).unwrap().declaration();
        let anchors = lowered.scoped_memory_anchors.as_ref().unwrap();
        assert!(anchors.objects.is_empty() && anchors.object_components.is_empty());
        let selected = anchors.rows.iter().enumerate().find_map(|(index, row)| {
            let ScopedMemoryAnchorKindV29::Access {
                pointer,
                payload: Some(ScopedMemoryPayloadV29::Load { result, read }),
            } = row.kind
            else {
                return None;
            };
            if read.prefix != 0
                || anchors.rows.iter().any(|other| {
                    matches!(other.kind,
                ScopedMemoryAnchorKindV29::Access { payload: Some(ScopedMemoryPayloadV29::Store {
                    source: ScopedMemoryStoreSourceV29::Operand {
                        source: ScopedMemoryOperandSourceV29::Memory { access, .. }, .. }, .. }), ..
                } if access == index)
                })
            {
                return None;
            }
            Some((index, *row, pointer, result, read))
        });
        let Some((index, row, pointer, result, read)) = selected else {
            continue;
        };
        let block = lowered
            .function
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .position(|block| block.id == row.block)
            .unwrap();
        let original_operation =
            lowered.function.body.as_ref().unwrap().blocks[block].operations[row.position].clone();
        let access = match original_operation.kind {
            OperationKind::Load { access, .. } | OperationKind::GuardedLoad { access, .. } => {
                access
            }
            _ => unreachable!(),
        };
        let place = scoped_payload_place_v29(declaration, read.site, read.role).unwrap();
        let mut source = endpoint(place.local().index(), 0);
        source.object = ScopedObjectIdentityV29::Local {
            instance: item.instance,
            local: place.local(),
            generation: 0,
        };
        source.root_type = declaration.locals()[place.local().index() as usize].ty();
        source.projected_type = read.ty;
        source.source = ScopedObjectSourceV29::Place {
            site: read.site,
            role: read.role,
            local: place.local(),
            prefix: 0,
        };
        let payload = ScopedObjectPayloadV29 {
            operation: ScopedObjectOperationV29::ReadValue {
                address: pointer,
                access,
            },
            result: Some(result),
            role: ScopedObjectRoleV29::ReadValue {
                source,
                read: ScopedObjectReadOriginV29::Original(read),
            },
        };
        let anchors = lowered.scoped_memory_anchors.as_mut().unwrap();
        let original_rows = std::mem::take(&mut anchors.rows);
        let original_objects = std::mem::take(&mut anchors.objects);
        let original_components = std::mem::take(&mut anchors.object_components);
        for mutation in 0..9 {
            let anchors = lowered.scoped_memory_anchors.as_mut().unwrap();
            anchors.rows = original_rows.clone();
            anchors.rows[index].kind = ScopedMemoryAnchorKindV29::Object(0);
            anchors.objects = vec![payload];
            anchors.object_components.clear();
            let operation =
                &mut lowered.function.body.as_mut().unwrap().blocks[block].operations[row.position];
            *operation = original_operation.clone();
            operation.kind = OperationKind::Storage(payload.operation);
            match mutation {
                0 => {}
                1 => anchors.rows[index].kind = row.kind,
                2 => {
                    anchors.objects.pop();
                }
                3 => anchors.objects.push(payload),
                4 => {
                    let duplicate = anchors.rows[index];
                    anchors.rows.insert(index, duplicate);
                }
                5 => anchors.rows[index].kind = ScopedMemoryAnchorKindV29::Object(1),
                6 => anchors
                    .object_components
                    .push(ScopedObjectComponentV29::View {
                        projection: ScopedObjectViewProjectionV29::Field(0),
                        ty: source.root_type,
                    }),
                7 => operation.results[0].id = ValueId(u32::MAX),
                8 => operation.kind = original_operation.kind.clone(),
                _ => unreachable!(),
            }
            let result = with_canonical_call_scratch_v1(budget, |budget| {
                check_scoped_memory_anchors_v29(
                    instances,
                    item,
                    lowered,
                    &slots.slots[item.slots.clone()],
                    budget,
                )
            });
            assert_eq!(
                result.is_ok(),
                mutation == 0,
                "inert census mutation {mutation}: {result:?}"
            );
        }
        let anchors = lowered.scoped_memory_anchors.as_mut().unwrap();
        anchors.rows = original_rows;
        anchors.objects = original_objects;
        anchors.object_components = original_components;
        lowered.function.body.as_mut().unwrap().blocks[block].operations[row.position] =
            original_operation;
        checked += 1;
    }
    assert!(
        checked >= 2,
        "census fixture must retain distinct original instances"
    );
    observe(lifecycle, instances, emitted, slots, budget)
}

#[test]
fn inert_object_census_rejects_missing_duplicate_orphan_and_changed_rows() {
    // Deliberately constructed Storage records test only the census contract.
    // They are not typed emission, schema, backend or currentness evidence.
    let (result, _, _) = run(
        false,
        ScopedFixture::CheckedSlot,
        inspect_inert_object_census,
        LIMIT,
        LIMIT,
    );
    assert!(is_stopped(&result), "{result:?}");
}

pub(super) fn check_role_frames(
    lowering: &mut SemanticFunctionLoweringV1<'_, '_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let payload = payload_cases()
        .into_iter()
        .find(|(row, _)| matches!(row.operation, ScopedObjectOperationV29::CopyObject { .. }))
        .unwrap()
        .0;
    for mode in 0..4 {
        let checkpoint = lowering
            .scoped_memory
            .as_ref()
            .unwrap()
            .anchors
            .object_checkpoint();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            lowering.with_scoped_object_role_v29(payload.role, |lowering| {
                lowering.with_emission_budget_v1(|lowering, budget| {
                    lowering
                        .scoped_memory
                        .as_mut()
                        .unwrap()
                        .anchors
                        .append_object_path(
                            &[ScopedObjectComponentV29::View {
                                projection: ScopedObjectViewProjectionV29::Field(0),
                                ty: SemanticTypeIdV1::from_index(0),
                            }],
                            budget,
                        )?;
                    Ok(())
                })?;
                if mode == 1 {
                    return Err(scoped_memory_error_v29());
                }
                if mode == 2 {
                    std::panic::panic_any(87_u32);
                }
                if mode == 3 {
                    return Ok(());
                }
                lowering.record_scoped_object_v29(0, payload.operation, &[])
            })
        }));
        match mode {
            0 => assert!(result.unwrap().is_ok()),
            1 | 3 => assert!(result.unwrap().is_err()),
            2 => assert_eq!(*result.unwrap_err().downcast::<u32>().unwrap(), 87),
            _ => unreachable!(),
        }
        let recorder = lowering.scoped_memory.as_mut().unwrap();
        assert!(recorder.object_role.is_none());
        assert_eq!(
            recorder.anchors.rows.len(),
            checkpoint.rows + usize::from(mode == 0)
        );
        assert_eq!(
            recorder.anchors.objects.len(),
            checkpoint.objects + usize::from(mode == 0)
        );
        assert_eq!(
            recorder.anchors.object_components.len(),
            checkpoint.components + 1
        );
        let capacities = (
            recorder.anchors.rows.capacity(),
            recorder.anchors.objects.capacity(),
            recorder.anchors.object_components.capacity(),
        );
        recorder.anchors.rollback_objects(checkpoint);
        assert_eq!(
            (
                recorder.anchors.rows.len(),
                recorder.anchors.objects.len(),
                recorder.anchors.object_components.len()
            ),
            (checkpoint.rows, checkpoint.objects, checkpoint.components)
        );
        assert_eq!(
            (
                recorder.anchors.rows.capacity(),
                recorder.anchors.objects.capacity(),
                recorder.anchors.object_components.capacity()
            ),
            capacities
        );
    }
    Ok(())
}

#[test]
fn inert_path_visitors_cover_original_view_and_value_component_paths_separately() {
    let mut destination = endpoint(0, 2);
    destination.source_path = ScopedObjectPathV29 { first: 7, count: 3 };
    destination.path.first = 11;
    let component = ScopedObjectPathV29 {
        first: 19,
        count: 4,
    };
    let role = ScopedObjectRoleV29::WriteValue {
        destination,
        value: ScopedObjectValueOriginV29::Component {
            original: ScopedMemoryStoreSourceV29::EntryArgument {
                local: SemanticLocalIdV1::from_index(0),
                ty: destination.root_type,
            },
            path: component,
        },
    };
    let mut visited = Vec::new();
    role.visit_paths(|path| {
        visited.push(path);
        Ok(())
    })
    .unwrap();
    assert_eq!(
        visited,
        [destination.source_path, destination.path, component]
    );
    for limit in 0..visited.len() {
        let mut actual = Vec::new();
        assert!(
            role.visit_paths(|path| {
                actual.push(path);
                if actual.len() == limit + 1 {
                    return Err(scoped_object_error_v29());
                }
                Ok(())
            })
            .is_err()
        );
        assert_eq!(actual, visited[..=limit]);
    }
}

fn inspect_original_reference_locators(
    _lifecycle: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    _slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut checked = 0;
    for lowered in emitted.iter().flatten() {
        let Some(anchors) = &lowered.scoped_memory_anchors else {
            continue;
        };
        let instance = anchors.subject.instance;
        let function = instances.instance(instance).unwrap().declaration();
        let occurrences = instances.occurrences(instance).unwrap();
        for event in occurrences.events() {
            let (site, role) = (event.site(), event.operand());
            let Some(place) = scoped_object_original_place_v29(function, site, role) else {
                continue;
            };
            for (index, projection) in place.projections().iter().enumerate() {
                if projection.kind() != SemanticProjectionKindV1::Dereference {
                    continue;
                }
                let prefix = index as u32 + 1;
                let mut view = endpoint(place.local().index(), 0);
                view.object = ScopedObjectIdentityV29::Reference {
                    instance,
                    site,
                    role,
                    dereference_prefix: prefix,
                };
                view.source = ScopedObjectSourceV29::Place {
                    site,
                    role,
                    local: place.local(),
                    prefix,
                };
                view.root_type = projection.result_type();
                view.projected_type = projection.result_type();
                anchors.check_object_identity(function, view, budget)?;
                for mutation in 0..4 {
                    let mut changed = view;
                    match mutation {
                        0 => {
                            changed.object = ScopedObjectIdentityV29::Reference {
                                instance,
                                site,
                                role,
                                dereference_prefix: 0,
                            }
                        }
                        1 => {
                            changed.object = ScopedObjectIdentityV29::Reference {
                                instance,
                                site,
                                role,
                                dereference_prefix: prefix + 1,
                            }
                        }
                        2 => {
                            changed.object = ScopedObjectIdentityV29::Reference {
                                instance: ProductionCallInstanceIdV1(usize::MAX),
                                site,
                                role,
                                dereference_prefix: prefix,
                            }
                        }
                        _ => changed.root_type = SemanticTypeIdV1::from_index(u32::MAX),
                    }
                    assert!(
                        anchors
                            .check_object_identity(function, changed, budget)
                            .is_err()
                    );
                }
                checked += 1;
            }
        }
    }
    assert!(
        checked > 0,
        "must exercise actual original dereference evaluations"
    );
    // This checks locators, not the legacy scalar-slot physical-use contract.
    OBSERVED.set(OBSERVED.get() + 1);
    Err(unsupported(0, None, None, STOP))
}

#[test]
fn reference_locators_name_original_evaluations_without_selecting_an_allocation() {
    // Locator validation is not physical alias/currentness admission.
    let (result, _, _) = run(
        false,
        ScopedFixture::RepeatedReferences,
        inspect_original_reference_locators,
        LIMIT,
        LIMIT,
    );
    assert_eq!(
        OBSERVED.get(),
        1,
        "all locator checks must finish before the stop"
    );
    assert!(is_stopped(&result), "{result:?}");
}
