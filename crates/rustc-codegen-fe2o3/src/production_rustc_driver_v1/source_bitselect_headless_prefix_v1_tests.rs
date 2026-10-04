//! Public promotion of a live pure-prefix kernel, followed by fresh source
//! admission, independent complete-buffer checks and inert LLVM inspection.
//! The seed comes from the separately built normal-library consumer. Negative
//! children call the real public Rust API in this test binary, not a fake callback.
use super::*;
use crate::production_rustc_driver_v1::source_bitselect_promotion_driver_v1::run_bitselect_source_promotion_driver_v1;
use crate::source_bitselect_promotion_v1::{
    BitselectPromotionRequestV1, CandidatePublicationStateV1, FailurePhaseV1,
};

const OUTPUT: &str = "FE2O3_TEST_SOURCE_HEADLESS_PREFIX_OUTPUT";
const MODE: &str = "FE2O3_TEST_SOURCE_HEADLESS_PREFIX_MODE";
const CHILD: &str = "production_rustc_driver_v1::source_bitselect_feasibility_v1_tests::roundtrip::machine::headless_machine::prefix::actual_source_headless_prefix_child";
const PREFIX: &str = "FE2O3_SOURCE_HEADLESS_PREFIX ";
const SELECTION: &str = "    let selected = b ^ ((a ^ b) & mask);\n";
const SHAPE: &str = "source-boundary prefix requires immutable direct-u32 bitwise lets";
const OPERAND: &str = "source-boundary prefix operands must be original immutable u32 formals";
const SHADOW: &str = "source-boundary prefix binding shadows a formal, prefix or selected result";
const CONTROLS: [&str; 17] = [
    "eight",
    "nine",
    "mutable",
    "alias",
    "dependent",
    "nested",
    "wrong-type",
    "shadow-prefix",
    "shadow-selected",
    "shadow-formal",
    "call",
    "branch",
    "statement",
    "macro",
    "ambiguous",
    "normalization",
    "attribution-collision",
];

fn source(case: &str) -> String {
    assert!(case == "positive" || CONTROLS.contains(&case));
    let baseline = std::str::from_utf8(FIXTURE_FILES[2].1).unwrap();
    let boundary = "#[cfg(feature = \"source-bitselect-ambiguous\")]";
    let (active, rest) = baseline.split_once(boundary).unwrap();
    assert_eq!(active.matches(SELECTION).count(), 1);
    assert_eq!(active.matches("*slot = selected;").count(), 1);
    let prefix = match case {
        "positive" | "normalization" => "    let tag = a | mask;\n".into(),
        "eight" | "nine" => {
            let count = if case == "eight" { 8 } else { 9 };
            let mut text = String::from("    let tag = a | mask;\n");
            let expressions = [
                "a & b", "a | b", "a & mask", "a ^ mask", "b & mask", "b | mask", "b ^ mask",
                "a ^ b",
            ];
            for (index, expression) in expressions.iter().take(count - 1).enumerate() {
                text.push_str(&format!("    let prefix{} = {expression};\n", index + 1));
            }
            text
        }
        "attribution-collision" => {
            // Optimized MIR may reuse this earlier identical inner XOR. The
            // exact selected HIR/semantic attribution must then refuse.
            let mut text = String::from("    let tag = a | mask;\n");
            for index in 1..8 {
                let operator = ["&", "|", "^"][index % 3];
                text.push_str(&format!("    let prefix{index} = a {operator} b;\n"));
            }
            text
        }
        "mutable" => "    let mut tag = a | mask;\n".into(),
        "alias" => "    let tag = a;\n".into(),
        "dependent" => "    let first = a | mask;\n    let tag = first ^ b;\n".into(),
        "nested" => "    let tag = (a | mask) ^ b;\n".into(),
        "wrong-type" => "    let extra: u64 = 1 | 2;\n    let tag = a | mask;\n".into(),
        "shadow-prefix" => "    let tag = a | b;\n    let tag = a | mask;\n".into(),
        "shadow-selected" => "    let selected = a | b;\n    let tag = a | mask;\n".into(),
        "shadow-formal" => "    let a = a | b;\n    let tag = a | mask;\n".into(),
        "call" => "    let tag = prefix_identity(a);\n".into(),
        "branch" => "    let tag = if a == b { a } else { mask };\n".into(),
        "statement" => {
            "    if let Some(slot) = output.get_mut(thread::index_1d()) { *slot = a; }\n    let tag = a | mask;\n"
                .into()
        }
        "macro" => "    let tag = prefix_value!(a, mask);\n".into(),
        "ambiguous" => "    let other = b ^ ((a ^ b) & mask);\n    let tag = a | mask;\n".into(),
        _ => unreachable!(),
    };
    let mut changed = active
        .replacen(SELECTION, &format!("{prefix}{SELECTION}"), 1)
        .replacen("*slot = selected;", "*slot = selected ^ tag;", 1);
    changed.push_str(boundary);
    changed.push_str(rest);
    if case == "call" {
        changed.insert_str(0, "fn prefix_identity(value: u32) -> u32 { value }\n");
    } else if case == "macro" {
        changed.insert_str(
            0,
            "macro_rules! prefix_value { ($a:expr, $b:expr) => { $a | $b }; }\n",
        );
    } else if case == "normalization" {
        changed = changed.replace('\n', "\r\n");
    }
    assert!(changed.len() <= 64 * 1024);
    changed
}

