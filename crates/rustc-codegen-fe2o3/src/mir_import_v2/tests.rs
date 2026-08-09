use super::accounting::recompute_capture_accounting_v2;
use super::normalized::*;
use super::preflight::preflight_body_v2;
use super::rustc_adapter::{
    CompilerCapturedBodyV2, RustcAuthenticCaptureV2, ValidatedRustcCaptureV2,
    capture_instance_body_v2, capture_instance_observation_v2, recapture_against_rustc_v2,
    rustc_authentic_capture_data_v2,
};
use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def::DefKind;
use rustc_interface::interface::Compiler;
use rustc_middle::mir::{Operand, TerminatorKind};
use rustc_middle::ty::{Instance, TyCtxt, TyKind, TypingEnv};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

trait AmbiguousIfDeserializeV2<A> {
    fn assert_not_implemented() {}
}

impl<T: ?Sized> AmbiguousIfDeserializeV2<()> for T {}
impl<T: ?Sized> AmbiguousIfDeserializeV2<u8> for T where for<'de> T: Deserialize<'de> {}

trait AmbiguousIfAuthenticCaptureV2<A> {
    fn assert_not_implemented() {}
}

impl<T: ?Sized> AmbiguousIfAuthenticCaptureV2<()> for T {}
impl<T: ?Sized + RustcAuthenticCaptureV2> AmbiguousIfAuthenticCaptureV2<u8> for T {}

const FIXTURE_SOURCE: &str = r#"
#![feature(core_intrinsics)]
#![feature(explicit_tail_calls)]
#![allow(dead_code, incomplete_features, internal_features)]

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
extern "C" fn c_identity(value: u32) -> u32 {
    value
}

#[inline(never)]
fn observed_c_call(value: u32) -> u32 {
    c_identity(value)
}

#[inline(never)]
fn tail_identity(value: u32) -> u32 {
    value
}

#[inline(never)]
fn observed_tail_call(value: u32) -> u32 {
    become tail_identity(value)
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
    c_call: CapturedBodyV2,
    tail_call: CapturedBodyV2,
    bounded_error: String,
    work_bound_error: String,
    scope_work_bound_error: String,
    text_bound_error: String,
    switch_bound_error: String,
    unsupported_error: String,
    recapture_errors: Vec<String>,
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
        let c_call_instance = Instance::mono(tcx, local_function(tcx, "observed_c_call"));
        let tail_call_instance = Instance::mono(tcx, local_function(tcx, "observed_tail_call"));
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
        let observed_body = tcx.instance_mir(observed_instance.def);
        let scope_entry_limit = 1usize
            .checked_add(observed_body.local_decls.len())
            .and_then(|value| value.checked_add(observed_body.basic_blocks.len()))
            .and_then(|value| value.checked_add(observed_body.source_scopes.len()))
            .and_then(|value| value.checked_sub(1))
            .expect("fixture preflight scope limit");
        let scope_work_bound_error = preflight_body_v2(
            observed_body,
            CaptureLimitsV2 {
                max_total_work_items: scope_entry_limit,
                ..limits
            },
        )
        .expect_err("tight aggregate work must reject source scopes during preflight")
        .to_string();
        let observed_capture =
            capture_instance_body_v2(tcx, observed_instance, limits).expect("capture observed MIR");
        assert!(!observed_capture.is_authorized_for_lowering());
        let observed = rustc_authentic_capture_data_v2(&observed_capture).clone();
        let recaptured = recapture_against_rustc_v2(tcx, observed_instance, limits, &observed)
            .expect("canonical data must pass exact rustc recapture");
        assert!(!recaptured.is_authorized_for_lowering());
        assert_eq!(rustc_authentic_capture_data_v2(&recaptured), &observed);
        let recapture_errors =
            adversarial_recapture_errors(tcx, observed_instance, limits, &observed);

        self.results = Some(DriverResults {
            observed,
            invoke_once: capture_data(tcx, invoke_instance, limits),
            closure_once_shim: capture_instance_observation_v2(tcx, closure_once_shim, limits)
                .expect("observe generated closure-once shim MIR without authority"),
            intrinsic: capture_data(tcx, intrinsic_instance, limits),
            function_pointer: capture_data(tcx, function_pointer_instance, limits),
            c_call: capture_data(tcx, c_call_instance, limits),
            tail_call: capture_data(tcx, tail_call_instance, limits),
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
            scope_work_bound_error,
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
            recapture_errors,
        });
        Compilation::Stop
    }
}

