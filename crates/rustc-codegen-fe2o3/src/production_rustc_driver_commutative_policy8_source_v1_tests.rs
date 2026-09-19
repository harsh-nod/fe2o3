//! Strict actual-source K native qualification; no signed-positive owner.
use super::post_policy7::{compare_simulations, traps};
use super::*;
use crate::production_pipeline::checked_output_policy8_v1::source_observation as live;
use fe2o3_kernel_ir::{BinaryOp, InertFormalMemoryReceiptFormatV4 as Formal};

#[path = "production_rustc_driver_commutative_policy8_protocol_v1_tests.rs"]
mod protocol8;

#[path = "production_rustc_driver_commutative_policy8_controls_v1_tests.rs"]
mod controls8;

#[path = "production_rustc_driver_commutative_policy8_unit_local_v1_tests.rs"]
mod unit_local;

const SCENARIOS: usize = 3 * 6 * 10 * 2;

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Source8 {
    semantic: [u8; 32],
    roots: Vec<census::SourceRoot>,
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Observation8 {
    case: Case,
    source: Source8,
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
    after_replay_work: usize,
    replay_work: usize,
    stage_floor: usize,
    after_replay_floor: usize,
    original_sim: sim::Report,
    final_sim: sim::Report,
    compared_scenarios: usize,
    native_policy: u16,
    native_subject: [u8; 32],
    native_llvm_sha256: [u8; 32],
    native_llvm_bytes: usize,
    grants_authority: bool,
}

fn source8(
    semantic: &fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
) -> Result<Source8, String> {
    let roots = census::roots(semantic)?;
    graph::roster(roots.iter().map(|r| r.name.as_str()))?;
    let identity = *semantic.semantic_sha256().as_bytes();
    if digest(semantic.canonical_encoding()) != identity {
        return Err("retained semantic identity/bytes disagree".into());
    }
    Ok(Source8 {
        semantic: identity,
        roots,
    })
}

fn observe_view(
    view: live::View<'_>,
    case: Case,
    budget: &mut Budget<'_>,
) -> Result<Observation8, String> {
    if !matches!(view.owner, live::Owner::Direct(_)) {
        return Err("this three-root fixture must retain its genuine Direct source route".into());
    }
    observe_bound_view(view, case, budget)
}

fn observe_bound_view(
    view: live::View<'_>,
    case: Case,
    budget: &mut Budget<'_>,
) -> Result<Observation8, String> {
    let (semantic, original, i, j, k, tail, kernels, grants_authority) = match view.owner {
        live::Owner::Direct(owner) => {
            let prefix7 = owner.prefix();
            let prefix6 = prefix7.prefix();
            (
                prefix6.source_semantic_kir().semantic().semantic(),
                prefix6
                    .source_semantic_kir()
                    .pre_ranked_executable()
                    .ok_or("actual original source N missing")?,
                prefix6.output(),
                prefix7.output(),
                owner.output(),
                owner.continuation(),
                owner.kernels(),
                owner.grants_artifact_or_launch_authority(),
            )
        }
        live::Owner::Erased(owner) => {
            let prefix7 = owner.prefix();
            let prefix6 = prefix7.prefix();
            (
                prefix6.original_source().semantic_ssa().source_semantic(),
                prefix6.original_source().executable(),
                prefix6.output(),
                prefix7.output(),
                owner.output(),
                owner.continuation(),
                owner.kernels(),
                owner.grants_artifact_or_launch_authority(),
            )
        }
    };
    let source = source8(semantic)?;
    if !std::ptr::eq(original, view.owner.original())
        || !std::ptr::eq(j, view.owner.historical_j())
        || !std::ptr::eq(k, view.owner.output())
        || !std::ptr::eq(k, tail.output())
        || tail.input_identity() != j.canonical().identity()
        || tail.proved_pairs() != ROOTS.len()
        || !tail.execution().changed()
        || j.canonical().identity() == k.canonical().identity()
        || grants_authority
        || tail.grants_authority()
        || view.artifacts.prefix_execution().policy_version() != 7
        || view.artifacts.policy_version() != 8
        || view.profile.device_target() != format!("{}:xnack-", case.target.cpu())
    {
        return Err("actual source/P7/J/K custody, nonempty mutation or authority changed".into());
    }
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let before = budget.work();
    view.replay(budget)?;
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
    let descriptor = view.artifacts.descriptor_source();
    checks::descriptor(&roots, descriptor.table().kernels())?;
    if descriptor.grants_link_authority()
        || descriptor.grants_load_authority()
        || descriptor.grants_launch_authority()
        || descriptor.table().producer().version().as_str()
            != format!("production-policy8-checked-{}-cov6-v1", case.target.cpu())
        || view.artifacts.llvm_ir().is_empty()
    {
        return Err("actual K descriptor/authority changed".into());
    }
    // Independent test reemission is not an additional production emission.
    budget
        .reserve_storage(3 * dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES)
        .map_err(|e| format!("{e:?}"))?;
    {
        let raw = match view.profile {
            fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942 => dialect_amdgcn::lower_canonical_v12_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(k),
            fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950 => dialect_amdgcn::lower_canonical_v12_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(k),
        }.map_err(|e| format!("{e:?}"))?;
        let bound = dialect_amdgcn::bind_production_llvm22_worker_layout_v1(&raw)
            .map_err(|e| format!("{e:?}"))?;
        let module =
            crate::kernel_ir_codegen::retain_production_compiler_module_text_v1(k.module(), bound)
                .map_err(|e| format!("{e:?}"))?;
        let complete =
            crate::kernel_ir_codegen::bind_compiler_descriptor_source_v1(module, descriptor)
                .map_err(|e| format!("{e:?}"))?;
        if complete.llvm_ir() != view.artifacts.llvm_ir() {
            return Err("independent complete K LLVM/descriptor bytes differ".into());
        }
    }
    budget
        .release_storage(3 * dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES)
        .map_err(|e| format!("{e:?}"))?;
    let mut formal = Vec::new();
    for report in kernels {
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
    let report = Observation8 {
        case,
        source,
        original: *original.canonical().identity().digest(),
        original_bytes: original.canonical().identity().canonical_length(),
        historical_i: *i.canonical().identity().digest(),
        historical_j: *j.canonical().identity().digest(),
        historical_j_bytes: j.canonical().identity().canonical_length(),
        final_k: *k.canonical().identity().digest(),
        final_k_bytes: k.canonical().identity().canonical_length(),
        original_order: graph::order(original.module()),
        j_order: graph::order(j.module()),
        k_order: graph::order(k.module()),
        roots,
        fresh_k_formal: formal,
        historical_p7_execution: digest(view.artifacts.prefix_execution().canonical_bytes()),
        proved_pairs: tail.proved_pairs(),
        changed: tail.execution().changed(),
        j_traps: traps(j.module()),
        k_traps: traps(k.module()),
        entry_work: view.entry_work,
        stage_work: view.prepared_work,
        after_replay_work: budget.work(),
        replay_work,
        stage_floor: view.prepared_floor,
        after_replay_floor: budget.storage(),
        original_sim,
        final_sim,
        compared_scenarios,
        native_policy: 8,
        native_subject: *k.canonical().identity().digest(),
        native_llvm_sha256: digest(view.artifacts.llvm_ir().as_bytes()),
        native_llvm_bytes: view.artifacts.llvm_ir().len(),
        grants_authority: false,
    };
    validate8(&report, case)?;
    Ok(report)
}

fn validate8(report: &Observation8, case: Case) -> Result<(), String> {
    graph::roster(report.original_order.iter().map(String::as_str))?;
    graph::roster(report.source.roots.iter().map(|r| r.name.as_str()))?;
    graph::roster(report.roots.iter().map(|r| r.name.as_str()))?;
    if report.case != case
        || report.original_order != report.j_order
        || report.j_order != report.k_order
        || report.historical_j == report.final_k
        || report.native_subject != report.final_k
        || report.native_policy != 8
        || report.grants_authority
        || report.proved_pairs != ROOTS.len()
        || !report.changed
        || report.j_traps != report.k_traps
        || report.original_bytes == 0
        || report.historical_j_bytes == 0
        || report.final_k_bytes == 0
        || report.native_llvm_bytes == 0
        || report.replay_work == 0
        || !(report.entry_work < report.stage_work && report.stage_work < report.after_replay_work)
        || report.stage_floor == 0
        || report.after_replay_floor != report.stage_floor
        || report.compared_scenarios != SCENARIOS
        || [
            report.source.semantic,
            report.original,
            report.historical_i,
            report.historical_j,
            report.final_k,
            report.historical_p7_execution,
            report.native_llvm_sha256,
        ]
        .contains(&[0; 32])
    {
        return Err(
            "actual-K report lost exact source/mutation/floor/fixed8 native evidence".into(),
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
fn actual_k_matrix_keeps_all_widths_profiles_and_operations() {
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
