//! Real external callbacks from complete cached AMD metadata, without building dependencies.

use super::*;
use crate::test_temp_dir::TestTempDir;
use fe2o3_rustc_invocation::derive_cargo_metadata_build_observation_v2;
use rustc_driver::{Callbacks, Compilation};
use rustc_interface::interface::{Compiler, Config};
use rustc_middle::{mir::RETURN_PLACE, ty::EarlyBinder};
use rustc_session::config::Input;
use rustc_span::FileName;
use std::process::Command;

const METADATA: &str = "fe2o3-reviewed-external-closure-v1";
const CHILD_ENV: &str = "FE2O3_REVIEWED_CLOSURE_TEST_CHILD";
const OUTPUT_ENV: &str = "FE2O3_REVIEWED_CLOSURE_TEST_OUTPUT";
const MAX_DEFINITIONS: usize = 128;
const MAX_BLOCKS: usize = 128;
const MAX_LOCALS: usize = 256;
const MAX_STATEMENTS: usize = 4096;
const MAX_ARGUMENTS: usize = 64;

const SOURCE: &str = r#"
#![no_std]
use fe2o3_device::{
    Bf16MatrixViewError, CurrentTarget, Global, GlobalBf16MfmaAMatrix,
    KernelCapabilityBrand, PolicyMatrixCapability, ReadOnly, RegisteredLaunch, StrictIeee,
};
pub struct KernelMarker;
type Brand = KernelCapabilityBrand<'static, KernelMarker, CurrentTarget, RegisteredLaunch>;

pub fn matrix_checked<'view>(
    matrix: &PolicyMatrixCapability<'_, Brand, Brand, StrictIeee>,
    bits: &'view Global<'static, u16, ReadOnly, Brand>,
) -> Result<GlobalBf16MfmaAMatrix<'view, 'static, Brand, Brand>, Bf16MatrixViewError> {
    matrix.bf16_a_global_row_major(bits, 0, 1, 1, 1)
}

pub fn local_callback(value: Result<u32, u32>) -> Result<u32, u32> {
    value.map_err(|error| error)
}

pub fn core_callback(
    values: core::iter::Map<core::ops::Range<u32>, fn(u32) -> u32>,
    fold: fn(u32, u32) -> u32,
) -> u32 {
    values.fold(0, fold)
}

macro_rules! expanded_callback {
    () => {
        pub fn expanded_callback(value: Result<u32, u32>) -> Result<u32, u32> {
            value.map_err(|error| error)
        }
    };
}
expanded_callback!();
"#;

#[derive(Clone, Copy)]
enum Case {
    MatrixCallback,
    WrongCrates,
    ExpandedSpans,
}

fn local_function(tcx: TyCtxt<'_>, name: &str) -> DefId {
    let definitions = tcx
        .iter_local_def_id()
        .take(MAX_DEFINITIONS + 1)
        .collect::<Vec<_>>();
    assert!(definitions.len() <= MAX_DEFINITIONS);
    let functions = definitions
        .into_iter()
        .filter(|id| {
            tcx.def_kind(*id) == DefKind::Fn && tcx.item_name(id.to_def_id()).as_str() == name
        })
        .collect::<Vec<_>>();
    assert_eq!(functions.len(), 1, "fixture function {name}");
    functions[0].to_def_id()
}

fn bounded_body<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> &'tcx Body<'tcx> {
    assert!(instance.args.len() <= MAX_ARGUMENTS);
    let body = tcx.instance_mir(instance.def);
    assert!(body.basic_blocks.len() <= MAX_BLOCKS);
    assert!(body.local_decls.len() <= MAX_LOCALS);
    let mut remaining = MAX_STATEMENTS;
    for block in body.basic_blocks.iter() {
        remaining = remaining
            .checked_sub(block.statements.len())
            .expect("fixture statement budget");
    }
    body
}

fn direct_callees<'tcx>(tcx: TyCtxt<'tcx>, caller: Instance<'tcx>) -> Vec<Instance<'tcx>> {
    bounded_body(tcx, caller)
        .basic_blocks
        .iter()
        .filter_map(|block| {
            let TerminatorKind::Call { func, args, .. } = &block.terminator().kind else {
                return None;
            };
            assert!(args.len() <= MAX_ARGUMENTS);
            let Operand::Constant(callee) = func else {
                return None;
            };
            let TyKind::FnDef(definition, arguments) = callee.const_.ty().kind() else {
                return None;
            };
            assert!(arguments.len() <= MAX_ARGUMENTS);
            let arguments = caller
                .try_instantiate_mir_and_normalize_erasing_regions(
                    tcx,
                    TypingEnv::fully_monomorphized(),
                    EarlyBinder::bind(*arguments),
                )
                .expect("normalize the actual call's arguments");
            Some(
                Instance::try_resolve(
                    tcx,
                    TypingEnv::fully_monomorphized(),
                    *definition,
                    arguments,
                )
                .expect("resolve the actual call")
                .expect("concrete fixture callee"),
            )
        })
        .collect()
}

