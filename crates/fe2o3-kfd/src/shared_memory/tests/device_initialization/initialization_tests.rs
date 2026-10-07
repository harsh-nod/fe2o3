use super::*;

#[test]
fn device_initializer_complete_entry_preserves_exact_success_and_readback_policy() {
    for configured in [false, true] {
        for repeated in [false, true] {
            for len in [1, 4096, 4097] {
                let mut fixture = Fixture::new(configured);
                let mut input = Input::new(repeated, len);
                fixture.memory.engine.backend.corrupt_readback = repeated;
                let id = fixture.memory.engine.next_device_memory_id;
                let currentness = fixture.memory.engine.backend.currentness_calls;
                let output = input.run(&mut fixture, 4096).unwrap();
                let engine = &fixture.memory.engine;
                let record = &engine.device_memory[1];
                assert_eq!(output.lease.id, id);
                assert_eq!(output.lease.generation, 1);
                assert_eq!(output.lease.device, fixture.memory.device.model_key());
                assert_eq!(output.lease.vm, fixture.memory.vm);
                assert_eq!(output.lease.layout, record.layout);
                assert_eq!(output.content(), input.content);
                assert_eq!(engine.backend.currentness_calls, currentness + 6);
                assert_eq!(record.phase, DeviceMemoryPhaseV1::Mapped);
                assert!(record.mapping.is_none());
                assert!(engine.terminal_device_initialization.is_none());
                assert_eq!(
                    engine.backend.flags[1],
                    KFD_ALLOC_MEMORY_FLAGS_DEVICE_LOCAL_PUBLIC
                );
                assert_eq!(
                    engine.backend.last_unmapped_readback_calls,
                    usize::from(!repeated)
                );
                let bytes = engine.backend.last_unmapped_bytes.as_ref().unwrap();
                assert_eq!(&bytes[..len], input.expected);
                assert!(bytes[len..].iter().all(|byte| *byte == 0));
                assert_eq!(
                    engine.backend.map_cpu_inputs,
                    [(
                        record.reservation.unwrap(),
                        record.mmap_offset,
                        record.layout.backing_bytes as usize
                    )]
                );
                assert_eq!(
                    engine.backend.map_gpu_inputs[1],
                    (record.handle.unwrap(), 0)
                );
                assert_eq!(
                    &engine.backend.operations[1..],
                    ["map_cpu", "prepare_cpu_mapping", "unmap_cpu", "map_gpu"]
                );
                fixture.assert_usage(4096 + record.layout.backing_bytes, 2, 0);
                fixture.assert_anchor();
            }
        }
    }
}

#[test]
fn device_initializer_currentness_matrix_retains_only_admitted_owners() {
    for configured in [false, true] {
        for repeated in [false, true] {
            for panic in [false, true] {
                for ordinal in 1..=6 {
                    let mut fixture = Fixture::new(configured);
                    let mut input = Input::new(repeated, 4097);
                    let backend = &mut fixture.memory.engine.backend;
                    let at = backend.currentness_calls + ordinal;
                    if panic {
                        backend.panic_currentness_at = Some(at);
                    } else {
                        backend.fail_currentness_at = Some(at);
                    }
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        input.run(&mut fixture, 4096)
                    }));
                    if panic {
                        assert_eq!(
                            result.unwrap_err().downcast_ref::<(&str, &str)>(),
                            Some(&("N2 native panic", "currentness"))
                        );
                    } else {
                        assert!(matches!(
                            result.unwrap(),
                            Err(MemorySessionError::Injected("currentness"))
                        ));
                    }
                    if ordinal == 1 {
                        assert!(
                            fixture
                                .memory
                                .engine
                                .terminal_device_initialization
                                .is_none()
                        );
                        assert_eq!(fixture.memory.engine.device_memory.len(), 1);
                        fixture.assert_usage(4096, 1, 0);
                        fixture.assert_anchor();
                        if panic && !configured {
                            assert_eq!(
                                fixture.memory.engine.phase(),
                                SharedMemorySessionPhaseV1::Active
                            );
                            fixture.memory.engine.backend.panic_currentness_at = None;
                            let _initialized =
                                Input::new(repeated, 17).run(&mut fixture, 4096).unwrap();
                            fixture.assert_anchor();
                        } else {
                            fixture.no_retry(&input);
                        }
                        continue;
                    }
                    let stage = match ordinal {
                        2 => Stage::Allocate,
                        3 => Stage::CpuCurrentness,
                        4 => Stage::CpuClosingCurrentness,
                        _ => Stage::GpuMap,
                    };
                    fixture.assert_terminal(&input, stage, ordinal >= 3);
                    let engine = &fixture.memory.engine;
                    assert_eq!(engine.device_memory.len(), 2);
                    assert!(engine.device_memory[1].mapping.is_none());
                    assert_eq!(
                        engine.device_memory[1].phase,
                        if ordinal == 2 || ordinal == 6 {
                            DeviceMemoryPhaseV1::Ambiguous
                        } else {
                            DeviceMemoryPhaseV1::Unmapped
                        }
                    );
                    let progress = engine
                        .terminal_device_initialization
                        .as_ref()
                        .unwrap()
                        .progress;
                    assert_eq!(progress.attempted, ordinal == 6);
                    assert_eq!(progress.returned_success, (ordinal == 6).then_some(true));
                    assert_eq!(progress.returned_map_prefix, (ordinal == 6).then_some(1));
                    fixture.assert_usage(12_288, 2, 0);
                    fixture.no_retry(&input);
                }
            }
        }
    }
}

