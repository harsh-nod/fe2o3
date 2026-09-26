use super::*;
use crate::production_semantic_kir_v1::*;

mod instance_plan_tests {
    include!("production_execution_instance_plan_v29_tests.rs");
}

mod call_sink_tests {
    include!("production_execution_call_sink_v29_tests.rs");
}

const PAIR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const HELPER: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(1);

#[test]
fn selected_raw_abi_authenticates_original_repeated_call_and_return_words() {
    for immutable in [false, true] {
        for graph in 0..3 {
            with_selected_pointer_test_plan_v29(
                selected_pointer_test_owner_v29(execution_owner(Flow::Linear).unwrap(), immutable, graph),
                |plan, budget| {
                    let mut children = Vec::new();
                    for call in plan.instances.calls(plan.root).unwrap() {
                        let child = call.child().unwrap();
                        let instance = plan.instances.instance(child).unwrap();
                        let declaration = instance.declaration();
                        check_execution_instance_abi_v29(
                            plan.instances.owner().source_semantic(), instance.function(),
                            Some((plan, child)), budget,
                        )?;
                        let node = source_reference_entry_node_v29(plan, child, SemanticLocalIdV1::from_index(1), None, budget)?.unwrap();
                        let ty = declaration.locals()[1].ty();
                        let mut path = source_reference_owned_vec_v29(plan, 0, budget)?;
                        let mut words = source_reference_owned_vec_v29(plan, 0, budget)?;
                        source_reference_abi_words_v29(plan, Some(node), ty, 0, &mut path, &mut words, &mut 0, budget)?;
                        assert_eq!(words.len(), 1);
                        let (path, source_type, validation_only, offset, leaf) = &words[0];
                        assert!(path.is_empty());
                        assert_eq!((*source_type, *offset), (ty, 0));
                        assert_eq!(validation_only, &Type::Unit);
                        let ParameterAbiLeafV1::Scalar(scalar) = leaf else { panic!("one original raw ABI word"); };
                        assert_eq!(scalar.primitive(), SemanticBackendPrimitiveV1::pointer(0, 8, 8));
                        assert_eq!(Some(*scalar), match plan.instances.owner().source_semantic().types()[ty.index() as usize].layout().backend_repr() {
                            SemanticBackendReprV1::Scalar(original) => Some(*original), _ => None,
                        });
                        let physical = source_reference_selected_pointer_type_v29(plan, node, ty, budget)?.unwrap();
                        assert_eq!(source_reference_return_types_v29(plan, child, budget)?, Some(vec![physical]));
                        children.push(child);
                    }
                    assert_eq!(children.len(), 2, "both actual calls require their own ABI join");
                    assert_ne!(children[0], children[1]);
                    Ok(())
                },
            ).unwrap();
        }
    }
}

#[test]
fn selected_raw_abi_word_metadata_never_authenticates_an_unbound_pointer() {
    with_selected_pointer_test_plan_v29(
        selected_pointer_test_owner_v29(execution_owner(Flow::Linear).unwrap(), false, 0),
        |plan, budget| {
            let ty = SemanticTypeIdV1::from_index(2);
            let copied = plan.instances.owner().source_semantic().types()[ty.index() as usize].clone();
            assert!(source_reference_raw_abi_scalar_v29(&copied).is_ok());
            let mut path = source_reference_owned_vec_v29(plan, 0, budget)?;
            let mut words = source_reference_owned_vec_v29(plan, 0, budget)?;
            assert!(source_reference_abi_words_v29(plan, None, ty, 0, &mut path, &mut words, &mut 0, budget).is_err());
            assert!(words.is_empty(), "copied ABI metadata cannot issue a pointer word");
            Ok(())
        },
    ).unwrap();
    for wrong in [SemanticTypeIdV1::from_index(1), SemanticTypeIdV1::from_index(3)] {
        let mut completed = false;
        let result = with_selected_pointer_test_plan_v29(
            selected_pointer_test_owner_v29(execution_owner(Flow::Linear).unwrap(), false, 0),
            |plan, budget| {
                let child = plan.instances.calls(plan.root).unwrap()[0].child().unwrap();
                let node = source_reference_entry_node_v29(plan, child, SemanticLocalIdV1::from_index(1), None, budget)?.unwrap();
                assert_ne!(plan.nodes[node].ty, wrong);
                assert!(source_reference_selected_abi_scalar_v29(plan, node, wrong, budget).is_err());
                let before = (budget.work(), budget.storage());
                assert!(source_reference_selected_abi_scalar_v29(plan, node, plan.nodes[node].ty, budget).is_err());
                assert_eq!((budget.work(), budget.storage()), before);
                completed = true;
                Ok(())
            },
        );
        assert!(completed, "all negative assertions must run, not be caught as a scope failure");
        assert!(matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Accounting))),
            "wrong scalar or pointer-pointee type retains its exact sticky error: {result:?}");
    }
}

