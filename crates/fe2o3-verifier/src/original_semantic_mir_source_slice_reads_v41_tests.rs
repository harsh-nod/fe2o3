use super::super::super::slice_reads::{self, Slice};
use super::super::super::{Access, Address, Destination, Event as ByteEvent, Value};
use super::*;

fn slice_transform(types: &mut Vec<Type>, functions: &mut Vec<Function>, mutable: bool) {
    super::transform(types, functions, mutable);
    let boolean = TypeId::from_index(types.len() as u32);
    types.push(Type::new(
        SemanticTypeIdentityV1::from_sha256([220; 32]),
        SemanticLayoutIdentityV1::from_sha256([221; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(1),
            1,
            BackendRepr::scalar(BackendScalar::initialized(
                BackendPrimitive::integer(false, 8, 1),
                SemanticScalarValidityRangeV1::new(0, 1),
            )),
            false,
        )
        .unwrap(),
        Shape::Scalar(SemanticScalarTypeV1::Bool),
    ));
    for root in 0..2 {
        let prior = &functions[root];
        let source = prior.source();
        let word = prior.locals()[1].ty();
        let reference = prior.locals()[4].ty();
        let length = prior.locals()[5].ty();
        let Shape::Pointer(pointer) = types[reference.index() as usize].shape() else {
            unreachable!()
        };
        let indexed = Place::new(
            SemanticLocalIdV1::from_index(4),
            vec![
                SemanticProjectionV1::new(Projection::Dereference, pointer.pointee()).unwrap(),
                SemanticProjectionV1::new(
                    Projection::Index(SemanticLocalIdV1::from_index(6)),
                    word,
                )
                .unwrap(),
            ],
            word,
        )
        .unwrap();
        let destination = Place::new(SemanticLocalIdV1::from_index(3), vec![], word).unwrap();
        let mut statements = [
            Rvalue::Use(Operand::Copy(indexed.clone())),
            Rvalue::Load(SemanticMemoryLoadV1::new(
                indexed,
                Volatility::NonVolatile,
                None,
            )),
        ]
        .into_iter()
        .map(|value| {
            SemanticStatementV1::new(
                source,
                Statement::Assign(SemanticAssignmentV1::new(
                    destination.clone(),
                    SemanticRvalueV1::new(word, value),
                )),
            )
        })
        .collect::<Vec<_>>();
        let mut blocks = prior.blocks().to_vec();
        // The original guard's success edge must dominate both source reads.
        // Preserve all existing call targets and append only the guarded block.
        statements.extend_from_slice(&blocks[0].statements()[3..]);
        let read_block = blocks.len() as u32;
        assert_eq!(read_block, 4);
        blocks.push(
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([226 + root as u8; 32]),
                source,
                statements,
                blocks[0].terminator().clone(),
            )
            .unwrap(),
        );
        let place =
            |local, ty| Place::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
        let guard = vec![
            blocks[0].statements()[0].clone(),
            SemanticStatementV1::new(
                source,
                Statement::Assign(SemanticAssignmentV1::new(
                    place(7, boolean),
                    SemanticRvalueV1::new(
                        boolean,
                        Rvalue::Binary {
                            operation: SemanticBinaryOpV1::LessThan,
                            left: Operand::Copy(place(6, length)),
                            right: Operand::Copy(place(5, length)),
                        },
                    ),
                )),
            ),
        ];
        blocks[0] = SemanticBasicBlockV1::new(
            blocks[0].identity(),
            blocks[0].source(),
            guard,
            SemanticTerminatorV1::new(
                source,
                SemanticTerminatorKindV1::Assert {
                    condition: Operand::Move(place(7, boolean)),
                    expected: true,
                    message: SemanticAssertMessageV1::BoundsCheck {
                        length: Operand::Copy(place(5, length)),
                        index: Operand::Copy(place(6, length)),
                    },
                    target: SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::AssertSuccess,
                        SemanticBlockIdV1::from_index(read_block),
                    ),
                    unwind: SemanticUnwindActionV1::Unreachable,
                },
            ),
        )
        .unwrap();
        let mut locals = prior.locals().to_vec();
        assert_eq!(locals.len(), 6);
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([222 + root as u8 * 2; 32]),
            length,
            SemanticLocalRoleV1::Argument(3),
            source,
        ));
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([223 + root as u8 * 2; 32]),
            boolean,
            SemanticLocalRoleV1::Temporary,
            source,
        ));
        let mut arguments: Vec<_> = prior
            .abi()
            .arguments()
            .iter()
            .map(|row| row.value().clone())
            .collect();
        arguments.push(SemanticAbiValueV1::new(
            length,
            SemanticAbiPassModeV1::Direct(
                SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                    SemanticAbiExtensionV1::None,
                    0,
                    None,
                )
                .unwrap(),
            ),
        ));
        let abi = SemanticFunctionAbiV1::new(
            prior.abi().identity(),
            prior.abi().layout_identity(),
            SemanticCanonAbiV1::GpuKernel,
            false,
            false,
            arguments,
            prior.abi().return_value().clone(),
        )
        .unwrap()
        .with_source_argument_ownership(vec![
            SemanticSourceArgumentOwnershipV1::ByValue,
            SemanticSourceArgumentOwnershipV1::ByValue,
            if mutable {
                SemanticSourceArgumentOwnershipV1::UniqueBorrow
            } else {
                SemanticSourceArgumentOwnershipV1::SharedBorrow
            },
            SemanticSourceArgumentOwnershipV1::ByValue,
        ])
        .unwrap();
        functions[root] = Function::new(
            prior.identity(),
            prior.role(),
            prior.item_definition_identity(),
            prior.monomorphization_identity(),
            prior.generic_type_arguments_identity(),
            prior.const_generic_arguments_identity(),
            source,
            abi,
            locals,
            prior.entry(),
            blocks,
        )
        .unwrap()
        .with_kernel_entry(prior.kernel_entry().unwrap().clone());
    }
}