#[test]
fn device_initializer_native_error_and_panic_prefixes_preserve_original_source() {
    for configured in [false, true] {
        for repeated in [false, true] {
            for panic in [false, true] {
                for (operation, stage) in [
                    ("reserve_va", Stage::Allocate),
                    ("alloc", Stage::Allocate),
                    ("map_cpu", Stage::CpuMap),
                    ("prepare_cpu_mapping", Stage::CpuPrepare),
                    ("unmap_cpu", Stage::CpuUnmap),
                    ("map_gpu", Stage::GpuMap),
                    ("with_bytes_mut", Stage::CpuWrite),
                    ("with_bytes", Stage::CpuVerify),
                ] {
                    if (operation.starts_with("with_bytes") && !panic)
                        || (operation == "with_bytes" && repeated)
                    {
                        continue;
                    }
                    let mut fixture = Fixture::new(configured);
                    let mut input = Input::new(repeated, 4097);
                    if panic {
                        fixture.memory.engine.backend.panic_operation = Some(operation);
                    } else {
                        fixture.memory.engine.backend.fail_operation = Some(operation);
                    }
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        input.run(&mut fixture, 4096)
                    }));
                    if panic {
                        assert_eq!(
                            result.unwrap_err().downcast_ref::<(&str, &str)>(),
                            Some(&("N2 native panic", operation))
                        );
                    } else {
                        assert!(
                            matches!(result.unwrap(), Err(MemorySessionError::Injected(observed)) if observed == operation)
                        );
                    }
                    let allocated = operation != "reserve_va" && operation != "alloc";
                    fixture.assert_terminal(&input, stage, allocated);
                    let engine = &fixture.memory.engine;
                    if operation == "reserve_va" {
                        assert_eq!(engine.device_memory.len(), 1);
                        assert_eq!(engine.retained_device_memory_bytes, 4096);
                        fixture.assert_usage(12_288, 2, 1);
                    } else {
                        let record = &engine.device_memory[1];
                        assert_eq!(record.handle.is_some(), !(operation == "alloc" && panic));
                        let mapped = matches!(
                            operation,
                            "prepare_cpu_mapping" | "unmap_cpu" | "with_bytes" | "with_bytes_mut"
                        );
                        assert_eq!(record.mapping.is_some(), mapped);
                        if let Some(mapping) = &record.mapping {
                            assert!(mapping.active);
                            assert_eq!(mapping.writable, operation != "prepare_cpu_mapping");
                            if operation == "unmap_cpu" || operation == "with_bytes" {
                                assert_eq!(&mapping.bytes[..4097], input.expected);
                            } else {
                                assert!(mapping.bytes.iter().all(|byte| *byte == 0));
                            }
                            assert!(mapping.bytes[4097..].iter().all(|byte| *byte == 0));
                        }
                        let progress = engine
                            .terminal_device_initialization
                            .as_ref()
                            .unwrap()
                            .progress;
                        assert_eq!(progress.attempted, operation == "map_gpu");
                        assert_eq!(
                            progress.returned_success,
                            (operation == "map_gpu" && !panic).then_some(false)
                        );
                        assert_eq!(
                            progress.returned_map_prefix,
                            (operation == "map_gpu" && !panic).then_some(1)
                        );
                        fixture.assert_usage(12_288, 2, 0);
                    }
                    fixture.no_retry(&input);
                }
            }
        }
    }
}

