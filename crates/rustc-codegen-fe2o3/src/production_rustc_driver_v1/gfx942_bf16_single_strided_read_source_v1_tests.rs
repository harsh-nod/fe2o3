//! Separate three-session source qualification for a single constant strided read.
//! Historical five-session CPU qualification and its cases are unchanged.
use super::*;
#[path = "gfx942_bf16_single_strided_read_inputs_v1_tests.rs"]
mod inputs;
const OUTPUT_ENV: &str = "FE2O3_TEST_BF16_SINGLE_READ_OUTPUT_V1";
const CHILD_ENV: &str = "FE2O3_TEST_BF16_SINGLE_READ_INPUTS_V1";
const CASE_ENV: &str = "FE2O3_TEST_BF16_SINGLE_READ_CASE_V1";
const CHILD: &str = "production_rustc_driver_v1::gfx942_bf16_call_source_cpu_qualification_v1_tests::single_strided_read::actual_single_strided_read_source_child";
const PREFIX: &str = "FE2O3_BF16_SINGLE_READ_SOURCE_OBSERVATION_V1 ";
const MARKER: &str = "fe2o3-whole-root-initial-nonempty-reads-v1 ";
const FEATURES: [&str; 3] = ["single-strided-read", "identity", "wrong-launch"];
const CASES: [&str; 3] = ["single-strided-read", "identity-empty", "wrong-launch"];
fn checked_feature(feature: &str) -> Result<(), &'static str> {
    FEATURES
        .contains(&feature)
        .then_some(())
        .ok_or("unknown or combined single-read source feature")
}
fn feature_for_case(case: &str) -> Result<&str, &'static str> {
    match case {
        "single-strided-read" => Ok("single-strided-read"),
        "identity-empty" => Ok("identity"),
        "wrong-launch" => Ok("wrong-launch"),
        _ => Err("unknown single-read source case"),
    }
}
fn flags_false(report: &Value) -> bool {
    [
        "normal_qualified",
        "numerical_cpu_qualified",
        "hardware_observed",
        "native_execution_attempted",
        "grants_artifact_or_launch_authority",
    ]
    .into_iter()
    .all(|key| report[key] == false)
}
fn accept(case: &str, report: &Value) -> Result<(), &'static str> {
    feature_for_case(case)?;
    if report["stage"] != "actual_single_strided_read_source_observation"
        || report["unexpected_normal_success"] != false
        || !flags_false(report)
    {
        return Err("single-read observation authority or stage differs");
    }
    let diagnostic = report["diagnostic"]
        .as_str()
        .ok_or("single-read refusal diagnostic absent")?;
    if case == "wrong-launch" {
        if !report["phase"].is_null()
            || !diagnostic.contains("BF16 helper requires explicit WG64 and one workgroup")
        {
            return Err("wrong launch did not refuse before single-read owner");
        }
        return Ok(());
    }
    let phase = &report["phase"];
    if phase["same_ledger"] != true
        || phase["failed_work"] != false
        || phase["failed_storage"] != false
        || phase["normal_succeeded"] != false
        || count(&phase["work"])? == 0
    {
        return Err("single-read source owner ledger differs");
    }
    let entry = count(&phase["entry_storage"])?;
    let final_storage = count(&phase["final_storage"])?;
    if case == "identity-empty" {
        if phase["materialized"] != false
            || phase["normal_attempted"] != false
            || final_storage != entry
            || !diagnostic.contains("genuine nonempty-read observer found no read-view effect")
        {
            return Err("empty source was not independently refused by nonempty profile");
        }
        return Ok(());
    }
    let retained = count(&phase["occurrence_storage"])?
        .checked_add(count(&phase["nominal_storage"])?)
        .ok_or("single-read receipt overflow")?;
    if phase["materialized"] != true
        || phase["normal_attempted"] != true
        || retained == 0
        || count(&phase["source_storage"])? == 0
        || final_storage
            != entry
                .checked_add(retained)
                .ok_or("single-read storage overflow")?
        || count(&phase["phase_peak_storage"])? < final_storage
        || count(&phase["phase_peak_storage"])? > 2 * 1024 * 1024 * 1024
        || !diagnostic.contains("BF16 nominal source-ranked projection")
    {
        return Err("single-read actual observation/unchanged ordinary refusal incomplete");
    }
    Ok(())
}

