//! Fresh actual rustc calls through the release recipe adapter, with a test-only
//! synchronous actual-L oracle. The normal public driver has no owner callback.
use super::*;
use crate::{
    SourceLocalOrderConstraintOutcomeV1 as Outcome,
    SourceLocalOrderIdentityObservationV1 as Identity, SourceLocalOrderOrderV1 as Order,
    SourceLocalOrderRecipeEvidenceV1 as Evidence, SourceLocalOrderRecipeFailurePhaseV1 as Phase,
    SourceLocalOrderRecipeFailureV1 as Failure, SourceLocalOrderRecipeRequestV1 as Request,
    SourceLocalOrderRelationV1 as Relation, SourceLocalOrderSourceBindingModeV1 as BindingMode,
    SourceLocalOrderStrengthV1 as Strength,
};
use fe2o3_kernel_ir::{
    ScalarType, Type, VerifiedCanonicalKernelIrModuleV12 as Owner, WorkgroupSize,
};
use std::io::{Seek, SeekFrom};
use std::sync::{Arc, Mutex};

#[path = "source_local_order_release_recipe_fixture_v1_tests.rs"]
mod cases_fixture;
#[path = "source_local_order_release_recipe_io_v1_tests.rs"]
mod io;
#[path = "source_local_order_release_recipe_ladder_v1_tests.rs"]
mod ladder;
// Reuse the exact existing independent whole-kernel oracle, not an output/JSON
// reimport. Its one cumulative ledger bounds every observation in this child.
#[path = "../production_pipeline/source_local_order_simulation_v1_tests.rs"]
mod simulation;

const OUTPUT: &str = "FE2O3_TEST_SOURCE_LOCAL_ORDER_RELEASE_RECIPE_OUTPUT";
const INPUT: &str = "FE2O3_TEST_SOURCE_LOCAL_ORDER_RELEASE_RECIPE_INPUT";
const STEP: &str = "FE2O3_TEST_SOURCE_LOCAL_ORDER_RELEASE_RECIPE_STEP";
const CHILD: &str = "production_rustc_driver_v1::source_bitselect_feasibility_v1_tests::roundtrip::local_order::release_recipe::actual_source_local_order_release_recipe_child";
const PREFIX: &str = "FE2O3_SOURCE_LOCAL_ORDER_RELEASE_RECIPE ";

fn order(value: Order) -> &'static str {
    match value {
        Order::SourceOrder => "source_order",
        Order::ReverseReady => "reverse_ready",
    }
}
fn relation(value: Relation) -> &'static str {
    match value {
        Relation::XorBeforeOr => "xor_before_or",
        Relation::OrBeforeXor => "or_before_xor",
    }
}
fn strength(value: Strength) -> &'static str {
    match value {
        Strength::Exact => "exact",
        Strength::Advisory => "advisory",
    }
}
fn binding(value: BindingMode) -> &'static str {
    match value {
        BindingMode::ExactRevision => "exact_revision",
        BindingMode::RebindCurrent => "rebind_current",
    }
}
fn identity(value: &Identity) -> Value {
    json!({"sha256":value.digest(),"canonical_length":value.canonical_length()})
}
fn evidence(value: &Evidence) -> Value {
    let outcome = match value.constraint_outcome() {
        Outcome::Honored { relation: actual } => {
            json!({"status":"honored","relation":relation(actual)})
        }
        Outcome::NotHonored { requested, actual } => {
            json!({"status":"not_honored","requested":relation(requested),"actual":relation(actual)})
        }
    };
    json!({
        "source_sha256":value.source_sha256(),"semantic_sha256":value.semantic_sha256(),
        "source_initializer":value.source_initializer(),
        "instance_axes":value.instance_axes(),"original":identity(value.original()),
        "input":identity(value.input()),"output":identity(value.output()),
        "requested_order":order(value.requested_order()),"requested_relation":relation(value.requested_relation()),
        "strength":strength(value.strength()),"source_binding_mode":binding(value.source_binding_mode()),
        "actual_relation":relation(value.actual_relation()),"constraint_outcome":outcome,
        "region":value.region(),"output_result_order":value.output_result_order(),
        "prefix_execution_bytes":value.prefix_execution_bytes().as_slice(),
        "transition_sha256":value.transition_sha256(),"transition_bytes":value.transition_bytes(),
        "transition_rows":value.transition_rows(),"fresh_formal_counts":value.fresh_formal_counts(),
        "llvm_sha256":value.llvm_sha256(),"descriptor_sha256":value.descriptor_sha256(),
        "descriptor_producer":value.descriptor_producer(),"recipe_sha256":value.recipe_sha256(),
        "canonical_work":value.canonical_work(),"canonical_peak_storage":value.canonical_peak_storage(),
        "created":value.created(),"composition":value.composition(),"grants_authority":value.grants_authority(),
    })
}
fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn check_actual_order(owner: &Owner, evidence: &Evidence) -> Result<(), String> {
    use fe2o3_kernel_ir::{BinaryOp, OperationKind};
    let [function, block, first, count] = *evidence.region();
    if count != 3 {
        return Err("release oracle exact region count".into());
    }
    let operations = owner
        .module()
        .functions
        .get(function as usize)
        .and_then(|function| function.body.as_ref())
        .and_then(|body| body.blocks.get(block as usize))
        .and_then(|block| block.operations.get(first as usize..))
        .and_then(|operations| operations.get(..3))
        .ok_or("release oracle actual region unavailable")?;
    let mut kinds = [BinaryOp::BitAnd; 3];
    let mut results = [0u32; 3];
    for (index, operation) in operations.iter().enumerate() {
        let [result] = operation.results.as_slice() else {
            return Err("release oracle result arity".into());
        };
        if result.ty != Type::Scalar(ScalarType::U32) {
            return Err("release oracle result type".into());
        }
        let OperationKind::Binary { op, .. } = &operation.kind else {
            return Err("release oracle binary region".into());
        };
        kinds[index] = *op;
        results[index] = result.id.0;
    }
    let actual = match kinds {
        [BinaryOp::BitXor, BinaryOp::BitOr, BinaryOp::BitAnd] => Relation::XorBeforeOr,
        [BinaryOp::BitOr, BinaryOp::BitXor, BinaryOp::BitAnd] => Relation::OrBeforeXor,
        _ => return Err("release oracle actual operation order".into()),
    };
    if &results != evidence.output_result_order() || actual != evidence.actual_relation() {
        return Err("release oracle actual L disagrees with evidence".into());
    }
    Ok(())
}

