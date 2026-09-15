//! Real macro registration -> complete collection -> source replay -> canonical
//! import. This unit proves reference ownership, not a numerical/export receipt.
use super::*;
use crate::reference_effect_v1::*;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::{Compiler, Config};
use rustc_session::config::Input;
use rustc_span::FileName;

mod harness;

const SOURCE: &str = r#"
#![no_std]
use fe2o3_device::{kernel, ExclusiveReadWrite, Global, KernelContext, ReadOnly};

pub fn cpu_point(point: usize, input: &[u32], output: &mut [u32]) {
    if input.len() != 64 || output.len() != 64 { return; }
    if point < 64 { output[point] = input[point]; }
}

macro_rules! root {
    ($name:ident) => {
        #[kernel(typed, reference = cpu_point,
            launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1]))]
        pub fn $name(context: KernelContext<'_>, input: Global<'_, u32, ReadOnly>,
            mut output: Global<'_, u32, ExclusiveReadWrite>) {
            let point = context.invocation().index_1d().get();
            if input.len() != 64 || output.len() != 64 { return; }
            if point < 64 {
                let Some(value) = input.load(point) else { return; };
                let _stored = output.store(point, value);
            }
        }
    }
}
root!(exclusive_primary);
root!(exclusive_other);
"#;

#[derive(Clone, Copy)]
enum Case {
    Original,
    Float,
    Atomic,
    WrongWidth,
    WrongCoordinate,
}

fn source(case: Case) -> String {
    match case {
        Case::Original => SOURCE.into(),
        Case::Float => SOURCE.replace("u32", "f32"),
        Case::WrongWidth => SOURCE
            .replace("output: &mut [u32]", "output: &mut [u64]")
            .replace(
                "output[point] = input[point]",
                "output[point] = input[point] as u64",
            ),
        Case::WrongCoordinate => SOURCE.replace("output[point] =", "output[point ^ 1] ="),
        Case::Atomic => SOURCE
            .replace(
                "Global<'_, u32, ExclusiveReadWrite>",
                "Global<'_, u32, fe2o3_device::AtomicReadWrite<fe2o3_device::SystemScope>>",
            )
            .replace(
                "let _stored = output.store(point, value);",
                "let _unused = (output, value);",
            ),
    }
}

struct Probe {
    case: Case,
    completed: bool,
}

fn binding_error(result: Result<(), CollectError>, expected: &str) {
    let error = result.expect_err("mutated exclusive ownership must reject");
    assert!(error.to_string().contains(expected), "{error}");
}

impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("exclusive_reference_source.rs".into()),
            input: source(self.case),
        };
    }

    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        use crate::production_target_v1::RetainedProductionTargetV1;
        use crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2;
        use fe2o3_mir_model::semantic_mir_v1::{AdmittedInertSemanticMirV1, SemanticMirLimitsV1};
        let partitions = tcx.collect_and_partition_mono_items(());
        let registered = kernel_roots(tcx, partitions.codegen_units).unwrap();
        assert_eq!(registered.len(), 2);
        for root in &registered {
            assert!(matches!(
                root.reference_effect_binding,
                Some(RootReferenceBindingV1::Pending(_))
            ));
            assert!(
                root.kernel_context_contract
                    .as_ref()
                    .unwrap()
                    .authenticated_source
                    .is_none()
            );
        }
        let target = RetainedProductionTargetV1::authenticate_live_before_collection(tcx).unwrap();
        let result =
            collect_authenticated_kernel_closure_v1(tcx, partitions.codegen_units, false, target);
        let closure = match self.case {
            Case::Atomic | Case::WrongWidth | Case::WrongCoordinate => {
                let error = match result {
                    Err(error) => error,
                    Ok(_) => panic!("invalid source must fail during reference finalization"),
                };
                let expected = match self.case {
                    Case::Atomic => "mutable carrier is not the exact exclusive role",
                    Case::WrongWidth => "logical ABI mismatch",
                    Case::WrongCoordinate => "not its exact source point coordinate",
                    _ => unreachable!(),
                };
                assert!(error.to_string().contains(expected), "{error}");
                self.completed = true;
                return Compilation::Stop;
            }
            Case::Original | Case::Float => result.unwrap(),
        };
        let functions = &closure.collection.functions;
        let roots = functions
            .iter()
            .enumerate()
            .filter(|(_, function)| function.is_kernel_entry())
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        assert_eq!(roots.len(), 2);
        validate_collected_bindings_v1(tcx, functions).unwrap();
        let inventory = inventory(tcx, functions).unwrap();
        let scalar = if matches!(self.case, Case::Float) {
            ReferenceScalarTypeV1::F32
        } else {
            ReferenceScalarTypeV1::U32
        };
        let mut carried = Vec::new();
        for &index in &roots {
            let root = &functions[index];
            let source = root
                .kernel_context_contract
                .as_ref()
                .unwrap()
                .authenticated_source
                .as_ref()
                .unwrap();
            let proof = sources(tcx, root, &inventory).unwrap();
            let root_signature = signature(tcx, root.instance);
            let physical = root_signature.inputs()[1];
            let Some(witness) = proof.for_argument_v1(root.instance, 1, physical) else {
                panic!("exact carrier")
            };
            assert!(proof.for_argument_v1(root.instance, 0, physical).is_none());
            assert!(
                proof
                    .for_argument_v1(root.instance, 1, root_signature.inputs()[0])
                    .is_none()
            );
            let other = &functions[roots.iter().copied().find(|other| *other != index).unwrap()];
            assert!(proof.for_argument_v1(other.instance, 1, physical).is_none());
            let binding = root.reference_effect_binding.as_ref().unwrap();
            assert_eq!(binding.kernel, function_identity_v1(tcx, root.instance));
            assert_eq!(
                binding.effect_ir.relations[2],
                ReferenceArgumentRelationV1::ExclusivePrimitiveOutputSlice1D {
                    argument: 1,
                    element: scalar,
                    source: witness,
                }
            );
            let helper = inventory[&source.logical_helper_identity];
            let helper_signature = signature(tcx, helper.instance);
            assert_ne!(helper_signature.inputs()[2], physical);
            let [write] = binding.observable_output_writes.as_ref() else {
                panic!("one source write")
            };
            assert_eq!(write.argument, 1);
            assert_eq!(
                write.coordinate,
                ReferenceOutputCoordinateV1::LogicalPoint(
                    vec![ReferenceEffectExpressionV1::PointCoordinate { axis: 0 }]
                        .into_boxed_slice()
                )
            );
            assert_eq!(write.guard.clauses.len(), 1);
            assert_eq!(write.guard.clauses[0].atoms.len(), 3);
            assert!(matches!(
                &write.rhs,
                ReferenceEffectExpressionV1::InputLoad {
                    reference_argument: 1,
                    ..
                }
            ));
            carried.push(binding.clone());
        }
        assert_ne!(
            carried[0].effect_ir_sha256, carried[1].effect_ir_sha256,
            "identical reference IR still carries different authenticated source roots"
        );
        let original = roots[0];
        let other = roots[1];
        let mut changed = functions.to_vec();
        changed[original]
            .kernel_context_contract
            .as_mut()
            .unwrap()
            .authenticated_source = None;
        binding_error(
            validate_collected_bindings_v1(tcx, &changed),
            "has not completed authentication",
        );
        let mut changed = functions.to_vec();
        changed[original].kernel_context_contract =
            functions[other].kernel_context_contract.clone();
        binding_error(
            validate_collected_bindings_v1(tcx, &changed),
            "different physical root",
        );
        let mut changed = functions.to_vec();
        changed[original].role = CollectedFunctionRole::InternalHelper;
        binding_error(
            validate_collected_bindings_v1(tcx, &changed),
            "not a physical kernel root",
        );
        let mut changed = functions.to_vec();
        changed[original].kernel_binding = functions[other].kernel_binding;
        binding_error(
            validate_collected_bindings_v1(tcx, &changed),
            "changed source, role, carrier, or argument",
        );
        let mut changed = functions.to_vec();
        let helper = changed[original]
            .kernel_context_contract
            .as_ref()
            .unwrap()
            .authenticated_source
            .as_ref()
            .unwrap()
            .logical_helper_identity;
        let helper = changed
            .iter()
            .position(|function| {
                function_identity_v1(tcx, function.instance).function_sha256 == helper
            })
            .unwrap();
        changed[helper].role = CollectedFunctionRole::DeviceFfiExport;
        binding_error(
            validate_collected_bindings_v1(tcx, &changed),
            "logical helper has a different role",
        );
        let mut changed = functions.to_vec();
        let source = changed[original]
            .kernel_context_contract
            .as_mut()
            .unwrap()
            .authenticated_source
            .as_mut()
            .unwrap();
        source.physical_argument_count += 1;
        binding_error(
            validate_collected_bindings_v1(tcx, &changed),
            "argument counts",
        );
        let mut changed = functions.to_vec();
        let source = changed[original]
            .kernel_context_contract
            .as_mut()
            .unwrap()
            .authenticated_source
            .as_mut()
            .unwrap();
        source.kernel_marker_identity[0] ^= 1;
        binding_error(
            validate_collected_bindings_v1(tcx, &changed),
            "different source root",
        );
        for mutation in 0..4 {
            let mut changed = functions.to_vec();
            let binding = changed[original].reference_effect_binding.as_mut().unwrap();
            match mutation {
                0 => binding.effect_ir.relations[2] = carried[1].effect_ir.relations[2],
                1 => {
                    binding.effect_ir.relations[2] =
                        ReferenceArgumentRelationV1::InvocationDisjointOutputSlice1D {
                            argument: 1,
                            element: ReferenceScalarTypeV1::U32,
                        }
                }
                2 | 3 => {
                    let ReferenceArgumentRelationV1::ExclusivePrimitiveOutputSlice1D {
                        argument,
                        element,
                        ..
                    } = &mut binding.effect_ir.relations[2]
                    else {
                        unreachable!()
                    };
                    if mutation == 2 {
                        *argument = 0;
                    } else {
                        *element = ReferenceScalarTypeV1::I32;
                    }
                }
                _ => unreachable!(),
            }
            binding.effect_ir_sha256 = binding.effect_ir.canonical_sha256_v1();
            let expected = match mutation {
                0 | 3 => "changed source, role, carrier, or argument",
                1 => "removed or substituted",
                2 => "different physical argument",
                _ => unreachable!(),
            };
            binding_error(validate_collected_bindings_v1(tcx, &changed), expected);
        }
        let cpu = tcx
            .hir_body_owners()
            .find(|id| tcx.item_name(id.to_def_id()).as_str() == "cpu_point")
            .unwrap();
        let cpu = Instance::mono(tcx, cpu.to_def_id());
        let body = tcx.instance_mir(cpu.def);
        assert_eq!(body.basic_blocks.iter().filter(|block|
            matches!(&block.terminator().kind, rustc_middle::mir::TerminatorKind::Assert { msg, .. }
                if matches!(msg.as_ref(), rustc_middle::mir::AssertMessage::BoundsCheck { .. }))
        ).count(), 2, "retain both original CPU bounds assertions");
        for binding in &carried {
            assert_eq!(binding.reference, function_identity_v1(tcx, cpu));
        }
        let imported = construct_production_semantic_mir_v1(
            tcx,
            closure,
            DebugSourceCaptureRequestV2::Disabled,
        )
        .unwrap();
        assert_eq!(
            imported.reference_effect_bindings.as_slice(),
            carried.as_slice()
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
        for binding in imported.reference_effect_bindings.as_slice() {
            assert!(decoded.functions().iter().any(|function| {
                binding.kernel.has_function_identity_v1(function.identity())
                    && function.role()
                        == fe2o3_mir_model::semantic_mir_v1::SemanticFunctionRoleV1::KernelRoot
            }));
        }
        let owner = fe2o3_pliron::ProductionSemanticMirOwnerV1::try_new(
            decoded,
            fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap();
        owner.verify_equivalence().unwrap();
        self.completed = true;
        Compilation::Stop
    }
}

