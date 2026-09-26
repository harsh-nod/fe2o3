//! Copies bounded observations from the actual live source view. These rows
//! cannot reconstruct source custody or authorize a later compiler operation.
use super::*;
use crate::production_tiled_region_source_v1::SourceOwnedBf16MfmaRegionV1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, MatrixOperationKind, OperationKind,
};
use fe2o3_lower_mir_kernel::{
    ProductionBf16MfmaRoleV1 as Role, ProductionTiledRegionInspectionErrorV1 as Error,
};
use fe2o3_mir_model::SsaValueV1;
use std::cell::Cell;

const ROLES: [Role; 6] = [
    Role::Context,
    Role::Lane,
    Role::Lhs,
    Role::Rhs,
    Role::Zero,
    Role::Result,
];
const CALLBACK_EXTRA: usize = 23;
const CALLBACK_WORK: usize = 17;
const CALLBACK_REFUSAL: &str = "genuine BF16 source callback refused for accounting control";

#[derive(Clone, Copy, Debug, Serialize)]
struct Snapshot {
    source_sha256: [u8; 32],
    mir_sha256: [u8; 32],
    semantic_sha256: [u8; 32],
    canonical_sha256: [u8; 32],
    canonical_identity: [u8; 32],
    source_bytes: usize,
    canonical_bytes: usize,
    source_blocks: usize,
    canonical_blocks: usize,
    canonical_operations: usize,
    source_spans: [[u32; 2]; 6],
    raw_blocks: [u32; 6],
    semantic_blocks: [u32; 6],
    // SSA tag 0 = real Definition; tag 1 = real BlockArgument(block,variable).
    producers: [[u32; 3]; 6],
    consumed: [[u32; 3]; 4],
    components: [[u32; 4]; 6],
    component_counts: [usize; 6],
    consumed_components: [[u32; 4]; 4],
    consumed_counts: [usize; 4],
    matrix_inputs: [u32; 12],
    matrix_results: [u32; 4],
    matrix_span: [u32; 5],
    outside_uses: usize,
    outside_uses_sha256: [u8; 32],
    outside_uses_per_component: [u32; 4],
    unused_result_components: [bool; 4],
    // Four fixed chunks avoid an unbounded observation allocation. Each
    // populated row is [block, operation-or-u32::MAX, operand, component].
    // The first outside_uses rows are Some; all remaining rows are None.
    outside_use_rows: [[Option<[u32; 4]>; 32]; 4],
    exact_workgroup: [u32; 3],
    max_grid: [u32; 3],
    callback_storage_before: usize,
    callback_storage_after: usize,
    callback_work_before: usize,
    callback_work_after: usize,
    alias_observed: bool,
    pre_ranked_only: bool,
    source_authority_in_copied_row: bool,
}
fn copy_use_row(
    rows: &mut [[Option<[u32; 4]>; 32]; 4],
    counts: &mut [u32; 4],
    index: usize,
    words: [u32; 4],
) -> Result<(), Error> {
    if index >= 128 || words[3] >= 4 {
        return Err(Error::Unavailable("test result-use row bound"));
    }
    let slot = &mut rows[index / 32][index % 32];
    if slot.is_some() {
        return Err(Error::Unavailable("test result-use row duplicated"));
    }
    counts[words[3] as usize] = counts[words[3] as usize]
        .checked_add(1)
        .ok_or(Error::Unavailable("test result-use count overflow"))?;
    *slot = Some(words);
    Ok(())
}
fn ssa(value: SsaValueV1) -> [u32; 3] {
    match value {
        SsaValueV1::Definition(id) => [0, id.get(), 0],
        SsaValueV1::BlockArgument { block, variable } => [1, block.get(), variable.get()],
    }
}
fn snapshot(
    view: &SourceOwnedBf16MfmaRegionV1<'_, '_>,
    budget: &mut Budget<'_>,
) -> Result<Snapshot, Error> {
    let storage_before = budget.storage();
    let work_before = budget.work();
    let emission = view.emission();
    let owner = emission.original();
    let canonical = owner.executable().canonical().canonical_bytes();
    if canonical.len() > 1024 * 1024 {
        return Err(Error::Unavailable("test canonical observation cap"));
    }
    // Pay fixed return/constructor coexistence and hash scratch BEFORE their
    // construction. The copied return stays charged through the phase; no
    // observer-owned reset, sibling ledger, Vec, String or source pointer.
    let charge = 2 * std::mem::size_of::<Snapshot>() + std::mem::size_of::<Sha256>();
    budget.reserve_storage(charge)?;
    budget.charge_work(canonical.len() + view.source().bytes().len() + 8192)?;
    let module = owner.executable().module();
    // The live emission view has already sealed this exact root/declaration
    // roster. Select its actual slot-zero kernel body, never an arbitrary
    // body-bearing function from a detached decoded module.
    let function = match module.functions.as_slice() {
        [function] => function,
        [function, trap]
            if trap.role == fe2o3_kernel_ir::FunctionRole::ExternalImport
                && trap.body.is_none()
                && trap.id.as_str() == "__fe2o3_ir_amdgpu_diagnostics_gfx942_v1_trap" =>
        {
            function
        }
        _ => return Err(Error::Unavailable("test source function roster")),
    };
    let [kernel] = module.kernels.as_slice() else {
        return Err(Error::Unavailable("test source kernel roster"));
    };
    assert_eq!(kernel.entry, function.id);
    let body = function
        .body
        .as_ref()
        .expect("actual defined canonical function");
    let op = emission.operation();
    let mut operations = 0;
    let mut selected = 0;
    for block in &body.blocks {
        operations += block.operations.len();
        budget.charge_work(block.operations.len())?;
        for actual in &block.operations {
            selected += usize::from(std::ptr::eq(actual, op));
        }
    }
    assert!(operations <= 1024);
    assert_eq!(
        selected, 1,
        "same live emission operation in the actual kernel root"
    );
    let OperationKind::Matrix(matrix) = &op.kind else {
        panic!("actual selected Matrix operation");
    };
    assert!(matches!(
        &matrix.kind,
        MatrixOperationKind::MultiplyAccumulate { .. }
    ));
    assert_eq!(matrix.active_lanes, 64);
    let mut inputs = [0; 12];
    let mut input_count = 0;
    op.visit_operands(|value| {
        assert!(input_count < inputs.len());
        inputs[input_count] = value.0;
        input_count += 1;
    });
    assert_eq!(input_count, 12);
    assert_eq!(op.results.len(), 4);
    let mut results = [0; 4];
    for (slot, value) in results.iter_mut().zip(&op.results) {
        *slot = value.id.0;
    }
    let mut producers = [[0; 3]; 6];
    let mut components = [[0; 4]; 6];
    let mut component_counts = [0; 6];
    let mut source_spans = [[0; 2]; 6];
    let mut raw_blocks = [0; 6];
    let mut semantic_blocks = [0; 6];
    for (index, role) in ROLES.into_iter().enumerate() {
        producers[index] = ssa(emission.producer_ssa(role));
        semantic_blocks[index] = emission.producer_block(role).index();
        raw_blocks[index] = view.raw_block(role);
        let span = view.source_span(role);
        assert!(!span.is_dummy() && !span.from_expansion());
        source_spans[index] = [span.lo().0, span.hi().0];
        let values = emission.producer_components(role);
        component_counts[index] = values.len();
        assert!(values.len() <= 4);
        for (slot, id) in components[index].iter_mut().zip(values) {
            *slot = id.0;
        }
    }
    assert_eq!(component_counts, [0, 1, 4, 4, 4, 4]);
    let mut consumed = [[0; 3]; 4];
    let mut consumed_components = [[0; 4]; 4];
    let mut consumed_counts = [0; 4];
    for index in 0..4 {
        consumed[index] = ssa(emission.consumed_ssa(index).expect("actual argument SSA"));
        let values = emission
            .consumed_components(index)
            .expect("actual argument components");
        assert!(values.len() <= 4);
        consumed_counts[index] = values.len();
        for (slot, id) in consumed_components[index].iter_mut().zip(values) {
            *slot = id.0;
        }
    }
    assert_eq!(consumed_counts, [0, 4, 4, 4]);
    assert_eq!(&inputs[..4], &consumed_components[1]);
    assert_eq!(&inputs[4..8], &consumed_components[2]);
    assert_eq!(&inputs[8..], &consumed_components[3]);
    assert_eq!(results, components[5]);
    let uses = emission.result_uses();
    let use_count = uses.len();
    assert!(use_count <= 128);
    let mut use_rows = [[None; 32]; 4];
    let mut use_counts = [0u32; 4];
    let mut use_hash = Sha256::new();
    use_hash.update(b"fe2o3.test.bf16.actual-result-uses.v1\0");
    for (index, row) in uses.enumerate() {
        // The fixed Snapshot coexistence reservation above covers these rows;
        // pay each borrowed row/copy/hash before observing it.
        budget.charge_work(20)?;
        let words = [
            row.block().0,
            row.operation().unwrap_or(u32::MAX),
            row.operand(),
            u32::from(row.component()),
        ];
        copy_use_row(&mut use_rows, &mut use_counts, index, words)?;
        for word in words {
            use_hash.update(word.to_le_bytes());
        }
    }
    let span = emission.terminator_span();
    let [launch] = owner.source_launch().roots() else {
        panic!("actual launch roster");
    };
    let exact = launch
        .source_launch()
        .exact_workgroup()
        .expect("actual exact WG");
    let grid = launch.source_launch().max_grid();
    assert_eq!(exact, [64, 1, 1]);
    assert_eq!(grid, [1, 1, 1]);
    assert!(!view.grants_artifact_or_launch_authority());
    assert!(!emission.grants_artifact_or_launch_authority());
    let result = Snapshot {
        source_sha256: *view.source().sha256(),
        mir_sha256: *view.mir_sha256(),
        semantic_sha256: *owner
            .semantic_ssa()
            .source_semantic()
            .semantic_sha256()
            .as_bytes(),
        canonical_sha256: Sha256::digest(canonical).into(),
        canonical_identity: *owner.executable().canonical().identity().digest(),
        source_bytes: view.source().bytes().len(),
        canonical_bytes: canonical.len(),
        source_blocks: emission.source_function().blocks().len(),
        canonical_blocks: body.blocks.len(),
        canonical_operations: operations,
        source_spans,
        raw_blocks,
        semantic_blocks,
        producers,
        consumed,
        components,
        component_counts,
        consumed_components,
        consumed_counts,
        matrix_inputs: inputs,
        matrix_results: results,
        matrix_span: [
            span.semantic_function().index(),
            span.semantic_block().index(),
            span.kernel_ir_block().0,
            span.first_operation_ordinal(),
            span.operation_count(),
        ],
        outside_uses: use_count,
        outside_uses_sha256: use_hash.finalize().into(),
        outside_uses_per_component: use_counts,
        unused_result_components: use_counts.map(|count| count == 0),
        outside_use_rows: use_rows,
        exact_workgroup: exact,
        max_grid: grid,
        callback_storage_before: storage_before,
        callback_storage_after: budget.storage(),
        callback_work_before: work_before,
        callback_work_after: budget.work(),
        // The selected LHS is actually a different SSA BlockArgument. The
        // lowerer's sealed resolver accepted its single-predecessor chain;
        // an unrelated context borrow or source-level move spelling is not
        // enough for this shape control.
        alias_observed: consumed[1][0] == 1 && consumed[1] != producers[2],
        pre_ranked_only: true,
        source_authority_in_copied_row: false,
    };
    assert_eq!(
        result.callback_storage_after - result.callback_storage_before,
        charge
    );
    Ok(result)
}

