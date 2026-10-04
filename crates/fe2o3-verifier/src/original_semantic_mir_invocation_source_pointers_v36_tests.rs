use super::*;
use fe2o3_kernel_ir::{ExplicitLaunchExtent, FormalIndexWidth};
use fe2o3_mir_model::semantic_mir_v1::*;

const LIMIT: usize = 100_000_000;

fn transform(types: &mut Vec<Type>, functions: &mut Vec<Function>) {
    let word = TypeId::from_index(0);
    let reference = TypeId::from_index(types.len() as u32);
    types.push(Type::new(
        SemanticTypeIdentityV1::from_sha256([221; 32]),
        SemanticLayoutIdentityV1::from_sha256([222; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            BackendRepr::scalar(BackendScalar::initialized(
                BackendPrimitive::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        Shape::Pointer(
            PointerType::new_with_kind(
                word,
                PointerKind::Reference,
                Mutability::Immutable,
                0,
                64,
                PointerMetadata::None,
            )
            .unwrap(),
        ),
    ));
    for root in 0..2 {
        let prior = &functions[root];
        let source = prior.source();
        let mut locals = prior.locals().to_vec();
        assert_eq!(locals.len(), 4);
        for ordinal in 4..7 {
            locals.push(SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([224 + 4 * root as u8 + ordinal; 32]),
                reference,
                SemanticLocalRoleV1::Temporary,
                source,
            ));
        }
        let place =
            |local, ty| Place::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
        let assign = |local, ty, value| {
            SemanticStatementV1::new(
                source,
                Statement::Assign(SemanticAssignmentV1::new(
                    place(local, ty),
                    SemanticRvalueV1::new(ty, value),
                )),
            )
        };
        let mut blocks = prior.blocks().to_vec();
        let mut statements = vec![
            assign(3, word, Rvalue::Use(Operand::Copy(place(1, word)))),
            assign(
                4,
                reference,
                Rvalue::Borrow {
                    kind: Borrow::Shared,
                    place: place(3, word),
                },
            ),
            assign(
                5,
                reference,
                Rvalue::Use(Operand::Copy(place(4, reference))),
            ),
            assign(
                6,
                reference,
                Rvalue::Use(Operand::Move(place(5, reference))),
            ),
            assign(
                2,
                word,
                Rvalue::Load(SemanticMemoryLoadV1::new(
                    Place::new(
                        SemanticLocalIdV1::from_index(6),
                        vec![SemanticProjectionV1::new(Projection::Dereference, word).unwrap()],
                        word,
                    )
                    .unwrap(),
                    Volatility::NonVolatile,
                    None,
                )),
            ),
        ];
        statements.extend_from_slice(blocks[0].statements());
        blocks[0] = SemanticBasicBlockV1::new(
            blocks[0].identity(),
            blocks[0].source(),
            statements,
            blocks[0].terminator().clone(),
        )
        .unwrap();
        functions[root] = Function::new(
            prior.identity(),
            prior.role(),
            prior.item_definition_identity(),
            prior.monomorphization_identity(),
            prior.generic_type_arguments_identity(),
            prior.const_generic_arguments_identity(),
            source,
            prior.abi().clone(),
            locals,
            prior.entry(),
            blocks,
        )
        .unwrap()
        .with_kernel_entry(prior.kernel_entry().unwrap().clone());
    }
}

fn run(
    work: usize,
    storage: usize,
    examine: impl FnOnce(&SourceByteBody<'_, '_, '_>, &mut Writer<'_, '_>) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    super::super::super::super::invocations::tests::run_source_transform(
        work,
        storage,
        transform,
        |plan, out| {
            let source = plan.source(out)?;
            let owner = source.canonical(out.budget)?;
            let (inventory, receipt) =
                fe2o3_kernel_analysis::CanonicalKirInventoryV18::derive_v18(owner, out.budget)?;
            out.budget.reserve_storage(receipt.retained_storage())?;
            let result = source.with_ranked_correspondence_v18(
                &inventory,
                out.budget,
                |relation, budget| {
                    let mut writer = Writer::new(budget)?;
                    let slots = SourceSlots::derive(plan, relation, &mut writer)?;
                    let body = SourceByteBody::derive(plan, &slots, 0, 0, &mut writer)?;
                    examine(&body, &mut writer)
                },
            );
            drop(inventory);
            if result.is_ok() {
                out.budget.release_storage(receipt.retained_storage())?;
            }
            result
        },
    )
}

#[test]
fn original_mir_pointer_events_consume_genuine_private_borrow_copy_move_and_load() {
    run(LIMIT, LIMIT, |body, out| {
        let local = body.locals.start;
        let super::super::Event::Pointer(Event::Borrow { destination, access, bits }) =
            body.event_at(0, 1, out)?
        else {
            panic!("exact original Borrow event");
        };
        assert_eq!(destination, local + 4);
        assert_eq!((access.bytes, access.alignment, bits), (4, 4, 32));
        let Address::Slot { descriptor, offset: 0 } = access.address else {
            panic!("Borrow must use the retained original private object");
        };
        assert_eq!(body.slots.legacy_descriptor_by_source(0, 0, 3, out)?.unwrap().0, descriptor);
        for (statement, destination, input, moved) in [(2, 5, 4, false), (3, 6, 5, true)] {
            let super::super::Event::Pointer(Event::Copy { destination: actual, operand, metadata_bits: 0 }) =
                body.event_at(0, statement, out)?
            else {
                panic!("exact original pointer carrier transfer");
            };
            assert_eq!(actual, local + destination);
            assert_eq!(operand.kind, OperandKind::Pointer { local: local + input, moved });
        }
        assert!(matches!(body.event_at(0, 4, out)?, super::super::Event::Transfer { value: Value::Read { access: Access { address: Address::Pointer { local: pointer, offset: 0 }, .. }, moved: false }, .. } if pointer == local + 6));
        body.emit(out)?;
        assert!(out.text.contains("InvocationSourceByteEventV36::Pointer"));
        assert!(out.text.contains("InvocationSourcePointerEventV36::Borrow"));
        assert!(out.text.contains("moved: true"));
        Ok(())
    }).0.unwrap();
}

#[test]
fn original_mir_pointer_extractor_refuses_changed_types_mutability_and_stored_carriers() {
    for mutation in 0..4 {
        run(LIMIT, LIMIT, |body, out| {
            // These are adversarial extractor inputs, not admitted source rows.
            let context = body.context(out)?;
            let Statement::Assign(prior) = context.function.blocks()[0].statements()[1].kind()
            else {
                panic!("original borrow assignment");
            };
            let word = TypeId::from_index(0);
            let reference = prior.destination().ty();
            let place =
                |local, ty| Place::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
            let (destination, ty, value) = match mutation {
                0 => (
                    prior.destination().clone(),
                    word,
                    Rvalue::Use(Operand::Copy(place(1, word))),
                ),
                1 => (
                    prior.destination().clone(),
                    reference,
                    Rvalue::Borrow {
                        kind: Borrow::Mutable,
                        place: place(3, word),
                    },
                ),
                2 => (
                    place(3, word),
                    reference,
                    Rvalue::Use(Operand::Copy(place(4, reference))),
                ),
                _ => (
                    prior.destination().clone(),
                    reference,
                    Rvalue::Use(Operand::Copy(
                        Place::new(
                            SemanticLocalIdV1::from_index(4),
                            vec![
                                SemanticProjectionV1::new(Projection::Dereference, reference)
                                    .unwrap(),
                            ],
                            reference,
                        )
                        .unwrap(),
                    )),
                ),
            };
            let statement = Statement::Assign(SemanticAssignmentV1::new(
                destination,
                SemanticRvalueV1::new(ty, value),
            ));
            assert!(derive(&context, &statement, out).is_err());
            assert!(out.text.is_empty());
            Ok(())
        })
        .0
        .unwrap();
    }
}

#[test]
fn original_mir_pointer_semantics_keep_formation_and_move_checks_at_the_event() {
    let borrow = SOURCE_POINTERS_V36
        .split("spec fn invocation_source_borrow_enabled_v36(")
        .nth(1)
        .unwrap()
        .split("spec fn invocation_source_pointer_step_v36(")
        .next()
        .unwrap();
    assert!(borrow.contains("invocation_source_read_enabled_v36"));
    assert!(borrow.contains("byte_pointer_type_v30(pointer, 2, 8)"));
    assert!(borrow.contains("invocation_source_byte_value_typed_v36"));
    let copy = SOURCE_POINTERS_V36
        .split("InvocationSourcePointerEventV36::Copy { destination, operand, metadata_bits } =>")
        .nth(1)
        .unwrap()
        .split("InvocationSourcePointerEventV36::Borrow")
        .next()
        .unwrap();
    assert!(
        copy.find("invocation_source_operand_evaluate_v36").unwrap()
            < copy.find("invocation_source_byte_put_local_v36").unwrap()
    );
    assert!(SOURCE_POINTERS_V36.contains("0 <= index < slice.length"));
    assert!(SOURCE_POINTERS_V36.contains("forall|index: int| 0 <= index < slice.length"));
    let (slice_call, index_borrow) = SOURCE_POINTERS_V36
        .split_once("InvocationSourcePointerEventV36::SliceBorrow { destination, local, metadata_bits, width, alignment, bits } =>")
        .unwrap()
        .1
        .split_once("InvocationSourcePointerEventV36::IndexBorrow { destination, local, index, index_bits, metadata_bits, width, alignment, bits } =>")
        .unwrap();
    assert_eq!(
        slice_call,
        "\n            invocation_source_slice_borrow_step_v77(source, destination, local,\n                metadata_bits, width, alignment, bits, little_endian),\n        "
    );
    // Struct update retains both allocation identity and the enclosing view.
    for branch in [
        slice_borrow_body_v77(),
        index_borrow.split("\nproof fn ").next().unwrap(),
    ] {
        assert_eq!(branch.matches("..slice.pointer").count(), 1);
        assert!(
            branch.contains(
                "byte_offset: slice.pointer.byte_offset + index * width, ..slice.pointer"
            )
        );
        assert!(!branch.contains("allocation:"));
        assert!(!branch.contains("view:"));
        assert!(branch.contains("invocation_source_borrow_enabled_v36"));
    }
    assert!(!SOURCE_POINTERS_V36.contains("byte_store_v30"));
    assert!(!SOURCE_POINTERS_V36.contains("byte_pop_frame_v30"));
    assert!(!SOURCE_POINTERS_V36.contains("byte_allocate_v30"));
    assert!(!SOURCE_POINTERS_V36.contains("target:"));
}

fn slice_borrow_body_v77() -> &'static str {
    SOURCE_POINTERS_V36
        .split_once("spec fn invocation_source_slice_borrow_step_v77(")
        .unwrap()
        .1
        .split_once("spec fn invocation_source_pointer_step_v36(")
        .unwrap()
        .0
        .split_once(") -> InvocationSourceByteStateV36 {\n")
        .unwrap()
        .1
        .strip_suffix("}\n\n")
        .unwrap()
}

#[test]
fn original_slice_borrow_trigger_preserves_exact_formation_and_refusal() {
    let mut slice = String::from(" {\n");
    for line in slice_borrow_body_v77().lines() {
        slice.push_str("        ");
        slice.push_str(line);
        slice.push('\n');
    }
    slice.push_str("        }\n        ");
    assert_eq!(slice.matches("#[trigger] ").count(), 1);
    assert_eq!(
        slice.replace("#[trigger] ", ""),
        r#" {
            if 0 <= local < source.machine.values.len() && width > 0
                && invocation_source_pointer_carrier_v36(source.machine.values[local], metadata_bits) {
                match source.machine.values[local] {
                    MemoryValueV30::Slice(slice) => {
                        if byte_range_aligned_v30(source.machine.memory, slice.pointer, slice.length * width, alignment)
                            && slice.pointer.byte_offset + slice.length * width < memory_value_modulus_v30(8)
                            && forall|index: int| 0 <= index < slice.length ==>
                                invocation_source_borrow_enabled_v36(source,
                                    MemoryPointerV30 { byte_offset: slice.pointer.byte_offset + index * width, ..slice.pointer },
                                    width, alignment, bits, little_endian) {
                            invocation_source_byte_put_local_v36(source, destination, MemoryValueV30::Slice(slice))
                        } else { invocation_source_byte_refused_v36(source) }
                    }
                    _ => invocation_source_byte_refused_v36(source),
                }
            } else { invocation_source_byte_refused_v36(source) }
        }
        "#
    );
}

#[test]
fn original_slice_borrow_refusal_and_carrier_laws_are_complete_model_obligations() {
    for name in [
        "invocation_source_slice_borrow_invalid_element_refuses_v74",
        "invocation_source_slice_borrow_valid_elements_preserve_carrier_v74",
    ] {
        let signature = format!("proof fn {name}(");
        assert_eq!(SOURCE_POINTERS_V36.matches(&signature).count(), 1);
        let law = SOURCE_POINTERS_V36
            .split_once(&signature)
            .unwrap()
            .1
            .split_once("\n{")
            .unwrap()
            .0;
        assert!(law.contains("source.machine.values[local] == MemoryValueV30::Slice(slice)"));
        assert!(law.contains("0 <= index < slice.length"));
        assert!(law.contains("slice.pointer.byte_offset + index * width, ..slice.pointer"));
        assert!(law.contains("width, alignment, bits, little_endian)"));
        assert!(law.contains("ensures"));
        assert!(law.contains("invocation_source_pointer_step_v36(source,"));
        for forbidden in ["assume", "admit", "external_body"] {
            assert!(!law.contains(forbidden));
        }
    }
    let generated = super::super::super::super::invocations::tests::run_variant(
        LIMIT,
        LIMIT,
        true,
        |plan, out| {
            super::super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                let relation = slots.correspondence(out)?;
                let launches = [ExplicitLaunchExtent::Exact {
                    rank: 1,
                    extents: [64, 1, 1],
                }; 2];
                super::super::super::generate_refinement_v36(
                    relation,
                    &launches,
                    FormalIndexWidth::Bits64,
                    fe2o3_kernel_ir::EndiannessV2::Little,
                    out,
                )?;
                assert!(out.text.contains(SOURCE_POINTERS_V36));
                Ok(())
            })
        },
    );
    generated.0.unwrap();
    assert_eq!(generated.2, 37);
}