fn unique<T>(values: impl IntoIterator<Item = T>, context: &str) -> T {
    let mut values = values.into_iter();
    let value = values.next().unwrap_or_else(|| panic!("missing {context}"));
    assert!(values.next().is_none(), "ambiguous {context}");
    value
}

// Names select only test fixture callsites; production authentication gets the
// real resolved DefId and never receives a source-name exception.
fn named_call<'tcx>(tcx: TyCtxt<'tcx>, caller: Instance<'tcx>, name: &str) -> Instance<'tcx> {
    unique(
        direct_callees(tcx, caller).into_iter().filter(|callee| {
            matches!(
                tcx.def_kind(callee.def_id()),
                DefKind::Fn | DefKind::AssocFn
            ) && tcx.item_name(callee.def_id()).as_str() == name
        }),
        name,
    )
}

fn map_err_callback<'tcx>(tcx: TyCtxt<'tcx>, caller: Instance<'tcx>) -> Instance<'tcx> {
    let map_err = named_call(tcx, caller, "map_err");
    assert_eq!(
        map_err.def_id().krate,
        tcx.lang_items().sized_trait().unwrap().krate
    );
    let callback = unique(
        direct_callees(tcx, map_err).into_iter().filter(|callee| {
            matches!(callee.def, InstanceKind::ClosureOnceShim { .. })
                || tcx.def_kind(callee.def_id()) == DefKind::Closure
        }),
        "map_err's concrete closure invocation",
    );
    match callback.def {
        InstanceKind::Item(_) => callback,
        InstanceKind::ClosureOnceShim { call_once, .. } => {
            assert_eq!(
                tcx.trait_of_assoc(call_once),
                tcx.lang_items().fn_once_trait()
            );
            let environment = callback.args[0].expect_ty();
            let TyKind::Closure(definition, arguments) = environment.kind() else {
                panic!("FnOnce shim must carry the actual closure environment");
            };
            unique(
                direct_callees(tcx, callback).into_iter().filter(|callee| {
                    callee.def == InstanceKind::Item(*definition) && callee.args == *arguments
                }),
                "FnOnce shim's retained closure body",
            )
        }
        _ => panic!("unexpected callback instance {callback:?}"),
    }
}

fn check_matrix_callback(tcx: TyCtxt<'_>) {
    let root = Instance::mono(tcx, local_function(tcx, "matrix_checked"));
    let public = named_call(tcx, root, "bf16_a_global_row_major");
    let checked = named_call(tcx, public, "checked");
    assert_eq!(tcx.def_kind(checked.def_id()), DefKind::AssocFn);
    assert_ne!(checked.def_id().krate, LOCAL_CRATE);
    assert_eq!(
        tcx.crate_name(checked.def_id().krate).as_str(),
        "fe2o3_device"
    );
    let owner = tcx
        .impl_of_assoc(checked.def_id())
        .expect("matrix inherent impl");
    let self_ty = tcx.type_of(owner).instantiate_identity();
    let TyKind::Adt(matrix, _) = self_ty.kind() else {
        panic!("matrix helper must have a real ADT owner");
    };
    assert!(tcx.is_diagnostic_item(
        Symbol::intern("fe2o3_device_bf16_mfma_global_matrix_view_v1"),
        matrix.did(),
    ));

    let callback = map_err_callback(tcx, checked);
    let closure = callback.def_id();
    assert_eq!(tcx.opt_parent(closure), Some(checked.def_id()));
    assert_eq!(closure.krate, checked.def_id().krate);
    assert!(authenticate_reviewed_safe_external_helper_v1(tcx, checked.def_id()).unwrap());
    assert!(authenticate(tcx, closure).unwrap());
    assert!(authenticate_reviewed_safe_external_helper_v1(tcx, closure).unwrap());

    let child_span = body_span(tcx, closure).unwrap();
    let parent_span = body_span(tcx, checked.def_id()).unwrap();
    assert!(nested_spans(child_span, parent_span));
    assert!(
        !nested_spans(tcx.def_span(closure), tcx.def_span(checked.def_id())),
        "real callback lies in the method body, not its header"
    );
    let child_path = compiled_provider_source_path_v1(tcx, closure).unwrap();
    assert_eq!(
        child_path,
        compiled_provider_source_path_v1(tcx, checked.def_id()).unwrap()
    );
    let file = tcx.sess.source_map().lookup_source_file(child_span.lo());
    assert_eq!(file.cnum, closure.krate);
    assert!(parent_span.lo() >= file.start_pos && parent_span.hi() <= file.end_position());

    assert!(stable_nominal_provider_path_v1(tcx, closure).is_err());
    assert!(classify(tcx, closure).is_none());
    assert!(crate::production_semantic_terminal_v1::classify(tcx, closure).is_none());
    let original = bounded_body(tcx, callback);
    assert_eq!(original.source.instance, InstanceKind::Item(closure));
    assert!(original.source.promoted.is_none());
    assert!(std::ptr::eq(original, tcx.optimized_mir(closure)));
    assert!(
        original
            .basic_blocks
            .iter()
            .any(|block| { matches!(block.terminator().kind, TerminatorKind::SwitchInt { .. }) }),
        "retain the actual error-mapping match"
    );
    let production = production_mir_v1(tcx, callback);
    assert_eq!(production.instance(), callback);
    assert!(!production.is_source_expansion());
    assert!(production.expansion_fingerprint(tcx).is_none());
    assert!(
        std::ptr::eq(production.body(), original),
        "source authentication must not replace MIR"
    );
}

