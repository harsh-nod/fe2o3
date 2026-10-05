use super::*;

#[test]
fn compact_layout_count_alignment_and_distinct_slot_boundaries() {
    for count in [1, 63, 64, 65, 652, 1024] {
        let extents: Vec<_> = (0..count)
            .map(|index| (index % 127, 1u64 << (index % 13)))
            .collect();
        let layout = ProgramLayout::new(&extents).unwrap();
        assert_eq!(layout.slots.len(), count);
        assert_eq!(layout.signal_bytes, (count * 64 + 4095) & !4095);
        assert!(layout.kernarg_bytes.is_multiple_of(PAGE_BYTES));
        assert!(layout.signal_bytes <= 65536);
        assert_eq!(
            layout.initialized_bytes,
            extents.iter().map(|(bytes, _)| *bytes as u64).sum::<u64>()
        );
        let mut previous_end = 0;
        for (offset, bytes, alignment) in layout.slots {
            assert!(offset >= previous_end);
            assert!(offset.is_multiple_of(alignment as usize));
            previous_end = offset + bytes.max(1);
            assert!(previous_end <= layout.kernarg_bytes);
        }
    }
    for count in [0, 1025] {
        assert!(ProgramLayout::new(&vec![(128, 8); count]).is_err());
    }
}

#[test]
fn compact_layout_rejects_bad_extents_and_preserves_exact_initialization_cost() {
    for extent in [
        (65537, 8),
        (usize::MAX, 8),
        (1, 0),
        (1, 3),
        (1, 8192),
        (1, u64::MAX),
    ] {
        assert!(ProgramLayout::new(&[extent]).is_err());
    }
    let maximal = ProgramLayout::new(&vec![(65536, 4096); 1024]).unwrap();
    assert_eq!(maximal.kernarg_bytes, 64 * 1024 * 1024);
    assert_eq!(maximal.signal_bytes, 65536);
    let compact = ProgramLayout::new(&vec![(120, 8); 652]).unwrap();
    assert_eq!(compact.initialized_bytes, 78_240);
    assert_eq!(compact.kernarg_bytes, 81_920);
    assert!(compact.initialized_bytes < 652 * 65536);
}

#[test]
fn retained_layout_reuses_only_the_same_extent_until_confirmed_release() {
    let retained = ProgramLayout::new(&vec![(120, 8); 65]).unwrap();
    assert!(!require_storage_layout(None, 0, &retained).unwrap());
    assert!(require_storage_layout(Some(&retained), 2, &retained).unwrap());
    for count in [0, 1, 3] {
        assert!(require_storage_layout(Some(&retained), count, &retained).is_err());
    }
    for count in [1, 2, 3] {
        assert!(require_storage_layout(None, count, &retained).is_err());
    }
    for changed in [
        ProgramLayout::new(&vec![(120, 8); 64]).unwrap(),
        ProgramLayout::new(&vec![(119, 8); 65]).unwrap(),
        ProgramLayout::new(&vec![(120, 16); 65]).unwrap(),
    ] {
        assert!(require_storage_layout(Some(&retained), 2, &changed).is_err());
        assert!(!require_storage_layout(None, 0, &changed).unwrap());
    }
}

#[test]
fn final_completion_never_substitutes_for_earlier_retained_signal_observation() {
    for bad_slot in [0, 63, 64, 650] {
        for observation in [
            AqlCompletionObservationV1::Pending,
            AqlCompletionObservationV1::Unexpected(-1),
        ] {
            let mut signals = vec![AqlCompletionObservationV1::Completed; 652];
            signals[bad_slot] = observation;
            assert_eq!(signals.last(), Some(&AqlCompletionObservationV1::Completed));
            assert!(require_signals_complete(signals).is_err());
        }
    }
    require_signals_complete(vec![AqlCompletionObservationV1::Completed; 1024]).unwrap();
}

#[test]
fn backend_identity_and_diagnostics_have_separate_exact_wire_schemas() {
    for (command, op) in [
        (
            CommandV1::DescribeTokenProgramBackendV1 {},
            "describe_token_program_backend_v1",
        ),
        (
            CommandV1::TokenProgramSnapshotV1 {},
            "token_program_snapshot_v1",
        ),
    ] {
        assert_eq!(command.payload_bytes().unwrap(), 0);
        let mut value = serde_json::to_value(&command).unwrap();
        assert_eq!(value["op"], op);
        assert_eq!(
            serde_json::from_value::<CommandV1>(value.clone()).unwrap(),
            command
        );
        value["payload_bytes"] = 0.into();
        assert!(serde_json::from_value::<CommandV1>(value).is_err());
    }
    for backend in [
        "disabled",
        "ordered64-groups-v1",
        "native-whole-program-v1",
        "native-boundary-fences-v1",
    ] {
        let response = ResponseV1::TokenProgramBackendV1 {
            backend: backend.into(),
        };
        let value = serde_json::to_value(&response).unwrap();
        assert_eq!(value["op"], "token_program_backend_v1");
        assert_eq!(
            serde_json::from_value::<ResponseV1>(value).unwrap(),
            response
        );
    }
    let response = ResponseV1::TokenProgramSnapshotV1 {
        backend: "native-whole-program-v1".into(),
        counters: TokenProgramCountersV1 {
            executions: 1,
            dispatches: 652,
            publications: 1,
            final_waits: 1,
            retirement_signals: 652,
            staging_ns: 19,
            kernarg_initialized_bytes: 78240,
        },
    };
    let value = serde_json::to_value(&response).unwrap();
    assert_eq!(
        serde_json::from_value::<ResponseV1>(value).unwrap(),
        response
    );
}

#[test]
fn native_arena_lifecycle_keeps_default_storage_and_validation_separate() {
    let source = include_str!("engineering_gfx950_token_program_native.rs");
    let context = include_str!("engineering_gfx950.rs");
    let token = include_str!("engineering_gfx950_token_program.rs");
    assert!(source.contains("self.context.token_program_storage.allocations.push(kernarg)"));
    assert!(
        source.find("allocations.push(signal)").unwrap()
            < source
                .find("Backend::initialize_engineering_program_signals(")
                .unwrap()
    );
    assert!(!source.contains("self.context.internal[SIGNAL]"));
    assert!(context.contains("token_program_native: false"));
    assert!(context.contains("token_program_boundary_fences: false"));
    for section in ["fn rollover_queue(", "fn close_inner("] {
        let body = context
            .split(section)
            .nth(1)
            .unwrap()
            .split("\n    fn ")
            .next()
            .unwrap();
        assert!(
            body.find("self.destroy_queue()?").unwrap()
                < body.find("self.release_token_program_storage()?").unwrap()
        );
    }
    let execute = token
        .split("pub(super) fn execute_token_program(")
        .nth(1)
        .unwrap();
    assert!(
        execute.find("self.prepare_token_dispatches(").unwrap()
            < execute.find("self.run_prepared_token_program(").unwrap()
    );
    let release = token
        .split("pub(super) fn release_token_program(")
        .nth(1)
        .unwrap()
        .split("pub(super) fn execute_token_program(")
        .next()
        .unwrap();
    assert!(
        release.find("self.check_idle()?").unwrap()
            < release
                .find("self.release_token_program_storage()?")
                .unwrap()
    );
    assert!(
        release.find("require_identity(").unwrap()
            < release
                .find("self.release_token_program_storage()?")
                .unwrap()
    );
}
