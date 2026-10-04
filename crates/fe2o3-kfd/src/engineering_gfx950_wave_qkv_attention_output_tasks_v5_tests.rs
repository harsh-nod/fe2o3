use super::*;
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn attention_output_v5_f32_transport_preserves_bits_and_checks_exact_extents() {
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
    result.extend((0..15).map(Event::Allocate));
    result.extend((0..14).map(Event::Upload));
    result.extend([
        Event::Initialize,
        Event::Prepare,
        Event::Submit,
        Event::Complete,
    ]);
    result.extend((0..14).map(Event::Read));
    result.extend([Event::ReadState, Event::Close]);
    result
}
fn completed() -> [u32; 22] {
    let mut state = [64; 22];
    state[..6].copy_from_slice(&[1, 0, 65535, 65535, 0x5555_5555, 0]);
    state
}
fn metadata(digest: [u8; 32], symbol: &str) -> KernelMetadataV1 {
    KernelMetadataV1 {
        symbol: symbol.into(),
        object_sha256: digest,
        kernarg_bytes: 376,
        kernarg_alignment: 8,
        group_segment_bytes: 512,
        private_segment_bytes: 0,
        wavefront_size: 64,
        implicit_argument_offset: Some(120),
        implicit_argument_bytes: 256,
        explicit_arguments: (0..15)
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
fn regions() -> [OwnedRegion; 15] {
    core::array::from_fn(|index| OwnedRegion {
        buffer: index as u64 + 1,
        base: (index as u64 + 1) * (32 << 20),
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
    initial: [u32; 22],
    final_state: [u32; 22],
    uploaded: [Vec<u8>; 14],
    corrupt_input: Option<usize>,
    short_output: Option<usize>,
    corrupt_cache: Option<usize>,
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
            corrupt_cache: None,
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
    fn initialize_state(&mut self, buffer: u64) -> Result<[u32; 22]> {
        self.step(Event::Initialize)?;
        assert_eq!(buffer, 15);
        Ok(self.initial)
    }
    fn prepare(
        &mut self,
        regions: &[OwnedRegion; 15],
        pointers: &[PointerFixupV1; 15],
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
        if role < 7 {
            if self.corrupt_input == Some(role) {
                result[0] ^= 1;
            }
        } else if role < 10 || role == 12 || role == 13 {
            result.fill(0x3f);
        } else {
            let start = cache_slot(&self.uploaded[5])? * TOKEN_WORDS * 2;
            result[start..start + TOKEN_WORDS * 2].fill(0x42);
            if self.corrupt_cache == Some(role) {
                result[0] ^= 1;
            }
        }
        if self.short_output == Some(role) {
            result.pop();
        }
        Ok(result)
    }
    fn read_state(&mut self, buffer: u64) -> Result<[u32; 22]> {
        assert!(self.trace.borrow().completed);
        self.step(Event::ReadState)?;
        assert_eq!(buffer, 15);
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
fn inputs() -> [Vec<u8>; 7] {
    let mut bytes = core::array::from_fn(|role| vec![0x3f; EXTENTS[role]]);
    let words = std::iter::once(16_u32).chain((0..144).map(|page| (page * 5 + 7) % 144));
    bytes[5] = words.flat_map(u32::to_le_bytes).collect();
    bytes
}
#[derive(Clone, Debug, PartialEq)]
struct Outputs {
    bf16: [Vec<u16>; 6],
    partial: Vec<f32>,
}
impl std::ops::Deref for Outputs {
    type Target = [Vec<u16>; 6];
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
        bf16: core::array::from_fn(|role| vec![0x7fc1 + role as u16; EXTENTS[role + 7] / 2]),
        partial: vec![f32::from_bits(0x3f81_2345); 4096],
    }
}
fn execute(
    fixture: Fixture,
    outputs: &mut Outputs,
) -> Result<Gfx950EngineeringWaveQkvAttentionOutputTasksResultV5> {
    let inputs = inputs();
    let object = vec![1, 2, 3];
    let digest = Sha256::digest(&object).into();
    // SAFETY: the private adapter has no GPU operation or executable image.
    unsafe {
        coordinate(
            fixture,
            object,
            digest,
            "fixture".into(),
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
fn attention_output_v5_commits_all_seven_outputs_only_after_completion_and_close() {
    let fixture = Fixture::new();
    let trace = Rc::clone(&fixture.trace);
    let mut output = outputs();
    let result = execute(fixture, &mut output).unwrap();
    assert_eq!(result.final_state, completed());
    assert_eq!(result.dispatch_elapsed_ns, 29);
    for words in &output[..3] {
        assert!(words.iter().all(|&x| x == 0x3f3f));
    }
    assert!(output[5].iter().all(|&x| x == 0x3f3f));
    assert!(output.partial.iter().all(|x| x.to_bits() == 0x3f3f_3f3f));
    let start = cache_slot(&inputs()[5]).unwrap() * TOKEN_WORDS;
    for (index, words) in output[3..5].iter().enumerate() {
        assert!(words[..start].iter().all(|&x| x == 0x7fc4 + index as u16));
        assert!(
            words[start..start + TOKEN_WORDS]
                .iter()
                .all(|&x| x == 0x4242)
        );
        assert!(
            words[start + TOKEN_WORDS..]
                .iter()
                .all(|&x| x == 0x7fc4 + index as u16)
        );
    }
    let trace = trace.borrow();
    assert_eq!(trace.events, events());
    assert!(trace.completed && trace.closed && !trace.quarantined);
}
#[test]
fn attention_output_v5_every_phase_failure_preserves_every_output_and_quarantines() {
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
fn attention_output_v5_all_readonly_inputs_output_extents_and_cache_neighbors_are_checked() {
    for role in 0..16 {
        let mut fixture = Fixture::new();
        if role < 7 {
            fixture.corrupt_input = Some(role);
        } else if role < 14 {
            fixture.short_output = Some(role);
        } else {
            fixture.corrupt_cache = Some(role - 4);
        }
        let trace = Rc::clone(&fixture.trace);
        let mut output = outputs();
        assert!(execute(fixture, &mut output).is_err(), "role {role}");
        unchanged(&output);
        assert!(trace.borrow().quarantined && !trace.borrow().closed);
    }
}
#[test]
fn attention_output_v5_invalid_initial_final_state_and_metadata_never_publish() {
    for kind in 0..3 {
        let mut fixture = Fixture::new();
        match kind {
            0 => fixture.initial[21] = 1,
            1 => fixture.final_state[21] = 63,
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
fn attention_output_v5_state_requires_all_sixteen_tasks_and_accepts_either_worker_owner() {
    for mask in 0_u32..65536 {
        let mut state = completed();
        state[4] = (0..16).fold(0, |word, task| {
            word | ((1 + ((mask >> task) & 1)) << (2 * task))
        });
        validate_final_state(state).unwrap();
    }
    for word in 0..22 {
        let mut state = completed();
        state[word] ^= 1;
        assert!(validate_final_state(state).is_err(), "word {word}");
    }
    for task in 0..16 {
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
fn attention_output_v5_metadata_requires_fifteen_exact_roots_and_real_source_lds() {
    validate_metadata(&metadata([1; 32], "post"), [1; 32], "post").unwrap();
    for role in 0..15 {
        for mutation in 0..5 {
            let mut value = metadata([1; 32], "post");
            let argument = &mut value.explicit_arguments[role];
            match mutation {
                0 => argument.offset += 8,
                1 => argument.bytes = 4,
                2 => argument.global_buffer = false,
                3 => {
                    argument.pointee_alignment = Some(if matches!(role, 4 | 5 | 13 | 14) {
                        2
                    } else {
                        4
                    })
                }
                _ => {
                    argument.access = Some(if role < 7 {
                        BufferAccessV1::ReadWrite
                    } else {
                        BufferAccessV1::Read
                    })
                }
            }
            assert!(validate_metadata(&value, [1; 32], "post").is_err());
        }
    }
    for mutation in 0..13 {
        let mut value = metadata([1; 32], "post");
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
            _ => value
                .explicit_arguments
                .push(value.explicit_arguments[12].clone()),
        }
        assert!(validate_metadata(&value, [1; 32], "post").is_err());
    }
    let mut optional = metadata([1; 32], "post");
    for role in &mut optional.explicit_arguments {
        role.access = None;
        role.pointee_alignment = None;
    }
    validate_metadata(&optional, [1; 32], "post").unwrap();
}
#[test]
fn attention_output_v5_regions_and_fixups_reject_aliases_extent_role_and_overflow() {
    let good = regions();
    validate_regions(&good, &fixups(&good)).unwrap();
    for role in 0..15 {
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
fn attention_output_v5_metadata_permutation_bounds_and_opaque_bits_fail_closed() {
    let good = inputs()[5].clone();
    assert_eq!(cache_slot(&good).unwrap(), 192);
    for position in [2304, 0x7f80_0001, 0x7fc0_0000, u32::MAX] {
        let mut bytes = good.clone();
        bytes[..4].copy_from_slice(&position.to_le_bytes());
        assert!(cache_slot(&bytes).is_err());
    }
    for index in 1..145 {
        for page in [144_u32, 0x7fc0_0000, u32::MAX] {
            let mut bytes = good.clone();
            bytes[index * 4..index * 4 + 4].copy_from_slice(&page.to_le_bytes());
            assert!(cache_slot(&bytes).is_err());
        }
    }
    let mut duplicate = good.clone();
    let word = duplicate[4..8].to_vec();
    duplicate[8..12].copy_from_slice(&word);
    assert!(cache_slot(&duplicate).is_err());
    assert!(cache_slot(&good[..579]).is_err());
    assert!(decode_words(&[0, 0], 2).is_err());
    for geometry in [([32, 1, 1], [64, 1, 1]), ([64, 1, 1], [64, 1, 1])] {
        assert!(validate_geometry(geometry.0, geometry.1).is_err());
    }
}

#[test]
fn attention_output_v5_rejects_error_zero_ready_but_unclaimed_output_after_bounded_retirement() {
    let mut state = completed();
    state[1] = 1 << 15;
    state[2] = (1 << 15) - 1;
    state[3] = (1 << 15) - 1;
    state[4] &= (1 << 30) - 1;
    state[21] = 0;
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
fn attention_output_v5_wrong_input_or_output_extent_never_loads_or_publishes() {
    for role in 0..14 {
        let fixture = Fixture::new();
        let trace = Rc::clone(&fixture.trace);
        let mut inputs = inputs();
        let mut outputs = outputs();
        if role < 7 {
            inputs[role].pop();
        } else if role < 13 {
            outputs[role - 7].pop();
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
                "fixture".into(),
                inputs.each_ref().map(Vec::as_slice),
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
fn attention_output_v5_cache_isolation_checks_both_sides_and_boundary_slots() {
    let before = vec![0x55; CACHE_WORDS * 2];
    for slot in [0, 192, 2303] {
        let start = slot * TOKEN_WORDS * 2;
        let end = start + TOKEN_WORDS * 2;
        let mut after = before.clone();
        after[start..end].fill(0x42);
        validate_cache_isolation(&before, &after, slot).unwrap();
        for byte in [0, end, before.len() - 1] {
            if byte >= before.len() || (start..end).contains(&byte) {
                continue;
            }
            after[byte] ^= 1;
            assert!(validate_cache_isolation(&before, &after, slot).is_err());
            after[byte] ^= 1;
        }
    }
    assert!(validate_cache_isolation(&before, &before, 2304).is_err());
    assert!(validate_cache_isolation(&before, &before[..before.len() - 1], 0).is_err());
}

#[test]
fn attention_output_v5_request_digest_symbol_and_deadline_fail_closed() {
    let object = vec![1, 2, 3];
    let digest = Sha256::digest(&object).into();
    validate_request(&object, digest, "post", 100).unwrap();
    assert!(validate_request(&[], digest, "post", 100).is_err());
    assert!(validate_request(&object, [0; 32], "post", 100).is_err());
    for symbol in ["", "bad\0symbol"] {
        assert!(validate_request(&object, digest, symbol, 100).is_err());
    }
    assert!(validate_request(&object, digest, &"x".repeat(257), 100).is_err());
    for timeout in [0, 600_001] {
        assert!(validate_request(&object, digest, "post", timeout).is_err());
    }
}