fn check_wrong_crates(tcx: TyCtxt<'_>) {
    let local_root = Instance::mono(tcx, local_function(tcx, "local_callback"));
    let local = map_err_callback(tcx, local_root).def_id();
    assert_eq!(local.krate, LOCAL_CRATE);
    assert_eq!(tcx.opt_parent(local), Some(local_root.def_id()));
    assert!(!authenticate(tcx, local).unwrap());
    assert!(!authenticate_reviewed_safe_external_helper_v1(tcx, local).unwrap());

    let core_root = Instance::mono(tcx, local_function(tcx, "core_callback"));
    let fold = named_call(tcx, core_root, "fold");
    let factory = named_call(tcx, fold, "map_fold");
    let body = bounded_body(tcx, factory);
    let return_ty = factory
        .try_instantiate_mir_and_normalize_erasing_regions(
            tcx,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(body.local_decls[RETURN_PLACE].ty),
        )
        .unwrap();
    let TyKind::Closure(core, _) = return_ty.kind() else {
        panic!("core map_fold must return its real closure, got {return_ty:?}");
    };
    assert_eq!(tcx.def_kind(*core), DefKind::Closure);
    assert_ne!(core.krate, LOCAL_CRATE);
    assert_eq!(core.krate, tcx.lang_items().sized_trait().unwrap().krate);
    assert_eq!(tcx.opt_parent(*core), Some(factory.def_id()));
    assert!(tcx.is_mir_available(*core));
    assert!(!authenticate(tcx, *core).unwrap());
    assert!(!authenticate_reviewed_safe_external_helper_v1(tcx, *core).unwrap());
}

fn check_expanded_spans(tcx: TyCtxt<'_>) {
    let root = Instance::mono(tcx, local_function(tcx, "expanded_callback"));
    let closure = map_err_callback(tcx, root);
    assert_eq!(tcx.opt_parent(closure.def_id()), Some(root.def_id()));
    for instance in [root, closure] {
        let header = tcx.def_span(instance.def_id());
        let body = bounded_body(tcx, instance).span;
        for span in [header, body] {
            assert!(
                span.from_expansion(),
                "must exercise a compiler-produced expansion"
            );
            assert!(!span.is_dummy() && span.lo() < span.hi());
            assert!(
                !nested_spans(span, span),
                "even self-containment cannot admit expansion"
            );
        }
        assert_eq!(header.ctxt(), body.ctxt());
        assert!(!nested_spans(header, body));
        assert!(body_span(tcx, instance.def_id()).is_err());
    }
}

struct Probe {
    case: Case,
    completed: bool,
}

impl Callbacks for Probe {
    fn config(&mut self, config: &mut Config) {
        config.input = Input::Str {
            name: FileName::Custom("reviewed_external_closure_fixture.rs".into()),
            input: SOURCE.into(),
        };
    }

    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        match self.case {
            Case::MatrixCallback => check_matrix_callback(tcx),
            Case::WrongCrates => check_wrong_crates(tcx),
            Case::ExpandedSpans => check_expanded_spans(tcx),
        }
        self.completed = true;
        Compilation::Stop
    }
}

fn configured(name: &str, directory: bool) -> PathBuf {
    let path = PathBuf::from(std::env::var_os(name).unwrap_or_else(|| {
        panic!("set {name} to complete cached metadata; this test never builds dependencies")
    }));
    assert!(
        if directory {
            path.is_dir()
        } else {
            path.is_file()
        },
        "{name}: {}",
        path.display()
    );
    path.canonicalize().unwrap()
}