fn canonical_count(value: &str) -> Result<u64, &'static str> {
    if value.is_empty()
        || !value.bytes().all(|b| b.is_ascii_digit())
        || value.len() > 20
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err("noncanonical source marker count");
    }
    value.parse().map_err(|_| "source marker count overflow")
}
// Count the bare tag everywhere, not just well-formed line prefixes. Linux
// emitters produce LF-terminated records; wrapped, embedded, unterminated and
// CR/CRLF records are refused rather than silently ignored.
fn canonical_prefixed_rows<'a>(text: &'a str, prefix: &str) -> Result<Vec<&'a str>, &'static str> {
    let tag = prefix.trim_end_matches(' ');
    if tag.is_empty() {
        return Err("empty source record tag");
    }
    let occurrences = text.matches(tag).count();
    let mut rows = Vec::new();
    for chunk in text.split_inclusive('\n') {
        if !chunk.contains(tag) {
            continue;
        }
        let line = chunk
            .strip_suffix('\n')
            .ok_or("unterminated source record")?;
        if line.contains('\r') {
            return Err("noncanonical CR in source record");
        }
        let row = line
            .strip_prefix(prefix)
            .ok_or("wrapped or malformed source record")?;
        if row.contains(tag) {
            return Err("embedded source record tag");
        }
        rows.push(row);
    }
    if rows.len() != occurrences {
        return Err("source record occurrence count differs");
    }
    Ok(rows)
}
fn single_observation_frame(stdout: &str) -> Result<&str, &'static str> {
    let rows = canonical_prefixed_rows(stdout, PREFIX)?;
    if rows.len() != 1 {
        return Err("single-read requires exactly one canonical JSON frame");
    }
    Ok(rows[0])
}
fn accept_markers(case: &str, stderr: &str) -> Result<(), &'static str> {
    feature_for_case(case)?;
    let rows = canonical_prefixed_rows(stderr, MARKER)?;
    if case != "single-strided-read" {
        return if rows.is_empty() {
            Ok(())
        } else {
            Err("refused source emitted nonempty marker")
        };
    }
    if rows.len() != 3 {
        return Err("single-read requires three completed canonical callback modes");
    }
    let numbers = [
        "locals",
        "blocks",
        "effect_block",
        "read_views",
        "projected_rows",
        "prefix_operations",
        "next_value",
        "next_argument",
    ];
    let flags = [
        ("single_constant_profile", "true"),
        ("nonempty_read_coverage", "true"),
        ("retained_legacy_oracle", "true"),
        ("unchanged_legacy_donor_invoked", "false"),
        ("same_source_ledger_counter", "true"),
        ("retained_postflight", "true"),
        ("postflight_rows_compared", "true"),
        ("before_writer_comparison", "true"),
        ("old_completion_refused", "true"),
        ("foreign_counter_refused", "true"),
        ("reentry_terminal", "true"),
        ("graph_started", "false"),
        ("invocation_started", "false"),
        ("private_component", "true"),
        ("ordinary_route", "false"),
    ];
    let mut previous = None;
    for (row, mode) in rows
        .into_iter()
        .zip(["Compare", "CallbackError", "CallbackPanic"])
    {
        let fields = row.split(' ').collect::<Vec<_>>();
        if fields.len() != 1 + numbers.len() + flags.len() || fields[0] != format!("mode={mode}") {
            return Err("single-read marker mode/order/field roster differs");
        }
        let mut observed = [0u64; 8];
        for (index, key) in numbers.iter().enumerate() {
            let (actual_key, value) = fields[index + 1]
                .split_once('=')
                .ok_or("marker count field")?;
            if actual_key != *key {
                return Err("marker count field order");
            }
            observed[index] = canonical_count(value)?;
        }
        for (index, (key, value)) in flags.into_iter().enumerate() {
            if fields[1 + numbers.len() + index] != format!("{key}={value}") {
                return Err("single-read marker claim differs");
            }
        }
        if !(1..=4096).contains(&observed[0])
            || !(1..=32).contains(&observed[1])
            || observed[2] >= observed[1]
            || observed[3] != 1
            || observed[4] != observed[1]
            || observed[5..] != [6, 5, 1]
        {
            return Err("single-read observed source/SSA profile differs");
        }
        if previous.is_some_and(|prior| prior != observed) {
            return Err("callback modes did not preserve actual source/SSA profile");
        }
        previous = Some(observed);
    }
    Ok(())
}