pub(super) fn observe<'tcx>(
    transaction: crate::production_pipeline::ProductionCompilation<
        'tcx,
        crate::production_pipeline::CollectedRustStage<'tcx>,
    >,
    case: &str,
) -> Result<Value, String> {
    let callback = Cell::new(None::<[usize; 4]>);
    let (result, phase) = transaction.observe_bf16_mfma_source_for_test_v1(|view, budget| {
        if matches!(case, "direct-error" | "direct-panic") {
            let before = [budget.storage(), budget.work()];
            budget.reserve_storage(CALLBACK_EXTRA)?;
            budget.charge_work(CALLBACK_WORK)?;
            callback.set(Some([
                before[0],
                before[1],
                budget.storage(),
                budget.work(),
            ]));
            if case == "direct-panic" {
                panic!("genuine BF16 source callback panic for accounting control");
            }
            return Err(Error::Unavailable(CALLBACK_REFUSAL));
        }
        snapshot(view, budget)
    });
    let diagnostic = result.as_ref().err().map(ToString::to_string);
    // Always retain the actual boundary/phase first. The outer child decides
    // whether this case qualified; malformed/optimized-away controls fail.
    let row = match result {
        Ok(snapshot) => {
            json!({"stage":"actual_pre_ranked_source_region","snapshot":snapshot,"phase":phase})
        }
        Err(_) => {
            json!({"stage":"actual_source_or_callback_refused","diagnostic":diagnostic,"phase":phase,
            "callback":callback.get()})
        }
    };
    Ok(row)
}