#[test]
fn original_copy_window_triggers_preserve_exact_padding_and_relocation_domains() {
    let runtime = include_str!("original_semantic_mir_observed_effects_v39.vrs");
    let window = runtime
        .split_once("spec fn invocation_copy_windows_related_v39(")
        .unwrap()
        .1
        .split_once("\nspec fn invocation_observed_effect_related_v39(")
        .unwrap()
        .0;
    let hints = [
        "#![trigger source.live[original.allocation].initialized[original.byte_offset + i]] ",
        "#![trigger source.live[original.allocation].relocations.contains_key(original.byte_offset + i)] ",
    ];
    let mut without_hints = window.to_owned();
    for hint in hints {
        assert_eq!(window.matches(hint).count(), 1);
        without_hints = without_hints.replace(hint, "");
    }
    assert_eq!(
        without_hints,
        r#"
    source: ByteMemoryV30, target: ByteMemoryV30,
    original: MemoryPointerV30, actual: MemoryPointerV30, width: int,
    map: InvocationByteMapV36,
) -> bool {
    byte_range_live_v30(source, original, width) && byte_range_live_v30(target, actual, width)
    && (forall|i: int| 0 <= i < width ==>
        source.live[original.allocation].initialized[original.byte_offset + i]
            == target.live[actual.allocation].initialized[actual.byte_offset + i]
        && (source.live[original.allocation].initialized[original.byte_offset + i] ==>
            invocation_byte_token_related_v37(source.live[original.allocation].bytes[original.byte_offset + i],
                target.live[actual.allocation].bytes[actual.byte_offset + i], map, source, target)))
    && (forall|i: int| 0 <= i < width ==> {
        let source_cells = source.live[original.allocation].relocations;
        let target_cells = target.live[actual.allocation].relocations;
        let source_complete = source_cells.contains_key(original.byte_offset + i)
            && i + source_cells[original.byte_offset + i].width <= width;
        let target_complete = target_cells.contains_key(actual.byte_offset + i)
            && i + target_cells[actual.byte_offset + i].width <= width;
        source_complete == target_complete && (source_complete ==>
            invocation_relocation_related_v37(source_cells[original.byte_offset + i],
                target_cells[actual.byte_offset + i], map, source, target))
    })
}
"#
    );
}