#[test]
fn selected_raw_abi_words_reject_wrong_width_address_space_metadata_and_layout() {
    let (mut layout_refusals, mut abi_refusals) = (0, 0);
    for mutation in 0..10 {
        let layout = SemanticTypeLayoutV1::new_with_backend_repr(
                Some(if mutation == 4 { 16 } else { 8 }),
                if mutation == 5 { 4 } else { 8 },
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    if mutation == 6 { SemanticBackendPrimitiveV1::integer(false, 64, 8) }
                    else { SemanticBackendPrimitiveV1::pointer(if mutation == 8 { 1 } else { 0 }, 8, 8) },
                    SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
                )),
                mutation == 7,
            );
        if matches!(mutation, 4 | 5) {
            // Scalar layout construction itself rejects a mismatched byte
            // size/alignment. Do not manufacture an impossible admitted row.
            assert!(matches!(layout, Err(SemanticMirErrorV1::InvalidTypeLayout)), "mutation {mutation}: {layout:?}");
            layout_refusals += 1;
            continue;
        }
        let declaration = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([212; 32]),
            SemanticLayoutIdentityV1::from_sha256([213; 32]),
            layout.unwrap_or_else(|error| panic!("mutation {mutation} must reach ABI word validation: {error:?}")),
            SemanticTypeShapeV1::Pointer(SemanticPointerTypeV1::new_with_kind(
                SemanticTypeIdV1::from_index(1),
                if mutation == 3 { SemanticPointerKindV1::Reference } else { SemanticPointerKindV1::Raw },
                SemanticMutabilityV1::Mutable,
                if mutation == 1 { 1 } else { 0 },
                if mutation == 0 { 32 } else { 64 },
                if mutation == 2 { SemanticPointerMetadataV1::SliceLength }
                else if mutation == 9 { SemanticPointerMetadataV1::VTable }
                else { SemanticPointerMetadataV1::None },
            ).unwrap()),
        );
        assert!(source_reference_raw_abi_scalar_v29(&declaration).is_err(), "mutation {mutation}");
        abi_refusals += 1;
    }
    assert_eq!((layout_refusals, abi_refusals), (2, 8));
}

#[test]
fn selected_raw_abi_query_preserves_exact_and_one_short_resource_limits() {
    let mut measured = None;
    let source = || selected_pointer_test_owner_v29(execution_owner(Flow::Linear).unwrap(), false, 0);
    let node = |plan: &SourceReferencePlanV29<'_, '_>| plan.boundary_values.iter().find(|row|
        row.site.instance == plan.instances.calls(plan.root).unwrap()[0].child().unwrap()
            && row.role == SourceReferenceBoundaryRoleV29::Return).unwrap().node;
    with_selected_pointer_test_plan_v29(source(), |plan, budget| {
        budget.reserve_storage(budget.peak_storage() - budget.storage() + 1)?;
        let node = node(plan);
        let before = (budget.work(), budget.storage());
        source_reference_selected_abi_scalar_v29(plan, node, plan.nodes[node].ty, budget)?;
        measured = Some((budget.work() - before.0, budget.storage() - before.1, budget.peak_storage() - before.1));
        Ok(())
    }).unwrap();
    let (work, storage, peak) = measured.unwrap();
    assert!(work > 12 && storage > 0 && peak >= storage);
    for storage_cut in [false, true] {
        for short in [0, 1] {
            let mut published = false;
            let mut completed = false;
            let result = with_selected_pointer_test_plan_v29(source(), |plan, budget| {
                budget.reserve_storage(budget.peak_storage() - budget.storage() + 1)?;
                let node = node(plan);
                if storage_cut { budget.reserve_storage(64 * 1024 * 1024 - budget.storage() - (peak - short))?; }
                else { budget.charge_work(20_000_000 - budget.work() - (work - short))?; }
                let before = (budget.work(), budget.storage());
                let result = source_reference_selected_abi_scalar_v29(plan, node, plan.nodes[node].ty, budget);
                if short == 0 {
                    result?;
                    assert_eq!((budget.work() - before.0, budget.storage() - before.1), (work, storage));
                    assert_eq!(budget.peak_storage() - before.1, peak);
                    published = true;
                } else {
                    match (storage_cut, result) {
                        (true, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Storage(_))))
                        | (false, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(_)))) => {},
                        (_, other) => panic!("wrong one-short failure: {other:?}"),
                    }
                    assert!(source_reference_selected_abi_scalar_v29(plan, node, plan.nodes[node].ty, budget).is_err());
                }
                completed = true;
                Ok(())
            });
            assert!(completed, "all resource assertions must execute: {result:?}");
            assert_eq!(published, short == 0);
            if short == 0 {
                result.unwrap();
            } else {
                match (storage_cut, result) {
                    (true, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Storage(_))))
                    | (false, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(_)))) => {},
                    (_, other) => panic!("the original one-short error must escape unchanged: {other:?}"),
                }
            }
        }
    }
}

