use super::*;

#[test]
fn bounded_generated_call_graphs_match_existing_effect_classification() {
    let names = ["root", "a", "b", "c", "d", "e", "f"];
    let mut seed = 71_u32;
    for acyclic in [false, true] {
        for _ in 0..32 {
            let mut source = Module::new("generated-call-graph");
            for (ordinal, name) in names.iter().enumerate() {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let calls = (1..names.len())
                    .filter(|index| seed & (1 << index) != 0 && (!acyclic || *index > ordinal))
                    .map(|index| names[index])
                    .collect::<Vec<_>>();
                source
                    .functions
                    .push(function(name, &calls, (seed & 1) as usize));
            }
            source.kernels.push(Kernel::new(
                "root",
                "root",
                LaunchDomain::D1 {
                    x: LaunchExtent::Static(4),
                },
            ));
            let old = fe2o3_kernel_ir::analyze_interprocedural_effects_v1(&source).unwrap();
            let (owner, _) = admit(&source);
            let (inventory, _) = inventory(&owner);
            let (report, _) = report(&inventory);
            let mut work = Work::new(100_000);
            let mut budget = Budget::new(&mut work, 100_000);
            for (index, function) in source.functions.iter().enumerate() {
                let old = old.function(&function.id).unwrap();
                let expected = if !old.is_complete() {
                    Decision::Incomplete
                } else if old.is_complete_and_pure() {
                    Decision::CompleteEmpty
                } else {
                    Decision::CompleteNonempty
                };
                if acyclic {
                    assert_ne!(expected, Decision::Incomplete);
                }
                assert_eq!(
                    report
                        .decision(Function(index as u32), &mut budget)
                        .unwrap(),
                    expected
                );
            }
        }
    }
}

#[test]
fn nested_returns_restore_parent_paths_before_later_effects() {
    let mut source = module(&[
        ("root", &["helper"], 1),
        ("helper", &["leaf"], 1),
        ("leaf", &[], 1),
    ]);
    for function in &mut source.functions[..2] {
        function.body.as_mut().unwrap().blocks[0]
            .operations
            .push(Operation::effect_free(
                ValueDef::new(ValueId(21), Type::Scalar(ScalarType::U32)),
                OperationKind::Load {
                    pointer: ValueId(1),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ));
    }
    let (owner, _) = admit(&source);
    let (inventory, _) = inventory(&owner);
    let (report, _) = report(&inventory);
    let mut work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, 100_000);
    let mut stream = Vec::new();
    report
        .try_visit(Function(0), &mut budget, |event| {
            stream.push((event.function(), event.call_path().to_vec()));
            Ok::<_, Error>(())
        })
        .unwrap();
    assert_eq!(
        stream,
        vec![
            (Function(0), vec![]),
            (Function(1), vec![0]),
            (Function(2), vec![0, 1]),
            (Function(1), vec![0]),
            (Function(0), vec![])
        ]
    );
}

#[test]
fn reserved_calls_do_not_hide_later_calls_or_require_a_declared_body() {
    use fe2o3_kernel_ir::{Constant, F32MathFunction, F32MathImplementation, FloatOperation};
    for declared in [false, true] {
        for reads in [0, 1] {
            let mut source = module(&[
                ("root", &["reader", "reader"], 0),
                ("reader", &[], reads),
                ("builtin_only", &[], 0),
            ]);
            let float = FloatOperation::F32Math {
                function: F32MathFunction::Abs,
                implementation: F32MathImplementation::IeeeFabsV1,
                arguments: vec![ValueId(80)],
            };
            for ordinal in [0, 2] {
                let operations =
                    &mut source.functions[ordinal].body.as_mut().unwrap().blocks[0].operations;
                let at = usize::from(ordinal == 0);
                operations.insert(
                    at,
                    Operation::effect_free(
                        ValueDef::new(ValueId(80), Type::F32),
                        OperationKind::Constant(Constant::F32Bits(1.0_f32.to_bits())),
                    ),
                );
                operations.insert(at + 1, float.operation(ValueId(81)));
            }
            if declared {
                source.functions.push(float.declaration());
            } else {
                let mut work = Work::new(10_000_000);
                let mut budget = Budget::new(&mut work, 10_000_000);
                let error =
                    VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
                        &source,
                        &mut budget,
                    )
                    .unwrap_err();
                let fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV12::Verification(errors) =
                    error
                else {
                    panic!("unexpected admission error: {error}");
                };
                assert_eq!(errors.diagnostics().len(), 2);
                assert!(errors.diagnostics().iter().all(|diagnostic| {
                    diagnostic.code == fe2o3_kernel_ir::DiagnosticCode::UnknownCallee
                }));
                continue;
            }
            let (owner, _) = admit(&source);
            let (inventory, _) = inventory(&owner);
            let (report, _) = report(&inventory);
            let mut work = Work::new(100_000);
            let mut budget = Budget::new(&mut work, 100_000);
            assert_eq!(
                report.decision(Function(2), &mut budget).unwrap(),
                Decision::CompleteEmpty
            );
            assert_eq!(
                report.decision(Function(0), &mut budget).unwrap(),
                if reads == 0 {
                    Decision::CompleteEmpty
                } else {
                    Decision::CompleteNonempty
                }
            );
            let mut paths = Vec::new();
            report
                .try_visit(Function(0), &mut budget, |event| {
                    paths.push(event.call_path().to_vec());
                    Ok::<_, Error>(())
                })
                .unwrap();
            assert_eq!(
                paths,
                if reads == 0 {
                    vec![]
                } else {
                    vec![vec![0], vec![2]]
                }
            );
        }
    }
}

#[test]
fn repeated_branching_call_paths_exhaust_work_without_expanding_a_graph() {
    let names = (0..20).map(|i| format!("helper{i}")).collect::<Vec<_>>();
    let mut source = module(&[("root", &[&names[0]], 0)]);
    for (index, name) in names.iter().enumerate() {
        source.functions.push(if index + 1 < names.len() {
            function(name, &[&names[index + 1], &names[index + 1]], 0)
        } else {
            function(name, &[], 1)
        });
    }
    let (owner, graph_bytes) = admit(&source);
    let (inventory, inventory_bytes) = inventory(&owner);
    let (report, report_bytes) = report(&inventory);
    let floor = graph_bytes + inventory_bytes + report_bytes;
    let mut work = Work::new(10_000);
    let mut budget = Budget::new(&mut work, floor + 100_000);
    budget.reserve_storage(floor).unwrap();
    let mut visited = 0;
    let result = report.try_visit(Function(0), &mut budget, |event| {
        assert!(report.belongs_to(event.inventory()));
        visited += 1;
        Ok::<_, Error>(())
    });
    assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
    assert!(visited > 0 && visited < 1 << 19);
    assert_eq!(budget.storage(), floor);
}

#[test]
fn root_effects_do_not_contaminate_a_complete_empty_helper() {
    let (owner, _) = admit(&module(&[("root", &["empty"], 1), ("empty", &[], 0)]));
    let (inventory, _) = inventory(&owner);
    let (report, _) = report(&inventory);
    let mut work = Work::new(100_000);
    let mut budget = Budget::new(&mut work, 100_000);
    assert_eq!(
        report.decision(Function(0), &mut budget).unwrap(),
        Decision::CompleteNonempty
    );
    assert_eq!(
        report.decision(Function(1), &mut budget).unwrap(),
        Decision::CompleteEmpty
    );
    report
        .try_visit(Function(0), &mut budget, |event| {
            assert_eq!(event.function(), Function(0));
            assert!(event.call_path().is_empty());
            Ok::<_, Error>(())
        })
        .unwrap();
}