#[test]
fn device_initializer_gpu_prefixes_and_readback_rejection_never_produce_output() {
    for configured in [false, true] {
        for repeated in [false, true] {
            for prefix in [0, 1, 2] {
                for errno in [false, true] {
                    if prefix == 1 && !errno {
                        continue;
                    }
                    let mut fixture = Fixture::new(configured);
                    let mut input = Input::new(repeated, 4097);
                    fixture.memory.engine.backend.map_progress = prefix;
                    fixture.memory.engine.backend.map_errno = errno;
                    assert!(input.run(&mut fixture, 4096).is_err());
                    fixture.assert_terminal(&input, Stage::GpuMap, true);
                    let engine = &fixture.memory.engine;
                    let progress = engine
                        .terminal_device_initialization
                        .as_ref()
                        .unwrap()
                        .progress;
                    assert!(progress.attempted);
                    assert_eq!(progress.returned_success, Some(!errno));
                    assert_eq!(progress.returned_map_prefix, Some(prefix));
                    assert_eq!(
                        engine.device_memory[1].phase,
                        DeviceMemoryPhaseV1::Ambiguous
                    );
                    assert!(engine.device_memory[1].mapping.is_none());
                    fixture.assert_usage(12_288, 2, 0);
                    fixture.no_retry(&input);
                }
            }
        }
        let mut fixture = Fixture::new(configured);
        let mut input = Input::new(false, 4097);
        fixture.memory.engine.backend.corrupt_readback = true;
        assert!(matches!(
            input.run(&mut fixture, 4096),
            Err(MemorySessionError::DeviceContentMismatch)
        ));
        fixture.assert_terminal(&input, Stage::CpuVerify, true);
        assert_eq!(
            fixture.memory.engine.device_memory[1]
                .mapping
                .as_ref()
                .unwrap()
                .readback_calls
                .get(),
            1
        );
        fixture.no_retry(&input);
    }
}

#[test]
fn device_initializer_preflight_rejects_before_native_effects_despite_prior_activity() {
    for configured in [false, true] {
        for invalid in ["empty", "length", "digest", "mutated", "alignment"] {
            let mut fixture = Fixture::new(configured);
            let mut input = Input::new(false, 17);
            match invalid {
                "empty" => input.bytes = Some(Vec::new().into_boxed_slice()),
                "length" => input.bytes = Some(vec![1; 16].into_boxed_slice()),
                "digest" => input.content = content(&[1; 17]),
                "mutated" => input.bytes.as_mut().unwrap()[0] ^= 1,
                _ => {}
            }
            let calls = fixture.calls();
            assert!(
                input
                    .run(&mut fixture, if invalid == "alignment" { 3 } else { 4096 })
                    .is_err()
            );
            assert_eq!(fixture.calls(), calls);
            assert!(
                fixture
                    .memory
                    .engine
                    .terminal_device_initialization
                    .is_none()
            );
            assert_eq!(
                fixture.memory.engine.phase(),
                SharedMemorySessionPhaseV1::Active
            );
            fixture.assert_usage(4096, 1, 0);
            fixture.assert_anchor();
            let _initialized = Input::new(false, 17).run(&mut fixture, 4096).unwrap();
        }
    }
}

