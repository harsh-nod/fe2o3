//! Source-driven component qualification; fixed6 remains the native baseline.
use super::*;

#[path = "production_rustc_driver_commutative_cse_graph_v1_tests.rs"]
mod checks;
#[path = "production_rustc_driver_commutative_cse_protocol_v1_tests.rs"]
mod protocol;

#[path = "production_rustc_driver_commutative_tail_source_v1_tests.rs"]
mod post_policy7;

pub(in super::super) fn name(case: Case) -> String {
    format!(
        "commutative-{}-{}-opt0",
        case.integer.name(),
        case.target.cpu()
    )
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(in super::super) struct Observation {
    case: Case,
    source_digest: [u8; 32],
    original_digest: [u8; 32],
    original_bytes: u64,
    component_digest: [u8; 32],
    component_bytes: u64,
    baseline_i_digest: [u8; 32],
    original_order: Vec<String>,
    component_order: Vec<String>,
    proved_pairs: usize,
    changed: bool,
    component_replay_work: usize,
    baseline_replay_work: usize,
    pub(in super::super) source_roots: Vec<census::SourceRoot>,
    roots: Vec<checks::Root>,
    simulation: sim::Report,
    pub(in super::super) baseline_llvm_sha256: [u8; 32],
    pub(in super::super) baseline_llvm_bytes: usize,
}

pub(in super::super) fn observe(
    stage: Stage,
    case: Case,
    artifact: &Path,
) -> Result<Outcome, String> {
    if stage.erased_digest().is_some() || stage.kernels().len() != ROOTS.len() {
        return Err("commutative qualifier requires actual Direct6 and three roots".into());
    }
    let original = stage.test_original_owner_v1();
    if !std::ptr::eq(original.module(), stage.original_module())
        || original.canonical().canonical_bytes() != stage.original_canonical_bytes()
        || original.canonical().identity().digest() != &stage.original_digest()
        || canonical_v12_digest(stage.original_canonical_bytes()) != stage.original_digest()
        || digest(stage.semantic().canonical_encoding())
            != *stage.semantic().semantic_sha256().as_bytes()
    {
        return Err("commutative capture does not borrow exact retained source N".into());
    }
    let original_digest = stage.original_digest();
    let baseline_i_digest = *stage.output().canonical().identity().digest();
    let baseline_replay_work = graph::replay_prefix(&stage)? + replay_actual_continuation(&stage)?;
    let mut work = Work::new(crate::production_canonical_phase_policy_v1::WORK_LIMIT as usize);
    let mut budget = Budget::new(
        &mut work,
        crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
    );
    let floor = stage.retained_storage_floor_v1();
    budget
        .reserve_storage(floor)
        .map_err(|e| format!("{e:?}"))?;
    let ledger = budget.work_ledger_identity_v1();
    let actual = fe2o3_pliron::optimize_checked_commutative_bitwise_cse_v1(original, &mut budget)
        .map_err(|e| format!("actual original-N commutative service: {e:?}"))?;
    if budget.storage() != floor
        || budget.work_ledger_identity_v1() != ledger
        || !std::ptr::eq(actual.input(), original)
        || actual.grants_authority()
    {
        return Err("commutative service lost actual N/ledger/floor boundary".into());
    }
    let retained = actual.retained_storage();
    budget
        .reserve_storage(retained)
        .map_err(|e| format!("{e:?}"))?;
    let output_floor = budget.storage();
    let before_replay = budget.work();
    let proved = actual
        .replay(&mut budget)
        .map_err(|e| format!("independent actual-pair replay: {e:?}"))?;
    let component_replay_work = budget.work() - before_replay;
    if budget.storage() != output_floor
        || budget.work_ledger_identity_v1() != ledger
        || proved != ROOTS.len()
        || actual.proved_pairs() != proved
        || !actual.execution().changed()
    {
        return Err("commutative service requires three genuine checked substitutions".into());
    }
    let roots = checks::observe(&stage, &actual, case)?;
    let mut report = Observation {
        case,
        source_digest: *stage.semantic().semantic_sha256().as_bytes(),
        original_digest,
        original_bytes: original.canonical().identity().canonical_length(),
        component_digest: *actual.owner().canonical().identity().digest(),
        component_bytes: actual.owner().canonical().identity().canonical_length(),
        baseline_i_digest,
        original_order: graph::order(original.module()),
        component_order: graph::order(actual.owner().module()),
        proved_pairs: proved,
        changed: actual.execution().changed(),
        component_replay_work,
        baseline_replay_work,
        source_roots: census::roots(stage.semantic())?,
        roots,
        simulation: sim::observe(actual.owner().canonical(), case)?,
        baseline_llvm_sha256: [0; 32],
        baseline_llvm_bytes: 0,
    };
    if stage.original_digest() != original_digest
        || stage.output().canonical().identity().digest() != &baseline_i_digest
    {
        return Err("component observation changed retained original/baseline subjects".into());
    }
    drop(actual);
    budget
        .release_storage(retained)
        .map_err(|e| format!("{e:?}"))?;
    if budget.storage() != floor || budget.work_ledger_identity_v1() != ledger {
        return Err("component owner did not drop before exact receipt release".into());
    }
    // This is unchanged fixed6 I, NOT native emission/admission of the component output.
    let (handoff, descriptor) = stage
        .into_worker_handoff_extraction_v1()
        .map_err(|e| format!("baseline fixed6 extraction: {e:?}"))?;
    if handoff.target()
        != fe2o3_compiler_ffi::DeviceTargetV1::parse(&format!("{}:xnack-", case.target.cpu()))
            .map_err(|e| format!("{e:?}"))?
        || handoff.code_object_version() != fe2o3_compiler_ffi::CodeObjectVersion::V6
        || descriptor.grants_link_authority()
        || descriptor.grants_load_authority()
        || descriptor.grants_launch_authority()
        || descriptor.table().producer().version().as_str()
            != format!("production-policy6-checked-{}-cov6-v1", case.target.cpu())
    {
        return Err("baseline target/descriptor/authority changed".into());
    }
    checks::descriptor(&report.roots, descriptor.table().kernels())?;
    report.baseline_llvm_sha256 = digest(handoff.module_bytes());
    report.baseline_llvm_bytes = handoff.module_bytes().len();
    use std::io::Write;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(artifact)
        .and_then(|mut file| file.write_all(handoff.module_bytes()))
        .map_err(|e| e.to_string())?;
    validate(case, &report)?;
    Ok(Outcome::ObservedCommutative(Box::new(report)))
}

pub(in super::super) fn validate(case: Case, report: &Observation) -> Result<(), String> {
    graph::roster(report.original_order.iter().map(String::as_str))?;
    graph::roster(report.source_roots.iter().map(|r| r.name.as_str()))?;
    graph::roster(report.roots.iter().map(|r| r.name.as_str()))?;
    if report.case != case
        || report.original_order != report.component_order
        || report.original_digest == report.component_digest
        || report.original_bytes == 0
        || report.component_bytes == 0
        || report.proved_pairs != ROOTS.len()
        || !report.changed
        || report.component_replay_work == 0
        || report.baseline_replay_work == 0
        || report.baseline_llvm_bytes == 0
        || [
            report.source_digest,
            report.original_digest,
            report.component_digest,
            report.baseline_i_digest,
            report.baseline_llvm_sha256,
        ]
        .contains(&[0; 32])
    {
        return Err(
            "commutative report lost exact source/nonempty component/baseline evidence".into(),
        );
    }
    for root in &report.roots {
        checks::validate(root)?;
        let source = report
            .source_roots
            .iter()
            .find(|s| s.name == root.name)
            .ok_or("source root")?;
        if source.function != root.source_function || source.body != root.source_body {
            return Err("commutative root does not bind selected actual source/body".into());
        }
    }
    sim::validate(
        &report.simulation,
        case,
        report.component_digest,
        report.component_bytes,
    )
}

#[test]
#[ignore = "strict actual original-N commutative component plus independent baseline fixed6 extraction"]
fn ordinary_rust_commutative_cse_all_integer_widths_and_both_profiles() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-commutative-source");
    let mut completed = 0;
    for case in cases() {
        let subject = FixtureCase::CommutativeCse(case);
        let source = source_stamps(&workspace, subject);
        let directory = scratch.path().join(name(case));
        std::fs::create_dir(&directory).unwrap();
        let mut captured = corpus_cargo::capture(
            &workspace,
            &fixture_for(
                &workspace,
                case,
                "commutative-cse",
                "commutative_cse.rs",
                name(case),
            ),
            &directory,
            &scratch.path().join(case.target.cpu()),
        )
        .unwrap();
        require_canonical_overflow_checks_v1(&captured.args).unwrap();
        captured
            .args
            .extend(["-Zmir-opt-level=0".into(), "-Zinline-mir=no".into()]);
        let request = Request {
            case: subject,
            mode: Mode::Observe,
            args_sha256: digest(&serde_json::to_vec(&captured.args).unwrap()),
            source: source.clone(),
        };
        let (observed, native) = child(&captured, &directory, request.clone());
        let Outcome::ObservedCommutative(report) = observed else {
            panic!("wrong component family")
        };
        validate(case, &report).unwrap();
        let (extracted, public) = child(
            &captured,
            &directory,
            Request {
                mode: Mode::Extract,
                ..request
            },
        );
        let Outcome::Extracted {
            llvm_sha256,
            llvm_bytes,
            source_roots,
        } = extracted
        else {
            panic!("wrong baseline extraction mode")
        };
        graph::roster(source_roots.iter().map(|r| r.name.as_str())).unwrap();
        let mut actual_roots = source_roots;
        let mut expected_roots = report.source_roots.clone();
        actual_roots.sort_by(|a, b| a.name.cmp(&b.name));
        expected_roots.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(actual_roots, expected_roots);
        let observed = std::fs::read(native).unwrap();
        assert_eq!(
            observed,
            std::fs::read(public).unwrap(),
            "baseline fixed6 bytes only"
        );
        assert_eq!(
            (digest(&observed), observed.len()),
            (report.baseline_llvm_sha256, report.baseline_llvm_bytes)
        );
        assert_eq!(
            (llvm_sha256, llvm_bytes),
            (report.baseline_llvm_sha256, report.baseline_llvm_bytes)
        );
        assert_eq!(source_stamps(&workspace, subject), source);
        completed += 1;
        eprintln!(
            "COMMUTATIVE {}: actual N/component 3 swapped substitutions, retained U32 Divide/effects and component SIM; public native is unchanged fixed6 baseline",
            name(case)
        );
    }
    assert_eq!(
        (completed, completed * ROOTS.len(), completed * 2),
        (16, 48, 32)
    );
}
