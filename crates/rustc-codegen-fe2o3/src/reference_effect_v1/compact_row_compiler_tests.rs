//! Original stage-gradient Rust/reference pair through registration, import,
//! replay and the ranked join. A prepared request is not a proof receipt.
use super::*;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::{Compiler, Config};
use rustc_session::config::Input;
use rustc_span::FileName;

#[path = "output_slice_v1/compiler_tests/harness.rs"]
mod harness;

#[derive(Clone, Copy)]
enum Case {
    Original,
    MissingLane,
    SmallerLane,
    CheckedArithmetic,
    WrongWidth,
}

fn source(case: Case) -> String {
    let directory = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/gfx950_advanced_systems/src")
        .canonicalize()
        .unwrap();
    let library = std::fs::read_to_string(directory.join("lib.rs")).unwrap();
    let mut reference = std::fs::read_to_string(directory.join("effect_reference.rs")).unwrap();
    let replace = |source: String, from: &str, to: &str| {
        assert_eq!(
            source.matches(from).count(),
            1,
            "exact fixture mutation anchor"
        );
        source.replace(from, to)
    };
    reference = match case {
        Case::Original => reference,
        Case::MissingLane => replace(reference, "&& lane < crate::MUON_ELEMENTS", ""),
        Case::SmallerLane => replace(reference, "&& lane < crate::MUON_ELEMENTS", "&& lane < 15"),
        Case::CheckedArithmetic => replace(
            reference,
            "batch.wrapping_mul(crate::MUON_ELEMENTS).wrapping_add(lane)",
            "batch * crate::MUON_ELEMENTS + lane",
        ),
        Case::WrongWidth => replace(
            replace(
                reference,
                "pub fn stage_gradient_shard_point_v1(point: usize, input: &[f32], output: &mut [f32])",
                "pub fn stage_gradient_shard_point_v1(point: usize, input: &[f32], output: &mut [f64])",
            ),
            "output[coordinate] = input[coordinate];",
            "output[coordinate] = input[coordinate] as f64;",
        ),
    };
    assert_eq!(library.matches("pub mod kernel;").count(), 1);
    assert_eq!(library.matches("pub mod effect_reference;").count(), 1);
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

impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("compact_row_original_stage.rs".into()),
            input: source(self.case),
        };
    }

    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        use crate::production_ranked_projection_v1::ProductionRankedProjectionErrorV1 as ProjectionError;
        use crate::production_reference_effect_join_v2::ProductionReferenceEffectJoinErrorV2 as JoinError;
        use crate::production_target_v1::RetainedProductionTargetV1;
        use crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2;
        use fe2o3_mir_model::semantic_mir_v1::{AdmittedInertSemanticMirV1, SemanticMirLimitsV1};
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
            Case::MissingLane | Case::CheckedArithmetic | Case::WrongWidth => {
                let error = match result {
                    Err(error) => error,
                    Ok(_) => panic!("mutated source reference must reject before import"),
                };
                let expected = match self.case {
                    Case::MissingLane => "guarded compact row",
                    Case::CheckedArithmetic => {
                        "reference checked overflow field requires exact unsigned constant operands"
                    }
                    Case::WrongWidth => "logical ABI mismatch",
                    _ => unreachable!(),
                };
                assert!(error.to_string().contains(expected), "{error}");
                self.completed = true;
                return Compilation::Stop;
            }
            _ => result
                .unwrap_or_else(|error| panic!("original CPU reference registration: {error}")),
        };
        let typed = closure.rederive_typed_descriptor_roots(tcx).unwrap();
        let imported = crate::collector::construct_production_semantic_mir_v1(
            tcx,
            closure,
            DebugSourceCaptureRequestV2::Disabled,
        )
        .unwrap();
        let [binding] = imported.reference_effect_bindings.as_slice() else {
            panic!("one stage root/reference")
        };
        assert_eq!(
            binding.logical_kernel_name,
            "gfx950_stage_gradient_shard_v1"
        );
        assert!(matches!(
            binding.effect_ir.relations[2],
            ReferenceArgumentRelationV1::ExclusivePrimitiveOutputSlice1D {
                argument: 1,
                element: ReferenceScalarTypeV1::F32,
                ..
            }
        ));
        let [write] = binding.observable_output_writes.as_ref() else {
            panic!("one original write")
        };
        let ReferenceOutputCoordinateV1::CompactRowsUsize1D(mapping) = write.coordinate else {
            panic!("compact row descriptor")
        };
        assert_eq!(
            (mapping.axis(), mapping.divisor(), mapping.stride()),
            (0, 64, 16)
        );
        assert_eq!(write.guard.clauses.len(), 1);
        assert_eq!(write.guard.clauses[0].atoms.len(), 4);
        let ReferenceEffectExpressionV1::InputLoad {
            reference_argument: 1,
            index,
        } = &write.rhs
        else {
            panic!("original exact read")
        };
        assert_eq!(
            **index,
            mapping
                .expression(&mut ReferenceSymbolicWorkBudgetV2::default())
                .unwrap()
        );
        assert_eq!(binding.effect_ir.resolved_bounds_checks_with_budget_v1(&mut ReferenceSymbolicWorkBudgetV2::default()).unwrap().len(), 2);
        let cpu = tcx
            .hir_body_owners()
            .find(|id| tcx.item_name(id.to_def_id()).as_str() == "stage_gradient_shard_point_v1")
            .unwrap();
        let cpu = Instance::mono(tcx, cpu.to_def_id());
        assert_eq!(
            binding.reference.rustc_mir_body_sha256,
            crate::rustc_semantic_adapter_v1::rustc_mir_body_sha256_v1(tcx, cpu)
        );
        assert_eq!(
            binding.effect_ir_sha256,
            binding.effect_ir.canonical_sha256_v1()
        );
        let decoded = AdmittedInertSemanticMirV1::decode_current_production_canonical(
            imported.semantic_mir.canonical_encoding(),
            SemanticMirLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(
            decoded.canonical_encoding(),
            imported.semantic_mir.canonical_encoding()
        );
        let typed = crate::compiler_descriptor::order_typed_descriptor_roots_by_semantic_v1(
            typed, &decoded,
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
            decoded,
            ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap();
        let ssa =
            ProductionSemanticSsaOwnerV1::try_new(mir, ProductionSemanticSsaLimitsV1::default())
                .unwrap();
        ssa.verify_replay().unwrap();
        let (result, observation) = crate::production_reference_effect_join_v2::guard_observation_tests_v1::with_observation(||
            crate::production_ranked_projection_v1::project_and_verify_ranked_semantic_mir_with_contexts_v1(ssa, &inputs, &imported.reference_effect_bindings, Some(&imported.kernel_contexts)));
        let effects = observation.effects.unwrap_or_else(|| {
            panic!(
                "GPU coordinate extraction not reached: {:?}",
                result.as_ref().err()
            )
        });
        assert_eq!(effects.len(), 1);
        assert_eq!(
            effects[0].coordinate, write.coordinate,
            "independent original GPU index normalizes to the same descriptor"
        );
        if matches!(self.case, Case::SmallerLane) {
            assert!(!observation.paired);
            assert_ne!(effects[0].guard, write.guard);
            assert!(matches!(result, Err(ProjectionError::ReferenceEffectJoin(JoinError::EffectBijection(crate::reference_effect_bijection_v1::ReferenceEffectBijectionErrorV1::GuardMismatch { .. })))));
        } else {
            assert_eq!(effects[0].guard, write.guard);
            assert!(observation.paired);
            crate::production_reference_effect_join_v2::guard_observation_tests_v1::assert_runtime_extent_guards(
                observation.kernel.as_ref().expect("final source-remapped recipe"), 2, 256,
            );
            let (gpu, reference) = observation.values.expect("prepared original output value");
            crate::production_reference_effect_join_v2::guard_observation_tests_v1::assert_proved_load_selection(
                &gpu, &reference,
            );
            match result {
                Ok(program) => assert_eq!(program.roots().len(), 1),
                Err(ProjectionError::ReferenceEffectJoin(JoinError::ProofRuntimeUnavailable {
                    ..
                })) => {
                    // This exact error is after preparation, bounds, read-root
                    // placement and source remapping. It is not a proof pass.
                    eprintln!("compact row request prepared; functional proof runtime unavailable");
                }
                Err(error) => {
                    panic!("original compact row must finish request preparation: {error}")
                }
            }
        }
        self.completed = true;
        Compilation::Stop
    }
}