fn run_slices(
    mutable: bool,
    work: usize,
    storage: usize,
    examine: impl Fn(&SourceByteBody<'_, '_, '_>, &mut Writer<'_, '_>) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    super::super::super::super::super::invocations::tests::run_source_transform(
        work,
        storage,
        |types, functions| slice_transform(types, functions, mutable),
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
                    for root in 0..2 {
                        let body = SourceByteBody::derive(plan, &slots, root, 0, &mut writer)?;
                        examine(&body, &mut writer)?;
                    }
                    Ok(())
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
fn original_mir_slice_reads_admit_genuine_copy_and_load_on_both_roots() {
    for mutable in [false, true] {
        run_slices(mutable, LIMIT, LIMIT, |body, out| {
            let first = body.locals.start;
            for statement in 0..2 {
                assert_eq!(
                    body.event_at(4, statement, out)?,
                    ByteEvent::Transfer {
                        destination: Destination::Local(first + 3),
                        value: Value::Read {
                            access: Access {
                                address: Address::Slice {
                                    source: Slice {
                                        local: first + 4,
                                        index: first + 6,
                                        index_bits: 64,
                                        metadata_bits: 64,
                                        stride: 4,
                                        alignment: 4,
                                    },
                                    offset: 0
                                },
                                ty: TypeId::from_index(0),
                                bytes: 4,
                                alignment: 4,
                            },
                            moved: false
                        },
                        scalar: ScalarV30::Integer {
                            signed: false,
                            width: 32
                        },
                    }
                );
            }
            body.emit(out)?;
            assert!(
                out.text
                    .contains("InvocationSourceByteBaseV36::SliceElement(")
            );
            Ok(())
        })
        .0
        .unwrap();
    }
}

#[test]
fn original_mir_slice_reads_preserve_write_move_and_projection_refusals() {
    run_slices(false, LIMIT, LIMIT, |body, out| {
        let context = body.context(out)?;
        let Statement::Assign(assignment) = context.function.blocks()[4].statements()[0].kind()
        else {
            unreachable!()
        };
        let Rvalue::Use(Operand::Copy(place)) = assignment.value().kind() else {
            unreachable!()
        };
        assert!(context.destination(place, out).is_err());
        assert!(context.value(&Operand::Move(place.clone()), out).is_err());
        for index in [
            Projection::Index(SemanticLocalIdV1::from_index(4)),
            Projection::ConstantIndex {
                offset: 0,
                minimum_length: 1,
                from_end: false,
            },
        ] {
            let mut projections = place.projections().to_vec();
            projections[1] = SemanticProjectionV1::new(index, place.ty()).unwrap();
            let forged = Place::new(place.local(), projections, place.ty()).unwrap();
            assert!(slice_reads::derive(&context, &forged, out).is_err());
        }
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn original_mir_slice_reads_have_exact_and_one_short_complete_resources() {
    let run = |work, storage| run_slices(true, work, storage, |body, out| body.emit(out));
    let measured = run(LIMIT, LIMIT);
    measured.0.unwrap();
    let (work, storage) = (measured.1, measured.3);
    let exact = run(work, storage);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.3), (work, storage));
    for (w, s, is_work) in [(work - 1, storage, true), (work, storage - 1, false)] {
        let failure = run(w, s);
        assert!(
            matches!((is_work, &failure.0),
            (true, Err(Error::Source(SourceError::Resource(Resource::Work(error)))))
                if error.limit() == w && error.actual() == work)
                || matches!((is_work, &failure.0),
                (false, Err(Error::Source(SourceError::Resource(Resource::Storage(error)))))
                    if error.limit() == s && error.actual() == storage),
            "{:?}",
            failure.0
        );
        assert!(failure.1 <= w && failure.3 <= s);
    }
}

#[test]
fn original_mir_slice_reads_require_current_metadata_index_extent_and_alignment() {
    let text = include_str!("original_semantic_mir_source_slice_reads_v41.vrs");
    for guard in [
        "invocation_source_byte_state_well_formed_v36(source)",
        "invocation_source_pointer_carrier_v36(source.machine.values[recipe.local], recipe.metadata_bits)",
        "invocation_source_byte_value_typed_v36(source.machine.values[recipe.index], recipe.index_bits)",
        "0 <= index < slice.length",
        "offset + width <= recipe.stride",
        "slice.length * recipe.stride, recipe.alignment)",
        "slice.pointer.byte_offset + slice.length * recipe.stride < memory_value_modulus_v30(8)",
        "recipe.stride, recipe.alignment)",
        "projected, width, alignment)",
        "..slice.pointer",
        "..base",
    ] {
        assert!(text.contains(guard), "missing guard {guard}");
    }
    assert!(
        !text.contains("assume(")
            && !text.contains("Symbol(")
            && !text.contains("MemoryAllocationV30::")
            && !text.contains("byte_store")
    );
}

#[test]
fn original_mir_slice_read_emission_and_headers_have_independent_exact_costs() {
    use crate::mixed_optimizer_refinement_v26::SOURCE_LIMIT;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    type Fields = (usize, usize, u32, u32, u64, u64);
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    let expected_header = h::<Fields>()
        + h::<(usize, u32)>()
        + h::<Option<(Fields, TypeId)>>()
        + 12 * size_of::<usize>()
        + 8 * size_of::<&()>();
    assert_eq!(size_of::<Slice>(), size_of::<Fields>());
    assert_eq!(slice_reads::headers(), expected_header);
    let source = Slice {
        local: 4,
        index: 1,
        index_bits: 32,
        metadata_bits: 64,
        stride: 4,
        alignment: 4,
    };
    let expected = "InvocationSourceByteBaseV36::SliceElement(InvocationSourceSliceReadV41 { local: 4int, index: 1int, index_bits: 32int, metadata_bits: 64int, stride: 4int, alignment: 4int })";
    let work = 1 + expected.len();
    let storage = SOURCE_LIMIT + expected_header;
    let run = |w, s| {
        let mut work = Work::new(w);
        let mut budget = Budget::new(&mut work, s);
        let result = (|| {
            budget.reserve_storage(SOURCE_LIMIT + slice_reads::headers())?;
            let mut out = Writer::new(&mut budget)?;
            slice_reads::emit(source, &mut out)?;
            out.finish()
        })();
        (result, budget.work(), budget.peak_storage())
    };
    let exact = run(work, storage);
    assert_eq!(exact.0.unwrap(), expected);
    assert_eq!((exact.1, exact.2), (work, storage));
    assert!(
        matches!(run(work-1,storage).0, Err(Error::Resource(Resource::Work(error))) if error.limit()==work-1 && error.actual()==work)
    );
    assert!(
        matches!(run(work,storage-1).0, Err(Error::Resource(Resource::Storage(error))) if error.limit()==storage-1 && error.actual()==storage)
    );
}