#[test]
fn selected_raw_call_nodes_keep_original_abi_instance_and_exact_physical_identity() {
    for immutable in [false, true] {
        let source = selected_pointer_test_owner_v29(execution_owner(Flow::Linear).unwrap(), immutable, 0);
        with_selected_pointer_test_plan_v29(source, |plan, budget| {
            let references = SourceReferenceEmissionV29::new(plan, budget)?;
            let mut children = Vec::new();
            for call in plan.instances.calls(plan.root).unwrap() {
                let child = call.child().unwrap();
                let declaration = plan.instances.instance(child).unwrap().declaration();
                assert_eq!(declaration.abi().arguments().len(), 1);
                let node = source_reference_entry_node_v29(plan, child, SemanticLocalIdV1::from_index(1), None, budget)?.unwrap();
                assert_eq!(plan.nodes[node].ty, declaration.locals()[1].ty());
                let expected = source_reference_selected_pointer_type_v29(plan, node, declaration.locals()[1].ty(), budget)?.unwrap();
                let Type::Pointer(pointer) = &expected else { panic!("ordinary selected pointer parameter"); };
                assert_eq!(pointer.address_space, AddressSpace::Private);
                assert_eq!(pointer.access, if immutable { AccessMode::ReadOnly } else { AccessMode::ReadWrite });
                assert!(matches!(*pointer.pointee, Type::StorageObject(_)));
                let id = ValueId(900 + child.index() as u32);
                let binding = SemanticValueBindingV1::Value { id, ty: expected.clone() };
                let (leaves, exact) = source_reference_call_shape_v29(&references, node, &binding, budget)?;
                assert_eq!(exact, [Some(id)], "selected ordinary pointers retain prepared argument IDs");
                let values = [ValueDef::new(id, expected.clone())];
                let mut values = values.iter();
                let mut leaves = leaves.iter();
                let rebuilt = source_reference_rebuild_node_v29(&references, node, true, &mut leaves, &mut values, &mut 0, budget)?;
                assert!(matches!(rebuilt, SemanticValueBindingV1::Value { id: actual, ty } if actual == id && ty == expected));
                assert!(leaves.next().is_none() && values.next().is_none());
                let returned = plan.boundary_values.iter().find(|row| row.site.instance == child && row.role == SourceReferenceBoundaryRoleV29::Return).unwrap();
                assert_eq!(plan.nodes[returned.node].ty, declaration.locals()[0].ty());
                assert_eq!(source_reference_node_types_v29(plan, returned.node, budget)?, [expected]);
                let caller = plan.boundary_values.iter().find(|row|
                    row.site.instance == plan.root && row.site.block == call.occurrence().block
                        && row.role == SourceReferenceBoundaryRoleV29::Argument(0)).unwrap();
                assert_eq!(source_reference_node_types_v29(plan, caller.node, budget)?, source_reference_node_types_v29(plan, node, budget)?);
                children.push(child);
            }
            assert_eq!(children.len(), 2);
            assert_ne!(children[0], children[1]);
            references.abort_scope(plan.instances, budget)
        }).unwrap();
    }
}

#[test]
fn selected_raw_call_and_return_components_refuse_wrong_type_and_same_type_id_substitution() {
    for mutation in 0..3 {
        let mut entered = false;
        with_selected_pointer_test_plan_v29(selected_pointer_test_owner_v29(execution_owner(Flow::Linear).unwrap(), false, 0), |plan, budget| {
            entered = true;
            let references = SourceReferenceEmissionV29::new(plan, budget)?;
            let child = plan.instances.calls(plan.root).unwrap()[0].child().unwrap();
            let node = source_reference_entry_node_v29(plan, child, SemanticLocalIdV1::from_index(1), None, budget)?.unwrap();
            let expected = source_reference_selected_pointer_type_v29(plan, node, plan.nodes[node].ty, budget)?.unwrap();
            let held = SemanticValueBindingV1::Value { id: ValueId(910), ty: expected.clone() };
            if mutation == 0 {
                let changed = SemanticValueBindingV1::Value { id: ValueId(910), ty: Type::Scalar(ScalarType::U32) };
                assert!(source_reference_call_shape_v29(&references, node, &changed, budget).is_err());
            } else if mutation == 1 {
                let changed = SemanticValueBindingV1::Value { id: ValueId(911), ty: expected };
                assert!(source_reference_merge_node_v29(&references, node, &held, &changed, &mut [].iter_mut(), &mut 0, budget).is_err());
            } else {
                let returned = plan.boundary_values.iter().find(|row| row.site.instance == child && row.role == SourceReferenceBoundaryRoleV29::Return).unwrap();
                let values = [ValueDef::new(ValueId(910), Type::Scalar(ScalarType::U32))];
                assert!(source_reference_rebuild_node_v29(&references, returned.node, true, &mut [].iter(), &mut values.iter(), &mut 0, budget).is_err());
            }
            references.abort_scope(plan.instances, budget)
        }).unwrap();
        assert!(entered);
    }
}

#[test]
fn selected_raw_call_shape_rejoins_original_source_argument_instance_and_component() {
    for mutation in 0..5 {
        with_selected_pointer_test_plan_v29(selected_pointer_test_owner_v29(execution_owner(Flow::Linear).unwrap(), false, 0), |plan, budget| {
            let references = SourceReferenceEmissionV29::new(plan, budget)?;
            with_execution_call_scope_v29(budget, |scope, budget| {
                let call = &plan.instances.calls(plan.root).unwrap()[0];
                let child = call.child().unwrap();
                let node = source_reference_entry_node_v29(plan, child, SemanticLocalIdV1::from_index(1), None, budget)?.unwrap();
                let expected = source_reference_selected_pointer_type_v29(plan, node, plan.nodes[node].ty, budget)?.unwrap();
                let binding = SemanticValueBindingV1::Value { id: ValueId(920), ty: expected.clone() };
                // A consumer mutation fixture, not authenticated call issuance.
                // The real producer/constructor route is observed separately.
                let mut origin = PreparedExecutionCallOriginV29 {
                    scope, ledger: budget.work_ledger_identity_v1(),
                    source: ExecutionCallSourceV29::from_instances(plan.instances, budget)?,
                    function: plan.instances.instance(plan.root).unwrap().function(),
                    occurrence: call.occurrence(), callee: plan.instances.instance(child).unwrap().function(),
                    projections: vec![HelperCallArgumentV1 { source_argument: 0, tuple_field: None, component: Some(0) }],
                    parameter_types: vec![expected],
                };
                assert!(source_reference_call_argument_shape_v29(&references, &origin, 0, &binding, budget)?);
                match mutation {
                    0 => origin.projections[0].source_argument = 1,
                    1 => origin.occurrence.caller = child,
                    2 => origin.projections[0].component = Some(1),
                    3 => origin.source.semantic[0] ^= 1,
                    4 => origin.parameter_types[0] = Type::Scalar(ScalarType::U32),
                    _ => unreachable!(),
                }
                assert!(source_reference_call_argument_shape_v29(&references, &origin, 0, &binding, budget).is_err());
                Ok(())
            })?;
            references.abort_scope(plan.instances, budget)
        }).unwrap();
    }
}