fn capture_data<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    limits: CaptureLimitsV2,
) -> CapturedBodyV2 {
    let captured = capture_instance_body_v2(tcx, instance, limits).expect("compiler capture");
    rustc_authentic_capture_data_v2(&captured).clone()
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
        self.body = Some(capture_data(tcx, instance, CaptureLimitsV2::default()));
        Compilation::Stop
    }
}

fn compile_observed(crate_name: &str, metadata: &str) -> CapturedBodyV2 {
    compile_source_observed(FIXTURE_SOURCE, crate_name, metadata)
}

fn compile_source_observed(source: &str, crate_name: &str, metadata: &str) -> CapturedBodyV2 {
    let fixture = CompilerFixture::create(source);
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
    original_span_hash: [u8; 16],
    span_hash: [u8; 16],
    expansion: MacroExpansionIdentityV2,
    start: (usize, usize),
    end: (usize, usize),
    source_scope: usize,
    source_scope_hash: [u8; 16],
    source_scope_parent: Option<usize>,
    inlined_instance_hash: Option<[u8; 16]>,
    source_scope_record_hash: [u8; 32],
}

impl From<&SourceSpanV2> for CanonicalSpanKey {
    fn from(span: &SourceSpanV2) -> Self {
        Self {
            authority: span.authority,
            remapped_file: span.remapped_file.clone(),
            source_file_hash: span.source_file_hash,
            original_span_hash: span.original_span_hash,
            span_hash: span.span_hash,
            expansion: span.expansion.clone(),
            start: (span.start_line, span.start_column),
            end: (span.end_line, span.end_column),
            source_scope: span.source_scope,
            source_scope_hash: span.source_scope_hash,
            source_scope_parent: span.source_scope_parent,
            inlined_instance_hash: span.inlined_instance_hash,
            source_scope_record_hash: span.source_scope_record_hash,
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

fn refresh_call_bindings(kind: &mut TerminatorKindV2) {
    match kind {
        TerminatorKindV2::Call {
            function,
            callee,
            arguments,
            destination,
            target,
            unwind,
            contract_hash,
            ..
        } => {
            refresh_callee_binding(function, callee);
            *contract_hash = call_contract_hash_v2(
                function,
                callee,
                arguments,
                Some(destination),
                *target,
                Some(unwind),
            )
            .unwrap();
        }
        TerminatorKindV2::TailCall {
            function,
            callee,
            arguments,
            contract_hash,
            ..
        } => {
            refresh_callee_binding(function, callee);
            *contract_hash =
                call_contract_hash_v2(function, callee, arguments, None, None, None).unwrap();
        }
        _ => panic!("expected call terminator"),
    }
}

fn refresh_callee_binding(function: &OperandV2, callee: &mut CalleeIdentityV2) {
    match callee {
        CalleeIdentityV2::Direct {
            declared,
            declared_generic_args_hash,
            declared_generic_arg_count,
            declared_signature,
            resolved,
            resolved_signature,
            intrinsic,
            resolution_binding_hash,
        } => {
            declared_signature.binding_hash =
                function_signature_binding_hash_v2(declared_signature).unwrap();
            resolved_signature.binding_hash =
                function_signature_binding_hash_v2(resolved_signature).unwrap();
            if let Some(intrinsic) = intrinsic {
                intrinsic.binding_hash = intrinsic_binding_hash_v2(intrinsic).unwrap();
            }
            let OperandV2::Constant { ty, .. } = function else {
                panic!("direct fixture call must use a constant function operand")
            };
            *resolution_binding_hash = resolution_binding_hash_v2(
                ty,
                declared,
                declared_generic_args_hash,
                *declared_generic_arg_count,
                declared_signature,
                resolved,
                resolved_signature,
                intrinsic.as_deref(),
            )
            .unwrap();
        }
        CalleeIdentityV2::Indirect {
            callable_type,
            signature,
            callable_binding_hash,
        } => {
            signature.binding_hash = function_signature_binding_hash_v2(signature).unwrap();
            *callable_binding_hash =
                indirect_callable_binding_hash_v2(callable_type, signature).unwrap();
        }
    }
}

fn adversarial_recapture_errors<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    limits: CaptureLimitsV2,
    canonical: &CapturedBodyV2,
) -> Vec<String> {
    let mut adversaries = Vec::new();

    let mut local_type = canonical.clone();
    local_type.locals[1].ty.stable_hash[0] ^= 1;
    refresh_capture_accounting(&mut local_type);
    adversaries.push(local_type);

    let mut place_type = canonical.clone();
    let place = place_type
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.statements)
        .find_map(|statement| match &mut statement.kind {
            StatementKindV2::Assign { destination, .. } => Some(destination),
            _ => None,
        })
        .expect("fixture must contain an assignment place");
    place.type_hash[0] ^= 1;
    if let Some(final_hash) = place.projection_type_hashes.last_mut() {
        *final_hash = place.type_hash;
    }
    refresh_capture_accounting(&mut place_type);
    adversaries.push(place_type);

    let mut resolution = canonical.clone();
    let call = resolution
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator.kind {
            kind @ TerminatorKindV2::Call {
                callee: CalleeIdentityV2::Direct { .. },
                ..
            } => Some(kind),
            _ => None,
        })
        .expect("fixture must contain a direct call");
    if let TerminatorKindV2::Call {
        callee: CalleeIdentityV2::Direct { resolved, .. },
        ..
    } = call
    {
        resolved.instance.instance_hash[0] ^= 1;
    }
    refresh_call_bindings(call);
    refresh_capture_accounting(&mut resolution);
    adversaries.push(resolution);

