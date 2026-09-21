//! Typed test transport only. Records cannot construct a source or proof owner.
use super::*;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) enum Case {
    DirectMotion,
    DirectNoop,
    UnitLocalMotion,
    UnitLocalNoop,
}
impl Case {
    pub(super) const ALL: [Self; 4] = [
        Self::DirectMotion,
        Self::DirectNoop,
        Self::UnitLocalMotion,
        Self::UnitLocalNoop,
    ];
    pub(super) fn feature(self) -> &'static str {
        match self {
            Self::DirectMotion => "licm-native",
            Self::DirectNoop => "licm-native-noop",
            Self::UnitLocalMotion => "licm-native-unitlocal",
            Self::UnitLocalNoop => "licm-native-unitlocal-noop",
        }
    }
    pub(super) fn motion(self) -> bool {
        matches!(self, Self::DirectMotion | Self::UnitLocalMotion)
    }
    pub(super) fn unit_local(self) -> bool {
        matches!(self, Self::UnitLocalMotion | Self::UnitLocalNoop)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) enum Purpose {
    Baseline,
    Measure,
    Exact,
    WorkShort,
    StorageShort,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Subject {
    pub case: Case,
    pub target: String,
    pub args_sha256: [u8; 32],
    pub cfg: Vec<String>,
    pub environment_sha256: [u8; 32],
    pub cwd: PathBuf,
    pub rustc_sha256: [u8; 32],
    pub test_binary_sha256: [u8; 32],
    pub sources: Vec<(String, [u8; 32])>,
}
impl Subject {
    pub(super) fn identity(&self) -> [u8; 32] {
        digest(&serde_json::to_vec(self).unwrap())
    }
    pub(super) fn check(&self, args: &[String]) -> Result<(), String> {
        profile(&self.target)?;
        if self.args_sha256 != digest(&serde_json::to_vec(args).map_err(|e| e.to_string())?)
            || self.sources != source_stamps()
            || self.cwd != env::current_dir().map_err(|e| e.to_string())?
            || self.environment_sha256 != environment_digest(env::vars_os().collect())?
            || self.test_binary_sha256
                != digest(
                    &std::fs::read(env::current_exe().map_err(|e| e.to_string())?)
                        .map_err(|e| e.to_string())?,
                )
            || self.rustc_sha256
                != digest(
                    &std::fs::read(args.first().ok_or("missing captured compiler")?)
                        .map_err(|e| e.to_string())?,
                )
            || !self
                .cfg
                .contains(&format!("feature=\"{}\"", self.case.feature()))
        {
            return Err("exact captured source/argv/environment/tool/cwd identity".into());
        }
        retention_flags(args)
    }
}
fn retention_flags(args: &[String]) -> Result<(), String> {
    if args
        .iter()
        .filter(|a| a.starts_with("-Zmir-opt-level"))
        .map(String::as_str)
        .collect::<Vec<_>>()
        != ["-Zmir-opt-level=0"]
        || args
            .iter()
            .filter(|a| a.starts_with("-Zinline-mir"))
            .map(String::as_str)
            .collect::<Vec<_>>()
            != ["-Zinline-mir=no"]
        || args.windows(2).any(|pair| {
            pair[0] == "-Z"
                && (pair[1].starts_with("mir-opt-level") || pair[1].starts_with("inline-mir"))
        })
    {
        return Err("exact canonical MIR-retention flags".into());
    }
    require_canonical_overflow_checks_v1(args)
}

// These are only the existing wrapper/child transport and cleared jobserver keys.
// In particular, managed crate-binding and build-observation entries are retained.
const TRANSPORT: &[&str] = &[
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "FE2O3_TEST_P4_CAPTURE_ARGS",
    "FE2O3_TEST_P4_CAPTURE_ENV",
    "FE2O3_TEST_P4_CAPTURE_MANIFEST",
    "FE2O3_TEST_P4_CAPTURE_CRATE",
    REQUEST,
    CHILD_ARGS,
    CHILD_RESULT,
    CHILD_PROOF_PROBE,
    "CARGO_MAKEFLAGS",
    "MAKEFLAGS",
    "MFLAGS",
];
pub(super) fn environment_digest(
    values: Vec<(std::ffi::OsString, std::ffi::OsString)>,
) -> Result<[u8; 32], String> {
    let mut rows = values
        .into_iter()
        .map(|(key, value)| (key.into_encoded_bytes(), value.into_encoded_bytes()))
        .collect::<Vec<_>>();
    rows.sort();
    if rows.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err("duplicate captured environment key".into());
    }
    rows.retain(|(key, _)| !TRANSPORT.iter().any(|name| key == name.as_bytes()));
    Ok(digest(
        &serde_json::to_vec(&rows).map_err(|e| e.to_string())?,
    ))
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Measurement {
    pub subject: [u8; 32],
    pub work: usize,
    pub peak: usize,
    pub floor: usize,
    pub retained: usize,
    pub llvm_bytes: usize,
    pub llvm_sha256: [u8; 32],
    pub original: [u8; 32],
    pub historical_p8: [u8; 32],
    pub promoted: [u8; 32],
    pub input: [u8; 32],
    pub output: [u8; 32],
    pub source: [u8; 32],
    pub preflight: [u8; 32],
    pub ranked: [u8; 32],
    pub execution: [u8; 32],
    pub hoists: usize,
    pub deleted_helpers: [usize; 2],
}
impl Measurement {
    pub(super) fn check(&self, subject: &Subject) -> Result<(), String> {
        profile(&subject.target)?;
        if self.subject != subject.identity()
            || self.work <= 7
            || self.peak <= self.floor
            || self.floor <= 37
            || self.retained == 0
            || self.llvm_bytes == 0
            || self.source != self.preflight
            || [
                self.llvm_sha256,
                self.original,
                self.historical_p8,
                self.promoted,
                self.input,
                self.output,
                self.source,
                self.ranked,
                self.execution,
            ]
            .contains(&[0; 32])
            || if subject.case.motion() {
                self.hoists < 2
            } else {
                self.hoists != 0
            }
            || (self.input != self.output) != subject.case.motion()
            || self.deleted_helpers
                != if subject.case.unit_local() {
                    [1, 1]
                } else {
                    [0, 0]
                }
        {
            return Err(
                "genuine source/native measurement and exact motion/erasure contract".into(),
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RequestRecord {
    pub ordinal: usize,
    pub purpose: Purpose,
    pub subject: Subject,
    #[serde(deserialize_with = "required_option")]
    pub baseline: Option<Measurement>,
}
impl RequestRecord {
    pub(super) fn check(&self, args: &[String]) -> Result<(), String> {
        self.subject.check(args)?;
        if self.ordinal == 0 {
            return Err("one-based child ordinal".into());
        }
        match (&self.baseline, self.purpose) {
            (None, Purpose::Baseline | Purpose::Measure) => Ok(()),
            (Some(value), Purpose::Exact | Purpose::WorkShort | Purpose::StorageShort) => {
                value.check(&self.subject)
            }
            _ => Err("exact resource measurement association".into()),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) enum Denial {
    Work {
        actual: usize,
        limit: usize,
    },
    StorageObservation {
        actual: usize,
        limit: usize,
        nested_error: String,
    },
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Report {
    pub request: RequestRecord,
    pub callback_count: usize,
    #[serde(deserialize_with = "required_option")]
    pub measurement: Option<Measurement>,
    #[serde(deserialize_with = "required_option")]
    pub denial: Option<Denial>,
    pub accepted_work: usize,
    pub accepted_peak: usize,
    pub final_storage: usize,
    #[serde(deserialize_with = "required_option")]
    pub failed_work: Option<usize>,
    #[serde(deserialize_with = "required_option")]
    pub failed_storage: Option<usize>,
    pub replay_work: usize,
    pub hostile_replays: usize,
    #[serde(deserialize_with = "required_option")]
    pub simulation: Option<sim::Observation>,
}
fn required_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}
impl Report {
    pub(super) fn check(&self, request: &RequestRecord) -> Result<(), String> {
        if &self.request != request
            || request.ordinal == 0
            || self.callback_count != 1
            || self.final_storage <= 37
        {
            return Err("child report identity/callback/floor".into());
        }
        match request.purpose {
            Purpose::Baseline | Purpose::Measure | Purpose::Exact => {
                let value = self
                    .measurement
                    .as_ref()
                    .ok_or("missing genuine owner measurement")?;
                value.check(&request.subject)?;
                if self.denial.is_some()
                    || self.accepted_work != value.work
                    || self.accepted_peak != value.peak
                    || self.final_storage != value.floor
                {
                    return Err("successful constructor exact resource measurement".into());
                }
                if request.purpose == Purpose::Exact {
                    if request.baseline.as_ref() != Some(value)
                        || self.failed_work != Some(value.work + 1)
                        || self.failed_storage != Some(value.peak + 1)
                    {
                        return Err("exact-bound success preserves seeded first denials".into());
                    }
                } else if self.failed_work.is_some() || self.failed_storage.is_some() {
                    return Err("unbounded measurement unexpectedly denied".into());
                }
                if request.purpose == Purpose::Baseline {
                    self.simulation
                        .as_ref()
                        .ok_or("missing actual three-graph SIM")?
                        .check()?;
                    if self.replay_work == 0 || self.hostile_replays != 5 {
                        return Err(
                            "complete replay, four restored faults and one consuming fault".into(),
                        );
                    }
                } else if self.simulation.is_some()
                    || self.replay_work != 0
                    || self.hostile_replays != 0
                {
                    return Err("constructor budget must exclude later replay/SIM".into());
                }
            }
            Purpose::WorkShort | Purpose::StorageShort => {
                let value = request
                    .baseline
                    .as_ref()
                    .ok_or("missing resource baseline")?;
                if self.measurement.is_some()
                    || self.simulation.is_some()
                    || self.replay_work != 0
                    || self.hostile_replays != 0
                    || self.final_storage != value.floor
                {
                    return Err("failed constructor leaked owner/replay/floor".into());
                }
                if request.purpose == Purpose::WorkShort {
                    if self.denial
                        != Some(Denial::Work {
                            actual: value.work,
                            limit: value.work - 1,
                        })
                        || self.failed_work != Some(value.work)
                        || self.failed_storage.is_some()
                        || self.accepted_work
                            != value
                                .work
                                .checked_sub(
                                    value
                                        .llvm_bytes
                                        .checked_mul(2)
                                        .and_then(|n| n.checked_add(1))
                                        .ok_or("terminal comparison size")?,
                                )
                                .ok_or("terminal comparison work")?
                        || self.accepted_peak != value.peak
                    {
                        return Err("exact final LLVM-comparison work denial".into());
                    }
                } else if !matches!(&self.denial, Some(Denial::StorageObservation {actual, limit, nested_error})
                    if *actual == value.peak && *limit == value.peak - 1 && !nested_error.is_empty() && nested_error.len() <= 8192)
                    || self.failed_work.is_some()
                    || self.failed_storage != Some(value.peak)
                    || self.accepted_work > value.work
                    || self.accepted_peak >= value.peak
                {
                    return Err(
                        "diagnostic storage first-denial/accepted-prefix observation".into(),
                    );
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Completed {
    pub ordinal: usize,
    pub purpose: Purpose,
    pub target: String,
    pub case: Case,
    pub subject: [u8; 32],
    pub observation_sha256: [u8; 32],
}
pub(super) fn write_observation(
    output: &mut impl std::io::Write,
    request: &RequestRecord,
    report: &Report,
    stdout: &[u8],
    stderr: &[u8],
) -> Result<Completed, String> {
    if &report.request != request {
        return Err("framing request/report association".into());
    }
    let record = serde_json::to_vec(&serde_json::json!({
        "child_test": CHILD, "request": request, "report": report,
        "stdout_bytes": stdout.len(), "stdout_sha256": digest(stdout),
        "stderr_bytes": stderr.len(), "stderr_sha256": digest(stderr),
        "qualification": false,
    }))
    .map_err(|e| e.to_string())?;
    for part in [
        b"FE2O3_LICM_SOURCE_OBSERVATION ".as_slice(),
        &record,
        b"\nFE2O3_LICM_STDOUT_BEGIN\n",
        stdout,
        b"\nFE2O3_LICM_STDERR_BEGIN\n",
        stderr,
        b"\nFE2O3_LICM_STREAMS_END\n",
    ] {
        output.write_all(part).map_err(|e| e.to_string())?;
    }
    output.flush().map_err(|e| e.to_string())?;
    Ok(Completed {
        ordinal: request.ordinal,
        purpose: request.purpose,
        target: request.subject.target.clone(),
        case: request.subject.case,
        subject: request.subject.identity(),
        observation_sha256: digest(&record),
    })
}
pub(super) fn write_completion(
    output: &mut impl std::io::Write,
    resource: bool,
    rows: &[Completed],
) -> Result<(), String> {
    let purposes: &[Purpose] = if resource {
        &[
            Purpose::Measure,
            Purpose::Exact,
            Purpose::WorkShort,
            Purpose::StorageShort,
        ]
    } else {
        &[Purpose::Baseline]
    };
    let expected = ["gfx942", "gfx950"]
        .into_iter()
        .flat_map(|target| {
            Case::ALL
                .into_iter()
                .flat_map(move |case| purposes.iter().map(move |purpose| (target, case, *purpose)))
        })
        .collect::<Vec<_>>();
    if rows.len() != expected.len()
        || rows
            .iter()
            .zip(expected)
            .enumerate()
            .any(|(i, (row, (target, case, purpose)))| {
                row.ordinal != i + 1
                    || row.target != target
                    || row.case != case
                    || row.purpose != purpose
                    || row.subject == [0; 32]
                    || row.observation_sha256 == [0; 32]
            })
    {
        return Err("exact ordered ordinary-source parent roster".into());
    }
    if resource
        && rows
            .chunks_exact(4)
            .any(|group| group.iter().any(|row| row.subject != group[0].subject))
    {
        return Err(
            "each resource measurement and attempts need identical captured subjects".into(),
        );
    }
    let record = serde_json::to_vec(&serde_json::json!({"child_test":CHILD, "completed":rows.len(), "observations":rows,
        "storage_phase_qualified":false, "unequal_owner_pairs_implemented":false, "qualification":false})).map_err(|e|e.to_string())?;
    output
        .write_all(b"FE2O3_LICM_SOURCE_COMPLETE ")
        .map_err(|e| e.to_string())?;
    output.write_all(&record).map_err(|e| e.to_string())?;
    output.write_all(b"\n").map_err(|e| e.to_string())?;
    output.flush().map_err(|e| e.to_string())
}

#[test]
fn licm_source_protocol_keeps_semantic_environment_and_refuses_duplicates() {
    let env = |name: &str, value: &str| vec![(name.into(), value.into())];
    assert_eq!(
        environment_digest(env("RUSTC_WRAPPER", "a")).unwrap(),
        environment_digest(env("RUSTC_WRAPPER", "b")).unwrap()
    );
    for name in [
        CRATE_BINDING_ID_ENV_V1,
        CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
        "CARGO_MANIFEST_DIR",
        "RUSTFLAGS",
        "UNRECOGNIZED",
    ] {
        assert_ne!(
            environment_digest(env(name, "a")).unwrap(),
            environment_digest(env(name, "b")).unwrap()
        );
    }
    assert!(
        environment_digest(vec![
            ("RUSTC_WRAPPER".into(), "a".into()),
            ("RUSTC_WRAPPER".into(), "b".into())
        ])
        .is_err()
    );
}

#[test]
fn licm_source_protocol_completion_rejects_missing_duplicate_reordered_and_wrong_subjects() {
    let mut rows = Vec::new();
    for target in ["gfx942", "gfx950"] {
        for case in Case::ALL {
            let subject = digest(format!("inert protocol-only {target} {case:?}").as_bytes());
            for purpose in [
                Purpose::Measure,
                Purpose::Exact,
                Purpose::WorkShort,
                Purpose::StorageShort,
            ] {
                rows.push(Completed {
                    ordinal: rows.len() + 1,
                    purpose,
                    target: target.into(),
                    case,
                    subject,
                    observation_sha256: [1; 32],
                });
            }
        }
    }
    let mut bytes = Vec::new();
    write_completion(&mut bytes, true, &rows).unwrap();
    assert!(!bytes.is_empty());
    for index in 0..rows.len() {
        let mut missing = rows.clone();
        missing.remove(index);
        assert!(write_completion(&mut Vec::new(), true, &missing).is_err());
        let mut wrong = rows.clone();
        wrong[index].subject[0] ^= 1;
        assert!(write_completion(&mut Vec::new(), true, &wrong).is_err());
    }
    rows.swap(0, 1);
    assert!(write_completion(&mut Vec::new(), true, &rows).is_err());
    rows[1] = rows[0].clone();
    assert!(write_completion(&mut Vec::new(), true, &rows).is_err());
}

// An inert protocol-shaped record, never accepted by Subject::check: no actual
// compiler argv, source stamps, environment or source owner exists for it.
fn inert_record() -> Report {
    let subject = Subject {
        case: Case::DirectMotion,
        target: "gfx942".into(),
        args_sha256: [1; 32],
        cfg: vec![],
        environment_sha256: [2; 32],
        cwd: PathBuf::from("/inert-protocol-only"),
        rustc_sha256: [3; 32],
        test_binary_sha256: [4; 32],
        sources: vec![],
    };
    let measurement = Measurement {
        subject: subject.identity(),
        work: 100,
        peak: 300,
        floor: 100,
        retained: 150,
        llvm_bytes: 20,
        llvm_sha256: [1; 32],
        original: [2; 32],
        historical_p8: [3; 32],
        promoted: [4; 32],
        input: [5; 32],
        output: [6; 32],
        source: [7; 32],
        preflight: [7; 32],
        ranked: [8; 32],
        execution: [9; 32],
        hoists: 2,
        deleted_helpers: [0, 0],
    };
    Report {
        request: RequestRecord {
            ordinal: 1,
            purpose: Purpose::Measure,
            subject,
            baseline: None,
        },
        callback_count: 1,
        measurement: Some(measurement),
        denial: None,
        accepted_work: 100,
        accepted_peak: 300,
        final_storage: 100,
        failed_work: None,
        failed_storage: None,
        replay_work: 0,
        hostile_replays: 0,
        simulation: None,
    }
}

#[test]
fn licm_source_protocol_rejects_missing_unknown_duplicate_and_wrong_typed_fields() {
    let report = inert_record();
    let mut value = serde_json::to_value(&report).unwrap();
    for name in [
        "request",
        "callback_count",
        "measurement",
        "denial",
        "accepted_work",
        "accepted_peak",
        "final_storage",
        "failed_work",
        "failed_storage",
        "replay_work",
        "hostile_replays",
        "simulation",
    ] {
        let mut missing = value.clone();
        missing.as_object_mut().unwrap().remove(name);
        // Optional values need an explicit field just like every other record.
        if serde_json::from_value::<Report>(missing).is_ok() {
            panic!("required protocol field omitted: {name}");
        }
    }
    value
        .as_object_mut()
        .unwrap()
        .insert("source_proof_passed".into(), serde_json::json!(true));
    assert!(serde_json::from_value::<Report>(value).is_err());
    let json = serde_json::to_string(&report).unwrap();
    let duplicate = format!("{{\"callback_count\":1,{}", &json[1..]);
    assert!(serde_json::from_str::<Report>(&duplicate).is_err());
    let mut wrong = serde_json::to_value(&report).unwrap();
    wrong["callback_count"] = serde_json::json!("1");
    assert!(serde_json::from_value::<Report>(wrong).is_err());
}

#[test]
fn licm_source_protocol_refuses_callback_motion_identity_and_resource_reclassification() {
    let report = inert_record();
    report.check(&report.request).unwrap();
    for mutation in 0..9 {
        let mut wrong = report.clone();
        match mutation {
            0 => wrong.callback_count = 0,
            1 => wrong.callback_count = 2,
            2 => wrong.measurement.as_mut().unwrap().hoists = 0,
            3 => wrong.measurement.as_mut().unwrap().preflight = [0; 32],
            4 => wrong.measurement.as_mut().unwrap().subject = [0; 32],
            5 => wrong.accepted_work += 1,
            6 => wrong.accepted_peak += 1,
            7 => wrong.final_storage += 1,
            8 => {
                wrong.denial = Some(Denial::Work {
                    actual: 100,
                    limit: 99,
                })
            }
            _ => unreachable!(),
        }
        assert!(wrong.check(&report.request).is_err());
    }
    let mut noop = report.clone();
    noop.request.subject.case = Case::DirectNoop;
    let subject = noop.request.subject.identity();
    let measurement = noop.measurement.as_mut().unwrap();
    measurement.subject = subject;
    measurement.hoists = 0;
    measurement.output = measurement.input;
    noop.check(&noop.request).unwrap();
    noop.measurement.as_mut().unwrap().hoists = 1;
    assert!(noop.check(&noop.request).is_err());
}

#[test]
fn licm_source_protocol_full_work_denial_and_storage_diagnostic_are_not_interchangeable() {
    let mut report = inert_record();
    report.request.purpose = Purpose::WorkShort;
    report.request.baseline = report.measurement.take();
    report.denial = Some(Denial::Work {
        actual: 100,
        limit: 99,
    });
    report.accepted_work = 59;
    report.failed_work = Some(100);
    report.check(&report.request).unwrap();
    report.accepted_work = 99;
    assert!(report.check(&report.request).is_err());
    report.accepted_work = 59;
    report.denial = Some(Denial::StorageObservation {
        actual: 300,
        limit: 299,
        nested_error: "inert, not compiler evidence".into(),
    });
    assert!(report.check(&report.request).is_err());
    report.request.purpose = Purpose::StorageShort;
    report.failed_work = None;
    report.failed_storage = Some(300);
    report.accepted_peak = 280;
    report.check(&report.request).unwrap();
    report.failed_storage = Some(299);
    assert!(report.check(&report.request).is_err());
}

#[test]
fn licm_source_protocol_framing_preserves_non_utf8_streams_and_rejects_identity_or_write_failure() {
    let report = inert_record();
    let stdout = b"raw\xffstdout";
    let stderr = b"raw\xfestderr";
    let mut output = Vec::new();
    let done = write_observation(&mut output, &report.request, &report, stdout, stderr).unwrap();
    let begin = b"\nFE2O3_LICM_STDOUT_BEGIN\n";
    let middle = b"\nFE2O3_LICM_STDERR_BEGIN\n";
    let pos = output
        .windows(begin.len())
        .position(|v| v == begin)
        .unwrap();
    assert_eq!(
        &output[pos + begin.len()..pos + begin.len() + stdout.len()],
        stdout
    );
    let rest = &output[pos + begin.len() + stdout.len()..];
    assert!(rest.starts_with(middle));
    assert_eq!(&rest[middle.len()..middle.len() + stderr.len()], stderr);
    let prefix = b"FE2O3_LICM_SOURCE_OBSERVATION ";
    assert!(output.starts_with(prefix));
    let record = &output[prefix.len()..pos];
    assert_eq!(digest(record), done.observation_sha256);
    let value: serde_json::Value = serde_json::from_slice(record).unwrap();
    assert_eq!(
        value["stdout_sha256"],
        serde_json::to_value(digest(stdout)).unwrap()
    );
    assert_eq!(
        value["stderr_sha256"],
        serde_json::to_value(digest(stderr)).unwrap()
    );
    assert_eq!(value["qualification"], serde_json::json!(false));
    let mut wrong = report.request.clone();
    wrong.ordinal += 1;
    assert!(write_observation(&mut Vec::new(), &wrong, &report, stdout, stderr).is_err());
    struct Broken;
    impl std::io::Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("inert formatting failure"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    assert!(write_observation(&mut Broken, &report.request, &report, stdout, stderr).is_err());
}

#[test]
fn licm_source_protocol_requires_exact_retention_flags_and_closed_case_target_rosters() {
    let args = [
        "rustc",
        "-Coverflow-checks=on",
        "-Zmir-opt-level=0",
        "-Zinline-mir=no",
    ]
    .map(str::to_owned)
    .to_vec();
    retention_flags(&args).unwrap();
    for index in 1..args.len() {
        let mut missing = args.clone();
        missing.remove(index);
        assert!(retention_flags(&missing).is_err());
        let mut duplicate = args.clone();
        duplicate.push(args[index].clone());
        assert!(retention_flags(&duplicate).is_err());
    }
    for replacement in ["-Zmir-opt-level=1", "-Zmir-opt-level=3"] {
        let mut wrong = args.clone();
        wrong[2] = replacement.into();
        assert!(retention_flags(&wrong).is_err());
    }
    for pair in [
        ["-Z", "mir-opt-level=3"],
        ["-Z", "inline-mir=yes"],
        ["-C", "overflow-checks=off"],
    ] {
        let mut wrong = args.clone();
        wrong.extend(pair.map(str::to_owned));
        assert!(retention_flags(&wrong).is_err());
    }
    assert_eq!(
        Case::ALL.map(Case::feature),
        [
            "licm-native",
            "licm-native-noop",
            "licm-native-unitlocal",
            "licm-native-unitlocal-noop"
        ]
    );
    assert_eq!(Case::ALL.map(Case::motion), [true, false, true, false]);
    assert_eq!(Case::ALL.map(Case::unit_local), [false, false, true, true]);
    assert!(profile("gfx942").is_ok());
    assert!(profile("gfx950").is_ok());
    for target in ["gfx942:xnack-", "gfx90a", "", "gfx950 "] {
        assert!(profile(target).is_err());
    }
}
