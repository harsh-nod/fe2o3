// Included in checked_output_session_tests: every positive uses fresh MIR
// admission, real materialization, the AMD binder and the checked optimizer.
use fe2o3_kernel_ir::{
    CanonicalKirDefinitionCoordinateV1 as Definition, CanonicalKirUseCoordinateV1 as UseCoordinate,
};
use fe2o3_lower_mir_kernel::{
    ProductionSourceOutputGlobalAccessV1 as GlobalAccess,
    ProductionSourceOutputGlobalUnsupportedV1 as GlobalUnsupported,
    ProductionSourceOutputOccurrencesV1,
};

const G_POINTER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(7);

#[derive(Clone, Copy)]
enum GlobalSourceShape {
    Straight,
    Merged,
    Omitted,
    MixedPrivate,
}

fn global_pointer_type() -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(237)),
        SemanticLayoutIdentityV1::from_sha256(bytes(237)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(1, 8, 8),
                SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                A_U32,
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Mutable,
                1,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap()),
            None,
        ),
    )
}

fn global_place() -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, A_U32).unwrap()],
        A_U32,
    )
    .unwrap()
}

fn genuine_global_source(shape: GlobalSourceShape) -> ProductionPreRankedKirOwnerV1 {
    let load = SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
        global_place(),
        SemanticVolatilityV1::NonVolatile,
        None,
    ));
    let statements = vec![
        typed_assignment(3, A_U32, load.clone()),
        statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            global_place(),
            typed_operand(3, A_U32),
            SemanticVolatilityV1::NonVolatile,
            None,
        ))),
    ];
    let blocks = match shape {
        GlobalSourceShape::Straight => {
            vec![block(201, statements, SemanticTerminatorKindV1::Return)]
        }
        GlobalSourceShape::Merged | GlobalSourceShape::Omitted => vec![
            block(
                201,
                Vec::new(),
                assertion_terminator(
                    typed_constant(
                        A_BOOL,
                        u128::from(matches!(shape, GlobalSourceShape::Merged)),
                        1,
                    ),
                    true,
                    1,
                ),
            ),
            block(202, statements, SemanticTerminatorKindV1::Return),
        ],
        GlobalSourceShape::MixedPrivate => vec![block(
            201,
            vec![
                typed_assignment(
                    4,
                    A_U32,
                    SemanticRvalueKindV1::Use(typed_constant(A_U32, 0, 4)),
                ),
                statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    SemanticPlaceV1::new(
                        SemanticLocalIdV1::from_index(5),
                        vec![
                            SemanticProjectionV1::new(
                                SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(4)),
                                A_U32,
                            )
                            .unwrap(),
                        ],
                        A_U32,
                    )
                    .unwrap(),
                    SemanticRvalueV1::new(A_U32, load),
                ))),
            ],
            SemanticTerminatorKindV1::Return,
        )],
    };
    let function = assertion_root_with_access(
        vec![
            (A_UNIT, SemanticLocalRoleV1::Return),
            (G_POINTER, SemanticLocalRoleV1::Argument(0)),
            (G_POINTER, SemanticLocalRoleV1::Argument(1)),
            (A_U32, SemanticLocalRoleV1::Temporary),
            (A_U32, SemanticLocalRoleV1::Temporary),
            (A_ARRAY, SemanticLocalRoleV1::Temporary),
        ],
        vec![G_POINTER, G_POINTER],
        blocks,
        false,
    );
    let mut types = assertion_types();
    types.push(global_pointer_type());
    assertion_materialized_functions(types, vec![function])
}

