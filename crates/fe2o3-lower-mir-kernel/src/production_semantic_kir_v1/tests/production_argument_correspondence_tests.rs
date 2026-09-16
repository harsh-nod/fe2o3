use super::correspondence_replay_core_tests::{Expected, check_both};
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAbiRegularAttributesV1, SemanticAbiValueAttributesV1, SemanticAggregateRvalueV1,
};

include!("production_argument_test_fixture.rs");

fn argument_launch_roster(
    source: &ProductionSemanticSsaOwnerV1,
) -> crate::ProductionSourceLaunchRosterV1 {
    let semantic = source.source_semantic();
    let inputs = semantic
        .roots()
        .iter()
        .map(|root| {
            let entry = semantic.functions()[root.index() as usize]
                .kernel_entry()
                .unwrap();
            crate::ProductionSourceLaunchRootInputV1::new(
                std::str::from_utf8(entry.export_symbol().as_bytes()).unwrap(),
                *entry.kernel_binding_identity().as_bytes(),
                crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
            )
        })
        .collect::<Vec<_>>();
    crate::ProductionSourceLaunchRosterV1::try_new(semantic, &inputs).unwrap()
}

fn lower_argument_owner(
    source: &ProductionSemanticSsaOwnerV1,
    limits: ProductionSemanticKirLimitsV1,
) -> Result<(Module, SemanticKirCorrespondenceV1), ProductionSemanticKirErrorV1> {
    let roster = argument_launch_roster(source);
    let roots = materialization_launch_roots_v1(source, &roster)?;
    lower_module(source, limits, Some(&roots))
}

#[path = "production_argument_view_tests.rs"]
mod complete_view_tests;

#[path = "production_call_storage_tests.rs"]
mod call_storage_tests;

#[path = "production_call_view_tests.rs"]
mod call_view_tests;

#[test]
fn exact_argument_correspondence_accepts_packed_expanded_empty_and_shared_owners() {
    for expanded in [false, true] {
        for empty in [false, true] {
            let source = argument_owner(expanded, empty, true);
            let (module, correspondence) =
                lower_argument_owner(&source, ProductionSemanticKirLimitsV1::default()).unwrap();
            assert_eq!(module.functions.len(), 3);
            assert_eq!(correspondence.lowered_functions.len(), 4);
            for root in &module.kernels {
                assert_eq!(
                    module
                        .function(&root.entry)
                        .unwrap()
                        .signature
                        .parameters
                        .len(),
                    5
                );
            }
            for instance in correspondence
                .lowered_functions
                .iter()
                .filter(|row| row.role == SemanticKirFunctionRoleV1::InternalHelper)
            {
                let same = |owner, function| {
                    owner == instance.correspondence_owner && function == instance.semantic_function
                };
                let direct = correspondence
                    .parameter_bindings
                    .iter()
                    .filter(|row| same(row.correspondence_owner, row.semantic_function))
                    .collect::<Vec<_>>();
                let components = correspondence
                    .parameter_component_bindings
                    .iter()
                    .filter(|row| same(row.correspondence_owner, row.semantic_function))
                    .collect::<Vec<_>>();
                let ignored = correspondence
                    .ignored_parameter_bindings
                    .iter()
                    .filter(|row| same(row.correspondence_owner, row.semantic_function))
                    .collect::<Vec<_>>();
                assert_eq!(
                    (direct.len(), components.len(), ignored.len()),
                    match (expanded, empty) {
                        (false, false) => (0, 3, 1),
                        (true, false) => (1, 2, 2),
                        (false, true) => (0, 0, 2),
                        (true, true) => (0, 0, 1),
                    }
                );
                let body = module
                    .function(&instance.kernel_ir_function)
                    .unwrap()
                    .body
                    .as_ref()
                    .unwrap();
                assert_eq!(body.parameters.len(), if empty { 0 } else { 3 });
                if !empty {
                    for (index, leaf) in [0, 2].into_iter().enumerate() {
                        let expected = if expanded {
                            vec![SemanticKirParameterProjectionV1::Field(leaf)]
                        } else {
                            vec![
                                SemanticKirParameterProjectionV1::Field(0),
                                SemanticKirParameterProjectionV1::Field(leaf),
                            ]
                        };
                        assert_eq!(
                            components[index].semantic_local.index(),
                            if expanded { 4 } else { 2 }
                        );
                        assert_eq!(components[index].projection.as_ref(), expected);
                        assert_eq!(components[index].semantic_component_type, U32);
                        assert_eq!(components[index].kernel_ir_value, body.parameters[index]);
                    }
                    if expanded {
                        assert_eq!(direct[0].semantic_local.index(), 2);
                        assert_eq!(direct[0].kernel_ir_value, body.parameters[2]);
                    } else {
                        assert_eq!(
                            components[2].projection.as_ref(),
                            [SemanticKirParameterProjectionV1::Field(2)]
                        );
                        assert_eq!(components[2].kernel_ir_value, body.parameters[2]);
                    }
                }
            }
            check_both(
                &source,
                &module,
                source.source_semantic().roots(),
                100,
                &correspondence,
                Expected::Accepted,
            );
        }
    }
}