struct SingleReadCallbacks {
    calls: usize,
    result: Option<Value>,
}
impl Callbacks for SingleReadCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.calls += 1;
        self.result = Some(
            match super::super::transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            ) {
                Ok(transaction) => {
                    let (result, phase) =
                        transaction.observe_bf16_single_strided_read_for_test_v1();
                    json!({"stage":"actual_single_strided_read_source_observation",
                    "phase":phase,"diagnostic":result.as_ref().err().map(ToString::to_string),
                    "unexpected_normal_success":result.is_ok(),"normal_qualified":false,
                    "numerical_cpu_qualified":false,"hardware_observed":false,
                    "native_execution_attempted":false,"grants_artifact_or_launch_authority":false})
                }
                Err(error) => json!({"stage":"single_read_collection_refused","diagnostic":error}),
            },
        );
        Compilation::Stop
    }
}
#[test]
#[ignore = "genuine isolated single-read source child; invoke through its three-session parent"]
fn actual_single_strided_read_source_child() {
    let started = std::time::Instant::now();
    let directory =
        PathBuf::from(std::env::var_os(CHILD_ENV).expect("actual single-read preparation"));
    assert!(directory.is_absolute());
    let case = std::env::var(CASE_ENV).expect("closed single-read case");
    let feature = feature_for_case(&case).unwrap();
    let actual = inputs::derive_record(&directory, feature);
    let retained: inputs::PreparedInvocation = serde_json::from_slice(
        &read_bounded(
            &directory.join(format!("{case}.invocation.json")),
            128 * 1024,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        actual, retained,
        "actual source/metadata/dependency/invocation drift"
    );
    for (key, value) in [
        (CRATE_BINDING_ID_ENV_V1, actual.crate_binding.as_str()),
        (
            CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
            actual.cargo_observation.as_str(),
        ),
        ("CARGO_PKG_NAME", PACKAGE),
        ("CARGO_PKG_VERSION", "0.0.0"),
        ("CARGO_CRATE_NAME", CRATE_NAME),
    ] {
        assert_eq!(std::env::var(key).unwrap(), value);
    }
    assert_eq!(
        PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap()),
        fixture()
    );
    super::super::require_canonical_overflow_checks_v1(&actual.args).unwrap();
    let mut callbacks = SingleReadCallbacks {
        calls: 0,
        result: None,
    };
    timely(started.elapsed(), 300).unwrap();
    rustc_driver::run_compiler(&actual.args, &mut callbacks);
    assert_eq!(callbacks.calls, 1);
    let observed = callbacks
        .result
        .expect("actual single-read callback absent");
    publish_json(
        &directory,
        &format!("{case}.observed.json"),
        &json!({"case":case,"observation":observed,"accepted":false,
            "acceptance_requires_completed_parent":true}),
    );
    accept(&case, &observed).unwrap();
    assert!(
        fs::read_dir(directory.join("analysis-output"))
            .unwrap()
            .next()
            .is_none()
    );
    assert_eq!(inputs::derive_record(&directory, feature), actual);
    let frame = serde_json::to_string(&json!({
        "schema":"fe2o3-test-bf16-single-strided-read-observation-v1",
        "case":case,"feature":feature,"invocation":actual,"observation":observed,
        "actual_rustc_callbacks":callbacks.calls,"source_and_dependencies_unchanged":true,
        "normal_qualified":false,"numerical_cpu_qualified":false,"hardware_observed":false,
        "native_execution_attempted":false,"grants_artifact_or_launch_authority":false
    }))
    .unwrap();
    assert!(frame.len() <= 256 * 1024);
    timely(started.elapsed(), 300).unwrap();
    println!("\n{PREFIX}{frame}");
}

