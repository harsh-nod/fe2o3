use super::normalized::*;
use super::rustc_adapter::capture_instance_body_v2;
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def::DefKind;
use rustc_interface::interface::Compiler;
use rustc_middle::mir::{Operand, TerminatorKind};
use rustc_middle::ty::{Instance, TyCtxt, TyKind, TypingEnv};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

const FIXTURE_SOURCE: &str = r#"
#![feature(core_intrinsics)]
#![allow(dead_code, internal_features)]

enum Choice {
    First(u32),
    Second { value: u32 },
}

#[inline(never)]
fn invoke_once<F: FnOnce(u32) -> u32>(function: F, value: u32) -> u32 {
    function(value)
}

#[inline(never)]
fn observed(mut seed: u32, choice: Choice, pointer: *const u8) -> u64 {
    let pair = (seed, seed.wrapping_add(1));
    let closure = move |delta: u32| pair.0.wrapping_add(delta);
    let mut accumulator = 0u32;
    let mut index = 0u32;
    while index < 3 {
        accumulator = accumulator.wrapping_add(index);
        index = index.wrapping_add(1);
    }
    seed = invoke_once(closure, accumulator);
    let selected = match choice {
        Choice::First(value) => value,
        Choice::Second { value } => value.wrapping_add(1),
    };
    (seed as u64)
        .wrapping_add(selected as u64)
        .wrapping_add(pointer as usize as u64)
}

#[inline(never)]
fn observed_intrinsic(value: u32) -> u32 {
    core::intrinsics::black_box(value)
}
"#;

#[derive(Clone, Debug)]
struct DriverResults {
    observed: CapturedBodyV2,
    invoke_once: CapturedBodyV2,
    intrinsic: CapturedBodyV2,
    bounded_error: String,
}

#[derive(Default)]
struct CaptureCallbacks {
    results: Option<DriverResults>,
}

impl Callbacks for CaptureCallbacks {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let limits = CaptureLimitsV2::default();
        let observed_instance = Instance::mono(tcx, local_function(tcx, "observed"));
        let intrinsic_instance = Instance::mono(tcx, local_function(tcx, "observed_intrinsic"));
        let invoke_instance = resolved_direct_calls(tcx, observed_instance)
            .into_iter()
            .find(|instance| tcx.item_name(instance.def_id()).as_str() == "invoke_once")
            .expect("observed fixture must call a concrete invoke_once instance");

        self.results = Some(DriverResults {
            observed: capture_instance_body_v2(tcx, observed_instance, limits)
                .expect("capture observed MIR"),
            invoke_once: capture_instance_body_v2(tcx, invoke_instance, limits)
                .expect("capture monomorphized invoke_once MIR"),
            intrinsic: capture_instance_body_v2(tcx, intrinsic_instance, limits)
                .expect("capture intrinsic caller MIR"),
            bounded_error: capture_instance_body_v2(
                tcx,
                observed_instance,
                CaptureLimitsV2 {
                    max_blocks: 1,
                    ..limits
                },
            )
            .expect_err("a one-block bound must reject the observed fixture")
            .to_string(),
        });
        Compilation::Stop
    }
}

fn local_function(tcx: TyCtxt<'_>, name: &str) -> rustc_hir::def_id::DefId {
    tcx.iter_local_def_id()
        .find(|definition| {
            tcx.def_kind(definition.to_def_id()) == DefKind::Fn
                && tcx.item_name(definition.to_def_id()).as_str() == name
        })
        .unwrap_or_else(|| panic!("missing fixture function `{name}`"))
        .to_def_id()
}

fn resolved_direct_calls<'tcx>(tcx: TyCtxt<'tcx>, caller: Instance<'tcx>) -> Vec<Instance<'tcx>> {
    tcx.instance_mir(caller.def)
        .basic_blocks
        .iter()
        .filter_map(|block| {
            let TerminatorKind::Call { func, .. } = &block.terminator().kind else {
                return None;
            };
            let Operand::Constant(constant) = func else {
                return None;
            };
            let TyKind::FnDef(def_id, args) = constant.const_.ty().kind() else {
                return None;
            };
            Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), *def_id, args)
                .ok()
                .flatten()
        })
        .collect()
}

struct CompilerFixture {
    source: PathBuf,
    output: PathBuf,
}

impl CompilerFixture {
    fn create() -> Self {
        let stem = format!("fe2o3-mir-v2-{}", std::process::id());
        let source = std::env::temp_dir().join(format!("{stem}.rs"));
        let output = std::env::temp_dir().join(format!("{stem}.rmeta"));
        fs::write(&source, FIXTURE_SOURCE).expect("write MIR V2 fixture");
        Self { source, output }
    }
}

