use super::accounting::recompute_capture_accounting_v2;
use super::normalized::*;
use super::rustc_adapter::{capture_instance_body_v2, capture_instance_observation_v2};
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def::DefKind;
use rustc_interface::interface::Compiler;
use rustc_middle::mir::{Operand, TerminatorKind};
use rustc_middle::ty::{Instance, TyCtxt, TyKind, TypingEnv};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

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

#[inline(never)]
fn observed_function_pointer(function: fn(u32) -> u32, value: u32) -> u32 {
    function(value)
}

#[inline(never)]
fn observed_inline_assembly() {
    unsafe { core::arch::asm!("", options(nomem, nostack)) }
}
"#;

#[derive(Clone, Debug)]
struct DriverResults {
    observed: CapturedBodyV2,
    invoke_once: CapturedBodyV2,
    closure_once_shim: CapturedBodyV2,
    intrinsic: CapturedBodyV2,
    function_pointer: CapturedBodyV2,
    bounded_error: String,
    work_bound_error: String,
    text_bound_error: String,
    switch_bound_error: String,
    unsupported_error: String,
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
        let function_pointer_instance =
            Instance::mono(tcx, local_function(tcx, "observed_function_pointer"));
        let inline_assembly_instance =
            Instance::mono(tcx, local_function(tcx, "observed_inline_assembly"));
        let invoke_instance = resolved_direct_calls(tcx, observed_instance)
            .into_iter()
            .find(|instance| tcx.item_name(instance.def_id()).as_str() == "invoke_once")
            .expect("observed fixture must call a concrete invoke_once instance");
        let closure_once_shim = resolved_direct_calls(tcx, invoke_instance)
            .into_iter()
            .find(|instance| {
                matches!(
                    instance.def,
                    rustc_middle::ty::InstanceKind::ClosureOnceShim { .. }
                )
            })
            .expect("invoke_once must resolve a closure-once shim");

        self.results = Some(DriverResults {
            observed: capture_instance_body_v2(tcx, observed_instance, limits)
                .expect("capture observed MIR"),
            invoke_once: capture_instance_body_v2(tcx, invoke_instance, limits)
                .expect("capture monomorphized invoke_once MIR"),
            closure_once_shim: capture_instance_observation_v2(tcx, closure_once_shim, limits)
                .expect("observe generated closure-once shim MIR without authority"),
            intrinsic: capture_instance_body_v2(tcx, intrinsic_instance, limits)
                .expect("capture intrinsic caller MIR"),
            function_pointer: capture_instance_body_v2(tcx, function_pointer_instance, limits)
                .expect("capture indirect function-pointer caller MIR"),
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
            work_bound_error: capture_instance_body_v2(
                tcx,
                observed_instance,
                CaptureLimitsV2 {
                    max_total_work_items: 1,
                    ..limits
                },
            )
            .expect_err("a one-item work bound must reject before capture")
            .to_string(),
            text_bound_error: capture_instance_body_v2(
                tcx,
                observed_instance,
                CaptureLimitsV2 {
                    max_total_text_bytes: 1,
                    ..limits
                },
            )
            .expect_err("a one-byte aggregate text bound must reject capture")
            .to_string(),
            switch_bound_error: capture_instance_body_v2(
                tcx,
                observed_instance,
                CaptureLimitsV2 {
                    max_switch_targets: 0,
                    ..limits
                },
            )
            .expect_err("a zero switch-target bound must reject before allocation")
            .to_string(),
            unsupported_error: capture_instance_body_v2(tcx, inline_assembly_instance, limits)
                .expect_err("inline assembly must remain an explicit unsupported record")
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
            let normalized_args = caller
                .try_instantiate_mir_and_normalize_erasing_regions(
                    tcx,
                    TypingEnv::fully_monomorphized(),
                    rustc_middle::ty::EarlyBinder::bind(*args),
                )
                .ok()?;
            Instance::try_resolve(
                tcx,
                TypingEnv::fully_monomorphized(),
                *def_id,
                normalized_args,
            )
            .ok()
            .flatten()
        })
        .collect()
}