#[test]
#[ignore = "three genuine source sessions: single read, empty-profile refusal, wrong launch"]
fn actual_single_strided_read_source_ladder() {
    let started = std::time::Instant::now();
    let directory = create_output(&PathBuf::from(
        std::env::var_os(OUTPUT_ENV).expect("fresh single-read output"),
    ));
    let sources = inputs::current_sources();
    let rustc_path =
        PathBuf::from(std::env::var_os("RUSTC").expect("absolute pinned-nightly RUSTC"));
    assert!(rustc_path.is_absolute());
    let mut rustc = Command::new(rustc_path);
    let bytes = checked(
        sanitized(&mut rustc).args(["--print", "sysroot"]),
        &directory,
        "sysroot",
        None,
    );
    let sysroot = PathBuf::from(std::str::from_utf8(&bytes).unwrap().trim_end());
    assert!(
        sysroot
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("nightly-2026-04-03-")
    );
    timely(started.elapsed(), 1200).unwrap();
    let mut metadata = Command::new(sysroot.join("bin/cargo"));
    let bytes = checked(
        sanitized(&mut metadata)
            .args([
                "metadata",
                "--locked",
                "--offline",
                "--no-deps",
                "--format-version=1",
                "--manifest-path",
            ])
            .arg(fixture().join("Cargo.toml")),
        &directory,
        "metadata",
        None,
    );
    let metadata: Value = serde_json::from_slice(&bytes).unwrap();
    for feature in FEATURES {
        inputs::feature_in_metadata(&metadata, feature).unwrap();
    }
    timely(started.elapsed(), 1200).unwrap();
    let dependency_target = directory.join("dependencies");
    let mut cargo = Command::new(sysroot.join("bin/cargo"));
    checked(sanitized(&mut cargo).current_dir(repository())
        .args(["check","--release","--locked","--offline","-Zbuild-std=core","-p","fe2o3-device",
            "--target","amdgcn-amd-amdhsa","--message-format=json","--manifest-path"])
        .arg(fixture().join("Cargo.toml")).arg("--target-dir").arg(&dependency_target)
        .env("RUSTC",sysroot.join("bin/rustc"))
        .env("CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS",
            "-Zalways-encode-mir -Ctarget-cpu=gfx942 -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32 -Coverflow-checks=on"),
        &directory,"dependencies",Some(&dependency_target));
    timely(started.elapsed(), 1200).unwrap();
    fs::create_dir(directory.join("analysis-output")).unwrap();
    let (dependencies, files) = inputs::dependency_snapshot(&directory);
    publish_json(&directory, "dependency-files.json", &files);
    drop(files);
    assert_eq!(inputs::current_sources(), sources);
    let mut observations = Vec::new();
    for case in CASES {
        let child_started = std::time::Instant::now();
        let feature = feature_for_case(case).unwrap();
        let record = inputs::derive_record(&directory, feature);
        assert_eq!(record.sources, sources);
        assert_eq!(record.dependencies, dependencies);
        publish_json(&directory, &format!("{case}.invocation.json"), &record);
        let mut child = Command::new(std::env::current_exe().unwrap());
        let stdout = checked(
            sanitized(&mut child)
                .current_dir(repository())
                .args(["--exact", CHILD, "--ignored", "--nocapture"])
                .env(CHILD_ENV, &directory)
                .env(CASE_ENV, case)
                .env(CRATE_BINDING_ID_ENV_V1, &record.crate_binding)
                .env(
                    CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
                    &record.cargo_observation,
                )
                .env("CARGO_MANIFEST_DIR", fixture())
                .env("CARGO_PKG_NAME", PACKAGE)
                .env("CARGO_PKG_VERSION", "0.0.0")
                .env("CARGO_CRATE_NAME", CRATE_NAME),
            &directory,
            case,
            None,
        );
        timely(child_started.elapsed(), 300).unwrap();
        let stdout = std::str::from_utf8(&stdout).unwrap();
        let frame = single_observation_frame(stdout).unwrap();
        assert_eq!(
            stdout
                .lines()
                .filter(|line| line.starts_with("test result: ok. 1 passed; 0 failed; 0 ignored;"))
                .count(),
            1
        );
        let observed: Value = serde_json::from_str(frame).unwrap();
        assert_eq!(
            observed["schema"],
            "fe2o3-test-bf16-single-strided-read-observation-v1"
        );
        assert_eq!(observed["case"], case);
        assert_eq!(observed["feature"], feature);
        assert_eq!(
            observed["invocation"],
            serde_json::to_value(&record).unwrap()
        );
        assert_eq!(observed["actual_rustc_callbacks"], 1);
        assert_eq!(observed["source_and_dependencies_unchanged"], true);
        assert!(flags_false(&observed));
        accept(case, &observed["observation"]).unwrap();
        let stderr = read_bounded(&directory.join(format!("{case}.stderr")), 1024 * 1024).unwrap();
        accept_markers(case, std::str::from_utf8(&stderr).unwrap()).unwrap();
        let raw: Value = serde_json::from_slice(
            &read_bounded(&directory.join(format!("{case}.observed.json")), 256 * 1024).unwrap(),
        )
        .unwrap();
        assert_eq!(raw["case"], case);
        assert_eq!(raw["observation"], observed["observation"]);
        assert_eq!(raw["accepted"], false);
        assert_eq!(inputs::derive_record(&directory, feature), record);
        timely(child_started.elapsed(), 300).unwrap();
        timely(started.elapsed(), 1200).unwrap();
        publish_json(&directory, &format!("{case}.accepted.json"), &observed);
        observations.push(observed);
    }
    assert_eq!(observations.len(), 3);
    assert_eq!(inputs::current_sources(), sources);
    assert_eq!(inputs::dependency_snapshot(&directory).0, dependencies);
    assert!(
        fs::read_dir(directory.join("analysis-output"))
            .unwrap()
            .next()
            .is_none()
    );
    let report = json!({"schema":"fe2o3-test-bf16-single-strided-read-ladder-v1",
        "actual_rustc_sessions":3,"expected_positive_sources":1,
        "expected_empty_profile_refusals":1,"expected_launch_refusals":1,
        "expected_completed_canonical_callback_modes":3,
        "observations":observations,"source_files":sources,"dependency_snapshot":dependencies,
        "fresh_dependency_builds":1,"normal_qualified":false,"numerical_cpu_qualified":false,
        "hardware_observed":false,"native_execution_attempted":false,
        "grants_artifact_or_launch_authority":false,"source_publication_attempted":false,
        "cleanup_scope":"reused bounded direct-child/process-group helper, not whole-family supervision",
        "acceptance":"completed successful parent test required; JSON is historical only"});
    publish_json(&directory, "observation.json", &report);
    timely(started.elapsed(), 1200).unwrap();
}

