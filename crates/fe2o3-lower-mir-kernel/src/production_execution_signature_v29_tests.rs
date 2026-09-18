use super::*;

mod reversed_fixture {
    use super::*;
    include!("production_execution_signature_fixture_v29_tests.rs");
}

fn with_instances<R>(
    mut owner: ProductionSemanticSsaOwnerV1,
    work_limit: usize,
    storage_limit: usize,
    consumer: impl FnOnce(
        &ExecutionInstancesV29<'_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<R, ProductionSemanticKirErrorV1>,
) -> (Result<R, ProductionSemanticKirErrorV1>, usize, usize) {
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
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(consumer(
                instances, budget,
            ))
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

fn selectors(signature: &LoweredFunctionSignatureV1) -> Vec<(u32, Option<u32>, Option<usize>)> {
    signature
        .call_arguments
        .iter()
        .map(|row| (row.source_argument, row.tuple_field, row.component))
        .collect()
}

fn checked_signature(
    instances: &ExecutionInstancesV29<'_>,
    instance: ProductionCallInstanceIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<LoweredFunctionSignatureV1, ProductionSemanticKirErrorV1> {
    let floor = budget.storage();
    let result = execution_function_signature_v29(instances, instance, budget);
    if let Ok(signature) = &result {
        let retained = signature.parameter_semantic_types.capacity()
            * std::mem::size_of::<SemanticTypeIdV1>()
            + signature.parameter_types.capacity() * std::mem::size_of::<Type>()
            + signature.call_arguments.capacity() * std::mem::size_of::<HelperCallArgumentV1>()
            + signature.result_types.capacity() * std::mem::size_of::<Type>();
        assert_eq!(budget.storage() - floor, retained);
        let abi = instances.instance(instance).unwrap().declaration().abi();
        assert_eq!(signature.parameter_semantic_types, abi.source_input_types());
        assert_eq!(signature.result_semantic_type, abi.source_output_type());
    } else {
        assert_eq!(budget.storage(), floor);
    }
    result
}

#[test]
fn signatures_preserve_source_rosters_and_match_real_instance_plans() {
    for shape in [
        Shape::Tuple,
        Shape::Struct,
        Shape::RustCall,
        Shape::IndexTuple,
        Shape::IndexRustCall,
        Shape::PackedRustCall,
    ] {
        with_instances(owner(shape), 10_000_000, 10_000_000, |instances, budget| {
            let calls = instances.calls(instances.root()).unwrap();
            let mut previous: Option<LoweredFunctionSignatureV1> = None;
            for (index, call) in calls.iter().enumerate() {
                let instance = call.child().unwrap();
                let signature = checked_signature(instances, instance, budget)?;
                let plan = execution_instance_plan_v29(
                    instances,
                    instance,
                    FunctionId::new(format!("real_child_{index}")),
                    SemanticEmissionPlacementV1 {
                        first_block: 17 + index as u32,
                        first_value: 300 + index as u32,
                    },
                    budget,
                )?;
                assert_eq!(signature.parameter_types, plan.parameter_types);
                assert_eq!(signature.result_types, plan.result_types);
                assert_eq!(
                    selectors(&signature),
                    plan.call_arguments
                        .iter()
                        .map(|row| { (row.source_argument, row.tuple_field, row.component) })
                        .collect::<Vec<_>>()
                );
                assert_eq!(plan.parameter_values, [ValueId(300 + index as u32)]);
                if let Some(prior) = &previous {
                    assert_eq!(selectors(prior), selectors(&signature));
                    assert_eq!(
                        prior.parameter_semantic_types,
                        signature.parameter_semantic_types
                    );
                    assert_eq!(prior.parameter_types, signature.parameter_types);
                    assert_eq!(prior.result_types, signature.result_types);
                }
                previous = Some(signature);
            }
            assert_eq!(calls.len(), 2);
            Ok(())
        })
        .0
        .unwrap();
    }
}

#[test]
fn erased_nominal_carriers_still_have_source_signature_arguments() {
    for input in [
        Input::MutableContext,
        Input::Workgroup,
        Input::SharedWorkgroup,
        Input::CapturedContext,
        Input::CapturedWorkgroup,
        Input::CapturedWorkgroupReference,
    ] {
        with_instances(
            nominal_owner(input, false),
            10_000_000,
            10_000_000,
            |instances, budget| {
                let child = instances.calls(instances.root()).unwrap()[0]
                    .child()
                    .unwrap();
                let signature = checked_signature(instances, child, budget)?;
                assert_eq!(signature.parameter_semantic_types.len(), 1);
                let scalar = matches!(
                    input,
                    Input::CapturedContext
                        | Input::CapturedWorkgroup
                        | Input::CapturedWorkgroupReference
                );
                assert_eq!(signature.parameter_types.len(), usize::from(scalar));
                assert_eq!(signature.call_arguments.len(), usize::from(scalar));
                assert!(signature.result_types.is_empty());
                Ok(())
            },
        )
        .0
        .unwrap();
    }
}

#[test]
fn signatures_preserve_reversed_same_typed_rust_call_fields() {
    with_instances(
        reversed_fixture::owner(),
        10_000_000,
        10_000_000,
        |instances, budget| {
            let calls = instances.calls(instances.root()).unwrap();
            for (index, call) in calls.iter().enumerate() {
                let child = call.child().unwrap();
                let signature = checked_signature(instances, child, budget)?;
                assert_eq!(signature.parameter_semantic_types, [UNIT, PAIR]);
                assert_eq!(
                    signature.parameter_types,
                    vec![Type::Scalar(ScalarType::U32); 2]
                );
                assert_eq!(
                    selectors(&signature),
                    [(1, Some(2), Some(0)), (1, Some(1), Some(0))]
                );
                let plan = execution_instance_plan_v29(
                    instances,
                    child,
                    FunctionId::new(format!("reversed_child_{index}")),
                    SemanticEmissionPlacementV1 {
                        first_block: 17,
                        first_value: 300,
                    },
                    budget,
                )?;
                assert_eq!(
                    plan.parameter_declarations,
                    [(0, 1, UNIT), (1, 2, U32), (1, 3, CONTEXT), (1, 4, U32),]
                );
                assert_eq!(plan.parameter_values, [ValueId(300), ValueId(301)]);
            }
            Ok(())
        },
    )
    .0
    .unwrap();
}

#[test]
fn failed_instance_placement_preserves_a_previously_built_signature() {
    with_instances(
        owner(Shape::PackedRustCall),
        10_000_000,
        10_000_000,
        |instances, budget| {
            let child = instances.calls(instances.root()).unwrap()[0]
                .child()
                .unwrap();
            let signature = checked_signature(instances, child, budget)?;
            let floor = budget.storage();
            assert!(matches!(
                execution_instance_plan_v29(
                    instances,
                    child,
                    FunctionId::new("exhausted"),
                    SemanticEmissionPlacementV1 {
                        first_block: 0,
                        first_value: u32::MAX
                    },
                    budget,
                ),
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Arithmetic
                    )
                )
            ));
            assert_eq!(budget.storage(), floor);
            assert_eq!(selectors(&signature), [(1, None, Some(0))]);
            assert!(checked_signature(instances, instances.root(), budget).is_err());
            assert_eq!(budget.storage(), floor);
            Ok(())
        },
    )
    .0
    .unwrap();
}

#[test]
fn signatures_have_exact_work_and_storage_boundaries() {
    let run = |work, storage| {
        with_instances(
            reversed_fixture::owner(),
            work,
            storage,
            |instances, budget| {
                let child = instances.calls(instances.root()).unwrap()[0]
                    .child()
                    .unwrap();
                checked_signature(instances, child, budget)
            },
        )
    };
    let (result, work, storage) = run(10_000_000, 10_000_000);
    result.unwrap();
    assert!(run(work, storage).0.is_ok());
    assert!(run(work - 1, storage).0.is_err());
    assert!(run(work, storage - 1).0.is_err());
}
