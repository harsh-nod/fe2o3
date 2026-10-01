use super::*;
use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
use fe2o3_mir_model::semantic_mir_v1::*;

const LIMIT: usize = 100_000_000;

fn transform(types: &mut Vec<Type>, functions: &mut Vec<Function>, mutable: bool) {
    let word = TypeId::from_index(0);
    let length = TypeId::from_index(types.len() as u32);
    types.push(Type::new(
        SemanticTypeIdentityV1::from_sha256([201; 32]),
        SemanticLayoutIdentityV1::from_sha256([202; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            BackendRepr::scalar(BackendScalar::initialized(
                BackendPrimitive::integer(false, 64, 8),
                SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        Shape::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 64,
        }),
    ));
    let slice = TypeId::from_index(types.len() as u32);
    types.push(Type::new(
        SemanticTypeIdentityV1::from_sha256([203; 32]),
        SemanticLayoutIdentityV1::from_sha256([204; 32]),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            4,
            Fields::array(4, 0),
            SemanticRustcVariantsV1::Single { index: 0 },
            BackendRepr::memory(false),
            None,
            false,
            None,
            4,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        Shape::Slice { element: word },
    ));
    let reference = TypeId::from_index(types.len() as u32);
    types.push(
        Type::new(
            SemanticTypeIdentityV1::from_sha256([205; 32]),
            SemanticLayoutIdentityV1::from_sha256([206; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(16),
                8,
                BackendRepr::scalar_pair(
                    BackendScalar::initialized(
                        BackendPrimitive::pointer(0, 8, 8),
                        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
                    ),
                    BackendScalar::initialized(
                        BackendPrimitive::integer(false, 64, 8),
                        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
                    ),
                ),
                false,
            )
            .unwrap(),
            Shape::Pointer(
                PointerType::new_with_kind(
                    slice,
                    PointerKind::Reference,
                    if mutable {
                        Mutability::Mutable
                    } else {
                        Mutability::Immutable
                    },
                    0,
                    64,
                    PointerMetadata::SliceLength,
                )
                .unwrap(),
            ),
        )
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(
                    SemanticAbiPointeeInfoV1::new(
                        if mutable {
                            SemanticAbiPointeeKindV1::MutableReference { unpin: true }
                        } else {
                            SemanticAbiPointeeKindV1::SharedReference { frozen: true }
                        },
                        0,
                        4,
                    )
                    .unwrap(),
                ),
                None,
            ),
        ),
    );
    let second = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let first = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(
            true,
            (!mutable).then_some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
            true,
            !mutable,
            false,
            true,
        ),
        SemanticAbiExtensionV1::None,
        0,
        Some(4),
    )
    .unwrap();
    for root in 0..2 {
        let prior = &functions[root];
        let source = prior.source();
        let mut locals = prior.locals().to_vec();
        assert_eq!(locals.len(), 4);
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([210 + root as u8 * 2; 32]),
            reference,
            SemanticLocalRoleV1::Argument(2),
            source,
        ));
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([211 + root as u8 * 2; 32]),
            length,
            SemanticLocalRoleV1::Temporary,
            source,
        ));
        let place =
            |local, ty| Place::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
        let assign = |value| {
            SemanticStatementV1::new(
                source,
                Statement::Assign(SemanticAssignmentV1::new(
                    place(5, length),
                    SemanticRvalueV1::new(length, value),
                )),
            )
        };
        let mut statements = vec![
            assign(Rvalue::Length(
                Place::new(
                    SemanticLocalIdV1::from_index(4),
                    vec![SemanticProjectionV1::new(Projection::Dereference, slice).unwrap()],
                    slice,
                )
                .unwrap(),
            )),
            assign(Rvalue::Unary {
                operation: SemanticUnaryOpV1::PointerMetadata,
                operand: Operand::Copy(place(4, reference)),
            }),
            assign(Rvalue::Unary {
                operation: SemanticUnaryOpV1::PointerMetadata,
                operand: Operand::Move(place(4, reference)),
            }),
        ];
        let mut blocks = prior.blocks().to_vec();
        statements.extend_from_slice(blocks[0].statements());
        blocks[0] = SemanticBasicBlockV1::new(
            blocks[0].identity(),
            blocks[0].source(),
            statements,
            blocks[0].terminator().clone(),
        )
        .unwrap();
        let mut arguments: Vec<_> = prior
            .abi()
            .arguments()
            .iter()
            .map(|row| row.value().clone())
            .collect();
        arguments.push(SemanticAbiValueV1::new(
            reference,
            SemanticAbiPassModeV1::Pair { first, second },
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

fn run(
    mutable: bool,
    work: usize,
    storage: usize,
    examine: impl FnOnce(&SourceByteBody<'_, '_, '_>, &mut Writer<'_, '_>) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    super::super::super::super::invocations::tests::run_source_transform(
        work,
        storage,
        |types, functions| transform(types, functions, mutable),
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
fn original_mir_metadata_reads_genuine_shared_and_mutable_source_slice_carriers() {
    for mutable in [false, true] {
        run(mutable, LIMIT, LIMIT, |body, out| {
            let start = body.locals.start;
            for (statement, moved) in [(0, false), (1, false), (2, true)] {
                assert_eq!(
                    body.event_at(0, statement, out)?,
                    super::super::Event::Pointer(Event::Length {
                        destination: start + 5,
                        local: start + 4,
                        metadata_bits: 64,
                        moved,
                    })
                );
            }
            body.emit(out)?;
            assert!(out.text.contains("InvocationSourcePointerEventV36::Length"));
            assert!(out.text.contains("metadata_bits: 64int, moved: true"));
            Ok(())
        })
        .0
        .unwrap();
    }
}

#[test]
fn original_mir_metadata_rejects_wrong_result_type_and_forged_source_local_type() {
    for mutation in 0..2 {
        run(false, LIMIT, LIMIT, |body, out| {
            let context = body.context(out)?;
            let Statement::Assign(original) = context.function.blocks()[0].statements()[1].kind()
            else {
                unreachable!()
            };
            let Rvalue::Unary { operand, .. } = original.value().kind() else {
                unreachable!()
            };
            let (destination, ty, input) = if mutation == 0 {
                let word = TypeId::from_index(0);
                (
                    Place::new(SemanticLocalIdV1::from_index(1), vec![], word).unwrap(),
                    word,
                    operand.clone(),
                )
            } else {
                (
                    original.destination().clone(),
                    original.value().result_type(),
                    Operand::Copy(
                        Place::new(SemanticLocalIdV1::from_index(1), vec![], operand.ty()).unwrap(),
                    ),
                )
            };
            let forged = Statement::Assign(SemanticAssignmentV1::new(
                destination,
                SemanticRvalueV1::new(
                    ty,
                    Rvalue::Unary {
                        operation: SemanticUnaryOpV1::PointerMetadata,
                        operand: input,
                    },
                ),
            ));
            assert!(derive(&context, &forged, out).is_err());
            Ok(())
        })
        .0
        .unwrap();
    }
}

#[test]
fn original_mir_metadata_has_exact_and_one_short_whole_resources() {
    let execute = |work, storage| run(true, work, storage, |body, out| body.emit(out));
    let (result, work, _, storage) = execute(LIMIT, LIMIT);
    result.unwrap();
    let exact = execute(work, storage);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.3), (work, storage));
    for (w, s, is_work) in [(work - 1, storage, true), (work, storage - 1, false)] {
        let failure = execute(w, s);
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
fn original_mir_metadata_runtime_uses_current_length_and_move_state_without_symbol_premises() {
    let body = SOURCE_POINTERS_V36.split_once(
        "InvocationSourcePointerEventV36::Length { destination, local, metadata_bits, moved } =>"
    ).unwrap().1.split_once("InvocationSourcePointerEventV36::SliceBorrow").unwrap().0;
    assert!(
        body.contains(
            "invocation_source_carrier_evaluate_v36(source, local, moved, metadata_bits)"
        )
    );
    assert!(body.contains("invocation_source_pointer_carrier_v36(evaluated.value, metadata_bits)"));
    assert!(body.contains("MemoryValueV30::Scalar(slice.length)"));
    assert!(body.contains("invocation_source_byte_put_local_v36(evaluated.source, destination"));
    assert!(
        !body.contains("byte_load_v30") && !body.contains("Symbol(") && !body.contains("assume(")
    );
}

#[test]
fn original_mir_metadata_emission_has_independent_exact_work_and_storage() {
    use crate::mixed_optimizer_refinement_v26::SOURCE_LIMIT;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    for moved in [false, true] {
        let event = Event::Length {
            destination: 8,
            local: 4,
            metadata_bits: 64,
            moved,
        };
        let expected = format!(
            "InvocationSourcePointerEventV36::Length {{ destination: 8int, local: 4int, metadata_bits: 64int, moved: {moved} }}"
        );
        let work = 1 + expected.len();
        let storage = SOURCE_LIMIT + headers();
        let run = |work, storage| {
            let mut work = Work::new(work);
            let mut budget = Budget::new(&mut work, storage);
            let result = (|| {
                budget.reserve_storage(SOURCE_LIMIT + headers())?;
                let mut out = Writer::new(&mut budget)?;
                emit(event, &mut out)?;
                out.finish()
            })();
            (result, budget.work(), budget.peak_storage())
        };
        let exact = run(work, storage);
        assert_eq!(exact.0.unwrap(), expected);
        assert_eq!((exact.1, exact.2), (work, storage));
        assert!(matches!(run(work - 1, storage).0,
            Err(Error::Resource(Resource::Work(error))) if error.limit() == work - 1 && error.actual() == work));
        assert!(matches!(run(work, storage - 1).0,
            Err(Error::Resource(Resource::Storage(error))) if error.limit() == storage - 1 && error.actual() == storage));
    }
}