fn run(case: Case, name: &str) {
    let device = configured("FE2O3_CORE_TRY_DEVICE_RMETA", false);
    let host_deps = configured("FE2O3_CORE_TRY_HOST_DEPS", true);
    let core = configured("FE2O3_CORE_TRY_AMDGPU_CORE", false);
    let builtins = configured("FE2O3_CORE_TRY_AMDGPU_BUILTINS", false);
    let observation = derive_cargo_metadata_build_observation_v2(&[METADATA]).to_hex();
    let child_key = format!("{name}:{observation}");
    if std::env::var(CHILD_ENV).ok().as_deref() != Some(child_key.as_str()) {
        let scratch = TestTempDir::create("fe2o3-reviewed-closure-compiler");
        let test = format!("trusted_device_items::reviewed_closure_v1::compiler_tests::{name}");
        let output = crate::process_execution::capture_output(
            Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    &test,
                    "--ignored",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
                .env(CHILD_ENV, child_key)
                .env(OUTPUT_ENV, scratch.path())
                .env(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, &observation)
                .env("FE2O3_CORE_TRY_DEVICE_RMETA", &device)
                .env("FE2O3_CORE_TRY_HOST_DEPS", &host_deps)
                .env("FE2O3_CORE_TRY_AMDGPU_CORE", &core)
                .env("FE2O3_CORE_TRY_AMDGPU_BUILTINS", &builtins),
        )
        .unwrap();
        assert!(
            output.status.success(),
            "reviewed-closure child failed:\n{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed"));
        return;
    }
    assert_eq!(
        std::env::var(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2).unwrap(),
        observation
    );
    assert_eq!(
        std::env::current_dir().unwrap(),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap()
    );
    let scratch = configured(OUTPUT_ENV, true);
    let sysroot = crate::process_execution::capture_output(
        Command::new("rustc").args(["--print", "sysroot"]),
    )
    .unwrap();
    assert!(sysroot.status.success());
    let args = vec![
        "rustc".into(),
        "--crate-name=fe2o3_reviewed_external_closure_fixture".into(),
        "--crate-type=lib".into(),
        "--edition=2024".into(),
        format!("--out-dir={}", scratch.display()),
        "--sysroot".into(),
        String::from_utf8(sysroot.stdout).unwrap().trim().into(),
        "--target=amdgcn-amd-amdhsa".into(),
        "-Ctarget-cpu=gfx942".into(),
        "-Ctarget-feature=-wavefrontsize32,+wavefrontsize64,-xnack".into(),
        "-Zno-codegen".into(),
        "-Zalways-encode-mir".into(),
        "-Zinline-mir=no".into(),
        "-Zmir-enable-passes=-JumpThreading".into(),
        "-Zunstable-options".into(),
        "-Copt-level=0".into(),
        "-Cpanic=abort".into(),
        "-Cdebuginfo=2".into(),
        format!("-Cmetadata={METADATA}"),
        "--extern".into(),
        format!("fe2o3_device={}", device.display()),
        "-L".into(),
        format!("dependency={}", device.parent().unwrap().display()),
        "-L".into(),
        format!("dependency={}", host_deps.display()),
        "--extern".into(),
        format!("noprelude,nounused:core={}", core.display()),
        "--extern".into(),
        format!(
            "noprelude,nounused:compiler_builtins={}",
            builtins.display()
        ),
        "-".into(),
    ];
    let mut probe = Probe {
        case,
        completed: false,
    };
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(probe.completed, "real compiler callback must finish");
}

#[test]
#[ignore = "requires complete FE2O3_CORE_TRY_{DEVICE_RMETA,HOST_DEPS,AMDGPU_CORE,AMDGPU_BUILTINS}"]
fn actual_matrix_checked_callback_keeps_source_mir_amdgpu() {
    run(
        Case::MatrixCallback,
        "actual_matrix_checked_callback_keeps_source_mir_amdgpu",
    );
}

#[test]
#[ignore = "requires complete FE2O3_CORE_TRY_{DEVICE_RMETA,HOST_DEPS,AMDGPU_CORE,AMDGPU_BUILTINS}"]
fn actual_local_and_core_closures_are_not_reviewed_device_helpers_amdgpu() {
    run(
        Case::WrongCrates,
        "actual_local_and_core_closures_are_not_reviewed_device_helpers_amdgpu",
    );
}

#[test]
#[ignore = "requires complete FE2O3_CORE_TRY_{DEVICE_RMETA,HOST_DEPS,AMDGPU_CORE,AMDGPU_BUILTINS}"]
fn actual_expanded_closure_spans_are_rejected_amdgpu() {
    run(
        Case::ExpandedSpans,
        "actual_expanded_closure_spans_are_rejected_amdgpu",
    );
}
