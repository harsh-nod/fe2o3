//! Opt-in genuine repeat publication and fresh-candidate CPU observations.
//! These adapters spawn nothing. A separately bounded root-owned parent must
//! retain started/terminal/stream records and enforce the finite campaign.
use super::*;
use crate::production_ordered_composition_source_v1::{
    OrderedCompositionSourcePublishEffectV1 as Effect,
    OrderedCompositionSourcePublishRequestV1 as Request, OrderedCompositionTypedEditV1 as Edit,
    publish_ordered_composition_source_v1,
};
use fe2o3_kernel_ir::Gfx942U32ProgramV1 as Program;
use std::os::unix::fs::MetadataExt;

#[path = "gfx942_ordered_repeat_promotion_cpu_v1_tests.rs"]
mod cpu;

const ROOT_ENV: &str = "FE2O3_TEST_ORDERED_REPEAT_ROOT_V1";
const COUNT_ENV: &str = "FE2O3_TEST_ORDERED_REPEAT_COUNT_V1";
const CASE_ENV: &str = "FE2O3_TEST_ORDERED_REPEAT_CASE_V1";
const SYSROOT_ENV: &str = "FE2O3_TEST_ORDERED_REPEAT_SYSROOT_V1";
const PREFIX: &str = "FE2O3_ORDERED_REPEAT_PROMOTION_V1 ";
const HELPER: &str = "__fe2o3_region_0123456789abcdef";
const FEATURE: &str = "ordered-composition-publish-direct";
const POSITIVES: [&str; 4] = [
    "publish-preserve",
    "recompile-preserve",
    "publish-edit",
    "recompile-edit",
];
const REFUSAL: &str = "publisher supports only exact flat or literal-repeat program spelling";

