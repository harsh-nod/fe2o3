use super::*;
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn mlp_v1_f32_transport_preserves_bits_and_checks_exact_extents() {
    let bits = [
        0u32,
        0x8000_0000,
        0x3f80_0001,
        0x3f81_2345,
        0x0000_0001,
        0x7f80_0000,
        0x7f80_0001,
        0x7fc0_1234,
        0xffc0_5678,
    ];
    let values: Vec<_> = bits.into_iter().map(f32::from_bits).collect();
    let bytes = encode_f32(&values);
    let decoded = decode_f32(&bytes, values.len()).unwrap();
    assert_eq!(
        decoded.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
        bits
    );
    assert!(decode_f32(&bytes[..bytes.len() - 1], values.len()).is_err());
    assert!(decode_f32(&bytes, values.len() - 1).is_err());
    assert!(decode_f32(&bytes, usize::MAX).is_err());
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Event {
    Load,
    Allocate(usize),
    Upload(usize),
    Initialize,
    Prepare,
    Submit,
    Complete,
    Read(usize),
    ReadState,
    Close,
}
fn events() -> Vec<Event> {
    let mut result = vec![Event::Load];
    result.extend((0..11).map(Event::Allocate));
    result.extend((0..10).map(Event::Upload));
    result.extend([
        Event::Initialize,
        Event::Prepare,
        Event::Submit,
        Event::Complete,
    ]);
    result.extend((0..10).map(Event::Read));
    result.extend([Event::ReadState, Event::Close]);
    result
}
fn completed() -> [u32; 11] {
    let mut state = [64; 11];
    state[..6].copy_from_slice(&[1, 0, 31, 31, 0x155, 0]);
    state
}
fn metadata(digest: [u8; 32], symbol: &str) -> KernelMetadataV1 {
    KernelMetadataV1 {
        symbol: symbol.into(),
        object_sha256: digest,
        kernarg_bytes: 344,
        kernarg_alignment: 8,
        group_segment_bytes: 512,
        private_segment_bytes: 0,
        wavefront_size: 64,
        implicit_argument_offset: Some(88),
        implicit_argument_bytes: 256,
        explicit_arguments: (0..11)
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
fn regions() -> [OwnedRegion; 11] {
    core::array::from_fn(|index| OwnedRegion {
        buffer: index as u64 + 1,
        base: (index as u64 + 1) * (64 << 20),
        requested: EXTENTS[index],
        backing: EXTENTS[index].div_ceil(PAGE_BYTES) * PAGE_BYTES,
    })
}
#[derive(Default)]
struct Trace {
    events: Vec<Event>,
    completed: bool,
    closed: bool,
    quarantined: bool,
}
struct Fixture {
    trace: Rc<RefCell<Trace>>,
    failure: Option<usize>,
    initial: [u32; 11],
    final_state: [u32; 11],
    uploaded: [Vec<u8>; 10],
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
            final_state: completed(),
            uploaded: core::array::from_fn(|_| Vec::new()),
            corrupt_input: None,
            short_output: None,
            malformed_metadata: false,
        }
    }
    fn step(&mut self, event: Event) -> Result<()> {
        let mut trace = self.trace.borrow_mut();
        let index = trace.events.len();
        assert_eq!(event, events()[index]);
        trace.events.push(event);
        if self.failure == Some(index) {
            return Err(format!("injected phase {index}"));
        }
        Ok(())
    }
}
impl Transaction for Fixture {
    type Prepared = ();
    type Pending = ();
    fn load(&mut self, _: Vec<u8>, digest: [u8; 32], symbol: String) -> Result<KernelMetadataV1> {
        self.step(Event::Load)?;
        let mut row = metadata(digest, &symbol);
        if self.malformed_metadata {
            row.kernarg_bytes = 304;
        }
        Ok(row)
    }
    fn allocate(&mut self, role: usize) -> Result<OwnedRegion> {
        self.step(Event::Allocate(role))?;
        Ok(regions()[role])
    }
    fn upload(&mut self, buffer: u64, bytes: &[u8]) -> Result<()> {
        let role = buffer as usize - 1;
        self.step(Event::Upload(role))?;
        assert_eq!(bytes.len(), EXTENTS[role]);
        self.uploaded[role] = bytes.to_vec();
        Ok(())
    }
    fn initialize_state(&mut self, buffer: u64) -> Result<[u32; 11]> {
        self.step(Event::Initialize)?;
        assert_eq!(buffer, 11);
        Ok(self.initial)
    }
    fn prepare(
        &mut self,
        regions: &[OwnedRegion; 11],
        pointers: &[PointerFixupV1; 11],
    ) -> Result<()> {
        self.step(Event::Prepare)?;
        validate_regions(regions, pointers)
    }
    unsafe fn submit(&mut self, _: (), timeout: u32) -> Result<()> {
        self.step(Event::Submit)?;
        assert_eq!(timeout, 100);
        Ok(())
    }
    fn complete(&mut self, _: ()) -> Result<u64> {
        self.step(Event::Complete)?;
        self.trace.borrow_mut().completed = true;
        Ok(29)
    }
    fn read(&mut self, buffer: u64, bytes: u32) -> Result<Vec<u8>> {
        assert!(self.trace.borrow().completed);
        let role = buffer as usize - 1;
        self.step(Event::Read(role))?;
        assert_eq!(bytes as usize, EXTENTS[role]);
        let mut result = self.uploaded[role].clone();
        if role < 5 {
            if self.corrupt_input == Some(role) {
                result[0] ^= 1;
            }
        } else {
            result.fill(0x3f);
        }
        if self.short_output == Some(role) {
            result.pop();
        }
        Ok(result)
    }
    fn read_state(&mut self, buffer: u64) -> Result<[u32; 11]> {
        assert!(self.trace.borrow().completed);
        self.step(Event::ReadState)?;
        assert_eq!(buffer, 11);
        Ok(self.final_state)
    }
    fn close(&mut self) -> Result<()> {
        assert!(self.trace.borrow().completed);
        self.step(Event::Close)?;
        self.trace.borrow_mut().closed = true;
        Ok(())
    }
    fn quarantine(self) {
        self.trace.borrow_mut().quarantined = true;
    }
}
fn inputs() -> &'static [Vec<u8>; 5] {
    static INPUTS: std::sync::OnceLock<[Vec<u8>; 5]> = std::sync::OnceLock::new();
    INPUTS.get_or_init(|| core::array::from_fn(|role| vec![0x30 + role as u8; EXTENTS[role]]))
}
#[derive(Clone, Debug, PartialEq)]
struct Outputs {
    bf16: [Vec<u16>; 4],
    partial: Vec<f32>,
}
impl std::ops::Deref for Outputs {
    type Target = [Vec<u16>; 4];
    fn deref(&self) -> &Self::Target {
        &self.bf16
    }
}
impl std::ops::DerefMut for Outputs {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.bf16
    }
}
fn outputs() -> Outputs {
    Outputs {
        bf16: core::array::from_fn(|role| vec![0x7fc1 + role as u16; EXTENTS[role + 5] / 2]),
        partial: vec![f32::from_bits(0x3f81_2345); 4096],
    }
}
fn execute(
    fixture: Fixture,
    outputs: &mut Outputs,
) -> Result<Gfx950EngineeringWaveMlpTasksResultV1> {
    let inputs = inputs();
    let object = vec![1, 2, 3];
    let digest = Sha256::digest(&object).into();
    // SAFETY: the private adapter has no GPU operation or executable image.
    unsafe {
        coordinate(
            fixture,
            object,
            digest,
            SYMBOL.into(),
            inputs.each_ref().map(Vec::as_slice),
            outputs.bf16.each_mut().map(Vec::as_mut_slice),
            &mut outputs.partial,
            100,
        )
    }
}
fn unchanged(outputs: &Outputs) {
    for (role, words) in outputs.iter().enumerate() {
        assert!(words.iter().all(|&word| word == 0x7fc1 + role as u16));
    }
    assert!(outputs.partial.iter().all(|x| x.to_bits() == 0x3f81_2345));
}
#[test]
fn mlp_v1_commits_all_five_outputs_only_after_completion_and_close() {
    let fixture = Fixture::new();
    let trace = Rc::clone(&fixture.trace);
    let mut output = outputs();
    let result = execute(fixture, &mut output).unwrap();
    assert_eq!(result.final_state, completed());
    assert_eq!(result.dispatch_elapsed_ns, 29);
    for words in &output[..] {
        assert!(words.iter().all(|&x| x == 0x3f3f));
    }
    assert!(output.partial.iter().all(|x| x.to_bits() == 0x3f3f_3f3f));
    let trace = trace.borrow();
    assert_eq!(trace.events, events());
    assert!(trace.completed && trace.closed && !trace.quarantined);
}
#[test]
fn mlp_v1_every_phase_failure_preserves_every_output_and_quarantines() {
    for phase in 0..events().len() {
        let mut fixture = Fixture::new();
        fixture.failure = Some(phase);
        let trace = Rc::clone(&fixture.trace);
        let mut output = outputs();
        assert!(execute(fixture, &mut output).is_err(), "phase {phase}");
        unchanged(&output);
        let trace = trace.borrow();
        assert_eq!(trace.events, events()[..=phase]);
        assert!(trace.quarantined && !trace.closed);
    }
}
#[test]
fn mlp_v1_all_readonly_inputs_and_output_extents_are_checked() {
    for role in 0..10 {
        let mut fixture = Fixture::new();
        if role < 5 {
            fixture.corrupt_input = Some(role);
        } else {
            fixture.short_output = Some(role);
        }
        let trace = Rc::clone(&fixture.trace);
        let mut output = outputs();
        assert!(execute(fixture, &mut output).is_err(), "role {role}");
        unchanged(&output);
        assert!(trace.borrow().quarantined && !trace.borrow().closed);
    }
}
#[test]
fn mlp_v1_invalid_initial_final_state_and_metadata_never_publish() {
    for kind in 0..3 {
        let mut fixture = Fixture::new();
        match kind {
            0 => fixture.initial[10] = 1,
            1 => fixture.final_state[10] = 63,
            _ => fixture.malformed_metadata = true,
        }
        let trace = Rc::clone(&fixture.trace);
        let mut output = outputs();
        assert!(execute(fixture, &mut output).is_err());
        unchanged(&output);
        assert!(trace.borrow().quarantined && !trace.borrow().closed);
        if kind == 0 {
            assert_eq!(trace.borrow().events.last(), Some(&Event::Initialize));
        }
        if kind == 2 {
            assert_eq!(trace.borrow().events, [Event::Load]);
        }
    }
}
#[test]
fn mlp_v1_state_requires_all_five_tasks_and_accepts_either_worker_owner() {
    for mask in 0_u32..32 {
        let mut state = completed();
        state[4] = (0..5).fold(0, |word, task| {
            word | ((1 + ((mask >> task) & 1)) << (2 * task))
        });
        validate_final_state(state).unwrap();
    }
    for word in 0..11 {
        let mut state = completed();
        state[word] ^= 1;
        assert!(validate_final_state(state).is_err(), "word {word}");
    }
    for task in 0..5 {
        for owner in [0, 3] {
            let mut state = completed();
            state[4] = (state[4] & !(3 << (2 * task))) | (owner << (2 * task));
            assert!(validate_final_state(state).is_err());
        }
        for arrivals in [0, 63, 65] {
            let mut state = completed();
            state[6 + task] = arrivals;
            assert!(validate_final_state(state).is_err());
        }
    }
}
#[test]
fn mlp_v1_metadata_requires_eleven_exact_roots_and_real_source_lds() {
    validate_metadata(&metadata([1; 32], SYMBOL), [1; 32], SYMBOL).unwrap();
    for role in 0..11 {
        for mutation in 0..5 {
            let mut value = metadata([1; 32], SYMBOL);
            let argument = &mut value.explicit_arguments[role];
            match mutation {
                0 => argument.offset += 8,
                1 => argument.bytes = 4,
                2 => argument.global_buffer = false,
                3 => argument.pointee_alignment = Some(if matches!(role, 9 | 10) { 2 } else { 4 }),
                _ => {
                    argument.access = Some(if role < 5 {
                        BufferAccessV1::ReadWrite
                    } else {
                        BufferAccessV1::Read
                    })
                }
            }
            assert!(validate_metadata(&value, [1; 32], SYMBOL).is_err());
        }
    }
    for mutation in 0..15 {
        let mut value = metadata([1; 32], SYMBOL);
        match mutation {
            0 => value.kernarg_bytes = 304,
            1 => value.implicit_argument_offset = Some(48),
            2 => value.implicit_argument_bytes = 0,
            3 => value.explicit_arguments.truncate(6),
            4 => value.kernarg_alignment = 4,
            5 => value.wavefront_size = 32,
            6 => value.private_segment_bytes = 4,
            7 => value.group_segment_bytes = 0,
            8 => value.object_sha256[0] ^= 1,
            9 => value.symbol.push('x'),
            10 => value.implicit_argument_offset = None,
            11 => value.group_segment_bytes = 256,
            12 => value.group_segment_bytes = 513,
            13 => value.kernarg_bytes = 376,
            _ => value
                .explicit_arguments
                .push(value.explicit_arguments[10].clone()),
        }
        assert!(validate_metadata(&value, [1; 32], SYMBOL).is_err());
    }
    let mut optional = metadata([1; 32], SYMBOL);
    for role in &mut optional.explicit_arguments {
        role.access = None;
        role.pointee_alignment = None;
    }
    validate_metadata(&optional, [1; 32], SYMBOL).unwrap();
}
#[test]
fn mlp_v1_regions_and_fixups_reject_aliases_extent_role_and_overflow() {
    let good = regions();
    validate_regions(&good, &fixups(&good)).unwrap();
    for role in 0..11 {
        for mutation in 0..7 {
            let mut value = good;
            match mutation {
                0 => value[role].buffer = 0,
                1 => value[role].base = 0,
                2 => value[role].base += 2,
                3 => value[role].requested -= 1,
                4 => value[role].backing = 0,
                5 => value[role].backing += 1,
                _ => value[role].base = u64::MAX & !(PAGE_BYTES as u64 - 1),
            }
            assert!(validate_regions(&value, &fixups(&value)).is_err());
        }
        for mutation in 0..5 {
            let mut pointers = fixups(&good);
            match mutation {
                0 => pointers[role].buffer += 1,
                1 => pointers[role].kernarg_offset += 8,
                2 => pointers[role].buffer_offset = 2,
                3 => pointers[role].extent_bytes -= 1,
                _ => pointers[role].access = BufferAccessV1::Write,
            }
            assert!(validate_regions(&good, &pointers).is_err());
        }
        if role > 0 {
            let mut value = good;
            value[role].buffer = value[0].buffer;
            assert!(validate_regions(&value, &fixups(&value)).is_err());
            let mut value = good;
            value[role].base = value[0].base;
            assert!(validate_regions(&value, &fixups(&value)).is_err());
        }
    }
}
#[test]
fn mlp_v1_rejects_error_zero_ready_but_unclaimed_output_after_bounded_retirement() {
    let mut state = completed();
    state[1] = 1 << 4;
    state[2] = (1 << 4) - 1;
    state[3] = (1 << 4) - 1;
    state[4] &= (1 << 8) - 1;
    state[10] = 0;
    assert_eq!(state[5], 0);
    assert!(validate_final_state(state).is_err());
    let mut fixture = Fixture::new();
    fixture.final_state = state;
    let trace = Rc::clone(&fixture.trace);
    let mut output = outputs();
    assert!(execute(fixture, &mut output).is_err());
    unchanged(&output);
    assert!(trace.borrow().completed && trace.borrow().quarantined && !trace.borrow().closed);
}

