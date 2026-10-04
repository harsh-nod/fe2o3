//! Closed masked-fixture adapter for the existing strict fixed6 child.
use super::super::masked_shift_source as shifts;
use super::*;
use simulation::masked_shift::{Batch, ROOTS, Spelling};

#[path = "production_rustc_driver_masked_shift_fixed6_protocol_v1_tests.rs"]
mod protocol;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct Case {
    pub(super) batch: Batch,
    pub(super) target: Target,
}
impl Case {
    pub(super) fn name(self) -> String {
        format!("{}-{}", self.target.cpu(), self.batch.name())
    }
    fn simulation(self) -> simulation::Case {
        simulation::Case::MaskedShift(self.batch)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct Observation6 {
    case: Case,
    before: shifts::ShiftObservation,
    output: shifts::ShiftObservation,
    original_order: Vec<String>,
    before_order: Vec<String>,
    output_order: Vec<String>,
    pub(super) source_roots: Vec<census::SourceRoot>,
    erased_digest: Option<[u8; 32]>,
    policy: u16,
    passes: Vec<String>,
    pass_changed: Vec<bool>,
    execution_sha256: [u8; 32],
    continuation_sha256: [u8; 32],
    replay_work: usize,
    simulation: simulation::SimulationObservation,
    pub(super) llvm_sha256: [u8; 32],
    pub(super) llvm_bytes: usize,
    descriptor_roots: usize,
}

fn order(module: &fe2o3_kernel_ir::Module) -> Vec<String> {
    module
        .kernels
        .iter()
        .map(|kernel| kernel.id.as_str().to_owned())
        .collect()
}

pub(super) fn observe(stage: Stage, case: Case, artifact: &Path) -> Result<Outcome, String> {
    if stage.erased_digest().is_some()
        || stage.kernels().len() != ROOTS.len()
        || !std::ptr::eq(stage.output(), stage.checked_output().owner())
    {
        return Err("masked fixed6 requires actual direct N and exact final-I ownership".into());
    }
    let replay_work = replay_actual_continuation(&stage)?;
    let source_digest = *stage.semantic().semantic_sha256().as_bytes();
    if digest(stage.semantic().canonical_encoding()) != source_digest
        || canonical_v12_digest(stage.original_canonical_bytes()) != stage.original_digest()
    {
        return Err("masked fixed6 source/N bytes differ from retained identities".into());
    }
    let checked = stage.checked_output();
    let before = checked.intermediate_policy5().owner();
    let observe_endpoint = |output: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12| {
        shifts::observe_subjects(
            stage.semantic(),
            stage.original_module(),
            stage.original_digest(),
            output.module(),
            *output.canonical().identity().digest(),
            case.batch,
        )
        .map_err(|error| format!("masked fixed6 endpoint observation: {error:?}"))
    };
    let before_observation = observe_endpoint(before)?;
    let output_observation = observe_endpoint(stage.output())?;
    simulation::constant_shift::check_preserved_root_order(
        before.module(),
        stage.output().module(),
    )
    .map_err(|error| format!("masked fixed6 O/I root order: {error:?}"))?;
    let passes = checked.continuation().report().passes();
    let mut report = Observation6 {
        case,
        before: before_observation,
        output: output_observation,
        original_order: order(stage.original_module()),
        before_order: order(before.module()),
        output_order: order(stage.output().module()),
        source_roots: census::roots(stage.semantic())?,
        erased_digest: stage.erased_digest().copied(),
        policy: checked.execution().policy_version(),
        passes: passes
            .iter()
            .map(|pass| pass.pass().name().into())
            .collect(),
        pass_changed: passes.iter().map(|pass| pass.changed()).collect(),
        execution_sha256: digest(checked.execution().canonical_bytes()),
        continuation_sha256: digest(checked.continuation().execution().canonical_bytes()),
        replay_work,
        simulation: simulation::observe(stage.output().canonical(), case.simulation())
            .map_err(|error| format!("masked actual-I simulation: {error:?}"))?,
        llvm_sha256: [0; 32],
        llvm_bytes: 0,
        descriptor_roots: 0,
    };
    let (handoff, descriptor) = stage
        .into_worker_handoff_extraction_v1()
        .map_err(|error| format!("masked final-I extraction: {error:?}"))?;
    let target =
        fe2o3_compiler_ffi::DeviceTargetV1::parse(&format!("{}:xnack-", case.target.cpu()))
            .map_err(|error| format!("{error:?}"))?;
    if handoff.target() != target
        || handoff.code_object_version() != fe2o3_compiler_ffi::CodeObjectVersion::V6
        || descriptor.grants_link_authority()
        || descriptor.grants_load_authority()
        || descriptor.grants_launch_authority()
        || descriptor.table().producer().version().as_str()
            != format!("production-policy6-checked-{}-cov6-v1", case.target.cpu())
    {
        return Err("masked final-I target/descriptor/authority changed".into());
    }
    shifts::check_descriptor(Some(&report.output), &descriptor)
        .map_err(|error| format!("masked final-I descriptor: {error:?}"))?;
    let llvm = std::str::from_utf8(handoff.module_bytes()).map_err(|error| error.to_string())?;
    shifts::check_native(Some(&report.output), llvm)
        .map_err(|error| format!("masked final-I native body: {error:?}"))?;
    report.llvm_sha256 = digest(handoff.module_bytes());
    report.llvm_bytes = handoff.module_bytes().len();
    report.descriptor_roots = descriptor.table().kernels().len();
    validate_observation(case, &report)?;
    use std::io::Write;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(artifact)
        .and_then(|mut file| file.write_all(handoff.module_bytes()))
        .map_err(|error| error.to_string())?;
    Ok(Outcome::ObservedMasked(Box::new(report)))
}

pub(super) fn validate_observation(case: Case, report: &Observation6) -> Result<(), String> {
    for roots in [
        &report.original_order,
        &report.before_order,
        &report.output_order,
    ] {
        simulation::masked_shift::unique_root_ordinals(roots.iter().map(String::as_str))
            .map_err(|error| format!("{error:?}"))?;
    }
    simulation::masked_shift::unique_root_ordinals(
        report.source_roots.iter().map(|root| root.name.as_str()),
    )
    .map_err(|error| format!("{error:?}"))?;
    if report.case != case
        || report.erased_digest.is_some()
        || report.original_order != report.before_order
        || report.before_order != report.output_order
        || report.policy != 6
        || report.passes != ["integer-neutral-canonicalization", "dead-code-elimination"]
        || report.pass_changed.len() != 2
        || report.execution_sha256 == [0; 32]
        || report.continuation_sha256 == [0; 32]
        || report.replay_work == 0
        || report.llvm_sha256 == [0; 32]
        || report.llvm_bytes == 0
        || report.descriptor_roots != ROOTS.len()
        || report.before.batch != case.batch
        || report.output.batch != case.batch
        || report.before.source_digest == [0; 32]
        || report.before.source_digest != report.output.source_digest
        || report.before.original_digest == [0; 32]
        || report.before.original_digest != report.output.original_digest
        || report.before.output_digest == [0; 32]
        || report.output.output_digest == [0; 32]
        || report.before.roots.len() != ROOTS.len()
        || report.output.roots.len() != ROOTS.len()
        || !report
            .before
            .roots
            .iter()
            .map(|row| &row.root)
            .eq(report.before_order.iter())
        || !report
            .output
            .roots
            .iter()
            .map(|row| &row.root)
            .eq(report.output_order.iter())
    {
        return Err(
            "masked fixed6 report lost exact source/N/O/I or compiled execution evidence".into(),
        );
    }
    for (before, output) in report.before.roots.iter().zip(&report.output.roots) {
        let source = report
            .source_roots
            .iter()
            .find(|root| root.name == output.root)
            .ok_or("masked fixed6 root has no exact actual source subject")?;
        if source.function == [0; 32]
            || source.body == [0; 32]
            || output.source_function != source.function
            || before.source_function != source.function
            || output.source_binding == [0; 32]
            || output.source_binding != before.source_binding
            || before.original_entry.is_empty()
            || before.original_operation_owner.is_empty()
            || before.original_entry != output.original_entry
            || before.original_operation_owner != output.original_operation_owner
            || before.entry.is_empty()
            || before.entry != output.entry
            || before.operation_owner.is_empty()
            || before.operation_owner != output.operation_owner
            || before.native_symbol != output.native_symbol
            || output.native_symbol.as_str()
                != if case.batch.retained {
                    output.operation_owner.as_str()
                } else {
                    output.root.as_str()
                }
        {
            return Err("masked fixed6 root/helper/source binding substitution".into());
        }
    }
    // The shift family need not trigger an identity rewrite. An unchanged I is
    // valid only after the same sealed execution and independent transition replay.
    simulation::check_report(
        &report.simulation,
        report.output.output_digest,
        case.simulation(),
    )
    .map_err(|error| format!("masked final-I report: {error:?}"))
}

fn cases() -> Vec<Case> {
    [Target::Gfx942, Target::Gfx950]
        .into_iter()
        .flat_map(|target| {
            Batch::all()
                .into_iter()
                .map(move |batch| Case { batch, target })
        })
        .collect()
}

fn selected_cases() -> Vec<Case> {
    let Some(name) = env::var_os("FE2O3_TEST_MASKED_SHIFT_BATCH") else {
        return cases();
    };
    let batch = Batch::parse(name.to_str().expect("masked fixed6 batch must be UTF-8"))
        .expect("unknown masked fixed6 batch");
    [Target::Gfx942, Target::Gfx950]
        .into_iter()
        .map(|target| Case { batch, target })
        .collect()
}

fn fixture(workspace: &Path, case: Case) -> corpus::Fixture {
    let hash = |relative: &str| {
        digest(&std::fs::read(workspace.join(relative)).unwrap())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    let manifest = format!("{BASE}/Cargo.toml");
    let mut features = vec![format!("masked-shift-{}", case.batch.integer.name())];
    if case.batch.retained {
        features.push("masked-shift-retained".into());
    }
    if case.batch.spelling == Spelling::WrappingMethod {
        features.push("masked-shift-wrapping".into());
    }
    corpus::Fixture {
        fixture_id: case.name(),
        target: case.target.cpu().into(),
        compiler_input: corpus::CompilerInput {
            package_manifest_sha256: hash(&manifest),
            package_manifest: manifest,
            cargo_lock_path: "Cargo.lock".into(),
            cargo_lock_sha256: hash("Cargo.lock"),
            source_paths: vec![
                format!("{BASE}/src/lib.rs"),
                format!("{BASE}/src/masked_shift.rs"),
            ],
            source_closure_sha256: String::new(),
            cargo_target: corpus::CargoTarget {
                kind: "lib".into(),
                name: "fe2o3_production_extraction_fixture".into(),
                source_path: "src/lib.rs".into(),
            },
            default_features: false,
            features,
            kernel_symbols: ROOTS.map(str::to_owned).to_vec(),
        },
    }
}

#[test]
#[ignore = "strict masked fixed6 Cargo matrix; actual I, independent public extraction, census and proof boundaries"]
fn ordinary_rust_masked_shifts_reach_fixed6_i_native_sim_and_census_both_profiles() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-policy6-masked-source");
    let cases = selected_cases();
    let expected = cases.len();
    let source = source_stamps(&workspace, FixtureCase::MaskedShift(cases[0]));
    let (mut completed, mut roots, mut compiler_children) = (0, 0, 0);
    for case in cases {
        let subject = FixtureCase::MaskedShift(case);
        assert_eq!(source_stamps(&workspace, subject), source);
        let directory = scratch.path().join(case.name());
        std::fs::create_dir(&directory).unwrap();
        let mut captured = corpus_cargo::capture(
            &workspace,
            &fixture(&workspace, case),
            &directory,
            &scratch.path().join(case.target.cpu()),
        )
        .unwrap();
        require_canonical_overflow_checks_v1(&captured.args).unwrap();
        if case.batch.retained {
            captured.args.push("-Zinline-mir=no".into());
        }
        let request = Request {
            case: subject,
            mode: Mode::Observe,
            source: source.clone(),
            args_sha256: digest(&serde_json::to_vec(&captured.args).unwrap()),
        };
        let (observed, children) = qualify_case(&captured, &directory, request);
        let Outcome::ObservedMasked(observed) = observed else {
            unreachable!()
        };
        validate_observation(case, &observed).unwrap();
        assert_eq!(source_stamps(&workspace, subject), source);
        completed += 1;
        roots += observed.output.roots.len();
        compiler_children += children;
        eprintln!(
            "FIXED6 MASKED {}: actual N={:02x?}; O={:02x?}; I={:02x?}; 2 roots; {} independent bytes; exact enabled/disabled census and missing-proof refusal; extraction-only, no default or launch authority",
            case.name(),
            observed.output.original_digest,
            observed.before.output_digest,
            observed.output.output_digest,
            observed.llvm_bytes
        );
    }
    assert_eq!(
        (completed, roots, compiler_children),
        (expected, expected * 2, expected * 4)
    );
    assert!(matches!(expected, 2 | 64));
}
