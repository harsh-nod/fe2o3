//! Inert shared KIR fixture through the real typed loader/Engine/JSONL adapter.
//! These controls do not mint or qualify source, native or hardware custody.
use super::*;
use fe2o3_kernel_ir as physical_global_copy_fixture_ir;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    Gfx942PhysicalGlobalCopyOpcodeV1 as Op, OperationKind, encode_module_v21,
};
use fe2o3_kir_sim::{
    PhysicalGlobalCopyDebugSymbolicKindV21 as Symbolic, SimulationDebugCheckpointPhaseV1 as Phase,
};
use fe2o3_kir_sim_cli::PhysicalGlobalCopyDebugInputErrorV21 as InputError;
use std::sync::atomic::{AtomicU64, Ordering};
#[path = "../../fe2o3-kernel-ir/tests/fixtures/physical_global_copy_v21.rs"]
mod fixture;
static NEXT: AtomicU64 = AtomicU64::new(1);
struct Files {
    directory: PathBuf,
    kir: PathBuf,
    request: PathBuf,
}
fn word(index: usize) -> u32 {
    (index as u32)
        .wrapping_mul(0x9e37_79b9)
        .wrapping_add(0x8000_0001)
}
fn document(grid: u32) -> serde_json::Value {
    let input: Vec<u8> = (0..128).flat_map(|i| word(i).to_le_bytes()).collect();
    serde_json::json!({
        "schema":"fe2o3-simulation-request-v1","kernel":"physical_global_copy_fixture",
        "grid":[grid,1,1],"workgroup":[64,1,1],
        "arguments":[
            {"kind":"buffer","element":"u32","access":"read_only","alignment":4,
                "bytes":hex_bytes(&input),"initialized":format!("0x{}","ff".repeat(64))},
            {"kind":"buffer","element":"u32","access":"read_write","alignment":4,
                "bytes":format!("0x{}","a5".repeat(516)),"initialized":format!("0x{}","00".repeat(65))}
        ]
    })
}
impl Files {
    fn new(grid: u32, edited: bool) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "fe2o3-v21-debug-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).unwrap();
        let kir = directory.join("canonical.kir");
        let request = directory.join("request.json");
        let mut module = fixture::module();
        if edited {
            for operation in &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations {
                if let OperationKind::Gfx942PhysicalGlobalCopyStep(step) = &mut operation.kind {
                    match step.instruction.opcode {
                        Op::GlobalLoadDword => step.instruction.destination = 22,
                        Op::GlobalStoreDword => step.instruction.source1 = 22,
                        _ => {}
                    }
                }
            }
        }
        std::fs::write(&kir, encode_module_v21(&module).unwrap()).unwrap();
        let files = Self {
            directory,
            kir,
            request,
        };
        files.set_request(&document(grid));
        files
    }
    fn set_request(&self, value: &serde_json::Value) {
        std::fs::write(&self.request, serde_json::to_vec(value).unwrap()).unwrap();
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.kir);
        let _ = std::fs::remove_file(&self.request);
        let _ = std::fs::remove_dir(&self.directory);
    }
}
fn load(files: &Files) -> (CopyInput, Owned) {
    let mut ledger = Owned::new(Work::new(WORK), STORAGE);
    ledger
        .with_budget(|b| {
            b.reserve_storage(73 + SCRATCH)?;
            b.charge_work(29 + RESPONSE * 2)
        })
        .unwrap();
    let (input, receipt) = ledger
        .with_budget(|b| load_physical_global_copy_debug_input_v21(&files.kir, &files.request, b))
        .unwrap();
    ledger
        .with_budget(|b| b.reserve_storage(receipt.retained_storage()))
        .unwrap();
    assert_eq!(
        input.canonical().identity().digest(),
        input.module().identity().digest()
    );
    assert_eq!(
        *input.request_digest(),
        <[u8; 32]>::from(Sha256::digest(std::fs::read(&files.request).unwrap()))
    );
    (input, ledger)
}
fn backend(files: &Files) -> (CopyInput, Backend<CopySession>) {
    let (input, ledger) = load(files);
    let b = capture(&input, ledger).unwrap();
    (input, b)
}
fn response(b: &mut Backend<CopySession>, request: DebugRequestV1) -> DebugResponseV1 {
    b.session.charge_query_work(QUERY_WORK).unwrap();
    let mut bytes = Vec::new();
    protocol::respond(b, request, &mut bytes, protocol_limits()).unwrap();
    assert!(bytes.len() <= RESPONSE);
    serde_json::from_slice(&bytes).unwrap()
}
fn seek_request(b: &Backend<CopySession>, sequence: u64) -> DebugRequestV1 {
    DebugRequestV1::Seek {
        schema: RequestSchemaV1::V1,
        request_id: 1,
        expected_revision: b.revision,
        cursor: DebugCursorV1 {
            configuration_identity: b.configuration,
            event_sequence: sequence,
            state_revision: b.revision,
        },
    }
}
fn seek(b: &mut Backend<CopySession>, sequence: u64) -> DebugResponseV1 {
    let request = seek_request(b, sequence);
    response(b, request)
}
fn inspect(b: &Backend<CopySession>, page: PageRequestV1) -> DebugRequestV1 {
    let invocation = b.session.current().unwrap().invocation();
    DebugRequestV1::InspectValues {
        schema: RequestSchemaV1::V1,
        request_id: 1,
        expected_revision: b.revision,
        scope: ExecutionScopeSelectorV1::Lane {
            workgroup: invocation.workgroup.map(|n| n as u32),
            wave: 0,
            lane: invocation.local[0] as u16,
        },
        frame: Some(1),
        selector: ValueSelectorV1::All,
        page,
    }
}
fn memory_request(b: &Backend<CopySession>, index: usize, offset: u64, len: u64) -> DebugRequestV1 {
    let (id, _) = b
        .session
        .current()
        .unwrap()
        .memory_allocation(index)
        .unwrap();
    DebugRequestV1::ReadMemory {
        schema: RequestSchemaV1::V1,
        request_id: 1,
        expected_revision: b.revision,
        allocation: AllocationIdentityV1 {
            ordinal: id,
            generation: 0,
        },
        byte_offset: offset,
        byte_len: len,
    }
}
fn memory(b: &mut Backend<CopySession>, index: usize, offset: u64, len: u64) -> (String, String) {
    let request = memory_request(b, index, offset, len);
    let DebugResponseV1::Ok { result, .. } = response(b, request) else {
        panic!("memory response")
    };
    let DebugResultV1::Memory { memory, .. } = *result else {
        panic!("memory result")
    };
    let MemoryAvailabilityV1::Captured {
        bytes,
        initialized,
        truncated: false,
        ..
    } = memory.availability
    else {
        panic!("memory captured")
    };
    (bytes, initialized)
}
fn first_checkpoint(b: &Backend<CopySession>) -> usize {
    (0..b.session.records_len())
        .find(|&i| b.session.record(i).unwrap().phase().is_some())
        .unwrap()
}
fn site(op: Op) -> u32 {
    fixture::module().functions[0].body.as_ref().unwrap().blocks[0].operations.iter().position(
        |o|matches!(&o.kind,OperationKind::Gfx942PhysicalGlobalCopyStep(s) if s.instruction.opcode==op)
    ).unwrap() as u32
}
fn checkpoint(b: &Backend<CopySession>, operation: u32, phase: Phase, lane: u64) -> usize {
    (0..b.session.records_len())
        .find(|&i| {
            let r = b.session.record(i).unwrap();
            r.invocation().global[0] == lane
                && r.site().operation == operation
                && r.phase() == Some(phase)
        })
        .unwrap()
}
#[test]
fn two_allocations_full_output_tail_and_reverse_match_actual_engine() {
    for (grid, edited) in [(64, false), (128, true)] {
        let files = Files::new(grid, edited);
        let (_input, mut b) = backend(&files);
        assert!(b.session.capture_stop().is_none());
        let first = first_checkpoint(&b);
        let last = (0..b.session.records_len())
            .rev()
            .find(|&i| b.session.record(i).unwrap().phase().is_some())
            .unwrap();
        for (index, written) in [(last, true), (first, false), (last, true)] {
            assert!(matches!(
                seek(&mut b, index as u64 + 1),
                DebugResponseV1::Ok { .. }
            ));
            for allocation in 0..2 {
                for offset in (0..512).step_by(256) {
                    let bytes: Vec<u8> = (offset / 4..(offset + 256) / 4)
                        .flat_map(|i| {
                            if allocation == 0 || (written && i < grid as usize) {
                                word(i).to_le_bytes()
                            } else {
                                [0xa5; 4]
                            }
                        })
                        .collect();
                    let expected_init: Vec<bool> = (offset..offset + 256)
                        .map(|i| allocation == 0 || (written && i < (grid as usize) * 4))
                        .collect();
                    assert_eq!(
                        memory(&mut b, allocation, offset as u64, 256),
                        (hex_bytes(&bytes), initialization_bits(&expected_init))
                    );
                }
            }
            assert_eq!(
                memory(&mut b, 1, 512, 4),
                ("0xa5a5a5a5".into(), "0x00".into())
            );
        }
        let view = b.view();
        assert!(view.simulated);
        assert!(!view.hardware_observed);
        assert!(!view.performance_prediction);
    }
}
#[test]
fn pending_global_load_stays_present_not_represented_until_actual_vm_wait() {
    let files = Files::new(64, true);
    let (_input, mut b) = backend(&files);
    let pending = checkpoint(&b, site(Op::GlobalLoadDword), Phase::AfterOperation, 0);
    let ready = checkpoint(&b, site(Op::WaitVm0), Phase::AfterOperation, 0);
    let r = b.session.record(pending).unwrap();
    let value = (0..r.binding_count(0).unwrap())
        .find_map(|i| {
            let v = r.binding(0, i).unwrap();
            (v.symbolic_kind() == Some(Symbolic::PendingGlobalRead)).then_some(v.value())
        })
        .unwrap();
    for (index, captured) in [(pending, false), (ready, true), (pending, false)] {
        seek(&mut b, index as u64 + 1);
        let request = inspect(
            &b,
            PageRequestV1 {
                cursor: None,
                limit: 64,
            },
        );
        let DebugResponseV1::Ok { result, .. } = response(&mut b, request) else {
            panic!("values")
        };
        let DebugResultV1::Values {
            values, snapshot, ..
        } = *result
        else {
            panic!("values")
        };
        assert!(matches!(
            snapshot.scope,
            ExecutionScopeV1::Lane {
                active_mask: u64::MAX,
                wave_width: 64,
                interpretation: WaveInterpretationV1::LogicalVisualization,
                ..
            }
        ));
        assert!(snapshot.frame.is_none() && snapshot.occurrence.is_none());
        let v=values.iter().find(|v|matches!(v.path.root,ValueRootV1::Ssa{value_ordinal,..} if value_ordinal==u64::from(value.0))).unwrap();
        if captured {
            assert!(
                matches!(&v.availability,ValueAvailabilityV1::Captured{value:CapturedValueV1::Bits{bits},provenance:ValueProvenanceV1::SimulatedObservation,..} if bits==&fixed_width_bits(u128::from(word(0)),32))
            );
        } else {
            assert_eq!(
                v.availability,
                ValueAvailabilityV1::Unavailable {
                    reason: ValueUnavailableReasonV1::NotRepresented
                }
            );
        }
    }
}
#[test]
fn global_store_checkpoint_is_same_site_before_after_and_reversible() {
    let files = Files::new(64, false);
    let (_input, mut b) = backend(&files);
    let before = checkpoint(&b, site(Op::GlobalStoreDword), Phase::BeforeOperation, 0);
    let after = checkpoint(&b, site(Op::GlobalStoreDword), Phase::AfterOperation, 0);
    assert_eq!(
        b.session.record(before).unwrap().site(),
        b.session.record(after).unwrap().site()
    );
    for (index, expected, init) in [
        (before, "0xa5a5a5a5", "0x00"),
        (after, "0x01000080", "0x0f"),
        (before, "0xa5a5a5a5", "0x00"),
    ] {
        seek(&mut b, index as u64 + 1);
        assert_eq!(memory(&mut b, 1, 0, 4), (expected.into(), init.into()));
    }
}
#[test]
fn discovery_and_projection_do_not_advertise_source_registers_or_hardware() {
    let files = Files::new(64, false);
    let (_input, mut b) = backend(&files);
    let request = DebugRequestV1::DiscoverCapabilities {
        schema: RequestSchemaV1::V1,
        request_id: 1,
        expected_revision: 0,
    };
    let DebugResponseV1::Ok { result, .. } = response(&mut b, request) else {
        panic!("capabilities")
    };
    let DebugResultV1::Capabilities { capabilities } = *result else {
        panic!("capabilities")
    };
    assert_eq!(capabilities.len(), 17);
    for c in capabilities {
        let available = matches!(
            c.name,
            DebugCapabilityNameV1::KirSites
                | DebugCapabilityNameV1::ForwardStep
                | DebugCapabilityNameV1::ReverseStep
                | DebugCapabilityNameV1::KirSsaValues
                | DebugCapabilityNameV1::AllocationRelativeMemory
        );
        assert_eq!(
            c.availability,
            if available {
                CapabilityAvailabilityV1::Available
            } else {
                CapabilityAvailabilityV1::Unavailable
            }
        );
    }
    let i = first_checkpoint(&b);
    seek(&mut b, i as u64 + 1);
    for selector in [
        ValueSelectorV1::Roots {
            roots: vec![ValueRootClassV1::Register],
        },
        ValueSelectorV1::Roots {
            roots: vec![ValueRootClassV1::SourceVariable],
        },
    ] {
        let mut request = inspect(
            &b,
            PageRequestV1 {
                cursor: None,
                limit: 64,
            },
        );
        if let DebugRequestV1::InspectValues { selector: s, .. } = &mut request {
            *s = selector;
        }
        let before = b.view();
        assert!(matches!(
            response(&mut b, request),
            DebugResponseV1::Unavailable { .. }
        ));
        assert_eq!(b.view(), before);
    }
    let snapshot = views::snapshot(&b, b.sequence(), b.view());
    let SnapshotAvailabilityV1::Captured { snapshot } = snapshot else {
        panic!("snapshot")
    };
    assert!(matches!(
        snapshot.anchor.site.unwrap().source,
        SourceSiteAvailabilityV1::Unavailable {
            reason: SourceSiteUnavailableReasonV1::RequiresAuthenticatedMap
        }
    ));
}
#[test]
fn stale_revision_cursor_configuration_and_page_refuse_transactionally() {
    let files = Files::new(64, false);
    let (_input, mut b) = backend(&files);
    let i = (0..b.session.records_len())
        .find(|&i| b.session.record(i).unwrap().binding_count(0).unwrap_or(0) > 1)
        .unwrap();
    seek(&mut b, i as u64 + 1);
    let request = inspect(
        &b,
        PageRequestV1 {
            cursor: None,
            limit: 1,
        },
    );
    let DebugResponseV1::Ok { result, .. } = response(&mut b, request) else {
        panic!("page")
    };
    let DebugResultV1::Values {
        next_cursor: Some(page),
        ..
    } = *result
    else {
        panic!("page")
    };
    let old = b.view();
    seek(&mut b, i as u64 + 1);
    let current = b.view();
    let requests = [
        DebugRequestV1::GetState {
            schema: RequestSchemaV1::V1,
            request_id: 1,
            expected_revision: old.revision,
        },
        DebugRequestV1::Seek {
            schema: RequestSchemaV1::V1,
            request_id: 1,
            expected_revision: b.revision,
            cursor: old.cursor,
        },
        DebugRequestV1::Seek {
            schema: RequestSchemaV1::V1,
            request_id: 1,
            expected_revision: b.revision,
            cursor: DebugCursorV1 {
                configuration_identity: OpaqueIdentityV1::new([0x7f; 32]).unwrap(),
                ..current.cursor
            },
        },
    ];
    for request in requests {
        assert!(matches!(
            response(&mut b, request),
            DebugResponseV1::Error { .. }
        ));
        assert_eq!(b.view(), current);
    }
    let request = inspect(
        &b,
        PageRequestV1 {
            cursor: Some(page),
            limit: 1,
        },
    );
    assert!(matches!(
        response(&mut b, request),
        DebugResponseV1::Unavailable { .. }
    ));
    assert_eq!(b.view(), current);
    let mut hash = Sha256::new();
    hash.update(Profile::EntryV20.page_domain());
    hash.update(b.configuration.as_bytes());
    hash.update(b.sequence().to_le_bytes());
    hash.update(b.revision.to_le_bytes());
    let request = inspect(
        &b,
        PageRequestV1 {
            cursor: Some(PageCursorV1 {
                query_identity: OpaqueIdentityV1::new(hash.finalize().into()).unwrap(),
                position: 1,
            }),
            limit: 1,
        },
    );
    assert!(matches!(
        response(&mut b, request),
        DebugResponseV1::Unavailable { .. }
    ));
    assert_eq!(b.view(), current);
}
#[test]
fn memory_frame_scope_generation_and_page_bounds_are_closed() {
    let files = Files::new(64, false);
    let (_input, mut b) = backend(&files);
    let i = first_checkpoint(&b);
    seek(&mut b, i as u64 + 1);
    let before = b.view();
    for mode in 0..7 {
        let mut request = memory_request(&b, 0, 0, 4);
        if let DebugRequestV1::ReadMemory {
            allocation,
            byte_offset,
            byte_len,
            ..
        } = &mut request
        {
            match mode {
                0 => allocation.generation = 1,
                1 => allocation.ordinal = u64::MAX,
                2 => *byte_offset = u64::MAX,
                3 => *byte_len = 257,
                4 => *byte_len = 0,
                5 => {
                    *byte_offset = 510;
                    *byte_len = 4
                }
                _ => *byte_offset = 512,
            }
        }
        assert!(matches!(
            response(&mut b, request),
            DebugResponseV1::Unavailable { .. }
        ));
        assert_eq!(b.view(), before);
    }
    for mode in 0..3 {
        let mut request = inspect(
            &b,
            PageRequestV1 {
                cursor: None,
                limit: 64,
            },
        );
        if let DebugRequestV1::InspectValues {
            frame, scope, page, ..
        } = &mut request
        {
            match mode {
                0 => *frame = Some(2),
                1 => {
                    *scope = ExecutionScopeSelectorV1::Lane {
                        workgroup: [1, 0, 0],
                        wave: 0,
                        lane: 0,
                    }
                }
                _ => page.limit = 65,
            }
        }
        assert!(matches!(
            response(&mut b, request),
            DebugResponseV1::Unavailable { .. }
        ));
        assert_eq!(b.view(), before);
    }
}
#[test]
fn response_exact_bound_and_one_short_never_commit_rejected_navigation() {
    let files = Files::new(64, false);
    let (_input, mut first) = backend(&files);
    let i = first_checkpoint(&first);
    let request = seek_request(&first, i as u64 + 1);
    let mut bytes = Vec::new();
    protocol::respond(&mut first, request.clone(), &mut bytes, protocol_limits()).unwrap();
    for (short, expected) in [(false, true), (true, false)] {
        let (_input, mut b) = backend(&files);
        let before = b.view();
        let mut limits = protocol_limits();
        limits.max_response_line_bytes = bytes.len() - usize::from(short);
        let mut actual = Vec::new();
        let result = protocol::respond(&mut b, request.clone(), &mut actual, limits);
        if expected {
            result.unwrap();
            assert_eq!(actual, bytes);
            assert_ne!(b.view(), before);
        } else {
            assert_eq!(b.view(), before);
            if !actual.is_empty() {
                assert!(matches!(
                    serde_json::from_slice::<DebugResponseV1>(&actual).unwrap(),
                    DebugResponseV1::Error { .. }
                ));
            }
        }
    }
}
#[test]
fn jsonl_framing_query_denial_and_end_reverse_keep_real_session_rules() {
    let files = Files::new(64, false);
    let (_input, mut b) = backend(&files);
    let end = b.session.records_len() as u64 + 1;
    seek(&mut b, end);
    let used = b.session.usage().work;
    let request = DebugRequestV1::Step {
        schema: RequestSchemaV1::V1,
        request_id: 1,
        expected_revision: b.revision,
        direction: StepDirectionV1::Forward,
        granularity: StepGranularityV1::Event,
        count: 1,
        focus: None,
    };
    response(&mut b, request);
    assert_eq!(b.session.usage().work, used + QUERY_WORK + 1);
    seek(&mut b, end - 1);
    seek(&mut b, 0);
    let before = b.view();
    let invalid=b"{\"schema\":\"fe2o3-debug-request-v2\",\"operation\":\"get_state\",\"request_id\":1,\"expected_revision\":0}\n";
    let mut output = Vec::new();
    assert_eq!(
        protocol::run(&mut b, &mut &invalid[..], &mut output, protocol_limits()),
        Err("kir_v21_debug_protocol_refused")
    );
    assert_eq!(b.view(), before);
    let used = b.session.usage().work;
    b.session.charge_query_work(WORK - used).unwrap();
    output.clear();
    assert_eq!(
        protocol::run(&mut b, &mut &b""[..], &mut output, protocol_limits()),
        Err("kir_v21_debug_query_work_limit")
    );
    assert_eq!(b.view(), before);
    assert!(b.session.usage().failed_work.is_some());
}
#[test]
fn loader_exact_work_storage_and_one_short_preserve_existing_floor() {
    let files = Files::new(64, false);
    let run = |wl, sl| {
        let mut work = Work::new(wl);
        let mut budget = Budget::new(&mut work, sl);
        budget.reserve_storage(73).unwrap();
        budget.charge_work(29).unwrap();
        let result =
            load_physical_global_copy_debug_input_v21(&files.kir, &files.request, &mut budget);
        let ok = result.is_ok();
        assert_eq!(budget.storage(), 73);
        if let Ok((input, receipt)) = result {
            assert!(receipt.retained_storage() > 0);
            assert_eq!(input.request().arguments.len(), 2);
        }
        let accepted = budget.work();
        let peak = budget.peak_storage();
        let failed_storage = budget.failed_storage();
        (ok, accepted, peak, work.failed_work(), failed_storage)
    };
    let (ok, w, p, _, _) = run(WORK, STORAGE);
    assert!(ok);
    assert!(run(w, p).0);
    let short = run(w - 1, p);
    assert!(!short.0 && short.3.is_some());
    let short = run(w, p - 1);
    assert!(!short.0 && short.4.is_some());
}
#[test]
fn failed_memory_preconditions_never_become_a_cli_session() {
    for mode in 0..5 {
        let files = Files::new(64, false);
        let mut d = document(64);
        match mode {
            0 => {
                d["arguments"][0]["initialized"] = format!("0x{}", "00".repeat(64)).into();
            }
            1 => {
                d["arguments"][0]["bytes"] = format!("0x{}", "00".repeat(252)).into();
                d["arguments"][0]["initialized"] = format!("0x{}", "ff".repeat(31) + "0f").into();
            }
            2 => {
                d["arguments"][0]["access"] = "write_only".into();
            }
            _ => {
                let offset = if mode == 3 { 0 } else { 512 };
                d["shared_buffers"] = serde_json::json!([{"id":7,"element":"u32","access":"read_write","alignment":4,"bytes":format!("0x{}","5a".repeat(1028)),"initialized":format!("0x{}","ff".repeat(128)+"0f")}]);
                d["arguments"] = serde_json::json!([
                    {"kind":"buffer_view","backing":7,"element":"u32","access":"read_only","alignment":4,"byte_offset":0,"elements":128},
                    {"kind":"buffer_view","backing":7,"element":"u32","access":"read_write","alignment":4,"byte_offset":offset,"elements":129}
                ]);
            }
        }
        files.set_request(&d);
        let (input, ledger) = load(&files);
        assert_eq!(
            capture(&input, ledger).err(),
            Some("kir_v21_debug_capture_refused")
        );
    }
}
#[test]
fn loader_wrong_version_shape_authority_symlink_and_size_refuse() {
    let refusal = |files: &Files| {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(73).unwrap();
        let result =
            load_physical_global_copy_debug_input_v21(&files.kir, &files.request, &mut budget);
        assert_eq!(budget.storage(), 73);
        result.err().unwrap()
    };
    for version in [19, 20, 22] {
        let files = Files::new(64, false);
        let mut bytes = std::fs::read(&files.kir).unwrap();
        bytes[8..10].copy_from_slice(&(version as u16).to_le_bytes());
        std::fs::write(&files.kir, bytes).unwrap();
        assert_eq!(refusal(&files), InputError::WrongVersion);
    }
    for mode in 0..6 {
        let files = Files::new(64, false);
        let mut d = document(64);
        match mode {
            0 => d["source_authority"] = true.into(),
            1 => d["register_map"] = "invented".into(),
            2 => d["workgroup"] = serde_json::json!([32, 1, 1]),
            3 => d["grid"] = serde_json::json!([129, 1, 1]),
            4 => {
                d["arguments"].as_array_mut().unwrap().pop();
            }
            _ => d["schema"] = "fe2o3-simulation-request-v2".into(),
        }
        files.set_request(&d);
        assert_eq!(refusal(&files), InputError::Request);
    }
    for request in [false, true] {
        let files = Files::new(64, false);
        let (path, limit) = if request {
            (&files.request, 16 * 1024)
        } else {
            (&files.kir, 128 * 1024)
        };
        std::fs::write(path, vec![b' '; limit + 1]).unwrap();
        assert_eq!(refusal(&files), InputError::Input);
    }
    let files = Files::new(64, false);
    std::fs::remove_file(&files.kir).unwrap();
    std::os::unix::fs::symlink(&files.request, &files.kir).unwrap();
    assert_eq!(refusal(&files), InputError::Input);
}
#[test]
fn explicit_profiles_cannot_mix_flags_maps_or_runtime_authority() {
    let args = |extra: &[&str]| {
        ["sim", "--diagnostic-kir-v21", "a", "--request", "b"]
            .into_iter()
            .chain(extra.iter().copied())
            .map(OsString::from)
            .collect()
    };
    assert!(parse_profile(args(&[]), Profile::GlobalCopyV21).is_ok());
    assert!(
        parse_profile(
            args(&["--protocol", "jsonl", "--wave-width", "64"]),
            Profile::GlobalCopyV21
        )
        .is_ok()
    );
    for extra in [
        ["--diagnostic-kir-v20", "c"],
        ["--diagnostic-kir-v21", "c"],
        ["--source-map", "c"],
        ["--register-map", "c"],
        ["--runtime-observations", "v1"],
        ["--replay-schedule", "c"],
        ["--wave-width", "32"],
        ["--bundle", "c"],
        ["--snapshot", "c"],
    ] {
        assert_eq!(
            parse_profile(args(&extra), Profile::GlobalCopyV21),
            Err("kir_v21_debug_option_unavailable")
        );
    }
    assert_eq!(parse(args(&[])), Err("kir_v20_debug_option_unavailable"));
}

