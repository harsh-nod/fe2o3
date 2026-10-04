//! Strict source qualification of the existing exact dominance-CSE schedule.
use super::*;
use simulation::constant_shift::Integer;

#[path = "production_rustc_driver_commutative_cse_source_v1_tests.rs"]
pub(super) mod commutative;

#[path = "production_rustc_driver_dominance_cse_graph_v1_tests.rs"]
mod graph;
#[path = "production_rustc_driver_dominance_cse_protocol_v1_tests.rs"]
mod protocol;
#[path = "production_rustc_driver_dominance_cse_simulation_v1_tests.rs"]
mod sim;

pub(super) const ROOTS: [&str; 3] = ["dominance_and", "dominance_or", "dominance_xor"];

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct Case {
    integer: Integer,
    pub(super) target: Target,
}
impl Case {
    pub(super) fn name(self) -> String {
        format!(
            "dominance-{}-{}-opt0",
            self.integer.name(),
            self.target.cpu()
        )
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Observation6 {
    case: Case,
    source_digest: [u8; 32],
    original_digest: [u8; 32],
    bound_digest: [u8; 32],
    cse_digest: [u8; 32],
    output_digest: [u8; 32],
    output_canonical_bytes: u64,
    pub(super) source_roots: Vec<census::SourceRoot>,
    roots: Vec<graph::Root>,
    original_order: Vec<String>,
    output_order: Vec<String>,
    prefix_passes: Vec<String>,
    dominance_changed: bool,
    prefix_execution: [u8; 32],
    policy6_execution: [u8; 32],
    replay_work: usize,
    simulation: sim::Report,
    pub(super) llvm_sha256: [u8; 32],
    pub(super) llvm_bytes: usize,
}

fn passes() -> Vec<String> {
    use fe2o3_pliron::PlironOptimizationPassV1 as P;
    [
        P::SparseConditionalConstantPropagation,
        P::SimplifyControlFlow,
        P::SelectSameValueCanonicalization,
        P::DeadCodeElimination,
        P::LocalPureCommonSubexpressionElimination,
        P::DominancePureCommonSubexpressionElimination,
        P::DeadCodeElimination,
        P::SimplifyControlFlow,
    ]
    .map(|p| p.name().to_owned())
    .to_vec()
}

pub(super) fn observe(stage: Stage, case: Case, artifact: &Path) -> Result<Outcome, String> {
    if stage.erased_digest().is_some() || stage.kernels().len() != ROOTS.len() {
        return Err("dominance source requires the actual Direct6 route and three roots".into());
    }
    let roots = graph::observe(&stage, case)?;
    let replay_work = graph::replay_prefix(&stage)? + replay_actual_continuation(&stage)?;
    let prefix = stage
        .checked_output()
        .intermediate_policy5()
        .intermediate_policy4()
        .intermediate_policy3();
    let source_digest = *stage.semantic().semantic_sha256().as_bytes();
    if digest(stage.semantic().canonical_encoding()) != source_digest
        || canonical_v12_digest(stage.original_canonical_bytes()) != stage.original_digest()
    {
        return Err("dominance source N/semantic byte identity changed".into());
    }
    let mut report = Observation6 {
        case,
        source_digest,
        original_digest: stage.original_digest(),
        bound_digest: *stage.test_bound_owner_v1().canonical().identity().digest(),
        cse_digest: *prefix.owner().canonical().identity().digest(),
        output_digest: *stage.output().canonical().identity().digest(),
        output_canonical_bytes: stage.output().canonical().identity().canonical_length(),
        source_roots: census::roots(stage.semantic())?,
        roots,
        original_order: graph::order(stage.original_module()),
        output_order: graph::order(stage.output().module()),
        prefix_passes: prefix
            .report()
            .passes()
            .iter()
            .map(|p| p.pass().name().to_owned())
            .collect(),
        dominance_changed: prefix.report().passes().get(5).is_some_and(|p| p.changed()),
        prefix_execution: digest(prefix.execution().canonical_bytes()),
        policy6_execution: digest(stage.checked_output().execution().canonical_bytes()),
        replay_work,
        simulation: sim::observe(stage.output().canonical(), case)?,
        llvm_sha256: [0; 32],
        llvm_bytes: 0,
    };
    // Fresh native lowering borrows actual I before consuming the source owner.
    // All raw/bound/final strings drop before releasing their overlapping scratch.
    let mut work = Work::new(crate::production_canonical_phase_policy_v1::WORK_LIMIT as usize);
    let mut budget = Budget::new(
        &mut work,
        crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
    );
    let floor = stage.retained_storage_floor_v1();
    budget
        .reserve_storage(floor)
        .map_err(|e| format!("{e:?}"))?;
    let scratch = 3 * dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES;
    budget
        .reserve_storage(scratch)
        .map_err(|e| format!("{e:?}"))?;
    {
        let raw = match case.target {
            Target::Gfx942 => dialect_amdgcn::lower_canonical_v12_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(stage.output()),
            Target::Gfx950 => dialect_amdgcn::lower_canonical_v12_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(stage.output()),
        }.map_err(|e| format!("fresh I LLVM: {e:?}"))?;
        let bound = dialect_amdgcn::bind_production_llvm22_worker_layout_v1(&raw)
            .map_err(|e| format!("{e:?}"))?;
        let fresh = crate::kernel_ir_codegen::retain_production_compiler_module_text_v1(
            stage.output().module(),
            bound,
        )
        .map_err(|e| format!("{e:?}"))?;
        let (handoff, descriptor) = stage
            .into_worker_handoff_extraction_v1()
            .map_err(|e| format!("fixed6 extraction: {e:?}"))?;
        let fresh =
            crate::kernel_ir_codegen::bind_compiler_descriptor_source_v1(fresh, &descriptor)
                .map_err(|e| format!("{e:?}"))?;
        if fresh.llvm_ir().as_bytes() != handoff.module_bytes()
            || handoff.target()
                != fe2o3_compiler_ffi::DeviceTargetV1::parse(&format!(
                    "{}:xnack-",
                    case.target.cpu()
                ))
                .map_err(|e| format!("{e:?}"))?
            || handoff.code_object_version() != fe2o3_compiler_ffi::CodeObjectVersion::V6
            || descriptor.grants_link_authority()
            || descriptor.grants_load_authority()
            || descriptor.grants_launch_authority()
            || descriptor.table().producer().version().as_str()
                != format!("production-policy6-checked-{}-cov6-v1", case.target.cpu())
        {
            return Err("dominance final-I native bytes/target/authority changed".into());
        }
        graph::descriptor(&report.roots, descriptor.table().kernels())?;
        graph::native(
            case,
            std::str::from_utf8(handoff.module_bytes()).map_err(|e| e.to_string())?,
        )?;
        report.llvm_sha256 = digest(handoff.module_bytes());
        report.llvm_bytes = handoff.module_bytes().len();
        use std::io::Write;
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(artifact)
            .and_then(|mut file| file.write_all(handoff.module_bytes()))
            .map_err(|e| e.to_string())?;
    }
    budget
        .release_storage(scratch)
        .map_err(|e| format!("{e:?}"))?;
    if budget.storage() != floor {
        return Err("native observation changed inherited floor".into());
    }
    validate_observation(case, &report)?;
    Ok(Outcome::ObservedDominance(Box::new(report)))
}

pub(super) fn validate_observation(case: Case, report: &Observation6) -> Result<(), String> {
    graph::roster(report.original_order.iter().map(String::as_str))?;
    graph::roster(report.source_roots.iter().map(|r| r.name.as_str()))?;
    graph::roster(report.roots.iter().map(|r| r.name.as_str()))?;
    if report.case != case
        || report.output_order != report.original_order
        || report.prefix_passes != passes()
        || !report.dominance_changed
        || report.bound_digest == report.cse_digest
        || report.replay_work == 0
        || report.llvm_bytes == 0
        || [
            report.source_digest,
            report.original_digest,
            report.bound_digest,
            report.cse_digest,
            report.output_digest,
            report.prefix_execution,
            report.policy6_execution,
            report.llvm_sha256,
        ]
        .contains(&[0; 32])
    {
        return Err(
            "dominance report lost exact case/source/prefix/mutation/native evidence".into(),
        );
    }
    for root in &report.roots {
        graph::validate_root(root)?;
        let source = report
            .source_roots
            .iter()
            .find(|s| s.name == root.name)
            .ok_or("missing semantic root")?;
        if source.function != root.source_function || source.body != root.source_body {
            return Err("dominance selected source/body identity mismatch".into());
        }
    }
    sim::validate(
        &report.simulation,
        case,
        report.output_digest,
        report.output_canonical_bytes,
    )
}

fn cases() -> Vec<Case> {
    [Target::Gfx942, Target::Gfx950]
        .into_iter()
        .flat_map(|target| {
            Integer::ALL
                .into_iter()
                .map(move |integer| Case { integer, target })
        })
        .collect()
}
fn fixture(workspace: &Path, case: Case) -> corpus::Fixture {
    fixture_for(
        workspace,
        case,
        "dominance-cse",
        "dominance_cse.rs",
        case.name(),
    )
}
fn fixture_for(
    workspace: &Path,
    case: Case,
    feature: &str,
    leaf: &str,
    fixture_id: String,
) -> corpus::Fixture {
    let hash = |relative: &str| {
        digest(&std::fs::read(workspace.join(relative)).unwrap())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    let manifest = format!("{BASE}/Cargo.toml");
    corpus::Fixture {
        fixture_id,
        target: case.target.cpu().into(),
        compiler_input: corpus::CompilerInput {
            package_manifest_sha256: hash(&manifest),
            package_manifest: manifest,
            cargo_lock_path: "Cargo.lock".into(),
            cargo_lock_sha256: hash("Cargo.lock"),
            source_paths: vec![format!("{BASE}/src/lib.rs"), format!("{BASE}/src/{leaf}")],
            source_closure_sha256: String::new(),
            cargo_target: corpus::CargoTarget {
                kind: "lib".into(),
                name: "fe2o3_production_extraction_fixture".into(),
                source_path: "src/lib.rs".into(),
            },
            default_features: false,
            features: vec![format!("{feature}-{}", case.integer.name())],
            kernel_symbols: ROOTS.map(str::to_owned).to_vec(),
        },
    }
}

#[test]
#[ignore = "strict existing dominance-CSE ordinary Rust matrix; actual B/C/I and public fixed6 bytes"]
fn ordinary_rust_cross_block_exact_cse_all_integer_widths_and_both_profiles() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-dominance-source");
    let mut completed = 0;
    for case in cases() {
        let subject = FixtureCase::DominanceCse(case);
        let source = source_stamps(&workspace, subject);
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
        let Outcome::ObservedDominance(report) = observed else {
            panic!("wrong observed fixture family")
        };
        validate_observation(case, &report).unwrap();
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
            panic!("wrong public output mode")
        };
        graph::roster(source_roots.iter().map(|r| r.name.as_str())).unwrap();
        let mut actual_roots = source_roots;
        let mut expected_roots = report.source_roots.clone();
        actual_roots.sort_by(|a, b| a.name.cmp(&b.name));
        expected_roots.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(actual_roots, expected_roots);
        let observed = std::fs::read(native).unwrap();
        let emitted = std::fs::read(public).unwrap();
        assert_eq!(
            observed, emitted,
            "independent public fixed6 full native bytes"
        );
        assert_eq!(
            (digest(&observed), observed.len()),
            (report.llvm_sha256, report.llvm_bytes)
        );
        assert_eq!(
            (llvm_sha256, llvm_bytes),
            (report.llvm_sha256, report.llvm_bytes)
        );
        assert_eq!(source_stamps(&workspace, subject), source);
        completed += 1;
        eprintln!(
            "FIXED6 DOMINANCE {}: 3 actual cross-block substitutions; fresh I/native/SIM and public output equality; extraction only",
            case.name()
        );
    }
    assert_eq!(
        (completed, completed * ROOTS.len(), completed * 2),
        (16, 48, 32)
    );
}