pub(super) fn accept(case: &str, row: &Value) -> Result<(), &'static str> {
    match case {
        "direct" | "single-predecessor" => {
            if row["stage"] != "actual_pre_ranked_source_region" {
                return Err("actual positive source inspection refused");
            }
            if row["phase"]["same_ledger"] != true
                || row["phase"]["result_ok"] != true
                || row["phase"]["failed_work"] != false
                || row["phase"]["failed_storage"] != false
            {
                return Err("actual positive phase accounting differs");
            }
            if case == "single-predecessor" && row["snapshot"]["alias_observed"] != true {
                return Err("source spelling did not produce an observed admitted SSA alias");
            }
        }
        "direct-error" | "direct-panic" => {
            if row["stage"] != "actual_source_or_callback_refused" {
                return Err("callback was not refused");
            }
            let diagnostic = row["diagnostic"]
                .as_str()
                .ok_or("actual callback diagnostic absent")?;
            let expected = if case == "direct-error" {
                CALLBACK_REFUSAL
            } else {
                "CallbackPanicked"
            };
            if !diagnostic.contains(expected) {
                return Err("wrong actual callback refusal boundary");
            }
            let values = row["callback"]
                .as_array()
                .ok_or("genuine callback not reached")?;
            if values.len() != 4 {
                return Err("callback observation shape");
            }
            let number = |i: usize| values[i].as_u64().ok_or("callback observation number");
            if number(2)?.checked_sub(number(0)?) != Some(CALLBACK_EXTRA as u64)
                || number(3)?.checked_sub(number(1)?) != Some(CALLBACK_WORK as u64)
            {
                return Err("callback did not consume exact original resources");
            }
            let phase = &row["phase"];
            let floor = phase["materializer_storage"]
                .as_u64()
                .ok_or("actual materializer entry observation absent")?;
            let source = phase["source_storage"]
                .as_u64()
                .ok_or("actual source reservation absent")?;
            let expected_final = floor
                .checked_sub(source)
                .and_then(|v| v.checked_add(CALLBACK_EXTRA as u64));
            if expected_final.is_none() {
                return Err("materializer/source floor relation");
            }
            if phase["same_ledger"] != true
                || phase["result_ok"] != false
                || phase["failed_work"] != false
                || phase["failed_storage"] != false
                || phase["final_storage"].as_u64() != expected_final
                || phase["work"]
                    .as_u64()
                    .is_none_or(|work| work < number(3).unwrap_or(u64::MAX))
            {
                return Err("callback cleanup changed original floor or consumed history");
            }
        }
        "wrong-launch" => {
            exact_refusal(row, "BF16 source requires explicit WG64 and one workgroup")?
        }
        // This first fixture has TWO actual producer calls. It qualifies only
        // this exact source census refusal, not a lowerer phi-edge refusal.
        "phi" => exact_refusal(row, "BF16 source nominal producer is duplicated or foreign")?,
        "loop-carried" => exact_refusal(
            row,
            "looping source unavailable for first inspection profile",
        )?,
        "retained-memory" => exact_refusal(row, "fragment origin is not an admitted call or move")?,
        _ => return Err("unknown source case"),
    }
    Ok(())
}
fn exact_refusal(row: &Value, expected: &str) -> Result<(), &'static str> {
    if row["stage"] != "actual_source_or_callback_refused"
        || row["diagnostic"]
            .as_str()
            .is_none_or(|v| !v.contains(expected))
    {
        return Err("wrong actual source rejection boundary");
    }
    Ok(())
}
#[test]
fn source_accounting_oracle_requires_callback_and_exact_original_floor() {
    let good = json!({"stage":"actual_source_or_callback_refused","diagnostic":CALLBACK_REFUSAL,"callback":[500,10,523,27],
        "phase":{"entry_storage":100,"source_storage":50,"materializer_storage":200,"final_storage":173,"work":27,"same_ledger":true,"result_ok":false,"failed_work":false,"failed_storage":false}});
    assert!(accept("direct-error", &good).is_ok());
    for (field, value) in [
        ("final_storage", json!(100)),
        ("work", json!(26)),
        ("same_ledger", json!(false)),
        ("result_ok", json!(true)),
    ] {
        let mut bad = good.clone();
        bad["phase"][field] = value;
        assert!(accept("direct-error", &bad).is_err());
    }
    let mut missing = good.clone();
    missing["callback"] = Value::Null;
    assert!(accept("direct-error", &missing).is_err());
    assert!(accept("direct-panic", &good).is_err());
}
#[test]
fn source_shape_oracle_never_counts_source_spelling_as_an_alias() {
    let direct = json!({"stage":"actual_pre_ranked_source_region","snapshot":{"alias_observed":false},
        "phase":{"same_ledger":true,"result_ok":true,"failed_work":false,"failed_storage":false}});
    assert!(accept("direct", &direct).is_ok());
    assert!(accept("single-predecessor", &direct).is_err());
    assert!(accept("retained-memory", &direct).is_err());
    for case in ["wrong-launch", "phi", "loop-carried", "retained-memory"] {
        assert!(accept(case,&json!({"stage":"actual_source_or_callback_refused","diagnostic":"unrelated compiler panic"})).is_err());
    }
}

