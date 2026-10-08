use super::*;
use crate::test_temp_dir::TestTempDir;
use fe2o3_mir_model::semantic_mir_v1::SemanticMirResourceV1;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::Compiler;
use rustc_middle::mir::TerminatorKind;
use rustc_middle::ty::{AliasTyKind, TyKind};

const SOURCE: &str = r#"
#![no_std]
#![recursion_limit = "512"]
#![allow(unused_unsafe, unused_braces)]
extern crate inline_source_dependency;
#[inline(always)]
fn safe_helper(value: u32) -> u32 { value }
pub fn safe_root(value: u32) -> u32 { safe_helper(value) }
#[inline(always)]
fn empty_unsafe(value: u32) -> u32 { unsafe {}; value }
pub fn unsafe_root(value: u32) -> u32 { empty_unsafe(value) }
#[inline(always)]
unsafe fn unsafe_signature(value: u32) -> u32 { value }
pub fn signature_root(value: u32) -> u32 { unsafe { unsafe_signature(value) } }
#[inline(always)]
fn outer_helper(value: u32) -> u32 { empty_unsafe(value) }
pub fn descendant_root(value: u32) -> u32 { outer_helper(value) }
#[inline(always)]
fn unused_closure(value: u32) -> u32 {
    let _uncalled = || { unsafe {}; value };
    value
}
pub fn unused_root(value: u32) -> u32 { unused_closure(value) }
#[inline(always)]
fn called_closure(value: u32) -> u32 {
    let called = || { unsafe {}; value };
    called()
}
pub fn closure_root(value: u32) -> u32 { called_closure(value) }
#[inline(always)]
fn called_safe_closure(value: u32) -> u32 {
    let called = || value;
    called()
}
pub fn safe_closure_root(value: u32) -> u32 { called_safe_closure(value) }
#[inline(always)]
fn apply_once<F: FnOnce() -> u32>(function: F) -> u32 { function() }
#[inline(always)]
fn through_fn<F: Fn() -> u32>(function: F) -> u32 { apply_once(function) }
pub fn once_root(value: u32) -> u32 { through_fn(|| value) }
pub fn unsafe_once_root(value: u32) -> u32 { through_fn(|| { unsafe {}; value }) }
#[inline(always)]
fn generic<T>(value: T) -> T { value }
pub fn generic_root(value: u32) -> u32 { generic(value) }
#[inline(always)]
fn unsafe_generic<T>(value: T) -> T { unsafe {}; value }
trait Project { type Output; }
struct Tag;
impl Project for Tag { type Output = u32; }
#[inline(never)]
fn generic_caller<T: Project>(value: T::Output) -> T::Output {
    generic::<T::Output>(value)
}
#[inline(never)]
fn unsafe_generic_caller<T: Project>(value: T::Output) -> T::Output {
    unsafe_generic::<T::Output>(value)
}
pub fn generic_entry(value: u32) -> (u32, u32) {
    (generic_caller::<Tag>(value), unsafe_generic_caller::<Tag>(value))
}
#[inline(always)]
fn deep_helper(value: u32) -> u32 { DEEP_HIR_EXPRESSION }
pub fn deep_root(value: u32) -> u32 { deep_helper(value) }
pub fn external_root(value: u32) -> u32 { inline_source_dependency::helper(value) }
pub fn reviewed_core_root(value: u32) -> u32 { value.wrapping_add(1) }
"#;

#[derive(Clone, Copy)]
enum Mode {
    Origins,
    Budget,
    External,
}

struct AuditCallbacks {
    mode: Mode,
    complete: bool,
}

fn root<'tcx>(tcx: TyCtxt<'tcx>, name: &str) -> Instance<'tcx> {
    let id = tcx
        .hir_body_owners()
        .find(|owner| {
            tcx.def_kind(*owner) == DefKind::Fn && tcx.item_name(owner.to_def_id()).as_str() == name
        })
        .unwrap_or_else(|| panic!("missing actual source function {name}"));
    Instance::new_raw(id.to_def_id(), tcx.mk_args(&[]))
}

fn observed<'tcx>(tcx: TyCtxt<'tcx>, caller: Instance<'tcx>, expected: Instance<'tcx>) {
    let body = tcx.instance_mir(caller.def);
    assert!(
        body.source_scopes.iter().any(|scope| {
            scope
                .inlined
                .is_some_and(|(instance, _)| instance == expected)
        }),
        "optimized MIR must retain the actual original Instance {expected:?}"
    );
}

