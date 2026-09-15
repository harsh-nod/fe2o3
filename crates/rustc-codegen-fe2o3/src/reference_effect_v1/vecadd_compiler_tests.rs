//! Unchanged vecadd through the ordinary importer and production ranked join.
//! Preparation and live conditional coverage are deliberately separate gates.
use super::*;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::{Compiler, Config};
use rustc_session::config::Input;

#[path = "output_slice_v1/compiler_tests/harness.rs"]
mod harness;

struct Probe {
    require_live_report: bool,
    completed: bool,
}

impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::File(harness::source_root().join("examples/vecadd/src/lib.rs"));
    }

    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        use crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1 as Projection;
        use crate::production_reference_effect_join_v2::ProductionReferenceEffectJoinErrorV2 as Join;
        use fe2o3_pliron::{
            ProductionRankedCompileErrorV1 as CompileV1,
            ProductionRankedCompileErrorV2 as CompileV2, ProductionRankedOperationV1 as Op,
            ProductionRankedTerminatorV1 as Term, ProductionRankedValueV1 as Value,
            ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1,
            ProductionSemanticSsaLimitsV1, ProductionSemanticSsaOwnerV1,
            ProductionSessionErrorV1 as Session,
        };
        let target = crate::production_target_v1::RetainedProductionTargetV1::authenticate_live_before_collection(tcx).unwrap();
        let partitions = tcx.collect_and_partition_mono_items(());
        let closure = crate::collector::collect_authenticated_kernel_closure_v1(
            tcx,
            partitions.codegen_units,
            false,
            target,
        )
        .unwrap();
        let typed = closure.rederive_typed_descriptor_roots(tcx).unwrap();
        let imported = crate::collector::construct_production_semantic_mir_v1(
            tcx,
            closure,
            crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
        )
        .unwrap();
        let [binding] = imported.reference_effect_bindings.as_slice() else {
            panic!("one authenticated original reference binding")
        };
        assert_eq!(binding.logical_kernel_name, "vecadd");
        let typed = crate::compiler_descriptor::order_typed_descriptor_roots_by_semantic_v1(
            typed,
            &imported.semantic_mir,
        )
        .unwrap();
        let inputs = typed
            .iter()
            .map(|root| {
                crate::production_ranked_projection_v1::ProductionRankedRootInputV1::new(
                    root.logical_name(),
                    root.kernel_binding_bytes(),
                    root.source_launch().unwrap(),
                )
            })
            .collect::<Vec<_>>();
        let mir = ProductionSemanticMirOwnerV1::try_new(
            imported.semantic_mir,
            ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap();
        let ssa =
            ProductionSemanticSsaOwnerV1::try_new(mir, ProductionSemanticSsaLimitsV1::default())
                .unwrap();
        ssa.verify_replay().unwrap();
        let (result, observed) = crate::production_reference_effect_join_v2::guard_observation_tests_v1::with_observation(||
            crate::production_ranked_projection_v1::project_and_verify_ranked_semantic_mir_with_contexts_v1(
                ssa, &inputs, &imported.reference_effect_bindings, Some(&imported.kernel_contexts),
            ));
        assert!(
            observed.paired,
            "original vecadd pairing: {:?}",
            result.as_ref().err()
        );
        let kernel = observed.kernel.expect("prepared unchanged vecadd");
        let invocations = kernel.blocks()[0]
            .operations()
            .iter()
            .filter_map(|op| match op {
                Op::InvocationIndex {
                    result,
                    dimension: 0,
                    launch_extent: 0,
                } => Some(Value::Local(*result)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [invocation] = invocations.as_slice() else {
            panic!("one dynamic axis-zero invocation")
        };
        let extents = kernel.blocks()[0]
            .operations()
            .iter()
            .filter_map(|op| match op {
                Op::ViewInSpace {
                    shape,
                    dynamic_extents,
                    ..
                } if shape == &[0] => {
                    let [extent] = dynamic_extents.as_slice() else {
                        panic!("rank-one extent")
                    };
                    assert!(matches!(extent, Value::Argument(_)));
                    Some(*extent)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(extents.len(), 3);
        assert_eq!(
            extents
                .iter()
                .copied()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            3
        );
        for extent in &extents {
            assert!(kernel.blocks().iter().any(|block| matches!(block.terminator(), Term::IndexLessThan { lhs, rhs, .. } if lhs == invocation && rhs == extent)), "original runtime length guard");
        }
        let (gpu, cpu) = observed.values.expect("source-derived stored value");
        use fe2o3_pliron::{
            ProductionSemanticBinaryOpV2 as Binary, ProductionSemanticExpressionV2 as Expression,
        };
        let (
            Expression::Binary {
                operation: Binary::Add,
                lhs: gl,
                rhs: gr,
                ..
            },
            Expression::Binary {
                operation: Binary::Add,
                lhs: cl,
                rhs: cr,
                ..
            },
        ) = (&gpu, &cpu)
        else {
            panic!("original stored add: {gpu:?}, {cpu:?}");
        };
        for (gpu, cpu) in [(gl, cl), (gr, cr)] {
            let (Expression::Load(gpu), Expression::Load(cpu)) = (gpu.as_ref(), cpu.as_ref())
            else {
                panic!("original direct load leaves: {gpu:?}, {cpu:?}");
            };
            assert_eq!(gpu, cpu, "source read association, not memory equality");
            assert_eq!(
                gpu.scalar,
                fe2o3_pliron::ProductionSemanticScalarTypeV2::Float { bits: 32 }
            );
            assert_eq!(
                gpu.read_mode,
                fe2o3_pliron::ProductionSemanticReadModeV2::UnorderedVolatile
            );
            assert_eq!(gpu.indices.as_ref(), [*invocation]);
        }
        assert_ne!(gl, gr, "two distinct original input reads");
        match result {
            Err(Projection::ReferenceEffectJoin(Join::Compile(CompileV2::Pipeline(
                CompileV1::Session(Session::RankedOwnership(error)),
            )))) => {
                let report = error.report();
                let coverage = report.conditional_prefix_coverage().unwrap_or_else(|| {
                    panic!(
                        "live production conditional coverage: {:?}; prepared control flow: {:?}",
                        report.conditional_prefix_failure(),
                        kernel
                            .blocks()
                            .iter()
                            .map(|block| block.terminator())
                            .collect::<Vec<_>>()
                    )
                });
                use fe2o3_kernel_analysis::{
                    ConditionalPrefixConditionV1 as Condition,
                    ConditionalPrefixExtentSourceV1 as Source,
                    ConditionalPrefixHostBindingObligationV1 as Binding,
                };
                let [guard] = coverage.source_guard_dnf() else {
                    panic!("one original conjunction")
                };
                let branch_count = kernel
                    .blocks()
                    .iter()
                    .filter(|block| matches!(block.terminator(), Term::IndexLessThan { .. }))
                    .count();
                assert!(branch_count >= 3);
                assert_eq!(guard.len(), branch_count, "retain repeated bounds checks");
                assert!(guard.iter().all(|atom| atom.less_than()));
                assert_eq!(
                    guard
                        .iter()
                        .map(|atom| atom.branch())
                        .collect::<std::collections::BTreeSet<_>>()
                        .len(),
                    branch_count
                );
                let mut expected_conditions = Vec::new();
                let mut expected_bindings = Vec::new();
                let mut actual_roles = Vec::new();
                let guarded_extents = guard
                    .iter()
                    .map(|atom| atom.extent())
                    .collect::<std::collections::BTreeSet<_>>();
                assert_eq!(guarded_extents.len(), 3);
                for extent in guarded_extents {
                    actual_roles.push((
                        extent.allocation_origin(),
                        extent.noalias_class(),
                        extent.source(),
                        extent == coverage.output(),
                    ));
                    expected_bindings
                        .push(Binding::RankedViewToPhysicalAllocationExtent { extent });
                    if extent != coverage.output() {
                        expected_conditions.push(Condition::OutputExtentAtMostInput {
                            output: coverage.output(),
                            input: extent,
                        });
                    }
                }
                let mut expected_roles = kernel.blocks()[0]
                    .operations()
                    .iter()
                    .filter_map(|op| match op {
                        Op::ViewInSpace {
                            element_width,
                            writable,
                            shape,
                            dynamic_extents,
                            allocation_origin,
                            noalias_class,
                            memory_space,
                            ..
                        } => {
                            assert_eq!(*element_width, 32);
                            assert_eq!(shape, &[0]);
                            assert_eq!(*memory_space, dialect_kernel::MemorySpaceAttr::Global);
                            let [Value::Argument(argument)] = dynamic_extents.as_slice() else {
                                panic!("original length")
                            };
                            Some((
                                *allocation_origin,
                                *noalias_class,
                                Source::RankedEntryArgument(*argument),
                                *writable,
                            ))
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                actual_roles.sort();
                expected_roles.sort();
                assert_eq!(actual_roles, expected_roles);
                let launch = coverage.launch();
                assert_eq!(launch.axis(), 0);
                assert_eq!(launch.declared_workitems(), 0);
                let layouts = kernel.blocks()[0]
                    .operations()
                    .iter()
                    .filter_map(|op| match op {
                        Op::ExecutionLayout {
                            grid_identity,
                            global_extents,
                            workgroup_extents,
                            ..
                        } => Some((*grid_identity, *global_extents, *workgroup_extents)),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                assert_eq!(layouts, [(launch.grid_identity(), [0, 1, 1], [64, 1, 1])]);
                expected_conditions.push(Condition::OutputExtentAtMostActualWorkitems {
                    output: coverage.output(),
                    launch,
                });
                expected_bindings.push(Binding::RankedLaunchToActualDispatch { launch });
                expected_conditions.sort();
                expected_bindings.sort();
                assert_eq!(coverage.conditions(), expected_conditions);
                assert_eq!(coverage.host_binding_obligations(), expected_bindings);
                assert!(!report.is_clean());
                assert!(!report.all_total_view_contracts_are_proved());
                assert_eq!(report.coverage_summary().total_view_proved(), 0);
                assert!(!coverage.proves_unconditional_total_view());
                assert!(!coverage.grants_launch_authority());
            }
            Err(Projection::ReferenceEffectJoin(Join::ProofRuntimeUnavailable { .. }))
                if !self.require_live_report =>
            {
                eprintln!("vecadd preparation only; protected proof runtime unavailable");
            }
            other => panic!(
                "vecadd live-report gate not satisfied: {:?}",
                other.as_ref().err()
            ),
        }
        self.completed = true;
        Compilation::Stop
    }
}

fn run(require_live_report: bool, name: &str) {
    let mut probe = Probe {
        require_live_report,
        completed: false,
    };
    if harness::run_probe_configured(
        "gfx942",
        "reference_effect_v1::vecadd_compiler_tests",
        name,
        "",
        &mut probe,
    ) {
        assert!(probe.completed);
    }
}

#[test]
#[ignore = "requires cached AMD core/device metadata"]
fn vecadd_actual_amdgpu_conditional_preparation() {
    run(false, "vecadd_actual_amdgpu_conditional_preparation");
}

#[test]
#[ignore = "requires cached AMD metadata and protected functional proof runtime"]
fn vecadd_actual_amdgpu_live_conditional_report() {
    run(true, "vecadd_actual_amdgpu_live_conditional_report");
}
