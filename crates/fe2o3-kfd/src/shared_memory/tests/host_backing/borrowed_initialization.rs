use super::*;
use crate::shared_memory::coherent_initialization::{CoherentInitializationV1, initialize_v1};

struct Harness {
    engine: SharedMemoryEngine<FakeBackend>,
    stages: [bool; 3],
    source: Option<(usize, usize)>,
}

impl Harness {
    fn new(bytes: u64, records: usize) -> Self {
        Self {
            engine: configured(bytes, records),
            stages: [false; 3],
            source: None,
        }
    }
}

impl CoherentInitializationV1 for Harness {
    fn allocate(&mut self, length: usize) -> Result<HostCpu, MemorySessionError> {
        self.stages[0] = true;
        self.engine.allocate(length)
    }

    fn copy(&mut self, token: HostCpu, source: &[u8]) -> Result<HostCpu, MemorySessionError> {
        self.stages[1] = true;
        self.source = Some((source.as_ptr() as usize, source.len()));
        crate::shared_memory::transitions::copy_coherent_v1(&mut self.engine, token, source)
    }

    fn map(&mut self, token: HostCpu) -> Result<HostMapped, MemorySessionError> {
        self.stages[2] = true;
        self.engine.map_mutable(token)
    }
}

#[test]
fn borrowed_initialization_copies_complete_source_and_preserves_one_padded_debit() {
    for length in [1usize, 4096, 4097] {
        let mut memory = Harness::new(8192, 2);
        let mut source = (0..length).map(|i| (i % 251) as u8).collect::<Vec<_>>();
        let original = (source.as_ptr() as usize, source.len(), source.capacity());
        let result = initialize_v1(&mut memory, &source).unwrap();
        assert_eq!(memory.stages, [true; 3]);
        assert_eq!(memory.source, Some((original.0, original.1)));
        assert_eq!(
            (source.as_ptr() as usize, source.len(), source.capacity()),
            original
        );
        assert!(
            source
                .iter()
                .enumerate()
                .all(|(i, &b)| b == (i % 251) as u8)
        );
        let identity = result.storage_identity();
        let padded = length.div_ceil(4096) as u64 * 4096;
        debit(&memory.engine, padded, 1);
        assert_eq!(result.layout().requested_bytes(), length);
        assert_eq!(memory.engine.backend.map_gpu_calls, 1);
        source.fill(0xa5);
        drop(source);
        let token = result.into_token();
        assert_eq!(token.storage_identity(), identity);
        let token = memory.engine.unmap_mutable(token).unwrap();
        debit(&memory.engine, padded, 1);
        memory
            .engine
            .with_bytes(&token, SharedAllocationPhaseV1::CpuWritable, |bytes| {
                assert_eq!(bytes.len(), length);
                assert!(bytes.iter().enumerate().all(|(i, &b)| b == (i % 251) as u8));
            })
            .unwrap();
        debit(&memory.engine, padded, 1);
        memory
            .engine
            .release(token, SharedAllocationPhaseV1::CpuWritable)
            .unwrap();
        debit(&memory.engine, 0, 0);
        assert_eq!(memory.engine.backend.free_calls, 1);
        assert_eq!(memory.engine.backend.release_va_calls, 1);
    }
}

#[test]
fn borrowed_initialization_empty_and_each_budget_reject_before_native_entry() {
    let mut empty = Harness::new(4096, 1);
    let before = calls(&empty.engine);
    assert!(matches!(
        initialize_v1(&mut empty, &[]),
        Err(MemorySessionError::InvalidRequestedSize)
    ));
    assert_eq!(empty.stages, [false; 3]);
    assert_eq!(calls(&empty.engine), before);
    debit(&empty.engine, 0, 0);

    for (bytes, records) in [(4096, 2), (8192, 1)] {
        let mut memory = Harness::new(bytes, records);
        let prior = initialize_v1(&mut memory, &[7]).unwrap();
        let before = calls(&memory.engine);
        memory.stages = [false; 3];
        assert!(matches!(
            initialize_v1(&mut memory, &[9]),
            Err(MemorySessionError::HostVisibleBackingCredits(_))
        ));
        assert_eq!(memory.stages, [true, false, false]);
        assert_eq!(calls(&memory.engine), before);
        debit(&memory.engine, 4096, 1);
        assert_eq!(memory.engine.allocations.len(), 1);
        let token = memory.engine.unmap_mutable(prior.into_token()).unwrap();
        memory
            .engine
            .release(token, SharedAllocationPhaseV1::CpuWritable)
            .unwrap();
        debit(&memory.engine, 0, 0);
    }
}