#[test]
fn device_initializer_existing_lease_preflight_retains_genuine_native_input() {
    for configured in [false, true] {
        for invalid in ["length", "validated_length", "validated_content", "profile"] {
            let mut fixture = Fixture::new(configured);
            let mut input = Input::new(false, 17);
            let mut source =
                validate_initialization_source(input.bytes.take().unwrap(), input.content).unwrap();
            if invalid == "validated_length" {
                source.byte_len += 1;
            }
            if invalid == "validated_content" {
                source.content = content(&[1; 18]);
            }
            let lease = fixture
                .memory
                .engine
                .allocate_device_memory_with_flags(
                    fixture.memory.device.model_key(),
                    fixture.memory.vm,
                    if invalid == "length" { 18 } else { 17 },
                    4096,
                    if invalid == "profile" {
                        KfdAllocMemoryFlags::DEVICE_LOCAL
                    } else {
                        KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC
                    },
                )
                .unwrap();
            let before = snapshot(&fixture.memory.engine.device_memory[1]);
            let calls = fixture.calls();
            let original_source = (source.byte_len(), source.content());
            assert!(matches!(
                fixture
                    .memory
                    .engine
                    .initialize_public_device_memory(lease, source),
                Err(MemorySessionError::DeviceContentMismatch)
            ));
            assert_eq!(fixture.calls(), calls);
            assert_eq!(
                fixture.memory.engine.phase(),
                SharedMemorySessionPhaseV1::Quarantined
            );
            assert_eq!(snapshot(&fixture.memory.engine.device_memory[1]), before);
            let root = fixture
                .memory
                .engine
                .terminal_device_initialization
                .as_ref()
                .unwrap();
            assert!(root.input_admitted && !root.native_started && root.failed);
            let init::InitializationLeaseV1::Unmapped(lease) = &root.lease else {
                panic!("lost original lease");
            };
            assert_eq!(
                (
                    lease.id,
                    lease.generation,
                    lease.device,
                    lease.vm,
                    lease.layout
                ),
                before.identity
            );
            let Some(init::InitializationSourceV1::Validated(source)) = &root.source else {
                panic!("lost original source");
            };
            assert_eq!(source.bytes(), input.expected);
            assert_eq!(Some(source.bytes().as_ptr() as usize), input.pointer);
            assert_eq!((source.byte_len(), source.content()), original_source);
            fixture.assert_usage(8192, 2, 0);
            fixture.assert_anchor();
        }
    }
}

#[test]
fn device_initializer_in_place_core_keeps_external_custody_until_extraction() {
    for configured in [false, true] {
        for outcome in ["success", "error", "panic"] {
            let mut fixture = Fixture::new(configured);
            let mut input = Input::new(false, 17);
            let mut root = init::DeviceInitializationCustodyV1::new(
                init::InitializationSourceV1::Unvalidated(
                    input.bytes.take().unwrap(),
                    input.content,
                ),
                init::InitializationLeaseV1::None,
            );
            if outcome == "error" {
                fixture.memory.engine.backend.fail_operation = Some("map_gpu");
            }
            if outcome == "panic" {
                fixture.memory.engine.backend.panic_operation = Some("map_gpu");
            }
            let request = init::AllocationRequestV1 {
                device: fixture.memory.device.model_key(),
                vm: fixture.memory.vm,
                alignment: 4096,
            };
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                root.prepare_in_place(&mut fixture.memory.engine, Some(request))
            }));
            if outcome == "panic" {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<(&str, &str)>(),
                    Some(&("N2 native panic", "map_gpu"))
                );
            } else {
                assert_eq!(result.unwrap().is_err(), outcome == "error");
            }
            assert!(
                fixture
                    .memory
                    .engine
                    .terminal_device_initialization
                    .is_none()
            );
            input.assert_source(&root);
            if outcome != "success" {
                assert_eq!(
                    fixture.memory.engine.phase(),
                    SharedMemorySessionPhaseV1::Quarantined
                );
                let before = root_snapshot(&root);
                assert!(root.take_complete().is_err());
                let calls = fixture.calls();
                assert!(
                    root.prepare_in_place(&mut fixture.memory.engine, Some(request))
                        .is_err()
                );
                assert_eq!(fixture.calls(), calls);
                assert!(root.take_complete().is_err());
                assert_eq!(root_snapshot(&root), before);
                assert!(matches!(
                    root.lease,
                    init::InitializationLeaseV1::Unmapped(_)
                ));
            } else {
                let output = root.take_complete().unwrap();
                assert_eq!(output.content(), input.content);
                assert!(root.take_complete().is_err());
            }
            fixture.assert_anchor();
        }
    }
}