#[derive(Clone, Copy, Debug)]
enum Shape {
    Tuple,
    Struct,
    RustCall,
    IndexTuple,
    IndexRustCall,
    PackedRustCall,
}

impl Shape {
    fn rust_call(self) -> bool {
        matches!(
            self,
            Self::RustCall | Self::IndexRustCall | Self::PackedRustCall
        )
    }

    fn expanded(self) -> bool {
        self.rust_call() && !matches!(self, Self::PackedRustCall)
    }

    fn index(self) -> bool {
        matches!(self, Self::IndexTuple | Self::IndexRustCall)
    }

    fn physical(self) -> Type {
        Type::Scalar(if self.index() {
            ScalarType::U64
        } else {
            ScalarType::U32
        })
    }

    fn caller_type(self) -> Type {
        if self.index() {
            Type::INDEX
        } else {
            self.physical()
        }
    }
}

// Source/SSA fixtures below deliberately use test-only nominal producers.
// They exercise shared emission and construction, not authenticated issuance.
fn owner(shape: Shape) -> ProductionSemanticSsaOwnerV1 {
    let mut types = execution_owner(Flow::Linear)
        .unwrap()
        .source_semantic()
        .types()
        .to_vec();
    if shape.index() {
        // Keep the fixture's type table closed over the selected scalar width.
        types[U32.index() as usize] = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([5; 32]),
            SemanticLayoutIdentityV1::from_sha256([5; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, 64, 8),
                    SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64,
            }),
        );
    }
    let scalar_ty = U32;
    let bytes = if shape.index() { 8 } else { 4 };
    let fields = SemanticAggregateTypeV1::new(vec![CONTEXT, scalar_ty]).unwrap();
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([201; 32]),
        SemanticLayoutIdentityV1::from_sha256([201; 32]),
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(bytes),
            bytes,
            *types[scalar_ty.index() as usize].layout().backend_repr(),
            false,
            SemanticAggregateLayoutV1::new(vec![0, 0], vec![]).unwrap(),
        )
        .unwrap(),
        if matches!(shape, Shape::Struct) {
            SemanticTypeShapeV1::Aggregate(fields)
        } else {
            SemanticTypeShapeV1::Tuple(fields)
        },
    ));
    let rust_call = shape.rust_call();
    let call = |input, target| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(1),
                if rust_call {
                    vec![
                        SemanticOperandV1::Constant(SemanticConstantV1::new(
                            UNIT,
                            SemanticConstantValueV1::ZeroSized,
                        )),
                        SemanticOperandV1::Move(place(input, PAIR)),
                    ]
                } else {
                    vec![SemanticOperandV1::Move(place(input, PAIR))]
                },
                Some(SemanticCallDestinationV1::new(
                    place(3, scalar_ty),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(target),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    let root = function(
        202,
        SemanticFunctionRoleV1::KernelRoot,
        abi(203, true, &[PAIR, PAIR]),
        vec![
            local(210, UNIT, SemanticLocalRoleV1::Return),
            local(211, PAIR, SemanticLocalRoleV1::Argument(0)),
            local(212, PAIR, SemanticLocalRoleV1::Argument(1)),
            local(213, scalar_ty, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            block(220, vec![], call(1, 1)),
            block(221, vec![], call(2, 2)),
            block(222, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"execution_parameter_fixture".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([204; 32]),
        SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
    ));
    let helper_abi = if rust_call {
        SemanticFunctionAbiV1::from_rustc_with_source_signature(
            SemanticAbiIdentityV1::from_sha256([205; 32]),
            SemanticLayoutIdentityV1::from_sha256([250; 32]),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::RustCall,
            false,
            false,
            1,
            vec![UNIT, PAIR],
            scalar_ty,
            vec![
                SemanticAbiArgumentV1::source(ignored(UNIT)),
                SemanticAbiArgumentV1::rust_call_tuple_field(0, ignored(CONTEXT)),
                SemanticAbiArgumentV1::rust_call_tuple_field(1, direct(scalar_ty)),
            ],
            direct(scalar_ty),
        )
        .unwrap()
    } else {
        SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([205; 32]),
            SemanticLayoutIdentityV1::from_sha256([250; 32]),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            1,
            vec![SemanticAbiArgumentV1::source(direct(PAIR))],
            direct(scalar_ty),
        )
        .unwrap()
    };
    let helper_abi = helper_abi
        .with_source_argument_ownership(vec![
            SemanticSourceArgumentOwnershipV1::ByValue;
            if rust_call { 2 } else { 1 }
        ])
        .unwrap();
    let projected = |field, ty| {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(field), ty).unwrap()],
            ty,
        )
        .unwrap()
    };
    let (locals, context, scalar_input, temporary) = if shape.expanded() {
        (
            vec![
                local(230, scalar_ty, SemanticLocalRoleV1::Return),
                local(231, UNIT, SemanticLocalRoleV1::Argument(0)),
                local(
                    232,
                    CONTEXT,
                    SemanticLocalRoleV1::RustCallTupleField {
                        argument: 1,
                        field: 0,
                    },
                ),
                local(
                    233,
                    scalar_ty,
                    SemanticLocalRoleV1::RustCallTupleField {
                        argument: 1,
                        field: 1,
                    },
                ),
                local(234, CONTEXT, SemanticLocalRoleV1::Temporary),
            ],
            place(2, CONTEXT),
            place(3, scalar_ty),
            4,
        )
    } else if rust_call {
        let projected = |field, ty| {
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(2),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Field(field), ty).unwrap(),
                ],
                ty,
            )
            .unwrap()
        };
        (
            vec![
                local(230, scalar_ty, SemanticLocalRoleV1::Return),
                local(231, UNIT, SemanticLocalRoleV1::Argument(0)),
                local(232, PAIR, SemanticLocalRoleV1::Argument(1)),
                local(234, CONTEXT, SemanticLocalRoleV1::Temporary),
            ],
            projected(0, CONTEXT),
            projected(1, scalar_ty),
            3,
        )
    } else {
        (
            vec![
                local(230, scalar_ty, SemanticLocalRoleV1::Return),
                local(231, PAIR, SemanticLocalRoleV1::Argument(0)),
                local(232, CONTEXT, SemanticLocalRoleV1::Temporary),
            ],
            projected(0, CONTEXT),
            projected(1, scalar_ty),
            2,
        )
    };
    let helper = function(
        206,
        SemanticFunctionRoleV1::InternalHelper,
        helper_abi,
        locals,
        vec![block(
            240,
            vec![
                assign(
                    place(temporary, CONTEXT),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(context)),
                ),
                assign(
                    place(0, scalar_ty),
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::BitXor,
                        left: SemanticOperandV1::Copy(scalar_input),
                        right: SemanticOperandV1::Constant(SemanticConstantV1::new(
                            scalar_ty,
                            SemanticConstantValueV1::Scalar(
                                SemanticScalarValueV1::new(7, bytes as u8).unwrap(),
                            ),
                        )),
                    },
                ),
            ],
            SemanticTerminatorKindV1::Return,
        )],
    );
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        vec![root, helper],
        vec![
            SemanticCallableDeclV1::defined(ROOT),
            SemanticCallableDeclV1::defined(HELPER),
        ],
        vec![ROOT],
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