#[cfg(test)]
mod controls {
    use super::*;
    // Parser fixtures only. These strings are not actual child evidence.
    fn marker_rows() -> String {
        ["Compare", "CallbackError", "CallbackPanic"].into_iter().map(|mode| format!(
            "{MARKER}mode={mode} locals=7 blocks=8 effect_block=3 read_views=1 projected_rows=8 prefix_operations=6 next_value=5 next_argument=1 single_constant_profile=true nonempty_read_coverage=true retained_legacy_oracle=true unchanged_legacy_donor_invoked=false same_source_ledger_counter=true retained_postflight=true postflight_rows_compared=true before_writer_comparison=true old_completion_refused=true foreign_counter_refused=true reentry_terminal=true graph_started=false invocation_started=false private_component=true ordinary_route=false\n"
        )).collect()
    }
    #[test]
    fn single_read_source_selector_rejects_combined_or_unknown_features() {
        for feature in FEATURES {
            assert!(checked_feature(feature).is_ok());
        }
        for bad in [
            "",
            "swap01",
            "single-strided-read,identity",
            "single-strided-read wrong-launch",
        ] {
            assert!(checked_feature(bad).is_err());
            assert!(feature_for_case(bad).is_err());
        }
        assert_eq!(feature_for_case("identity-empty"), Ok("identity"));
    }
    #[test]
    fn single_read_marker_requires_exact_three_modes_and_no_refusal_markers() {
        let good = marker_rows();
        assert!(accept_markers("single-strided-read", &good).is_ok());
        for case in ["identity-empty", "wrong-launch"] {
            assert!(accept_markers(case, "").is_ok());
            assert!(accept_markers(case, &good).is_err());
        }
        assert!(accept_markers("single-strided-read", "").is_err());
        assert!(accept_markers("single-strided-read", &format!("{good}{good}")).is_err());
        assert!(
            accept_markers(
                "single-strided-read",
                &good.replace("mode=CallbackError", "mode=Compare")
            )
            .is_err()
        );
        assert!(accept_markers("unknown", &good).is_err());
    }
    #[test]
    fn single_read_marker_rejects_noncanonical_counts_and_missing_owner_claims() {
        let good = marker_rows();
        for (before, after) in [
            ("locals=7", "locals=07"),
            ("locals=7", "locals=4097"),
            ("locals=7", "locals=18446744073709551616"),
            ("blocks=8", "blocks=0"),
            ("effect_block=3", "effect_block=8"),
            ("read_views=1", "read_views=0"),
            ("projected_rows=8", "projected_rows=7"),
            ("next_value=5", "next_value=4"),
            ("next_argument=1", "next_argument=2"),
            ("ordinary_route=false", "ordinary_route=true"),
            (
                "same_source_ledger_counter=true",
                "same_source_ledger_counter=false",
            ),
            (
                "postflight_rows_compared=true",
                "postflight_rows_compared=false",
            ),
            (
                "unchanged_legacy_donor_invoked=false",
                "unchanged_legacy_donor_invoked=true",
            ),
            ("graph_started=false", "graph_started=true"),
            ("private_component=true ", ""),
            ("ordinary_route=false", "ordinary_route=false extra=true"),
        ] {
            assert!(accept_markers("single-strided-read", &good.replace(before, after)).is_err());
        }
        assert!(
            accept_markers(
                "single-strided-read",
                &good.replacen("locals=7", "locals=6", 1)
            )
            .is_err()
        );
    }

