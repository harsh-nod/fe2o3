use super::*;

#[path = "production_atomic_argument_view_tests.rs"]
mod atomic_view_tests;

#[path = "../../production_conditional_output_binding_v1_tests.rs"]
mod conditional_output_binding_tests;

fn argument_view_owner(expanded: bool, shape: ArgumentTupleShape) -> ProductionPreRankedKirOwnerV1 {
    let source = argument_owner_shape(expanded, shape, true, true);
    materialize_argument_view(source)
}

fn materialize_argument_view(
    source: ProductionSemanticSsaOwnerV1,
) -> ProductionPreRankedKirOwnerV1 {
    let roster = argument_launch_roster(&source);
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        source,
        roster,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap()
}

fn empty_ordinary_argument_owner(variadic: bool) -> ProductionSemanticSsaOwnerV1 {
    let original =
        argument_owner_shape_with_fixed(true, ArgumentTupleShape::EmptyUnit, false, false, false);
    let semantic = original.source_semantic();
    let mut functions = Vec::new();
    for (index, function) in semantic.functions().iter().enumerate() {
        let (abi, locals, blocks) = if index == 0 {
            let abi = SemanticFunctionAbiV1::from_rustc(
                function.abi().identity(),
                semantic.target().identity(),
                SemanticCanonAbiV1::C,
                SemanticExternAbiV1::C { unwind: false },
                false,
                variadic,
                0,
                vec![],
                SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
            )
            .unwrap();
            (
                abi,
                vec![function.locals()[0].clone()],
                function.blocks().to_vec(),
            )
        } else {
            let mut blocks = function.blocks().to_vec();
            let first = &blocks[0];
            let call = SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(0),
                vec![],
                Some(SemanticCallDestinationV1::new(
                    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(0), vec![], UNIT).unwrap(),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(1),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap();
            blocks[0] = SemanticBasicBlockV1::new(
                first.identity(),
                first.source(),
                first.statements().to_vec(),
                SemanticTerminatorV1::new(first.source(), SemanticTerminatorKindV1::Call(call)),
            )
            .unwrap();
            (function.abi().clone(), function.locals().to_vec(), blocks)
        };
        let mut replacement = SemanticFunctionDeclV1::new(
            function.identity(),
            function.role(),
            function.item_definition_identity(),
            function.monomorphization_identity(),
            function.generic_type_arguments_identity(),
            function.const_generic_arguments_identity(),
            function.source(),
            abi,
            locals,
            function.entry(),
            blocks,
        )
        .unwrap();
        if let Some(entry) = function.kernel_entry() {
            replacement = replacement.with_kernel_entry(entry.clone());
        }
        functions.push(replacement);
    }
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn complete_argument_view_checks_function_abi_with_no_source_or_adjusted_rows() {
    let source = empty_ordinary_argument_owner(false);
    let owner = materialize_argument_view(source);
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 100_000);
    owner
        .with_checked_arguments_v1(
            SemanticFunctionIdV1::from_index(1),
            SemanticFunctionIdV1::from_index(0),
            &mut budget,
            |view| {
                assert_eq!(view.source_arguments()?.len(), 0);
                assert_eq!(view.adjusted_arguments()?.len(), 0);
                assert!(view.physical(0)?.is_none());
                assert!(collect_argument_nodes(view)?.is_empty());
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(budget.storage(), 0);
    let variadic = empty_ordinary_argument_owner(true);
    let Err(error) = lower_argument_owner(&variadic, ProductionSemanticKirLimitsV1::default())
    else {
        panic!("zero-row variadic helper must reject before materialization");
    };
    assert!(matches!(
        error,
        ProductionSemanticKirErrorV1::Unsupported { .. }
    ));
    assert!(
        error
            .to_string()
            .contains("non-unwinding direct scalar ABI")
    );
}

fn zero_array_argument_owner(length: u64) -> ProductionSemanticSsaOwnerV1 {
    let original = argument_owner(true, true, false);
    let semantic = original.source_semantic();
    let mut types = semantic.types().to_vec();
    types[ZERO.index() as usize] = SemanticTypeDeclV1::new(
        types[ZERO.index() as usize].identity(),
        types[ZERO.index() as usize].layout_identity(),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            1,
            SemanticFieldsShapeV1::array(0, length),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            1,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Array {
            element: UNIT,
            length,
        },
    );
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
        vec![],
        vec![],
        vec![],
        semantic.functions().to_vec(),
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn complete_argument_view_counts_zero_nodes_at_the_existing_component_limit() {
    let owner = materialize_argument_view(zero_array_argument_owner(255));
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    let nodes = owner
        .with_checked_arguments_v1(
            SemanticFunctionIdV1::from_index(1),
            SemanticFunctionIdV1::from_index(0),
            &mut budget,
            collect_argument_nodes,
        )
        .unwrap();
    let array = nodes
        .iter()
        .filter(|node| node.source == 0)
        .collect::<Vec<_>>();
    assert_eq!(array.len(), 256);
    assert!(
        array
            .iter()
            .all(|node| node.coverage == ObservedCoverage::Zero)
    );
    assert_eq!(
        array[254].path,
        [ProductionArgumentProjectionV1::ArrayIndex(254)]
    );
    assert_eq!(array[255].ty, ZERO);
    assert!(array[255].path.is_empty());
    assert_eq!(budget.storage(), 0);
    let over = zero_array_argument_owner(256);
    let Err(error) = lower_argument_owner(&over, ProductionSemanticKirLimitsV1::default()) else {
        panic!("257 structural nodes exceed the existing by-value cap");
    };
    assert!(matches!(
        error,
        ProductionSemanticKirErrorV1::Unsupported { .. }
    ));
    assert!(error.to_string().contains("component limit"));
}

#[test]
fn complete_argument_view_keeps_source_envelopes_with_no_adjusted_arguments() {
    for expanded in [false, true] {
        for shape in [
            ArgumentTupleShape::EmptyUnit,
            ArgumentTupleShape::EmptyTuple,
        ] {
            let source = argument_owner_shape_with_fixed(expanded, shape, true, true, false);
            assert!(
                source.source_semantic().functions()[0]
                    .abi()
                    .adjusted_arguments()
                    .is_empty()
            );
            let owner = materialize_argument_view(source);
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100_000);
            let mut budget = ArgumentBudgetV1::new(&mut work, 100_000);
            let nodes = owner
                .with_checked_arguments_v1(
                    SemanticFunctionIdV1::from_index(1),
                    SemanticFunctionIdV1::from_index(0),
                    &mut budget,
                    |view| {
                        assert_eq!(view.source_arguments()?.len(), 1);
                        assert_eq!(view.adjusted_arguments()?.len(), 0);
                        assert!(view.physical(0)?.is_none());
                        assert_eq!(
                            view.ignored_local(SemanticLocalIdV1::from_index(1))?
                                .is_some(),
                            !expanded
                        );
                        collect_argument_nodes(view)
                    },
                )
                .unwrap();
            assert_eq!(
                nodes,
                [ObservedArgumentNode {
                    source: 0,
                    adjusted: None,
                    ty: TUPLE,
                    path: vec![],
                    local: (!expanded).then_some((SemanticLocalIdV1::from_index(1), vec![])),
                    coverage: ObservedCoverage::Zero
                }]
            );
            assert_eq!(budget.storage(), 0);
        }
    }
}

#[derive(Debug, PartialEq)]
enum ObservedCoverage {
    Zero,
    Components(usize, usize),
    Parameter(usize),
    Within(usize),
}

#[derive(Debug, PartialEq)]
struct ObservedArgumentNode {
    source: u32,
    adjusted: Option<u32>,
    ty: SemanticTypeIdV1,
    path: Vec<ProductionArgumentProjectionV1>,
    local: Option<(SemanticLocalIdV1, Vec<ProductionArgumentProjectionV1>)>,
    coverage: ObservedCoverage,
}

fn collect_argument_nodes(
    view: &mut ProductionArgumentViewV1<'_, '_>,
) -> Result<Vec<ObservedArgumentNode>, ProductionSemanticKirErrorV1> {
    let mut nodes = Vec::new();
    view.visit_nodes(|node| {
        assert_eq!(node.source().ordinal(), node.source_argument());
        assert_eq!(
            node.adjusted().map(|row| row.ordinal()),
            node.adjusted_argument()
        );
        if let Some(ignored) = node.ignored_local_binding() {
            assert_eq!(node.local_binding().unwrap().0, ignored.semantic_local());
        }
        let coverage = match node.coverage() {
            ProductionArgumentCoverageV1::Zero => ObservedCoverage::Zero,
            ProductionArgumentCoverageV1::Components { first, end } => {
                ObservedCoverage::Components(first, end)
            }
            ProductionArgumentCoverageV1::Parameter(physical) => {
                ObservedCoverage::Parameter(physical.slot())
            }
            ProductionArgumentCoverageV1::WithinAtomicParameter(physical) => {
                ObservedCoverage::Within(physical.slot())
            }
        };
        nodes.push(ObservedArgumentNode {
            source: node.source_argument(),
            adjusted: node.adjusted_argument(),
            ty: node.semantic_type(),
            path: node.source_path().to_vec(),
            local: node
                .local_binding()
                .map(|(local, path)| (local, path.to_vec())),
            coverage,
        });
        Ok(())
    })?;
    Ok(nodes)
}

#[test]
fn complete_argument_view_preserves_nested_zero_nodes_and_packed_expanded_identity() {
    use ProductionArgumentProjectionV1::{ArrayIndex as A, Field as F};
    for shape in [
        ArgumentTupleShape::Mixed,
        ArgumentTupleShape::AllZero,
        ArgumentTupleShape::EmptyUnit,
        ArgumentTupleShape::EmptyTuple,
    ] {
        let mut source_nodes = None;
        for expanded in [false, true] {
            let owner = argument_view_owner(expanded, shape);
            let source_hash = *owner
                .semantic_ssa
                .source_semantic()
                .semantic_sha256()
                .as_bytes();
            for root in [1, 2] {
                let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
                let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
                budget.reserve_storage(23).unwrap();
                let nodes = owner
                    .with_checked_arguments_v1(
                        SemanticFunctionIdV1::from_index(root),
                        SemanticFunctionIdV1::from_index(0),
                        &mut budget,
                        |view| {
                            assert_eq!(view.association().correspondence_owner().index(), root);
                            assert_eq!(view.source_arguments()?.len(), 2);
                            let original = &owner.semantic_ssa.source_semantic().functions()[0];
                            view.visit_nodes(|node| {
                                if let Some(mapped) = node.adjusted() {
                                    assert!(std::ptr::eq(mapped.abi(),
                                        &original.abi().adjusted_arguments()[mapped.ordinal() as usize]));
                                }
                                let expected_ignored = node.local_binding().and_then(|(local, _)|
                                    owner.correspondence.ignored_parameter_bindings.iter().find(|row|
                                        row.correspondence_owner.index() == root
                                        && row.semantic_function.index() == 0
                                        && row.semantic_local == local));
                                match (node.ignored_local_binding(), expected_ignored) {
                                    (Some(actual), Some(expected)) => assert!(std::ptr::eq(actual, expected)),
                                    (None, None) => (),
                                    _ => panic!("node must retain exactly the applicable owner-qualified ignored row"),
                                }
                                Ok(())
                            })?;
                            let adjusted = view.adjusted_arguments()?;
                            let first = view.physical(0)?;
                            let second = view.physical(1)?;
                            let collected = collect_argument_nodes(view)?;
                            for mapped in adjusted {
                                assert!(std::ptr::eq(
                                    mapped.abi(),
                                    &original.abi().adjusted_arguments()[mapped.ordinal() as usize]
                                ));
                                assert_eq!(
                                    mapped.source_ownership(),
                                    SemanticSourceArgumentOwnershipV1::ByValue
                                );
                            }
                            assert!(
                                view.ignored_local(SemanticLocalIdV1::from_index(1))?
                                    .is_some()
                            );
                            let count = if shape == ArgumentTupleShape::Mixed {
                                3
                            } else {
                                0
                            };
                            for slot in 0..count {
                                let parameter = view.physical(slot)?.unwrap();
                                assert_eq!(parameter.slot(), slot);
                                assert_eq!(parameter.ty(), &Type::Scalar(ScalarType::U32));
                                let body = owner
                                    .executable
                                    .module()
                                    .function(view.association().kernel_ir_function())
                                    .unwrap();
                                assert_eq!(
                                    parameter.value(),
                                    body.body.as_ref().unwrap().parameters[slot]
                                );
                                assert!(std::ptr::eq(
                                    parameter.ty(),
                                    &body.signature.parameters[slot]
                                ));
                                match parameter.trace() {
                                    ProductionArgumentTraceV1::Direct(row) => {
                                        assert!(expanded && slot == 2);
                                        let expected = owner
                                            .correspondence
                                            .parameter_bindings
                                            .iter()
                                            .find(|row| {
                                                row.correspondence_owner.index() == root
                                                    && row.semantic_function.index() == 0
                                                    && row.kernel_ir_value == parameter.value()
                                            })
                                            .unwrap();
                                        assert!(std::ptr::eq(row, expected));
                                    }
                                    ProductionArgumentTraceV1::Component(row) => {
                                        assert!(!expanded || slot < 2);
                                        let expected = owner
                                            .correspondence
                                            .parameter_component_bindings
                                            .iter()
                                            .find(|row| {
                                                row.correspondence_owner.index() == root
                                                    && row.semantic_function.index() == 0
                                                    && row.kernel_ir_value == parameter.value()
                                            })
                                            .unwrap();
                                        assert!(std::ptr::eq(row, expected));
                                    }
                                }
                            }
                            assert_eq!(first, view.physical(0)?);
                            assert_eq!(second, view.physical(1)?);
                            assert!(view.physical(count)?.is_none());
                            Ok(collected)
                        },
                    )
                    .unwrap();
                assert_eq!(budget.storage(), 23);
                let receiver = nodes
                    .iter()
                    .filter(|node| node.source == 0)
                    .collect::<Vec<_>>();
                assert_eq!(
                    receiver
                        .iter()
                        .map(|node| (node.path.clone(), node.ty))
                        .collect::<Vec<_>>(),
                    vec![
                        (vec![F(0), F(0)], UNIT),
                        (vec![F(0), F(1)], UNIT),
                        (vec![F(0)], SemanticTypeIdV1::from_index(5)),
                        (vec![F(1), A(0)], UNIT),
                        (vec![F(1), A(1)], UNIT),
                        (vec![F(1)], SemanticTypeIdV1::from_index(6)),
                        (vec![F(2)], SemanticTypeIdV1::from_index(7)),
                        (vec![], ZERO),
                    ]
                );
                assert!(receiver.iter().all(
                    |node| node.coverage == ObservedCoverage::Zero && node.adjusted == Some(0)
                ));
                let tuple = nodes
                    .iter()
                    .filter(|node| node.source == 1)
                    .collect::<Vec<_>>();
                let mut expected = Vec::new();
                if shape == ArgumentTupleShape::Mixed {
                    expected.extend([
                        (vec![F(0), F(0)], U32, ObservedCoverage::Parameter(0)),
                        (vec![F(0), F(1)], UNIT, ObservedCoverage::Zero),
                        (vec![F(0), F(2)], U32, ObservedCoverage::Parameter(1)),
                        (vec![F(0)], PAIR, ObservedCoverage::Components(0, 2)),
                    ]);
                }
                if matches!(
                    shape,
                    ArgumentTupleShape::Mixed | ArgumentTupleShape::AllZero
                ) {
                    let prefix = F(u32::from(shape == ArgumentTupleShape::Mixed));
                    expected.extend(receiver.iter().map(|node| {
                        let mut path = vec![prefix];
                        path.extend(&node.path);
                        (path, node.ty, ObservedCoverage::Zero)
                    }));
                    expected.push(if shape == ArgumentTupleShape::Mixed {
                        (vec![F(2)], U32, ObservedCoverage::Parameter(2))
                    } else {
                        (vec![F(1)], UNIT, ObservedCoverage::Zero)
                    });
                }
                expected.push((
                    vec![],
                    TUPLE,
                    if shape == ArgumentTupleShape::Mixed {
                        ObservedCoverage::Components(0, 3)
                    } else {
                        ObservedCoverage::Zero
                    },
                ));
                assert_eq!(
                    tuple
                        .iter()
                        .map(|node| (&node.path, node.ty, &node.coverage))
                        .collect::<Vec<_>>(),
                    expected
                        .iter()
                        .map(|(path, ty, coverage)| (path, *ty, coverage))
                        .collect::<Vec<_>>()
                );
                let envelope = tuple.last().unwrap();
                assert_eq!(
                    (envelope.adjusted, envelope.ty, envelope.path.as_slice()),
                    (None, TUPLE, &[][..])
                );
                assert_eq!(
                    envelope.local,
                    (!expanded).then_some((SemanticLocalIdV1::from_index(2), vec![]))
                );
                assert_eq!(
                    envelope.coverage,
                    if shape == ArgumentTupleShape::Mixed {
                        ObservedCoverage::Components(0, 3)
                    } else {
                        ObservedCoverage::Zero
                    }
                );
                for node in &tuple[..tuple.len() - 1] {
                    let F(field) = node.path[0] else {
                        panic!("outer source tuple requires a field");
                    };
                    assert_eq!(node.adjusted, Some(field + 1));
                    let (local, path) = node.local.as_ref().unwrap();
                    assert_eq!(
                        local.index(),
                        if expanded {
                            if shape == ArgumentTupleShape::Mixed {
                                4 - field
                            } else {
                                3 - field
                            }
                        } else {
                            2
                        }
                    );
                    assert_eq!(path.as_slice(), &node.path[usize::from(expanded)..]);
                }
                if shape == ArgumentTupleShape::Mixed {
                    let physical = tuple
                        .iter()
                        .filter_map(|node| match node.coverage {
                            ObservedCoverage::Parameter(slot) => Some((node.path.clone(), slot)),
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    assert_eq!(
                        physical,
                        [
                            (vec![F(0), F(0)], 0),
                            (vec![F(0), F(2)], 1),
                            (vec![F(2)], 2)
                        ]
                    );
                }
                let inert = nodes
                    .into_iter()
                    .map(|mut node| {
                        node.local = None;
                        node
                    })
                    .collect::<Vec<_>>();
                if let Some(expected) = &source_nodes {
                    assert_eq!(&inert, expected);
                } else {
                    source_nodes = Some(inert);
                }
            }
            assert_eq!(
                *owner
                    .semantic_ssa
                    .source_semantic()
                    .semantic_sha256()
                    .as_bytes(),
                source_hash
            );
        }
    }
}

#[test]
fn complete_argument_view_callback_and_budget_errors_restore_live_floor() {
    let owner = argument_view_owner(true, ArgumentTupleShape::Mixed);
    let root = SemanticFunctionIdV1::from_index(1);
    let function = SemanticFunctionIdV1::from_index(0);
    let run = |budget: &mut ArgumentBudgetV1<'_>, fail, entered: &mut bool| {
        owner.with_checked_arguments_v1(root, function, budget, |view| {
            *entered = true;
            let retained = view.budget.storage();
            assert!(retained > 23);
            let work_before = view.budget.work();
            collect_argument_nodes(view)?;
            let work_after = view.budget.work();
            assert!(work_after > work_before);
            let peak = view.budget.peak_storage();
            collect_argument_nodes(view)?;
            assert_eq!(view.budget.work() - work_after, work_after - work_before);
            assert_eq!(view.budget.peak_storage(), peak);
            assert_eq!(view.budget.storage(), retained);
            if fail {
                let error =
                    view.visit_nodes(|_| Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch));
                assert!(matches!(
                    error,
                    Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
                ));
                assert_eq!(view.budget.storage(), retained);
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            Ok(())
        })
    };
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(23).unwrap();
    run(&mut budget, false, &mut false).unwrap();
    let (work_limit, peak) = (budget.work(), budget.peak_storage());
    assert_eq!(budget.storage(), 23);
    assert!(matches!(
        run(&mut budget, true, &mut false),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
    assert_eq!(budget.storage(), 23);
    for (w, s, failure) in [
        (work_limit, peak, None),
        (work_limit - 1, peak, Some("work")),
        (work_limit, peak - 1, Some("storage")),
    ] {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(w);
        let mut budget = ArgumentBudgetV1::new(&mut work, s);
        budget.reserve_storage(23).unwrap();
        let mut entered = false;
        let result = run(&mut budget, false, &mut entered);
        match failure {
            None => result.unwrap(),
            Some("work") => {
                assert!(
                    entered,
                    "late work exhaustion must occur in the consumer callback"
                );
                assert!(matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Work(_)
                        )
                    )
                ));
            }
            Some("storage") => {
                assert!(matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Storage(_)
                        )
                    )
                ));
            }
            _ => unreachable!(),
        }
        assert_eq!(budget.storage(), 23);
    }
    let mut storage_work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut storage_budget = ArgumentBudgetV1::new(&mut storage_work, peak);
    storage_budget.reserve_storage(23).unwrap();
    owner
        .with_checked_arguments_v1(root, function, &mut storage_budget, |view| {
            let retained = view.budget.storage();
            // Model a live consumer payload competing with a new traversal's scratch.
            let payload_bytes = peak - retained;
            view.budget.reserve_storage(payload_bytes)?;
            let payload = vec![0_u8; payload_bytes];
            let mut visited = 0;
            let error = view.visit_nodes(|_| {
                visited += 1;
                Ok(())
            });
            let Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage(error),
            )) = error
            else {
                panic!("a live consumer payload must leave insufficient traversal scratch");
            };
            assert_eq!(visited, 0);
            assert_eq!(error.limit(), peak);
            assert!(error.actual() > peak);
            assert_eq!(view.budget.failed_storage(), Some(error.actual()));
            assert_eq!(view.budget.storage(), peak);
            assert_eq!(view.budget.peak_storage(), peak);
            drop(payload);
            view.budget.release_storage(payload_bytes)?;
            collect_argument_nodes(view)?;
            assert_eq!(view.budget.storage(), retained);
            Ok(())
        })
        .unwrap();
    assert_eq!(storage_budget.storage(), 23);
    let mut called = false;
    let unknown = owner.with_checked_arguments_v1(
        SemanticFunctionIdV1::from_index(0),
        function,
        &mut budget,
        |_| {
            called = true;
            Ok(())
        },
    );
    assert!(matches!(
        unknown,
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
    assert!(!called);
    assert_eq!(budget.storage(), 23);
}
