use super::*;
use std::cell::RefCell;
use std::rc::Rc;

const EVENTS: [&str; 23] = [
    "load",
    "allocate-input",
    "allocate-norm-weight",
    "allocate-qkv-weight",
    "allocate-normalized",
    "allocate-qkv-output",
    "allocate-state",
    "upload-input",
    "upload-norm-weight",
    "upload-qkv-weight",
    "upload-normalized",
    "upload-qkv-output",
    "initialize-state",
    "prepare",
    "submit",
    "complete",
    "read-input",
    "read-norm-weight",
    "read-qkv-weight",
    "read-normalized",
    "read-qkv-output",
    "read-state",
    "close",
];
const COMPLETE: [u32; 19] = [
    1,
    0,
    8191,
    8191,
    0x0155_5555,
    0,
    64,
    64,
    64,
    64,
    64,
    64,
    64,
    64,
    64,
    64,
    64,
    64,
    64,
];

fn metadata(digest: [u8; 32], symbol: &str) -> KernelMetadataV1 {
    KernelMetadataV1 {
        symbol: symbol.into(),
        object_sha256: digest,
        kernarg_bytes: KERNARG_BYTES as u32,
        kernarg_alignment: 8,
        group_segment_bytes: 512,
        private_segment_bytes: 0,
        wavefront_size: 64,
        implicit_argument_offset: Some(48),
        implicit_argument_bytes: 256,
        explicit_arguments: (0..6)
            .map(|index| ExplicitArgumentV1 {
                offset: index as u32 * 8,
                bytes: 8,
                global_buffer: true,
                pointee_alignment: Some(role_alignment(index)),
                access: Some(role_access(index)),
            })
            .collect(),
    }
}

fn regions() -> [OwnedRegion; 6] {
    core::array::from_fn(|index| OwnedRegion {
        buffer: index as u64 + 1,
        base: (index as u64 + 1) * (32 << 20),
        requested: EXTENTS[index],
        backing: EXTENTS[index].div_ceil(PAGE_BYTES) * PAGE_BYTES,
    })
}

#[derive(Default)]
struct Trace {
    events: Vec<&'static str>,
    quarantined: bool,
    completed: bool,
    closed: bool,
}

struct Fixture {
    trace: Rc<RefCell<Trace>>,
    failure: Option<usize>,
    initial: [u32; 19],
    final_state: [u32; 19],
    uploaded: [Vec<u8>; 5],
    corrupt_input: Option<usize>,
    short_output: Option<usize>,
    malformed_metadata: bool,
}

