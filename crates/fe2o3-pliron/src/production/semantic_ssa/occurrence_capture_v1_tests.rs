use super::*;
#[path = "typed_elision_capture_v1_tests.rs"]
mod typed_elision_capture_v1_tests;

use crate::{
    ProductionSemanticSsaEntryOriginV1 as EntryOrigin,
    ProductionSemanticSsaEventRoleV1 as EventRole,
    ProductionSemanticSsaOccurrenceErrorV1 as CaptureError,
    ProductionSemanticSsaOccurrenceSiteV1 as Site, ProductionSemanticSsaOperandRoleV1 as Operand,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAbiArgumentV1, SemanticAggregateKindV1, SemanticAssignmentV1,
    SemanticBackendPrimitiveV1, SemanticBackendReprV1, SemanticBackendScalarV1, SemanticConstantV1,
    SemanticConstantValueV1, SemanticFieldsShapeV1, SemanticRustcVariantsV1, SemanticScalarTypeV1,
    SemanticScalarValidityRangeV1, SemanticScalarValueV1, SemanticTypeLayoutDetailsV1,
};
use fe2o3_mir_model::{SsaDefinitionIdV1, SsaEdgeIdV1, SsaResolvedEventV1, SsaValueV1};

fn variable(index: u32) -> SsaVariableIdV1 {
    SsaVariableIdV1::new(index)
}

fn definition(index: u32) -> SsaValueV1 {
    SsaValueV1::Definition(SsaDefinitionIdV1::new(index))
}

fn statement(block: u32, statement: u32) -> Site {
    Site::Statement {
        block: SsaBlockIdV1::new(block),
        statement,
    }
}

fn terminator(block: u32) -> Site {
    Site::Terminator {
        block: SsaBlockIdV1::new(block),
    }
}

fn number(value: u64) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        SemanticTypeIdV1::from_index(1),
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value.into(), 8).unwrap()),
    ))
}

fn unit() -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        SemanticTypeIdV1::from_index(0),
        SemanticConstantValueV1::ZeroSized,
    ))
}

fn scalar_types(base: &AdmittedInertSemanticMirV1) -> Vec<SemanticTypeDeclV1> {
    let mut types = base.types().to_vec();
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(test_bytes(160)),
        SemanticLayoutIdentityV1::from_sha256(test_bytes(161)),
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
    ));
    types
}

fn root_with(
    original: &SemanticFunctionDeclV1,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        locals,
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone())
}

fn admit_owner(request: InertSemanticMirRequestV1) -> ProductionSemanticSsaOwnerV1 {
    let admitted = request
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
    let source =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(source, ProductionSemanticSsaLimitsV1::default()).unwrap()
}

fn indexed() -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(2)),
                SemanticTypeIdV1::from_index(1),
            )
            .unwrap(),
        ],
        SemanticTypeIdV1::from_index(1),
    )
    .unwrap()
}

