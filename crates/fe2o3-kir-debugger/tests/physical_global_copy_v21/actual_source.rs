//! Root-run actual-byte qualification only. Decoding bytes does not recover source custody.
use super::*;
use std::fs::OpenOptions;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::PathBuf;

const INPUT_MAX: usize = 128 * 1024;
const CALLER_FLOOR: usize = 73 + INPUT_MAX + 16 * 1024;
const PREFIX: &str = "FE2O3_GLOBAL_COPY_DEBUG_V21 ";
fn ledger(work: usize, storage: usize, bytes: usize) -> Owned {
    let mut b = Owned::new(Work::new(work), storage);
    b.with_budget(|b| {
        b.reserve_storage(CALLER_FLOOR)?;
        b.charge_work(11 + bytes)
    })
    .unwrap();
    b
}
fn read_input() -> Vec<u8> {
    let p =
        PathBuf::from(std::env::var_os("FE2O3_V21_DEBUG_KIR").expect("explicit canonical input"));
    assert!(p.is_absolute());
    // Linux O_NOFOLLOW | O_NONBLOCK; never wait on a pipe or follow a final symlink.
    let mut f = OpenOptions::new()
        .read(true)
        .custom_flags(0x20000 | 0x800)
        .open(p)
        .unwrap();
    let before = f.metadata().unwrap();
    assert!(before.is_file() && before.len() > 0 && before.len() <= INPUT_MAX as u64);
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(before.len() as usize).unwrap();
    assert_eq!(bytes.capacity(), before.len() as usize);
    bytes.resize(before.len() as usize, 0);
    f.read_exact(&mut bytes).unwrap();
    assert_eq!(f.read(&mut [0]).unwrap(), 0);
    let after = f.metadata().unwrap();
    assert_eq!(
        (
            before.dev(),
            before.ino(),
            before.len(),
            before.mtime(),
            before.mtime_nsec()
        ),
        (
            after.dev(),
            after.ino(),
            after.len(),
            after.mtime(),
            after.mtime_nsec()
        )
    );
    bytes
}
fn admit(bytes: &[u8], mut b: Owned) -> (Owner, AdmittedSimulationModuleV1, Owned) {
    let (owner, receipt) = b
        .with_budget(|b| Owner::from_canonical_bytes_with_verification_budget_v21(bytes, b))
        .unwrap();
    b.with_budget(|b| b.reserve_storage(receipt.retained_storage()))
        .unwrap();
    let (module, receipt) = b
        .with_budget(|b| {
            AdmittedSimulationModuleV1::admit_v21_with_verification_budget(&owner, limits(), b)
        })
        .unwrap();
    b.with_budget(|b| b.reserve_storage(receipt.retained_storage()))
        .unwrap();
    assert_eq!(owner.canonical_bytes(), bytes);
    assert_eq!(module.module(), owner.module());
    assert_eq!(module.identity().digest(), owner.identity().digest());
    (owner, module, b)
}
fn req(
    entry: &str,
    grid: u64,
    input: usize,
    output: usize,
    uninitialized: Option<usize>,
) -> SimulationRequestV1 {
    let mut r = request(input, output, uninitialized);
    r.kernel = entry.into();
    r.grid = GridShapeV1([grid, 1, 1]);
    r
}
fn locate(s: &Session, op: u32, phase: SimulationDebugCheckpointPhaseV1) -> usize {
    (0..s.records_len())
        .find(|&n| {
            let r = s.record(n).unwrap();
            r.invocation().global == [0, 0, 0]
                && r.site().operation == op
                && r.phase() == Some(phase)
        })
        .unwrap()
}
fn final_memory(s: &Session, r: &SimulationRequestV1, grid: usize) -> usize {
    let index = (0..s.records_len())
        .rev()
        .find(|&n| s.record(n).unwrap().phase().is_some())
        .unwrap();
    let last = s.record(index).unwrap();
    assert_eq!(last.invocation().global, [grid as u64 - 1, 0, 0]);
    assert_eq!(last.memory_allocation(2), None);
    for allocation in 0..2 {
        let original = &r.shared_buffers[allocation].buffer;
        assert_eq!(
            last.memory_allocation(allocation).unwrap().1,
            original.bytes().len()
        );
        for i in 0..original.bytes().len() {
            let written = allocation == 1 && (8..8 + grid * 4).contains(&i);
            let byte = if written {
                word((i - 8) / 4).to_le_bytes()[i % 4]
            } else {
                original.bytes()[i]
            };
            assert_eq!(
                last.memory_byte_at(allocation, i),
                Some((byte, written || original.initialized()[i]))
            );
        }
    }
    index
}
fn restored(b: Owned, usage: PhysicalGlobalCopyDebugUsageV21) {
    assert_eq!(b.storage(), usage.entry_storage);
    assert_eq!(b.work(), usage.work);
    assert_eq!(b.peak_storage(), usage.peak_storage);
    assert_eq!(b.failed_work(), usage.failed_work);
    assert_eq!(b.failed_storage(), usage.failed_storage);
}
fn negative_requests(
    owner: &Owner,
    module: &AdmittedSimulationModuleV1,
    mut b: Owned,
    entry: &str,
    grid: u64,
) -> Owned {
    for (r, uninitialized) in [
        (req(entry, grid, 128, 0, Some(grid as usize - 1)), true),
        (req(entry, grid, grid as usize - 1, 0, None), false),
    ] {
        let Err(SimulationErrorV1::Execution(error)) =
            module.simulate(&r, SimulationTargetV1::amdgpu_64(), limits())
        else {
            panic!("exact read refusal")
        };
        if uninitialized {
            assert!(matches!(
                error.kind,
                SimulationExecutionErrorKindV1::UninitializedRead { .. }
            ));
        } else {
            assert!(matches!(
                error.kind,
                SimulationExecutionErrorKindV1::OutOfBounds { .. }
            ));
        }
        let s = Session::capture(module, owner, &r, options(8192), b);
        assert_eq!(
            s.outcome(),
            PhysicalGlobalCopyDebugOutcomeV21::ExecutionFailed
        );
        assert_eq!(s.capture_error(), None);
        assert_eq!(s.capture_stop(), None);
        assert_eq!(
            (0..s.records_len())
                .filter(|&n| s.record(n).unwrap().is_committed_store())
                .count(),
            0
        );
        let u = s.usage();
        b = s.into_budget();
        assert_eq!(b.storage(), u.entry_storage);
        assert_eq!(b.work(), u.work);
    }
    for offset in [8, 520] {
        let mut r = req(entry, grid, 128, 129, None);
        r.shared_buffers[0].buffer = BufferArgumentV1::new(
            ScalarType::U32,
            AccessMode::ReadWrite,
            4,
            vec![0; 2048],
            vec![true; 2048],
            SimulationTargetV1::amdgpu_64(),
        )
        .unwrap();
        r.arguments[1] = SimulationArgumentV1::BufferView(
            BufferViewArgumentV1::new(
                BufferBackingIdV1(7),
                ScalarType::U32,
                AccessMode::ReadWrite,
                4,
                offset,
                129,
                SimulationTargetV1::amdgpu_64(),
            )
            .unwrap(),
        );
        assert_eq!(8 + 128 * 4 <= offset, offset == 520);
        assert!(matches!(
            module.preflight(&r, SimulationTargetV1::amdgpu_64(), limits()),
            Err(SimulationPreflightErrorV1::PhysicalGlobalCopyAliasedArgumentsV21)
        ));
        let s = Session::capture(module, owner, &r, options(8192), b);
        assert_eq!(
            s.outcome(),
            PhysicalGlobalCopyDebugOutcomeV21::PreflightRefused
        );
        assert_eq!(s.records_len(), 0);
        let u = s.usage();
        b = s.into_budget();
        assert_eq!(b.storage(), u.entry_storage);
        assert_eq!(b.work(), u.work);
    }
    b
}
fn resource_controls(bytes: &[u8], entry: &str, grid: u64, work: usize, peak: usize) {
    for mode in 0..3 {
        let (owner, module, b) = admit(
            bytes,
            ledger(
                work - usize::from(mode == 1),
                peak - usize::from(mode == 2),
                bytes.len(),
            ),
        );
        let s = Session::capture(
            &module,
            &owner,
            &req(entry, grid, 128, 129, None),
            options(8192),
            b,
        );
        assert_eq!(s.outcome(), PhysicalGlobalCopyDebugOutcomeV21::Completed);
        assert_eq!(s.capture_error(), None);
        if mode == 0 {
            assert_eq!(s.capture_stop(), None);
            assert_eq!(s.usage().work, work);
            assert_eq!(s.usage().peak_storage, peak);
        } else {
            assert!(matches!(
                s.capture_stop(),
                Some(PhysicalGlobalCopyDebugCaptureStopV21::Resource(_))
            ));
        }
        if mode == 1 {
            assert!(s.usage().failed_work.is_some())
        }
        if mode == 2 {
            assert!(s.usage().failed_storage.is_some())
        }
        let u = s.usage();
        restored(s.into_budget(), u);
    }
}
#[test]
#[ignore = "root supplies pinned actual source exports; no fixture/source-owner minting"]
fn supplied_actual_source_bytes_capture_copy_and_pending_readiness() {
    // Prepay the bounded caller-owned input/request/report envelope before read allocation.
    let mut initial = ledger(WORK, STORAGE, 0);
    let bytes = read_input();
    initial.with_budget(|b| b.charge_work(bytes.len())).unwrap();
    let entry = std::env::var("FE2O3_V21_DEBUG_ENTRY").expect("exact source entry");
    assert!(matches!(
        entry.as_str(),
        "physical_global_copy_one" | "physical_global_copy_registers"
    ));
    let grid = std::env::var("FE2O3_V21_DEBUG_GRID")
        .unwrap()
        .parse::<u64>()
        .unwrap();
    assert!(matches!(grid, 64 | 128));
    let (owner, module, b) = admit(&bytes, initial);
    assert_eq!(owner.module().kernels.len(), 1);
    assert_eq!(
        owner.module().kernels[0].id,
        fe2o3_kernel_ir::KernelId::from(entry.as_str())
    );
    let floor = b.storage();
    let r = req(&entry, grid, 128, 129, None);
    let unchanged = r.clone();
    let mut s = Session::capture(&module, &owner, &r, options(8192), b);
    assert_eq!(s.outcome(), PhysicalGlobalCopyDebugOutcomeV21::Completed);
    assert_eq!(s.capture_error(), None);
    assert_eq!(s.capture_stop(), None);
    assert!(s.records_len() <= 8192);
    assert_eq!(r, unchanged);
    let captured = s.usage();
    let records = s.records_len();
    let final_index = final_memory(&s, &r, grid as usize);
    let (load, loaded) = site(owner.module(), Op::GlobalLoadDword);
    let OperationKind::Gfx942PhysicalGlobalCopyStep(load_step) =
        &owner.module().functions[0].body.as_ref().unwrap().blocks[0].operations[load as usize]
            .kind
    else {
        panic!("actual load step")
    };
    let loaded_register = load_step.instruction.destination;
    let (store, _) = site(owner.module(), Op::GlobalStoreDword);
    use SimulationDebugCheckpointPhaseV1::{AfterOperation as After, BeforeOperation as Before};
    let pending = locate(&s, load, After);
    let ready = locate(&s, load + 1, After);
    let before = locate(&s, store, Before);
    let after = locate(&s, store, After);
    assert!(binding(s.record(locate(&s, load, Before)).unwrap(), loaded).is_none());
    for index in [pending, locate(&s, load + 1, Before)] {
        let b = binding(s.record(index).unwrap(), loaded).unwrap();
        assert_eq!(
            b.symbolic_kind(),
            Some(PhysicalGlobalCopyDebugSymbolicKindV21::PendingGlobalRead)
        );
        assert_eq!(b.scalar(), None);
        assert_eq!(b.logical_pointer(), None);
    }
    assert_eq!(
        binding(s.record(ready).unwrap(), loaded).unwrap().scalar(),
        Some(ScalarBitsV1::u32(word(0)))
    );
    for index in [ready, pending, ready] {
        assert!(matches!(s.seek(index), Nav::Record { .. }));
        assert_eq!(
            binding(s.current().unwrap(), loaded)
                .unwrap()
                .scalar()
                .is_some(),
            index == ready
        );
    }
    for (index, written) in [
        (before, false),
        (after, true),
        (before, false),
        (after, true),
    ] {
        assert!(matches!(s.seek(index), Nav::Record { .. }));
        let value = if written {
            word(0).to_le_bytes()[0]
        } else {
            0xa5
        };
        assert_eq!(
            s.current().unwrap().memory_byte_at(1, 8),
            Some((value, written))
        );
        assert_eq!(
            s.current().unwrap().memory_byte_at(0, 8),
            Some((word(0).to_le_bytes()[0], true))
        );
    }
    assert!(matches!(s.step_reverse(), Nav::Record { .. }));
    assert!(s.current().unwrap().is_committed_store());
    assert!(matches!(s.step_reverse(),Nav::Record{index,..}if index==before));
    assert!(matches!(s.step_forward(), Nav::Record { .. }));
    let cursor = s.cursor();
    assert_eq!(s.seek(records + 1), Nav::Unavailable);
    assert_eq!(s.cursor(), cursor);
    // Stable old indexes remain valid; this library has no revision/page token protocol.
    assert!(matches!(s.seek(pending), Nav::Record { .. }));
    assert_eq!(s.cursor(), Some(pending));
    let ready_count = (0..records)
        .filter_map(|n| s.record(n))
        .filter(|r| r.phase() == Some(After) && r.site().operation == load + 1)
        .filter(|r| {
            binding(*r, loaded).unwrap().scalar()
                == Some(ScalarBitsV1::u32(word(r.invocation().global[0] as usize)))
        })
        .count();
    assert_eq!(ready_count, grid as usize);
    assert_eq!(
        (0..records)
            .filter(|&n| s.record(n).unwrap().is_committed_store())
            .count(),
        grid as usize
    );
    for row in (0..records).filter_map(|n| s.record(n)) {
        for value in (0..row.binding_count(0).unwrap_or(0)).filter_map(|i| row.binding(0, i)) {
            if value.symbolic_kind().is_some() {
                assert_eq!(value.scalar(), None);
                assert_eq!(value.logical_pointer(), None);
            }
        }
    }
    let usage = s.usage();
    let b = s.into_budget();
    assert_eq!(b.storage(), floor);
    assert_eq!(b.work(), usage.work);
    let mut b = negative_requests(&owner, &module, b, &entry, grid);
    assert_eq!(b.storage(), floor);
    assert!(b.work() > usage.work);
    // Capture once more on the same cumulative ledger, then deliberately exhaust
    // query work. Failed navigation preserves its cursor and the original floor.
    let mut s = Session::capture(&module, &owner, &r, options(8192), b);
    assert_eq!(s.capture_stop(), None);
    assert!(matches!(s.seek(0), Nav::Record { .. }));
    s.charge_query_work(WORK - s.usage().work).unwrap();
    assert_eq!(s.step_forward(), Nav::Unavailable);
    assert_eq!(s.cursor(), Some(0));
    assert!(s.usage().failed_work.is_some());
    b = s.into_budget();
    assert_eq!(b.storage(), floor);
    assert_eq!(b.work(), WORK);
    assert!(b.failed_work().is_some());
    resource_controls(&bytes, &entry, grid, captured.work, captured.peak_storage);
    let digest = owner
        .identity()
        .digest()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    println!(
        "{PREFIX}{{\"schema\":\"fe2o3-actual-source-global-copy-cpu-capture-v21\",\"entry\":\"{entry}\",\"grid\":{grid},\"canonical_identity\":\"{digest}\",\"canonical_bytes\":{},\"records\":{records},\"final_checkpoint_index\":{final_index},\"pending_index\":{pending},\"ready_index\":{ready},\"before_store_index\":{before},\"after_store_index\":{after},\"loaded_value_id\":{},\"loaded_register\":{loaded_register},\"load_operation\":{load},\"store_operation\":{store},\"ready_reads\":{ready_count},\"written_words\":{grid},\"input_allocation_bytes\":528,\"output_allocation_bytes\":532,\"unchanged_output_bytes\":{},\"capture_work\":{},\"capture_peak\":{},\"entry_floor\":{floor},\"negative_requests\":4,\"resource_denials\":2,\"invalid_navigation_controls\":2,\"original_ledger_floor_restored\":true,\"revision_tokens\":\"not_in_library_api\",\"source_custody\":false,\"hardware_observed\":false,\"runtime_authority\":false,\"protected_authority\":false,\"resumable_execution\":false}}",
        bytes.len(),
        loaded.0,
        532 - grid * 4,
        captured.work,
        captured.peak_storage
    );
}