fn run(case: Case, cpu: &'static str, name: &str) {
    let mut probe = Probe {
        case,
        completed: false,
    };
    if harness::run_probe_configured(
        cpu,
        "reference_effect_v1::compact_row_compiler_tests",
        name,
        "kernel-stage-gradient-shard",
        &mut probe,
    ) {
        assert!(probe.completed);
    }
}

#[test]
#[ignore = "requires complete cached AMD core/device metadata"]
fn compact_row_actual_amdgpu_gfx950_original() {
    run(
        Case::Original,
        "gfx950",
        "compact_row_actual_amdgpu_gfx950_original",
    );
}
#[test]
#[ignore = "requires complete cached AMD core/device metadata"]
fn compact_row_actual_amdgpu_gfx942_original() {
    run(
        Case::Original,
        "gfx942",
        "compact_row_actual_amdgpu_gfx942_original",
    );
}
#[test]
#[ignore = "requires complete cached AMD core/device metadata"]
fn compact_row_actual_amdgpu_missing_lane() {
    run(
        Case::MissingLane,
        "gfx950",
        "compact_row_actual_amdgpu_missing_lane",
    );
}
#[test]
#[ignore = "requires complete cached AMD core/device metadata"]
fn compact_row_actual_amdgpu_smaller_lane_frame_mismatch() {
    run(
        Case::SmallerLane,
        "gfx950",
        "compact_row_actual_amdgpu_smaller_lane_frame_mismatch",
    );
}
#[test]
#[ignore = "requires complete cached AMD core/device metadata"]
fn compact_row_actual_amdgpu_checked_assert_not_premise() {
    run(
        Case::CheckedArithmetic,
        "gfx950",
        "compact_row_actual_amdgpu_checked_assert_not_premise",
    );
}
#[test]
#[ignore = "requires complete cached AMD core/device metadata"]
fn compact_row_actual_amdgpu_wrong_width() {
    run(
        Case::WrongWidth,
        "gfx950",
        "compact_row_actual_amdgpu_wrong_width",
    );
}