fn count(text: &str) -> u8 {
    match text {
        "1" => 1,
        "2" => 2,
        "15" => 15,
        _ => panic!("closed repeat count"),
    }
}
fn seed(n: u8) -> &'static [u8] {
    match n {
        1 => include_bytes!("ordered_repeat_inputs_v1/repeat-1.rs"),
        2 => include_bytes!("ordered_repeat_inputs_v1/repeat-2.rs"),
        15 => include_bytes!("ordered_repeat_inputs_v1/repeat-15.rs"),
        _ => panic!("closed repeat source"),
    }
}
fn noncanonical() -> Vec<u8> {
    let source = std::str::from_utf8(seed(2)).unwrap();
    let from = "repeat(2) { add(out, out, input1); }";
    assert_eq!(source.matches(from).count(), 1);
    source
        .replace(
            from,
            "repeat(2) { /* spelling intentionally refused */ add(out, out, input1); }",
        )
        .into_bytes()
}
fn package(n: u8, case: &str) -> &'static str {
    match case {
        "publish-preserve" | "publish-edit" => "package-original",
        "recompile-preserve" => "package-promoted-preserve",
        "recompile-edit" => "package-promoted-edit",
        "refuse-noncanonical" if n == 2 => "package-promoted-copy",
        _ => panic!("closed repeat case"),
    }
}
fn packages(n: u8) -> &'static [&'static str] {
    match n {
        1 | 15 => &[
            "package-original",
            "package-promoted-preserve",
            "package-promoted-edit",
        ],
        2 => &[
            "package-original",
            "package-promoted-preserve",
            "package-promoted-edit",
            "package-promoted-copy",
        ],
        _ => panic!("closed repeat package roster"),
    }
}
fn candidate(case: &str) -> &'static str {
    match case {
        "publish-preserve" | "recompile-preserve" => {
            "package-promoted-preserve/src/ordered_composition_publish_v1.rs"
        }
        "publish-edit" | "recompile-edit" => {
            "package-promoted-edit/src/ordered_composition_publish_v1.rs"
        }
        "refuse-noncanonical" => "refused-noncanonical.rs",
        _ => panic!("closed repeat candidate"),
    }
}
fn fresh(case: &str) -> bool {
    matches!(case, "recompile-preserve" | "recompile-edit")
}
fn edited(case: &str) -> bool {
    matches!(case, "publish-edit" | "recompile-edit")
}
fn original(n: u8, case: &str) -> Vec<u8> {
    if case == "refuse-noncanonical" {
        assert_eq!(n, 2);
        noncanonical()
    } else {
        seed(n).to_vec()
    }
}
/// Independent fixed descriptor oracle: mov(out,input0), then N additions.
/// This does not invoke the device repeat expander or publisher recognizer.
fn expected_program(n: u8, change: bool) -> Program {
    assert!(matches!(n, 1 | 2 | 15));
    let mut words = [0_u16; 16];
    if change {
        words[0] = 40; // mov(out,input2)
        Program::from_descriptors(1, words).unwrap()
    } else {
        words[0] = 8; // mov(out,input0)
        words[1..=usize::from(n)].fill(201); // add(out,out,input1)
        Program::from_descriptors(n + 1, words).unwrap()
    }
}
/// An independently spelled complete expected candidate for the three exact
/// inputs, not a call to production rendering/splicing and not an HIR authority.
fn expected_candidate(n: u8, change: bool) -> Vec<u8> {
    let source = std::str::from_utf8(seed(n)).unwrap();
    let anchor = "c: u32) {";
    assert_eq!(source.matches(anchor).count(), 1);
    let insertion = source.find(anchor).unwrap() + anchor.len();
    let start = source
        .find("fe2o3_device::amdgpu_ordered_program!")
        .unwrap();
    let end = source[start..].find("\n    };").unwrap() + start + "\n    }".len();
    let mut helper = format!(
        "\n    #[inline(never)]\n    fn {HELPER}(a: u32, b: u32, c: u32) -> u32 {{\n        fe2o3_device::amdgpu_ordered_program! {{\n            gfx942_xnack_off_wave64;\n            scratch(8); out(9);\n            in(10) = a; in(11) = b; in(12) = c;\n"
    );
    if change {
        helper.push_str("            mov(out, input2);\n");
    } else {
        helper.push_str("            mov(out, input0);\n");
        for _ in 0..n {
            helper.push_str("            add(out, out, input1);\n");
        }
    }
    helper.push_str("        }\n    }\n");
    let result = format!(
        "{}{}{}{HELPER}(a, b, c){}",
        &source[..insertion],
        helper,
        &source[insertion..start],
        &source[end..]
    );
    assert!(result.len() <= 72 * 1024);
    result.into_bytes()
}
fn root_and_count() -> (PathBuf, u8) {
    let root = PathBuf::from(std::env::var_os(ROOT_ENV).expect("fresh profile root"));
    assert!(root.is_absolute());
    let n = count(&std::env::var(COUNT_ENV).expect("repeat count"));
    (root, n)
}
fn emit(value: &Value) {
    let bytes = serde_json::to_string(value).unwrap();
    assert!(bytes.len() <= 256 * 1024 && !bytes.contains('\r') && !bytes.contains('\n'));
    println!("\n{PREFIX}{bytes}");
}
fn save(root: &Path, name: &str, value: &impl Serialize) {
    let bytes = serde_json::to_vec(value).unwrap();
    create(&root.join(name), &bytes, 256 * 1024);
}
/// No process launches here. The parent must run the exact retained Cargo
/// metadata/dependency commands from the proposal before deriving invocations.
#[test]
#[ignore = "fresh root, pinned sysroot and bounded external parent required"]
fn prepare_ordered_repeat_packages() {
    let (root, n) = root_and_count();
    assert!(!root.exists(), "never reuse a profile root");
    fs::create_dir(&root).unwrap();
    let root = root.canonicalize().unwrap();
    assert!(!root.starts_with(repository().parent().unwrap()));
    let sysroot = PathBuf::from(std::env::var_os(SYSROOT_ENV).expect("pinned sysroot"));
    assert!(sysroot.is_absolute());
    assert_eq!(sysroot.canonicalize().unwrap(), sysroot);
    assert!(
        sysroot
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("nightly-2026-04-03-")
    );
    let workspace = read_bounded(&repository().join("Cargo.lock"), 1024 * 1024).unwrap();
    for &name in packages(n) {
        let negative = (name == "package-promoted-copy").then(noncanonical);
        let source = if name == "package-original" {
            Some(seed(n))
        } else {
            negative.as_deref()
        };
        staging::package_with_source(&root, name, source);
        let prep = preparation(&root, name);
        fs::create_dir(&prep).unwrap();
        fs::create_dir(prep.join("analysis-output")).unwrap();
        create(
            &prep.join("sysroot.stdout"),
            format!("{}\n", sysroot.display()).as_bytes(),
            4096,
        );
        create(&prep.join("workspace-lock.seed"), &workspace, 1024 * 1024);
        create(&root.join(name).join("Cargo.lock"), &workspace, 1024 * 1024);
    }
    let fact = json!({
        "schema":"fe2o3-test-ordered-repeat-package-preparation-v1",
        "repeat_count":n,"packages":packages(n),"profile_source_sha256":digest(seed(n)),
        "workspace_lock_sha256":digest(&workspace),"sysroot":sysroot,
        "frontend_sessions":0,"subprocesses_spawned":0,
        "dependency_builds_required":1,"metadata_commands_required":packages(n).len()*2,
        "normal_checked_handoff_qualified":false,"acceptance":"preparation only",
    });
    save(&root, "repeat-preparation.json", &fact);
    emit(&fact);
}
fn validate_preparation(root: &Path, n: u8) {
    let workspace = read_bounded(&repository().join("Cargo.lock"), 1024 * 1024).unwrap();
    for &name in packages(n) {
        let prep = preparation(root, name);
        assert_eq!(
            read_bounded(&prep.join("workspace-lock.seed"), 1024 * 1024).unwrap(),
            workspace
        );
        let resolved = read_bounded(&prep.join("resolve.stdout"), 16 * 1024 * 1024).unwrap();
        let locked = read_bounded(&prep.join("metadata.stdout"), 16 * 1024 * 1024).unwrap();
        assert_eq!(resolved, locked, "locked actual metadata differs");
        staging::metadata_feature(&locked, &root.join(name), FEATURE).unwrap();
        let lock = read_bounded(&root.join(name).join("Cargo.lock"), 1024 * 1024).unwrap();
        staging::closure_matches(&workspace, &lock).unwrap();
    }
    assert_eq!(
        read_bounded(
            &root.join("package-original").join(staging::LEAF),
            64 * 1024
        )
        .unwrap(),
        seed(n)
    );
}
fn derive_repeat(root: &Path, n: u8, case: &str) -> Invocation {
    validate_preparation(root, n);
    let name = package(n, case);
    let bytes = read_bounded(&root.join(name).join(staging::LEAF), 72 * 1024).unwrap();
    let expected = if fresh(case) {
        expected_candidate(n, edited(case))
    } else {
        original(n, case)
    };
    assert_eq!(bytes, expected);
    let record = derive_package(root, case, name, FEATURE);
    assert_eq!(record.leaf_sha256, digest(&bytes));
    record
}
#[test]
#[ignore = "after parent-supervised Cargo setup or successful publication, before a fresh child"]
fn prepare_ordered_repeat_invocation() {
    let (root, n) = root_and_count();
    let case = std::env::var(CASE_ENV).expect("closed case");
    let record = derive_repeat(&root, n, &case);
    save(&root, &format!("{case}.invocation.json"), &record);
    emit(&json!({
        "schema":"fe2o3-test-ordered-repeat-invocation-preparation-v1",
        "repeat_count":n,"case":case,"invocation":record,
        "profile_source_sha256":digest(seed(n)),"frontend_sessions":0,
        "subprocesses_spawned":0,"acceptance":"invocation only",
    }));
}
fn receipt_source_check(root: &Path, n: u8, case: &str) {
    let prior = if edited(case) {
        "publish-edit"
    } else {
        "publish-preserve"
    };
    let bytes = read_bounded(&root.join(candidate(case)), 72 * 1024).unwrap();
    assert_eq!(bytes, expected_candidate(n, edited(case)));
    let facts: Value = serde_json::from_slice(
        &read_bounded(
            &root.join(format!("{prior}.publication-facts.json")),
            256 * 1024,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        facts["schema"],
        "fe2o3-test-ordered-repeat-publication-facts-v1"
    );
    assert_eq!(facts["case"], prior);
    assert_eq!(facts["repeat_count"], n);
    assert_eq!(facts["candidate"], candidate(case));
    assert_eq!(facts["published"]["original_sha256"], digest(seed(n)));
    assert_eq!(facts["published"]["candidate_sha256"], digest(&bytes));
    assert_eq!(facts["published"]["original_bytes"], seed(n).len());
    assert_eq!(facts["published"]["candidate_bytes"], bytes.len());
    assert_eq!(facts["published"]["program_edited"], edited(case));
    assert_eq!(facts["publisher_error"], Value::Null);
    assert_eq!(facts["inspection_error"], Value::Null);
    let meta = fs::symlink_metadata(root.join(candidate(case))).unwrap();
    assert!(meta.is_file() && !meta.file_type().is_symlink());
    assert_eq!(facts["published"]["candidate_device"], meta.dev());
    assert_eq!(facts["published"]["candidate_inode"], meta.ino());
}
fn observe(
    mut target: crate::production_pipeline::ordered_composition_v1::AuthenticatedOrderedCompositionDiagnosticV1<'_>,
    root: &Path,
    n: u8,
    case: &str,
    started: std::time::Instant,
) -> Result<Value, String> {
    target
        .with_observation_budget(|owner, budget| owner.verify_equivalence(budget))
        .map_err(|e| e.to_string())?;
    let source = target.materialized();
    assert_eq!(source.composition().definitions().len(), 1);
    assert_eq!(
        source.composition().helpers().len(),
        usize::from(fresh(case))
    );
    assert_eq!(source.composition().calls().len(), usize::from(fresh(case)));
    assert_eq!(source.composition().occurrences().len(), 1);
    let semantic = *source
        .semantic_ssa()
        .source_semantic()
        .semantic_sha256()
        .as_bytes();
    assert_eq!(target.source_seed().semantic_sha256(), &semantic);
    let identity = *source.executable().identity();
    let before = digest(source.executable().canonical_bytes());
    let definition = source.composition().definitions()[0].key();
    let expected = expected_program(n, fresh(case) && edited(case));
    target
        .with_observation_budget(|owner, budget| {
            fe2o3_lower_mir_kernel::with_ordered_composition_inspection_v1(
                owner,
                budget,
                |view, budget| {
                    let region = view.definition(definition, &identity, &semantic, budget)?;
                    assert_eq!(*region.program().program(), expected);
                    let registers = region.program().registers();
                    assert_eq!(registers.scratch(), 8);
                    assert_eq!(registers.output(), 9);
                    assert_eq!(registers.inputs(), [10, 11, 12]);
                    assert_eq!(
                        region.canonical_site().function_ordinal()
                            != owner.composition().root_function_ordinal(),
                        fresh(case),
                    );
                    Ok(())
                },
            )
        })
        .map_err(|e| e.to_string())?;
    if fresh(case) {
        receipt_source_check(root, n, case);
        let result = cpu::observe(target.materialized().executable(), n, edited(case), started);
        return Ok(json!({
            "stage":"fresh_candidate_frontend","repeat_count":n,
            "program_count":expected.count(),"descriptors":expected.descriptors(),
            "registers":[10,11,12,8,9],"cpu":result,
            "canonical_identity":super::super::super::lower_hex_v1(identity.digest()),
            "canonical_bytes_sha256":before,
            "semantic_sha256":super::super::super::lower_hex_v1(&semantic),
            "source_owner_reused":false,"helpers":1,"calls":1,"occurrences":1,
            "flat_expanded_helper":true,"generator_recovered":false,
            "normal_checked_handoff_qualified":false,
        }));
    }
    let original_path = format!("{}/{}", package(n, case), staging::LEAF);
    let old_bytes = read_bounded(&root.join(&original_path), 64 * 1024).unwrap();
    assert_eq!(old_bytes, original(n, case));
    assert!(!root.join(candidate(case)).exists());
    let hash: [u8; 32] = Sha256::digest(&old_bytes).into();
    let mut published = None;
    let mut publish_error = None;
    let inspected = target.with_source_observation_budget(|owner, seed, budget| {
        fe2o3_lower_mir_kernel::with_ordered_composition_inspection_v1(
            owner,
            budget,
            |view, budget| {
                let region = view.definition(definition, &identity, &semantic, budget)?;
                assert_eq!(*region.program().program(), expected_program(n, false));
                let edit = edited(case).then_some(Edit {
                    program: expected_program(n, true),
                    registers: region.program().registers(),
                });
                let request = Request {
                    definition,
                    expected_canonical: &identity,
                    expected_semantic: semantic,
                    original_path: &original_path,
                    original_sha256: hash,
                    candidate_path: candidate(case),
                    helper_name: HELPER,
                    edit,
                };
                match publish_ordered_composition_source_v1(seed, view, &request, budget) {
                    Ok(receipt) => published = Some(receipt),
                    Err(error) => publish_error = Some(error),
                }
                Ok(())
            },
        )
    });
    // Retain actual effects BEFORE later assertions, CPU work, or error unwrap.
    // A killed child may leave no facts; the external parent must mark unknown.
    let facts = json!({
        "schema":"fe2o3-test-ordered-repeat-publication-facts-v1",
        "repeat_count":n,"case":case,"candidate":candidate(case),
        "published":published.as_ref().map(|p|json!({
            "original_sha256":super::super::super::lower_hex_v1(&p.original_sha256),
            "candidate_sha256":super::super::super::lower_hex_v1(&p.candidate_sha256),
            "original_bytes":p.original_bytes,"candidate_bytes":p.candidate_bytes,
            "candidate_device":p.candidate_device,"candidate_inode":p.candidate_inode,
            "program_edited":p.program_edited,
        })),
        "publisher_error":publish_error.as_ref().map(ToString::to_string),
        "inspection_error":inspected.as_ref().err().map(ToString::to_string),
        "publication_may_have_created":published.is_some() ||
            publish_error.as_ref().is_some_and(|e|e.effect==Effect::MayHaveCreatedCandidate),
        "acceptance":"historical effects only; requires completed child and parent",
    });
    save(root, &format!("{case}.publication-facts.json"), &facts);
    inspected.map_err(|e| format!("inspection failed after retained effects: {e}"))?;
    assert_eq!(
        read_bounded(&root.join(&original_path), 64 * 1024).unwrap(),
        old_bytes
    );
    assert_eq!(
        digest(target.materialized().executable().canonical_bytes()),
        before
    );
    if case == "refuse-noncanonical" {
        assert!(published.is_none());
        let error = publish_error.expect("expected exact spelling refusal");
        assert_eq!(error.effect, Effect::NotAttempted);
        assert!(error.to_string().contains(REFUSAL));
        assert!(!root.join(candidate(case)).exists());
        return Ok(json!({
            "stage":"exact_noncanonical_repeat_spelling_refused","repeat_count":n,
            "program_count":expected.count(),"descriptors":expected.descriptors(),
            "facts":facts,"candidate_created":false,"cpu_cases":0,
            "normal_checked_handoff_qualified":false,
        }));
    }
    assert!(publish_error.is_none());
    let receipt = published.expect("genuine source publication");
    receipt_source_check(root, n, case);
    target
        .with_observation_budget(|_, budget| {
            budget.reserve_storage(receipt.retained_storage_bytes())
        })
        .map_err(|e| e.to_string())?;
    let result = cpu::observe(target.materialized().executable(), n, false, started);
    Ok(json!({
        "stage":"actual_repeat_source_publication","repeat_count":n,
        "program_count":expected.count(),"descriptors":expected.descriptors(),
        "registers":[10,11,12,8,9],"cpu":result,"facts":facts,
        "canonical_identity":super::super::super::lower_hex_v1(identity.digest()),
        "canonical_bytes_sha256":before,
        "semantic_sha256":super::super::super::lower_hex_v1(&semantic),
        "source_ledger":target.resource_usage(),"helpers":0,"calls":0,"occurrences":1,
        "normal_checked_handoff_qualified":false,
    }))
}
struct RepeatCallbacks<'a> {
    root: &'a Path,
    n: u8,
    case: &'a str,
    started: std::time::Instant,
    calls: usize,
    result: Option<Result<Value, String>>,
}
impl Callbacks for RepeatCallbacks<'_> {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        self.result = Some((|| {
            let transaction = super::super::super::transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            let target = transaction.lower_ordered_composition_diagnostic_v1()?;
            observe(target, self.root, self.n, self.case, self.started)
        })());
        Compilation::Stop
    }
}
#[test]
#[ignore = "one fresh frontend only; use the separately bounded repeat campaign parent"]
fn actual_ordered_repeat_promotion_child() {
    let started = std::time::Instant::now();
    let (root, n) = root_and_count();
    assert_eq!(
        std::env::current_dir().unwrap().canonicalize().unwrap(),
        root.canonicalize().unwrap()
    );
    let case = std::env::var(CASE_ENV).expect("closed case");
    let actual = derive_repeat(&root, n, &case);
    let retained: Invocation = serde_json::from_slice(
        &read_bounded(&root.join(format!("{case}.invocation.json")), 256 * 1024).unwrap(),
    )
    .unwrap();
    assert_eq!(actual, retained);
    for (key, expected) in [
        (CRATE_BINDING_ID_ENV_V1, actual.crate_binding.as_str()),
        (
            CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
            actual.cargo_observation.as_str(),
        ),
        ("CARGO_PKG_NAME", staging::PACKAGE_NAME),
        ("CARGO_PKG_VERSION", "0.1.0"),
        ("CARGO_CRATE_NAME", staging::LIB_NAME),
    ] {
        assert_eq!(std::env::var(key).unwrap(), expected);
    }
    assert_eq!(
        PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap()),
        root.join(package(n, &case))
    );
    super::super::super::require_canonical_overflow_checks_v1(&actual.args).unwrap();
    let mut callbacks = RepeatCallbacks {
        root: &root,
        n,
        case: &case,
        started,
        calls: 0,
        result: None,
    };
    rustc_driver::run_compiler(&actual.args, &mut callbacks);
    assert_eq!(callbacks.calls, 1);
    let result = callbacks.result.expect("actual frontend callback missing");
    if let Err(error) = &result {
        assert!(error.len() <= 64 * 1024);
        save(
            &root,
            &format!("{case}.callback-error.json"),
            &json!({
                "accepted":false,"diagnostic":error,
                "publication_effects":"consult retained facts; unknown if absent or child interrupted",
            }),
        );
    }
    let observed = result.unwrap();
    assert_eq!(derive_repeat(&root, n, &case), actual);
    assert!(
        fs::read_dir(preparation(&root, package(n, &case)).join("analysis-output"))
            .unwrap()
            .next()
            .is_none()
    );
    timely(started.elapsed(), 300).unwrap();
    emit(&json!({
        "schema":"fe2o3-test-ordered-repeat-promotion-observation-v1",
        "case":case,"repeat_count":n,"profile_source_sha256":digest(seed(n)),
        "invocation":actual,"observation":observed,"actual_rustc_callbacks":callbacks.calls,
        "fresh_frontend":true,"normal_checked_handoff_qualified":false,
        "native_execution":false,"launch_authority":false,
    }));
    timely(started.elapsed(), 300).unwrap();
}
#[test]
fn repeat_roster_has_twelve_positive_frontends_and_one_separate_refusal() {
    let mut total = 0;
    for n in [1, 2, 15] {
        for case in POSITIVES {
            assert!(!package(n, case).is_empty());
            assert_eq!(fresh(case), case.starts_with("recompile-"));
            total += 1;
        }
    }
    assert_eq!(total, 12);
    assert_eq!(package(2, "refuse-noncanonical"), "package-promoted-copy");
    assert_eq!(
        packages(1).len() + packages(2).len() + packages(15).len(),
        10
    );
}
#[test]
fn repeat_descriptor_oracle_includes_count_padding_and_edit_distinction() {
    for n in [1, 2, 15] {
        let p = expected_program(n, false);
        assert_eq!(p.count(), n + 1);
        assert_eq!(p.descriptors()[0], 8);
        assert!(
            p.descriptors()[1..=usize::from(n)]
                .iter()
                .all(|&v| v == 201)
        );
        assert!(
            p.descriptors()[usize::from(n) + 1..]
                .iter()
                .all(|&v| v == 0)
        );
        assert_eq!(expected_program(n, true).descriptors()[0], 40);
        assert_ne!(p, expected_program(n, true));
    }
}
#[test]
fn repeat_source_oracle_is_flat_and_preserves_unselected_source() {
    for n in [1, 2, 15] {
        for change in [false, true] {
            let bytes = expected_candidate(n, change);
            let source = std::str::from_utf8(&bytes).unwrap();
            assert_eq!(source.matches(&format!("fn {HELPER}(")).count(), 1);
            assert_eq!(source.matches(&format!("{HELPER}(a, b, c)")).count(), 1);
            assert_eq!(
                source
                    .matches("fe2o3_device::amdgpu_ordered_program!")
                    .count(),
                1
            );
            assert!(!source.contains("repeat(") && !source.contains("init {"));
            assert_eq!(
                source.matches("add(out, out, input1);").count(),
                if change { 0 } else { usize::from(n) }
            );
            assert!(source.ends_with("    let index = thread::index_1d();\n    if let Some(element) = output.get_mut(index) {\n        *element = value;\n    }\n}\n"));
        }
    }
    let text = String::from_utf8(noncanonical()).unwrap();
    assert!(text.contains("/* spelling intentionally refused */"));
    assert_eq!(
        text.replace("/* spelling intentionally refused */ ", "")
            .as_bytes(),
        seed(2)
    );
}
#[test]
fn repeat_profile_parser_refuses_noncanonical_and_unbounded_counts() {
    for bad in [
        "0", "01", "3", "16", "256", "-1", "1 ", "1\n", "0xf", "15u8",
    ] {
        assert!(std::panic::catch_unwind(|| count(bad)).is_err());
    }
}