impl Drop for CompilerFixture {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.source);
        let _ = fs::remove_file(&self.output);
    }
}

fn compiler_results() -> DriverResults {
    static RESULTS: OnceLock<DriverResults> = OnceLock::new();
    RESULTS
        .get_or_init(|| {
            let fixture = CompilerFixture::create();
            let sysroot = Command::new("rustc")
                .args(["--print", "sysroot"])
                .output()
                .expect("query rustc sysroot");
            assert!(sysroot.status.success(), "rustc --print sysroot failed");
            let sysroot = String::from_utf8(sysroot.stdout).expect("UTF-8 rustc sysroot");
            let args = vec![
                "rustc".to_owned(),
                "--crate-name".to_owned(),
                "fe2o3_mir_v2_fixture".to_owned(),
                "--crate-type".to_owned(),
                "lib".to_owned(),
                "--edition".to_owned(),
                "2024".to_owned(),
                "--emit".to_owned(),
                "metadata".to_owned(),
                "-Zmir-opt-level=0".to_owned(),
                "-Coverflow-checks=off".to_owned(),
                "--sysroot".to_owned(),
                sysroot.trim().to_owned(),
                "-o".to_owned(),
                fixture.output.display().to_string(),
                fixture.source.display().to_string(),
            ];
            let mut callbacks = CaptureCallbacks::default();
            rustc_driver::run_compiler(&args, &mut callbacks);
            callbacks.results.expect("MIR V2 callback did not run")
        })
        .clone()
}

fn assignments(body: &CapturedBodyV2) -> impl Iterator<Item = (&PlaceV2, &RvalueV2)> {
    body.blocks
        .iter()
        .flat_map(|block| &block.statements)
        .filter_map(|statement| match &statement.kind {
            StatementKindV2::Assign { destination, value } => Some((destination, value)),
            _ => None,
        })
}

fn calls(body: &CapturedBodyV2) -> impl Iterator<Item = &TerminatorKindV2> {
    body.blocks
        .iter()
        .map(|block| &block.terminator.kind)
        .filter(|kind| matches!(kind, TerminatorKindV2::Call { .. }))
}

fn places(body: &CapturedBodyV2) -> Vec<&PlaceV2> {
    let mut places = Vec::new();
    for block in &body.blocks {
        for statement in &block.statements {
            match &statement.kind {
                StatementKindV2::Assign { destination, value } => {
                    places.push(destination);
                    push_rvalue_places(value, &mut places);
                }
                StatementKindV2::SetDiscriminant { place, .. }
                | StatementKindV2::Deinit { place }
                | StatementKindV2::Retag { place, .. }
                | StatementKindV2::PlaceMention { place } => places.push(place),
                StatementKindV2::Intrinsic(IntrinsicStatementV2::CopyNonOverlapping {
                    source,
                    destination,
                    count,
                }) => {
                    push_operand_place(source, &mut places);
                    push_operand_place(destination, &mut places);
                    push_operand_place(count, &mut places);
                }
                StatementKindV2::Intrinsic(IntrinsicStatementV2::Assume { condition }) => {
                    push_operand_place(condition, &mut places);
                }
                StatementKindV2::StorageLive { .. }
                | StatementKindV2::StorageDead { .. }
                | StatementKindV2::Intrinsic(IntrinsicStatementV2::CompilerOpaque { .. })
                | StatementKindV2::Coverage { .. }
                | StatementKindV2::Nop
                | StatementKindV2::CompilerOpaque { .. } => {}
            }
        }
        match &block.terminator.kind {
            TerminatorKindV2::SwitchInt { discriminant, .. } => {
                push_operand_place(discriminant, &mut places);
            }
            TerminatorKindV2::Call {
                function,
                arguments,
                destination,
                ..
            } => {
                push_operand_place(function, &mut places);
                for argument in arguments {
                    push_operand_place(&argument.operand, &mut places);
                }
                places.push(destination);
            }
            TerminatorKindV2::TailCall {
                function,
                arguments,
                ..
            } => {
                push_operand_place(function, &mut places);
                for argument in arguments {
                    push_operand_place(&argument.operand, &mut places);
                }
            }
            TerminatorKindV2::Drop { place, .. } => places.push(place),
            TerminatorKindV2::Assert { condition, .. } => {
                push_operand_place(condition, &mut places);
            }
            TerminatorKindV2::Return
            | TerminatorKindV2::Unreachable
            | TerminatorKindV2::Goto { .. }
            | TerminatorKindV2::InlineAsm { .. }
            | TerminatorKindV2::CompilerOpaque { .. } => {}
        }
    }
    places
}