#[test]
fn v21_jsonl_v1_runs_discover_forward_reverse_and_terminate() {
    let files = Files::new(64, false);
    let (_input, mut b) = backend(&files);
    let requests = [
        DebugRequestV1::DiscoverCapabilities {
            schema: RequestSchemaV1::V1,
            request_id: 1,
            expected_revision: 0,
        },
        DebugRequestV1::Step {
            schema: RequestSchemaV1::V1,
            request_id: 2,
            expected_revision: 0,
            direction: StepDirectionV1::Forward,
            granularity: StepGranularityV1::Event,
            count: 1,
            focus: None,
        },
        DebugRequestV1::Step {
            schema: RequestSchemaV1::V1,
            request_id: 3,
            expected_revision: 1,
            direction: StepDirectionV1::Reverse,
            granularity: StepGranularityV1::Event,
            count: 1,
            focus: None,
        },
        DebugRequestV1::Terminate {
            schema: RequestSchemaV1::V1,
            request_id: 4,
            expected_revision: 2,
        },
    ];
    let mut input = Vec::new();
    for request in requests {
        serde_json::to_writer(&mut input, &request).unwrap();
        input.push(b'\n');
    }
    let mut output = Vec::new();
    protocol::run(&mut b, &mut &input[..], &mut output, protocol_limits()).unwrap();
    let rows: Vec<DebugResponseV1> = output
        .split(|b| *b == b'\n')
        .filter(|s| !s.is_empty())
        .map(|s| serde_json::from_slice(s).unwrap())
        .collect();
    assert_eq!(rows.len(), 4);
    assert_eq!(b.revision, 3);
    assert_eq!(b.sequence(), 0);
    assert!(b.terminated);
    for row in rows {
        assert!(matches!(
            row,
            DebugResponseV1::Ok {
                session: SessionViewV1 {
                    simulated: true,
                    hardware_observed: false,
                    performance_prediction: false,
                    ..
                },
                ..
            }
        ));
    }
}
#[test]
fn exhausted_navigation_does_not_commit_and_truncation_never_claims_end() {
    let files = Files::new(64, false);
    let (_input, mut b) = backend(&files);
    let i = first_checkpoint(&b);
    let request = seek_request(&b, i as u64 + 1);
    let before = b.view();
    // Models an already prepaid command whose final one-unit navigation cannot fit.
    let used = b.session.usage().work;
    b.session.charge_query_work(WORK - used).unwrap();
    let mut output = Vec::new();
    protocol::respond(&mut b, request, &mut output, protocol_limits()).unwrap();
    assert_eq!(b.view(), before);
    assert!(b.session.usage().failed_work.is_some());
    assert!(matches!(
        serde_json::from_slice::<DebugResponseV1>(&output).unwrap(),
        DebugResponseV1::Error {
            error: DebugErrorV1 {
                code: DebugErrorCodeV1::ResourceLimit,
                state_changed: false,
                ..
            },
            ..
        }
    ));
    let (input, ledger) = load(&files);
    let configuration = configuration_for(
        Profile::GlobalCopyV21,
        input.canonical().identity().digest(),
        input.canonical().identity().canonical_length(),
        input.request_digest(),
        input.request_bytes(),
        input.limits(),
    )
    .unwrap();
    let session = CopySession::capture(
        input.module(),
        input.canonical(),
        input.request(),
        PhysicalGlobalCopyDebugOptionsV21::new(
            input.limits(),
            SimulationDebugCaptureLimitsV1::new(1, 768, 8, 16384).unwrap(),
            1,
        )
        .unwrap(),
        ledger,
    );
    assert!(session.capture_stop().is_some());
    let mut b = Backend {
        session,
        configuration,
        revision: 0,
        terminated: false,
    };
    let before = b.view();
    let end = b.session.records_len() as u64 + 1;
    assert!(matches!(
        seek(&mut b, end),
        DebugResponseV1::Unavailable {
            unavailable: CapabilityUnavailableV1 {
                reason: CapabilityUnavailableReasonV1::Truncated,
                ..
            },
            ..
        }
    ));
    assert_eq!(b.view(), before);
}
