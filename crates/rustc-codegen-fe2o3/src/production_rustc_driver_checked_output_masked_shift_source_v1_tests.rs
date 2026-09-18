//! Ordinary safe Rust discovery and strict qualification share the same pipeline.
use super::*;
use simulation::masked_shift::{Batch, ROOTS, Spelling};

#[path = "production_rustc_driver_checked_output_masked_shift_discovery_v1_tests.rs"]
mod discovery;
pub(super) use discovery::record_child_refusal;
#[path = "production_rustc_driver_checked_output_masked_shift_fixture_v1_tests.rs"]
mod fixture_wiring;
#[path = "production_rustc_driver_checked_output_masked_shift_launch_v1_tests.rs"]
mod launch_contract;
pub(super) use launch_contract::check_descriptor;

pub(super) struct Config {
    pub(super) batch: Batch,
    pub(super) discovery: bool,
}

impl Config {
    pub(super) fn invoke_discovery(
        &self,
        command: &mut Command,
        args: &[String],
        response: &Path,
    ) -> (std::process::Output, Result<Observation, SourceFailure>) {
        discovery::invoke(self, command, args, response)
    }

    pub(super) fn name(&self) -> String {
        self.batch.name().into()
    }
    pub(super) fn roots(&self) -> &'static [&'static str] {
        &ROOTS
    }

    pub(super) fn check_root_roster(&self, roots: &[String]) -> Result<(), SourceFailure> {
        simulation::masked_shift::unique_root_ordinals(roots.iter().map(String::as_str))
            .map(|_| ())
            .map_err(|error| SourceFailure::new(SourceStage::Observation, error.detail))
    }

    pub(super) fn configure(&self, args: &mut Vec<String>) {
        args.push(format!(
            "--cfg=feature=\"masked-shift-{}\"",
            self.batch.integer.name()
        ));
        if self.batch.spelling == Spelling::WrappingMethod {
            args.push("--cfg=feature=\"masked-shift-wrapping\"".into());
        }
        if self.batch.retained {
            args.push("--cfg=feature=\"masked-shift-retained\"".into());
            args.push("-Zinline-mir=no".into());
        }
        // In particular, do not turn off the canonical overflow checks. A
        // retained Assert or unresolved core helper is a discovery result.
    }

    pub(super) fn record_refusal(
        &self,
        profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
        result: &Result<Observation, SourceFailure>,
    ) -> bool {
        let Err(error) = result else {
            return false;
        };
        if !self.discovery || !discovery::ordinary_refusal(error) {
            return false;
        }
        eprintln!(
            "MASKED_SHIFT_DISCOVERY {}",
            serde_json::json!({
                "status": "blocked", "qualified": false, "profile": profile.cpu(),
                "batch": self.batch, "name": self.name(), "first_failure": error,
                "scope": "ordinary-source-first-observed-gate; not tutorial or hardware qualification"
            })
        );
        true
    }

    pub(super) fn record_qualified(
        &self,
        profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
        observed: &Observation,
    ) {
        eprintln!(
            "MASKED_SHIFT_DISCOVERY {}",
            serde_json::json!({
                "status": "qualified", "qualified": true, "profile": profile.cpu(),
                "batch": self.batch, "name": self.name(), "observation": observed,
                "scope": "this exact fixture through actual O, CPU simulation, and LLVM handoff; no hardware claim"
            })
        );
    }

    pub(super) fn check(&self, observed: &Observation) {
        self.check_root_roster(&observed.roots).unwrap();
        assert_eq!(
            dispatch::check_private_helper_route(observed, false).unwrap(),
            dispatch::Route::DirectRawEmpty
        );
        let report = observed
            .masked_shift
            .as_ref()
            .expect("missing dynamic shift endpoint observations");
        assert_eq!(report.batch, self.batch);
        assert_eq!(report.output_digest, observed.output_digest);
        assert_ne!(report.source_digest, [0; 32]);
        assert_ne!(report.original_digest, [0; 32]);
        assert_eq!(report.roots.len(), ROOTS.len());
        assert!(
            report
                .roots
                .iter()
                .map(|row| &row.root)
                .eq(observed.roots.iter())
        );
        for row in &report.roots {
            assert_ne!(row.source_function, [0; 32]);
            assert_ne!(row.source_binding, [0; 32]);
            assert!(!row.original_entry.is_empty() && !row.original_operation_owner.is_empty());
            assert!(
                !row.entry.is_empty()
                    && !row.operation_owner.is_empty()
                    && !row.native_symbol.is_empty()
            );
        }
        let helpers = if self.batch.retained { ROOTS.len() } else { 0 };
        assert_eq!(
            (observed.internal_helpers, observed.helper_calls),
            (helpers, helpers)
        );
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct RootObservation {
    root: String,
    source_function: [u8; 32],
    source_binding: [u8; 32],
    original_entry: String,
    original_operation_owner: String,
    entry: String,
    operation_owner: String,
    native_symbol: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct ShiftObservation {
    batch: Batch,
    source_digest: [u8; 32],
    original_digest: [u8; 32],
    output_digest: [u8; 32],
    roots: Vec<RootObservation>,
}

pub(super) fn check_actual(
    stage: &dispatch::Stage,
    requested: Option<simulation::Case>,
) -> Result<Option<ShiftObservation>, SourceFailure> {
    let Some(simulation::Case::MaskedShift(batch)) = requested else {
        return Ok(None);
    };
    let dispatch::Stage::Direct(direct) = stage else {
        return Err(SourceFailure::new(
            SourceStage::Observation,
            "masked shifts require original RawEmpty custody",
        ));
    };
    let source = direct.output().source_semantic_kir();
    let semantic = stage.semantic();
    assert!(std::ptr::eq(semantic, source.semantic().semantic()));
    let source_digest = *semantic.semantic_sha256().as_bytes();
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(semantic.canonical_encoding())),
        source_digest
    );
    assert_eq!(semantic.roots().len(), ROOTS.len());
    let semantic_roots = semantic
        .roots()
        .iter()
        .map(|root| {
            let function = &semantic.functions()[root.index() as usize];
            let entry = function.kernel_entry().unwrap();
            (
                std::str::from_utf8(entry.export_symbol().as_bytes()).unwrap(),
                (
                    *function.identity().as_bytes(),
                    *entry.kernel_binding_identity().as_bytes(),
                ),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    assert_eq!(
        semantic_roots
            .keys()
            .copied()
            .collect::<std::collections::BTreeSet<_>>(),
        ROOTS.into_iter().collect::<std::collections::BTreeSet<_>>()
    );
    let output = stage.output().module();
    let observation_failure =
        |error: SourceFailure| SourceFailure::new(SourceStage::Observation, error.detail);
    simulation::constant_shift::check_preserved_root_order(source.module(), output)
        .map_err(observation_failure)?;
    let original_owners = simulation::masked_shift::check_graph(source.module(), batch)
        .map_err(observation_failure)?;
    let owners =
        simulation::masked_shift::check_graph(output, batch).map_err(observation_failure)?;
    let roots = output
        .kernels
        .iter()
        .map(|kernel| {
            let root = kernel.id.as_str();
            let original = simulation::constant_shift::kernel_for_root(source.module(), root)
                .map_err(observation_failure)?;
            let owner = owners[root].clone();
            Ok(RootObservation {
                root: root.into(),
                source_function: semantic_roots[root].0,
                source_binding: semantic_roots[root].1,
                original_entry: original.entry.as_str().into(),
                original_operation_owner: original_owners[root].clone(),
                entry: kernel.entry.as_str().into(),
                native_symbol: if batch.retained {
                    owner.clone()
                } else {
                    kernel.id.as_str().into()
                },
                operation_owner: owner,
            })
        })
        .collect::<Result<Vec<_>, SourceFailure>>()?;
    Ok(Some(ShiftObservation {
        batch,
        source_digest,
        original_digest: *source.canonical_kernel_ir_identity().digest(),
        output_digest: *stage.output().canonical().identity().digest(),
        roots,
    }))
}

fn check_native_body(batch: Batch, ordinal: usize, body: &str) -> Result<(), SourceFailure> {
    let opcode = if ordinal == 0 {
        "shl"
    } else if batch.integer.signed() {
        "ashr"
    } else {
        "lshr"
    };
    let expected = format!("{opcode} i{} ", batch.integer.width());
    let instructions = body
        .lines()
        .filter_map(|line| line.split_once(" = ").map(|(_, rhs)| rhs))
        .filter(|rhs| {
            ["shl ", "ashr ", "lshr "]
                .iter()
                .any(|opcode| rhs.starts_with(opcode))
        })
        .collect::<Vec<_>>();
    let [instruction] = instructions.as_slice() else {
        return Err(SourceFailure::new(
            SourceStage::NativeHandoff,
            "masked shift requires one actual-owner dynamic shift",
        ));
    };
    if !instruction.starts_with(&expected)
        || !instruction
            .split_once(',')
            .is_some_and(|(_, rhs)| rhs.trim().starts_with('%'))
    {
        return Err(SourceFailure::new(
            SourceStage::NativeHandoff,
            "masked shift native sign/width/flags/dynamic RHS differ",
        ));
    }
    Ok(())
}

pub(super) fn check_native(
    report: Option<&ShiftObservation>,
    llvm: &str,
) -> Result<(), SourceFailure> {
    let Some(report) = report else {
        return Ok(());
    };
    for row in &report.roots {
        let ordinal = simulation::masked_shift::root_ordinal(&row.root)
            .map_err(|error| SourceFailure::new(SourceStage::Observation, error.detail))?;
        let body = shift_source::native_body(llvm, &row.native_symbol)?;
        check_native_body(report.batch, ordinal, body)?;
        if report.batch.retained
            && !shift_source::native_body(llvm, &row.root)?
                .contains(&format!("@{}(", row.native_symbol))
        {
            return Err(SourceFailure::new(
                SourceStage::NativeHandoff,
                "masked shift lost exact retained helper call",
            ));
        }
    }
    Ok(())
}

const PROFILES: [fe2o3_amd_target::ProductionAmdTargetProfileV1; 2] = [
    fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
    fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950,
];

fn cases(discovery: bool) -> Vec<OrdinarySourceCase> {
    Batch::all()
        .into_iter()
        .map(|batch| OrdinarySourceCase::MaskedShift(Config { batch, discovery }))
        .collect()
}

fn selected_cases(discovery: bool) -> Vec<OrdinarySourceCase> {
    let Some(name) = env::var_os("FE2O3_TEST_MASKED_SHIFT_BATCH") else {
        return cases(discovery);
    };
    let name = name
        .to_str()
        .expect("masked-shift test batch must be UTF-8");
    let batch = Batch::parse(name).expect("unknown masked-shift test batch");
    vec![OrdinarySourceCase::MaskedShift(Config { batch, discovery })]
}

#[test]
fn masked_shift_matrix_separates_spellings_and_preserves_canonical_overflow_checks() {
    let mut names = std::collections::BTreeSet::new();
    for profile in PROFILES {
        for case in cases(false) {
            let OrdinarySourceCase::MaskedShift(config) = case else {
                unreachable!()
            };
            assert!(names.insert((profile.cpu(), config.name())));
            let mut args = vec!["-Coverflow-checks=on".into()];
            config.configure(&mut args);
            assert_eq!(
                args.iter()
                    .filter(|arg| arg.starts_with("-Coverflow-checks="))
                    .collect::<Vec<_>>(),
                [&"-Coverflow-checks=on".to_owned()]
            );
            assert_eq!(
                args.iter().any(|arg| arg == "-Zinline-mir=no"),
                config.batch.retained
            );
        }
    }
    assert_eq!((names.len(), names.len() * ROOTS.len()), (64, 128));
}

#[test]
fn masked_shift_native_observation_refuses_constants_wrong_opcode_and_flags() {
    for batch in Batch::all() {
        let right = if batch.integer.signed() {
            "ashr"
        } else {
            "lshr"
        };
        let exact = format!("%x = {right} i{} %value, %count", batch.integer.width());
        check_native_body(batch, 1, &exact).unwrap();
        for bad in [
            exact.replace("%count", "1"),
            exact.replace(right, "shl"),
            exact.replace(right, &format!("{right} exact")),
            exact.replace(&format!("i{} ", batch.integer.width()), "i128 "),
            String::new(),
        ] {
            assert!(check_native_body(batch, 1, &bad).is_err());
        }
    }
}

#[test]
fn discovery_never_relabels_a_refusal_as_qualification() {
    let mut config = Config {
        batch: Batch::all()[0],
        discovery: true,
    };
    let refusal = Err(SourceFailure::new(
        SourceStage::Policy4,
        "exact retained assertion gate",
    ));
    assert!(config.record_refusal(PROFILES[0], &refusal));
    config.discovery = false;
    assert!(!config.record_refusal(PROFILES[0], &refusal));
    assert!(matches!(&refusal, Err(error)
        if error.stage == SourceStage::Policy4
            && error.detail == "exact retained assertion gate"));
}

#[test]
#[ignore = "discovery only: records actual first refusal; a passing discovery test is not qualification"]
fn ordinary_rust_masked_shifts_discover_first_gate_both_profiles() {
    let cases = selected_cases(true);
    for profile in PROFILES {
        ordinary_rust_checked_output_cases_for_profile(&cases, profile);
    }
}

#[test]
#[ignore = "strict qualification requires ordinary wrapping/masked capture, assertion and helper admission"]
fn ordinary_rust_masked_shifts_reach_actual_o_and_cpu_oracle_both_profiles() {
    let cases = selected_cases(false);
    for profile in PROFILES {
        ordinary_rust_checked_output_cases_for_profile(&cases, profile);
    }
}