#[test]
fn exact_argument_correspondence_rejects_path_type_slot_and_owner_substitutions() {
    let source = argument_owner(false, false, true);
    let (module, correspondence) =
        lower_argument_owner(&source, ProductionSemanticKirLimitsV1::default()).unwrap();
    let helper = correspondence
        .lowered_functions
        .iter()
        .find(|instance| instance.role == SemanticKirFunctionRoleV1::InternalHelper)
        .unwrap();
    for mutation in 0..17 {
        let mut changed = correspondence.clone();
        let mut graph = module.clone();
        let index = changed
            .parameter_component_bindings
            .iter()
            .position(|binding| binding.semantic_function == helper.semantic_function)
            .unwrap();
        match mutation {
            0 => {
                changed.parameter_component_bindings[index].projection[1] =
                    SemanticKirParameterProjectionV1::Field(1)
            }
            1 => {
                changed.parameter_component_bindings[index].projection =
                    vec![SemanticKirParameterProjectionV1::Field(0)].into_boxed_slice()
            }
            2 => changed.parameter_component_bindings[index].semantic_component_type = ZERO,
            3 => {
                let first = changed.parameter_component_bindings[index].kernel_ir_value;
                changed.parameter_component_bindings[index].kernel_ir_value =
                    changed.parameter_component_bindings[index + 1].kernel_ir_value;
                changed.parameter_component_bindings[index + 1].kernel_ir_value = first;
            }
            4 => {
                graph
                    .functions
                    .iter_mut()
                    .find(|function| function.id == helper.kernel_ir_function)
                    .unwrap()
                    .signature
                    .parameters
                    .pop();
            }
            5 => {
                graph
                    .functions
                    .iter_mut()
                    .find(|function| function.id == helper.kernel_ir_function)
                    .unwrap()
                    .signature
                    .parameters[0] = Type::Scalar(ScalarType::F32)
            }
            6 => {
                changed.parameter_component_bindings[index].correspondence_owner =
                    SemanticFunctionIdV1::from_index(2)
            }
            7 => {
                changed.parameter_component_bindings[index].semantic_function =
                    SemanticFunctionIdV1::from_index(1)
            }
            8 => changed.ignored_parameter_bindings[0].semantic_type = UNIT,
            9 => {
                let mut rows = changed.ignored_parameter_bindings.to_vec();
                rows.remove(0);
                changed.ignored_parameter_bindings = rows.into_boxed_slice();
            }
            10 => {
                let body = graph
                    .functions
                    .iter_mut()
                    .find(|function| function.id == helper.kernel_ir_function)
                    .unwrap()
                    .body
                    .as_mut()
                    .unwrap();
                body.parameters[1] = body.parameters[0];
            }
            11 => {
                std::mem::swap(
                    &mut changed.parameter_bindings[0].kernel_ir_value,
                    &mut changed.parameter_component_bindings[0].kernel_ir_value,
                );
            }
            12 => {
                let mut rows = changed.ignored_parameter_bindings.to_vec();
                rows.insert(0, rows[0]);
                changed.ignored_parameter_bindings = rows.into_boxed_slice();
            }
            13 => {
                changed.parameter_component_bindings[index].kernel_ir_value =
                    changed.parameter_component_bindings[index + 1].kernel_ir_value
            }
            14 => {
                changed.parameter_component_bindings[index].projection[0] =
                    SemanticKirParameterProjectionV1::ArrayIndex(0)
            }
            15 => {
                let mut rows = changed.ignored_parameter_bindings.to_vec();
                let at = rows
                    .iter()
                    .position(|row| row.semantic_function == helper.semantic_function)
                    .unwrap();
                rows.insert(
                    at,
                    SemanticKirIgnoredParameterBindingV1 {
                        correspondence_owner: helper.correspondence_owner,
                        semantic_function: helper.semantic_function,
                        semantic_local: SemanticLocalIdV1::from_index(2),
                        semantic_type: TUPLE,
                    },
                );
                changed.ignored_parameter_bindings = rows.into_boxed_slice();
            }
            16 => {
                let function = graph
                    .functions
                    .iter_mut()
                    .find(|function| function.id == helper.kernel_ir_function)
                    .unwrap();
                function.signature.parameters.remove(0);
                function.body.as_mut().unwrap().parameters.remove(0);
                let mut rows = changed.parameter_component_bindings.to_vec();
                rows.retain(|row| {
                    row.semantic_function != helper.semantic_function
                        || row.projection.as_ref()
                            != [
                                SemanticKirParameterProjectionV1::Field(0),
                                SemanticKirParameterProjectionV1::Field(0),
                            ]
                });
                changed.parameter_component_bindings = rows.into_boxed_slice();
            }
            _ => unreachable!(),
        }
        check_both(
            &source,
            &graph,
            source.source_semantic().roots(),
            100,
            &changed,
            Expected::CorrespondenceMismatch,
        );
    }
}