#[derive(Clone, Copy)]
enum Fault {
    None,
    OtherInstance,
    WrongType,
    WrongSource,
    WrongSsa,
    WrongRoot,
    WrongProjection,
    Moved,
    Alias,
    ConstructorInstance,
    ForeignBudget,
    ForeignScope,
    SecondInstance,
    DuplicateSeed,
}

fn run(
    shape: Shape,
    fault: Fault,
    work_limit: usize,
    storage_limit: usize,
) -> (
    Result<LoweredFunctionResultV1, ProductionSemanticKirErrorV1>,
    usize,
    usize,
) {
    let mut owner = owner(shape);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    let result = (|| {
        budget.reserve_storage(FLOOR)?;
        let captured = owner
            .try_capture_occurrences_with_budget_v1(&mut budget)
            .map_err(|error| match error {
                fe2o3_pliron::ProductionSemanticSsaOccurrenceErrorV1::Resource(error) => {
                    error.into()
                }
                _ => execution_call_error_v29(),
            })?;
        budget.reserve_storage(captured.retained_storage())?;
        with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(
                with_execution_call_scope_v29(budget, |scope, budget| {
                    let first = instances.calls(instances.root()).unwrap()[0]
                        .child()
                        .unwrap();
                    let second = instances.calls(instances.root()).unwrap()[1]
                        .child()
                        .unwrap();
                    let mut plan = execution_instance_plan_v29(
                        instances,
                        first,
                        FunctionId::new("scoped_child"),
                        SemanticEmissionPlacementV1 {
                            first_block: 17,
                            first_value: 300,
                        },
                        budget,
                    )?;
                    assert_eq!(plan.parameter_types, [shape.physical()]);
                    assert_eq!(plan.parameter_values, [ValueId(300)]);
                    assert_eq!(plan.result_types, [shape.physical()]);
                    let (prepared, duplicate) = with_execution_availability_v29(
                        instances,
                        instances.root(),
                        budget,
                        |mut cursor, budget| {
                            let semantic = owner.source_semantic();
                            let seed = |value| {
                                SemanticExecutionBindingV29::context(
                                    semantic.types(),
                                    CONTEXT,
                                    ProductionCallOccurrenceV1 {
                                        caller: instances.root(),
                                        block: SemanticBlockIdV1::from_index(0),
                                    },
                                    ValueId(value),
                                )
                                .unwrap()
                            };
                            let binding = |id, nominal| {
                                SemanticValueBindingV1::Aggregate(vec![
                                    if matches!(fault, Fault::Moved) {
                                        SemanticValueBindingV1::MovedExecution
                                    } else {
                                        SemanticValueBindingV1::Execution(seed(nominal))
                                    },
                                    SemanticValueBindingV1::Value {
                                        id: ValueId(id),
                                        ty: shape.caller_type(),
                                    },
                                ])
                            };
                            cursor.entry_seeds = vec![(1, binding(20, 90)), (2, binding(21, 91))];
                            let mut parent = SemanticFunctionLoweringV1::new_interprocedural(
                                semantic.types(),
                                semantic.callables(),
                                &semantic.functions()[0],
                                owner.plan_for_function(ROOT).unwrap(),
                                ROOT,
                                ROOT,
                                BTreeMap::new(),
                                BTreeMap::new(),
                                vec![],
                                SemanticParameterBindingsV1 {
                                    declarations: &[],
                                    values: &[ValueId(20), ValueId(21)],
                                    types: &[shape.caller_type(), shape.caller_type()],
                                    local_bindings: Some(&[]),
                                },
                                None,
                                Some([64, 1, 1]),
                                BTreeSet::new().into(),
                                1,
                                false,
                                1024,
                                PrivateArrayRecorderWorkV1::Owned(PrivateArrayLazyBudgetV1::new(
                                    1, 1024,
                                )),
                                None,
                                CallReturnBufferV1::empty(),
                                Some(budget),
                                SemanticEmissionPlacementV1 {
                                    first_block: 0,
                                    first_value: 200,
                                },
                                Some(cursor),
                                None,
                            )?;
                            let mut block = BasicBlock::new(BlockId(0));
                            parent.begin_block(SemanticBlockIdV1::from_index(0), &mut block)?;
                            let incoming = instances.incoming(first).unwrap();
                            let projections = &plan.call_arguments;
                            if matches!(fault, Fault::ForeignScope) {
                                let storage = parent.emission_work.as_deref().unwrap().storage();
                                let mut foreign_work =
                                    CanonicalKernelIrWorkBudgetV1::new(10_000_000);
                                let mut foreign =
                                    ArgumentBudgetV1::new(&mut foreign_work, 10_000_000);
                                let rejected =
                                    with_execution_call_scope_v29(&mut foreign, |token, _| {
                                        parent
                                            .prepare_defined_call_arguments_v1(
                                                SemanticBlockIdV1::from_index(0),
                                                incoming.source(),
                                                HELPER,
                                                DefinedCallArgumentSignatureV1 {
                                                    projection: DefinedCallProjectionV29::Execution(
                                                        token,
                                                    ),
                                                    semantic_types: semantic.functions()[1]
                                                        .abi()
                                                        .source_input_types(),
                                                    projections,
                                                    parameter_types: vec![shape.physical()],
                                                },
                                                &mut block.operations,
                                            )
                                            .map(|_| ())
                                    });
                                assert_eq!(
                                    parent.emission_work.as_deref().unwrap().storage(),
                                    storage
                                );
                                assert!(block.operations.is_empty());
                                assert_eq!(foreign.peak_storage(), 0);
                                assert_eq!(foreign_work.work(), 0);
                                return Err(rejected.expect_err("foreign scope token accepted"));
                            }
                            let next_value = ValueId(parent.next_value);
                            let prepared = parent.prepare_defined_call_arguments_v1(
                                SemanticBlockIdV1::from_index(0),
                                incoming.source(),
                                HELPER,
                                DefinedCallArgumentSignatureV1 {
                                    projection: DefinedCallProjectionV29::Execution(scope),
                                    semantic_types: semantic.functions()[1]
                                        .abi()
                                        .source_input_types(),
                                    projections,
                                    parameter_types: vec![shape.physical()],
                                },
                                &mut block.operations,
                            )?;
                            if shape.index() {
                                assert_eq!(block.operations.len(), 1);
                                assert_eq!(
                                    block.operations[0],
                                    Operation::effect_free(
                                        ValueDef::new(next_value, shape.physical()),
                                        OperationKind::Cast {
                                            kind: CastKind::Bitcast,
                                            value: ValueId(20),
                                            to: shape.physical(),
                                        },
                                    )
                                );
                                assert_ne!(next_value, ValueId(20));
                                assert_eq!(prepared.arguments, [next_value]);
                            } else {
                                assert!(block.operations.is_empty());
                                assert_eq!(prepared.arguments, [ValueId(20)]);
                            }
                            if matches!(fault, Fault::SecondInstance | Fault::DuplicateSeed) {
                                assert!(!shape.index());
                                let destination = incoming.source().destination().unwrap();
                                let prepared_destination = parent.prepare_call_destination_v1(
                                    SemanticBlockIdV1::from_index(0),
                                    destination.place(),
                                    &mut block.operations,
                                )?;
                                let results = parent.emit_results(
                                    &mut block.operations,
                                    vec![shape.physical()],
                                    OperationKind::Call {
                                        callee: plan.kernel_ir_function.clone(),
                                        arguments: prepared.arguments.clone(),
                                    },
                                )?;
                                let result_binding =
                                    binding_from_value_defs(semantic.types(), U32, &results)?;
                                parent.finish_call_destination_v1(
                                    SemanticBlockIdV1::from_index(0),
                                    destination.place(),
                                    prepared_destination,
                                    result_binding,
                                    None,
                                    &mut block.operations,
                                )?;
                                let arguments = parent.edge_arguments(
                                    SemanticBlockIdV1::from_index(0),
                                    0,
                                    SemanticBlockIdV1::from_index(1),
                                    &mut block.operations,
                                )?;
                                parent.with_emission_budget_v1(|this, budget| {
                                    this.execution.as_mut().unwrap().finish_block(budget)
                                })?;
                                let mut next_block = BasicBlock::new(BlockId(1));
                                parent.begin_block(
                                    SemanticBlockIdV1::from_index(1),
                                    &mut next_block,
                                )?;
                                let input = match parent.locals[2].as_ref().unwrap() {
                                    SemanticValueBindingV1::Aggregate(fields) => {
                                        fields[1].value().unwrap().0
                                    }
                                    _ => panic!("second capture lost aggregate shape"),
                                };
                                if let Some(slot) = next_block
                                    .parameters
                                    .iter()
                                    .position(|value| value.id == input)
                                {
                                    assert_eq!(arguments[slot], ValueId(21));
                                } else {
                                    assert_eq!(input, ValueId(21));
                                }
                                let second_prepared = parent.prepare_defined_call_arguments_v1(
                                    SemanticBlockIdV1::from_index(1),
                                    instances.incoming(second).unwrap().source(),
                                    HELPER,
                                    DefinedCallArgumentSignatureV1 {
                                        projection: DefinedCallProjectionV29::Execution(scope),
                                        semantic_types: semantic.functions()[1]
                                            .abi()
                                            .source_input_types(),
                                        projections,
                                        parameter_types: vec![shape.physical()],
                                    },
                                    &mut next_block.operations,
                                )?;
                                assert_eq!(second_prepared.arguments, [input]);
                                if matches!(fault, Fault::SecondInstance) {
                                    return Ok((second_prepared, None));
                                }
                                return Ok((prepared, Some(second_prepared)));
                            }
                            Ok((prepared, None))
                        },
                    )?;
                    let mut prepared = prepared;
                    match fault {
                        Fault::WrongType => {
                            prepared.execution.as_mut().unwrap().parameter_types[0] = Type::INDEX
                        }
                        Fault::WrongSource => {
                            prepared.execution.as_mut().unwrap().source.semantic[0] ^= 1
                        }
                        Fault::WrongSsa => {
                            prepared.execution.as_mut().unwrap().source.ssa =
                                self::owner(Shape::Struct).identity()
                        }
                        Fault::WrongRoot => {
                            prepared.execution.as_mut().unwrap().source.root = HELPER
                        }
                        Fault::WrongProjection => {
                            prepared.execution.as_mut().unwrap().projections[0].source_argument = 99
                        }
                        Fault::Alias => plan.parameter_values[0] = ValueId(90),
                        _ => {}
                    }
                    let selected = if matches!(fault, Fault::OtherInstance | Fault::SecondInstance)
                    {
                        second
                    } else {
                        first
                    };
                    if matches!(fault, Fault::ForeignBudget) {
                        let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(10_000_000);
                        let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, 10_000_000);
                        let error = match prepare_execution_parameters_v29(
                            instances,
                            selected,
                            prepared,
                            &plan,
                            &mut foreign,
                        ) {
                            Err(error) => error,
                            Ok(_) => panic!("foreign call-parameter ledger accepted"),
                        };
                        assert_eq!(foreign.peak_storage(), 0);
                        assert_eq!(foreign_work.work(), 12);
                        return Err(error);
                    }
                    let expected_arguments = prepared.arguments.clone();
                    let (arguments, parameters) = prepare_execution_parameters_v29(
                        instances, selected, prepared, &plan, budget,
                    )?;
                    assert_eq!(arguments, expected_arguments);
                    let duplicate = duplicate
                        .map(|prepared| {
                            prepare_execution_parameters_v29(
                                instances, second, prepared, &plan, budget,
                            )
                            .map(|(_, parameters)| parameters)
                        })
                        .transpose()?;
                    let selected =
                        if matches!(fault, Fault::ConstructorInstance | Fault::SecondInstance) {
                            second
                        } else {
                            first
                        };
                    with_execution_availability_v29(
                        instances,
                        selected,
                        budget,
                        |cursor, budget| {
                            let cursor = cursor.with_call_parameters_v29(parameters)?;
                            if let Some(duplicate) = duplicate {
                                return match cursor.with_call_parameters_v29(duplicate) {
                                    Err(error) => Err(error),
                                    Ok(_) => panic!("duplicate call parameters accepted"),
                                };
                            }
                            let mut private = PrivateArrayLazyBudgetV1::new(1, 1024);
                            lower_one_semantic_function_v1(
                                owner.source_semantic(),
                                &plan,
                                owner.plan_for_function(HELPER).unwrap(),
                                &BTreeMap::new(),
                                &BTreeMap::new(),
                                None,
                                BTreeSet::new(),
                                1,
                                false,
                                1024,
                                None,
                                &mut private,
                                None,
                                budget,
                                SemanticEmissionPlacementV1 {
                                    first_block: 17,
                                    first_value: 300,
                                },
                                Some(cursor),
                            )
                        },
                    )
                }),
            )
        })
        .map_err(|error| match error {
            production_call_instances_v1::ProductionCallInstanceErrorV1::Resource(error) => {
                error.into()
            }
            _ => execution_call_error_v29(),
        })?
    })();
    let peak = budget.peak_storage();
    (result, work.work(), peak)
}

