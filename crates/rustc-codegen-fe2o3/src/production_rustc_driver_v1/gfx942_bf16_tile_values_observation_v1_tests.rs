//! Actual copied call-transport rows; no emitted graph or normal/CPU authority.
use super::*;
use crate::production_bf16_tile_values_source_v1::{
    Role as SourceRole, SourceOwnedBf16TileValuesRegionV1,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use fe2o3_lower_mir_kernel::{Bf16CallInstanceErrorV1 as Error, Bf16CallInstanceRoleV1 as Role};
use fe2o3_mir_model::SsaValueV1;
use std::cell::Cell;
const ROLES: [Role; 7] = [
    Role::Context,
    Role::Lane,
    Role::Lhs,
    Role::Rhs,
    Role::Zero,
    Role::Result,
    Role::Values,
];
const SOURCE_ROLES: [SourceRole; 8] = [
    SourceRole::Context,
    SourceRole::Lane,
    SourceRole::Lhs,
    SourceRole::Rhs,
    SourceRole::Zero,
    SourceRole::Matrix,
    SourceRole::Values,
    SourceRole::HelperCall,
];
const EXTRA: usize = 23;
const WORK: usize = 17;
const CALLBACK_REFUSAL: &str = "genuine BF16 helper callback refused for accounting control";
const NORMAL_REFUSAL: &str =
    "helper parameter is not an exact by-value scalar aggregate or shared slice";
#[derive(Clone, Copy, Debug, Serialize)]
struct Snapshot {
    source_sha256: [u8; 32],
    root_mir_sha256: [u8; 32],
    helper_mir_sha256: [u8; 32],
    semantic_sha256: [u8; 32],
    helper_source_signature_sha256: [u8; 32],
    helper_fn_abi_sha256: [u8; 32],
    source_bytes: usize,
    root: u32,
    helper: u32,
    call_block: u32,
    function_blocks: [usize; 2],
    source_spans: [[u32; 2]; 8],
    raw_blocks: [u32; 8],
    producers: [[u32; 5]; 7],
    call_arguments: [[u32; 3]; 4],
    formals: [[u32; 3]; 4],
    return_permutation: [u8; 4],
    helper_actual_abi_modes: [u8; 5],
    callback_storage_before: usize,
    callback_storage_after: usize,
    callback_work_before: usize,
    callback_work_after: usize,
    source_authority_in_copied_row: bool,
    emitted_helper_qualified: bool,
    normal_qualified: bool,
    numerical_cpu_qualified: bool,
}
fn ssa(value: SsaValueV1) -> [u32; 3] {
    match value {
        SsaValueV1::Definition(id) => [0, id.get(), 0],
        SsaValueV1::BlockArgument { block, variable } => [1, block.get(), variable.get()],
    }
}
fn abi_mode(mode: &rustc_target::callconv::PassMode) -> u8 {
    use rustc_target::callconv::PassMode;
    match mode {
        PassMode::Ignore => 0,
        PassMode::Direct(_) => 1,
        PassMode::Pair(..) => 2,
        PassMode::Cast { .. } => 3,
        PassMode::Indirect { .. } => 4,
    }
}
fn snapshot(
    view: &SourceOwnedBf16TileValuesRegionV1<'_, '_>,
    budget: &mut Budget<'_>,
) -> Result<Snapshot, Error> {
    let storage = budget.storage();
    let work = budget.work();
    budget.reserve_storage(2 * std::mem::size_of::<Snapshot>())?;
    budget.charge_work(view.source().bytes().len() + 4096)?;
    let relation = view.relation();
    let semantic = relation.owner().source_semantic();
    if semantic.functions().len() != 2
        || view.helper_actual_fn_abi().args.len() != 4
        || view.grants_artifact_or_launch_authority()
    {
        return Err(Error::Unavailable("test helper snapshot shape"));
    }
    let mut producers = [[0; 5]; 7];
    for (index, role) in ROLES.into_iter().enumerate() {
        let p = relation.producer(role);
        let value = ssa(p.value());
        producers[index] = [
            p.function().index(),
            p.block().index(),
            value[0],
            value[1],
            value[2],
        ];
    }
    let mut call_arguments = [[0; 3]; 4];
    let mut formals = [[0; 3]; 4];
    for i in 0..4 {
        call_arguments[i] = ssa(relation
            .call_argument_ssa(i)
            .ok_or(Error::Unavailable("test actual call argument absent"))?);
        formals[i] = ssa(relation
            .formal_ssa(i)
            .ok_or(Error::Unavailable("test actual formal absent"))?);
    }
    if relation.call_argument_ssa(4).is_some() || relation.formal_ssa(4).is_some() {
        return Err(Error::Unavailable("test actual argument overrun"));
    }
    let source_spans = SOURCE_ROLES.map(|role| {
        let span = view.source_span(role);
        [span.lo().0, span.hi().0]
    });
    let raw_blocks = SOURCE_ROLES.map(|role| view.raw_block(role));
    let abi = view.helper_actual_fn_abi();
    let mut modes = [0; 5];
    for (i, arg) in abi.args.iter().enumerate() {
        modes[i] = abi_mode(&arg.mode);
    }
    modes[4] = abi_mode(&abi.ret.mode);
    Ok(Snapshot {
        source_sha256: *view.source().sha256(),
        root_mir_sha256: *view.root_mir_sha256(),
        helper_mir_sha256: *view.helper_mir_sha256(),
        semantic_sha256: *semantic.semantic_sha256().as_bytes(),
        helper_source_signature_sha256: *view.helper_source_signature_sha256(),
        helper_fn_abi_sha256: *view.helper_fn_abi_sha256(),
        source_bytes: view.source().bytes().len(),
        root: relation.root().index(),
        helper: relation.helper().index(),
        call_block: relation.call_block().index(),
        function_blocks: [
            semantic.functions()[0].blocks().len(),
            semantic.functions()[1].blocks().len(),
        ],
        source_spans,
        raw_blocks,
        producers,
        call_arguments,
        formals,
        return_permutation: relation.return_permutation(),
        helper_actual_abi_modes: modes,
        callback_storage_before: storage,
        callback_storage_after: budget.storage(),
        callback_work_before: work,
        callback_work_after: budget.work(),
        source_authority_in_copied_row: false,
        emitted_helper_qualified: false,
        normal_qualified: false,
        numerical_cpu_qualified: false,
    })
}
pub(super) fn observe<'tcx>(
    transaction: crate::production_pipeline::ProductionCompilation<
        'tcx,
        crate::production_pipeline::CollectedRustStage<'tcx>,
    >,
    case: &str,
) -> Value {
    let captured = Cell::new(None::<Snapshot>);
    let callback = Cell::new(None::<[usize; 4]>);
    let (result, phase) =
        transaction.observe_bf16_tile_values_source_for_test_v1(|view, budget| {
            if matches!(case, "identity-error" | "identity-panic") {
                let before = [budget.storage(), budget.work()];
                budget.reserve_storage(EXTRA)?;
                budget.charge_work(WORK)?;
                callback.set(Some([
                    before[0],
                    before[1],
                    budget.storage(),
                    budget.work(),
                ]));
                if case == "identity-panic" {
                    panic!("genuine BF16 helper callback panic for accounting control");
                }
                return Err(Error::Unavailable(CALLBACK_REFUSAL));
            }
            if captured.replace(Some(snapshot(view, budget)?)).is_some() {
                return Err(Error::Unavailable("test helper callback repeated"));
            }
            Ok(())
        });
    let stage = if result.is_ok() {
        "unexpected_ordinary_success"
    } else if captured.get().is_some() {
        "actual_borrowed_transport_then_ordinary_refusal"
    } else if callback.get().is_some() {
        "actual_callback_refused"
    } else {
        "actual_source_transport_unavailable"
    };
    json!({"stage":stage,"snapshot":captured.get(),"callback":callback.get(),"phase":phase,"diagnostic":result.as_ref().err().map(ToString::to_string),"unexpected_ordinary_success":result.is_ok(),"emitted_helper_qualified":false,"normal_qualified":false,"numerical_cpu_qualified":false,"hardware_observed":false})
}
fn u(value: &Value) -> Result<u64, &'static str> {
    value
        .as_u64()
        .ok_or("actual helper scalar observation absent")
}
pub(super) fn accept(case: &str, row: &Value) -> Result<(), &'static str> {
    let diagnostic = row["diagnostic"]
        .as_str()
        .ok_or("actual helper refusal absent")?;
    if row["unexpected_ordinary_success"] == true {
        return Err("ordinary helper behavior changed; separate review required");
    }
    if case == "wrong-launch" {
        if !diagnostic.contains("BF16 helper requires explicit WG64 and one workgroup")
            || !row["snapshot"].is_null()
            || !row["callback"].is_null()
        {
            return Err("wrong actual helper source refusal boundary");
        }
        return Ok(());
    }
    let phase = &row["phase"];
    if phase["same_ledger"] != true
        || phase["result_ok"] != false
        || phase["failed_work"] != false
        || phase["failed_storage"] != false
        || u(&phase["occurrence_storage"])? == 0
    {
        return Err("actual helper original ledger relation differs");
    }
    let extra = if matches!(case, "identity-error" | "identity-panic") {
        let expected = if case == "identity-panic" {
            "CallbackPanicked"
        } else {
            CALLBACK_REFUSAL
        };
        if row["stage"] != "actual_callback_refused"
            || !diagnostic.contains(expected)
            || phase["normal_attempted"] != false
            || !row["snapshot"].is_null()
        {
            return Err("actual helper callback boundary differs");
        }
        let values = row["callback"]
            .as_array()
            .ok_or("actual helper callback absent")?;
        if values.len() != 4
            || u(&values[2])?.checked_sub(u(&values[0])?) != Some(EXTRA as u64)
            || u(&values[3])?.checked_sub(u(&values[1])?) != Some(WORK as u64)
            || u(&phase["work"])? < u(&values[3])?
        {
            return Err("actual helper callback resources differ");
        }
        EXTRA as u64
    } else {
        let expected = match case {
            "identity" => [0, 1, 2, 3],
            "swap01" => [1, 0, 2, 3],
            _ => return Err("unknown helper case"),
        };
        let snapshot = &row["snapshot"];
        if row["stage"] != "actual_borrowed_transport_then_ordinary_refusal"
            || !diagnostic.contains(NORMAL_REFUSAL)
            || phase["normal_attempted"] != true
            || snapshot["return_permutation"] != json!(expected)
            || snapshot["root"] == snapshot["helper"]
            || snapshot["source_authority_in_copied_row"] != false
            || snapshot["emitted_helper_qualified"] != false
        {
            return Err("borrowed helper source relation or unchanged normal refusal differs");
        }
        let before = u(&snapshot["callback_storage_before"])?;
        let after = u(&snapshot["callback_storage_after"])?;
        if after.checked_sub(before) != Some((2 * std::mem::size_of::<Snapshot>()) as u64) {
            return Err("actual helper snapshot storage differs");
        }
        for field in [
            "source_sha256",
            "root_mir_sha256",
            "helper_mir_sha256",
            "semantic_sha256",
            "helper_source_signature_sha256",
            "helper_fn_abi_sha256",
        ] {
            if snapshot[field].as_array().is_none_or(|v| v.len() != 32) {
                return Err("actual helper identity observation absent");
            }
        }
        after - before
    };
    if u(&phase["final_storage"])?
        != u(&phase["entry_storage"])?
            .checked_add(extra)
            .ok_or("helper floor overflow")?
    {
        return Err("actual helper cleanup did not restore original floor plus surviving output");
    }
    Ok(())
}
#[test]
fn helper_acceptance_never_counts_spelling_or_earlier_refusal_as_transport() {
    for case in ["identity", "swap01", "identity-error", "identity-panic"] {
        assert!(accept(case,&json!({"diagnostic":NORMAL_REFUSAL,"snapshot":null,"callback":null,"phase":null,"unexpected_ordinary_success":false})).is_err());
    }
    assert!(
        accept(
            "wrong-launch",
            &json!({"diagnostic":"unrelated compiler panic","snapshot":null,"callback":null})
        )
        .is_err()
    );
}
