//! Root-run test-only locator over actual typed loader/capture; no ISA oracle.
use super::*;
use fe2o3_kernel_ir::{Gfx942PhysicalGlobalCopyOpcodeV1 as Op, OperationKind};
use fe2o3_kir_sim::{
    PhysicalGlobalCopyDebugSymbolicKindV21 as Symbolic, SimulationDebugCheckpointPhaseV1 as Phase,
};

#[test]
#[ignore = "root supplies exact actual-source canonical and CPU request files"]
fn supplied_actual_v21_input_reports_public_capture_locations() {
    let kir = PathBuf::from(std::env::var_os("FE2O3_V21_DEBUG_KIR").expect("actual canonical"));
    let request =
        PathBuf::from(std::env::var_os("FE2O3_V21_DEBUG_REQUEST").expect("exact request"));
    assert!(kir.is_absolute() && request.is_absolute());
    let mut ledger = Owned::new(Work::new(WORK), STORAGE);
    ledger
        .with_budget(|b| {
            b.reserve_storage(SCRATCH)?;
            b.charge_work(RESPONSE * 2)
        })
        .unwrap();
    let (input, receipt) = ledger
        .with_budget(|b| load_physical_global_copy_debug_input_v21(&kir, &request, b))
        .unwrap();
    ledger
        .with_budget(|b| b.reserve_storage(receipt.retained_storage()))
        .unwrap();
    let b = capture(&input, ledger).unwrap();
    assert!(b.session.capture_stop().is_none());
    assert!(b.session.capture_error().is_none());
    assert_eq!(b.session.outcome(), PhysicalEntryDebugOutcomeV20::Completed);
    assert!(b.session.records_len() <= RECORDS);
    let module = input.canonical().module();
    assert_eq!(module.functions.len(), 1);
    assert_eq!(module.kernels.len(), 1);
    let body = module.functions[0].body.as_ref().unwrap();
    assert_eq!(body.blocks.len(), 1);
    let block = &body.blocks[0];
    let declarations = block
        .operations
        .iter()
        .filter_map(|operation| {
            if let OperationKind::Gfx942PhysicalGlobalCopyDeclaration(d) = &operation.kind {
                Some(d.parameters)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(declarations.len(), 1);
    let find_op = |opcode| {
        let found = block
            .operations
            .iter()
            .enumerate()
            .filter_map(|(i, operation)| {
                matches!(&operation.kind, OperationKind::Gfx942PhysicalGlobalCopyStep(s)
                if s.instruction.opcode == opcode)
                .then_some(i)
            })
            .collect::<Vec<_>>();
        if opcode != Op::WaitVm0 {
            assert_eq!(found.len(), 1);
        }
        found
    };
    let load = find_op(Op::GlobalLoadDword)[0];
    let store = find_op(Op::GlobalStoreDword)[0];
    let waits = find_op(Op::WaitVm0);
    let ready = *waits.iter().find(|&&i| i > load && i < store).unwrap();
    assert_eq!(ready, load + 1);
    assert_eq!(block.operations[load].results.len(), 1);
    let loaded = block.operations[load].results[0].id;
    let locate = |operation: usize, phase| {
        let found = (0..b.session.records_len())
            .filter(|&index| {
                let r = b.session.record(index).unwrap();
                r.invocation().global == [0, 0, 0]
                    && r.site().block == block.id
                    && r.site().operation == operation as u32
                    && r.phase() == Some(phase)
            })
            .collect::<Vec<_>>();
        assert_eq!(found.len(), 1);
        found[0]
    };
    let load_before = locate(load, Phase::BeforeOperation);
    let pending = locate(load, Phase::AfterOperation);
    let wait_before = locate(ready, Phase::BeforeOperation);
    let ready_index = locate(ready, Phase::AfterOperation);
    let store_before = locate(store, Phase::BeforeOperation);
    let store_after = locate(store, Phase::AfterOperation);
    for (index, is_pending) in [(pending, true), (ready_index, false)] {
        let r = b.session.record(index).unwrap();
        let binding = (0..r.binding_count(0).unwrap())
            .find_map(|n| {
                let value = r.binding(0, n).unwrap();
                (value.value() == loaded).then_some(value)
            })
            .unwrap();
        if is_pending {
            assert_eq!(binding.symbolic_kind(), Some(Symbolic::PendingGlobalRead));
            assert_eq!(binding.scalar(), None);
        } else {
            assert_eq!(binding.symbolic_kind(), None);
            assert!(binding.scalar().is_some());
        }
    }
    let record = b.session.record(load_before).unwrap();
    let allocations = declarations[0]
        .iter()
        .map(|parameter| {
            let binding = (0..record.binding_count(0).unwrap())
                .find_map(|n| {
                    let value = record.binding(0, n).unwrap();
                    (value.value() == *parameter).then_some(value)
                })
                .unwrap();
            let (id, offset) = binding.logical_pointer().unwrap();
            let (_, bytes) = (0..2)
                .filter_map(|n| record.memory_allocation(n))
                .find(|(allocation, _)| *allocation == id)
                .unwrap();
            serde_json::json!({"parameter":parameter.0,"id":id,"offset":offset,"bytes":bytes})
        })
        .collect::<Vec<_>>();
    assert_eq!(record.memory_allocation(2), None);
    let final_index = (0..b.session.records_len())
        .rev()
        .find(|&index| b.session.record(index).unwrap().phase().is_some())
        .unwrap();
    let checkpoint = |index: usize, operation: usize, phase: &str| serde_json::json!({"index":index,"block":block.id.0,"operation":operation,"phase":phase});
    let observation = serde_json::json!({
        "schema":"fe2o3-physical-global-copy-public-cli-locations-v21",
        "canonical_identity":hex_bytes(input.canonical().identity().digest()),
        "canonical_bytes":input.canonical().identity().canonical_length(),
        "request_sha256":hex_bytes(input.request_digest()),
        "records":b.session.records_len(),"loaded_value_id":loaded.0,"allocations":allocations,
        "load_before":checkpoint(load_before,load,"before_operation"),
        "pending":checkpoint(pending,load,"after_operation"),
        "wait_before":checkpoint(wait_before,ready,"before_operation"),
        "ready":checkpoint(ready_index,ready,"after_operation"),
        "store_before":checkpoint(store_before,store,"before_operation"),
        "store_after":checkpoint(store_after,store,"after_operation"),
        "final_checkpoint_index":final_index,
        "source_custody":false,"hardware_observed":false,"runtime_authority":false,
        "protected_authority":false,"resumable_execution":false
    });
    let mut ledger = b.session.into_budget();
    assert_eq!(ledger.storage(), SCRATCH + receipt.retained_storage());
    drop(input);
    ledger
        .with_budget(|b| b.release_storage(SCRATCH + receipt.retained_storage()))
        .unwrap();
    assert_eq!(ledger.storage(), 0);
    let encoded = serde_json::to_string(&observation).unwrap();
    assert!(encoded.len() <= 8192);
    println!("FE2O3_GLOBAL_COPY_CLI_LOCATIONS_V21 {encoded}");
}