#[test]
fn mlp_v1_wrong_input_or_output_extent_never_loads_or_publishes() {
    for role in 0..10 {
        let fixture = Fixture::new();
        let trace = Rc::clone(&fixture.trace);
        let mut inputs = inputs().each_ref().map(Vec::as_slice);
        let mut outputs = outputs();
        if role < 5 {
            inputs[role] = &inputs[role][..inputs[role].len() - 1];
        } else if role < 9 {
            outputs[role - 5].pop();
        } else {
            outputs.partial.pop();
        }
        let expected = outputs.clone();
        let object = vec![1, 2, 3];
        let digest = Sha256::digest(&object).into();
        // SAFETY: private non-GPU adapter; validation rejects before load.
        let result = unsafe {
            coordinate(
                fixture,
                object,
                digest,
                SYMBOL.into(),
                inputs,
                outputs.bf16.each_mut().map(Vec::as_mut_slice),
                &mut outputs.partial,
                100,
            )
        };
        assert!(result.is_err());
        assert_eq!(outputs, expected);
        assert!(trace.borrow().events.is_empty() && trace.borrow().quarantined);
    }
}

#[test]
fn mlp_v1_request_digest_symbol_and_deadline_fail_closed() {
    let object = vec![1, 2, 3];
    let digest = Sha256::digest(&object).into();
    validate_request(&object, digest, SYMBOL, 100).unwrap();
    assert!(validate_request(&[], digest, SYMBOL, 100).is_err());
    assert!(validate_request(&object, [0; 32], SYMBOL, 100).is_err());
    for symbol in [
        "",
        "bad\0symbol",
        "ferric_qwen3_claimed_mlp_bf16_f32_v1_extra",
    ] {
        assert!(validate_request(&object, digest, symbol, 100).is_err());
    }
    assert!(validate_request(&object, digest, &"x".repeat(257), 100).is_err());
    for timeout in [0, 600_001] {
        assert!(validate_request(&object, digest, SYMBOL, timeout).is_err());
    }
}

#[test]
fn mlp_v1_owner_high_bits_and_all_geometry_dimensions_fail_closed() {
    for bit in 10..32 {
        let mut state = completed();
        state[4] |= 1 << bit;
        assert!(validate_final_state(state).is_err());
    }
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

#[test]
fn mlp_v1_weight_dimensions_fail_before_device_open() {
    validate_weight_extents([WEIGHT_WORDS; 3]).unwrap();
    for role in 0..3 {
        for length in [0, WEIGHT_WORDS - 1, WEIGHT_WORDS + 1, usize::MAX] {
            let mut lengths = [WEIGHT_WORDS; 3];
            lengths[role] = length;
            assert!(validate_weight_extents(lengths).is_err());
        }
    }
}
