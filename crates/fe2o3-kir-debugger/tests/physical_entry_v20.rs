//! Observational navigation of actual Engine snapshots, not interpreter resume.
use fe2o3_kernel_ir as physical_entry_fixture_ir;
use fe2o3_kernel_ir::{
    AccessMode, CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    Gfx942PhysicalEntryOpcodeV20 as Op, OperationKind, ScalarType,
    VerifiedCanonicalKernelIrModuleV20 as Owner,
};
use fe2o3_kir_debugger::{
    PhysicalEntryDebugNavigationV20 as Navigation, PhysicalEntryDebugSessionV20 as Session,
};
use fe2o3_kir_sim::*;
#[path = "../../fe2o3-kernel-ir/tests/fixtures/physical_entry_v20.rs"]
mod fixture;
fn session(select: bool, selector: u32, max_records: usize) -> Session {
    session_with_work(select, selector, max_records, 100_000_000)
}
fn session_with_work(
    select: bool,
    selector: u32,
    max_records: usize,
    work_limit: usize,
) -> Session {
    let module = fixture::module(select);
    let mut work = Work::new(16_000_000);
    let (owner, receipt) = Owner::from_module_ref_with_verification_budget_v20(
        &module,
        &mut Budget::new(&mut work, 16_000_000),
    )
    .unwrap();
    let limits = SimulationLimitsV1 {
        max_memory_access_records: 1024,
        max_allocations: 4,
        max_call_depth: 1,
        max_ssa_values: 160,
        max_total_bytes: 8192,
        ..SimulationLimitsV1::default()
    };
    let admitted = AdmittedSimulationModuleV1::admit_v20(&owner, limits).unwrap();
    let buffer = BufferArgumentV1::new(
        ScalarType::U32,
        AccessMode::ReadWrite,
        4,
        vec![0xa5; 256],
        vec![false; 256],
        SimulationTargetV1::amdgpu_64(),
    )
    .unwrap();
    let request = SimulationRequestV1::new(
        "physical_fixture",
        [64, 1, 1],
        [64, 1, 1],
        vec![
            SimulationArgumentV1::Buffer(buffer),
            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(19)),
            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(23)),
            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(42)),
            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(selector)),
        ],
    );
    let mut ledger = Owned::new(Work::new(work_limit), 512 * 1024 * 1024);
    ledger
        .with_budget(|b| {
            b.reserve_storage(73 + receipt.retained_storage() + admitted.admitted_resident_bytes())
        })
        .unwrap();
    Session::capture(
        &admitted,
        &owner,
        &request,
        PhysicalEntryDebugOptionsV20::new(
            limits,
            SimulationDebugCaptureLimitsV1::new(1, 160, 4, 8192).unwrap(),
            max_records,
        )
        .unwrap(),
        ledger,
    )
}
#[test]
fn before_store_after_store_reverse_forward_preserve_same_checkpoint_memory() {
    for (select, selector, expected) in [(false, 0, 19), (true, 0, 19), (true, 1, 23)] {
        let mut session = session(select, selector, 4096);
        assert_eq!(session.outcome(), PhysicalEntryDebugOutcomeV20::Completed);
        let module = fixture::module(select);
        let body = module.functions[0].body.as_ref().unwrap();
        let (block,operation)=body.blocks.iter().find_map(|b|b.operations.iter().enumerate().find_map(|(i,o)|
            matches!(&o.kind,OperationKind::Gfx942PhysicalEntryStep(step) if step.instruction.opcode==Op::GlobalStoreDword)
                .then_some((b.id,i as u32)))).unwrap();
        let mut before = None;
        let mut after = None;
        for index in 0..session.records_len() {
            assert!(matches!(session.seek(index), Navigation::Record { .. }));
            let record = session.current().unwrap();
            if record.site().block == block
                && record.site().operation == operation
                && record.invocation().global[0] == 0
            {
                match record.phase() {
                    Some(SimulationDebugCheckpointPhaseV1::BeforeOperation) => before = Some(index),
                    Some(SimulationDebugCheckpointPhaseV1::AfterOperation) => after = Some(index),
                    _ => {}
                }
            }
        }
        let before = before.unwrap();
        let after = after.unwrap();
        for (index, byte, initialized) in [
            (before, 0xa5, false),
            (after, expected, true),
            (before, 0xa5, false),
            (after, expected, true),
        ] {
            session.seek(index);
            let record = session.current().unwrap();
            let (_allocation, _) = record.memory_allocation(0).unwrap();
            assert_eq!(record.memory_byte_at(0, 0), Some((byte, initialized)));
        }
        session.seek(after);
        assert!(matches!(session.step_reverse(), Navigation::Record { .. }));
        assert!(session.current().unwrap().is_committed_store());
        assert!(matches!(session.step_reverse(),Navigation::Record {index,..} if index==before));
        assert!(matches!(session.step_forward(), Navigation::Record { .. }));
        assert!(matches!(session.step_forward(),Navigation::Record {index,..} if index==after));
        let floor = session.usage().entry_storage;
        let work = session.usage().work;
        let ledger = session.into_budget();
        assert_eq!(ledger.storage(), floor);
        assert_eq!(ledger.work(), work);
    }
}
#[test]
fn capture_cutoff_and_invalid_cursor_never_claim_complete_or_move_cursor() {
    let mut session = session(false, 0, 7);
    assert_eq!(session.records_len(), 7);
    assert!(matches!(session.seek(6), Navigation::Record { .. }));
    assert_eq!(session.seek(7), Navigation::Incomplete);
    assert_eq!(session.cursor(), Some(6));
    assert_eq!(session.seek(8), Navigation::Unavailable);
    assert_eq!(session.cursor(), Some(6));
    session.seek(0);
    assert_eq!(session.step_reverse(), Navigation::Beginning);
    assert_eq!(session.cursor(), None);
}

#[test]
fn exact_navigation_budget_and_one_below_preserve_cursor_floor_and_denial() {
    let baseline = session(false, 0, 4096);
    assert_eq!(baseline.outcome(), PhysicalEntryDebugOutcomeV20::Completed);
    let work = baseline.usage().work;
    let mut exact = session_with_work(false, 0, 4096, work + 1);
    assert!(matches!(exact.seek(0), Navigation::Record { index: 0, .. }));
    let before = exact.usage();
    assert_eq!(exact.step_forward(), Navigation::Unavailable);
    assert_eq!(exact.cursor(), Some(0));
    assert_eq!(exact.usage().work, before.work);
    assert!(exact.usage().failed_work.is_some());
    let floor = exact.usage().entry_storage;
    let failed = exact.usage().failed_work;
    let returned = exact.into_budget();
    assert_eq!(returned.storage(), floor);
    assert_eq!(returned.failed_work(), failed);
    let mut short = session_with_work(false, 0, 4096, work);
    assert_eq!(short.outcome(), PhysicalEntryDebugOutcomeV20::Completed);
    assert_eq!(short.seek(0), Navigation::Unavailable);
    assert_eq!(short.cursor(), None);
}

#[test]
fn session_inline_cursor_fits_the_prepaid_capture_header_reserve() {
    assert!(
        std::mem::size_of::<Session>()
            <= std::mem::size_of::<PhysicalEntryDebugCaptureV20>()
                + std::mem::size_of::<Option<usize>>()
    );
}
