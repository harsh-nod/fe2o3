use super::super::super::super::{byte_bindings, paired, source_function};
use super::*;

#[derive(Clone, Copy, Debug)]
enum Case {
    Shared,
    Mutable,
    Reborrow,
}

fn original_borrow(
    types: &mut Vec<SemanticTypeDeclV1>,
    functions: &mut Vec<SemanticFunctionDeclV1>,
    case: Case,
) {
    super::retained_transform_v44(types, functions, SemanticCheckedBinaryOpV1::Add);
    let old = functions.last_mut().unwrap();
    let source = old.source();
    let pair = old.locals()[4].ty();
    let Shape::Tuple(fields) = types[pair.index() as usize].shape() else {
        panic!("original tuple")
    };
    let word = fields.fields()[0];
    let boolean = fields.fields()[1];
    let mutable = matches!(case, Case::Mutable);
    let reference = TypeId::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([251; 32]),
        SemanticLayoutIdentityV1::from_sha256([251; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        Shape::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                pair,
                SemanticPointerKindV1::Reference,
                if mutable {
                    SemanticMutabilityV1::Mutable
                } else {
                    SemanticMutabilityV1::Immutable
                },
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    ));
    let mut locals = old.locals().to_vec();
    assert_eq!(locals.len(), 6);
    for ordinal in [251u8, 252] {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([ordinal; 32]),
            reference,
            SemanticLocalRoleV1::Temporary,
            source,
        ));
    }
    let local =
        |index, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(index), vec![], ty).unwrap();
    let deref = |index| {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(index),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, pair).unwrap()],
            pair,
        )
        .unwrap()
    };
    let assign = |destination, value| {
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                destination,
                SemanticRvalueV1::new(reference, value),
            )),
        )
    };
    let mut blocks = old.blocks().to_vec();
    let mut entry: Vec<_> = [4, 5, 6, 7]
        .into_iter()
        .map(|index| {
            SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(index)),
            )
        })
        .collect();
    entry.extend_from_slice(blocks[0].statements());
    entry.push(assign(
        local(6, reference),
        SemanticRvalueKindV1::Borrow {
            kind: if mutable {
                SemanticBorrowKindV1::Mutable
            } else {
                SemanticBorrowKindV1::Shared
            },
            place: local(4, pair),
        },
    ));
    if matches!(case, Case::Reborrow) {
        entry.push(assign(
            local(7, reference),
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: deref(6),
            },
        ));
    }
    blocks[0] = SemanticBasicBlockV1::new(
        blocks[0].identity(),
        source,
        entry,
        blocks[0].terminator().clone(),
    )
    .unwrap();
    let reader = if matches!(case, Case::Reborrow) { 7 } else { 6 };
    let SemanticTerminatorKindV1::Assert {
        expected,
        message,
        target,
        unwind,
        ..
    } = blocks[1].terminator().kind()
    else {
        panic!("original overflow guard")
    };
    let condition = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(reader),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, pair).unwrap(),
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), boolean).unwrap(),
        ],
        boolean,
    )
    .unwrap();
    blocks[1] = SemanticBasicBlockV1::new(
        blocks[1].identity(),
        source,
        blocks[1].statements().to_vec(),
        SemanticTerminatorV1::new(
            source,
            SemanticTerminatorKindV1::Assert {
                condition: SemanticOperandV1::Copy(condition),
                expected: *expected,
                message: message.clone(),
                target: *target,
                unwind: *unwind,
            },
        ),
    )
    .unwrap();
    let read = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(reader),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, pair).unwrap(),
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), word).unwrap(),
        ],
        word,
    )
    .unwrap();
    let mut finish = vec![SemanticStatementV1::new(
        source,
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            local(0, word),
            SemanticRvalueV1::new(
                word,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(read)),
            ),
        )),
    )];
    for index in [7, 6, 5, 4] {
        finish.push(SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(index)),
        ));
    }
    blocks[2] = SemanticBasicBlockV1::new(
        blocks[2].identity(),
        source,
        finish,
        blocks[2].terminator().clone(),
    )
    .unwrap();
    *old = SemanticFunctionDeclV1::new(
        old.identity(),
        old.role(),
        old.item_definition_identity(),
        old.monomorphization_identity(),
        old.generic_type_arguments_identity(),
        old.const_generic_arguments_identity(),
        source,
        old.abi().clone(),
        locals,
        old.entry(),
        blocks,
    )
    .unwrap();
}