    let mut signature = canonical.clone();
    let call = signature
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator.kind {
            kind @ TerminatorKindV2::Call {
                callee: CalleeIdentityV2::Direct { .. },
                ..
            } => Some(kind),
            _ => None,
        })
        .expect("fixture must contain a direct call");
    if let TerminatorKindV2::Call {
        callee:
            CalleeIdentityV2::Direct {
                declared_signature,
                resolved_signature,
                ..
            },
        ..
    } = call
    {
        declared_signature.stable_hash[0] ^= 1;
        resolved_signature.stable_hash[0] ^= 1;
    }
    refresh_call_bindings(call);
    refresh_capture_accounting(&mut signature);
    adversaries.push(signature);

    adversaries
        .into_iter()
        .map(|data| {
            data.validate_untrusted_shape(limits)
                .expect("refreshed adversarial data must remain shape-valid");
            recapture_against_rustc_v2(tcx, instance, limits, &data)
                .expect_err("mutated data must fail exact rustc recapture")
                .to_string()
        })
        .collect()
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
fn raw_data_is_serializable_but_cannot_satisfy_the_authentic_capture_api() {
    fn assert_serializable<T: Serialize + for<'de> Deserialize<'de>>() {}
    fn assert_authentic<T: RustcAuthenticCaptureV2>() {}

    assert_serializable::<CapturedBodyV2>();
    assert_authentic::<CompilerCapturedBodyV2>();
    assert_authentic::<ValidatedRustcCaptureV2>();
    <CapturedBodyV2 as AmbiguousIfAuthenticCaptureV2<_>>::assert_not_implemented();
    <CompilerCapturedBodyV2 as AmbiguousIfDeserializeV2<_>>::assert_not_implemented();
    <ValidatedRustcCaptureV2 as AmbiguousIfDeserializeV2<_>>::assert_not_implemented();
}

#[test]
fn exact_rustc_recapture_rejects_refreshed_structural_forgeries() {
    let results = compiler_results();
    assert_eq!(results.recapture_errors.len(), 4);
    assert!(
        results
            .recapture_errors
            .iter()
            .all(|error| error.contains("differs from bounded canonical rustc recapture"))
    );
}