fn checked<'tcx>(tcx: TyCtxt<'tcx>, caller: Instance<'tcx>) -> Result<(), AuditError<'tcx>> {
    audit(
        tcx,
        caller,
        tcx.instance_mir(caller.def),
        &mut SourceClosureWorkV1::default(),
        |_, _, _| {
            panic!("local fixtures must not need an external authority");
        },
    )
}

impl Callbacks for AuditCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        let caller = root(tcx, "safe_root");
        let helper = root(tcx, "safe_helper");
        observed(tcx, caller, helper);
        checked(tcx, caller).unwrap();
        match self.mode {
            Mode::Origins => {
                for (root_name, helper_name, signature) in [
                    ("unsafe_root", "empty_unsafe", false),
                    ("signature_root", "unsafe_signature", true),
                    ("descendant_root", "empty_unsafe", false),
                ] {
                    let caller = root(tcx, root_name);
                    let original = root(tcx, helper_name);
                    observed(tcx, caller, original);
                    match checked(tcx, caller).unwrap_err() {
                        AuditError::Origin {
                            instance,
                            source,
                            callsite,
                            reason,
                        } => {
                            assert_eq!(instance, original);
                            assert_eq!(source, original);
                            assert!(!callsite.is_dummy());
                            match reason {
                                OriginRefusal::UnsafeSignature => assert!(signature),
                                OriginRefusal::UserUnsafeBlock(span) => {
                                    assert!(!signature);
                                    assert!(!span.is_dummy());
                                    let owner = source.def_id().as_local().unwrap();
                                    let hir = tcx.hir_maybe_body_owned_by(owner).unwrap();
                                    assert!(hir.value.span.contains(span));
                                    assert!(
                                        !tcx.sess
                                            .source_map()
                                            .span_to_snippet(span)
                                            .unwrap()
                                            .is_empty()
                                    );
                                }
                                other => panic!("wrong original-source refusal: {other:?}"),
                            }
                        }
                        error => panic!("wrong audit error: {error:?}"),
                    }
                }
                let unused = root(tcx, "unused_root");
                observed(tcx, unused, root(tcx, "unused_closure"));
                checked(tcx, unused).unwrap();
                let closure_root = root(tcx, "closure_root");
                let closure_body = tcx.instance_mir(closure_root.def);
                let closure = closure_body
                    .source_scopes
                    .iter()
                    .filter_map(|scope| scope.inlined)
                    .map(|(instance, _)| instance)
                    .find(|instance| tcx.def_kind(instance.def_id()) == DefKind::Closure)
                    .expect("actual invoked closure original survives optimized MIR");
                match checked(tcx, closure_root).unwrap_err() {
                    AuditError::Origin {
                        source,
                        reason: OriginRefusal::UserUnsafeBlock(_),
                        ..
                    } => assert_eq!(source, closure),
                    error => panic!("wrong invoked closure refusal: {error:?}"),
                }
                for name in ["safe_closure_root"] {
                    let caller = root(tcx, name);
                    let body = tcx.instance_mir(caller.def);
                    assert!(
                        body.source_scopes
                            .iter()
                            .filter_map(|scope| scope.inlined)
                            .any(
                                |(instance, _)| tcx.def_kind(instance.def_id()) == DefKind::Closure
                            ),
                        "actual safe called closure source origin"
                    );
                    checked(tcx, caller).unwrap();
                }
                for (name, unsafe_source) in [("once_root", false), ("unsafe_once_root", true)] {
                    let once = root(tcx, name);
                    let shim = tcx
                        .instance_mir(once.def)
                        .source_scopes
                        .iter()
                        .filter_map(|scope| scope.inlined)
                        .map(|(instance, _)| instance)
                        .find(|instance| {
                            matches!(instance.def, InstanceKind::ClosureOnceShim { .. })
                        })
                        .expect(
                            "actual generated FnOnce adapter provenance, not a synthetic instance",
                        );
                    let closure = crate::closure_profile_v1::authenticate_once_shim_v1(tcx, shim)
                        .unwrap()
                        .expect("actual retained adapter must authenticate its closure");
                    assert!(
                        tcx.instance_mir(once.def).basic_blocks.iter().any(|block| {
                            let TerminatorKind::Call { func, .. } = &block.terminator().kind else {
                                return false;
                            };
                            crate::closure_profile_v1::resolve_direct_call(tcx, once, func)
                                .expect("actual residual closure call must resolve")
                                == closure
                        }),
                        "retained adapter and residual call must name the same original closure",
                    );
                    if unsafe_source {
                        match checked(tcx, once).unwrap_err() {
                            AuditError::Origin {
                                instance,
                                source,
                                callsite,
                                reason: OriginRefusal::UserUnsafeBlock(span),
                            } => {
                                assert_eq!(instance, shim);
                                assert_eq!(source, closure);
                                assert!(!callsite.is_dummy());
                                assert!(!span.is_dummy());
                                let hir = tcx
                                    .hir_maybe_body_owned_by(source.def_id().as_local().unwrap())
                                    .unwrap();
                                assert!(hir.value.span.contains(span));
                            }
                            error => panic!("wrong adapter source refusal: {error:?}"),
                        }
                    } else {
                        checked(tcx, once).unwrap();
                    }
                }
                let generic_root = root(tcx, "generic_root");
                let generic_body = tcx.instance_mir(generic_root.def);
                let generic = generic_body
                    .source_scopes
                    .iter()
                    .filter_map(|scope| scope.inlined)
                    .map(|(instance, _)| instance)
                    .find(|instance| tcx.item_name(instance.def_id()).as_str() == "generic")
                    .expect("actual generic inline origin");
                assert_eq!(generic.args, tcx.mk_args(&[tcx.types.u32.into()]));
                checked(tcx, generic_root).unwrap();
                let entry = root(tcx, "generic_entry");
                for (caller_name, helper_name, unsafe_source) in [
                    ("generic_caller", "generic", false),
                    ("unsafe_generic_caller", "unsafe_generic", true),
                ] {
                    let generic_caller = tcx
                        .instance_mir(entry.def)
                        .basic_blocks
                        .iter()
                        .filter_map(|block| match &block.terminator().kind {
                            TerminatorKind::Call { func, .. } => Some(
                                crate::closure_profile_v1::resolve_direct_call(tcx, entry, func)
                                    .expect("actual generic entry call must resolve"),
                            ),
                            _ => None,
                        })
                        .find(|instance| tcx.item_name(instance.def_id()).as_str() == caller_name)
                        .expect("actual concrete generic caller must remain a call");
                    assert_eq!(generic_caller.args.len(), 1);
                    assert!(super::super::is_fully_monomorphized(tcx, generic_caller));
                    let original = tcx
                        .instance_mir(generic_caller.def)
                        .source_scopes
                        .iter()
                        .filter_map(|scope| scope.inlined)
                        .map(|(instance, _)| instance)
                        .find(|instance| tcx.item_name(instance.def_id()).as_str() == helper_name)
                        .expect("actual projected generic inline origin");
                    assert_eq!(original.args.len(), 1);
                    assert!(matches!(
                        original.args.type_at(0).kind(),
                        TyKind::Alias(AliasTyKind::Projection, _)
                    ));
                    assert!(!super::super::is_fully_monomorphized(tcx, original));
                    let substituted =
                        EarlyBinder::bind(original).instantiate(tcx, generic_caller.args);
                    assert_ne!(
                        substituted, original,
                        "caller substitution must change the origin"
                    );
                    let normalized = tcx
                        .try_normalize_erasing_regions(
                            TypingEnv::fully_monomorphized(),
                            substituted,
                        )
                        .unwrap();
                    assert_ne!(
                        normalized, substituted,
                        "projection normalization must be necessary"
                    );
                    assert_eq!(normalized.args, tcx.mk_args(&[tcx.types.u32.into()]));
                    if unsafe_source {
                        match checked(tcx, generic_caller).unwrap_err() {
                            AuditError::Origin {
                                instance,
                                source,
                                callsite,
                                reason: OriginRefusal::UserUnsafeBlock(span),
                            } => {
                                assert_eq!(instance, normalized);
                                assert_eq!(source, normalized);
                                assert!(!callsite.is_dummy());
                                let hir = tcx
                                    .hir_maybe_body_owned_by(source.def_id().as_local().unwrap())
                                    .unwrap();
                                assert!(hir.value.span.contains(span));
                            }
                            error => panic!("wrong normalized generic source refusal: {error:?}"),
                        }
                    } else {
                        checked(tcx, generic_caller).unwrap();
                    }
                }
                let deep = root(tcx, "deep_root");
                let deep_helper = root(tcx, "deep_helper");
                observed(tcx, deep, deep_helper);
                assert!(
                    matches!(checked(tcx, deep), Err(AuditError::Origin {
                    instance, source, reason: OriginRefusal::HirDepth, ..
                }) if instance == deep_helper && source == deep_helper),
                    "actual original HIR must reach the bounded visitor depth"
                );
                assert!(matches!(
                    audit(
                        tcx,
                        caller,
                        tcx.instance_mir(unused.def),
                        &mut SourceClosureWorkV1::default(),
                        |_, _, _| Ok(false)
                    ),
                    Err(AuditError::CallerBodyMismatch { .. })
                ));
                let cloned = tcx.instance_mir(caller.def).clone();
                assert_eq!(cloned.source.instance, caller.def);
                assert!(
                    matches!(
                        audit(
                            tcx,
                            caller,
                            &cloned,
                            &mut SourceClosureWorkV1::default(),
                            |_, _, _| Ok(false)
                        ),
                        Err(AuditError::CallerBodyMismatch { .. })
                    ),
                    "an identical detached Body is not the authenticated compiler owner"
                );
            }
            Mode::Budget => {
                let body = tcx.instance_mir(caller.def);
                let before = format!("{body:?}");
                let mut measured = SourceClosureWorkV1::default();
                audit(tcx, caller, body, &mut measured, |_, _, _| Ok(false)).unwrap();
                let cost = measured.validation_work_for_test();
                assert!(
                    cost > body.source_scopes.len() as u64 + 1,
                    "actual local HIR visits must be debited beyond the source-scope census"
                );
                for short in [false, true] {
                    let mut work = SourceClosureWorkV1::default();
                    let maximum = work.limits().limit(SemanticMirResourceV1::ValidationWork);
                    let floor = maximum - cost + u64::from(short);
                    work.charge(usize::try_from(floor).unwrap()).unwrap();
                    let result = audit(tcx, caller, body, &mut work, |_, _, _| Ok(false));
                    if short {
                        assert!(matches!(result, Err(AuditError::Work(
                            ProductionSemanticPreflightErrorV1::LimitExceeded {
                                resource: SemanticMirResourceV1::ValidationWork,
                                actual, maximum: limit,
                            })) if actual == maximum + 1 && limit == maximum));
                    } else {
                        result.unwrap();
                    }
                    assert_eq!(work.validation_work_for_test(), maximum + u64::from(short));
                }
                let mut exhausted = SourceClosureWorkV1::default();
                let maximum = exhausted
                    .limits()
                    .limit(SemanticMirResourceV1::ValidationWork);
                exhausted.charge(usize::try_from(maximum).unwrap()).unwrap();
                assert!(matches!(
                    audit(tcx, caller, body, &mut exhausted, |_, _, _| Ok(false)),
                    Err(AuditError::Work(_))
                ));
                let mut depth_work = SourceClosureWorkV1::default();
                let local = helper.def_id().as_local().unwrap();
                let hir = tcx.hir_maybe_body_owned_by(local).unwrap();
                let mut visitor = SourceVisitor {
                    work: &mut depth_work,
                    depth: MAX_HIR_DEPTH,
                    instance: helper,
                    source: helper,
                    callsite: body.span,
                };
                assert!(matches!(
                    visitor.visit_body(hir),
                    ControlFlow::Break(AuditError::Origin {
                        reason: OriginRefusal::HirDepth,
                        ..
                    })
                ));
                assert_eq!(depth_work.validation_work_for_test(), 1);
                assert_eq!(format!("{body:?}"), before);
            }
            Mode::External => {
                let caller = root(tcx, "external_root");
                let body = tcx.instance_mir(caller.def);
                let external = body
                    .source_scopes
                    .iter()
                    .filter_map(|scope| scope.inlined)
                    .map(|(instance, _)| instance)
                    .find(|instance| !instance.def_id().is_local())
                    .expect("real dependency body must actually inline");
                assert_eq!(
                    tcx.crate_name(external.def_id().krate).as_str(),
                    "inline_source_dependency"
                );
                let mut visits = 0;
                let result = audit(
                    tcx,
                    caller,
                    body,
                    &mut SourceClosureWorkV1::default(),
                    |instance, callsite, work| {
                        work.charge(1)?;
                        assert_eq!(instance, external);
                        assert!(!callsite.is_dummy());
                        visits += 1;
                        Ok(false)
                    },
                );
                assert_eq!(visits, 1);
                assert!(matches!(result, Err(AuditError::Origin {
                    instance, reason: OriginRefusal::UnknownExternalSource, ..
                }) if instance == external));
                let caller = root(tcx, "reviewed_core_root");
                let body = tcx.instance_mir(caller.def);
                let reviewed = body
                    .source_scopes
                    .iter()
                    .filter_map(|scope| scope.inlined)
                    .map(|(instance, _)| instance)
                    .find(|instance| !instance.def_id().is_local())
                    .expect("real reviewed core body must actually inline");
                assert!(matches!(reviewed.def, InstanceKind::Item(_)));
                assert_eq!(tcx.crate_name(reviewed.def_id().krate).as_str(), "core");
                assert_eq!(
                    tcx.def_path_str(reviewed.def_id()),
                    "core::num::<impl u32>::wrapping_add"
                );
                let mut visits = 0;
                audit(
                    tcx,
                    caller,
                    body,
                    &mut SourceClosureWorkV1::default(),
                    |instance, callsite, work| {
                        work.charge(1)?;
                        assert_eq!(instance, reviewed);
                        assert!(!callsite.is_dummy());
                        visits += 1;
                        let authenticated =
                            reviewed_external_source(tcx, instance, "gfx942:xnack-").expect(
                                "reviewed core source policy must authenticate without error",
                            );
                        assert!(
                            authenticated,
                            "actual reviewed core owner must satisfy production policy"
                        );
                        Ok(authenticated)
                    },
                )
                .unwrap();
                assert_eq!(
                    visits, 1,
                    "actual retained core owner must reach production policy"
                );
            }
        }
        self.complete = true;
        Compilation::Stop
    }
}