fn sparse_owner() -> ProductionSemanticSsaOwnerV1 {
    let base = admitted_single_function_semantic();
    let mut types = scalar_types(&base);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(test_bytes(162)),
        SemanticLayoutIdentityV1::from_sha256(test_bytes(163)),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            16,
            8,
            SemanticFieldsShapeV1::array(8, 2),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            8,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Array {
            element: SemanticTypeIdV1::from_index(1),
            length: 2,
        },
    ));
    let array = SemanticStatementV1::new(
        base.functions()[0].source(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            test_typed_place(1, 2),
            SemanticRvalueV1::new(
                SemanticTypeIdV1::from_index(2),
                SemanticRvalueKindV1::aggregate(
                    SemanticAggregateKindV1::Array,
                    vec![number(10), number(11)],
                )
                .unwrap(),
            ),
        )),
    );
    let root = root_with(
        &base.functions()[0],
        vec![
            test_local(170, 0, SemanticLocalRoleV1::Return),
            test_local(171, 2, SemanticLocalRoleV1::Temporary),
            test_local(172, 1, SemanticLocalRoleV1::Temporary),
            test_local(173, 1, SemanticLocalRoleV1::Temporary),
            test_local(174, 1, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            test_block(
                180,
                vec![
                    array,
                    test_assign_to(test_typed_place(2, 1), number(0)),
                    test_assign_to(indexed(), number(12)),
                    test_assign_to(test_typed_place(3, 1), SemanticOperandV1::Copy(indexed())),
                    test_assign_to(
                        test_typed_place(4, 1),
                        SemanticOperandV1::Move(test_typed_place(2, 1)),
                    ),
                    test_storage_dead(3),
                ],
                SemanticTerminatorKindV1::Goto(test_edge(SemanticEdgeRoleV1::Goto, 2)),
            ),
            // Unreachable uses are retained as source occurrences, not resolved proof.
            test_block(
                181,
                vec![test_assign_to(
                    test_typed_place(3, 1),
                    SemanticOperandV1::Copy(test_typed_place(2, 1)),
                )],
                SemanticTerminatorKindV1::Goto(test_edge(SemanticEdgeRoleV1::Goto, 2)),
            ),
            test_block(182, vec![], SemanticTerminatorKindV1::Return),
        ],
    );
    admit_owner(
        InertSemanticMirRequestV1::new(
            base.target(),
            types,
            vec![],
            vec![],
            vec![],
            vec![root],
            vec![SemanticFunctionIdV1::from_index(0)],
        )
        .unwrap(),
    )
}

