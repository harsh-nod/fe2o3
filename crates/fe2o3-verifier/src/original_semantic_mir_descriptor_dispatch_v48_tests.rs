use super::*;
use fe2o3_kernel_descriptor::{
    AccessMode, DeviceLayoutDescriptorV1, DeviceLayoutRecordV1, LogicalArgumentV1,
    ScalarTypeV1 as DescriptorScalar, SourceTypeDescriptorV1, SourceTypeDescriptorV3,
    SourceTypeRecordV1, ValidName,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use fe2o3_lower_mir_kernel::{
    ProductionExecutionSourceInputV29, ProductionKernelArgumentAbiArgumentV18,
    ProductionKernelArgumentAbiInputV18, ProductionKernelArgumentAbiKindV18,
    ProductionKernelArgumentAbiRootV18, ProductionPendingScopedSourceOwnerV29,
    ProductionPreparedSourceV18, ProductionScopeCallableCandidateV29,
    ProductionSemanticKirLimitsV1, ProductionSourceLaunchRosterV1,
    ProductionSourceOwnedViewErrorV18 as SourceError,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::ProductionSemanticSsaOwnerV1;

const LIMIT: usize = 256 << 20;

#[path = "original_semantic_mir_descriptor_loans_v51_tests.rs"]
mod descriptor_loans_v51_tests;

fn fixture(
    types: &mut Vec<Type>,
    functions: &mut Vec<Function>,
    callables: &mut Vec<SemanticCallableDeclV1>,
) {
    let word = TypeId::from_index(0);
    let unit = TypeId::from_index(1);
    assert!(matches!(types[unit.index() as usize].shape(), Shape::Unit));
    let length = TypeId::from_index(types.len() as u32);
    let integer = BackendScalar::initialized(
        BackendPrimitive::integer(false, 64, 8),
        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
    );
    let raw = BackendScalar::initialized(
        BackendPrimitive::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
    );
    let raw_properties = SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
        Some(SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap()),
        None,
    );
    types.push(Type::new(
        SemanticTypeIdentityV1::from_sha256([201; 32]),
        SemanticLayoutIdentityV1::from_sha256([201; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            BackendRepr::scalar(integer),
            false,
        )
        .unwrap(),
        Shape::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 64,
        }),
    ));
    let pointer = TypeId::from_index(types.len() as u32);
    types.push(
        Type::new(
            SemanticTypeIdentityV1::from_sha256([202; 32]),
            SemanticLayoutIdentityV1::from_sha256([202; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                BackendRepr::scalar(raw),
                false,
            )
            .unwrap(),
            Shape::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    word,
                    PointerKind::Raw,
                    SemanticMutabilityV1::Mutable,
                    0,
                    64,
                    PointerMetadata::None,
                )
                .unwrap(),
            ),
        )
        .with_rustc_abi_properties(raw_properties),
    );
    let descriptor = TypeId::from_index(types.len() as u32);
    types.push(
        Type::new(
            SemanticTypeIdentityV1::from_sha256([203; 32]),
            SemanticLayoutIdentityV1::from_sha256([203; 32]),
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(16),
                8,
                BackendRepr::scalar_pair(raw, integer),
                false,
                SemanticAggregateLayoutV1::new(vec![0, 8, 16], vec![]).unwrap(),
            )
            .unwrap(),
            Shape::Aggregate(SemanticAggregateTypeV1::new(vec![pointer, length, unit]).unwrap()),
        )
        .with_rustc_abi_properties(raw_properties),
    );
    let reference = TypeId::from_index(types.len() as u32);
    types.push(
        Type::new(
            SemanticTypeIdentityV1::from_sha256([204; 32]),
            SemanticLayoutIdentityV1::from_sha256([204; 32]),
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
                SemanticPointerTypeV1::new_with_kind(
                    descriptor,
                    PointerKind::Reference,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    PointerMetadata::None,
                )
                .unwrap(),
            ),
        )
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(
                    SemanticAbiPointeeInfoV1::new(
                        SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                        16,
                        8,
                    )
                    .unwrap(),
                ),
                None,
            ),
        ),
    );
    let lookalike = TypeId::from_index(types.len() as u32);
    types.push(
        Type::new(
            SemanticTypeIdentityV1::from_sha256([205; 32]),
            SemanticLayoutIdentityV1::from_sha256([205; 32]),
            types[descriptor.index() as usize].layout().clone(),
            types[descriptor.index() as usize].shape().clone(),
        )
        .with_rustc_abi_properties(raw_properties),
    );

    let plain = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let shared = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(
            true,
            Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
            true,
            true,
            false,
            true,
        ),
        SemanticAbiExtensionV1::None,
        16,
        Some(8),
    )
    .unwrap();
    // The nominal binding must belong to a genuinely reachable intrinsic call,
    // not an unused declaration inserted merely to classify an equal layout.
    let intrinsic_abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256([240; 32]),
        functions[0].abi().layout_identity(),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![SemanticAbiValueV1::new(
            reference,
            SemanticAbiPassModeV1::Direct(shared),
        )],
        SemanticAbiValueV1::new(length, SemanticAbiPassModeV1::Direct(plain)),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::SharedBorrow])
    .unwrap();
    let length_callable = SemanticCallableIdV1::from_index(callables.len() as u32);
    callables.push(SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([240; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([240; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([240; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([240; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([240; 32]),
            functions[0].source(),
            intrinsic_abi,
        ),
        operation: SemanticCompilerIntrinsicOperationV1::DisjointSliceLen {
            disjoint_slice: descriptor,
            element: word,
            raw_index: length,
            index_space: SemanticDisjointIndexSpaceV1::Index1d,
        },
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([240; 32]),
    });
    for root in 0..2 {
        let old = &functions[root];
        let source = old.source();
        let mut locals = old.locals().to_vec();
        assert_eq!(locals.len(), 4);
        for (ordinal, ty) in [descriptor, descriptor, reference, lookalike, length]
            .into_iter()
            .enumerate()
        {
            locals.push(SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([210 + root as u8 * 5 + ordinal as u8; 32]),
                ty,
                if ordinal == 0 {
                    SemanticLocalRoleV1::Argument(2)
                } else {
                    SemanticLocalRoleV1::Temporary
                },
                source,
            ));
        }
        let place =
            |local| Place::new(SemanticLocalIdV1::from_index(local), vec![], descriptor).unwrap();
        let mut statements = Vec::new();
        for (destination, input) in [(5, 4), (4, 5)] {
            statements.push(SemanticStatementV1::new(
                source,
                Statement::Assign(SemanticAssignmentV1::new(
                    place(destination),
                    SemanticRvalueV1::new(descriptor, Rvalue::Use(Operand::Move(place(input)))),
                )),
            ));
        }
        let mut blocks = old.blocks().to_vec();
        assert_eq!(old.entry().index(), 0);
        let continuation = SemanticBlockIdV1::from_index(blocks.len() as u32);
        let original_entry = &blocks[0];
        let resumed = SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([244 + root as u8; 32]),
            original_entry.source(),
            original_entry.statements().to_vec(),
            original_entry.terminator().clone(),
        )
        .unwrap();
        let borrowed = Place::new(SemanticLocalIdV1::from_index(6), vec![], reference).unwrap();
        statements.push(SemanticStatementV1::new(
            source,
            Statement::Assign(SemanticAssignmentV1::new(
                borrowed.clone(),
                SemanticRvalueV1::new(
                    reference,
                    Rvalue::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place: place(4),
                    },
                ),
            )),
        ));
        blocks[0] = SemanticBasicBlockV1::new(
            blocks[0].identity(),
            blocks[0].source(),
            statements,
            SemanticTerminatorV1::new(
                source,
                Terminator::Call(
                    SemanticDirectCallV1::new_callable(
                        length_callable,
                        vec![Operand::Copy(borrowed)],
                        Some(SemanticCallDestinationV1::new(
                            Place::new(SemanticLocalIdV1::from_index(8), vec![], length).unwrap(),
                            SemanticControlFlowEdgeV1::new(
                                SemanticEdgeRoleV1::CallReturn,
                                continuation,
                            ),
                        )),
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                ),
            ),
        )
        .unwrap();
        blocks.push(resumed);
        let mut arguments: Vec<_> = old
            .abi()
            .arguments()
            .iter()
            .map(|arg| arg.value().clone())
            .collect();
        arguments.push(SemanticAbiValueV1::new(
            descriptor,
            SemanticAbiPassModeV1::Pair {
                first: plain,
                second: plain,
            },
        ));
        let abi = SemanticFunctionAbiV1::new(
            old.abi().identity(),
            old.abi().layout_identity(),
            SemanticCanonAbiV1::GpuKernel,
            false,
            false,
            arguments,
            old.abi().return_value().clone(),
        )
        .unwrap()
        .with_source_argument_ownership(vec![
            SemanticSourceArgumentOwnershipV1::ByValue,
            SemanticSourceArgumentOwnershipV1::ByValue,
            SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
        ])
        .unwrap();
        functions[root] = Function::new(
            old.identity(),
            old.role(),
            old.item_definition_identity(),
            old.monomorphization_identity(),
            old.generic_type_arguments_identity(),
            old.const_generic_arguments_identity(),
            source,
            abi,
            locals,
            old.entry(),
            blocks,
        )
        .unwrap()
        .with_kernel_entry(old.kernel_entry().unwrap().clone());
    }
}

fn capture(
    owner: ProductionSemanticSsaOwnerV1,
    launch: ProductionSourceLaunchRosterV1,
    budget: &mut Budget<'_>,
) -> Result<ProductionPreparedSourceV18> {
    let original = owner.source_semantic();
    let mut bindings = Vec::new();
    let mut exports = Vec::new();
    let mut arguments = Vec::new();
    for root in original.roots() {
        let function = &original.functions()[root.index() as usize];
        let entry = function.kernel_entry().unwrap();
        bindings.push(*entry.kernel_binding_identity().as_bytes());
        exports.push(
            std::str::from_utf8(entry.export_symbol().as_bytes())
                .unwrap()
                .to_owned(),
        );
        let mut rows = Vec::new();
        assert_eq!(function.abi().source_input_types().len(), 3);
        for (ordinal, ty) in function.abi().source_input_types().iter().enumerate() {
            let name = ValidName::new(format!("arg{ordinal}")).unwrap();
            let (source, argument) = if ordinal == 2 {
                let source = SourceTypeRecordV1::new(SourceTypeDescriptorV1::disjoint_slice(
                    DescriptorScalar::U32,
                ));
                let layout = DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::disjoint_slice(
                    DescriptorScalar::U32,
                ));
                (
                    SourceTypeDescriptorV3::DisjointSlice(DescriptorScalar::U32),
                    LogicalArgumentV1::disjoint_slice(
                        ordinal as u16,
                        name,
                        &source,
                        &layout,
                        AccessMode::ReadWrite,
                        8,
                    )
                    .unwrap(),
                )
            } else {
                assert!(matches!(
                    original.types()[ty.index() as usize].shape(),
                    Shape::Scalar(SemanticScalarTypeV1::Integer {
                        signed: false,
                        bits: 32
                    })
                ));
                let source =
                    SourceTypeRecordV1::new(SourceTypeDescriptorV1::scalar(DescriptorScalar::U32));
                let layout = DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::scalar(
                    DescriptorScalar::U32,
                ));
                (
                    SourceTypeDescriptorV3::Scalar(DescriptorScalar::U32),
                    LogicalArgumentV1::scalar(
                        ordinal as u16,
                        name,
                        &source,
                        &layout,
                        ordinal as u32 * 4,
                    )
                    .unwrap(),
                )
            };
            rows.push(ProductionKernelArgumentAbiArgumentV18 {
                semantic_type_identity: original.types()[ty.index() as usize].identity(),
                kind: ProductionKernelArgumentAbiKindV18::Descriptor { source, argument },
            });
        }
        arguments.push(rows);
    }
    let roots: Vec<_> = (0..bindings.len())
        .map(|root| ProductionKernelArgumentAbiRootV18 {
            kernel_binding: &bindings[root],
            export: &exports[root],
            arguments: &arguments[root],
            explicit_argument_bytes: 24,
            kernarg_alignment_bytes: 8,
        })
        .collect();
    let source_hash = *owner.source_semantic_sha256();
    let callable_count = original.callables().len();
    Ok(
        ProductionPendingScopedSourceOwnerV29::prepare_source_with_kernel_abi_budget_v18(
            owner,
            launch,
            ProductionExecutionSourceInputV29 {
                semantic_sha256: &source_hash,
                roots: &[],
                classes: &vec![ProductionScopeCallableCandidateV29::Ordinary; callable_count],
                events: &[],
            },
            ProductionKernelArgumentAbiInputV18 { roots: &roots },
            ProductionSemanticKirLimitsV1::default(),
            budget,
        )?,
    )
}

fn run(work: usize, storage: usize, negatives: bool) -> (Result<()>, usize, usize, usize) {
    let reached = std::cell::Cell::new(false);
    let result = super::super::super::invocations::tests::run_captured_callable_transform(
        work,
        storage,
        fixture,
        capture,
        |plan, out| {
            super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                for root in 0..2 {
                    let body = SourceByteBody::derive(plan, slots, root, 0, out)?;
                    let context = body.context(out)?;
                    let ty = context.function.locals()[4].ty();
                    assert!(matches!(
                        context.types[ty.index() as usize].shape(),
                        Shape::Aggregate(_)
                    ));
                    assert_eq!(slots.descriptor_slice_bits(ty, out)?, Some(64));
                    assert_eq!(slots.aggregate_leaf_count(ty, out)?, None);
                    assert_eq!(
                        slots.descriptor_slice_bits(context.function.locals()[7].ty(), out)?,
                        None
                    );
                    for (statement, destination, input) in [(0, 5, 4), (1, 4, 5)] {
                        assert_eq!(
                            body.event_at(0, statement, out)?,
                            Event::Pointer(pointer_events::Event::Copy {
                                destination: body.locals.start + destination,
                                operand: TypedOperand {
                                    ty,
                                    kind: OperandKind::Slice {
                                        local: body.locals.start + input,
                                        moved: true,
                                        metadata_bits: 64
                                    }
                                },
                                metadata_bits: 64,
                            })
                        );
                    }
                    if negatives {
                        let assign = |destination, input| {
                            Statement::Assign(SemanticAssignmentV1::new(
                                destination,
                                SemanticRvalueV1::new(ty, Rvalue::Use(Operand::Move(input))),
                            ))
                        };
                        let place = |local, ty| {
                            Place::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
                        };
                        for forged in [
                            assign(place(5, ty), place(1, ty)),
                            assign(place(5, ty), place(1, TypeId::from_index(0))),
                            assign(place(7, ty), place(4, ty)),
                        ] {
                            assert!(matches!(
                                context.statement(&forged, out),
                                Err(Error::Statement(
                                    "original MIR typed byte statement identity or layout differs"
                                ))
                            ));
                        }
                        let lookalike = context.function.locals()[7].ty();
                        let statement = Statement::Assign(SemanticAssignmentV1::new(
                            place(7, lookalike),
                            SemanticRvalueV1::new(
                                lookalike,
                                Rvalue::Use(Operand::Move(place(7, lookalike))),
                            ),
                        ));
                        assert_eq!(pointer_events::derive(&context, &statement, out)?, None);
                    }
                }
                let mut program =
                    super::super::source_function::SourceByteProgram::derive(plan, slots, out)?;
                let paired = super::super::paired::PairedInvocations::derive(
                    plan,
                    &program,
                    fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                    out,
                )?;
                program.emit(out)?;
                paired.emit(out)?;
                assert!(out.text.contains("InvocationSourcePointerEventV36::Copy"));
                assert!(out.text.contains("InvocationSourceOperandV36::Slice"));
                reached.set(true);
                Ok(())
            })
        },
    );
    if result.0.is_ok() {
        assert!(reached.get());
    }
    result
}