#[test]
fn scoped_arguments_reach_shared_constructor_entry_archive_and_scalar_emission() {
    for shape in [
        Shape::RustCall,
        Shape::IndexRustCall,
        Shape::Tuple,
        Shape::Struct,
        Shape::IndexTuple,
        Shape::PackedRustCall,
    ] {
        let result = run(shape, Fault::None, 10_000_000, 10_000_000)
            .0
            .unwrap_or_else(|error| panic!("{shape:?}: {error:?}"));
        assert_eq!(result.function.signature.parameters, [shape.physical()]);
        assert_eq!(result.function.signature.results, [shape.physical()]);
        let observation = result.execution_observation.unwrap();
        let expected = SemanticExecutionBindingV29::context(
            owner(shape).source_semantic().types(),
            CONTEXT,
            ProductionCallOccurrenceV1 {
                caller: ProductionCallInstanceIdV1(0),
                block: SemanticBlockIdV1::from_index(0),
            },
            ValueId(90),
        )
        .unwrap();
        let destination = if shape.expanded() {
            4
        } else if shape.rust_call() {
            3
        } else {
            2
        };
        assert!(matches!(
            &observation.locals[destination],
            Some(SemanticValueBindingV1::Execution(actual)) if actual == &expected
        ));
        if shape.expanded() {
            assert!(matches!(
                observation.locals[2],
                None | Some(SemanticValueBindingV1::MovedExecution)
            ));
        } else {
            let local = if shape.rust_call() { 2 } else { 1 };
            assert!(matches!(&observation.locals[local],
                Some(SemanticValueBindingV1::Aggregate(fields))
                    if matches!(fields[0], SemanticValueBindingV1::MovedExecution)));
        }
        let blocks = &result.function.body.as_ref().unwrap().blocks;
        let constants: Vec<_> = blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| {
                matches!(
                    operation.kind,
                    OperationKind::Constant(Constant::U32(7) | Constant::U64(7))
                )
            })
            .collect();
        assert_eq!(constants.len(), 1);
        assert_eq!(
            constants[0].kind,
            OperationKind::Constant(if shape.index() {
                Constant::U64(7)
            } else {
                Constant::U32(7)
            })
        );
        assert_eq!(constants[0].results.len(), 1);
        assert_eq!(constants[0].results[0].ty, shape.physical());
        let binaries: Vec<_> = blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| matches!(operation.kind, OperationKind::Binary { .. }))
            .collect();
        assert_eq!(binaries.len(), 1);
        assert_eq!(binaries[0].results.len(), 1);
        assert_eq!(binaries[0].results[0].ty, shape.physical());
        assert_eq!(
            binaries[0].kind,
            OperationKind::Binary {
                op: BinaryOp::BitXor,
                lhs: ValueId(300),
                rhs: constants[0].results[0].id,
            }
        );
        assert!(blocks.iter().any(|block| block.terminator
            == Some(Terminator::Return {
                values: vec![binaries[0].results[0].id],
            })));
    }
}