fn edge_owner() -> ProductionSemanticSsaOwnerV1 {
    let base = admitted_single_function_semantic();
    let original = &base.functions()[0];
    let unit_type = SemanticTypeIdV1::from_index(0);
    let call = |arguments| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(1),
                arguments,
                Some(SemanticCallDestinationV1::new(
                    test_typed_place(2, 0),
                    test_edge(SemanticEdgeRoleV1::CallReturn, 2),
                )),
                SemanticUnwindActionV1::Cleanup(test_edge(SemanticEdgeRoleV1::CallUnwind, 2)),
            )
            .unwrap(),
        )
    };
    let root = root_with(
        original,
        vec![
            test_local(170, 0, SemanticLocalRoleV1::Return),
            test_local(171, 1, SemanticLocalRoleV1::Temporary),
            test_local(172, 0, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            test_block(
                180,
                vec![
                    test_assign_to(test_typed_place(1, 1), number(0)),
                    test_assign_to(test_typed_place(2, 0), unit()),
                ],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(test_typed_place(1, 1)),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![
                            SemanticSwitchTargetV1::new(
                                0,
                                test_edge(SemanticEdgeRoleV1::SwitchValue, 1),
                            ),
                            SemanticSwitchTargetV1::new(
                                1,
                                test_edge(SemanticEdgeRoleV1::SwitchValue, 1),
                            ),
                        ],
                        test_edge(SemanticEdgeRoleV1::SwitchOtherwise, 1),
                    )
                    .unwrap(),
                },
            ),
            test_block(
                181,
                vec![],
                call(vec![
                    SemanticOperandV1::Copy(test_typed_place(2, 0)),
                    SemanticOperandV1::Copy(test_typed_place(2, 0)),
                    unit(),
                ]),
            ),
            test_block(182, vec![], SemanticTerminatorKindV1::Return),
            test_block(183, vec![], call(vec![unit(), unit(), unit()])),
        ],
    );
    let helper_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(test_bytes(210)),
        SemanticLayoutIdentityV1::from_sha256(test_bytes(211)),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        3,
        vec![
            SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                unit_type,
                SemanticAbiPassModeV1::Ignore
            ));
            3
        ],
        SemanticAbiValueV1::new(unit_type, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap();
    let helper = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(test_bytes(220)),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256(test_bytes(221)),
        SemanticMonomorphizationIdentityV1::from_sha256(test_bytes(222)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(test_bytes(223)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(test_bytes(224)),
        original.source(),
        helper_abi,
        vec![
            test_local(225, 0, SemanticLocalRoleV1::Return),
            test_local(226, 0, SemanticLocalRoleV1::Argument(0)),
            test_local(227, 0, SemanticLocalRoleV1::Argument(1)),
            test_local(228, 0, SemanticLocalRoleV1::Argument(2)),
        ],
        SemanticBlockIdV1::from_index(0),
        vec![test_block(229, vec![], SemanticTerminatorKindV1::Return)],
    )
    .unwrap();
    admit_owner(
        InertSemanticMirRequestV1::new_with_callables(
            base.target(),
            scalar_types(&base),
            vec![],
            vec![],
            vec![],
            vec![root, helper],
            vec![
                SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
                SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
            ],
            vec![SemanticFunctionIdV1::from_index(0)],
        )
        .unwrap(),
    )
}

fn with_capture(
    owner: ProductionSemanticSsaOwnerV1,
    check: impl FnOnce(&ProductionSemanticSsaOwnerV1),
) {
    let mut owner = owner;
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    assert!(owner.occurrences_v1().is_none());
    let storage = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    assert_eq!(budget.storage(), 0);
    budget.reserve_storage(storage.retained_storage()).unwrap();
    check(&owner);
    assert_eq!(owner.occurrence_storage(), Some(storage));
    assert!(!owner.grants_proof_or_artifact_authority());
    drop(owner);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn sealed_capture_preserves_rust_call_field_origins_in_local_order() {
    for count in [0, 3] {
        let base = admitted_single_function_semantic();
        let unit = SemanticTypeIdV1::from_index(0);
        let tuple = SemanticTypeIdV1::from_index(1);
        let value = || SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore);
        let mut arguments = vec![SemanticAbiArgumentV1::source(value())];
        arguments.extend(
            (0..count).map(|field| SemanticAbiArgumentV1::rust_call_tuple_field(field, value())),
        );
        let abi = SemanticFunctionAbiV1::from_rustc_with_source_signature(
            SemanticAbiIdentityV1::from_sha256(test_bytes(210)),
            base.functions()[0].abi().layout_identity(),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::RustCall,
            false,
            false,
            1,
            vec![unit, tuple],
            unit,
            arguments,
            value(),
        )
        .unwrap();
        let mut locals = vec![
            test_local(220, 0, SemanticLocalRoleV1::Return),
            test_local(221, 0, SemanticLocalRoleV1::Argument(0)),
        ];
        let fields = if count == 0 { vec![] } else { vec![2, 0, 1] };
        locals.extend(fields.iter().enumerate().map(|(local, field)| {
            test_local(
                222 + local as u8,
                0,
                SemanticLocalRoleV1::RustCallTupleField {
                    argument: 1,
                    field: *field,
                },
            )
        }));
        let helper = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256(test_bytes(230)),
            SemanticFunctionRoleV1::InternalHelper,
            SemanticItemDefinitionIdentityV1::from_sha256(test_bytes(231)),
            SemanticMonomorphizationIdentityV1::from_sha256(test_bytes(232)),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256(test_bytes(233)),
            SemanticConstGenericArgumentsIdentityV1::from_sha256(test_bytes(234)),
            base.functions()[0].source(),
            abi,
            locals,
            SemanticBlockIdV1::from_index(0),
            vec![test_block(235, vec![], SemanticTerminatorKindV1::Return)],
        )
        .unwrap();
        let mut types = base.types().to_vec();
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(test_bytes(236)),
            SemanticLayoutIdentityV1::from_sha256(test_bytes(237)),
            SemanticTypeLayoutV1::aggregate(
                Some(0),
                1,
                fe2o3_mir_model::semantic_mir_v1::SemanticAggregateLayoutV1::new(
                    vec![0; count as usize],
                    vec![],
                )
                .unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(
                fe2o3_mir_model::semantic_mir_v1::SemanticAggregateTypeV1::new(vec![
                    unit;
                    count as usize
                ])
                .unwrap(),
            ),
        ));
        let root = root_with(
            &base.functions()[0],
            vec![
                test_local(238, 0, SemanticLocalRoleV1::Return),
                test_local(239, 0, SemanticLocalRoleV1::Temporary),
            ],
            vec![
                test_block(
                    240,
                    vec![],
                    SemanticTerminatorKindV1::Call(
                        SemanticDirectCallV1::new_callable(
                            SemanticCallableIdV1::from_index(1),
                            [unit, tuple]
                                .into_iter()
                                .map(|ty| {
                                    SemanticOperandV1::Constant(SemanticConstantV1::new(
                                        ty,
                                        SemanticConstantValueV1::ZeroSized,
                                    ))
                                })
                                .collect(),
                            Some(SemanticCallDestinationV1::new(
                                test_typed_place(1, 0),
                                test_edge(SemanticEdgeRoleV1::CallReturn, 1),
                            )),
                            SemanticUnwindActionV1::Unreachable,
                        )
                        .unwrap(),
                    ),
                ),
                test_block(241, vec![], SemanticTerminatorKindV1::Return),
            ],
        );
        let owner = admit_owner(
            InertSemanticMirRequestV1::new_with_callables(
                base.target(),
                types,
                vec![],
                vec![],
                vec![],
                vec![root, helper],
                vec![
                    SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
                    SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
                ],
                vec![SemanticFunctionIdV1::from_index(0)],
            )
            .unwrap(),
        );
        with_capture(owner, |owner| {
            let view = owner.occurrences_v1().unwrap();
            let rows = view.function(SemanticFunctionIdV1::from_index(1)).unwrap();
            assert_eq!(rows.entry_definitions().len(), count as usize + 1);
            for (index, row) in rows.entry_definitions().iter().enumerate() {
                assert_eq!(row.ordinal(), index as u32);
                assert_eq!(row.variable(), variable(index as u32 + 1));
                assert_eq!(
                    row.origin(),
                    if index == 0 {
                        EntryOrigin::Argument(0)
                    } else {
                        EntryOrigin::RustCallTupleField {
                            argument: 1,
                            field: fields[index - 1],
                        }
                    }
                );
            }
            assert!(rows.events().is_empty());
        });
    }
}