    #[test]
    fn single_read_marker_rejects_every_extra_bare_prefix_in_positive_output() {
        let good = marker_rows();
        let bare = MARKER.trim_end_matches(' ');
        for extra in [
            format!("wrapped {MARKER}ignored\n"),
            format!("prefix:{bare}\n"),
            format!("{bare}\n"),
            format!("{MARKER}mode=Compare\n"),
            format!("{MARKER}mode=Compare\r\n"),
            format!("{MARKER}mode=Compare"),
            format!("payload contains {bare} embedded\n"),
        ] {
            assert!(accept_markers("single-strided-read", &format!("{good}{extra}")).is_err());
            assert!(accept_markers("single-strided-read", &format!("{extra}{good}")).is_err());
        }
        assert!(accept_markers("single-strided-read", &good.replace('\n', "\r\n")).is_err());
        assert!(accept_markers("single-strided-read", good.trim_end_matches('\n')).is_err());
        assert!(
            accept_markers(
                "single-strided-read",
                &good.replacen("mode=Compare", "mode=Com\rpare", 1)
            )
            .is_err()
        );
    }
    #[test]
    fn single_read_both_negative_cases_reject_wrapped_embedded_and_cr_markers() {
        let bare = MARKER.trim_end_matches(' ');
        for case in ["identity-empty", "wrong-launch"] {
            assert!(accept_markers(case, "ordinary diagnostic\n").is_ok());
            for malformed in [
                format!("wrapped {MARKER}ignored\n"),
                format!("some diagnostic contains {bare} text\n"),
                format!("{bare}\n"),
                format!("{MARKER}mode=Compare\n"),
                format!("{MARKER}mode=Compare\r\n"),
                format!("{MARKER}mode=Compare\r"),
                format!("{MARKER}mode=Compare"),
                format!("leading\r{MARKER}mode=Compare\n"),
            ] {
                assert!(accept_markers(case, &malformed).is_err());
            }
        }
    }
    #[test]
    fn single_read_json_frame_requires_exactly_one_lf_record_and_total_tag_occurrence() {
        let good = format!("test harness text\n{PREFIX}{{}}\nsummary\n");
        assert_eq!(single_observation_frame(&good), Ok("{}"));
        let bare = PREFIX.trim_end_matches(' ');
        for extra in [
            format!("wrapped {PREFIX}{{}}\n"),
            format!("payload contains {bare} embedded\n"),
            format!("{bare}\n"),
            format!("{PREFIX}{{}}\n"),
            format!("{PREFIX}{{}}\r\n"),
            format!("{PREFIX}{{}}"),
        ] {
            assert!(single_observation_frame(&format!("{good}{extra}")).is_err());
            assert!(single_observation_frame(&format!("{extra}{good}")).is_err());
        }
        for malformed in [
            String::new(),
            format!("{PREFIX}{{}}\r\n"),
            format!("{PREFIX}{{}}\r"),
            format!("{PREFIX}{{}}"),
            format!("{PREFIX}{{\"embedded\":\"{bare}\"}}\n"),
            format!("wrapped {PREFIX}{{}}\n"),
            format!("{PREFIX}{{\r}}\n"),
        ] {
            assert!(single_observation_frame(&malformed).is_err());
        }
    }
}
