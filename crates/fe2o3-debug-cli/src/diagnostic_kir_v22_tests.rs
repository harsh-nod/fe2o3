//! Shared inert KIR fixture through the real V22 loader, Engine and JSONL route.
//! No actual Rust/source/native qualification is inferred from these tests.
use super::*;
use fe2o3_kernel_ir as physical_lds_exchange_fixture_ir;
use fe2o3_kernel_ir::encode_module_v22;
use fe2o3_kir_sim::{
    PhysicalLdsExchangeDebugSymbolicKindV22 as Symbolic, SimulationDebugCheckpointPhaseV1 as Phase,
};
use std::sync::atomic::{AtomicU64, Ordering};
#[path = "../../fe2o3-kernel-ir/tests/fixtures/physical_lds_exchange_v22.rs"]
mod fixture;
static NEXT: AtomicU64 = AtomicU64::new(1);
pub(super) struct Files {
    pub(super) directory: PathBuf,
    kir: PathBuf,
    request: PathBuf,
}
pub(super) fn word(index: usize) -> u32 {
    (index as u32)
        .wrapping_mul(0x9e37_79b9)
        .wrapping_add(0x8000_0001)
}
pub(super) fn document(output: usize) -> serde_json::Value {
    let mut input = vec![0x5a; 528];
    for i in 0..128 {
        input[8 + i * 4..12 + i * 4].copy_from_slice(&word(i).to_le_bytes());
    }
    let output_bytes = (output + 4) * 4;
    serde_json::json!({
        "schema":"fe2o3-simulation-request-v1","kernel":"physical_lds_exchange_fixture",
        "grid":[128,1,1],"workgroup":[128,1,1],
        "shared_buffers":[
            {"id":7,"element":"u32","access":"read_only","alignment":4,
                "bytes":hex_bytes(&input),"initialized":format!("0x{}","ff".repeat(66))},
            {"id":9,"element":"u32","access":"read_write","alignment":4,
                "bytes":format!("0x{}","a5".repeat(output_bytes)),
                "initialized":format!("0x{}","00".repeat(output_bytes.div_ceil(8)))}
        ],
        "arguments":[
            {"kind":"buffer_view","backing":7,"element":"u32","access":"read_only","alignment":4,"byte_offset":8,"elements":128},
            {"kind":"buffer_view","backing":9,"element":"u32","access":"read_write","alignment":4,"byte_offset":8,"elements":output}
        ]
    })
}
impl Files {
    pub(super) fn new(output: usize, edited: bool) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "fe2o3-v22-debug-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).unwrap();
        let files = Self {
            kir: directory.join("canonical.kir"),
            request: directory.join("request.json"),
            directory,
        };
        let module = if edited {
            fixture::module_with_registers(true)
        } else {
            fixture::module()
        };
        std::fs::write(&files.kir, encode_module_v22(&module).unwrap()).unwrap();
        files.set_request(&document(output));
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
        let _ = std::fs::remove_file(self.directory.join("index.json"));
        let _ = std::fs::remove_file(self.directory.join("index-link.json"));
        let _ = std::fs::remove_dir(&self.directory);
    }
}
pub(super) fn load(files: &Files, export: bool) -> (LdsInput, Owned, usize) {
    let mut ledger = Owned::new(Work::new(WORK), STORAGE);
    ledger
        .with_budget(|b| {
            b.reserve_storage(73 + workspace(export))?;
            b.charge_work(29 + RESPONSE * 2)
        })
        .unwrap();
    let (input, receipt) = ledger
        .with_budget(|b| load_physical_lds_exchange_debug_input_v22(&files.kir, &files.request, b))
        .unwrap();
    ledger
        .with_budget(|b| b.reserve_storage(receipt.retained_storage()))
        .unwrap();
    (input, ledger, receipt.retained_storage())
}
pub(super) fn backend(files: &Files, export: bool) -> (LdsInput, Backend<LdsSession>, usize) {
    let (input, ledger, receipt) = load(files, export);
    let b = capture(&input, ledger, export).unwrap();
    assert!(b.session.capture_stop().is_none());
    (input, b, receipt)
}
pub(super) fn response(b: &mut Backend<LdsSession>, request: DebugRequestV1) -> DebugResponseV1 {
    b.session.charge_query_work(QUERY_WORK).unwrap();
    let mut bytes = Vec::new();
    protocol::respond(b, request, &mut bytes, protocol_limits()).unwrap();
    serde_json::from_slice(&bytes).unwrap()
}
fn seek_request(b: &Backend<LdsSession>, sequence: u64) -> DebugRequestV1 {
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
pub(super) fn seek(b: &mut Backend<LdsSession>, sequence: u64) -> DebugResponseV1 {
    let r = seek_request(b, sequence);
    response(b, r)
}
fn inspect(b: &Backend<LdsSession>, page: PageRequestV1) -> DebugRequestV1 {
    let i = b.session.current().unwrap().invocation();
    DebugRequestV1::InspectValues {
        schema: RequestSchemaV1::V1,
        request_id: 1,
        expected_revision: b.revision,
        scope: ExecutionScopeSelectorV1::Lane {
            workgroup: i.workgroup.map(|n| n as u32),
            wave: i.local[0] / 64,
            lane: (i.local[0] % 64) as u16,
        },
        frame: Some(1),
        selector: ValueSelectorV1::All,
        page,
    }
}
fn last_checkpoint(b: &Backend<LdsSession>) -> usize {
    (0..b.session.records_len())
        .rfind(|&i| b.session.record(i).unwrap().phase().is_some())
        .unwrap()
}
#[test]
fn actual_two_wave_scopes_three_allocations_and_canaries() {
    for (edited, output) in [(false, 129), (true, 13)] {
        let files = Files::new(output, edited);
        let (_input, mut b, _) = backend(&files, false);
        assert!(b.session.records_len() > 8192);
        for local in [0, 63, 64, 127] {
            let i = (0..b.session.records_len())
                .find(|&n| {
                    let r = b.session.record(n).unwrap();
                    r.phase() == Some(Phase::BeforeOperation) && r.invocation().local[0] == local
                })
                .unwrap();
            let DebugResponseV1::Ok { result, .. } = seek(&mut b, i as u64 + 1) else {
                panic!("seek")
            };
            let DebugResultV1::Control {
                snapshot: SnapshotAvailabilityV1::Captured { snapshot },
                ..
            } = *result
            else {
                panic!("snapshot")
            };
            assert!(
                matches!(snapshot.anchor.scope,ExecutionScopeV1::Lane{wave,lane,logical_workitem,..}
                if wave==local/64 && lane==(local%64)as u16 && logical_workitem==[u64::from(local),0,0])
            );
        }
        let last = last_checkpoint(&b);
        seek(&mut b, last as u64 + 1);
        let r = b.session.current().unwrap();
        assert!(r.memory_allocation(2).is_some() && r.memory_allocation(3).is_none());
        let output_slot = (0..3)
            .find(|&slot| r.memory_allocation(slot).unwrap().1 == (output + 4) * 4)
            .unwrap();
        for i in 0..(output + 4) * 4 {
            let expected = if (8..8 + output.min(128) * 4).contains(&i) {
                word(((i - 8) / 4) ^ 64).to_le_bytes()[(i - 8) % 4]
            } else {
                0xa5
            };
            assert_eq!(
                r.memory_byte_at(output_slot, i),
                Some((expected, (8..8 + output.min(128) * 4).contains(&i)))
            );
        }
        let (ordinal, _) = r.memory_allocation(output_slot).unwrap();
        let request = DebugRequestV1::ReadMemory {
            schema: RequestSchemaV1::V1,
            request_id: 2,
            expected_revision: b.revision,
            allocation: AllocationIdentityV1 {
                ordinal,
                generation: 0,
            },
            byte_offset: 0,
            byte_len: 16,
        };
        assert!(
            matches!(response(&mut b,request),DebugResponseV1::Ok{result,..} if matches!(*result,DebugResultV1::Memory{..}))
        );
        seek(&mut b, 1);
        seek(&mut b, last as u64 + 1);
        assert_eq!(
            b.session.current().unwrap().memory_byte_at(output_slot, 0),
            Some((0xa5, false))
        );
    }
}
#[test]
fn pending_global_and_lds_values_are_unavailable_until_same_ssa_is_ready() {
    let files = Files::new(129, false);
    let (_input, mut b, _) = backend(&files, false);
    for local in [0, 64] {
        for kind in [Symbolic::PendingGlobalRead, Symbolic::PendingLdsRead] {
            let (index, value) = (0..b.session.records_len())
                .find_map(|i| {
                    let r = b.session.record(i)?;
                    if r.phase().is_none() || r.invocation().local[0] != local {
                        return None;
                    }
                    (0..r.binding_count(0)?).find_map(|n| {
                        let x = r.binding(0, n)?;
                        (x.symbolic_kind() == Some(kind)).then_some((i, x.value()))
                    })
                })
                .unwrap();
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
            let DebugResultV1::Values { values, .. } = *result else {
                panic!("values")
            };
            assert!(values.iter().any(|v|matches!(v.path.root,ValueRootV1::Ssa{value_ordinal,..} if value_ordinal==u64::from(value.0))
                && matches!(v.availability,ValueAvailabilityV1::Unavailable{reason:ValueUnavailableReasonV1::NotRepresented})));
            let ready = (index + 1..b.session.records_len())
                .find(|&i| {
                    let r = b.session.record(i).unwrap();
                    r.phase().is_some()
                        && r.invocation().local[0] == local
                        && (0..r.binding_count(0).unwrap()).any(|n| {
                            let x = r.binding(0, n).unwrap();
                            x.value() == value && x.scalar().is_some()
                        })
                })
                .unwrap();
            seek(&mut b, ready as u64 + 1);
            let r = b.session.current().unwrap();
            let scalar = (0..r.binding_count(0).unwrap())
                .find_map(|n| {
                    let x = r.binding(0, n)?;
                    (x.value() == value).then(|| x.scalar()).flatten()
                })
                .unwrap();
            let expected = if kind == Symbolic::PendingGlobalRead {
                word(local as usize)
            } else {
                word((local as usize) ^ 64)
            };
            assert_eq!(scalar.bits(), u128::from(expected));
            seek(&mut b, index as u64 + 1);
            assert!(
                (0..b.session.current().unwrap().binding_count(0).unwrap()).any(|n| b
                    .session
                    .current()
                    .unwrap()
                    .binding(0, n)
                    .unwrap()
                    .symbolic_kind()
                    == Some(kind))
            );
        }
    }
}
#[test]
fn foreign_stale_and_invalid_navigation_are_transactional() {
    let files = Files::new(129, false);
    let (_input, mut b, _) = backend(&files, false);
    seek(&mut b, 1);
    let before = (b.sequence(), b.revision);
    let mut requests = vec![seek_request(&b, b.session.records_len() as u64 + 3)];
    let mut stale = seek_request(&b, 2);
    if let DebugRequestV1::Seek {
        expected_revision, ..
    } = &mut stale
    {
        *expected_revision -= 1;
    }
    requests.push(stale);
    let mut foreign = seek_request(&b, 2);
    if let DebugRequestV1::Seek { cursor, .. } = &mut foreign {
        cursor.configuration_identity = OpaqueIdentityV1::new([9; 32]).unwrap();
    }
    requests.push(foreign);
    for request in requests {
        assert!(matches!(
            response(&mut b, request),
            DebugResponseV1::Error {
                error: DebugErrorV1 {
                    state_changed: false,
                    ..
                },
                ..
            }
        ));
        assert_eq!((b.sequence(), b.revision), before);
    }
    let mut hash = Sha256::new();
    hash.update(Profile::GlobalCopyV21.page_domain());
    hash.update(b.configuration.as_bytes());
    hash.update(b.sequence().to_le_bytes());
    hash.update(b.revision.to_le_bytes());
    let request = inspect(
        &b,
        PageRequestV1 {
            cursor: Some(PageCursorV1 {
                query_identity: OpaqueIdentityV1::new(hash.finalize().into()).unwrap(),
                position: 0,
            }),
            limit: 1,
        },
    );
    assert!(matches!(
        response(&mut b, request),
        DebugResponseV1::Unavailable { .. }
    ));
    assert_eq!((b.sequence(), b.revision), before);
}
#[test]
fn input_preconditions_do_not_become_successful_sessions() {
    for mode in 0..4 {
        let files = Files::new(129, false);
        let mut doc = document(129);
        match mode {
            0 => doc["shared_buffers"][0]["initialized"] = format!("0x{}", "00".repeat(66)).into(),
            1 => doc["arguments"][0]["elements"] = 127.into(),
            _ => {
                doc["shared_buffers"] = serde_json::json!([{"id":7,"element":"u32","access":"read_write","alignment":4,
                    "bytes":format!("0x{}","5a".repeat(1072)),"initialized":format!("0x{}","ff".repeat(134))}]);
                doc["arguments"][1]["backing"] = 7.into();
                doc["arguments"][1]["byte_offset"] = if mode == 2 { 8 } else { 536 }.into();
            }
        }
        files.set_request(&doc);
        let (input, ledger, _) = load(&files, false);
        assert_eq!(
            capture(&input, ledger, false).err(),
            Some("kir_v22_debug_capture_refused")
        );
    }
}
#[test]
fn parser_is_explicit_and_older_profiles_still_reject_new_flag() {
    let args = |extra: &[&str]| {
        ["sim", "--diagnostic-kir-v22", "a", "--request", "b"]
            .into_iter()
            .chain(extra.iter().copied())
            .map(OsString::from)
            .collect()
    };
    assert!(
        parse(args(&[
            "--capture-index",
            "x",
            "--protocol",
            "jsonl",
            "--wave-width",
            "64"
        ]))
        .is_ok()
    );
    for option in [
        "--source-map",
        "--register-map",
        "--runtime-observations",
        "--replay-schedule",
        "--snapshot",
        "--diagnostic-kir-v21",
    ] {
        assert_eq!(
            parse(args(&[option, "x"])).err(),
            Some("kir_v22_debug_option_unavailable")
        );
    }
    assert_eq!(
        parse(args(&["--wave-width", "32"])).err(),
        Some("kir_v22_debug_option_unavailable")
    );
    assert_eq!(
        parse(args(&["--capture-index", "x", "--capture-index", "y"])).err(),
        Some("kir_v22_debug_option_unavailable")
    );
    for profile in [Profile::EntryV20, Profile::GlobalCopyV21] {
        let arguments = [
            "sim",
            profile.selector(),
            "a",
            "--request",
            "b",
            "--capture-index",
            "x",
        ]
        .into_iter()
        .map(OsString::from)
        .collect();
        assert_eq!(
            parse_profile(arguments, profile).err(),
            Some(profile.code(Code::OptionUnavailable))
        );
    }
}
#[test]
fn discovery_and_unavailable_source_trace_are_truthful() {
    let files = Files::new(129, false);
    let (_input, mut b, _) = backend(&files, false);
    let request = DebugRequestV1::DiscoverCapabilities {
        schema: RequestSchemaV1::V1,
        request_id: 1,
        expected_revision: 0,
    };
    let DebugResponseV1::Ok {
        session, result, ..
    } = response(&mut b, request)
    else {
        panic!("discovery")
    };
    assert!(session.simulated && !session.hardware_observed && !session.performance_prediction);
    let DebugResultV1::Capabilities { capabilities } = *result else {
        panic!("capabilities")
    };
    for name in [
        DebugCapabilityNameV1::SourceSites,
        DebugCapabilityNameV1::RegisterValues,
        DebugCapabilityNameV1::HardwareWaveState,
        DebugCapabilityNameV1::SemanticTrace,
    ] {
        assert!(
            capabilities
                .iter()
                .any(|c| c.name == name && c.availability == CapabilityAvailabilityV1::Unavailable)
        );
    }
    let request = DebugRequestV1::ExportTrace {
        schema: RequestSchemaV1::V1,
        request_id: 2,
        expected_revision: 0,
        max_bytes: 1024,
    };
    assert!(matches!(
        response(&mut b, request),
        DebugResponseV1::Unavailable { .. }
    ));
}
#[test]
fn export_configuration_and_reservation_are_distinct_but_old_limits_unchanged() {
    let files = Files::new(129, false);
    let (input, ledger, _) = load(&files, true);
    assert_ne!(
        configuration(&input, false).unwrap(),
        configuration(&input, true).unwrap()
    );
    assert_eq!(workspace(true) - workspace(false), index::TEMP_STORAGE);
    let floor = ledger.storage();
    let mut b = capture(&input, ledger, true).unwrap();
    assert_eq!(b.session.usage().entry_storage, floor);
    let before = b.session.usage();
    index::export(&mut b, &input, &files.directory.join("index.json")).unwrap();
    let after = b.session.usage();
    assert_eq!(after.entry_storage, before.entry_storage);
    assert_eq!(after.retained_storage, before.retained_storage);
    assert_eq!(after.peak_storage, before.peak_storage);
    assert_eq!(after.work, before.work + index::PREPAID_WORK);
    assert_eq!(
        (after.failed_work, after.failed_storage),
        (before.failed_work, before.failed_storage)
    );
    assert_eq!((b.sequence(), b.revision), (0, 0));
    let mut ledger = b.session.into_budget();
    assert_eq!(ledger.storage(), floor);
    ledger
        .with_budget(|b| b.release_storage(index::TEMP_STORAGE))
        .unwrap();
    assert_eq!(ledger.storage(), floor - index::TEMP_STORAGE);
    assert_eq!(Profile::EntryV20.record_limit(), 8192);
    assert_eq!(Profile::GlobalCopyV21.record_limit(), 8192);
}

#[test]
fn typed_public_file_loader_exact_and_short_resources_preserve_floor() {
    use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    let files = Files::new(129, false);
    let run = |wl, sl| {
        let mut work = Work::new(wl);
        let mut b = Budget::new(&mut work, sl);
        b.reserve_storage(73).unwrap();
        b.charge_work(29).unwrap();
        let result = load_physical_lds_exchange_debug_input_v22(&files.kir, &files.request, &mut b);
        let ok = result.is_ok();
        assert_eq!(b.storage(), 73);
        let used = b.work();
        let peak = b.peak_storage();
        let denial = b.failed_storage();
        (ok, used, peak, work.failed_work(), denial)
    };
    let (ok, used, peak, _, _) = run(WORK, STORAGE);
    assert!(ok);
    assert!(run(used, peak).0);
    let short = run(used - 1, peak);
    assert!(!short.0 && short.3.is_some());
    let short = run(used, peak - 1);
    assert!(!short.0 && short.4.is_some());
}
#[test]
fn stale_page_wrong_scope_and_memory_limits_do_not_change_state() {
    let files = Files::new(129, false);
    let (_input, mut b, _) = backend(&files, false);
    seek(&mut b, 1);
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
        next_cursor: Some(cursor),
        ..
    } = *result
    else {
        panic!("next page")
    };
    seek(&mut b, 1);
    let before = b.view();
    let stale = inspect(
        &b,
        PageRequestV1 {
            cursor: Some(cursor),
            limit: 1,
        },
    );
    assert!(matches!(
        response(&mut b, stale),
        DebugResponseV1::Unavailable { .. }
    ));
    let mut wrong = inspect(
        &b,
        PageRequestV1 {
            cursor: None,
            limit: 64,
        },
    );
    if let DebugRequestV1::InspectValues { scope, .. } = &mut wrong {
        *scope = ExecutionScopeSelectorV1::Lane {
            workgroup: [0, 0, 0],
            wave: 1,
            lane: 0,
        };
    }
    assert!(matches!(
        response(&mut b, wrong),
        DebugResponseV1::Unavailable { .. }
    ));
    let (ordinal, bytes) = b.session.current().unwrap().memory_allocation(0).unwrap();
    for (generation, offset, length) in [
        (1, 0, 1),
        (0, bytes as u64, 1),
        (0, 0, 257),
        (0, u64::MAX, 1),
    ] {
        let request = DebugRequestV1::ReadMemory {
            schema: RequestSchemaV1::V1,
            request_id: 1,
            expected_revision: b.revision,
            allocation: AllocationIdentityV1 {
                ordinal,
                generation,
            },
            byte_offset: offset,
            byte_len: length,
        };
        assert!(matches!(
            response(&mut b, request),
            DebugResponseV1::Unavailable { .. }
        ));
    }
    assert_eq!(b.view(), before);
}
#[test]
fn exhausted_navigation_and_protocol_query_are_transactional() {
    let files = Files::new(129, false);
    let (_input, mut b, _) = backend(&files, false);
    let request = seek_request(&b, 1);
    let before = b.view();
    b.session
        .charge_query_work(WORK - b.session.usage().work)
        .unwrap();
    let mut bytes = Vec::new();
    protocol::respond(&mut b, request, &mut bytes, protocol_limits()).unwrap();
    assert_eq!(b.view(), before);
    assert!(matches!(
        serde_json::from_slice::<DebugResponseV1>(&bytes).unwrap(),
        DebugResponseV1::Error {
            error: DebugErrorV1 {
                code: DebugErrorCodeV1::ResourceLimit,
                state_changed: false,
                ..
            },
            ..
        }
    ));
    let mut bytes = Vec::new();
    assert_eq!(
        protocol::run(&mut b, &mut &b""[..], &mut bytes, protocol_limits()),
        Err("kir_v22_debug_query_work_limit")
    );
    assert_eq!(b.view(), before);
    assert!(b.session.usage().failed_work.is_some());
}
