use super::*;
use std::cell::RefCell;
use std::rc::Rc;

const EVENTS: [&str; 14] = [
    "load",
    "allocate-input",
    "allocate-payload",
    "allocate-state",
    "upload-input",
    "upload-payload",
    "initialize-state",
    "prepare",
    "submit",
    "complete",
    "read-input",
    "read-payload",
    "read-state",
    "close",
];

fn metadata(digest: [u8; 32], symbol: &str) -> KernelMetadataV1 {
    KernelMetadataV1 {
        symbol: symbol.into(),
        object_sha256: digest,
        kernarg_bytes: KERNARG_BYTES as u32,
        kernarg_alignment: 8,
        group_segment_bytes: 0,
        private_segment_bytes: 0,
        wavefront_size: 64,
        implicit_argument_offset: Some(24),
        implicit_argument_bytes: 256,
        explicit_arguments: (0..3)
            .map(|index| ExplicitArgumentV1 {
                offset: index as u32 * 8,
                bytes: 8,
                global_buffer: true,
                pointee_alignment: Some(4),
                access: Some(role_access(index)),
            })
            .collect(),
    }
}

fn regions() -> [OwnedRegion; 3] {
    core::array::from_fn(|index| OwnedRegion {
        buffer: index as u64 + 1,
        base: (index as u64 + 1) * 4096,
        requested: EXTENTS[index],
        backing: 4096,
    })
}

#[derive(Default)]
struct Trace {
    events: Vec<&'static str>,
    quarantined: bool,
    closed: bool,
    completed: bool,
    allocations: usize,
}

struct Fixture {
    trace: Rc<RefCell<Trace>>,
    failure: Option<usize>,
    initial: [u32; 6],
    final_state: [u32; 6],
    input: Vec<u8>,
    corrupt_input: bool,
    short_payload: bool,
}

impl Fixture {
    fn new() -> Self {
        Self {
            trace: Rc::default(),
            failure: None,
            initial: INITIAL_STATE,
            final_state: [1, 0, 7, 7, 0b10_01_10, 0],
            input: Vec::new(),
            corrupt_input: false,
            short_payload: false,
        }
    }

    fn step(&mut self, event: &'static str) -> Result<()> {
        let mut trace = self.trace.borrow_mut();
        let index = trace.events.len();
        assert_eq!(event, EVENTS[index]);
        trace.events.push(event);
        if self.failure == Some(index) {
            return Err(format!("injected {event}"));
        }
        Ok(())
    }
}

impl Transaction for Fixture {
    type Prepared = ();
    type Pending = ();
    fn load(
        &mut self,
        _object: Vec<u8>,
        digest: [u8; 32],
        symbol: String,
    ) -> Result<KernelMetadataV1> {
        self.step("load")?;
        Ok(metadata(digest, &symbol))
    }
    fn allocate(&mut self, role: usize) -> Result<OwnedRegion> {
        self.step(EVENTS[role + 1])?;
        self.trace.borrow_mut().allocations += 1;
        Ok(regions()[role])
    }
    fn upload(&mut self, buffer: u64, bytes: &[u8]) -> Result<()> {
        self.step(if buffer == 1 {
            "upload-input"
        } else {
            "upload-payload"
        })?;
        assert_eq!(bytes.len(), EXTENTS[buffer as usize - 1]);
        if buffer == 1 {
            self.input = bytes.to_vec();
        }
        Ok(())
    }
    fn initialize_state(&mut self, buffer: u64) -> Result<[u32; 6]> {
        self.step("initialize-state")?;
        assert_eq!(buffer, 3);
        Ok(self.initial)
    }
    fn prepare(
        &mut self,
        regions: &[OwnedRegion; 3],
        pointers: &[PointerFixupV1; 3],
    ) -> Result<()> {
        self.step("prepare")?;
        validate_regions(regions, pointers)
    }
    unsafe fn submit(&mut self, _prepared: (), timeout_ms: u32) -> Result<()> {
        self.step("submit")?;
        assert_eq!(timeout_ms, 100);
        Ok(())
    }
    fn complete(&mut self, _pending: ()) -> Result<u64> {
        self.step("complete")?;
        self.trace.borrow_mut().completed = true;
        Ok(17)
    }
    fn read(&mut self, buffer: u64, bytes: u32) -> Result<Vec<u8>> {
        assert!(self.trace.borrow().completed);
        self.step(if buffer == 1 {
            "read-input"
        } else {
            "read-payload"
        })?;
        assert_eq!(bytes as usize, EXTENTS[buffer as usize - 1]);
        if buffer == 1 {
            let mut input = self.input.clone();
            if self.corrupt_input {
                input[0] ^= 1;
            }
            Ok(input)
        } else {
            Ok(vec![0x3f; EXTENTS[1] - usize::from(self.short_payload)])
        }
    }
    fn read_state(&mut self, buffer: u64) -> Result<[u32; 6]> {
        assert!(self.trace.borrow().completed);
        self.step("read-state")?;
        assert_eq!(buffer, 3);
        Ok(self.final_state)
    }
    fn close(&mut self) -> Result<()> {
        assert!(self.trace.borrow().completed);
        self.step("close")?;
        self.trace.borrow_mut().closed = true;
        Ok(())
    }
    fn quarantine(self) {
        self.trace.borrow_mut().quarantined = true;
    }
}