impl Fixture {
    fn new() -> Self {
        Self {
            trace: Rc::default(),
            failure: None,
            initial: INITIAL_STATE,
            final_state: COMPLETE,
            uploaded: core::array::from_fn(|_| Vec::new()),
            corrupt_input: None,
            short_output: None,
            malformed_metadata: false,
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
        let mut row = metadata(digest, &symbol);
        if self.malformed_metadata {
            row.explicit_arguments[0].pointee_alignment = Some(4);
        }
        Ok(row)
    }
    fn allocate(&mut self, role: usize) -> Result<OwnedRegion> {
        self.step(EVENTS[role + 1])?;
        Ok(regions()[role])
    }
    fn upload(&mut self, buffer: u64, bytes: &[u8]) -> Result<()> {
        let role = buffer as usize - 1;
        self.step(EVENTS[7 + role])?;
        assert_eq!(bytes.len(), EXTENTS[role]);
        self.uploaded[role] = bytes.to_vec();
        Ok(())
    }
    fn initialize_state(&mut self, buffer: u64) -> Result<[u32; 19]> {
        self.step("initialize-state")?;
        assert_eq!(buffer, 6);
        Ok(self.initial)
    }
    fn prepare(
        &mut self,
        regions: &[OwnedRegion; 6],
        pointers: &[PointerFixupV1; 6],
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
        Ok(29)
    }
    fn read(&mut self, buffer: u64, bytes: u32) -> Result<Vec<u8>> {
        assert!(self.trace.borrow().completed);
        let role = buffer as usize - 1;
        self.step(EVENTS[16 + role])?;
        assert_eq!(bytes as usize, EXTENTS[role]);
        if role < 3 {
            let mut result = self.uploaded[role].clone();
            if self.corrupt_input == Some(role) {
                result[0] ^= 1;
            }
            Ok(result)
        } else {
            Ok(vec![
                if role == 3 { 0x3f } else { 0x42 };
                EXTENTS[role]
                    - usize::from(self.short_output == Some(role))
            ])
        }
    }
    fn read_state(&mut self, buffer: u64) -> Result<[u32; 19]> {
        assert!(self.trace.borrow().completed);
        self.step("read-state")?;
        assert_eq!(buffer, 6);
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

fn execute(
    fixture: Fixture,
    normalized: &mut [u16; 4096],
    qkv: &mut [u16; 3072],
) -> Result<Gfx950EngineeringWaveQkvTasksResultV2> {
    let inputs: [Vec<u16>; 3] = core::array::from_fn(|role| vec![0x3f80; EXTENTS[role] / 2]);
    let object = vec![1, 2, 3];
    let digest = Sha256::digest(&object).into();
    // SAFETY: this instantiation has no GPU operations or executable object.
    unsafe {
        coordinate(
            fixture,
            object,
            digest,
            "fixture".into(),
            inputs.each_ref().map(Vec::as_slice),
            normalized,
            qkv,
            100,
        )
    }
}

#[test]
fn engineering_wave_qkv_tasks_v2_coordinator_commits_both_outputs_only_after_close() {
    let fixture = Fixture::new();
    let trace = Rc::clone(&fixture.trace);
    let mut norm = [0xdead; 4096];
    let mut qkv = [0xbeef; 3072];
    let result = execute(fixture, &mut norm, &mut qkv).unwrap();
    assert_eq!(result.final_state, COMPLETE);
    assert_eq!(result.dispatch_elapsed_ns, 29);
    assert_eq!(norm, [0x3f3f; 4096]);
    assert_eq!(qkv, [0x4242; 3072]);
    let trace = trace.borrow();
    assert_eq!(trace.events, EVENTS);
    assert!(trace.completed && trace.closed && !trace.quarantined);
}

#[test]
fn engineering_wave_qkv_tasks_v2_every_phase_failure_quarantines_and_preserves_outputs() {
    for failure in 0..EVENTS.len() {
        let mut fixture = Fixture::new();
        fixture.failure = Some(failure);
        let trace = Rc::clone(&fixture.trace);
        let mut norm = [0xdead; 4096];
        let mut qkv = [0xbeef; 3072];
        assert!(execute(fixture, &mut norm, &mut qkv).is_err(), "{failure}");
        assert_eq!(norm, [0xdead; 4096]);
        assert_eq!(qkv, [0xbeef; 3072]);
        let trace = trace.borrow();
        assert_eq!(trace.events, EVENTS[..=failure]);
        assert!(trace.quarantined && !trace.closed);
    }
}

#[test]
fn engineering_wave_qkv_tasks_v2_wrong_initial_atomic_word_never_submits() {
    for word in 0..19 {
        let mut fixture = Fixture::new();
        fixture.initial[word] ^= 1;
        let trace = Rc::clone(&fixture.trace);
        let mut norm = [0xdead; 4096];
        let mut qkv = [0xbeef; 3072];
        assert!(execute(fixture, &mut norm, &mut qkv).is_err());
        assert_eq!(trace.borrow().events.last(), Some(&"initialize-state"));
        assert!(trace.borrow().quarantined);
        assert_eq!(norm, [0xdead; 4096]);
        assert_eq!(qkv, [0xbeef; 3072]);
    }
}

#[test]
fn engineering_wave_qkv_tasks_v2_final_state_and_arrivals_reject_before_close() {
    for word in 0..19 {
        let mut fixture = Fixture::new();
        fixture.final_state[word] = if word == 4 { 0 } else { COMPLETE[word] ^ 1 };
        let trace = Rc::clone(&fixture.trace);
        let mut norm = [0xdead; 4096];
        let mut qkv = [0xbeef; 3072];
        assert!(execute(fixture, &mut norm, &mut qkv).is_err());
        assert!(trace.borrow().completed && trace.borrow().quarantined && !trace.borrow().closed);
        assert_eq!(norm, [0xdead; 4096]);
        assert_eq!(qkv, [0xbeef; 3072]);
    }
    for task in 0..13 {
        for owners in [0, 3] {
            let mut state = COMPLETE;
            state[4] = (state[4] & !(3 << (2 * task))) | (owners << (2 * task));
            assert!(validate_final_state(state).is_err());
        }
        let mut state = COMPLETE;
        state[6 + task] = 63;
        assert!(validate_final_state(state).is_err());
        state[6 + task] = 65;
        assert!(validate_final_state(state).is_err());
    }
}

#[test]
fn engineering_wave_qkv_tasks_v2_each_read_only_input_and_output_extent_is_checked() {
    for role in 0..5 {
        let mut fixture = Fixture::new();
        if role < 3 {
            fixture.corrupt_input = Some(role);
        } else {
            fixture.short_output = Some(role);
        }
        let trace = Rc::clone(&fixture.trace);
        let mut norm = [0xdead; 4096];
        let mut qkv = [0xbeef; 3072];
        assert!(execute(fixture, &mut norm, &mut qkv).is_err());
        assert!(trace.borrow().quarantined && !trace.borrow().closed);
        assert_eq!(norm, [0xdead; 4096]);
        assert_eq!(qkv, [0xbeef; 3072]);
    }
}

#[test]
fn engineering_wave_qkv_tasks_v2_metadata_rejects_before_allocation() {
    let mut fixture = Fixture::new();
    fixture.malformed_metadata = true;
    let trace = Rc::clone(&fixture.trace);
    assert!(execute(fixture, &mut [0; 4096], &mut [0; 3072]).is_err());
    assert_eq!(trace.borrow().events, ["load"]);
    assert!(trace.borrow().quarantined);
}

#[test]
fn engineering_wave_qkv_tasks_v2_metadata_roles_offsets_and_resources_are_exact() {
    let expected = metadata([1; 32], "wave");
    validate_metadata(&expected, [1; 32], "wave").unwrap();
    let mut old_three_root = metadata([1; 32], "wave");
    old_three_root.kernarg_bytes = 280;
    old_three_root.implicit_argument_offset = Some(24);
    old_three_root.explicit_arguments.truncate(3);
    for argument in &mut old_three_root.explicit_arguments {
        argument.pointee_alignment = Some(4);
    }
    assert!(validate_metadata(&old_three_root, [1; 32], "wave").is_err());
    for mutation in 0..11 {
        let mut value = metadata([1; 32], "wave");
        match mutation {
            0 => value.object_sha256[0] ^= 1,
            1 => value.symbol.push('x'),
            2 => value.kernarg_bytes = 280,
            3 => value.kernarg_alignment = 4,
            4 => value.wavefront_size = 32,
            5 => value.private_segment_bytes = 4,
            6 => value.group_segment_bytes = 160 * 1024 + 1,
            7 => value.implicit_argument_offset = Some(24),
            8 => value.implicit_argument_bytes = 0,
            9 => {
                value.explicit_arguments.pop();
            }
            10 => value.implicit_argument_offset = None,
            _ => unreachable!(),
        }
        assert!(validate_metadata(&value, [1; 32], "wave").is_err());
    }
    for role in 0..6 {
        for mutation in 0..5 {
            let mut value = metadata([1; 32], "wave");
            let argument = &mut value.explicit_arguments[role];
            match mutation {
                0 => argument.offset += 8,
                1 => argument.bytes = 4,
                2 => argument.global_buffer = false,
                3 => argument.pointee_alignment = Some(if role == 5 { 2 } else { 4 }),
                4 => {
                    argument.access = Some(if role < 3 {
                        BufferAccessV1::ReadWrite
                    } else {
                        BufferAccessV1::Read
                    })
                }
                _ => unreachable!(),
            }
            assert!(validate_metadata(&value, [1; 32], "wave").is_err());
        }
    }
    let mut optional = metadata([1; 32], "wave");
    for argument in &mut optional.explicit_arguments {
        argument.access = None;
        argument.pointee_alignment = None;
    }
    validate_metadata(&optional, [1; 32], "wave").unwrap();
    // This is only a synthetic metadata validator test, not emitted LDS evidence.
    for bytes in [0, 256, 160 * 1024] {
        optional.group_segment_bytes = bytes;
        validate_metadata(&optional, [1; 32], "wave").unwrap();
    }
}

#[test]
fn engineering_wave_qkv_tasks_v2_regions_reject_alias_extent_alignment_and_overflow() {
    for role in 0..6 {
        for mutation in 0..7 {
            let mut value = regions();
            match mutation {
                0 => value[role].buffer = 0,
                1 => value[role].base = 0,
                2 => value[role].base += 2,
                3 => value[role].requested -= 1,
                4 => value[role].backing = 0,
                5 => value[role].backing += 1,
                6 => value[role].base = u64::MAX & !(PAGE_BYTES as u64 - 1),
                _ => unreachable!(),
            }
            assert!(validate_regions(&value, &fixups(&value)).is_err());
        }
    }
    for role in 1..6 {
        let mut value = regions();
        value[role].buffer = value[0].buffer;
        assert!(validate_regions(&value, &fixups(&value)).is_err());
        let mut value = regions();
        value[role].base = value[0].base;
        assert!(validate_regions(&value, &fixups(&value)).is_err());
    }
}

#[test]
fn engineering_wave_qkv_tasks_v2_fixups_bind_each_owned_root_exactly() {
    let regions = regions();
    validate_regions(&regions, &fixups(&regions)).unwrap();
    for role in 0..6 {
        for mutation in 0..5 {
            let mut pointers = fixups(&regions);
            match mutation {
                0 => pointers[role].buffer += 1,
                1 => pointers[role].kernarg_offset += 8,
                2 => pointers[role].buffer_offset = 2,
                3 => pointers[role].extent_bytes -= 1,
                4 => pointers[role].access = BufferAccessV1::Write,
                _ => unreachable!(),
            }
            assert!(validate_regions(&regions, &pointers).is_err());
        }
    }
}

#[test]
fn engineering_wave_qkv_tasks_v2_request_geometry_and_encoding_are_closed() {
    let object = vec![1, 2, 3];
    let digest = Sha256::digest(&object).into();
    validate_request(&object, digest, "wave", 100).unwrap();
    assert!(validate_request(&[], digest, "wave", 100).is_err());
    assert!(validate_request(&object, [0; 32], "wave", 100).is_err());
    for symbol in ["", "bad\0symbol"] {
        assert!(validate_request(&object, digest, symbol, 100).is_err());
    }
    assert!(validate_request(&object, digest, &"a".repeat(257), 100).is_err());
    for timeout in [0, 600_001] {
        assert!(validate_request(&object, digest, "wave", timeout).is_err());
    }
    validate_geometry(WORKGROUP, GRID).unwrap();
    for (workgroup, grid) in [
        ([128, 1, 1], [256, 1, 1]),
        ([64, 1, 1], [64, 1, 1]),
        ([32, 2, 1], [64, 2, 1]),
    ] {
        assert!(validate_geometry(workgroup, grid).is_err());
    }
    assert_eq!(encode_words(&[0x1234, 0xabcd]), [0x34, 0x12, 0xcd, 0xab]);
    assert_eq!(
        decode_words::<2>(&[0x34, 0x12, 0xcd, 0xab]).unwrap(),
        [0x1234, 0xabcd]
    );
    assert!(decode_words::<2>(&[0x34, 0x12]).is_err());
}

#[test]
fn engineering_wave_qkv_tasks_v2_wrong_input_lengths_never_load() {
    let fixture = Fixture::new();
    let trace = Rc::clone(&fixture.trace);
    let object = vec![1, 2, 3];
    let digest = Sha256::digest(&object).into();
    // SAFETY: this fixture never loads or dispatches any GPU object.
    let result = unsafe {
        coordinate(
            fixture,
            object,
            digest,
            "wave".into(),
            [&[], &[], &[]],
            &mut [0; 4096],
            &mut [0; 3072],
            100,
        )
    };
    assert!(result.is_err());
    assert!(trace.borrow().events.is_empty() && trace.borrow().quarantined);
}

#[test]
fn qkv_state_accepts_all_schedules_without_assuming_fairness() {
    for mask in 0u32..8192 {
        let mut state = COMPLETE;
        state[4] = (0..13).fold(0, |word, task| {
            word | ((1 + ((mask >> task) & 1)) << (2 * task))
        });
        validate_final_state(state).unwrap();
    }
    for bit in 26..32 {
        let mut state = COMPLETE;
        state[4] |= 1 << bit;
        assert!(validate_final_state(state).is_err());
    }
}

#[test]
fn qkv_roots_cannot_reuse_key_profile_extents() {
    for (role, legacy) in [(2, 4_194_304), (4, 1024), (5, 36)] {
        let mut roots = regions();
        roots[role].requested = legacy;
        assert!(validate_regions(&roots, &fixups(&roots)).is_err());
    }
}