#[test]
fn device_initializer_capacity_rejection_keeps_source_host_only_and_allows_later_retry() {
    for configured in [false, true] {
        let mut fixture = Fixture::new(configured);
        let capacity = if configured {
            8
        } else {
            MAX_GFX942_DEVICE_MEMORY_ALLOCATION_RECORDS_V1
        };
        let mut extras = Vec::new();
        for _ in 1..capacity {
            extras.push(
                fixture
                    .memory
                    .engine
                    .allocate_device_memory(
                        fixture.memory.device.model_key(),
                        fixture.memory.vm,
                        17,
                        4096,
                    )
                    .unwrap(),
            );
        }
        let calls = fixture.calls();
        let usage = fixture.memory.usage();
        for repeated in [false, true] {
            assert!(Input::new(repeated, 17).run(&mut fixture, 4096).is_err());
            assert_eq!(fixture.calls(), calls);
            assert_eq!(fixture.memory.usage(), usage);
            assert!(
                fixture
                    .memory
                    .engine
                    .terminal_device_initialization
                    .is_none()
            );
            assert_eq!(
                fixture.memory.engine.phase(),
                SharedMemorySessionPhaseV1::Active
            );
            fixture.assert_anchor();
        }
        fixture
            .memory
            .engine
            .release_device_memory(extras.pop().unwrap())
            .unwrap();
        let _initialized = Input::new(false, 17).run(&mut fixture, 4096).unwrap();
        assert_eq!(
            snapshot(&fixture.memory.engine.device_memory[0]),
            fixture.anchor_native
        );
        assert_eq!(fixture.memory.engine.backend.free_calls, 1);
        assert_eq!(fixture.memory.engine.backend.release_va_calls, 1);
    }
}

#[test]
fn device_initializer_malformed_allocations_retain_source_without_fabricated_lease() {
    for configured in [false, true] {
        for repeated in [false, true] {
            for fault in [
                "oom",
                "va",
                "size",
                "gpu",
                "flags",
                "handle",
                "offset",
                "alignment",
                "collision",
                "overlap",
            ] {
                let mut fixture = Fixture::new(configured);
                let mut input = Input::new(repeated, 4097);
                let backend = &mut fixture.memory.engine.backend;
                match fault {
                    "oom" => backend.alloc_oom = true,
                    "va" => backend.allocation_output_mutator = Some(|args| args.va_addr += 4096),
                    "size" => backend.allocation_output_mutator = Some(|args| args.size += 4096),
                    "gpu" => backend.allocation_output_mutator = Some(|args| args.gpu_id += 1),
                    "flags" => backend.corrupt_flags = true,
                    "handle" => backend.allocation_output_mutator = Some(|args| args.handle = 0),
                    "offset" => {
                        backend.allocation_output_mutator = Some(|args| args.mmap_offset = 0)
                    }
                    "alignment" => {
                        backend.allocation_output_mutator = Some(|args| args.mmap_offset += 1)
                    }
                    "collision" => backend.allocation_output_mutator = Some(|args| args.handle = 1),
                    "overlap" => backend.fixed_va = Some(fixture.anchor_native.gpu_va),
                    _ => unreachable!(),
                }
                assert!(input.run(&mut fixture, 4096).is_err());
                fixture.assert_terminal(&input, Stage::Allocate, false);
                let engine = &fixture.memory.engine;
                let record = &engine.device_memory[1];
                assert_eq!(record.phase, DeviceMemoryPhaseV1::Ambiguous);
                assert!(record.mapping.is_none());
                if fault == "overlap" {
                    assert!(record.handle.is_none());
                    assert_eq!(record.mmap_offset, 0);
                } else {
                    let output = engine.backend.last_allocation_output.unwrap();
                    assert_eq!(record.handle, (output.handle != 0).then_some(output.handle));
                    assert_eq!(record.mmap_offset, output.mmap_offset);
                }
                assert_eq!(engine.backend.map_cpu_calls, 0);
                assert_eq!(engine.backend.map_gpu_calls, 1);
                fixture.assert_usage(12_288, 2, 0);
                fixture.no_retry(&input);
            }
        }
    }
}