fn run(fixture: Fixture, payload: &mut [f32; 384]) -> Result<Gfx950EngineeringFiniteJoinResultV1> {
    let object = b"CPU fixture, never executable".to_vec();
    let digest = Sha256::digest(&object).into();
    // SAFETY: this implementation only records CPU operations; it never creates
    // a Context, publishes a packet, or accesses a GPU.
    unsafe {
        coordinate(
            fixture,
            object,
            digest,
            "fixture".into(),
            &[2.; 256],
            payload,
            100,
        )
    }
}

#[test]
fn engineering_finite_join_coordinator_returns_only_after_complete_and_close() {
    let fixture = Fixture::new();
    let trace = fixture.trace.clone();
    let mut payload = [f32::from_bits(0x7fc0_0001); 384];
    let result = run(fixture, &mut payload).unwrap();
    assert_eq!(result.final_state, [1, 0, 7, 7, 0b10_01_10, 0]);
    assert_eq!(result.dispatch_elapsed_ns, 17);
    assert!(payload.iter().all(|value| value.to_bits() == 0x3f3f_3f3f));
    let trace = trace.borrow();
    assert_eq!(trace.events, EVENTS);
    assert!(trace.completed && trace.closed && !trace.quarantined);
    assert_eq!(trace.allocations, 3);
}

#[test]
fn engineering_finite_join_every_operation_failure_quarantines_without_output() {
    for failure in 0..EVENTS.len() {
        let mut fixture = Fixture::new();
        fixture.failure = Some(failure);
        let trace = fixture.trace.clone();
        let mut payload = [f32::from_bits(0x7fc0_0001); 384];
        assert!(run(fixture, &mut payload).is_err(), "failure {failure}");
        assert!(payload.iter().all(|value| value.to_bits() == 0x7fc0_0001));
        let trace = trace.borrow();
        assert_eq!(trace.events, EVENTS[..=failure]);
        assert!(trace.quarantined && !trace.closed);
        assert_eq!(trace.completed, failure > 9);
        assert_eq!(trace.allocations, failure.saturating_sub(1).min(3));
    }
}

#[test]
fn engineering_finite_join_state_initialization_mutants_stop_before_submit() {
    for word in 0..6 {
        let mut fixture = Fixture::new();
        fixture.initial[word] ^= 1;
        let trace = fixture.trace.clone();
        let mut payload = [7.; 384];
        assert!(run(fixture, &mut payload).is_err());
        assert_eq!(payload, [7.; 384]);
        assert_eq!(trace.borrow().events, EVENTS[..7]);
        assert!(trace.borrow().quarantined);
    }
}

#[test]
fn engineering_finite_join_readback_mutants_do_not_publish_output_or_close() {
    for mutation in 0..8 {
        let mut fixture = Fixture::new();
        match mutation {
            0 => fixture.corrupt_input = true,
            1 => fixture.short_payload = true,
            index => fixture.final_state[index - 2] ^= 1,
        }
        let trace = fixture.trace.clone();
        let mut payload = [7.; 384];
        assert!(run(fixture, &mut payload).is_err(), "mutation {mutation}");
        assert_eq!(payload, [7.; 384]);
        let trace = trace.borrow();
        assert!(trace.completed && trace.quarantined && !trace.closed);
        assert!(!trace.events.contains(&"close"));
    }
}

#[test]
fn engineering_finite_join_terminal_state_requires_exact_conservation_and_owners() {
    for owners in 0..128 {
        let valid =
            owners & !63 == 0 && (0..3).all(|task| matches!((owners >> (task * 2)) & 3, 1 | 2));
        assert_eq!(validate_final_state([1, 0, 7, 7, owners, 0]).is_ok(), valid);
    }
    for word in [0, 1, 2, 3, 5] {
        for replacement in [0, 1, 2, 3, 7, 8, u32::MAX] {
            let mut state = [1, 0, 7, 7, 21, 0];
            if state[word] != replacement {
                state[word] = replacement;
                assert!(validate_final_state(state).is_err());
            }
        }
    }
}

