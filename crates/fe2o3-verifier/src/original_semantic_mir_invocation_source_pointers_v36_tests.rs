use super::*;
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
    let (slice_borrow, index_borrow) = SOURCE_POINTERS_V36
        .split_once("InvocationSourcePointerEventV36::SliceBorrow { destination, local, metadata_bits, width, alignment, bits } =>")
        .unwrap()
        .1
        .split_once("InvocationSourcePointerEventV36::IndexBorrow { destination, local, index, index_bits, metadata_bits, width, alignment, bits } =>")
        .unwrap();
    // Struct update retains both allocation identity and the enclosing view.
    for branch in [slice_borrow, index_borrow] {
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