fn genuine_global_call_source() -> ProductionPreRankedKirOwnerV1 {
    let root = assertion_root_with_access(
        vec![
            (A_UNIT, SemanticLocalRoleV1::Return),
            (G_POINTER, SemanticLocalRoleV1::Argument(0)),
            (A_U32, SemanticLocalRoleV1::Temporary),
        ],
        vec![G_POINTER],
        vec![
            block(
                201,
                vec![],
                neutral_test_call_v1(1, vec![typed_constant(A_U32, 7, 4)], 2, A_U32, 1),
            ),
            block(
                202,
                vec![statement(SemanticStatementKindV1::Store(
                    SemanticMemoryStoreV1::new(
                        global_place(),
                        typed_operand(2, A_U32),
                        SemanticVolatilityV1::NonVolatile,
                        None,
                    ),
                ))],
                SemanticTerminatorKindV1::Return,
            ),
        ],
        false,
    );
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(243)),
        SemanticLayoutIdentityV1::from_sha256(bytes(250)),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(
            neutral_plain_direct_abi_value_v1(A_U32),
        )],
        neutral_plain_direct_abi_value_v1(A_U32),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
    .unwrap();
    let helper = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(244)),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(245)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(246)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(248)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(249)),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        vec![
            local(221, A_U32, SemanticLocalRoleV1::Return),
            local(222, A_U32, SemanticLocalRoleV1::Argument(0)),
        ],
        SemanticBlockIdV1::from_index(0),
        vec![block(
            223,
            vec![typed_assignment(
                0,
                A_U32,
                SemanticRvalueKindV1::Use(typed_operand(1, A_U32)),
            )],
            SemanticTerminatorKindV1::Return,
        )],
    )
    .unwrap();
    let mut types = assertion_types();
    types.push(global_pointer_type());
    assertion_materialized_functions(types, vec![root, helper])
}

