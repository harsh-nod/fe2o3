//! Actual safe wrapper traversal and inert capture, not collective execution.

use super::*;
use crate::rustc_semantic_adapter_v1::canonical_function_identities_v1;
use crate::trusted_device_items::{self, TrustedDeviceItem, Wave64ShuffleScalarV1};
use fe2o3_mir_model::semantic_mir_v1::{
    AdmittedInertSemanticMirV1, SemanticCallableDeclV1, SemanticCompilerIntrinsicOperationV1,
    SemanticMirLimitsV1, SemanticMirWireVersionV1, SemanticTerminatorKindV1,
};
use rustc_middle::ty::{self, Instance, TypingEnv};

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::wave64_capture_source::wave64_capture_source_child";
const CASE_ENV: &str = "FE2O3_TEST_WAVE64_CAPTURE_CASE_V1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
enum Scalar {
    U32,
    I32,
    F32,
}
impl Scalar {
    fn provider(self) -> Wave64ShuffleScalarV1 {
        match self {
            Self::U32 => Wave64ShuffleScalarV1::U32,
            Self::I32 => Wave64ShuffleScalarV1::I32,
            Self::F32 => Wave64ShuffleScalarV1::F32,
        }
    }
    fn feature(self) -> &'static str {
        match self {
            Self::U32 => "wave64-capture-u32",
            Self::I32 => "wave64-capture-i32",
            Self::F32 => "wave64-capture-f32",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
enum Expectation {
    InertCapture,
    UserUnsafe,
    UnmarkedUnsafeScan,
    WrongTarget,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct Case {
    scalar: Scalar,
    retained: bool,
    expectation: Expectation,
}
impl Case {
    fn target(self) -> &'static str {
        if self.expectation == Expectation::WrongTarget {
            "gfx950"
        } else {
            "gfx942"
        }
    }
    fn feature(self) -> &'static str {
        match self.expectation {
            Expectation::UserUnsafe => "wave64-capture-direct-unsafe",
            Expectation::UnmarkedUnsafeScan => "wave64-capture-inclusive",
            _ => self.scalar.feature(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Report {
    case: Case,
    primitive_calls: usize,
    retained_wrapper_functions: usize,
    actual_can_unwind: Option<bool>,
    semantic_bytes: usize,
    refusal: String,
}
impl Report {
    fn validate(&self, case: Case) -> Result<(), String> {
        if self.case != case || self.refusal.is_empty() {
            return Err("wave capture report lost its exact case or later refusal".into());
        }
        if case.expectation == Expectation::InertCapture {
            if self.primitive_calls == 0
                || self.actual_can_unwind.is_none()
                || self.semantic_bytes == 0
                || (case.retained && self.retained_wrapper_functions != 1)
            {
                return Err("source report lacks actual inert primitive/body/ABI evidence".into());
            }
        } else if self.primitive_calls != 0
            || self.retained_wrapper_functions != 0
            || self.actual_can_unwind.is_some()
            || self.semantic_bytes != 0
        {
            return Err("source safety refusal was substituted with descriptor evidence".into());
        }
        Ok(())
    }
}

fn observe_imported(
    tcx: TyCtxt<'_>,
    case: Case,
    semantic: &AdmittedInertSemanticMirV1,
) -> Result<Report, String> {
    if semantic.wire_version() != SemanticMirWireVersionV1::V33 || semantic.roots().len() != 1 {
        return Err("actual ordinary wrapper did not import exact V33 with one root".into());
    }
    let scalar = case.scalar.provider();
    let primitive =
        trusted_device_items::definition(tcx, TrustedDeviceItem::Gfx942Wave64Shuffle(scalar))
            .ok_or("exact primitive DefId is unavailable")?;
    trusted_device_items::check_actual_sealed_trait_chain_paths_v1(tcx, primitive, scalar);
    let instance = Instance::mono(tcx, primitive);
    let identity = canonical_function_identities_v1(tcx, instance).function();
    let actual_fn_abi = tcx
        .fn_abi_of_instance(
            TypingEnv::fully_monomorphized().as_query_input((instance, ty::List::empty())),
        )
        .map_err(|error| format!("actual FnAbi: {error:?}"))?;
    let mut primitive_ids = Vec::new();
    for (index, callable) in semantic.callables().iter().enumerate() {
        if let SemanticCallableDeclV1::CompilerIntrinsic {
            binding,
            operation: SemanticCompilerIntrinsicOperationV1::Gfx942Wave64ShuffleIndex { .. },
            ..
        } = callable
        {
            if binding.identity() != identity
                || binding.abi().can_unwind() != actual_fn_abi.can_unwind
            {
                return Err(
                    "primitive descriptor substituted the actual instance or unwind ABI".into(),
                );
            }
            crate::collector::check_wave64_descriptor_mutations_v1(
                tcx,
                instance,
                scalar,
                binding.abi(),
                semantic.types(),
            );
            primitive_ids.push(index);
        }
    }
    if primitive_ids.len() != 1 {
        return Err(format!(
            "expected one exact primitive declaration, got {}",
            primitive_ids.len()
        ));
    }
    let primitive_calls = semantic.functions().iter().flat_map(|function| function.blocks())
        .filter(|block| matches!(block.terminator().kind(), SemanticTerminatorKindV1::Call(call)
            if primitive_ids.contains(&(call.callee().index() as usize)) && call.arguments().len() == 3))
        .count();
    let wrapper = trusted_device_items::definition(tcx, TrustedDeviceItem::Gfx942Wave64ReduceSum)
        .ok_or("reviewed safe reduce_sum wrapper unavailable")?;
    let wrapper_identity = canonical_function_identities_v1(
        tcx,
        Instance::new_raw(wrapper, ty::GenericArgs::identity_for_item(tcx, wrapper)),
    )
    .item_definition();
    let retained_wrapper_functions = semantic
        .functions()
        .iter()
        .filter(|function| function.item_definition_identity() == wrapper_identity)
        .count();
    let decoded = AdmittedInertSemanticMirV1::decode_exact_v33_canonical(
        semantic.canonical_encoding(),
        SemanticMirLimitsV1::default(),
    )
    .map_err(|error| format!("actual V33 decode: {error:?}"))?;
    if decoded.canonical_encoding() != semantic.canonical_encoding()
        || decoded.functions() != semantic.functions()
        || decoded.callables() != semantic.callables()
    {
        return Err("actual source V33 roundtrip changed body or primitive binding".into());
    }
    Ok(Report {
        case,
        primitive_calls,
        retained_wrapper_functions,
        actual_can_unwind: Some(actual_fn_abi.can_unwind),
        semantic_bytes: semantic.canonical_encoding().len(),
        refusal: String::new(),
    })
}

fn observe_capture(tcx: TyCtxt<'_>, case: Case) -> Result<Report, String> {
    use crate::collector::semantic_import_observation_v1_tests::with_observer;
    use crate::production_pipeline::ProductionPipelineError as Pipeline;
    use std::cell::RefCell;
    use std::rc::Rc;
    let transaction = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    )?;
    let observed = Rc::new(RefCell::new((0_usize, None)));
    let callback_state = Rc::clone(&observed);
    let result = with_observer(
        Box::new(move |tcx, semantic| {
            let mut state = callback_state.borrow_mut();
            state.0 += 1;
            state.1 = Some(observe_imported(tcx, case, semantic));
        }),
        || transaction.verify_general_kernel_checks(),
    );
    let (count, report) = Rc::try_unwrap(observed)
        .expect("observer was removed")
        .into_inner();
    if count != 1 {
        return Err(format!(
            "expected one actual admitted-owner observation, got {count}; result: {:?}",
            result.err()
        ));
    }
    let mut report = report.ok_or("missing actual source observation")??;
    report.refusal = match result {
        Err(error @ (Pipeline::PreRankedMaterialization(_) | Pipeline::RankedProjection(_))) => {
            format!("{error:?}")
        }
        Err(error) => return Err(format!("unexpected pre-materialization refusal: {error:?}")),
        Ok(_) => {
            return Err("inert primitive capture acquired unchecked collective execution".into());
        }
    };
    report.validate(case)?;
    Ok(report)
}

fn observe_refusal(tcx: TyCtxt<'_>, case: Case) -> Result<Report, String> {
    let refusal = match transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    ) {
        Err(error) => error,
        Ok(_) => {
            return Err("ordinary source acquired an unauthorized unsafe/target closure".into());
        }
    };
    let expected = match case.expectation {
        Expectation::UserUnsafe => refusal.contains("user-provided unsafe block"),
        Expectation::UnmarkedUnsafeScan => {
            refusal.contains("reaches unsafe function instance")
                && refusal.contains("wave64_inclusive_scan")
        }
        Expectation::WrongTarget => {
            refusal.contains("reaches unsafe function instance")
                && refusal.contains("__fe2o3_wave64_shuffle_index")
        }
        Expectation::InertCapture => false,
    };
    if !expected {
        return Err(format!(
            "wrong source boundary for {:?}: {refusal}",
            case.expectation
        ));
    }
    let report = Report {
        case,
        primitive_calls: 0,
        retained_wrapper_functions: 0,
        actual_can_unwind: None,
        semantic_bytes: 0,
        refusal,
    };
    report.validate(case)?;
    Ok(report)
}

struct CaptureCallbacks {
    case: Case,
    result: Option<Result<Report, String>>,
}
impl Callbacks for CaptureCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some(if self.case.expectation == Expectation::InertCapture {
            observe_capture(tcx, self.case)
        } else {
            observe_refusal(tcx, self.case)
        });
        Compilation::Stop
    }
}

#[test]
#[ignore = "subprocess helper; parent supplies exact Cargo capture and source boundary"]
fn wave64_capture_source_child() {
    let Some(path) = env::var_os(CHILD_ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let case: Case = serde_json::from_str(&env::var(CASE_ENV).unwrap()).unwrap();
    let mut callbacks = CaptureCallbacks { case, result: None };
    let completed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        rustc_driver::run_compiler(&args, &mut callbacks);
    }));
    let result = if completed.is_err() {
        Err("rustc or wave capture callback panicked".to_owned())
    } else {
        callbacks
            .result
            .unwrap_or_else(|| Err("source callback did not execute".into()))
    };
    std::fs::write(
        env::var_os(CHILD_RESULT).unwrap(),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "actual wave source capture: {result:?}");
}