#[test]
fn sealed_capture_keeps_an_actual_eventless_function() {
    let source = ProductionSemanticMirOwnerV1::try_new(
        admitted_single_function_semantic(),
        ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    let owner =
        ProductionSemanticSsaOwnerV1::try_new(source, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    with_capture(owner, |owner| {
        let view = owner.occurrences_v1().unwrap();
        assert_eq!(view.function_count(), 1);
        let rows = view.function(SemanticFunctionIdV1::from_index(0)).unwrap();
        assert_eq!(owner.summary().input_blocks(), 1);
        assert!(rows.events().is_empty());
        assert!(rows.constants().is_empty());
        assert!(rows.successors().is_empty());
        assert!(rows.entry_definitions().is_empty());
        assert!(rows.edge_definitions().is_empty());
        assert!(rows.elisions().is_empty());
    });
}

#[test]
fn sealed_capture_keeps_sparse_original_ordinals_and_unreachable_move_uses() {
    with_capture(sparse_owner(), |owner| {
        let view = owner.occurrences_v1().unwrap();
        assert_eq!(view.function_count(), 1);
        assert!(view.function(SemanticFunctionIdV1::from_index(1)).is_none());
        let rows = view.function(SemanticFunctionIdV1::from_index(0)).unwrap();
        assert!(std::ptr::eq(rows.owner(), owner));
        assert_eq!(rows.function(), SemanticFunctionIdV1::from_index(0));
        assert_eq!(owner.summary().input_events(), 13);
        assert_eq!(rows.events().len(), 13);
        let original = [
            (
                0,
                0,
                0,
                Operand::Destination,
                EventRole::DestinationDefine,
                SsaEventV1::Define(variable(1)),
            ),
            (
                0,
                1,
                1,
                Operand::Destination,
                EventRole::DestinationDefine,
                SsaEventV1::Define(variable(2)),
            ),
            (
                0,
                2,
                2,
                Operand::Destination,
                EventRole::BaseUse,
                SsaEventV1::Use(variable(1)),
            ),
            (
                0,
                2,
                3,
                Operand::Destination,
                EventRole::ProjectionIndexUse(0),
                SsaEventV1::Use(variable(2)),
            ),
            (
                0,
                3,
                4,
                Operand::RvalueOperand(0),
                EventRole::BaseUse,
                SsaEventV1::Use(variable(1)),
            ),
            (
                0,
                3,
                5,
                Operand::RvalueOperand(0),
                EventRole::ProjectionIndexUse(0),
                SsaEventV1::Use(variable(2)),
            ),
            (
                0,
                3,
                6,
                Operand::Destination,
                EventRole::DestinationDefine,
                SsaEventV1::Define(variable(3)),
            ),
            (
                0,
                4,
                7,
                Operand::RvalueOperand(0),
                EventRole::BaseUse,
                SsaEventV1::Use(variable(2)),
            ),
            (
                0,
                4,
                8,
                Operand::RvalueOperand(0),
                EventRole::MoveKill,
                SsaEventV1::Kill(variable(2)),
            ),
            (
                0,
                4,
                9,
                Operand::Destination,
                EventRole::DestinationDefine,
                SsaEventV1::Define(variable(4)),
            ),
            (
                0,
                5,
                10,
                Operand::StorageDead,
                EventRole::StorageKill,
                SsaEventV1::Kill(variable(3)),
            ),
            (
                1,
                0,
                0,
                Operand::RvalueOperand(0),
                EventRole::BaseUse,
                SsaEventV1::Use(variable(2)),
            ),
            (
                1,
                0,
                1,
                Operand::Destination,
                EventRole::DestinationDefine,
                SsaEventV1::Define(variable(3)),
            ),
        ];
        for (row, (block, source, ordinal, operand, role, event)) in
            rows.events().iter().zip(original)
        {
            assert_eq!(
                (
                    row.site(),
                    row.ordinal(),
                    row.operand(),
                    row.role(),
                    row.event()
                ),
                (statement(block, source), ordinal, operand, role, event)
            );
            assert_eq!(row.is_reachable(), block == 0);
            assert_eq!(row.is_promoted(), event.variable() != variable(1));
        }
        let resolved = rows
            .events()
            .iter()
            .filter_map(|row| row.resolved().map(|value| (row.ordinal(), value)))
            .collect::<Vec<_>>();
        assert_eq!(
            resolved,
            vec![
                (
                    1,
                    SsaResolvedEventV1::Define {
                        variable: variable(2),
                        value: definition(0)
                    }
                ),
                (
                    3,
                    SsaResolvedEventV1::Use {
                        variable: variable(2),
                        value: definition(0)
                    }
                ),
                (
                    5,
                    SsaResolvedEventV1::Use {
                        variable: variable(2),
                        value: definition(0)
                    }
                ),
                (
                    6,
                    SsaResolvedEventV1::Define {
                        variable: variable(3),
                        value: definition(1)
                    }
                ),
                (
                    7,
                    SsaResolvedEventV1::Use {
                        variable: variable(2),
                        value: definition(0)
                    }
                ),
                (
                    8,
                    SsaResolvedEventV1::Kill {
                        variable: variable(2),
                        previous: Some(definition(0))
                    }
                ),
                (
                    9,
                    SsaResolvedEventV1::Define {
                        variable: variable(4),
                        value: definition(2)
                    }
                ),
                (
                    10,
                    SsaResolvedEventV1::Kill {
                        variable: variable(3),
                        previous: Some(definition(1))
                    }
                ),
            ]
        );
        assert_eq!(
            rows.constants()
                .iter()
                .map(|row| (row.site(), row.operand(), row.next_event(), row.ty()))
                .collect::<Vec<_>>(),
            vec![
                (
                    statement(0, 0),
                    Operand::RvalueOperand(0),
                    0,
                    SemanticTypeIdV1::from_index(1)
                ),
                (
                    statement(0, 0),
                    Operand::RvalueOperand(1),
                    0,
                    SemanticTypeIdV1::from_index(1)
                ),
                (
                    statement(0, 1),
                    Operand::RvalueOperand(0),
                    1,
                    SemanticTypeIdV1::from_index(1)
                ),
                (
                    statement(0, 2),
                    Operand::RvalueOperand(0),
                    2,
                    SemanticTypeIdV1::from_index(1)
                ),
            ]
        );
        assert!(rows.entry_definitions().is_empty());
        assert!(rows.edge_definitions().is_empty());
        assert!(rows.elisions().is_empty());
        assert_eq!(
            rows.successors()
                .iter()
                .map(|row| (row.id(), row.edge()))
                .collect::<Vec<_>>(),
            vec![
                (
                    SsaEdgeIdV1::new(SsaBlockIdV1::new(0), 0),
                    test_edge(SemanticEdgeRoleV1::Goto, 2)
                ),
                (
                    SsaEdgeIdV1::new(SsaBlockIdV1::new(1), 0),
                    test_edge(SemanticEdgeRoleV1::Goto, 2)
                ),
            ]
        );
    });
}

#[test]
fn sealed_capture_preserves_duplicate_targets_return_edges_and_repeated_arguments() {
    with_capture(edge_owner(), |owner| {
        let view = owner.occurrences_v1().unwrap();
        assert_eq!(view.function_count(), 2);
        let root = view.function(SemanticFunctionIdV1::from_index(0)).unwrap();
        assert_eq!(root.successors().len(), 7);
        let expected = [
            (0, 0, SemanticEdgeRoleV1::SwitchValue, 1),
            (0, 1, SemanticEdgeRoleV1::SwitchValue, 1),
            (0, 2, SemanticEdgeRoleV1::SwitchOtherwise, 1),
            (1, 0, SemanticEdgeRoleV1::CallReturn, 2),
            (1, 1, SemanticEdgeRoleV1::CallUnwind, 2),
            (3, 0, SemanticEdgeRoleV1::CallReturn, 2),
            (3, 1, SemanticEdgeRoleV1::CallUnwind, 2),
        ];
        for (row, (block, ordinal, role, target)) in root.successors().iter().zip(expected) {
            assert_eq!(
                row.id(),
                SsaEdgeIdV1::new(SsaBlockIdV1::new(block), ordinal)
            );
            assert_eq!(row.edge(), test_edge(role, target));
        }
        assert_eq!(root.edge_definitions().len(), 2);
        for (row, (block, reachable, value)) in root
            .edge_definitions()
            .iter()
            .zip([(1, true, Some(definition(2))), (3, false, None)])
        {
            assert_eq!(row.edge(), SsaEdgeIdV1::new(SsaBlockIdV1::new(block), 0));
            assert_eq!(row.ordinal(), 0);
            assert_eq!(row.variable(), variable(2));
            assert_eq!(row.is_reachable(), reachable);
            assert!(row.is_promoted());
            assert_eq!(row.value(), value);
        }
        assert_eq!(root.events().len(), 5);
        for (argument, row) in root.events()[3..].iter().enumerate() {
            assert_eq!(
                (row.site(), row.operand(), row.ordinal(), row.role()),
                (
                    terminator(1),
                    Operand::CallArgument(argument as u32),
                    argument as u32,
                    EventRole::BaseUse
                )
            );
            assert_eq!(
                row.resolved(),
                Some(SsaResolvedEventV1::Use {
                    variable: variable(2),
                    value: definition(1)
                })
            );
        }
        assert_eq!(root.constants().len(), 6);
        assert_eq!(
            (
                root.constants()[2].site(),
                root.constants()[2].operand(),
                root.constants()[2].next_event()
            ),
            (terminator(1), Operand::CallArgument(2), 2)
        );
        for (argument, row) in root.constants()[3..].iter().enumerate() {
            assert_eq!(
                (row.site(), row.operand(), row.next_event()),
                (terminator(3), Operand::CallArgument(argument as u32), 0)
            );
        }
        let helper = view.function(SemanticFunctionIdV1::from_index(1)).unwrap();
        assert!(std::ptr::eq(helper.owner(), owner));
        assert!(helper.events().is_empty());
        assert!(helper.constants().is_empty());
        assert_eq!(helper.entry_definitions().len(), 3);
        for (argument, row) in helper.entry_definitions().iter().enumerate() {
            assert_eq!(row.ordinal(), argument as u32);
            assert_eq!(row.variable(), variable(argument as u32 + 1));
            assert_eq!(row.origin(), EntryOrigin::Argument(argument as u32));
            assert_eq!(row.value(), Some(definition(argument as u32)));
        }
    });
}

#[test]
fn sealed_capture_rejects_full_plan_changes_with_stored_hashes_unchanged() {
    for change in 0..9 {
        let mut owner = edge_owner();
        let original_identity = owner.identity;
        let original_plan_identity = owner.plans[1].plan.identity();
        match change {
            0 => owner.plans[1].function = SemanticFunctionIdV1::from_index(0),
            1 => {
                owner.plans[1].function_identity =
                    SemanticFunctionIdentityV1::from_sha256(test_bytes(230))
            }
            2 => owner.plans[1].partial_moves.projected_moves += 1,
            3 => owner.plans[1].partial_moves.state_entries += 1,
            4 => owner.plans[1].partial_moves.work_units += 1,
            5 => owner.plans[1].auxiliary_resources.storage_words += 1,
            6 => owner.plans[1].auxiliary_resources.work_units += 1,
            7 => owner.plans[1].implicit_entry_variables = Box::new([variable(0)]),
            8 => owner.plans[1].retained_cross_edge_variables = Box::new([variable(0)]),
            _ => unreachable!(),
        }
        assert_eq!(owner.identity, original_identity);
        assert_eq!(owner.plans[1].plan.identity(), original_plan_identity);
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        let error = owner
            .try_capture_occurrences_with_budget_v1(&mut budget)
            .unwrap_err();
        assert!(
            matches!(
                error,
                CaptureError::Replay(ProductionSemanticSsaErrorV1::ReplayMismatch)
            ),
            "change {change}: {error:?}"
        );
        assert!(owner.occurrences_v1().is_none());
        assert!(owner.occurrence_storage().is_none());
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn sealed_capture_rejects_plan_roster_summary_and_source_identity_changes() {
    for change in 0..7 {
        let mut owner = edge_owner();
        match change {
            0 => owner.plans = Box::new([]),
            1 => owner.plans = vec![owner.plans[0].clone()].into_boxed_slice(),
            2 => owner.plans.swap(0, 1),
            3 => owner.summary.input_blocks += 1,
            4 => owner.identity.0[0] ^= 1,
            5 => owner.source_semantic_sha256[0] ^= 1,
            6 => owner.plans[0].plan = owner.plans[1].plan.clone(),
            _ => unreachable!(),
        }
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        let error = owner
            .try_capture_occurrences_with_budget_v1(&mut budget)
            .unwrap_err();
        assert!(
            matches!(
                error,
                CaptureError::Replay(ProductionSemanticSsaErrorV1::ReplayMismatch)
            ),
            "change {change}: {error:?}"
        );
        assert!(owner.occurrences_v1().is_none());
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn sealed_capture_moves_with_owner_without_rebinding_or_replay_mutation() {
    let mut owner = sparse_owner();
    let source = owner.source_semantic().functions().as_ptr();
    let plans = owner.plans.as_ptr();
    let identity = owner.identity();
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let storage = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let events = owner
        .occurrences_v1()
        .unwrap()
        .function(SemanticFunctionIdV1::from_index(0))
        .unwrap()
        .events()
        .as_ptr();
    // Moving the inline owner is permitted; only its retained allocations are stable.
    let moved = Box::new(owner);
    assert_eq!(moved.source_semantic().functions().as_ptr(), source);
    assert_eq!(moved.plans.as_ptr(), plans);
    assert_eq!(moved.identity(), identity);
    let view = moved.occurrences_v1().unwrap();
    let rows = view.function(SemanticFunctionIdV1::from_index(0)).unwrap();
    assert!(std::ptr::eq(rows.owner(), moved.as_ref()));
    assert_eq!(rows.events().as_ptr(), events);
    moved.verify_replay().unwrap();
    assert_eq!(rows.events().as_ptr(), events);
    assert_eq!(
        rows.events()[8].resolved(),
        Some(SsaResolvedEventV1::Kill {
            variable: variable(2),
            previous: Some(definition(0))
        })
    );
    drop(moved);
    budget.release_storage(storage.retained_storage()).unwrap();
}

#[path = "occurrence_payload_resource_v1_tests.rs"]
mod occurrence_payload_resource_v1_tests;
