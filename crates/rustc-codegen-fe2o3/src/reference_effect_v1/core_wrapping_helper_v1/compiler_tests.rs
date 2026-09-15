use super::*;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::{Compiler, Config};
use rustc_session::config::Input;
use rustc_span::FileName;

#[allow(dead_code)]
#[path = "../output_slice_v1/compiler_tests/harness.rs"]
mod harness;

const SOURCE: &str = r#"
#![no_std]
use fe2o3_device::{kernel, ExclusiveReadWrite, Global, KernelContext, ReadOnly};

pub fn cpu_point(point: usize, input: &[u32], output: &mut [u32]) {
    if input.len() != 64 || output.len() != 64 { return; }
    if point < 64 {
        let a = input[point].wrapping_add(3);
        let b = a.wrapping_sub(5);
        output[point] = b.wrapping_mul(7);
    }
}

#[kernel(typed, reference = cpu_point,
    launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1]))]
pub fn wrapping_reference(context: KernelContext<'_>, input: Global<'_, u32, ReadOnly>,
    mut output: Global<'_, u32, ExclusiveReadWrite>) {
    let point = context.invocation().index_1d().get();
    if input.len() != 64 || output.len() != 64 { return; }
    if point < 64 {
        let Some(value) = input.load(point) else { return; };
        let value = value.wrapping_add(3).wrapping_sub(5).wrapping_mul(7);
        let _stored = output.store(point, value);
    }
}
"#;

#[derive(Clone, Copy)]
enum Case {
    U32,
    Usize,
    ConstantExtent,
    Impostor,
    UnsafeHelper,
    UnsafeRoot,
    OtherCore,
}

fn source(case: Case) -> String {
    let mut source = SOURCE.to_owned();
    let call = "input[point].wrapping_add(3)";
    assert_eq!(source.matches(call).count(), 1);
    match case {
        Case::U32 => {}
        Case::ConstantExtent => {
            let (cpu, gpu) = source.split_once("#[kernel").unwrap();
            source = format!("{}#[kernel{gpu}", cpu.replace("!= 64", "!= 8_usize * 8"));
        }
        Case::Usize => {
            source = source.replace(call, "point.wrapping_add(3)");
            source = source.replace(
                "output[point] = b.wrapping_mul(7);",
                "output[point] = b.wrapping_mul(7) as u32;",
            );
            source = source.replace(
                "let Some(value) = input.load(point) else { return; };\n        let value = value.wrapping_add(3).wrapping_sub(5).wrapping_mul(7);",
                "let value = point.wrapping_add(3).wrapping_sub(5).wrapping_mul(7) as u32;",
            );
        }
        Case::Impostor => {
            source = source.replace(call, "wrapping_add(input[point], 3)");
            source.push_str("\n#[inline(never)] fn wrapping_add(a: u32, b: u32) -> u32 { nested(a, b) }\n#[inline(never)] fn nested(a: u32, b: u32) -> u32 { a + b }\n");
        }
        Case::UnsafeHelper => {
            source = source.replace(call, "wrapping_add(input[point], 3)");
            source.push_str(
                "\n#[inline(never)] fn wrapping_add(a: u32, b: u32) -> u32 { unsafe { a + b } }\n",
            );
        }
        Case::UnsafeRoot => {
            source = source.replace(call, "unsafe { input[point].wrapping_add(3) }")
        }
        Case::OtherCore => source = source.replace(call, "input[point].saturating_add(3)"),
    }
    source
}

struct Probe {
    case: Case,
    completed: bool,
}

fn resolved<'tcx>(tcx: TyCtxt<'tcx>, function: &Operand<'tcx>) -> Instance<'tcx> {
    let Operand::Constant(constant) = function else {
        panic!("direct call")
    };
    let TyKind::FnDef(definition, arguments) = constant.const_.ty().kind() else {
        panic!("FnDef")
    };
    Instance::try_resolve(
        tcx,
        TypingEnv::fully_monomorphized(),
        *definition,
        arguments,
    )
    .unwrap()
    .unwrap()
}

fn formals(operation: ReferenceBinaryOpV1) -> ReferenceEffectExpressionV1 {
    ReferenceEffectExpressionV1::Binary {
        operation,
        lhs: Box::new(ReferenceEffectExpressionV1::KernelScalarArgument { argument: 0 }),
        rhs: Box::new(ReferenceEffectExpressionV1::KernelScalarArgument { argument: 1 }),
        checked: false,
    }
}

