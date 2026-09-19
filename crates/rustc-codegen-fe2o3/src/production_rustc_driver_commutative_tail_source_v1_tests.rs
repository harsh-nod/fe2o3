//! Strict actual-source J/K qualification. Native bytes remain historical P7 J.
use super::*;
use crate::production_pipeline::checked_output_policy7_v1::commutative_source_observation as live;
use fe2o3_kernel_ir::{
    AmdGpuDiagnosticOperation as Diagnostic, BinaryOp, InertFormalMemoryReceiptFormatV4 as Formal,
    OperationKind as Kind,
};

#[path = "production_rustc_driver_commutative_tail_protocol_v1_tests.rs"]
mod protocol7;

#[path = "production_rustc_driver_commutative_tail_controls_v1_tests.rs"]
mod controls7;

const SCENARIOS: usize = 3 * 6 * 10 * 2;

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Source7 {
    semantic: [u8; 32],
    roots: Vec<census::SourceRoot>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Observation7 {
    case: Case,
    source: Source7,
    original: [u8; 32],
    original_bytes: u64,
    historical_i: [u8; 32],
    historical_j: [u8; 32],
    historical_j_bytes: u64,
    final_k: [u8; 32],
    final_k_bytes: u64,
    original_order: Vec<String>,
    j_order: Vec<String>,
    k_order: Vec<String>,
    roots: Vec<checks::Root>,
    fresh_k_formal: Vec<(String, [u8; 32])>,
    historical_p7_execution: [u8; 32],
    proved_pairs: usize,
    changed: bool,
    j_traps: usize,
    k_traps: usize,
    entry_work: usize,
    stage_work: usize,
    tail_work: usize,
    replay_work: usize,
    stage_floor: usize,
    tail_floor: usize,
    original_sim: sim::Report,
    final_sim: sim::Report,
    compared_scenarios: usize,
    baseline_policy: u16,
    baseline_subject: [u8; 32],
    baseline_llvm_sha256: [u8; 32],
    baseline_llvm_bytes: usize,
    grants_authority: bool,
}

fn source7(
    semantic: &fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
) -> Result<Source7, String> {
    let roots = census::roots(semantic)?;
    graph::roster(roots.iter().map(|r| r.name.as_str()))?;
    let identity = *semantic.semantic_sha256().as_bytes();
    if digest(semantic.canonical_encoding()) != identity {
        return Err("retained semantic identity/bytes disagree".into());
    }
    Ok(Source7 {
        semantic: identity,
        roots,
    })
}

fn traps(module: &fe2o3_kernel_ir::Module) -> usize {
    module
        .functions
        .iter()
        .flat_map(|f| &f.body)
        .flat_map(|b| &b.blocks)
        .flat_map(|b| &b.operations)
        .filter(|op| {
            matches!(&op.kind,
            Kind::Call { callee, arguments } if matches!(
                Diagnostic::from_intrinsic_call(callee, arguments),
                Some(Diagnostic::Trap | Diagnostic::AssertFail { .. })
            ))
        })
        .count()
}

// Both independently executed actual graphs are already checked against the
// same typed host oracle, full buffers/canaries and repeated immutable requests.
// Bind their exact experiment roster as well; executed step counts may differ.
fn compare_simulations(original: &sim::Report, final_k: &sim::Report) -> Result<usize, String> {
    let original = serde_json::to_value(original).map_err(|e| e.to_string())?;
    let final_k = serde_json::to_value(final_k).map_err(|e| e.to_string())?;
    let a = original["scenarios"]
        .as_array()
        .ok_or("original SIM scenarios")?;
    let b = final_k["scenarios"]
        .as_array()
        .ok_or("final SIM scenarios")?;
    if a.len() != SCENARIOS || b.len() != a.len() {
        return Err("original/final SIM experiment count changed".into());
    }
    for (a, b) in a.iter().zip(b) {
        for field in [
            "root",
            "len",
            "lhs",
            "rhs",
            "choose",
            "grid",
            "workgroup",
            "invocations",
            "checked_bytes",
            "replays",
        ] {
            let (Some(a), Some(b)) = (a.get(field), b.get(field)) else {
                return Err("original/final SIM experiment field missing".into());
            };
            if a != b {
                return Err("original/final SIM experiments differ".into());
            }
        }
    }
    Ok(a.len())
}

fn observe_view(
    view: live::View<'_>,
    case: Case,
    budget: &mut Budget<'_>,
) -> Result<Observation7, String> {
    let live::Owner::Direct(owner) = view.owner else {
        return Err("this three-root fixture must retain its genuine Direct source route".into());
    };
    let prefix7 = owner.prefix();
    let prefix6 = prefix7.prefix();
    let semantic = prefix6.source_semantic_kir().semantic().semantic();
    let source = source7(semantic)?;
    let original = prefix6
        .source_semantic_kir()
        .pre_ranked_executable()
        .ok_or("actual original source N missing")?;
    let j = prefix7.output();
    let k = owner.output();
    let tail = owner.continuation();
    if !std::ptr::eq(original, view.owner.original())
        || !std::ptr::eq(j, view.owner.historical_j())
        || !std::ptr::eq(k, view.owner.output())
        || !std::ptr::eq(k, tail.output())
        || tail.input_identity() != j.canonical().identity()
        || tail.proved_pairs() != ROOTS.len()
        || !tail.execution().changed()
        || j.canonical().identity() == k.canonical().identity()
        || owner.grants_artifact_or_launch_authority()
        || tail.grants_authority()
        || view.baseline_execution.policy_version() != 7
        || view.profile.device_target() != format!("{}:xnack-", case.target.cpu())
    {
        return Err("actual source/P7/J/K custody, nonempty mutation or authority changed".into());
    }
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let before = budget.work();
    owner
        .verify_equivalence(budget)
        .map_err(|e| format!("actual full source/J/K replay: {e:?}"))?;
    let replay_work = budget
        .work()
        .checked_sub(before)
        .ok_or("replay work went backwards")?;
    if budget.storage() != floor || budget.work_ledger_identity_v1() != ledger || replay_work == 0 {
        return Err("same live stage/tail replay lost ledger/floor/work".into());
    }
    // P7 may have changed N already. The expected two reversed dynamic bitwise
    // expressions and one retained dynamic U32 Divide are checked in actual J,
    // not inferred by copying original-N coordinates or an old component report.
    let roots = checks::observe_graphs(
        semantic,
        j.module(),
        k.module(),
        tail.occurrences().candidate(),
        case,
    )?;
    checks::descriptor(&roots, view.baseline_descriptor.table().kernels())?;
    if view.baseline_descriptor.grants_link_authority()
        || view.baseline_descriptor.grants_load_authority()
        || view.baseline_descriptor.grants_launch_authority()
        || view
            .baseline_descriptor
            .table()
            .producer()
            .version()
            .as_str()
            != format!("production-policy7-checked-{}-cov6-v1", case.target.cpu())
        || view.baseline_llvm.is_empty()
    {
        return Err("historical J baseline descriptor/authority changed".into());
    }
    let mut formal = Vec::new();
    for report in owner.kernels() {
        if report.accesses().is_empty() {
            return Err("fresh K formal report lost nonvacuous external effects".into());
        }
        let receipt = Formal::from_current_obligations(report).map_err(|e| format!("{e:?}"))?;
        formal.push((
            report.kernel().as_str().to_owned(),
            digest(receipt.canonical_bytes()),
        ));
    }
    let original_sim = sim::observe(original.canonical(), case)?;
    let final_sim = sim::observe(k.canonical(), case)?;
    let compared_scenarios = compare_simulations(&original_sim, &final_sim)?;
    let report = Observation7 {
        case,
        source,
        original: *original.canonical().identity().digest(),
        original_bytes: original.canonical().identity().canonical_length(),
        historical_i: *prefix6.output().canonical().identity().digest(),
        historical_j: *j.canonical().identity().digest(),
        historical_j_bytes: j.canonical().identity().canonical_length(),
        final_k: *k.canonical().identity().digest(),
        final_k_bytes: k.canonical().identity().canonical_length(),
        original_order: graph::order(original.module()),
        j_order: graph::order(j.module()),
        k_order: graph::order(k.module()),
        roots,
        fresh_k_formal: formal,
        historical_p7_execution: digest(view.baseline_execution.canonical_bytes()),
        proved_pairs: tail.proved_pairs(),
        changed: tail.execution().changed(),
        j_traps: traps(j.module()),
        k_traps: traps(k.module()),
        entry_work: view.entry_work,
        stage_work: view.stage_work,
        tail_work: view.tail_work,
        replay_work,
        stage_floor: view.stage_floor,
        tail_floor: view.tail_floor,
        original_sim,
        final_sim,
        compared_scenarios,
        baseline_policy: 7,
        baseline_subject: *j.canonical().identity().digest(),
        baseline_llvm_sha256: digest(view.baseline_llvm.as_bytes()),
        baseline_llvm_bytes: view.baseline_llvm.len(),
        grants_authority: false,
    };
    validate7(&report, case)?;
    Ok(report)
}

fn validate7(report: &Observation7, case: Case) -> Result<(), String> {
    graph::roster(report.original_order.iter().map(String::as_str))?;
    graph::roster(report.source.roots.iter().map(|r| r.name.as_str()))?;
    graph::roster(report.roots.iter().map(|r| r.name.as_str()))?;
    if report.case != case
        || report.original_order != report.j_order
        || report.j_order != report.k_order
        || report.historical_j == report.final_k
        || report.baseline_subject != report.historical_j
        || report.baseline_policy != 7
        || report.grants_authority
        || report.proved_pairs != ROOTS.len()
        || !report.changed
        || report.j_traps != report.k_traps
        || report.original_bytes == 0
        || report.historical_j_bytes == 0
        || report.final_k_bytes == 0
        || report.baseline_llvm_bytes == 0
        || report.replay_work == 0
        || !(report.entry_work < report.stage_work && report.stage_work < report.tail_work)
        || report.stage_floor == 0
        || report.tail_floor <= report.stage_floor
        || report.compared_scenarios != SCENARIOS
        || [
            report.source.semantic,
            report.original,
            report.historical_i,
            report.historical_j,
            report.final_k,
            report.historical_p7_execution,
            report.baseline_llvm_sha256,
        ]
        .contains(&[0; 32])
    {
        return Err(
            "actual-J report lost exact source/tail/nonempty/floor/P7-baseline evidence".into(),
        );
    }
    if report
        .fresh_k_formal
        .iter()
        .map(|r| &r.0)
        .ne(report.k_order.iter())
        || report.fresh_k_formal.iter().any(|r| r.1 == [0; 32])
    {
        return Err("fresh K report roster/identity differs from actual K order".into());
    }
    for root in &report.roots {
        checks::validate(root)?;
        let source = report
            .source
            .roots
            .iter()
            .find(|s| s.name == root.name)
            .ok_or("source root")?;
        if source.function != root.source_function || source.body != root.source_body {
            return Err("actual J/K report substituted selected source/body".into());
        }
    }
    sim::validate(
        &report.original_sim,
        case,
        report.original,
        report.original_bytes,
    )?;
    sim::validate(
        &report.final_sim,
        case,
        report.final_k,
        report.final_k_bytes,
    )?;
    if compare_simulations(&report.original_sim, &report.final_sim)? != report.compared_scenarios {
        return Err("original/final SIM comparison changed".into());
    }
    Ok(())
}

#[test]
fn actual_j_matrix_keeps_all_widths_profiles_and_operations() {
    let values = cases();
    assert_eq!(values.len(), 16);
    for integer in Integer::ALL {
        for target in [Target::Gfx942, Target::Gfx950] {
            assert_eq!(
                values
                    .iter()
                    .filter(|c| c.integer == integer && c.target == target)
                    .count(),
                1
            );
        }
    }
    assert_eq!(
        ROOTS.map(|root| root.rsplit('_').next().unwrap()),
        ["and", "or", "xor"]
    );
    assert_eq!(
        [
            graph::operation(0),
            graph::operation(1),
            graph::operation(2)
        ],
        [BinaryOp::BitAnd, BinaryOp::BitOr, BinaryOp::BitXor]
    );
    assert_eq!(SCENARIOS, 360);
}