struct CompilerFixture {
    root: PathBuf,
    source: PathBuf,
    output: PathBuf,
}

impl CompilerFixture {
    fn create(source_text: &str) -> Self {
        static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);
        let serial = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let root =
            std::env::temp_dir().join(format!("fe2o3-mir-v2-{}-{serial}", std::process::id()));
        let source = root.join("fixture.rs");
        let output = root.join("fixture.rmeta");
        fs::create_dir(&root).expect("create MIR V2 fixture directory");
        fs::write(&source, source_text).expect("write MIR V2 fixture");
        Self {
            root,
            source,
            output,
        }
    }
}

impl Drop for CompilerFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn compiler_args(fixture: &CompilerFixture, crate_name: &str, metadata: &str) -> Vec<String> {
    static SYSROOT: OnceLock<String> = OnceLock::new();
    let sysroot = SYSROOT.get_or_init(|| {
        let output = Command::new("rustc")
            .args(["--print", "sysroot"])
            .output()
            .expect("query rustc sysroot");
        assert!(output.status.success(), "rustc --print sysroot failed");
        String::from_utf8(output.stdout)
            .expect("UTF-8 rustc sysroot")
            .trim()
            .to_owned()
    });
    vec![
        "rustc".to_owned(),
        "--crate-name".to_owned(),
        crate_name.to_owned(),
        "--crate-type".to_owned(),
        "lib".to_owned(),
        "--edition".to_owned(),
        "2024".to_owned(),
        "--emit".to_owned(),
        "metadata".to_owned(),
        "-Zmir-opt-level=0".to_owned(),
        "-Coverflow-checks=off".to_owned(),
        format!("-Cmetadata={metadata}"),
        format!("--remap-path-prefix={}=/workspace", fixture.root.display()),
        "--sysroot".to_owned(),
        sysroot.clone(),
        "-o".to_owned(),
        fixture.output.display().to_string(),
        fixture.source.display().to_string(),
    ]
}

fn run_compiler_serialized(args: &[String], callbacks: &mut (impl Callbacks + Send)) {
    static DRIVER_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    let _guard = DRIVER_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("rustc driver lock poisoned");
    rustc_driver::run_compiler(args, callbacks);
}

fn compiler_results() -> DriverResults {
    static RESULTS: OnceLock<DriverResults> = OnceLock::new();
    RESULTS
        .get_or_init(|| {
            let fixture = CompilerFixture::create(FIXTURE_SOURCE);
            let args = compiler_args(&fixture, "fe2o3_mir_v2_fixture", "stable");
            let mut callbacks = CaptureCallbacks::default();
            run_compiler_serialized(&args, &mut callbacks);
            callbacks.results.expect("MIR V2 callback did not run")
        })
        .clone()
}

#[derive(Default)]
struct SingleCaptureCallbacks {
    body: Option<CapturedBodyV2>,
}

impl Callbacks for SingleCaptureCallbacks {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let instance = Instance::mono(tcx, local_function(tcx, "observed"));
        self.body = Some(
            capture_instance_body_v2(tcx, instance, CaptureLimitsV2::default())
                .expect("capture single observed body"),
        );
        Compilation::Stop
    }
}

fn compile_observed(crate_name: &str, metadata: &str) -> CapturedBodyV2 {
    let fixture = CompilerFixture::create(FIXTURE_SOURCE);
    let args = compiler_args(&fixture, crate_name, metadata);
    let mut callbacks = SingleCaptureCallbacks::default();
    run_compiler_serialized(&args, &mut callbacks);
    callbacks.body.expect("single capture callback did not run")
}

struct LimitedCaptureCallbacks {
    limits: CaptureLimitsV2,
    error: Option<String>,
}

impl Callbacks for LimitedCaptureCallbacks {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let instance = Instance::mono(tcx, local_function(tcx, "observed"));
        self.error = Some(
            capture_instance_body_v2(tcx, instance, self.limits)
                .expect_err("adversarial type must exceed its configured preflight bound")
                .to_string(),
        );
        Compilation::Stop
    }
}