#[test]
fn engineering_finite_join_exact_owned_regions_reject_alias_size_and_offset_mutants() {
    let good = regions();
    validate_regions(&good, &fixups(&good)).unwrap();
    for role in 0..3 {
        for mutation in 0..8 {
            let mut changed = good;
            match mutation {
                0 => changed[role].buffer = 0,
                1 => changed[role].base = 0,
                2 => changed[role].base += 4,
                3 => changed[role].requested -= 1,
                4 => changed[role].requested += 1,
                5 => changed[role].backing = 0,
                6 => changed[role].backing += 1,
                7 => changed[role].base = u64::MAX - 4095,
                _ => unreachable!(),
            }
            assert!(
                validate_regions(&changed, &fixups(&changed)).is_err(),
                "{role}/{mutation}"
            );
        }
        for other in 0..3 {
            if other == role {
                continue;
            }
            let mut changed = good;
            changed[role].buffer = changed[other].buffer;
            assert!(validate_regions(&changed, &fixups(&changed)).is_err());
            changed = good;
            changed[role].base = changed[other].base;
            assert!(validate_regions(&changed, &fixups(&changed)).is_err());
        }
        for mutation in 0..5 {
            let mut pointers = fixups(&good);
            match mutation {
                0 => pointers[role].buffer += 10,
                1 => pointers[role].buffer_offset = 4,
                2 => pointers[role].extent_bytes -= 1,
                3 => pointers[role].kernarg_offset += 8,
                4 => pointers[role].access = BufferAccessV1::Write,
                _ => unreachable!(),
            }
            assert!(validate_regions(&good, &pointers).is_err());
        }
    }
    let mut overlap = good;
    overlap[0].backing = 8192;
    assert!(validate_regions(&overlap, &fixups(&overlap)).is_err());
}

#[test]
fn engineering_finite_join_metadata_rejects_physical_abi_and_role_mutants() {
    let good = metadata([1; 32], "worker");
    validate_metadata(&good, [1; 32], "worker").unwrap();
    // Missing qualifiers are not claimed as native alignment/coherence proof;
    // concrete owned addresses and fixed roles are still checked independently.
    let mut unqualified = good.clone();
    for argument in &mut unqualified.explicit_arguments {
        argument.pointee_alignment = None;
        argument.access = None;
    }
    validate_metadata(&unqualified, [1; 32], "worker").unwrap();
    for mutation in 0..11 {
        let mut changed = good.clone();
        match mutation {
            0 => changed.object_sha256[0] ^= 1,
            1 => changed.symbol.push('x'),
            2 => changed.kernarg_bytes = 24,
            3 => changed.kernarg_alignment = 4,
            4 => changed.wavefront_size = 32,
            5 => changed.private_segment_bytes = 4,
            6 => changed.implicit_argument_offset = None,
            7 => changed.implicit_argument_offset = Some(32),
            8 => changed.implicit_argument_bytes = 0,
            9 => {
                changed.explicit_arguments.pop();
            }
            10 => changed
                .explicit_arguments
                .push(changed.explicit_arguments[0].clone()),
            _ => unreachable!(),
        }
        assert!(validate_metadata(&changed, [1; 32], "worker").is_err());
    }
    for role in 0..3 {
        for mutation in 0..5 {
            let mut changed = good.clone();
            let argument = &mut changed.explicit_arguments[role];
            match mutation {
                0 => argument.offset += 8,
                1 => argument.bytes = 4,
                2 => argument.global_buffer = false,
                3 => argument.pointee_alignment = Some(1),
                4 => argument.access = Some(BufferAccessV1::Write),
                _ => unreachable!(),
            }
            assert!(validate_metadata(&changed, [1; 32], "worker").is_err());
        }
    }
}

#[test]
fn engineering_finite_join_request_and_geometry_are_exact() {
    let object = b"not executable";
    let digest = Sha256::digest(object).into();
    validate_request(object, digest, "worker", 1).unwrap();
    validate_request(object, digest, "worker", 600_000).unwrap();
    for timeout in [0, 600_001, u32::MAX] {
        assert!(validate_request(object, digest, "worker", timeout).is_err());
    }
    for symbol in ["", "worker\0x"] {
        assert!(validate_request(object, digest, symbol, 1).is_err());
    }
    assert!(validate_request(object, [0; 32], "worker", 1).is_err());
    assert!(validate_request(&[], Sha256::digest([]).into(), "worker", 1).is_err());
    validate_geometry(WORKGROUP, GRID).unwrap();
    for axis in 0..3 {
        let mut workgroup = WORKGROUP;
        workgroup[axis] += 1;
        assert!(validate_geometry(workgroup, GRID).is_err());
        let mut grid = GRID;
        grid[axis] += 1;
        assert!(validate_geometry(WORKGROUP, grid).is_err());
    }
}