fn run(mode: Mode) {
    let directory = TestTempDir::create("fe2o3-inlined-source-safety");
    let source = directory.path().join("fixture.rs");
    assert_eq!(SOURCE.matches("DEEP_HIR_EXPRESSION").count(), 1);
    let deep_expression = format!("{}value{}", "{".repeat(160), "}".repeat(160));
    let fixture = SOURCE.replace("DEEP_HIR_EXPRESSION", &deep_expression);
    std::fs::write(&source, fixture).unwrap();
    let dependency = directory.path().join("dependency.rs");
    std::fs::write(
        &dependency,
        "#![no_std]\n#[inline(always)] pub fn helper(value: u32) -> u32 { value }\n",
    )
    .unwrap();
    let sysroot = std::process::Command::new("rustc")
        .args(["--print", "sysroot"])
        .output()
        .unwrap();
    assert!(sysroot.status.success());
    let sysroot = String::from_utf8(sysroot.stdout).unwrap();
    let artifact = directory.path().join("libinline_source_dependency.rlib");
    let dependency_output = std::process::Command::new("rustc")
        .args([
            "--crate-name=inline_source_dependency",
            "--crate-type=rlib",
            "--edition=2024",
            "-Zalways-encode-mir",
            "-Cpanic=abort",
            "--sysroot",
            sysroot.trim(),
            "-o",
        ])
        .arg(&artifact)
        .arg(&dependency)
        .output()
        .unwrap();
    assert!(
        dependency_output.status.success(),
        "{}",
        String::from_utf8_lossy(&dependency_output.stderr)
    );
    let args = vec![
        "rustc".into(),
        "--crate-name=fe2o3_inline_source_fixture".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        "--emit=metadata".into(),
        "-Copt-level=3".into(),
        "-Zmir-opt-level=2".into(),
        "-Cpanic=abort".into(),
        "--sysroot".into(),
        sysroot.trim().into(),
        "--extern".into(),
        format!("inline_source_dependency={}", artifact.display()),
        "-o".into(),
        directory.path().join("fixture.rmeta").display().to_string(),
        source.display().to_string(),
    ];
    let mut callbacks = AuditCallbacks {
        mode,
        complete: false,
    };
    rustc_driver::run_compiler(&args, &mut callbacks);
    assert!(
        callbacks.complete,
        "actual compiler callback must run to completion"
    );
}

#[test]
fn actual_optimized_origins_preserve_local_source_safety() {
    run(Mode::Origins);
}

#[test]
fn actual_optimized_origin_audit_has_exact_and_one_short_work() {
    run(Mode::Budget);
}

#[test]
fn actual_optimized_external_origin_requires_exact_authentication() {
    run(Mode::External);
}