fn compile_capture_error(source: &str, limits: CaptureLimitsV2) -> String {
    let fixture = CompilerFixture::create(source);
    let args = compiler_args(&fixture, "fe2o3_mir_v2_type_bound", "type-bound");
    let mut callbacks = LimitedCaptureCallbacks {
        limits,
        error: None,
    };
    run_compiler_serialized(&args, &mut callbacks);
    callbacks
        .error
        .expect("limited capture callback did not run")
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CanonicalSpanKey {
    authority: SourceAuthorityV2,
    remapped_file: String,
    source_file_hash: [u8; 16],
    span_hash: [u8; 16],
    start: (usize, usize),
    end: (usize, usize),
    source_scope: usize,
    source_scope_hash: [u8; 16],
    source_scope_parent: Option<usize>,
    inlined_instance_hash: Option<[u8; 16]>,
}

impl From<&SourceSpanV2> for CanonicalSpanKey {
    fn from(span: &SourceSpanV2) -> Self {
        Self {
            authority: span.authority,
            remapped_file: span.remapped_file.clone(),
            source_file_hash: span.source_file_hash,
            span_hash: span.span_hash,
            start: (span.start_line, span.start_column),
            end: (span.end_line, span.end_column),
            source_scope: span.source_scope,
            source_scope_hash: span.source_scope_hash,
            source_scope_parent: span.source_scope_parent,
            inlined_instance_hash: span.inlined_instance_hash,
        }
    }
}

fn canonical_span_keys(body: &CapturedBodyV2) -> Vec<CanonicalSpanKey> {
    let mut keys = vec![CanonicalSpanKey::from(&body.source)];
    keys.extend(
        body.locals
            .iter()
            .map(|local| CanonicalSpanKey::from(&local.source)),
    );
    for block in &body.blocks {
        keys.extend(
            block
                .statements
                .iter()
                .map(|statement| CanonicalSpanKey::from(&statement.source)),
        );
        keys.push(CanonicalSpanKey::from(&block.terminator.source));
        match &block.terminator.kind {
            TerminatorKindV2::Call {
                arguments,
                function_span,
                ..
            }
            | TerminatorKindV2::TailCall {
                arguments,
                function_span,
                ..
            } => {
                keys.push(CanonicalSpanKey::from(function_span));
                keys.extend(
                    arguments
                        .iter()
                        .map(|argument| CanonicalSpanKey::from(&argument.source)),
                );
            }
            _ => {}
        }
    }
    keys
}

fn assignments(body: &CapturedBodyV2) -> impl Iterator<Item = (&PlaceV2, &RvalueV2)> {
    body.blocks
        .iter()
        .flat_map(|block| &block.statements)
        .filter_map(|statement| match &statement.kind {
            StatementKindV2::Assign { destination, value } => Some((destination, value.as_ref())),
            _ => None,
        })
}

fn calls(body: &CapturedBodyV2) -> impl Iterator<Item = &TerminatorKindV2> {
    body.blocks
        .iter()
        .map(|block| &block.terminator.kind)
        .filter(|kind| matches!(kind, TerminatorKindV2::Call { .. }))
}

fn refresh_capture_accounting(body: &mut CapturedBodyV2) {
    body.capture_work_items = 0;
    body.capture_text_bytes = 0;
    let accounting = recompute_capture_accounting_v2(body, CaptureLimitsV2::default()).unwrap();
    body.capture_work_items = accounting.work_items;
    body.capture_text_bytes = accounting.text_bytes;
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
                | StatementKindV2::Retag { place, .. }
                | StatementKindV2::PlaceMention { place } => places.push(place),
                StatementKindV2::Intrinsic(intrinsic) => match intrinsic.as_ref() {
                    IntrinsicStatementV2::CopyNonOverlapping {
                        source,
                        destination,
                        count,
                    } => {
                        push_operand_place(source, &mut places);
                        push_operand_place(destination, &mut places);
                        push_operand_place(count, &mut places);
                    }
                    IntrinsicStatementV2::Assume { condition } => {
                        push_operand_place(condition, &mut places);
                    }
                },
                StatementKindV2::StorageLive { .. }
                | StatementKindV2::StorageDead { .. }
                | StatementKindV2::Coverage { .. }
                | StatementKindV2::Nop
                | StatementKindV2::Unsupported(_) => {}
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
            | TerminatorKindV2::UnwindResume
            | TerminatorKindV2::UnwindTerminate { .. }
            | TerminatorKindV2::CoroutineDrop
            | TerminatorKindV2::FalseEdge { .. }
            | TerminatorKindV2::FalseUnwind { .. }
            | TerminatorKindV2::Unsupported(_) => {}
            TerminatorKindV2::Yield {
                value,
                resume_argument,
                ..
            } => {
                push_operand_place(value, &mut places);
                places.push(resume_argument);
            }
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
        RvalueV2::ThreadLocalRef { .. } => {}
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
    assert_eq!(body.source.remapped_file, "/workspace/fixture.rs");
    assert!(body.blocks.iter().all(|block| {
        !block.terminator.source.remapped_file.is_empty()
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
            kind: AggregateKindV2::Closure { .. },
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
            |projection| matches!(projection, ProjectionV2::Field { ty, .. } if !ty.diagnostic_display.is_empty())
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
    let generated = calls(&results.invoke_once).find_map(|kind| {
        let TerminatorKindV2::Call {
            callee: CalleeIdentityV2::Direct {
                declared, resolved, ..
            },
            ..
        } = kind
        else {
            return None;
        };
        matches!(
            resolved.instance.kind,
            InstanceKindV2::ClosureOnceShim { .. }
        )
        .then_some(declared)
    });
    let invoke_calls = calls(&results.invoke_once).collect::<Vec<_>>();
    let declared = generated
        .unwrap_or_else(|| panic!("missing generated closure callable: {invoke_calls:#?}"));
    assert!(declared.diagnostic_def_path.contains("FnOnce"));
    assert_ne!(declared.def_path_hash, [0; 16]);
    assert!(
        results
            .invoke_once
            .locals
            .iter()
            .all(|local| local.ty.diagnostic_display != "F")
    );
    assert!(matches!(
        results.closure_once_shim.function.instance.kind,
        InstanceKindV2::ClosureOnceShim { .. }
    ));
    assert!(
        results
            .closure_once_shim
            .locals
            .iter()
            .all(|local| !matches!(local.ty.class, TypeClassV2::Unsupported(_)))
    );
    assert!(!results.closure_once_shim.is_authorized_for_lowering());
    let unauthoritative_error = results
        .closure_once_shim
        .validate(CaptureLimitsV2::default())
        .unwrap_err()
        .to_string();
    assert!(
        unauthoritative_error.contains("source identity is unauthoritative"),
        "{unauthoritative_error}"
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
            callee:
                CalleeIdentityV2::Direct {
                    intrinsic: Some(intrinsic),
                    resolved,
                    ..
                },
            ..
        } => Some((intrinsic, resolved)),
        _ => None,
    });
    let (intrinsic, resolved) = intrinsic.expect("missing compiler intrinsic identity");
    assert_eq!(intrinsic.name, "black_box");
    assert_eq!(intrinsic.definition, resolved.definition);
    assert_ne!(intrinsic.definition.def_path_hash, [0; 16]);
    assert!(matches!(resolved.instance.kind, InstanceKindV2::Intrinsic));

    let indirect = calls(&results.function_pointer).find_map(|kind| match kind {
        TerminatorKindV2::Call {
            callee: CalleeIdentityV2::Indirect { callable_type },
            ..
        } => Some(callable_type),
        _ => None,
    });
    assert!(matches!(
        indirect.expect("missing legitimate indirect call").class,
        TypeClassV2::FunctionPointer
    ));
}

#[test]
fn compiler_capture_enforces_bounds_before_normalized_validation() {
    let results = compiler_results();
    let error = results.bounded_error;
    assert!(error.contains("blocks bound exceeded"), "{error}");
    assert!(error.contains("> 1"), "{error}");
    assert!(
        results.work_bound_error.contains("total capture work"),
        "{}",
        results.work_bound_error
    );
    assert!(
        results.text_bound_error.contains("text"),
        "{}",
        results.text_bound_error
    );
    assert!(
        results.switch_bound_error.contains("switch targets"),
        "{}",
        results.switch_bound_error
    );
    assert!(
        results.unsupported_error.contains("unsupported terminator"),
        "{}",
        results.unsupported_error
    );
}

#[test]
fn compiler_capture_iteratively_rejects_deep_and_wide_types_before_hashing() {
    let mut nested = "u8".to_owned();
    for _ in 0..96 {
        nested = format!("({nested},)");
    }
    let deep_source = format!(
        "#![recursion_limit = \"512\"]\n#[inline(never)]\nfn observed(value: {nested}) -> {nested} {{ value }}\n"
    );
    let depth_error = compile_capture_error(
        &deep_source,
        CaptureLimitsV2 {
            max_type_depth: 16,
            ..CaptureLimitsV2::default()
        },
    );
    assert!(
        depth_error.contains("type depth bound exceeded"),
        "{depth_error}"
    );

    let fields = std::iter::repeat_n("u8", 64).collect::<Vec<_>>().join(", ");
    let wide_source =
        format!("#[inline(never)]\nfn observed(value: ({fields})) -> ({fields}) {{ value }}\n");
    let arity_error = compile_capture_error(
        &wide_source,
        CaptureLimitsV2 {
            max_type_arity: 8,
            ..CaptureLimitsV2::default()
        },
    );
    assert!(
        arity_error.contains("type arity bound exceeded"),
        "{arity_error}"
    );
}

#[test]
fn normalized_validation_rejects_cfg_projection_identity_and_bound_mutations() {
    let original = compiler_results().observed;

    let mut bad_cfg = original.clone();
    let block_count = bad_cfg.blocks.len();
    bad_cfg.blocks[0].terminator.successors.push(block_count);
    refresh_capture_accounting(&mut bad_cfg);
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
    refresh_capture_accounting(&mut inconsistent_cfg);
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
    refresh_capture_accounting(&mut bad_place);
    assert!(
        bad_place
            .validate(CaptureLimitsV2::default())
            .unwrap_err()
            .to_string()
            .contains("outside")
    );

    let mut bad_identity = original.clone();
    bad_identity
        .function
        .definition
        .diagnostic_def_path
        .push('\0');
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
fn normalized_validation_enforces_call_type_and_source_scope_coherence() {
    let mut bad_call = compiler_results().invoke_once;
    let declared_args_hash = bad_call
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator.kind {
            TerminatorKindV2::Call {
                callee:
                    CalleeIdentityV2::Direct {
                        declared_generic_args_hash,
                        ..
                    },
                ..
            } => Some(declared_generic_args_hash),
            _ => None,
        })
        .expect("fixture must contain a direct call");
    declared_args_hash[0] ^= 1;
    assert!(
        bad_call
            .validate(CaptureLimitsV2::default())
            .unwrap_err()
            .to_string()
            .contains("disagrees with its operand type")
    );

    let mut bad_scope = compiler_results().observed;
    bad_scope.source.source_scope_parent = Some(bad_scope.source.source_scope);
    refresh_capture_accounting(&mut bad_scope);
    assert!(
        bad_scope
            .validate(CaptureLimitsV2::default())
            .unwrap_err()
            .to_string()
            .contains("source scope")
    );
}

#[test]
fn normalized_validation_rejects_direct_call_substitution_and_signature_mutation() {
    let replacement = calls(&compiler_results().observed)
        .find_map(|kind| match kind {
            TerminatorKindV2::Call {
                callee: CalleeIdentityV2::Direct { resolved, .. },
                ..
            } => Some(resolved.as_ref().clone()),
            _ => None,
        })
        .expect("observed fixture must have a direct call replacement");
    let mut substituted = compiler_results().invoke_once;
    let direct = substituted
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator.kind {
            TerminatorKindV2::Call {
                callee: CalleeIdentityV2::Direct { resolved, .. },
                ..
            } => Some(resolved),
            _ => None,
        })
        .expect("invoke_once fixture must have a direct call");
    **direct = replacement;
    refresh_capture_accounting(&mut substituted);
    let substitution_error = substituted
        .validate(CaptureLimitsV2::default())
        .unwrap_err()
        .to_string();
    assert!(
        substitution_error.contains("resolution_binding_hash"),
        "{substitution_error}"
    );

    let mut bad_signature = compiler_results().invoke_once;
    let signature = bad_signature
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator.kind {
            TerminatorKindV2::Call {
                callee:
                    CalleeIdentityV2::Direct {
                        resolved_signature, ..
                    },
                ..
            } => Some(resolved_signature),
            _ => None,
        })
        .expect("invoke_once fixture must have a resolved signature");
    signature.stable_hash[0] ^= 1;
    let signature_error = bad_signature
        .validate(CaptureLimitsV2::default())
        .unwrap_err()
        .to_string();
    assert!(
        signature_error.contains("resolution_binding_hash"),
        "{signature_error}"
    );
}