fn program(case: Case, work: usize, storage: usize) -> (Result<()>, usize, usize, usize) {
    super::super::super::super::super::invocations::tests::run_source_transform(
        work,
        storage,
        |types, functions| original_borrow(types, functions, case),
        |plan, out| {
            source_function::tests::with_slots(plan, out, |slots, out| {
                let original = plan.source(out)?.source_semantic(out.budget)?;
                let pair = original.functions().last().unwrap().locals()[4].ty();
                assert_eq!(
                    slots.original_memory_layout_v51(pair, out)?,
                    Some((8, 4, true))
                );
                assert_eq!(slots.aggregate_leaf_count(pair, out)?, None);
                for root in 0..2 {
                    for instance in 1..=2 {
                        assert!(slots.has_original_object(root, instance, 4, out)?);
                    }
                }
                let mut source = source_function::SourceByteProgram::derive(plan, slots, out)?;
                let bindings = byte_bindings::SourceByteBindings::derive(slots, out)?;
                let paired = paired::PairedInvocations::derive(
                    plan,
                    &source,
                    fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                    out,
                )?;
                slots.emit(out)?;
                source.emit(out)?;
                bindings.emit(out)?;
                paired.emit(out)?;
                assert_eq!(
                    out.text
                        .matches("InvocationSourcePointerEventV36::TypedBorrow {")
                        .count(),
                    if matches!(case, Case::Reborrow) { 8 } else { 4 }
                );
                assert!(
                    out.text
                        .contains("InvocationSourceByteBaseV36::ObjectLocal(")
                );
                assert!(
                    out.text
                        .contains("InvocationSourceByteBaseV36::PointerLocal(")
                );
                assert!(!out.text.contains("assume("));
                Ok(())
            })
        },
    )
}

#[test]
fn original_addressable_structural_borrows_and_reborrows_reach_complete_paired_consumer() {
    for case in [Case::Shared, Case::Mutable, Case::Reborrow] {
        program(case, LIMIT, LIMIT)
            .0
            .unwrap_or_else(|error| panic!("{case:?}: {error:?}"));
    }
}