fn refusal(case: &str) -> Option<&'static str> {
    match case {
        "eight" => None,
        "nine" => Some("source-boundary prefix binding limit"),
        "mutable" | "alias" | "wrong-type" | "call" | "branch" | "statement" => Some(SHAPE),
        "dependent" | "nested" => Some(OPERAND),
        "shadow-prefix" | "shadow-selected" => Some(SHADOW),
        "shadow-formal" => Some("source-boundary local alias is not a parameter"),
        "macro" => Some(SHAPE),
        "ambiguous" => Some("source-boundary ambiguous bitselect initializers"),
        "normalization" => Some("source-boundary normalization changes original offsets"),
        "attribution-collision" => Some("source-boundary exact HIR operator/semantic span absent"),
        _ => panic!("closed prefix control"),
    }
}

fn source_pair(case: &str) -> (String, String) {
    assert!(CONTROLS.contains(&case));
    (
        format!("prefix-{case}.rs"),
        format!("prefix-{case}-loader.rs"),
    )
}

fn control_record(directory: &Path, case: &str) -> RoundtripInvocation {
    let (source, loader) = source_pair(case);
    let absolute = paths::absolute_case(directory, "positive");
    paths::checked_source_file(&absolute.join(&source));
    paths::checked_source_file(&absolute.join(&loader));
    let (args, crate_binding, cargo_observation) = invocation_for_fixture_source(
        directory,
        &fixture(),
        PACKAGE,
        CRATE_NAME,
        Some(FEATURES[0]),
        &absolute.join(&loader),
        "gfx942",
    );
    RoundtripInvocation {
        case: case.into(),
        phase: "prefix-public-api".into(),
        source_directory: paths::relative_case(directory, "positive"),
        args,
        crate_binding,
        cargo_observation,
        source_sha256: hash(&absolute.join(source)),
        loader_sha256: hash(&absolute.join(loader)),
        fixture_sha256: FIXTURE_FILES.map(|(path, _)| hash(&fixture().join(path))),
        artifacts_sha256: hash(&directory.join("dependencies.stdout")),
        metadata_sha256: hash(&directory.join("metadata.stdout")),
    }
}

fn bind_environment<'a>(command: &'a mut Command, record: &RoundtripInvocation) -> &'a mut Command {
    command
        .env(CRATE_BINDING_ID_ENV_V1, &record.crate_binding)
        .env(
            CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
            &record.cargo_observation,
        )
        .env("CARGO_MANIFEST_DIR", fixture())
        .env("CARGO_PKG_NAME", PACKAGE)
        .env("CARGO_PKG_VERSION", "0.1.0")
        .env("CARGO_CRATE_NAME", CRATE_NAME)
}

fn check_environment(record: &RoundtripInvocation) {
    assert_eq!(std::env::current_dir().unwrap(), repository());
    assert_eq!(
        std::env::var(CRATE_BINDING_ID_ENV_V1).unwrap(),
        record.crate_binding
    );
    assert_eq!(
        std::env::var(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2).unwrap(),
        record.cargo_observation
    );
    assert_eq!(
        std::env::var("CARGO_MANIFEST_DIR").unwrap(),
        fixture().to_str().unwrap()
    );
    assert_eq!(std::env::var("CARGO_PKG_NAME").unwrap(), PACKAGE);
    assert_eq!(std::env::var("CARGO_PKG_VERSION").unwrap(), "0.1.0");
    assert_eq!(std::env::var("CARGO_CRATE_NAME").unwrap(), CRATE_NAME);
    crate::production_rustc_driver_v1::require_canonical_overflow_checks_v1(&record.args).unwrap();
}