#[test]
fn normalized_validation_binds_intrinsic_presence_name_flags_and_definition() {
    let original = compiler_results().intrinsic;

    let mut removed = original.clone();
    let intrinsic = removed
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator.kind {
            TerminatorKindV2::Call {
                callee: CalleeIdentityV2::Direct { intrinsic, .. },
                ..
            } if intrinsic.is_some() => Some(intrinsic),
            _ => None,
        })
        .expect("intrinsic fixture must have intrinsic metadata");
    *intrinsic = None;
    refresh_capture_accounting(&mut removed);
    let removed_error = removed
        .validate(CaptureLimitsV2::default())
        .unwrap_err()
        .to_string();
    assert!(removed_error.contains("if and only if"), "{removed_error}");

    for mutate in [
        |intrinsic: &mut IntrinsicIdentityV2| intrinsic.name = "white_box".to_owned(),
        |intrinsic: &mut IntrinsicIdentityV2| {
            intrinsic.must_be_overridden = !intrinsic.must_be_overridden;
        },
        |intrinsic: &mut IntrinsicIdentityV2| intrinsic.definition.def_path_hash[0] ^= 1,
    ] {
        let mut body = original.clone();
        let intrinsic = body
            .blocks
            .iter_mut()
            .find_map(|block| match &mut block.terminator.kind {
                TerminatorKindV2::Call {
                    callee:
                        CalleeIdentityV2::Direct {
                            intrinsic: Some(intrinsic),
                            ..
                        },
                    ..
                } => Some(intrinsic),
                _ => None,
            })
            .expect("intrinsic fixture must have intrinsic metadata");
        mutate(intrinsic);
        let error = body
            .validate(CaptureLimitsV2::default())
            .unwrap_err()
            .to_string();
        assert!(error.contains("intrinsic identity binding"), "{error}");
    }

    let metadata = calls(&original)
        .find_map(|kind| match kind {
            TerminatorKindV2::Call {
                callee:
                    CalleeIdentityV2::Direct {
                        intrinsic: Some(intrinsic),
                        ..
                    },
                ..
            } => Some(intrinsic.clone()),
            _ => None,
        })
        .unwrap();
    let mut added = compiler_results().observed;
    let target = added
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator.kind {
            TerminatorKindV2::Call {
                callee: CalleeIdentityV2::Direct { intrinsic, .. },
                ..
            } if intrinsic.is_none() => Some(intrinsic),
            _ => None,
        })
        .expect("observed fixture must have a non-intrinsic direct call");
    *target = Some(metadata);
    refresh_capture_accounting(&mut added);
    let added_error = added
        .validate(CaptureLimitsV2::default())
        .unwrap_err()
        .to_string();
    assert!(added_error.contains("if and only if"), "{added_error}");
}

