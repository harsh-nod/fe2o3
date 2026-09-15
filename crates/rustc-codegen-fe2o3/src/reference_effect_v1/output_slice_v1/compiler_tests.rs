//! Real registered combine source and its independently extracted GPU guard.
//! This is not a numerical proof or a successful complete export.
use super::*;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::{Compiler, Config};
use rustc_session::config::Input;
use rustc_span::FileName;

#[path = "compiler_tests/harness.rs"]
mod harness;

#[derive(Clone, Copy)]
enum Case {
    Original,
    FullProjection,
    WrongCoordinate,
    WrongWidth,
    MissingLastPoint,
}

fn source(case: Case) -> String {
    let directory = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/gfx950_advanced_systems/src")
        .canonicalize()
        .unwrap();
    let library = std::fs::read_to_string(directory.join("lib.rs")).unwrap();
    let mut reference = std::fs::read_to_string(directory.join("effect_reference.rs")).unwrap();
    match case {
        Case::Original | Case::FullProjection => {}
        Case::WrongCoordinate => {
            reference = reference.replace("output[point] =", "output[point ^ 1] =")
        }
        Case::WrongWidth => {
            let parameters = "rank0: &[f32],\n    rank1: &[f32],\n    output: &mut [f32]";
            assert_eq!(reference.matches(parameters).count(), 1);
            assert_eq!(reference.matches("rank0[point] + rank1[point]").count(), 1);
            reference = reference
                .replace(
                    parameters,
                    "rank0: &[f32],\n    rank1: &[f32],\n    output: &mut [f64]",
                )
                .replace(
                    "rank0[point] + rank1[point]",
                    "(rank0[point] + rank1[point]) as f64",
                )
        }
        Case::MissingLastPoint => reference = reference.replace("point < ELEMENTS", "point < 1023"),
    }
    assert!(
        library.contains("pub mod effect_reference;"),
        "source patch must be installed explicitly"
    );
    library
        .replace(
            "pub mod kernel;",
            &format!(
                "#[path = {:?}] pub mod kernel;",
                directory.join("kernel.rs")
            ),
        )
        .replace(
            "pub mod effect_reference;",
            &format!("pub mod effect_reference {{\n{reference}\n}}"),
        )
}

struct Probe {
    case: Case,
    completed: bool,
}

#[test]
fn output_slice_width_mutation_preserves_other_reference_functions() {
    let source = source(Case::WrongWidth);
    assert_eq!(source.matches("output: &mut [f64]").count(), 1);
    assert!(source.contains(
        "rank0: &[f32],\n    rank1: &[f32],\n    output: &mut [f64]"
    ));
    assert!(source.contains(
        "pub fn stage_gradient_shard_point_v1(point: usize, input: &[f32], output: &mut [f32])"
    ));
}

impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("output_slice_reference.rs".into()),
            input: source(self.case),
        };
    }
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        use crate::production_target_v1::RetainedProductionTargetV1;
        use crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2;
        use fe2o3_pliron::{
            ProductionSemanticMirLimitsV1, ProductionSemanticMirOwnerV1,
            ProductionSemanticSsaLimitsV1, ProductionSemanticSsaOwnerV1,
        };
        let target = RetainedProductionTargetV1::authenticate_live_before_collection(tcx).unwrap();
        let partitions = tcx.collect_and_partition_mono_items(());
        let result = crate::collector::collect_authenticated_kernel_closure_v1(
            tcx,
            partitions.codegen_units,
            false,
            target,
        );
        let closure = match self.case {
            Case::WrongCoordinate | Case::WrongWidth => {
                let error = match result {
                    Err(error) => error,
                    Ok(_) => panic!("mutated source reference must reject at registration"),
                };
                let expected = match self.case {
                    Case::WrongCoordinate => "not its exact source point coordinate",
                    Case::WrongWidth => "logical ABI mismatch",
                    _ => unreachable!(),
                };
                assert!(error.to_string().contains(expected), "{error}");
                self.completed = true;
                return Compilation::Stop;
            }
            Case::Original | Case::FullProjection | Case::MissingLastPoint => result
                .unwrap_or_else(|error| panic!("real explicit CPU source registration: {error}")),
        };
        let typed = closure.rederive_typed_descriptor_roots(tcx).unwrap();
        let imported = crate::collector::construct_production_semantic_mir_v1(
            tcx,
            closure,
            DebugSourceCaptureRequestV2::Disabled,
        )
        .unwrap();
        let [binding] = imported.reference_effect_bindings.as_slice() else {
            panic!("exactly one independent root/reference binding")
        };
        assert_eq!(
            binding.logical_kernel_name,
            "gfx950_combine_expert_ranks_v1"
        );
        assert_eq!(
            binding.effect_ir.relations[3],
            ReferenceArgumentRelationV1::InvocationDisjointOutputSlice1D {
                argument: 2,
                element: ReferenceScalarTypeV1::F32,
            }
        );
        let [write] = binding.observable_output_writes.as_ref() else {
            panic!("one source write")
        };
        assert_eq!(write.argument, 2);
        assert_eq!(
            write.coordinate,
            ReferenceOutputCoordinateV1::LogicalPoint(
                vec![ReferenceEffectExpressionV1::PointCoordinate { axis: 0 }].into_boxed_slice()
            )
        );
        assert_eq!(write.guard.clauses.len(), 1);
        assert_eq!(
            write.guard.clauses[0].atoms.len(),
            4,
            "three exact lengths plus active point"
        );
        assert_eq!(binding.effect_ir.resolved_bounds_checks_with_budget_v1(&mut ReferenceSymbolicWorkBudgetV2::default()).unwrap().len(), 3);
        let ReferenceEffectExpressionV1::Binary {
            operation: ReferenceBinaryOpV1::Add,
            lhs,
            rhs,
            checked: false,
        } = &write.rhs
        else {
            panic!("original ordered add")
        };
        assert!(matches!(
            **lhs,
            ReferenceEffectExpressionV1::InputLoad {
                reference_argument: 1,
                ..
            }
        ));
        assert!(matches!(
            **rhs,
            ReferenceEffectExpressionV1::InputLoad {
                reference_argument: 2,
                ..
            }
        ));
        let owner = tcx
            .hir_body_owners()
            .find(|id| tcx.item_name(id.to_def_id()).as_str() == "combine_expert_ranks_point_v1")
            .unwrap();
        let instance = Instance::mono(tcx, owner.to_def_id());
        assert_eq!(
            binding.reference.rustc_mir_body_sha256,
            crate::rustc_semantic_adapter_v1::rustc_mir_body_sha256_v1(tcx, instance)
        );
        let body = tcx.instance_mir(instance.def);
        assert!(body.basic_blocks.iter().flat_map(|b| &b.statements).any(|s|
            matches!(&s.kind, StatementKind::Assign(a) if matches!(a.1, Rvalue::RawPtr(rustc_middle::mir::RawPtrKind::FakeForPtrMetadata, _)))));
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
        let semantic = ProductionSemanticMirOwnerV1::try_new(
            imported.semantic_mir,
            ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap();
        let ssa = ProductionSemanticSsaOwnerV1::try_new(
            semantic,
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
        ssa.verify_replay().unwrap();
        // Observe the actual source extractor and its actual bijection. The
        // test cannot turn later numerical/frame failures into proof receipts.
        let (result,observation)=crate::production_reference_effect_join_v2::guard_observation_tests_v1::with_observation(||
            crate::production_ranked_projection_v1::project_and_verify_ranked_semantic_mir_with_contexts_v1(
                ssa,&inputs,&imported.reference_effect_bindings,Some(&imported.kernel_contexts),
            ));
        let [binding] = imported.reference_effect_bindings.as_slice() else {
            panic!("one original CPU binding")
        };
        let effects = observation.effects.unwrap_or_else(|| {
            panic!(
                "actual GPU guard extractor not reached: {}",
                result
                    .as_ref()
                    .err()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "unexpected complete result".into())
            )
        });
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].guard.clauses.len(), 1);
        assert_eq!(
            effects[0].guard.clauses[0].atoms.len(),
            4,
            "gpu_guard={:?}; cpu_guard={:?}; production_paired={}; production_error={:?}",
            effects[0].guard,
            binding.observable_output_writes.first().map(|write| &write.guard),
            observation.paired,
            result.as_ref().err().map(ToString::to_string),
        );
        let paired = crate::reference_effect_bijection_v1::establish_reference_effect_bijection_v1(
            &binding.observable_output_writes,
            &effects,
        );
        match self.case {
            Case::Original | Case::FullProjection => {
                paired.unwrap();
                let kernel = observation.kernel.as_ref().expect("final source-remapped recipe");
                crate::production_reference_effect_join_v2::guard_observation_tests_v1::assert_runtime_extent_guards(kernel, 3, 1024);
                assert!(
                    observation.paired,
                    "real production bijection must complete"
                );
                let (gpu, reference) = observation.values.expect("prepared original output value");
                use fe2o3_pliron::{ProductionSemanticBinaryOpV2, ProductionSemanticExpressionV2};
                let (
                    ProductionSemanticExpressionV2::Binary {
                        operation: gpu_op, scalar: gpu_scalar, overflow: gpu_overflow,
                        lhs: gpu_lhs, rhs: gpu_rhs,
                    },
                    ProductionSemanticExpressionV2::Binary {
                        operation: cpu_op, scalar: cpu_scalar, overflow: cpu_overflow,
                        lhs: cpu_lhs, rhs: cpu_rhs,
                    },
                ) = (&gpu, &reference) else {
                    panic!("source-derived ordered sums: GPU={gpu:?}, CPU={reference:?}");
                };
                assert_eq!(*gpu_op, ProductionSemanticBinaryOpV2::Add);
                assert_eq!(*cpu_op, ProductionSemanticBinaryOpV2::Add);
                assert_eq!((gpu_scalar, gpu_overflow), (cpu_scalar, cpu_overflow));
                for (gpu, cpu) in [(gpu_lhs, cpu_lhs), (gpu_rhs, cpu_rhs)] {
                    crate::production_reference_effect_join_v2::guard_observation_tests_v1::assert_proved_load_selection(gpu, cpu);
                }
                if matches!(self.case, Case::FullProjection) {
                    let program = result.unwrap_or_else(|error| {
                        panic!("original combined source must complete full ranked projection: {error}")
                    });
                    assert_eq!(program.roots().len(), 1);
                } else if let Err(error) = result {
                    eprintln!("exact guard paired; remaining full-proof stage: {error}");
                }
            }
            Case::MissingLastPoint => {
                assert!(!observation.paired);
                assert!(matches!(paired,Err(crate::reference_effect_bijection_v1::ReferenceEffectBijectionErrorV1::GuardMismatch {..})));
                assert!(matches!(result,Err(crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1::ReferenceEffectJoin(
                    crate::production_reference_effect_join_v2::ProductionReferenceEffectJoinErrorV2::EffectBijection(
                        crate::reference_effect_bijection_v1::ReferenceEffectBijectionErrorV1::GuardMismatch {..}
                    )
                ))));
            }
            _ => unreachable!(),
        }
        self.completed = true;
        Compilation::Stop
    }
}