#[test]
fn reference_core_wrapping_summary_vocabulary_stays_closed() {
    for (name, expected) in [
        ("wrapping_add", ReferenceBinaryOpV1::Add),
        ("wrapping_sub", ReferenceBinaryOpV1::Subtract),
        ("wrapping_mul", ReferenceBinaryOpV1::Multiply),
    ] {
        assert_eq!(operation(name), Some(expected));
    }
    for name in [
        "add",
        "unchecked_add",
        "overflowing_add",
        "saturating_add",
        "wrapping_shl",
        "wrapping_add_impostor",
    ] {
        assert_eq!(operation(name), None);
    }
}

#[test]
fn reference_core_wrapping_fixture_mutations_do_not_change_gpu_body() {
    let original = source(Case::U32);
    let gpu = original.split("#[kernel").nth(1).unwrap();
    for case in [
        Case::ConstantExtent,
        Case::Impostor,
        Case::UnsafeHelper,
        Case::UnsafeRoot,
        Case::OtherCore,
    ] {
        let changed = source(case);
        let actual = changed.split("#[kernel").nth(1).unwrap();
        assert!(actual.starts_with(gpu));
    }
}

#[test]
fn reference_core_wrapping_usize_fixture_keeps_u32_memory() {
    let changed = source(Case::Usize);
    assert!(!changed.contains("Word"));
    assert!(changed.contains("input: &[u32], output: &mut [u32]"));
    assert!(changed.contains("Global<'_, u32, ReadOnly>"));
    assert!(changed.contains("Global<'_, u32, ExclusiveReadWrite>"));
    assert_eq!(changed.matches("point.wrapping_add(3)").count(), 2);
    assert_eq!(changed.matches(".wrapping_sub(5)").count(), 2);
    assert_eq!(changed.matches(".wrapping_mul(7) as u32").count(), 2);
    assert!(!changed.contains("input[point]"));
    assert!(!changed.contains("input.load(point)"));
    assert!(changed.contains("output[point] = b.wrapping_mul(7) as u32;"));
    assert!(changed.contains("output.store(point, value)"));
}

fn check_body_mutations<'tcx>(tcx: TyCtxt<'tcx>, reviewed: &ReviewedWrapping<'tcx>) {
    use rustc_middle::mir::{BasicBlock, Local, Statement};
    let body = tcx.instance_mir(reviewed.helper.def);
    let expected = formals(reviewed.operation);
    assert_eq!(reviewed.summary(tcx).unwrap(), expected);
    let entry = BasicBlock::from_usize(0);
    let reject = |body: &Body<'tcx>| {
        let error = reviewed.summary_from_body(tcx, body).unwrap_err();
        assert_eq!(
            error.to_string(),
            "reviewed core wrapping body does not match its exact scalar summary"
        );
    };
    let mut changed = body.clone();
    changed.arg_count = 1;
    reject(&changed);
    let mut changed = body.clone();
    changed.local_decls[Local::from_usize(1)].ty = tcx.types.bool;
    reject(&changed);
    let mut changed = body.clone();
    changed.basic_blocks_mut()[entry].is_cleanup = true;
    reject(&changed);
    let mut changed = body.clone();
    changed.basic_blocks_mut()[entry].terminator = None;
    reject(&changed);
    let mut changed = body.clone();
    changed.basic_blocks_mut()[entry]
        .statements
        .push(Statement::new(
            body.basic_blocks[entry].terminator().source_info,
            StatementKind::StorageDead(Local::from_usize(1)),
        ));
    reject(&changed);
    let mut changed = body.clone();
    if body.basic_blocks.len() == 1 {
        let StatementKind::Assign(assignment) =
            &mut changed.basic_blocks_mut()[entry].statements[0].kind
        else {
            panic!("binary")
        };
        let Rvalue::BinaryOp(_, operands) = &mut assignment.1 else {
            panic!("binary")
        };
        std::mem::swap(&mut operands.0, &mut operands.1);
    } else {
        let TerminatorKind::Call { args, .. } =
            &mut changed.basic_blocks_mut()[entry].terminator_mut().kind
        else {
            panic!("intrinsic")
        };
        args.swap(0, 1);
    }
    reject(&changed);
    let wrong_operation = ReviewedWrapping {
        helper: reviewed.helper,
        element: reviewed.element,
        operation: if reviewed.operation == ReferenceBinaryOpV1::Add {
            ReferenceBinaryOpV1::Subtract
        } else {
            ReferenceBinaryOpV1::Add
        },
    };
    assert!(wrong_operation.summary(tcx).is_err());
    assert_eq!(reviewed.summary(tcx).unwrap(), expected);
}