#[test]
fn original_mir_pointer_probe_and_header_oracles_are_independent() {
    run(LIMIT, LIMIT, |body, out| {
        let context = body.context(out)?;
        let before = (out.budget.work(), out.budget.storage());
        assert_eq!(derive(&context, &Statement::Nop, out)?, None);
        assert_eq!(out.budget.work() - before.0, 2);
        assert_eq!(out.budget.storage(), before.1);
        assert_eq!(
            headers(),
            size_of::<Event>()
                + 2 * size_of::<Result<Event>>()
                + size_of::<Option<Event>>()
                + 2 * size_of::<Result<Option<Event>>>()
                + size_of::<&PointerType>()
                + 2 * size_of::<Result<&PointerType>>()
                + size_of::<TypedOperand>()
                + 2 * size_of::<Result<TypedOperand>>()
                + size_of::<(usize, TypeId, u32)>()
                + 2 * size_of::<Result<(usize, TypeId, u32)>>()
                + size_of::<(u64, u64, u32)>()
                + 2 * size_of::<Result<(u64, u64, u32)>>()
                + size_of::<Option<(u64, u64, bool)>>()
                + 2 * size_of::<Result<Option<(u64, u64, bool)>>>()
                + 18 * size_of::<usize>()
                + 10 * size_of::<&()>()
        );
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn original_mir_pointer_body_has_exact_and_one_short_cumulative_resources() {
    let inspect = |body: &SourceByteBody<'_, '_, '_>, out: &mut Writer<'_, '_>| body.emit(out);
    let measured = run(LIMIT, LIMIT, inspect);
    measured.0.unwrap();
    run(measured.1, measured.3, inspect).0.unwrap();
    assert!(run(measured.1 - 1, measured.3, inspect).0.is_err());
    assert!(run(measured.1, measured.3 - 1, inspect).0.is_err());
}