#[test]
fn device_initializer_lower_entry_rejects_foreign_coordinates_and_account_before_effects() {
    for configured in [false, true] {
        for coordinate in ["id", "generation", "device", "vm", "layout", "account"] {
            if coordinate == "account" && !configured {
                continue;
            }
            let mut fixture = Fixture::new(configured);
            let mut input = Input::new(false, 17);
            let source =
                validate_initialization_source(input.bytes.take().unwrap(), input.content).unwrap();
            let mut lease = fixture
                .memory
                .engine
                .allocate_device_memory_with_flags(
                    fixture.memory.device.model_key(),
                    fixture.memory.vm,
                    17,
                    4096,
                    KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC,
                )
                .unwrap();
            let original_account = if coordinate == "account" {
                let engine = &mut fixture.memory.engine;
                let replacement = DeviceBackingAccountV1::new(
                    engine.session_id,
                    fixture.memory.device.model_key(),
                    fixture.memory.vm,
                    Gfx942DeviceBackingBudgetV1::new(32_768, 8).unwrap(),
                )
                .unwrap();
                engine.device_backing_account.replace(replacement)
            } else {
                None
            };
            match coordinate {
                "id" => lease.id += 1,
                "generation" => lease.generation += 1,
                "device" => lease.device.generation.0 += 1,
                "vm" => lease.vm.id.0 += 1,
                "layout" => lease.layout.requested_bytes += 1,
                _ => {}
            }
            let records: Vec<_> = fixture
                .memory
                .engine
                .device_memory
                .iter()
                .map(snapshot)
                .collect();
            let calls = fixture.calls();
            assert!(matches!(
                fixture
                    .memory
                    .engine
                    .initialize_public_device_memory(lease, source),
                Err(MemorySessionError::InvalidDeviceMemoryAuthority)
            ));
            assert_eq!(fixture.calls(), calls);
            assert_eq!(
                fixture
                    .memory
                    .engine
                    .device_memory
                    .iter()
                    .map(snapshot)
                    .collect::<Vec<_>>(),
                records
            );
            assert!(
                fixture
                    .memory
                    .engine
                    .terminal_device_initialization
                    .is_none()
            );
            assert_eq!(
                fixture.memory.engine.phase(),
                SharedMemorySessionPhaseV1::Active
            );
            if let Some(account) = original_account {
                fixture.memory.engine.device_backing_account = Some(account);
            }
            fixture.assert_anchor();
        }
    }
}

#[test]
fn device_initializer_terminal_storage_is_preallocated_and_cannot_overwrite_custody() {
    assert!(
        std::mem::size_of::<init::TerminalInitializationSlotV1>()
            <= 3 * std::mem::size_of::<usize>()
    );
    for configured in [false, true] {
        for repeated in [false, true] {
            let mut fixture = Fixture::new(configured);
            assert!(fixture.terminal_storage.1 >= 1);
            assert!(
                fixture
                    .memory
                    .engine
                    .terminal_device_initialization
                    .is_none()
            );
            let mut input = Input::new(repeated, 4097);
            fixture.memory.engine.backend.fail_operation = Some("map_cpu");
            assert!(matches!(
                input.run(&mut fixture, 4096),
                Err(MemorySessionError::Injected("map_cpu"))
            ));
            fixture.assert_terminal(&input, Stage::CpuMap, true);
            let original = root_snapshot(
                fixture
                    .memory
                    .engine
                    .terminal_device_initialization
                    .as_ref()
                    .unwrap(),
            );
            let replacement = init::DeviceInitializationCustodyV1::new(
                init::InitializationSourceV1::Repeated(repeated_content(17, 0x33)),
                init::InitializationLeaseV1::None,
            );
            let rejected = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                fixture
                    .memory
                    .engine
                    .terminal_device_initialization
                    .retain(replacement);
            }));
            assert_eq!(
                rejected.unwrap_err().downcast_ref::<&str>(),
                Some(&"occupied initialization custody")
            );
            assert_eq!(
                root_snapshot(
                    fixture
                        .memory
                        .engine
                        .terminal_device_initialization
                        .as_ref()
                        .unwrap()
                ),
                original
            );
            fixture.no_retry(&input);
        }
    }
}