fn check_typed_calls<'tcx>(
    tcx: TyCtxt<'tcx>,
    cpu: Instance<'tcx>,
) -> Vec<ReferenceFunctionIdentityV1> {
    let body = tcx.instance_mir(cpu.def);
    let mut identities = Vec::new();
    for (block, data) in body.basic_blocks.iter_enumerated() {
        let TerminatorKind::Call {
            func,
            args,
            destination,
            ..
        } = &data.terminator().kind
        else {
            continue;
        };
        let helper = resolved(tcx, func);
        let reviewed = authenticate(tcx, helper).expect("original core source proof");
        let identity = function_identity_v1(tcx, helper);
        check_body_mutations(tcx, &reviewed);
        let call = |body: &Body<'tcx>, arguments: &[Spanned<Operand<'tcx>>]| {
            lower_safe_scalar_helper_call_v2(
                tcx,
                cpu,
                ReferenceHelperCallSiteV2 {
                    body,
                    block: block.as_usize(),
                    statement: data.statements.len() as u32,
                    destination: *destination,
                },
                func,
                arguments,
            )
        };
        let assignment = call(body, args).unwrap();
        assert!(
            matches!(&assignment.value, ReferenceValueV1::SafeHelperCall { helper: actual, summary, .. }
            if *actual == identity && **summary == formals(reviewed.operation))
        );
        let error = call(body, &args[..1]).unwrap_err();
        assert!(
            error.to_string().contains("argument count disagrees"),
            "{error}"
        );
        let (Operand::Copy(place) | Operand::Move(place)) = &args[0].node else {
            panic!("scalar local argument")
        };
        let mut changed = body.clone();
        changed.local_decls[place.local].ty = tcx.types.bool;
        let error = call(&changed, args).unwrap_err();
        assert!(
            error.to_string().contains("has type 'bool', expected"),
            "{error}"
        );
        let mut changed = body.clone();
        changed.local_decls[destination.local].ty = tcx.types.bool;
        let error = call(&changed, args).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("return destination type disagrees"),
            "{error}"
        );
        assert_eq!(
            identity,
            function_identity_v1(tcx, helper),
            "mutations never change live MIR"
        );
        identities.push(identity);
    }
    assert_eq!(identities.len(), 3, "retain all three original core calls");
    identities
}

impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("reference_core_wrapping.rs".into()),
            input: source(self.case),
        };
    }

    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let cpu = tcx
            .hir_body_owners()
            .find(|id| tcx.item_name(id.to_def_id()).as_str() == "cpu_point")
            .unwrap();
        let cpu = Instance::mono(tcx, cpu.to_def_id());
        assert!(
            authenticate(tcx, cpu).is_none(),
            "root never acquires core authority"
        );
        if matches!(self.case, Case::Impostor | Case::UnsafeHelper) {
            let local = tcx
                .hir_body_owners()
                .find(|id| tcx.item_name(id.to_def_id()).as_str() == "wrapping_add")
                .unwrap();
            assert!(authenticate(tcx, Instance::mono(tcx, local.to_def_id())).is_none());
        }
        let identities = if matches!(self.case, Case::U32 | Case::Usize | Case::ConstantExtent) {
            check_typed_calls(tcx, cpu)
        } else {
            Vec::new()
        };
        let target = crate::production_target_v1::RetainedProductionTargetV1::authenticate_live_before_collection(tcx).unwrap();
        let partitions = tcx.collect_and_partition_mono_items(());
        let result = crate::collector::collect_authenticated_kernel_closure_v1(
            tcx,
            partitions.codegen_units,
            false,
            target,
        );
        let expected_error = match self.case {
            Case::U32 | Case::Usize | Case::ConstantExtent => None,
            Case::Impostor => Some("nested safe helper calls are unsupported"),
            Case::UnsafeHelper | Case::UnsafeRoot => Some("contains a user-provided unsafe block"),
            Case::OtherCore => Some("must be local so unsafe-block absence is authenticated"),
        };
        if let Some(expected) = expected_error {
            let error = match result {
                Err(error) => error,
                Ok(_) => panic!("negative reference must reject"),
            };
            assert!(error.to_string().contains(expected), "{error}");
            self.completed = true;
            return Compilation::Stop;
        }
        let imported = crate::collector::construct_production_semantic_mir_v1(
            tcx,
            result.unwrap(),
            crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
        )
        .unwrap();
        let [binding] = imported.reference_effect_bindings.as_slice() else {
            panic!("one registered reference")
        };
        assert_eq!(binding.reference, function_identity_v1(tcx, cpu));
        let scalar = if matches!(self.case, Case::Usize) {
            ReferenceScalarTypeV1::Usize
        } else {
            ReferenceScalarTypeV1::U32
        };
        let helpers = binding
            .effect_ir
            .blocks
            .iter()
            .flat_map(|block| &block.assignments)
            .filter_map(|assignment| match &assignment.value {
                ReferenceValueV1::SafeHelperCall {
                    helper,
                    parameters,
                    result,
                    summary,
                    ..
                } => Some((helper, parameters, result, summary)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(helpers.len(), 3);
        for ((helper, parameters, result, summary), (identity, operation)) in
            helpers.into_iter().zip(identities.iter().zip([
                ReferenceBinaryOpV1::Add,
                ReferenceBinaryOpV1::Subtract,
                ReferenceBinaryOpV1::Multiply,
            ]))
        {
            assert_eq!(helper, identity);
            assert_eq!(parameters.as_ref(), &[scalar, scalar]);
            assert_eq!(*result, scalar);
            assert_eq!(**summary, formals(operation));
        }
        let [write] = binding.observable_output_writes.as_ref() else {
            panic!("one point write")
        };
        let mut rhs = if matches!(self.case, Case::Usize) {
            ReferenceEffectExpressionV1::PointCoordinate { axis: 0 }
        } else {
            ReferenceEffectExpressionV1::InputLoad {
                reference_argument: 1,
                index: Box::new(ReferenceEffectExpressionV1::PointCoordinate { axis: 0 }),
            }
        };
        for (operation, bits) in [
            (ReferenceBinaryOpV1::Add, 3),
            (ReferenceBinaryOpV1::Subtract, 5),
            (ReferenceBinaryOpV1::Multiply, 7),
        ] {
            rhs = ReferenceEffectExpressionV1::Binary {
                operation,
                lhs: Box::new(rhs),
                rhs: Box::new(ReferenceEffectExpressionV1::Constant(
                    ReferenceConstantV1::Scalar { scalar, bits },
                )),
                checked: false,
            };
        }
        if matches!(self.case, Case::Usize) {
            rhs = ReferenceEffectExpressionV1::Cast {
                kind: ReferenceCastKindV1::Integer,
                source: ReferenceScalarTypeV1::Usize,
                target: ReferenceScalarTypeV1::U32,
                operand: Box::new(rhs),
            };
        }
        assert_eq!(write.rhs, rhs);
        assert_eq!(write.guard.clauses.len(), 1);
        assert_eq!(
            write.guard.clauses[0].atoms.len(),
            3,
            "original length and point guards stay intact"
        );
        let decoded = fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1::decode_current_production_canonical(
            imported.semantic_mir.canonical_encoding(), fe2o3_mir_model::semantic_mir_v1::SemanticMirLimitsV1::default(),
        ).unwrap();
        assert_eq!(
            decoded.canonical_encoding(),
            imported.semantic_mir.canonical_encoding()
        );
        self.completed = true;
        Compilation::Stop
    }
}

fn run(case: Case, cpu: &'static str, name: &str) {
    let mut probe = Probe {
        case,
        completed: false,
    };
    if harness::run_probe_named(
        cpu,
        "reference_effect_v1::core_wrapping_helper_v1::compiler_tests",
        name,
        &mut probe,
    ) {
        assert!(probe.completed);
    }
}

#[test]
#[ignore = "requires complete cached AMD core/device metadata"]
fn reference_core_wrapping_actual_amdgpu_gfx942() {
    run(
        Case::U32,
        "gfx942",
        "reference_core_wrapping_actual_amdgpu_gfx942",
    );
}
#[test]
#[ignore = "requires complete cached AMD core/device metadata"]
fn reference_core_wrapping_actual_amdgpu_gfx950_usize() {
    run(
        Case::Usize,
        "gfx950",
        "reference_core_wrapping_actual_amdgpu_gfx950_usize",
    );
}
#[test]
#[ignore = "requires complete cached AMD core/device metadata"]
fn reference_core_wrapping_actual_amdgpu_constant_extent() {
    run(
        Case::ConstantExtent,
        "gfx950",
        "reference_core_wrapping_actual_amdgpu_constant_extent",
    );
}

#[test]
#[ignore = "requires complete cached AMD core/device metadata"]
fn reference_core_wrapping_actual_amdgpu_impostor() {
    run(
        Case::Impostor,
        "gfx950",
        "reference_core_wrapping_actual_amdgpu_impostor",
    );
}
#[test]
#[ignore = "requires complete cached AMD core/device metadata"]
fn reference_core_wrapping_actual_amdgpu_unsafe_helper() {
    run(
        Case::UnsafeHelper,
        "gfx950",
        "reference_core_wrapping_actual_amdgpu_unsafe_helper",
    );
}
#[test]
#[ignore = "requires complete cached AMD core/device metadata"]
fn reference_core_wrapping_actual_amdgpu_unsafe_root() {
    run(
        Case::UnsafeRoot,
        "gfx950",
        "reference_core_wrapping_actual_amdgpu_unsafe_root",
    );
}
#[test]
#[ignore = "requires complete cached AMD core/device metadata"]
fn reference_core_wrapping_actual_amdgpu_other_core() {
    run(
        Case::OtherCore,
        "gfx950",
        "reference_core_wrapping_actual_amdgpu_other_core",
    );
}