fn run(case: Case, cpu: &'static str, name: &str) {
    let mut probe = Probe {
        case,
        completed: false,
    };
    if harness::run_probe(cpu, name, &mut probe) {
        assert!(probe.completed);
    }
}

#[test]
#[ignore = "requires complete cached AMD core/device metadata"]
fn exclusive_reference_actual_amdgpu_gfx942() {
    run(
        Case::Original,
        "gfx942",
        "exclusive_reference_actual_amdgpu_gfx942",
    );
}
#[test]
#[ignore = "requires complete cached AMD core/device metadata"]
fn exclusive_reference_actual_amdgpu_gfx950() {
    run(
        Case::Float,
        "gfx950",
        "exclusive_reference_actual_amdgpu_gfx950",
    );
}
#[test]
#[ignore = "requires complete cached AMD core/device metadata"]
fn exclusive_reference_actual_amdgpu_atomic_role() {
    run(
        Case::Atomic,
        "gfx950",
        "exclusive_reference_actual_amdgpu_atomic_role",
    );
}
#[test]
#[ignore = "requires complete cached AMD core/device metadata"]
fn exclusive_reference_actual_amdgpu_wrong_width() {
    run(
        Case::WrongWidth,
        "gfx950",
        "exclusive_reference_actual_amdgpu_wrong_width",
    );
}
#[test]
#[ignore = "requires complete cached AMD core/device metadata"]
fn exclusive_reference_actual_amdgpu_wrong_coordinate() {
    run(
        Case::WrongCoordinate,
        "gfx950",
        "exclusive_reference_actual_amdgpu_wrong_coordinate",
    );
}