fn push_rvalue_places<'a>(value: &'a RvalueV2, places: &mut Vec<&'a PlaceV2>) {
    match value {
        RvalueV2::Use(operand)
        | RvalueV2::Repeat { operand, .. }
        | RvalueV2::Unary { operand, .. }
        | RvalueV2::Cast { operand, .. }
        | RvalueV2::WrapUnsafeBinder { operand, .. } => push_operand_place(operand, places),
        RvalueV2::Reference { place, .. }
        | RvalueV2::RawPointer { place, .. }
        | RvalueV2::Len(place)
        | RvalueV2::Discriminant { place }
        | RvalueV2::CopyForDeref(place) => places.push(place),
        RvalueV2::Binary { lhs, rhs, .. } => {
            push_operand_place(lhs, places);
            push_operand_place(rhs, places);
        }
        RvalueV2::Aggregate { operands, .. } => {
            for operand in operands {
                push_operand_place(operand, places);
            }
        }
        RvalueV2::Nullary { .. }
        | RvalueV2::ThreadLocalRef { .. }
        | RvalueV2::CompilerOpaque { .. } => {}
    }
}

fn push_operand_place<'a>(operand: &'a OperandV2, places: &mut Vec<&'a PlaceV2>) {
    if let OperandV2::Copy(place) | OperandV2::Move(place) = operand {
        places.push(place);
    }
}

#[test]
fn compiler_capture_preserves_cfg_reassignments_and_source_spans() {
    let body = compiler_results().observed;
    body.validate(CaptureLimitsV2::default()).unwrap();
    assert!(!body.is_authorized_for_lowering());
    assert!(
        body.blocks.len() > 3,
        "fixture should retain nontrivial CFG"
    );
    assert!(body.blocks.iter().any(|block| {
        block
            .terminator
            .successors
            .iter()
            .any(|target| *target <= block.index)
    }));

    let mut assigned_locals = Vec::new();
    assigned_locals.extend(assignments(&body).map(|(place, _)| place.local));
    assigned_locals.sort_unstable();
    assert!(
        assigned_locals
            .windows(2)
            .any(|locals| locals[0] == locals[1]),
        "repeated MIR-local assignments must survive capture"
    );
    assert!(body.source.file.contains("fe2o3-mir-v2"));
    assert!(body.blocks.iter().all(|block| {
        !block.terminator.source.file.is_empty()
            && block.terminator.source.start_line > 0
            && block.terminator.source.end_line > 0
    }));
}

#[test]
fn compiler_capture_preserves_aggregates_casts_discriminants_and_projections() {
    let body = compiler_results().observed;
    let rvalues = assignments(&body)
        .map(|(_, value)| value)
        .collect::<Vec<_>>();
    assert!(rvalues.iter().any(|value| matches!(
        value,
        RvalueV2::Aggregate {
            kind: AggregateKindV2 {
                class: AggregateClassV2::Closure,
                definition: Some(_),
                ..
            },
            ..
        }
    )));
    assert!(
        rvalues
            .iter()
            .any(|value| matches!(value, RvalueV2::Cast { .. }))
    );
    assert!(
        rvalues
            .iter()
            .any(|value| matches!(value, RvalueV2::Discriminant { .. }))
    );
    let captured_places = places(&body);
    assert!(
        captured_places.iter().any(|place| {
            place.projection.iter().any(
            |projection| matches!(projection, ProjectionV2::Field { ty, .. } if !ty.rust.is_empty())
        )
        }),
        "captured places: {captured_places:#?}"
    );
    assert!(places(&body).iter().any(|place| {
        place
            .projection
            .iter()
            .any(|projection| matches!(projection, ProjectionV2::Downcast { .. }))
    }));
}