#[test]
fn original_mir_aggregate_descriptors_keep_authenticated_native_carriers_on_both_roots() {
    run(LIMIT, LIMIT, false).0.unwrap();
}

#[test]
fn original_mir_aggregate_descriptor_dispatch_rejects_wrong_local_type_and_layout_lookalike() {
    run(LIMIT, LIMIT, true).0.unwrap();
}

#[test]
fn original_mir_aggregate_descriptor_dispatch_has_exact_and_one_short_complete_resources() {
    let generous = run(LIMIT, LIMIT, false);
    generous.0.unwrap();
    let exact = run(generous.1, generous.3, false);
    exact.0.unwrap();
    assert_eq!(
        (exact.1, exact.2, exact.3),
        (generous.1, generous.2, generous.3)
    );
    for work in [true, false] {
        let w = generous.1 - usize::from(work);
        let s = generous.3 - usize::from(!work);
        let failed = run(w, s, false);
        assert!(
            matches!((work, &failed.0),
            (true, Err(Error::Source(SourceError::Resource(Resource::Work(error)))))
                if error.actual() == generous.1 && error.limit() == w)
                || matches!((work, &failed.0),
                (false, Err(Error::Source(SourceError::Resource(Resource::Storage(error)))))
                    if error.actual() == generous.3 && error.limit() == s),
            "{:?}",
            failed.0
        );
    }
}