fn run(case: Case, name: &str) {
    let mut probe = Probe {
        case,
        completed: false,
    };
    if harness::run_probe("gfx950", name, &mut probe) {
        assert!(probe.completed);
    }
}

#[test]
#[ignore = "requires complete cached AMD core/device metadata"]
fn output_slice_reference_actual_amdgpu_import_exact_guard() {
    run(
        Case::Original,
        "output_slice_reference_actual_amdgpu_import_exact_guard",
    );
}

#[test]
#[ignore = "requires complete cached AMD core/device metadata"]
fn output_slice_reference_actual_amdgpu_full_projection() {
    run(
        Case::FullProjection,
        "output_slice_reference_actual_amdgpu_full_projection",
    );
}

#[test]
#[ignore = "requires complete cached AMD core/device metadata"]
fn output_slice_reference_actual_amdgpu_missing_last_point_guard() {
    run(
        Case::MissingLastPoint,
        "output_slice_reference_actual_amdgpu_missing_last_point_guard",
    );
}

#[test]
#[ignore = "requires complete cached AMD core/device metadata"]
fn output_slice_reference_actual_amdgpu_wrong_coordinate() {
    run(
        Case::WrongCoordinate,
        "output_slice_reference_actual_amdgpu_wrong_coordinate",
    );
}

#[test]
#[ignore = "requires complete cached AMD core/device metadata"]
fn output_slice_reference_actual_amdgpu_wrong_width() {
    run(
        Case::WrongWidth,
        "output_slice_reference_actual_amdgpu_wrong_width",
    );
}