#[test]
fn compiler_capture_preserves_generated_callable_and_intrinsic_def_ids() {
    let results = compiler_results();
    let generated = calls(&results.invoke_once).find_map(|kind| match kind {
        TerminatorKindV2::Call {
            declared: Some(declared),
            resolved:
                Some(FunctionIdentityV2 {
                    instance:
                        InstanceIdentityV2 {
                            kind: InstanceKindV2::GeneratedCallable { rustc_kind },
                            ..
                        },
                    ..
                }),
            ..
        } => Some((declared, rustc_kind)),
        _ => None,
    });
    let invoke_calls = calls(&results.invoke_once).collect::<Vec<_>>();
    let (declared, generated_kind) = generated
        .unwrap_or_else(|| panic!("missing generated closure callable: {invoke_calls:#?}"));
    assert!(declared.def_path.contains("FnOnce"));
    assert!(generated_kind.contains("ClosureOnceShim"));
    assert_ne!(declared.def_path_hash, [0; 16]);
    assert!(
        results
            .invoke_once
            .locals
            .iter()
            .all(|local| local.ty.rust != "F")
    );
    let call_spans = calls(&results.invoke_once).find_map(|kind| match kind {
        TerminatorKindV2::Call {
            arguments,
            function_span,
            ..
        } => Some((arguments, function_span)),
        _ => None,
    });
    let (arguments, function_span) = call_spans.expect("missing spanned closure call");
    assert!(function_span.start_line > 0);
    assert!(
        arguments
            .iter()
            .all(|argument| argument.source.start_line > 0)
    );

    let intrinsic = calls(&results.intrinsic).find_map(|kind| match kind {
        TerminatorKindV2::Call {
            intrinsic: Some(intrinsic),
            resolved: Some(resolved),
            ..
        } => Some((intrinsic, resolved)),
        _ => None,
    });
    let (intrinsic, resolved) = intrinsic.expect("missing compiler intrinsic identity");
    assert_eq!(intrinsic.name, "black_box");
    assert_eq!(intrinsic.definition, resolved.definition);
    assert_ne!(intrinsic.definition.def_path_hash, [0; 16]);
    assert!(matches!(
        resolved.instance.kind,
        InstanceKindV2::GeneratedCallable { .. }
    ));
}

#[test]
fn compiler_capture_enforces_bounds_before_normalized_validation() {
    let error = compiler_results().bounded_error;
    assert!(error.contains("blocks bound exceeded"), "{error}");
    assert!(error.contains("> 1"), "{error}");
}

#[test]
fn normalized_validation_rejects_cfg_projection_identity_and_bound_mutations() {
    let original = compiler_results().observed;

    let mut bad_cfg = original.clone();
    let block_count = bad_cfg.blocks.len();
    bad_cfg.blocks[0].terminator.successors.push(block_count);
    assert!(
        bad_cfg
            .validate(CaptureLimitsV2::default())
            .unwrap_err()
            .to_string()
            .contains("outside")
    );

    let mut inconsistent_cfg = original.clone();
    let block = inconsistent_cfg
        .blocks
        .iter_mut()
        .find(|block| matches!(block.terminator.kind, TerminatorKindV2::Goto { .. }))
        .expect("fixture must contain a goto");
    block.terminator.successors.clear();
    assert!(
        inconsistent_cfg
            .validate(CaptureLimitsV2::default())
            .unwrap_err()
            .to_string()
            .contains("disagrees with terminator targets")
    );

    let mut bad_place = original.clone();
    let local_count = bad_place.locals.len();
    let destination = bad_place
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.statements)
        .find_map(|statement| match &mut statement.kind {
            StatementKindV2::Assign { destination, .. } => Some(destination),
            _ => None,
        })
        .expect("fixture must assign");
    destination
        .projection
        .push(ProjectionV2::Index { local: local_count });
    assert!(
        bad_place
            .validate(CaptureLimitsV2::default())
            .unwrap_err()
            .to_string()
            .contains("outside")
    );

    let mut bad_identity = original.clone();
    bad_identity.function.definition.def_path.push('\0');
    assert!(
        bad_identity
            .validate(CaptureLimitsV2::default())
            .unwrap_err()
            .to_string()
            .contains("NUL")
    );

    let tight = CaptureLimitsV2 {
        max_blocks: original.blocks.len() - 1,
        ..CaptureLimitsV2::default()
    };
    assert!(
        original
            .validate(tight)
            .unwrap_err()
            .to_string()
            .contains("bound exceeded")
    );
}

#[test]
fn normalized_validation_accepts_repeated_assignments_but_never_authorizes_them() {
    let body = compiler_results().observed;
    let mut counts = std::collections::BTreeMap::new();
    for (place, _) in assignments(&body) {
        *counts.entry(place.local).or_insert(0usize) += 1;
    }
    assert!(counts.values().any(|count| *count > 1));
    body.validate(CaptureLimitsV2::default()).unwrap();
    assert!(!body.is_authorized_for_lowering());
}