#[test]
fn compiler_capture_preserves_cfg_reassignments_and_source_spans() {
    let body = compiler_results().observed;
    body.validate_untrusted_shape(CaptureLimitsV2::default())
        .unwrap();
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
        .validate_untrusted_shape(CaptureLimitsV2::default())
        .unwrap_err()
        .to_string();
    assert!(
        unauthoritative_error.contains("canonical-remapped structural variant"),
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
            callee: CalleeIdentityV2::Indirect { callable_type, .. },
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
        results
            .scope_work_bound_error
            .contains("total capture work"),
        "{}",
        results.scope_work_bound_error
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
            .validate_untrusted_shape(CaptureLimitsV2::default())
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
            .validate_untrusted_shape(CaptureLimitsV2::default())
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
    destination
        .projection_type_hashes
        .push(destination.type_hash);
    refresh_capture_accounting(&mut bad_place);
    assert!(
        bad_place
            .validate_untrusted_shape(CaptureLimitsV2::default())
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
            .validate_untrusted_shape(CaptureLimitsV2::default())
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
            .validate_untrusted_shape(tight)
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
            .validate_untrusted_shape(CaptureLimitsV2::default())
            .unwrap_err()
            .to_string()
            .contains("disagrees with its operand type")
    );

    let mut bad_scope = compiler_results().observed;
    bad_scope.source.source_scope_parent = Some(bad_scope.source.source_scope);
    refresh_capture_accounting(&mut bad_scope);
    assert!(
        bad_scope
            .validate_untrusted_shape(CaptureLimitsV2::default())
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
        .validate_untrusted_shape(CaptureLimitsV2::default())
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
        .validate_untrusted_shape(CaptureLimitsV2::default())
        .unwrap_err()
        .to_string();
    assert!(
        signature_error.contains("signature structural binding"),
        "{signature_error}"
    );
}

#[test]
fn normalized_validation_binds_ordered_call_arguments_destination_and_control_flow() {
    let original = compiler_results().observed;
    let signature = calls(&original)
        .find_map(|kind| match kind {
            TerminatorKindV2::Call {
                callee:
                    CalleeIdentityV2::Direct {
                        declared_signature, ..
                    },
                arguments,
                ..
            } if arguments.len() >= 2 => Some(declared_signature),
            _ => None,
        })
        .expect("fixture must contain a multi-argument direct call");
    assert_eq!(signature.inputs.len(), 2);
    assert_ne!(signature.output.stable_hash, [0; 16]);
    assert_ne!(signature.abi.stable_hash, [0; 16]);
    assert_ne!(signature.binding_hash, [0; 32]);

    let mut removed = original.clone();
    let arguments = removed
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator.kind {
            TerminatorKindV2::Call { arguments, .. } if arguments.len() >= 2 => Some(arguments),
            _ => None,
        })
        .unwrap();
    arguments.pop();
    refresh_capture_accounting(&mut removed);
    let error = removed
        .validate_untrusted_shape(CaptureLimitsV2::default())
        .unwrap_err()
        .to_string();
    assert!(error.contains("argument count"), "{error}");

    let mut added = original.clone();
    let arguments = added
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator.kind {
            TerminatorKindV2::Call { arguments, .. } if arguments.len() >= 2 => Some(arguments),
            _ => None,
        })
        .unwrap();
    arguments.push(arguments[0].clone());
    refresh_capture_accounting(&mut added);
    let error = added
        .validate_untrusted_shape(CaptureLimitsV2::default())
        .unwrap_err()
        .to_string();
    assert!(error.contains("argument count"), "{error}");

    let mut reordered = original.clone();
    let arguments = reordered
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator.kind {
            TerminatorKindV2::Call { arguments, .. } if arguments.len() >= 2 => Some(arguments),
            _ => None,
        })
        .unwrap();
    arguments.swap(0, 1);
    refresh_capture_accounting(&mut reordered);
    let error = reordered
        .validate_untrusted_shape(CaptureLimitsV2::default())
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("ordered signature input") || error.contains("contract_hash"),
        "{error}"
    );

    let mut changed_destination = original.clone();
    let destination = changed_destination
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator.kind {
            TerminatorKindV2::Call {
                arguments,
                destination,
                ..
            } if arguments.len() >= 2 => Some(destination),
            _ => None,
        })
        .unwrap();
    destination.local = 0;
    refresh_capture_accounting(&mut changed_destination);
    let error = changed_destination
        .validate_untrusted_shape(CaptureLimitsV2::default())
        .unwrap_err()
        .to_string();
    assert!(error.contains("contract_hash"), "{error}");

    let mut bad_destination_type = original.clone();
    let destination = bad_destination_type
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator.kind {
            TerminatorKindV2::Call {
                arguments,
                destination,
                ..
            } if arguments.len() >= 2 => Some(destination),
            _ => None,
        })
        .unwrap();
    destination.type_hash[0] ^= 1;
    refresh_capture_accounting(&mut bad_destination_type);
    let error = bad_destination_type
        .validate_untrusted_shape(CaptureLimitsV2::default())
        .unwrap_err()
        .to_string();
    assert!(error.contains("destination place type"), "{error}");

    let mut bad_target = original;
    let target = bad_target
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator.kind {
            TerminatorKindV2::Call {
                arguments, target, ..
            } if arguments.len() >= 2 => Some(target),
            _ => None,
        })
        .unwrap();
    *target = None;
    refresh_capture_accounting(&mut bad_target);
    let error = bad_target
        .validate_untrusted_shape(CaptureLimitsV2::default())
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("target presence") || error.contains("normal target"),
        "{error}"
    );
}