#[test]
fn scoped_call_rejects_instance_identity_type_projection_and_tombstone_substitution() {
    for fault in [
        Fault::OtherInstance,
        Fault::WrongType,
        Fault::WrongSource,
        Fault::WrongSsa,
        Fault::WrongRoot,
        Fault::WrongProjection,
        Fault::Moved,
        Fault::Alias,
        Fault::ConstructorInstance,
        Fault::ForeignBudget,
        Fault::ForeignScope,
    ] {
        assert!(
            run(Shape::RustCall, fault, 10_000_000, 10_000_000)
                .0
                .is_err()
        );
    }
}

#[test]
fn scoped_call_obeys_exact_and_one_short_shared_work_budget() {
    let (result, exact, _) = run(Shape::RustCall, Fault::None, 10_000_000, 10_000_000);
    result.unwrap();
    assert!(
        run(Shape::RustCall, Fault::None, exact, 10_000_000)
            .0
            .is_ok()
    );
    assert!(
        run(Shape::RustCall, Fault::None, exact - 1, 10_000_000)
            .0
            .is_err()
    );
}

#[test]
fn scoped_call_normalizes_index_to_u64_once_before_callee_rebinding() {
    run(Shape::IndexRustCall, Fault::None, 10_000_000, 10_000_000)
        .0
        .unwrap();
}