fn assert_failure(step: cases_fixture::Step, error: &Failure) {
    use cases_fixture::Expected;
    let (phase, diagnostic) = match step.expected {
        Expected::ExactSourceRevision => (
            Phase::RecipeBinding,
            "local-order recipe source revision changed",
        ),
        Expected::ItemBinding => (
            Phase::RecipeBinding,
            "local-order recipe item/instance binding changed",
        ),
        Expected::SourceCase => (
            Phase::Eligibility,
            fixture_cases::refusal(step.source).unwrap(),
        ),
        Expected::CurrentSourceRevision => (
            Phase::SourceCurrentness,
            "local-order recipe expected current source revision differs",
        ),
        Expected::ExactConstraint => (
            Phase::Constraint,
            "local-order recipe exact canonical constraint not honored",
        ),
        Expected::PostObserverSource => (
            Phase::SourceCurrentness,
            "source changed before candidate publication",
        ),
        Expected::Reentry => (
            Phase::Frontend,
            "local-order recipe received a repeated live callback",
        ),
        Expected::Fatal => (
            Phase::Frontend,
            "rustc reported a fatal error during the recipe attempt",
        ),
        Expected::Success => panic!("successful source case returned refusal"),
    };
    assert_eq!(
        error.phase(),
        phase,
        "wrong designated release failure phase"
    );
    assert_eq!(
        error.diagnostic(),
        diagnostic,
        "another compiler error is not this negative"
    );
    assert_eq!(
        error.compiler_fatal(),
        matches!(step.expected, Expected::Fatal)
    );
}