#[test]
fn compiler_capture_binds_tail_call_to_the_caller_contract() {
    let original = compiler_results().tail_call;
    original
        .validate_untrusted_shape(CaptureLimitsV2::default())
        .unwrap();
    assert!(
        original
            .blocks
            .iter()
            .any(|block| matches!(block.terminator.kind, TerminatorKindV2::TailCall { .. }))
    );
    assert!(original.caller_signature.is_some());

    let mut missing_caller = original.clone();
    missing_caller.caller_signature = None;
    refresh_capture_accounting(&mut missing_caller);
    let error = missing_caller
        .validate_untrusted_shape(CaptureLimitsV2::default())
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("compiler-captured caller signature"),
        "{error}"
    );

    let mut changed_safety = original.clone();
    let signature = changed_safety.caller_signature.as_mut().unwrap();
    signature.safety = match signature.safety {
        FunctionSafetyV2::Safe => FunctionSafetyV2::Unsafe,
        FunctionSafetyV2::Unsafe => FunctionSafetyV2::Safe,
    };
    signature.binding_hash = function_signature_binding_hash_v2(signature).unwrap();
    refresh_capture_accounting(&mut changed_safety);
    let error = changed_safety
        .validate_untrusted_shape(CaptureLimitsV2::default())
        .unwrap_err()
        .to_string();
    assert!(error.contains("differs from the caller"), "{error}");

    let mut changed_unwind = original.clone();
    let signature = changed_unwind.caller_signature.as_mut().unwrap();
    signature.abi.canonical_name = "C".to_owned();
    signature.abi.unwind_allowed = false;
    signature.binding_hash = function_signature_binding_hash_v2(signature).unwrap();
    refresh_capture_accounting(&mut changed_unwind);
    let error = changed_unwind
        .validate_untrusted_shape(CaptureLimitsV2::default())
        .unwrap_err()
        .to_string();
    assert!(error.contains("differs from the caller"), "{error}");

    let mut changed_output = original;
    let signature = changed_output.caller_signature.as_mut().unwrap();
    signature.output.stable_hash[0] ^= 1;
    signature.binding_hash = function_signature_binding_hash_v2(signature).unwrap();
    refresh_capture_accounting(&mut changed_output);
    let error = changed_output
        .validate_untrusted_shape(CaptureLimitsV2::default())
        .unwrap_err()
        .to_string();
    assert!(error.contains("differs from the caller"), "{error}");
}