#[test]
fn exact_argument_correspondence_rejects_a_phantom_expanded_empty_tuple_local() {
    let source = argument_owner(true, true, false);
    let (module, mut correspondence) =
        lower_argument_owner(&source, ProductionSemanticKirLimitsV1::default()).unwrap();
    let mut rows = correspondence.ignored_parameter_bindings.to_vec();
    let helper = rows
        .iter()
        .find(|binding| binding.semantic_function.index() == 0)
        .copied()
        .unwrap();
    rows.push(SemanticKirIgnoredParameterBindingV1 {
        semantic_local: SemanticLocalIdV1::from_index(2),
        semantic_type: UNIT,
        ..helper
    });
    correspondence.ignored_parameter_bindings = rows.into_boxed_slice();
    check_both(
        &source,
        &module,
        source.source_semantic().roots(),
        100,
        &correspondence,
        Expected::CorrespondenceMismatch,
    );
}

#[test]
fn exact_argument_correspondence_rejects_another_valid_expanded_local() {
    let source = argument_owner(true, false, false);
    let (module, mut correspondence) =
        lower_argument_owner(&source, ProductionSemanticKirLimitsV1::default()).unwrap();
    let row = correspondence
        .parameter_component_bindings
        .iter_mut()
        .find(|row| row.semantic_function.index() == 0)
        .unwrap();
    row.semantic_local = SemanticLocalIdV1::from_index(2);
    check_both(
        &source,
        &module,
        source.source_semantic().roots(),
        100,
        &correspondence,
        Expected::CorrespondenceMismatch,
    );
}

#[test]
fn exact_argument_correspondence_small_ignored_shapes_use_small_quotas() {
    let source = argument_owner(true, true, false);
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(
        DEFAULT_ARGUMENT_CORRESPONDENCE_WORK_V1,
    );
    let mut budget = ArgumentBudgetV1::new(&mut work, DEFAULT_ARGUMENT_CORRESPONDENCE_STORAGE_V1);
    for _ in 0..64 {
        prepay_argument_shape_v1(source.source_semantic(), ZERO, &mut budget).unwrap();
        budget.release_storage(budget.storage()).unwrap();
    }
    assert!(budget.work() < 64 * 1024);
    assert!(budget.peak_storage() < 8192);
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    assert!(matches!(
        prepay_argument_shape_v1(source.source_semantic(), ZERO, &mut budget),
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(
                _
            ))
        )
    ));
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (1, 0, 0)
    );
}

#[test]
fn parameter_shape_diagnostics_do_not_format_wide_type_tables() {
    // Descriptor-only rejection test, not an admitted source/proof fixture.
    let wide = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([9; 32]),
        SemanticLayoutIdentityV1::from_sha256([9; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(0),
            1,
            SemanticAggregateLayoutV1::new(vec![0; 16_384], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![UNIT; 16_384]).unwrap()),
    );
    let types = [unit_type(), wide];
    for result in [
        lower_parameter_memory_element_v1(&types, U32),
        lower_parameter_scalar_v1(&types, U32),
    ] {
        let Err(error) = result else {
            panic!("wide non-scalar descriptor must reject");
        };
        assert!(matches!(
            error,
            ProductionSemanticKirErrorV1::Unsupported { .. }
        ));
        assert!(error.to_string().len() < 256);
    }
}

#[test]
fn exact_argument_trace_radix_index_orders_the_full_value_domain() {
    let bindings =
        [u32::MAX, 0, 1 << 31, 7, (1 << 31) - 1].map(|value| SemanticKirParameterBindingV1 {
            correspondence_owner: SemanticFunctionIdV1::from_index(0),
            semantic_function: SemanticFunctionIdV1::from_index(0),
            semantic_local: SemanticLocalIdV1::from_index(0),
            kernel_ir_value: ValueId(value),
        });
    let mut rows = bindings
        .iter()
        .map(|binding| IndexedArgumentTraceV1 {
            trace: PhysicalArgumentTraceV1::Direct(binding),
            used: false,
        })
        .collect::<Vec<_>>();
    sort_argument_trace_v1(&mut rows, 31);
    assert_eq!(
        rows.iter()
            .map(|row| row.trace.value().0)
            .collect::<Vec<_>>(),
        [0, 7, (1 << 31) - 1, 1 << 31, u32::MAX]
    );
}