#[test]
fn borrowed_initialization_failures_retain_backing_without_initialized_authority_or_retry() {
    for operation in [
        "reserve_va",
        "alloc",
        "map_cpu",
        "prepare_cpu_mapping",
        "map_gpu",
    ] {
        let mut memory = Harness::new(8192, 2);
        memory.engine.backend.fail_operation = Some(operation);
        let source = [0x5a; 4097];
        assert!(initialize_v1(&mut memory, &source).is_err(), "{operation}");
        assert_eq!(source, [0x5a; 4097]);
        debit(&memory.engine, 8192, 1);
        assert_eq!(
            memory.stages,
            [true, operation == "map_gpu", operation == "map_gpu"]
        );
        assert_eq!(
            memory.engine.allocations.len(),
            usize::from(operation == "map_gpu")
        );
        assert_eq!(memory.engine.backend.free_calls, 0);
        assert_eq!(memory.engine.backend.release_va_calls, 0);
        closed(&mut memory.engine);
    }
    for (progress, errno) in [(0, false), (0, true), (1, true), (2, false)] {
        let mut memory = Harness::new(8192, 2);
        memory.engine.backend.map_progress = progress;
        memory.engine.backend.map_errno = errno;
        assert!(initialize_v1(&mut memory, &[0x5a; 4097]).is_err());
        debit(&memory.engine, 8192, 1);
        assert_eq!(memory.engine.allocations.len(), 1);
        assert_eq!(memory.engine.backend.map_gpu_calls, 1);
        assert_eq!(memory.engine.backend.unmap_gpu_calls, 0);
        assert_eq!(memory.engine.backend.free_calls, 0);
        assert_eq!(memory.engine.backend.release_va_calls, 0);
        closed(&mut memory.engine);
    }
}

#[test]
fn borrowed_initialization_panics_preserve_original_payload_and_quarantined_charge() {
    for operation in [
        "reserve_va",
        "alloc",
        "map_cpu",
        "prepare_cpu_mapping",
        "with_bytes_mut",
        "map_gpu",
    ] {
        let mut memory = Harness::new(8192, 2);
        memory.engine.backend.panic_operation = Some(operation);
        let before = memory.engine.backend.currentness_calls;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            initialize_v1(&mut memory, &[0x5a; 4097])
        }));
        let payload = result.unwrap_err();
        panic_payload(payload.as_ref(), "N2 native panic", operation);
        debit(&memory.engine, 8192, 1);
        if operation == "with_bytes_mut" {
            assert_eq!(memory.stages, [true, true, false]);
            assert_eq!(memory.engine.backend.map_gpu_calls, 0);
            assert_eq!(memory.engine.backend.currentness_calls - before, 4);
        }
        assert_eq!(memory.engine.backend.free_calls, 0);
        assert_eq!(memory.engine.backend.release_va_calls, 0);
        closed(&mut memory.engine);
    }
}

#[test]
fn borrowed_initialization_all_currentness_boundaries_preserve_existing_prefix_and_debits() {
    let mut probe = Harness::new(16384, 3);
    let before = probe.engine.backend.currentness_calls;
    let initialized = initialize_v1(&mut probe, &[3]).unwrap();
    let steps = probe.engine.backend.currentness_calls - before;
    assert_eq!(steps, 7);
    let token = probe
        .engine
        .unmap_mutable(initialized.into_token())
        .unwrap();
    probe
        .engine
        .release(token, SharedAllocationPhaseV1::CpuWritable)
        .unwrap();

    for panic in [false, true] {
        for step in 1..=steps {
            let mut memory = Harness::new(16384, 3);
            let prior = initialize_v1(&mut memory, &[7]).unwrap();
            let prior_identity = prior.storage_identity();
            let prior_reservations = memory.engine.backend.reserve_va_calls;
            let failure_at = memory.engine.backend.currentness_calls + step;
            if panic {
                memory.engine.backend.panic_currentness_at = Some(failure_at);
            } else {
                memory.engine.backend.fail_currentness_at = Some(failure_at);
            }
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                initialize_v1(&mut memory, &[9])
            }));
            if panic {
                panic_payload(
                    result.unwrap_err().as_ref(),
                    "N2 native panic",
                    "currentness",
                );
            } else {
                assert!(result.unwrap().is_err());
            }
            debit(
                &memory.engine,
                if step == 1 { 4096 } else { 8192 },
                if step == 1 { 1 } else { 2 },
            );
            assert_eq!(prior.storage_identity(), prior_identity);
            assert_eq!(
                memory.engine.backend.reserve_va_calls,
                prior_reservations + usize::from(step != 1)
            );
            assert_eq!(
                usage(&memory.engine).quarantined_records,
                usize::from(step == 2 || step == 3)
            );
            assert_eq!(memory.engine.allocations.len(), 1 + usize::from(step >= 4));
            assert_eq!(
                memory.engine.allocations[0].mapping.as_ref().unwrap().bytes[0],
                7
            );
            assert_eq!(memory.engine.backend.free_calls, 0);
            closed(&mut memory.engine);
        }
    }
}

#[test]
fn borrowed_initialization_live_facade_uses_model_aware_sequence_without_encoded_copy() {
    let source = include_str!("../../coherent_initialization.rs");
    assert!(source.contains("self.allocate_host_visible_coherent(length)"));
    assert!(source.contains("transitions::copy_coherent_v1(&mut self.engine, allocation, source)"));
    assert!(source.contains("self.map_to_gpu(allocation)"));
    for copy in ["to_vec(", "to_owned(", "Box::from(", "Vec::", "clone("] {
        assert!(!source.contains(copy), "extra encoded copy: {copy}");
    }
    let source = include_str!("../../../shared_memory.rs");
    let owned = source
        .split("pub fn initialize_host_visible_coherent(")
        .nth(1)
        .unwrap()
        .split("/// Copies the complete borrowed")
        .next()
        .unwrap();
    assert!(owned.contains("self.initialize_host_visible_coherent_from_slice_v1(&bytes)"));
}