struct PrefixCallbacks {
    source: Option<RetainedInput>,
    plan: Gfx942OrderedProgramRegistersV1,
    calls: usize,
    result: Option<Result<(Value, String), String>>,
}
impl Callbacks for PrefixCallbacks {
    fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        let input = self
            .source
            .take()
            .expect("one retained fresh prefix candidate");
        self.result = Some(
            crate::production_rustc_driver_v1::transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )
            .and_then(|transaction| {
                transaction.observe_source_bitselect_prefix_candidate_machine(input, self.plan)
            }),
        );
        Compilation::Stop
    }
}

#[test]
#[ignore = "actual isolated compiler child; use the prefix ladder"]
fn actual_source_headless_prefix_child() {
    let directory = PathBuf::from(std::env::var_os(INPUT).unwrap());
    let case = std::env::var(SELECTOR).unwrap();
    let mode = std::env::var(MODE).unwrap();
    let fresh = mode == "fresh";
    assert!(fresh || mode == "control");
    if fresh {
        assert!(["default", "edited", "repeat"].contains(&case.as_str()));
    }
    require_current_source();
    let actual = if fresh {
        derive(&directory, &case)
    } else {
        control_record(&directory, &case)
    };
    let saved: RoundtripInvocation = serde_json::from_slice(
        &read_bounded(
            &directory.join(format!("prefix-{case}.invocation.json")),
            64 * 1024,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(actual, saved);
    check_environment(&actual);
    let source_name = if fresh {
        source_files(&case).0.to_owned()
    } else {
        source_pair(&case).0
    };
    let source_path = actual.source_directory.join(source_name);
    let before = read_bounded(&source_path, 128 * 1024).unwrap();
    let observation = if fresh {
        let mut callbacks = PrefixCallbacks {
            source: Some(RetainedInput::open(source_path.to_str().unwrap(), true).unwrap()),
            plan: register_plan(&case),
            calls: 0,
            result: None,
        };
        rustc_driver::run_compiler(&actual.args, &mut callbacks);
        assert_eq!(callbacks.calls, 1);
        let (observation, llvm) = callbacks.result.unwrap().unwrap();
        assert_eq!(observation["fresh"]["boolean_oracle_cases"], 128);
        assert_eq!(observation["whole_kernel_simulation"]["runs"], 30);
        assert_eq!(
            observation["whole_kernel_simulation"]["oracle"],
            "host_u32_masked_or_xor_or_prefix_exact"
        );
        let llvm_path = directory.join("positive").join(format!("prefix-{case}.ll"));
        assert!(llvm.len() <= 64 * 1024);
        paths::write_new(&llvm_path, llvm.as_bytes());
        assert_eq!(
            read_bounded(&llvm_path, 64 * 1024).unwrap(),
            llvm.as_bytes()
        );
        json!({"result":observation,"llvm_path":llvm_path,"actual_fresh_callback":true})
    } else {
        let candidate = actual
            .source_directory
            .join(format!("prefix-{case}-candidate.rs"));
        assert!(!candidate.try_exists().unwrap());
        let request = BitselectPromotionRequestV1::new(
            source_path.to_str().unwrap(),
            candidate.to_str().unwrap(),
            Sha256::digest(&before).into(),
            Gfx942OrderedProgramRegistersV1::new(4, 5, [0, 1, 2]).unwrap(),
        )
        .unwrap();
        let attempt = run_bitselect_source_promotion_driver_v1(&actual.args, request);
        match (refusal(&case), attempt.result()) {
            (None, Ok(published)) => {
                let bytes = read_bounded(&candidate, 128 * 1024).unwrap();
                assert_eq!(
                    published.candidate_sha256(),
                    &<[u8; 32]>::from(Sha256::digest(&bytes))
                );
                assert_eq!(published.candidate_bytes(), bytes.len());
                json!({"published":true,"candidate":candidate,"candidate_sha256":published.candidate_sha256(),
                    "candidate_bytes":bytes.len(),"fresh_candidate_compile_observed":false})
            }
            (Some(expected), Err(error)) => {
                assert!(
                    !error.compiler_fatal(),
                    "unexpected compiler fatal for {case}: {}",
                    error.diagnostic()
                );
                assert_eq!(error.phase(), FailurePhaseV1::Eligibility);
                assert_eq!(
                    error.publication(),
                    CandidatePublicationStateV1::NotAttempted
                );
                assert_eq!(
                    error.diagnostic(),
                    expected,
                    "wrong refusal boundary for {case}"
                );
                assert!(!candidate.try_exists().unwrap());
                json!({"published":false,"diagnostic":error.diagnostic(),
                    "public_api_in_test_process":true,"external_normal_consumer":false})
            }
            (None, Err(error)) => panic!(
                "eligible prefix {case} refused: {:?}/{:?}: {}",
                error.phase(),
                error.publication(),
                error.diagnostic()
            ),
            _ => panic!("unexpected public prefix result for {case}"),
        }
    };
    assert_eq!(read_bounded(&source_path, 128 * 1024).unwrap(), before);
    assert_eq!(hash(&source_path), actual.source_sha256);
    assert!(
        fs::read_dir(directory.join("analysis-output"))
            .unwrap()
            .next()
            .is_none()
    );
    require_current_source();
    let bytes =
        serde_json::to_vec(&json!({"invocation":actual,"observation":observation})).unwrap();
    assert!(bytes.len() <= 64 * 1024);
    println!("\n{PREFIX}{}", std::str::from_utf8(&bytes).unwrap());
}

fn child(directory: &Path, case: &str, fresh: bool) -> Value {
    let record = if fresh {
        derive(directory, case)
    } else {
        control_record(directory, case)
    };
    paths::write_new(
        &directory.join(format!("prefix-{case}.invocation.json")),
        &serde_json::to_vec_pretty(&record).unwrap(),
    );
    let mut command = Command::new(std::env::current_exe().unwrap());
    let stdout = checked(
        bind_environment(
            sanitized(&mut command)
                .current_dir(repository())
                .args(["--exact", CHILD, "--ignored", "--nocapture"])
                .env(INPUT, directory)
                .env(SELECTOR, case)
                .env(MODE, if fresh { "fresh" } else { "control" }),
            &record,
        ),
        directory,
        &format!("prefix-{case}"),
        None,
    );
    let text = std::str::from_utf8(&stdout).unwrap();
    assert_eq!(
        text.lines()
            .filter(|line| line.starts_with("test result: ok. 1 passed; 0 failed; 0 ignored;"))
            .count(),
        1
    );
    let mut lines = text.lines().filter_map(|line| line.strip_prefix(PREFIX));
    let line = lines.next().unwrap();
    assert!(line.len() <= 64 * 1024 && lines.next().is_none());
    let value: Value = serde_json::from_str(line).unwrap();
    assert_eq!(value["invocation"], serde_json::to_value(&record).unwrap());
    value
}

#[test]
#[ignore = "normal consumer + genuine fresh callbacks; serialized Cargo; fresh bounded output"]
fn actual_source_headless_prefix_ladder() {
    let consumer = PathBuf::from(std::env::var_os(CONSUMER).unwrap());
    let consumer_before = consumer_observation(&consumer);
    let directory = PathBuf::from(std::env::var_os(OUTPUT).unwrap());
    assert!(directory.is_absolute());
    fs::create_dir(&directory).unwrap();
    let directory = directory.canonicalize().unwrap();
    let root = paths::create_root(&directory);
    require_current_source();
    super::super::super::prepare(&directory);
    fs::create_dir(directory.join("positive")).unwrap();
    let case = root.join("positive");
    fs::create_dir(&case).unwrap();
    let original = source("positive");
    paths::write_new(&case.join("original.rs"), original.as_bytes());
    paths::write_new(&case.join("original-loader.rs"), ORIGINAL_LOADER);
    paths::write_new(&case.join("candidate-loader.rs"), CANDIDATE_LOADER);
    let publication = publish_with_normal_consumer(&directory, &consumer);
    let candidate = read_bounded(&case.join("candidate.rs"), 128 * 1024).unwrap();
    let candidate_text = std::str::from_utf8(&candidate).unwrap();
    let (prefix, suffix) = original.split_once("b ^ ((a ^ b) & mask)").unwrap();
    assert!(candidate_text.starts_with(prefix) && candidate_text.ends_with(suffix));
    assert!(prefix.contains("let tag = a | mask;"));
    assert!(suffix.contains("*slot = selected ^ tag;"));
    const LOW: &str = "scratch(4); out(5);\n    in(0) = a;\n    in(1) = b;\n    in(2) = mask;";
    const HIGH: &str =
        "scratch(32); out(33);\n    in(34) = a;\n    in(35) = b;\n    in(36) = mask;";
    assert_eq!(candidate_text.matches(LOW).count(), 1);
    let edited = candidate_text.replacen(LOW, HIGH, 1);
    assert!(edited.len() <= 128 * 1024);
    assert!(edited.starts_with(prefix) && edited.ends_with(suffix));
    paths::write_new(&case.join("edited.rs"), edited.as_bytes());
    paths::write_new(
        &case.join("edited-loader.rs"),
        b"#![no_std]\n#[path = \"edited.rs\"] mod source_bitselect_feasibility;\n",
    );
    let default = child(&directory, "default", true);
    let high = child(&directory, "edited", true);
    let repeat = child(&directory, "repeat", true);
    assert_eq!(
        high["observation"]["result"],
        repeat["observation"]["result"]
    );
    for identity in ["kernel_ir_sha256", "semantic_sha256", "candidate_sha256"] {
        assert_ne!(
            default["observation"]["result"]["fresh"][identity],
            high["observation"]["result"]["fresh"][identity]
        );
    }
    assert_ne!(
        default["observation"]["result"]["llvm_sha256"],
        high["observation"]["result"]["llvm_sha256"]
    );
    let mut controls = Vec::new();
    for control in CONTROLS {
        let (source_name, loader_name) = source_pair(control);
        paths::write_new(&case.join(&source_name), source(control).as_bytes());
        paths::write_new(
            &case.join(loader_name),
            format!("#![no_std]\n#[path = \"{source_name}\"] mod source_bitselect_feasibility;\n")
                .as_bytes(),
        );
        controls.push(child(&directory, control, false));
    }
    assert_eq!(
        read_bounded(&case.join("original.rs"), 128 * 1024).unwrap(),
        original.as_bytes()
    );
    assert_eq!(
        read_bounded(&case.join("candidate.rs"), 128 * 1024).unwrap(),
        candidate
    );
    assert_eq!(
        read_bounded(&case.join("edited.rs"), 128 * 1024).unwrap(),
        edited.as_bytes()
    );
    assert_eq!(consumer_observation(&consumer), consumer_before);
    let mut inventory = Vec::new();
    for entry in fs::read_dir(&case).unwrap().take(43) {
        let path = entry.unwrap().path();
        paths::checked_source_file(&path);
        inventory.push(
            json!({"path":path,"bytes":fs::metadata(&path).unwrap().len(),"sha256":hash(&path)}),
        );
    }
    assert_eq!(inventory.len(), 41); // Six positive leaves, 34 control leaves, one cap-positive candidate.
    inventory.sort_by_key(|row| row["path"].as_str().unwrap().to_owned());
    let report = json!({
        "normal_consumer_publication":publication,"default":default,"edited":high,"repeat":repeat,
        "public_api_controls":controls,"normal_library_publication_processes":1,
        "fresh_candidate_callbacks":3,"positive_whole_kernel_simulation_runs":90,
        "public_api_control_sessions":17,"exact_prefix_refusals":16,
        "unchanged_prefix_and_suffix":true,"source_files":inventory,
        "source_file_limit":42,"source_byte_limit":42*128*1024,
        "ranked_checks":false,"production_resume":false,"native_qualified":false,
        "hardware_observed":false,"grants_artifact_or_launch_authority":false,
    });
    let bytes = serde_json::to_vec_pretty(&report).unwrap();
    assert!(bytes.len() <= 512 * 1024);
    paths::write_new(&directory.join("prefix-observation.json"), &bytes);
    require_current_source();
}

#[test]
fn source_bitselect_prefix_controls_preserve_inactive_source_and_live_output() {
    let baseline = std::str::from_utf8(FIXTURE_FILES[2].1).unwrap();
    let boundary = "#[cfg(feature = \"source-bitselect-ambiguous\")]";
    let rest = baseline.split_once(boundary).unwrap().1;
    for case in std::iter::once("positive").chain(CONTROLS) {
        let text = source(case);
        if case != "normalization" {
            assert_eq!(text.split_once(boundary).unwrap().1, rest);
        }
        assert!(text.contains("*slot = selected ^ tag;"));
    }
    assert_eq!(source("eight").matches("    let prefix").count(), 7);
    assert_eq!(source("nine").matches("    let prefix").count(), 8);
    assert_eq!(refusal("eight"), None);
}