#[test]
fn scoped_call_preserves_second_instance_capture_across_the_real_cfg_edge() {
    let result = run(
        Shape::RustCall,
        Fault::SecondInstance,
        10_000_000,
        10_000_000,
    )
    .0
    .unwrap();
    let observation = result.execution_observation.unwrap();
    let expected = SemanticExecutionBindingV29::context(
        owner(Shape::RustCall).source_semantic().types(),
        CONTEXT,
        ProductionCallOccurrenceV1 {
            caller: ProductionCallInstanceIdV1(0),
            block: SemanticBlockIdV1::from_index(0),
        },
        ValueId(91),
    )
    .unwrap();
    assert!(matches!(&observation.locals[4],
        Some(SemanticValueBindingV1::Execution(actual)) if actual == &expected));
}

#[test]
fn scoped_call_rejects_replacing_an_already_attached_parameter_seed() {
    assert!(matches!(
        run(
            Shape::RustCall,
            Fault::DuplicateSeed,
            10_000_000,
            10_000_000
        )
        .0,
        Err(ProductionSemanticKirErrorV1::Unsupported {
            detail: "execution call parameters are already attached",
            ..
        })
    ));
}

#[test]
fn scoped_call_obeys_exact_and_one_short_storage_with_existing_caller_floor() {
    let (result, _, peak) = run(Shape::RustCall, Fault::None, 10_000_000, 10_000_000);
    result.unwrap();
    assert!(peak > FLOOR);
    assert!(
        run(Shape::RustCall, Fault::None, 10_000_000, peak)
            .0
            .is_ok()
    );
    assert!(matches!(
        run(Shape::RustCall, Fault::None, 10_000_000, peak - 1).0,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage(_)
            )
        )
    ));
}