#[test]
#[ignore = "actual rustc child; use actual_source_local_order_release_recipe_ladder"]
fn actual_source_local_order_release_recipe_child() {
    let directory = PathBuf::from(std::env::var_os(INPUT).expect("recipe qualification input"));
    let step = cases_fixture::step(&std::env::var(STEP).expect("recipe qualification step"));
    require_current_source();
    assert_eq!(std::env::current_dir().unwrap(), repository());
    let actual = cases_fixture::derive(&directory, step);
    let saved: cases_fixture::Invocation = serde_json::from_slice(
        &read_bounded(
            &directory.join(format!("{}.invocation.json", step.id)),
            64 * 1024,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(actual, saved, "fresh recipe invocation pins changed");
    assert_eq!(
        std::env::var(CRATE_BINDING_ID_ENV_V1).unwrap(),
        actual.crate_binding
    );
    assert_eq!(
        std::env::var(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2).unwrap(),
        actual.cargo_observation
    );
    assert_eq!(
        std::env::var("CARGO_MANIFEST_DIR").unwrap(),
        fixture().to_str().unwrap()
    );
    assert_eq!(std::env::var("CARGO_PKG_NAME").unwrap(), PACKAGE);
    assert_eq!(std::env::var("CARGO_PKG_VERSION").unwrap(), "0.1.0");
    assert_eq!(std::env::var("CARGO_CRATE_NAME").unwrap(), CRATE_NAME);
    crate::production_rustc_driver_v1::require_canonical_overflow_checks_v1(&actual.args).unwrap();

    let source_bytes = read_bounded(&actual.source_relative, 64 * 1024).unwrap();
    let expected_source = digest(&source_bytes);
    assert_eq!(hex(&expected_source), actual.source_sha256);
    let mut retained = actual
        .recipe_name
        .as_deref()
        .map(|name| io::RetainedRecipe::open(&io::recipe_path(&directory, name)).unwrap());
    if let Some(file) = &mut retained {
        file.recheck().unwrap();
        assert_eq!(Some(hex(&digest(file.bytes()))), actual.recipe_sha256);
    }
    let request = match step.mode {
        cases_fixture::Mode::Create {
            order,
            relation,
            strength,
            binding,
            ..
        } => Request::create(
            actual.source_relative.to_str().unwrap(),
            expected_source,
            order,
            relation,
            strength,
            binding,
        )
        .unwrap(),
        cases_fixture::Mode::Replay { .. } => Request::replay(
            actual.source_relative.to_str().unwrap(),
            expected_source,
            retained.as_ref().unwrap().bytes(),
        )
        .unwrap(),
    };
    if step.source == "stale-source" {
        // Genuine private source bytes change after the caller chooses a current
        // revision. This is expected-current-revision refusal, NOT a claim about
        // a race after the public driver opened its own retained descriptor.
        fs::OpenOptions::new()
            .append(true)
            .open(&actual.source_relative)
            .unwrap()
            .write_all(b"\n// actual source changed after expected-current revision\n")
            .unwrap();
    }

    let fault = cases_fixture::is_fault(step);
    let source_to_change =
        (step.id == "probe-source-change").then(|| repository().join(&actual.source_relative));
    let observed: Arc<Mutex<Option<(Evidence, Value)>>> = Arc::new(Mutex::new(None));
    let observer_slot = Arc::clone(&observed);
    let mut oracle = simulation::OracleLedger::default();
    let observer = Box::new(
        move |owner: &Owner, current: &Evidence| -> Result<(), String> {
            if owner.canonical().identity().digest() != current.output().digest()
                || owner.canonical().identity().canonical_length()
                    != current.output().canonical_length()
            {
                return Err("release recipe oracle actual-L identity mismatch".into());
            }
            check_actual_order(owner, current)?;
            let simulation = if fault {
                if let Some(path) = source_to_change {
                    // One same-inode/length byte change in this isolated task source.
                    // It occurs after successful current-owner validation, before
                    // the release adapter's final retained-source recheck.
                    let mut file = fs::OpenOptions::new()
                        .write(true)
                        .open(path)
                        .map_err(|e| e.to_string())?;
                    let length = file.metadata().map_err(|e| e.to_string())?.len();
                    file.seek(SeekFrom::Start(
                        length.checked_sub(1).ok_or("empty fault source")?,
                    ))
                    .map_err(|e| e.to_string())?;
                    file.write_all(b" ").map_err(|e| e.to_string())?;
                }
                json!({"not_run":"fault probe has no CPU oracle credit"})
            } else {
                simulation::observe(owner, &mut oracle)?
            };
            let mut slot = observer_slot
                .lock()
                .map_err(|_| "release recipe oracle result poisoned")?;
            if slot.is_some() {
                return Err("release recipe repeated actual-L observer".into());
            }
            *slot = Some((
                *current,
                json!({"simulation":simulation,"oracle_accounting":oracle}),
            ));
            Ok(())
        },
    );
    let attempt = crate::production_rustc_driver_v1::source_local_order_recipe_driver_v1::
        run_source_local_order_recipe_probe_v1(&actual.args, request, Some(observer),
            step.id == "probe-reentry", step.id == "probe-fatal");
    assert_eq!(
        attempt.compiler_callback_count(),
        1,
        "one genuine rustc callback entry"
    );
    assert_eq!(
        attempt.callback_count(),
        if step.id == "probe-reentry" { 2 } else { 1 },
        "deliberate method reentry is counted separately"
    );
    assert_eq!(
        attempt.request().source_path(),
        actual.source_relative.to_str().unwrap()
    );
    assert_eq!(
        attempt.request().expected_current_source_sha256(),
        &expected_source
    );
    if let Some(file) = &mut retained {
        file.recheck().unwrap();
        assert_eq!(attempt.request().replay_recipe_bytes(), Some(file.bytes()));
    }
    let observed = observed.lock().unwrap().take();
    let observation = match attempt.result() {
        Err(error) => {
            assert_failure(step, error);
            if fault {
                let (_, probe) = observed.expect("fault follows actual current-L observer");
                assert_eq!(
                    probe["simulation"]["not_run"],
                    "fault probe has no CPU oracle credit"
                );
                assert_eq!(probe["oracle_accounting"]["runs"], 0);
            } else {
                assert!(
                    observed.is_none(),
                    "designated negative has no admitted actual-L observation"
                );
            }
            json!({"status":"refused","phase":format!("{:?}",error.phase()),
                "diagnostic":error.diagnostic(),"compiler_fatal":error.compiler_fatal(),
                "simulation_runs":0})
        }
        Ok(output) => {
            assert!(matches!(step.expected, cases_fixture::Expected::Success));
            let current = output.evidence();
            let (observed_evidence, simulation) =
                observed.expect("same-adapter actual-L observation");
            assert_eq!(
                *current, observed_evidence,
                "returned evidence differs from actual L observed"
            );
            assert_eq!(current.source_sha256(), &expected_source);
            assert_eq!(current.llvm_sha256(), &digest(output.llvm_ir().as_bytes()));
            assert!(!output.llvm_ir().is_empty() && output.llvm_ir().len() <= 48 * 1024);
            assert_eq!(
                current.descriptor_producer(),
                "source-local-order-policy6-v1/gfx942"
            );
            assert_eq!(current.composition(), "source-local-order-policy6-v1");
            assert!(!current.grants_authority());
            assert!(!output.grants_artifact_or_launch_authority());
            assert_eq!(simulation["simulation"]["runs"], 30);
            assert_eq!(simulation["simulation"]["scenarios"], 15);
            assert_eq!(
                simulation["simulation"]["output_and_canaries_checked"],
                true
            );
            assert_eq!(current.region()[3], 3);
            assert!(current.transition_bytes() > 0 && current.canonical_work() > 0);
            match step.mode {
                cases_fixture::Mode::Create {
                    order,
                    relation,
                    strength,
                    binding,
                    saved,
                } => {
                    assert!(current.created());
                    assert_eq!(current.requested_order(), order);
                    assert_eq!(current.requested_relation(), relation);
                    assert_eq!(current.strength(), strength);
                    assert_eq!(current.source_binding_mode(), binding);
                    let bytes = output
                        .created_recipe_bytes()
                        .expect("explicit Create bytes");
                    assert_eq!(current.recipe_sha256(), &digest(bytes));
                    io::persist(
                        &directory,
                        saved.expect("successful Create recipe name"),
                        bytes,
                    );
                }
                cases_fixture::Mode::Replay { .. } => {
                    assert!(!current.created());
                    assert!(
                        output.created_recipe_bytes().is_none(),
                        "Replay never regenerates"
                    );
                    assert_eq!(
                        current.recipe_sha256(),
                        &digest(retained.as_ref().unwrap().bytes())
                    );
                }
            }
            if step.id == "advisory-mismatch" {
                assert_eq!(
                    current.constraint_outcome(),
                    Outcome::NotHonored {
                        requested: Relation::OrBeforeXor,
                        actual: Relation::XorBeforeOr
                    }
                );
                assert_eq!(current.requested_order(), Order::SourceOrder);
            } else {
                assert_eq!(
                    current.constraint_outcome(),
                    Outcome::Honored {
                        relation: current.actual_relation()
                    }
                );
            }
            json!({"status":"accepted","evidence":evidence(current),"llvm_text":output.llvm_ir(),
                "oracle":simulation,"simulation_runs":30})
        }
    };
    let final_source_hash = hash(&actual.source_relative);
    if matches!(step.source, "stale-source" | "probe-source-change") {
        assert_ne!(final_source_hash, actual.source_sha256);
    } else {
        assert_eq!(final_source_hash, actual.source_sha256);
    }
    require_current_source();
    assert!(
        fs::read_dir(directory.join("analysis-output"))
            .unwrap()
            .next()
            .is_none()
    );
    let report = serde_json::to_vec(&json!({
        "invocation":actual,"observation":observation,"callback_count":attempt.callback_count(),
        "compiler_callback_count":attempt.compiler_callback_count(),"fault_probe":fault,
        "final_source_sha256":final_source_hash,
        "recipe_io":retained.as_ref().map(|file| &file.accounting),
        "same_release_adapter":true,"test_only_actual_l_observer":true,
        "normal_example_executed":false,"serialized_owner_read":false,
        "native_object_emitted":false,"hardware_observed":false,
        "grants_artifact_or_launch_authority":false,
    }))
    .unwrap();
    assert!(report.len() <= 128 * 1024);
    println!("\n{PREFIX}{}", std::str::from_utf8(&report).unwrap());
}