fn fixture(workspace: &Path, case: Case) -> corpus::Fixture {
    let base = "crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device";
    let manifest = format!("{base}/Cargo.toml");
    let hash = |path: &str| {
        Sha256::digest(std::fs::read(workspace.join(path)).unwrap())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    corpus::Fixture {
        fixture_id: format!(
            "{}-{}-retained-{}",
            case.target(),
            case.feature(),
            case.retained
        ),
        target: case.target().into(),
        compiler_input: corpus::CompilerInput {
            package_manifest_sha256: hash(&manifest),
            package_manifest: manifest,
            cargo_lock_path: "Cargo.lock".into(),
            cargo_lock_sha256: hash("Cargo.lock"),
            source_paths: vec![
                format!("{base}/src/lib.rs"),
                format!("{base}/src/wave64_capture.rs"),
            ],
            source_closure_sha256: String::new(),
            cargo_target: corpus::CargoTarget {
                kind: "lib".into(),
                name: "fe2o3_production_extraction_fixture".into(),
                source_path: "src/lib.rs".into(),
            },
            default_features: false,
            features: vec![case.feature().into()],
            kernel_symbols: vec!["wave64_capture".into()],
        },
    }
}

#[test]
#[ignore = "genuine Cargo/AMD rustc source capture; requires pinned nightly toolchain"]
fn safe_wave64_wrappers_capture_exact_primitives_without_execution_authority() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-wave64-capture");
    let mut cases = Vec::new();
    for scalar in [Scalar::U32, Scalar::I32, Scalar::F32] {
        for retained in [false, true] {
            cases.push(Case {
                scalar,
                retained,
                expectation: Expectation::InertCapture,
            });
        }
    }
    for expectation in [Expectation::UserUnsafe, Expectation::WrongTarget] {
        for retained in [false, true] {
            cases.push(Case {
                scalar: Scalar::U32,
                retained,
                expectation,
            });
        }
    }
    cases.push(Case {
        scalar: Scalar::U32,
        retained: true,
        expectation: Expectation::UnmarkedUnsafeScan,
    });
    for case in cases {
        let fixture = fixture(&workspace, case);
        let directory = scratch.path().join(&fixture.fixture_id);
        std::fs::create_dir(&directory).unwrap();
        let target_dir = scratch.path().join(case.target());
        let mut captured =
            corpus_cargo::capture(&workspace, &fixture, &directory, &target_dir).unwrap();
        if case.retained {
            captured.args.push("-Zinline-mir=no".into());
            captured.args.push("-Zmir-opt-level=0".into());
        }
        let request = directory.join("args.json");
        let response = directory.join("result.json");
        std::fs::write(&request, serde_json::to_vec(&captured.args).unwrap()).unwrap();
        let mut command = Command::new(env::current_exe().unwrap());
        command
            .env_clear()
            .envs(captured.environment)
            .current_dir(captured.cwd)
            .env_remove("RUSTC_WRAPPER")
            .env_remove("RUSTC_WORKSPACE_WRAPPER")
            .env_remove(CHILD_PROOF_PROBE)
            .env(CHILD_ARGS, &request)
            .env(CHILD_RESULT, &response)
            .env(CASE_ENV, serde_json::to_string(&case).unwrap())
            .args(["--exact", CHILD, "--ignored", "--nocapture"]);
        progress::clear_inherited_jobserver(&mut command);
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}: {}",
            fixture.fixture_id,
            corpus_cargo::diagnostics(&output)
        );
        let report: Result<Report, String> =
            serde_json::from_slice(&std::fs::read(response).unwrap()).unwrap();
        let report = report.unwrap();
        report.validate(case).unwrap();
        eprintln!(
            "WAVE64 SOURCE {}: {} primitive calls, {} retained wrappers; {}",
            fixture.fixture_id,
            report.primitive_calls,
            report.retained_wrapper_functions,
            report.refusal
        );
    }
}