#[test]
fn exact_argument_correspondence_full_core_accumulates_shared_root_work() {
    let source = argument_owner(true, false, true);
    let (module, correspondence) =
        lower_argument_owner(&source, ProductionSemanticKirLimitsV1::default()).unwrap();
    let check = |limit| {
        validate_semantic_kir_correspondence_after_source_replay_v1(
            &source,
            &module,
            source.source_semantic().roots(),
            ProductionSemanticKirLimitsV1::default()
                .with_argument_correspondence_limits(limit, usize::MAX),
            &correspondence,
        )
    };
    source.verify_replay().unwrap();
    let (mut rejected, mut accepted) = (0, DEFAULT_ARGUMENT_CORRESPONDENCE_WORK_V1);
    check(accepted).unwrap();
    while rejected + 1 < accepted {
        let limit = rejected + (accepted - rejected) / 2;
        match check(limit) {
            Ok(()) => accepted = limit,
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Work(_),
            )) => rejected = limit,
            Err(error) => panic!("unexpected boundary error: {error}"),
        }
    }
    check(accepted).unwrap();
    assert!(matches!(
        check(accepted - 1),
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(
                _
            ))
        )
    ));
    // A single owner association fits a strictly smaller budget than the
    // complete roster, including its repeated helper association.
    let instance = &correspondence.lowered_functions[0];
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(accepted - 1);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    validate_parameter_correspondence_v1(
        source.source_semantic(),
        instance,
        module.function(&instance.kernel_ir_function).unwrap(),
        ArgumentTraceV1 {
            direct: &correspondence.parameter_bindings[..1],
            components: &correspondence.parameter_component_bindings[..4],
            ignored: &correspondence.ignored_parameter_bindings[..1],
        },
        &mut budget,
    )
    .unwrap();
    assert!(budget.work() < accepted);
}

#[test]
fn exact_argument_correspondence_budget_is_cumulative_and_restores_scratch() {
    let source = argument_owner(true, false, true);
    let (module, correspondence) =
        lower_argument_owner(&source, ProductionSemanticKirLimitsV1::default()).unwrap();
    let check = |budget: &mut ArgumentBudgetV1<'_>, wrong_type: bool| {
        let instance = correspondence
            .lowered_functions
            .iter()
            .find(|row| row.role == SemanticKirFunctionRoleV1::InternalHelper)
            .unwrap();
        let direct = correspondence
            .parameter_bindings
            .iter()
            .filter(|row| {
                row.correspondence_owner == instance.correspondence_owner
                    && row.semantic_function == instance.semantic_function
            })
            .copied()
            .collect::<Vec<_>>();
        let mut components = correspondence
            .parameter_component_bindings
            .iter()
            .filter(|row| {
                row.correspondence_owner == instance.correspondence_owner
                    && row.semantic_function == instance.semantic_function
            })
            .cloned()
            .collect::<Vec<_>>();
        if wrong_type {
            components[0].semantic_component_type = ZERO;
        }
        let ignored = correspondence
            .ignored_parameter_bindings
            .iter()
            .filter(|row| {
                row.correspondence_owner == instance.correspondence_owner
                    && row.semantic_function == instance.semantic_function
            })
            .copied()
            .collect::<Vec<_>>();
        validate_parameter_correspondence_v1(
            source.source_semantic(),
            instance,
            module.function(&instance.kernel_ir_function).unwrap(),
            ArgumentTraceV1 {
                direct: &direct,
                components: &components,
                ignored: &ignored,
            },
            budget,
        )
    };
    let floor = 23;
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(floor).unwrap();
    check(&mut budget, false).unwrap();
    let required_work = budget.work();
    let required_storage = budget.peak_storage();
    assert_eq!(budget.storage(), floor);
    assert!(required_work > 0 && required_storage > floor);
    assert!(matches!(
        check(&mut budget, true),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
    assert_eq!(budget.storage(), floor);
    for (work_limit, storage_limit, expected) in [
        (required_work, required_storage, true),
        (required_work - 1, required_storage, false),
        (required_work, required_storage - 1, false),
        (0, required_storage, false),
        (required_work, floor, false),
    ] {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let result = check(&mut budget, false);
        if expected {
            result.unwrap();
            assert!(matches!(
                check(&mut budget, false),
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(_)
                    )
                )
            ));
        } else {
            assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(_))
            ));
        }
        assert_eq!(budget.storage(), floor);
    }
    let limits =
        ProductionSemanticKirLimitsV1::default().with_argument_correspondence_limits(0, usize::MAX);
    assert!(matches!(
        lower_argument_owner(&source, limits),
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(
                _
            ))
        )
    ));
}