fn with_global_view(
    source: &ProductionPreRankedKirOwnerV1,
    profile: Profile,
    body: impl FnOnce(&ProductionSourceOutputOccurrencesV1<'_, '_>, &mut Budget<'_>),
) {
    with_actual(source, profile, |bound, checked, budget| {
        let floor = budget.storage();
        let (coordinates, coordinates_storage) =
            dialect_amdgcn::check_production_target_coordinate_preservation_v1(
                source.executable(),
                bound,
                profile,
                budget,
            )
            .unwrap();
        budget
            .reserve_storage(coordinates_storage.retained_storage())
            .unwrap();
        let derived_floor = budget.storage();
        let (view, storage) =
            derive_source_output_occurrences_v1(source, &coordinates, checked, budget).unwrap();
        assert_eq!(budget.storage(), derived_floor);
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let retained_floor = budget.storage();
        body(&view, budget);
        assert_eq!(budget.storage(), retained_floor);
        assert!(!view.grants_authority());
        drop(view);
        budget.release_storage(storage.retained_storage()).unwrap();
        drop(coordinates);
        budget
            .release_storage(coordinates_storage.retained_storage())
            .unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn actual_global_source_load_store_keep_original_sites_and_checked_output_uses() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for shape in [GlobalSourceShape::Straight, GlobalSourceShape::Merged] {
            let source = genuine_global_source(shape);
            with_global_view(&source, profile, |view, budget| {
                let block = u32::from(matches!(shape, GlobalSourceShape::Merged));
                let read = view
                    .global_access(ROOT, ROOT, block, Some(0), 0, budget)
                    .unwrap();
                let write = view
                    .global_access(ROOT, ROOT, block, Some(1), 0, budget)
                    .unwrap();
                let GlobalAccess::Retained {
                    original,
                    operation,
                    source_argument,
                    parameter,
                    pointer,
                    value: None,
                    result: Some(result),
                    executable: true,
                } = read
                else {
                    panic!("genuine Global Load was not retained: {read:?}");
                };
                assert_eq!(source_argument, 0);
                assert_eq!(
                    parameter,
                    Definition::FunctionArgument {
                        function: operation.block.function,
                        argument: 0
                    }
                );
                assert_eq!(
                    pointer.coordinate,
                    UseCoordinate::OperationOperand {
                        operation,
                        operand: 0
                    }
                );
                assert_eq!(
                    result,
                    Definition::Result {
                        operation,
                        result: 0
                    }
                );
                let load = &view.output().module().functions[operation.block.function.0 as usize]
                    .body
                    .as_ref()
                    .unwrap()
                    .blocks[operation.block.block as usize]
                    .operations[operation.operation as usize];
                assert!(
                    matches!(load.kind, fe2o3_kernel_ir::OperationKind::Load { access, .. }
                    if access.address_space == fe2o3_kernel_ir::AddressSpace::Global && !access.volatile)
                );
                if matches!(shape, GlobalSourceShape::Merged) {
                    assert_ne!(original.block, operation.block);
                }
                let GlobalAccess::Retained {
                    operation: store,
                    source_argument: 0,
                    pointer: store_pointer,
                    value: Some(stored),
                    result: None,
                    executable: true,
                    ..
                } = write
                else {
                    panic!("genuine Global Store was not retained: {write:?}");
                };
                assert_eq!(
                    store_pointer.coordinate,
                    UseCoordinate::OperationOperand {
                        operation: store,
                        operand: 0
                    }
                );
                assert_eq!(
                    stored.coordinate,
                    UseCoordinate::OperationOperand {
                        operation: store,
                        operand: 1
                    }
                );
                assert_eq!(stored.definition, result);
                assert!(
                    view.global_access(ROOT, ROOT, block, Some(0), 1, budget)
                        .is_err()
                );
                assert!(
                    view.global_access(
                        SemanticFunctionIdV1::from_index(1),
                        ROOT,
                        block,
                        Some(0),
                        0,
                        budget
                    )
                    .is_err()
                );
            });
        }
    }
}

#[test]
fn actual_global_noop_and_checked_omission_are_distinct() {
    let source = genuine_global_source(GlobalSourceShape::Straight);
    with_actual(&source, Profile::Gfx942, |bound, checked, _| {
        assert_eq!(bound.module(), checked.owner().module());
    });
    let omitted = genuine_global_source(GlobalSourceShape::Omitted);
    with_global_view(&omitted, Profile::Gfx942, |view, budget| {
        for statement in [0, 1] {
            assert!(matches!(
                view.global_access(ROOT, ROOT, 1, Some(statement), 0, budget)
                    .unwrap(),
                GlobalAccess::OmittedUnreachable { .. }
            ));
        }
        assert!(
            view.output()
                .module()
                .functions
                .iter()
                .filter_map(|f| f.body.as_ref())
                .flat_map(|b| &b.blocks)
                .flat_map(|b| &b.operations)
                .all(|op| !matches!(
                    op.kind,
                    fe2o3_kernel_ir::OperationKind::Load { .. }
                        | fe2o3_kernel_ir::OperationKind::Store { .. }
                ))
        );
    });
}

#[test]
fn actual_mixed_global_and_private_span_is_not_partially_relabelled() {
    let source = genuine_global_source(GlobalSourceShape::MixedPrivate);
    with_global_view(&source, Profile::Gfx942, |view, budget| {
        assert_eq!(
            view.global_access(ROOT, ROOT, 0, Some(1), 0, budget)
                .unwrap(),
            GlobalAccess::Unsupported(GlobalUnsupported::PrivateMemory)
        );
    });
}

#[test]
fn genuine_call_and_nonentry_source_spans_keep_explicit_refusals() {
    let source = genuine_global_call_source();
    with_global_view(&source, Profile::Gfx942, |view, budget| {
        assert_eq!(
            view.global_access(ROOT, ROOT, 0, None, 0, budget).unwrap(),
            GlobalAccess::Unsupported(GlobalUnsupported::Call)
        );
        assert!(matches!(
            view.global_access(ROOT, ROOT, 0, Some(u32::MAX), 0, budget),
            Err(ProductionSourceOutputErrorV1::Invalid(
                "global source span is absent"
            ))
        ));
        assert_eq!(
            view.global_access(
                ROOT,
                SemanticFunctionIdV1::from_index(1),
                0,
                Some(0),
                0,
                budget
            )
            .unwrap(),
            GlobalAccess::Unsupported(GlobalUnsupported::NonEntryFunction)
        );
    });
}

#[test]
fn actual_global_output_with_mutated_parameter_ancestry_cannot_enter_the_checked_bridge() {
    let source = genuine_global_source(GlobalSourceShape::Straight);
    with_actual(&source, Profile::Gfx942, |bound, checked, budget| {
        let floor = budget.storage();
        let mut changed = checked.owner().module().clone();
        let body = changed.functions[0].body.as_mut().unwrap();
        let foreign_parameter = body.parameters[1];
        let load = body
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
            .find(|operation| matches!(operation.kind, fe2o3_kernel_ir::OperationKind::Load { .. }))
            .unwrap();
        let fe2o3_kernel_ir::OperationKind::Load { pointer, .. } = &mut load.kind else {
            unreachable!()
        };
        assert_ne!(*pointer, foreign_parameter);
        *pointer = foreign_parameter;
        let (changed, changed_storage) =
            Owner::from_module_ref_with_verification_budget_v12(&changed, budget).unwrap();
        budget
            .reserve_storage(changed_storage.retained_storage())
            .unwrap();
        let (input, input_storage) =
            fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(bound, budget).unwrap();
        budget
            .reserve_storage(input_storage.retained_storage())
            .unwrap();
        let (output, output_storage) =
            fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(&changed, budget).unwrap();
        budget
            .reserve_storage(output_storage.retained_storage())
            .unwrap();
        let checked_floor = budget.storage();
        assert!(matches!(
            fe2o3_kernel_analysis::check_canonical_kir_transition_v1(
                &input,
                &output,
                checked.occurrences().candidate(),
                budget,
            ),
            Err(fe2o3_kernel_analysis::CanonicalKirTransitionErrorV1::Rule(
                "final operand has no exact descendant"
            ))
        ));
        assert_eq!(budget.storage(), checked_floor);
        drop(output);
        budget
            .release_storage(output_storage.retained_storage())
            .unwrap();
        drop(input);
        budget
            .release_storage(input_storage.retained_storage())
            .unwrap();
        drop(changed);
        budget
            .release_storage(changed_storage.retained_storage())
            .unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn genuine_global_repeated_lookup_has_one_shared_32_31_boundary() {
    let source = genuine_global_source(GlobalSourceShape::Straight);
    with_global_view(&source, Profile::Gfx942, |view, budget| {
        let floor = budget.storage();
        // Three source spans sort as statement0, statement1, terminator; the
        // Store query hits the one middle comparison. Each query is 4+1+6+2+3;
        // the source key compares five fields, including a distinct site tag.
        for limit in [32, 31] {
            let mut work = Work::new(7 + limit);
            {
                let mut query = Budget::new(&mut work, floor);
                query.charge_work(7).unwrap();
                query.reserve_storage(floor).unwrap();
                view.global_access(ROOT, ROOT, 0, Some(1), 0, &mut query)
                    .unwrap();
                assert_eq!(query.work(), 23);
                let second = view.global_access(ROOT, ROOT, 0, Some(1), 0, &mut query);
                if limit == 32 {
                    assert!(second.is_ok());
                    assert_eq!(query.work(), 39);
                } else {
                    assert!(matches!(
                        second,
                        Err(ProductionSourceOutputErrorV1::Resource(
                            fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Work(_)
                        ))
                    ));
                    assert_eq!(query.work(), 36);
                }
                assert_eq!(query.storage(), floor);
            }
            assert_eq!(work.failed_work(), (limit == 31).then_some(39));
        }
        let mut work = Work::new(100);
        let mut missing = Budget::new(&mut work, 0);
        assert!(matches!(
            view.global_access(ROOT, ROOT, 0, Some(1), 0, &mut missing),
            Err(ProductionSourceOutputErrorV1::Resource(
                fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting
            ))
        ));
        assert_eq!(missing.storage(), 0);
        assert_eq!(missing.work(), 4);
    });
}

mod generated_global_boundary_tests {
    use super::*;
    include!("source_output_pipeline_fixture_v1_tests.rs");

    #[test]
    fn genuine_generated_intrinsic_spans_do_not_become_ordinary_global_relations() {
        let (ssa, launch) = pipeline_fixture(false);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
        let source = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
            ssa,
            launch,
            fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )
        .unwrap();
        // Whole-call refusal uses the source intrinsic, not the absence of
        // physical KIR Calls. The fixture also emits storage and synchronization.
        let mut allocations = 0;
        let mut barriers = 0;
        for operation in source
            .executable()
            .module()
            .functions
            .iter()
            .filter_map(|function| function.body.as_ref())
            .flat_map(|body| &body.blocks)
            .flat_map(|block| &block.operations)
        {
            match operation.kind {
                fe2o3_kernel_ir::OperationKind::WorkgroupMemory(_) => allocations += 1,
                fe2o3_kernel_ir::OperationKind::WorkgroupBarrier(_) => barriers += 1,
                _ => {}
            }
        }
        assert_eq!((allocations, barriers), (1, 1));
        with_global_view(&source, Profile::Gfx942, |view, budget| {
            let mut seen = 0;
            for (block, contents) in source.semantic_ssa().source_semantic().functions()[0]
                .blocks()
                .iter()
                .enumerate()
            {
                if matches!(
                    contents.terminator().kind(),
                    SemanticTerminatorKindV1::Call(_)
                ) {
                    assert_eq!(
                        view.global_access(ROOT, ROOT, block as u32, None, 0, budget)
                            .unwrap(),
                        GlobalAccess::Unsupported(GlobalUnsupported::Call)
                    );
                    seen += 1;
                }
            }
            // Scope, creation, write, read, and six pipeline events.
            assert_eq!(seen, 10);
        });
    }
}

include!("source_output_global_ranked_v1_tests.rs");
include!("source_output_operation_contract_v1_tests.rs");