#[test]
fn original_addressable_structural_borrow_consumer_has_exact_resource_boundaries() {
    let measured = program(Case::Reborrow, LIMIT, LIMIT);
    measured.0.unwrap();
    let exact = program(Case::Reborrow, measured.1, measured.3);
    exact.0.unwrap();
    assert_eq!(
        (exact.1, exact.2, exact.3),
        (measured.1, measured.2, measured.3)
    );
    for (work, storage, is_work) in [
        (measured.1 - 1, measured.3, true),
        (measured.1, measured.3 - 1, false),
    ] {
        let denied = program(Case::Reborrow, work, storage).0;
        let mut error: &(dyn std::error::Error + 'static) = denied.as_ref().unwrap_err();
        loop {
            if let Some(resource) = error.downcast_ref::<Resource>() {
                match resource {
                    Resource::Work(bound) if is_work => {
                        assert_eq!((bound.actual(), bound.limit()), (measured.1, work))
                    }
                    Resource::Storage(bound) if !is_work => {
                        assert_eq!((bound.actual(), bound.limit()), (measured.3, storage))
                    }
                    other => panic!("exact structural borrow resource refusal: {other:?}"),
                }
                break;
            }
            error = error
                .source()
                .unwrap_or_else(|| panic!("missing resource: {denied:?}"));
        }
    }
}

#[test]
fn original_typed_memory_schema_queries_retain_owner_and_ledger_custody() {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
    for foreign in [false, true] {
        let reached = std::cell::Cell::new(false);
        let result = super::super::super::super::super::invocations::tests::run_source_transform(
            LIMIT,
            LIMIT,
            |types, functions| original_borrow(types, functions, Case::Shared),
            |plan, out| {
                source_function::tests::with_slots(plan, out, |slots, out| {
                    let source = plan.source(out)?.source_semantic(out.budget)?;
                    let pair = source.functions().last().unwrap().locals()[4].ty();
                    assert_eq!(
                        slots.original_memory_layout_v51(pair, out)?,
                        Some((8, 4, true))
                    );
                    if foreign {
                        let mut work = Work::new(LIMIT);
                        let mut budget = Budget::new(&mut work, LIMIT);
                        budget.reserve_storage(out.budget.storage())?;
                        let before = (budget.work(), budget.storage(), budget.peak_storage());
                        let mut writer = Writer::new(&mut budget)?;
                        assert!(matches!(
                            slots.original_memory_layout_v51(pair, &mut writer),
                            Err(Error::Source(SourceError::Resource(Resource::Accounting)))
                        ));
                        assert!(writer.text.is_empty());
                        assert_eq!(
                            (
                                writer.budget.work(),
                                writer.budget.storage(),
                                writer.budget.peak_storage()
                            ),
                            before
                        );
                    } else {
                        let released = out.budget.storage() - slots.required + 1;
                        out.budget.release_storage(released)?;
                        assert!(matches!(
                            slots.original_memory_layout_v51(pair, out),
                            Err(Error::Source(SourceError::Resource(Resource::Accounting)))
                        ));
                        out.budget.reserve_storage(released)?;
                    }
                    let before = (
                        out.budget.work(),
                        out.budget.storage(),
                        out.budget.peak_storage(),
                        out.text.len(),
                    );
                    let retry = slots.original_memory_layout_v51(pair, out);
                    assert!(matches!(
                        retry,
                        Err(Error::Source(SourceError::Resource(Resource::Accounting)))
                    ));
                    assert_eq!(
                        (
                            out.budget.work(),
                            out.budget.storage(),
                            out.budget.peak_storage(),
                            out.text.len()
                        ),
                        before
                    );
                    reached.set(true);
                    retry.map(|_| ())
                })
            },
        );
        assert!(
            reached.get(),
            "custody attack did not reach its exact query: {:?}",
            result.0
        );
        assert!(matches!(
            result.0,
            Err(Error::Source(SourceError::Resource(Resource::Accounting)))
        ));
    }
}

#[test]
fn typed_borrow_runtime_validates_fields_without_reading_padding_or_creating_backing() {
    let runtime = include_str!("original_semantic_mir_source_memory_values_v51.vrs");
    assert!(runtime.contains("invocation_source_memory_value_valid_v51(source,"));
    assert!(
        runtime.contains(
            "byte_pointer_load_valid_v37(source.machine.memory, pointer, 8, little_endian)"
        )
    );
    assert!(runtime.contains("fields[i].source_type, (fuel - 1) as nat"));
    assert!(runtime.contains(
        "byte_range_aligned_v30(source.machine.memory, pointer, layout.width, layout.alignment)"
    ));
    assert!(!runtime.contains("byte_allocate_v30"));
    assert!(!runtime.contains("byte_store_v30"));
    let structural = runtime
        .split("InvocationSourceMemoryKindV51::Fields(fields) =>")
        .nth(1)
        .unwrap()
        .split("InvocationSourceMemoryKindV51::Array")
        .next()
        .unwrap();
    assert!(!structural.contains("byte_load_v30"));
    assert!(!structural.contains("byte_range_initialized_v30"));
    let laws = include_str!("original_semantic_mir_source_memory_laws_v51.vrs");
    for name in [
        "memory_missing_shape_refuses",
        "typed_borrow_requires_live_current_referent",
        "typed_borrow_keeps_memory",
    ] {
        assert!(laws.contains(name));
        assert!(super::super::super::super::source_bytes::SOURCE_BYTES_V36.contains(name));
    }
}

#[test]
fn typed_array_trigger_preserves_stride_bounds_and_recursive_validity() {
    let runtime = include_str!("original_semantic_mir_source_memory_values_v51.vrs");
    let array = runtime
        .split_once("InvocationSourceMemoryKindV51::Array { element, count, stride } =>")
        .unwrap()
        .1
        .split_once("\n                }")
        .unwrap()
        .0
        .trim();
    assert_eq!(runtime.matches("#[trigger] ").count(), 1);
    let original = r#"0 <= count && 0 <= stride && layout.width == count * stride
                            && forall|i: int| 0 <= i < count ==>
                                invocation_source_memory_value_valid_v51(source,
                                    MemoryPointerV30 { byte_offset: pointer.byte_offset + i * stride, ..pointer },
                                    element, (fuel - 1) as nat, little_endian),"#;
    assert_eq!(array.replace("#[trigger] ", ""), original);
}

#[test]
fn typed_array_invalid_element_law_is_in_complete_source_model() {
    let laws = include_str!("original_semantic_mir_source_memory_laws_v51.vrs");
    let name = "proof fn invocation_source_typed_memory_invalid_array_element_refuses_v71(";
    let law = laws.split_once(name).unwrap().1;
    assert!(law.contains("0 <= index < count, fuel > 0,"));
    assert!(law.contains("pointer.byte_offset + index * stride"));
    assert!(law.contains("element, (fuel - 1) as nat, little_endian)"));
    assert!(law.contains(
        "ensures !invocation_source_memory_value_valid_v51(source, pointer, ty, fuel, little_endian),"
    ));
    assert_eq!(
        law.split_once("\n{\n").unwrap().1.trim_end(),
        concat!(
            "    reveal_with_fuel(invocation_source_memory_value_valid_v51, 1);\n",
            "    if invocation_source_memory_value_valid_v51(source, pointer, ty, fuel, little_endian) {\n",
            "        assert forall|element_index: int| 0 <= element_index < count implies\n",
            "            invocation_source_memory_value_valid_v51(source,\n",
            "                MemoryPointerV30 { byte_offset: pointer.byte_offset + element_index * stride, ..pointer },\n",
            "                element, (fuel - 1) as nat, little_endian) by {\n",
            "        }\n",
            "        assert(invocation_source_memory_value_valid_v51(source,\n",
            "            MemoryPointerV30 { byte_offset: pointer.byte_offset + index * stride, ..pointer },\n",
            "            element, (fuel - 1) as nat, little_endian));\n",
            "    }\n",
            "}",
        ),
    );
    assert!(!law.contains("admit"));
    assert!(!law.contains("assume"));
    assert!(!law.contains("external_body"));
    let complete = super::super::super::super::source_bytes::SOURCE_BYTES_V36;
    assert_eq!(complete.matches(name).count(), 1);
    assert!(complete.contains(laws));
}

#[test]
fn typed_borrow_finite_reference_cases_require_defined_fields_and_current_storage_not_padding() {
    // This executable bounded reference matrix is distinct from the generated
    // source proof obligations; it is not execution of the Verus model.
    let valid =
        |initialized: u8, boolean: u8, live: bool, generation: u8, pointer_generation: u8| {
            live && generation == pointer_generation && initialized & 0x1f == 0x1f && boolean <= 1
        };
    for initialized in 0..=255u8 {
        for boolean in [0, 1, 2, 255] {
            let actual = valid(initialized, boolean, true, 2, 2);
            assert_eq!(
                actual,
                (0..5).all(|field_byte| initialized & (1 << field_byte) != 0) && boolean <= 1
            );
            assert_eq!(actual, valid(initialized ^ 0xe0, boolean, true, 2, 2));
            assert!(!valid(initialized, boolean, false, 2, 2));
            assert!(!valid(initialized, boolean, true, 3, 2));
            for moved_byte in 0..5 {
                assert!(!valid(
                    initialized & !(1 << moved_byte),
                    boolean,
                    true,
                    2,
                    2
                ));
            }
        }
    }
    assert!(!valid(0, 0, true, 3, 3));
    assert!(valid(0x1f, 1, true, 3, 3));
}