#[test]
fn normalized_validation_rejects_explicit_unsupported_records() {
    let mut body = compiler_results().observed;
    body.blocks[0].statements[0].kind =
        StatementKindV2::Unsupported(UnsupportedStatementV2::ConstEvalCounter);
    refresh_capture_accounting(&mut body);
    assert!(
        body.validate(CaptureLimitsV2::default())
            .unwrap_err()
            .to_string()
            .contains("unsupported statement")
    );
}

#[test]
fn diagnostic_strings_do_not_define_structural_identity() {
    let mut body = compiler_results().observed;
    body.function.definition.diagnostic_crate_name = "diagnostic-only-crate".to_owned();
    body.function.definition.diagnostic_def_path = "diagnostic-only-path".to_owned();
    body.locals[0].ty.diagnostic_display = "diagnostic-only-type".to_owned();
    body.locals[0].ty.diagnostic_debug = "diagnostic-only-debug".to_owned();
    refresh_capture_accounting(&mut body);
    body.validate(CaptureLimitsV2::default()).unwrap();
}

#[test]
fn normalized_validation_recomputes_and_exactly_matches_capture_counters() {
    let original = compiler_results().observed;
    for forged in [
        original.capture_work_items - 1,
        original.capture_work_items + 1,
    ] {
        let mut body = original.clone();
        body.capture_work_items = forged;
        let error = body
            .validate(CaptureLimitsV2::default())
            .unwrap_err()
            .to_string();
        assert!(error.contains("reported work count"), "{error}");
    }
    for forged in [
        original.capture_text_bytes - 1,
        original.capture_text_bytes + 1,
    ] {
        let mut body = original.clone();
        body.capture_text_bytes = forged;
        let error = body
            .validate(CaptureLimitsV2::default())
            .unwrap_err()
            .to_string();
        assert!(error.contains("reported text count"), "{error}");
    }

    let work_error = original
        .validate(CaptureLimitsV2 {
            max_total_work_items: original.capture_work_items - 1,
            ..CaptureLimitsV2::default()
        })
        .unwrap_err()
        .to_string();
    assert!(work_error.contains("recomputed work bound"), "{work_error}");
    let text_error = original
        .validate(CaptureLimitsV2 {
            max_total_text_bytes: original.capture_text_bytes - 1,
            ..CaptureLimitsV2::default()
        })
        .unwrap_err()
        .to_string();
    assert!(text_error.contains("recomputed text bound"), "{text_error}");
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

#[test]
fn canonical_remapped_spans_are_deterministic_across_source_roots() {
    let first = compile_observed("mir_v2_span_fixture", "same-identity");
    let second = compile_observed("mir_v2_span_fixture", "same-identity");
    let first_keys = canonical_span_keys(&first);
    let second_keys = canonical_span_keys(&second);
    assert_eq!(first_keys, second_keys);
    assert!(first_keys.iter().all(|key| {
        key.authority == SourceAuthorityV2::CanonicalRemapped
            && key.remapped_file == "/workspace/fixture.rs"
    }));
}

#[test]
fn stable_definition_identity_separates_same_named_cross_crate_items() {
    let first = compile_observed("mir_v2_collision_fixture", "crate-a");
    let second = compile_observed("mir_v2_collision_fixture", "crate-b");
    let first = first.function.definition;
    let second = second.function.definition;
    assert_eq!(first.diagnostic_crate_name, second.diagnostic_crate_name);
    assert_eq!(first.diagnostic_def_path, second.diagnostic_def_path);
    assert_ne!(first.stable_crate_id, second.stable_crate_id);
    assert_ne!(first.def_path_hash, second.def_path_hash);
    assert!(first.local_def_path_hash.iter().any(|byte| *byte != 0));
    assert!(second.local_def_path_hash.iter().any(|byte| *byte != 0));
}