#[test]
fn normalized_validation_binds_indirect_callable_type_and_abi_unwind() {
    let mut indirect = compiler_results().function_pointer;
    let replacement = indirect
        .locals
        .iter()
        .find(|local| {
            local.role == LocalRoleV2::Argument
                && !matches!(local.ty.class, TypeClassV2::FunctionPointer)
        })
        .map(|local| (local.index, local.ty.stable_hash))
        .expect("fixture must contain a non-callable argument");
    let function = indirect
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator.kind {
            TerminatorKindV2::Call {
                function: OperandV2::Copy(place) | OperandV2::Move(place),
                callee: CalleeIdentityV2::Indirect { signature, .. },
                ..
            } => {
                assert_eq!(signature.inputs.len(), 1);
                Some(place)
            }
            _ => None,
        })
        .expect("fixture must contain an indirect call through a place");
    function.local = replacement.0;
    function.type_hash = replacement.1;
    refresh_capture_accounting(&mut indirect);
    let error = indirect
        .validate_untrusted_shape(CaptureLimitsV2::default())
        .unwrap_err()
        .to_string();
    assert!(error.contains("indirect callable operand type"), "{error}");

    let mut non_unwinding = compiler_results().c_call;
    let call = non_unwinding
        .blocks
        .iter_mut()
        .find_map(|block| match &mut block.terminator.kind {
            kind @ TerminatorKindV2::Call { .. } => Some(kind),
            _ => None,
        })
        .expect("fixture must contain a C ABI call");
    let TerminatorKindV2::Call {
        function,
        callee,
        arguments,
        destination,
        target,
        unwind,
        contract_hash,
        ..
    } = call
    else {
        unreachable!()
    };
    let signature = match callee {
        CalleeIdentityV2::Direct {
            declared_signature, ..
        } => declared_signature,
        CalleeIdentityV2::Indirect { .. } => unreachable!(),
    };
    assert_eq!(signature.abi.canonical_name, "C");
    assert!(!signature.abi.unwind_allowed);
    *unwind = UnwindActionV2::Continue;
    *contract_hash = call_contract_hash_v2(
        function,
        callee,
        arguments,
        Some(destination),
        *target,
        Some(unwind),
    )
    .unwrap();
    refresh_capture_accounting(&mut non_unwinding);
    let error = non_unwinding
        .validate_untrusted_shape(CaptureLimitsV2::default())
        .unwrap_err()
        .to_string();
    assert!(error.contains("non-unwinding ABI"), "{error}");
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
        .validate_untrusted_shape(CaptureLimitsV2::default())
        .unwrap_err()
        .to_string();
    assert!(removed_error.contains("if and only if"), "{removed_error}");

    for mutate in [
        |intrinsic: &mut IntrinsicIdentityV2| intrinsic.name = "white_box".to_owned(),
        |intrinsic: &mut IntrinsicIdentityV2| {
            intrinsic.must_be_overridden = !intrinsic.must_be_overridden;
        },
        |intrinsic: &mut IntrinsicIdentityV2| {
            intrinsic.const_stable = !intrinsic.const_stable;
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
            .validate_untrusted_shape(CaptureLimitsV2::default())
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
        .validate_untrusted_shape(CaptureLimitsV2::default())
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
        body.validate_untrusted_shape(CaptureLimitsV2::default())
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
    body.validate_untrusted_shape(CaptureLimitsV2::default())
        .unwrap();
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
            .validate_untrusted_shape(CaptureLimitsV2::default())
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
            .validate_untrusted_shape(CaptureLimitsV2::default())
            .unwrap_err()
            .to_string();
        assert!(error.contains("reported text count"), "{error}");
    }

    let work_error = original
        .validate_untrusted_shape(CaptureLimitsV2 {
            max_total_work_items: original.capture_work_items - 1,
            ..CaptureLimitsV2::default()
        })
        .unwrap_err()
        .to_string();
    assert!(work_error.contains("recomputed work bound"), "{work_error}");
    let text_error = original
        .validate_untrusted_shape(CaptureLimitsV2 {
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
    body.validate_untrusted_shape(CaptureLimitsV2::default())
        .unwrap();
    assert!(!body.is_authorized_for_lowering());
}

#[test]
fn canonical_remapped_spans_are_deterministic_across_source_roots() {
    let first = compile_observed("mir_v2_span_fixture", "same-identity");
    let second = compile_observed("mir_v2_span_fixture", "same-identity");
    let first_keys = canonical_span_keys(&first);
    let second_keys = canonical_span_keys(&second);
    assert_eq!(first_keys, second_keys);
    assert_eq!(first.source_scopes, second.source_scopes);
    assert!(first_keys.iter().all(|key| {
        key.authority == SourceAuthorityV2::CanonicalRemapped
            && key.remapped_file == "/workspace/fixture.rs"
    }));
}

#[test]
fn normalized_validation_binds_canonical_source_scope_records_topologically() {
    let original = compiler_results().observed;
    assert!(!original.source_scopes.is_empty());

    let mut bad_span_record = original.clone();
    bad_span_record.source.source_scope_record_hash[0] ^= 1;
    let span_error = bad_span_record
        .validate_untrusted_shape(CaptureLimitsV2::default())
        .unwrap_err()
        .to_string();
    assert!(span_error.contains("exactly match"), "{span_error}");

    let mut bad_record = original.clone();
    bad_record.source_scopes[0].record_hash[0] ^= 1;
    let record_error = bad_record
        .validate_untrusted_shape(CaptureLimitsV2::default())
        .unwrap_err()
        .to_string();
    assert!(record_error.contains("record binding"), "{record_error}");

    if original.source_scopes.len() > 1 {
        let mut cyclic = original.clone();
        cyclic.source_scopes[1].parent = Some(1);
        refresh_capture_accounting(&mut cyclic);
        let cycle_error = cyclic
            .validate_untrusted_shape(CaptureLimitsV2::default())
            .unwrap_err()
            .to_string();
        assert!(
            cycle_error.contains("earlier canonical scope"),
            "{cycle_error}"
        );
    }

    let mut bad_inlined = original.clone();
    let function = bad_inlined.function.clone();
    let scope = bad_inlined
        .source_scopes
        .last_mut()
        .expect("captured body must have a source scope");
    scope.inlined = Some(function);
    scope.inlined_callsite = Some(scope.scope_span.clone());
    scope.record_hash = source_scope_record_hash_v2(scope).unwrap();
    scope.inlined.as_mut().unwrap().instance.instance_hash[0] ^= 1;
    refresh_capture_accounting(&mut bad_inlined);
    let inlined_error = bad_inlined
        .validate_untrusted_shape(CaptureLimitsV2::default())
        .unwrap_err()
        .to_string();
    assert!(inlined_error.contains("record binding"), "{inlined_error}");

    let tight_error = original
        .validate_untrusted_shape(CaptureLimitsV2 {
            max_source_scopes: original.source_scopes.len() - 1,
            ..CaptureLimitsV2::default()
        })
        .unwrap_err()
        .to_string();
    assert!(tight_error.contains("source_scopes"), "{tight_error}");
}

#[test]
fn macro_expansion_identity_is_bounded_deterministic_and_does_not_collapse() {
    let source = r#"
#![allow(dead_code)]
macro_rules! two_assignments {
    ($value:ident) => {{
        $value = $value.wrapping_add(1);
        $value = $value.wrapping_add(2);
    }};
}

#[inline(never)]
fn observed(mut value: u32) -> u32 {
    two_assignments!(value);
    value
}
"#;
    let first = compile_source_observed(source, "mir_v2_macro_fixture", "same-macro");
    let second = compile_source_observed(source, "mir_v2_macro_fixture", "same-macro");
    first
        .validate_untrusted_shape(CaptureLimitsV2::default())
        .unwrap();
    let first_keys = canonical_span_keys(&first);
    let second_keys = canonical_span_keys(&second);
    assert_eq!(first_keys, second_keys);
    assert_eq!(first.source_scopes, second.source_scopes);

    let expanded = first_keys
        .iter()
        .filter(|key| !key.expansion.frames.is_empty())
        .collect::<Vec<_>>();
    assert!(
        !expanded.is_empty(),
        "fixture must retain expanded MIR spans"
    );
    assert!(expanded.iter().all(|key| {
        key.expansion.chain_hash == expansion_chain_hash_v2(&key.expansion).unwrap()
    }));
    assert!(expanded.iter().enumerate().any(|(index, left)| {
        expanded[index + 1..].iter().any(|right| {
            left.span_hash == right.span_hash && left.original_span_hash != right.original_span_hash
        })
    }));

    let mut nested = String::new();
    for index in 0..12 {
        let next = index + 1;
        nested.push_str(&format!(
            "macro_rules! m{index} {{ ($value:ident) => {{ m{next}!($value); }}; }}\n"
        ));
    }
    nested.push_str("macro_rules! m12 { ($value:ident) => { $value += 1; }; }\n");
    nested.push_str("#[inline(never)]\nfn observed(mut value: u32) -> u32 { m0!(value); value }\n");
    let bound_error = compile_capture_error(
        &nested,
        CaptureLimitsV2 {
            max_macro_expansion_depth: 4,
            ..CaptureLimitsV2::default()
        },
    );
    assert!(bound_error.contains("macro expansion"), "{bound_error}");
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