#[test]
fn copied_use_rows_preserve_exact_coordinates_and_unused_components() {
    let mut rows = [[None; 32]; 4];
    let mut counts = [0; 4];
    copy_use_row(&mut rows, &mut counts, 0, [7, 31, 1, 0]).unwrap();
    copy_use_row(&mut rows, &mut counts, 1, [9, u32::MAX, 2, 0]).unwrap();
    assert_eq!(rows[0][0], Some([7, 31, 1, 0]));
    assert_eq!(rows[0][1], Some([9, u32::MAX, 2, 0]));
    assert_eq!(counts, [2, 0, 0, 0]);
    assert_eq!(counts.map(|count| count == 0), [false, true, true, true]);
    assert!(rows.iter().flatten().skip(2).all(Option::is_none));
}
#[test]
fn copied_use_rows_exact_capacity_and_component_limits_refuse_before_mutation() {
    let mut rows = [[None; 32]; 4];
    let mut counts = [0; 4];
    for index in 0..128 {
        copy_use_row(
            &mut rows,
            &mut counts,
            index,
            [7, index as u32, 1, (index % 4) as u32],
        )
        .unwrap();
    }
    assert_eq!(counts, [32; 4]);
    let before = rows;
    for (index, words) in [(128, [7, 0, 1, 0]), (0, [7, 0, 1, 4]), (0, [7, 0, 1, 0])] {
        assert!(copy_use_row(&mut rows, &mut counts, index, words).is_err());
        assert_eq!(rows, before);
        assert_eq!(counts, [32; 4]);
    }
}
#[test]
fn copied_empty_use_roster_is_all_unused_with_no_synthetic_rows() {
    let rows: [[Option<[u32; 4]>; 32]; 4] = [[None; 32]; 4];
    let counts = [0u32; 4];
    assert!(rows.iter().flatten().all(Option::is_none));
    assert_eq!(counts.map(|count| count == 0), [true; 4]);
}

#[path = "gfx942_tiled_region_normal_v1_tests.rs"]
pub(super) mod normal;

// Separate genuine-source numerical gate; historical inspection/normal modes are unchanged.
#[path = "gfx942_tiled_region_cpu_v1_tests.rs"]
pub(in crate::production_rustc_driver_v1) mod cpu;
