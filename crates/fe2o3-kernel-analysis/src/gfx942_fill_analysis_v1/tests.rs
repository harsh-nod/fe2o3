use super::*;
use crate::{
    PHYSICAL_MACHINE_ANALYSIS_BUNDLE_DOMAIN_V1 as BUNDLE,
    PHYSICAL_MACHINE_EFFECT_EVIDENCE_DOMAIN_V1 as EFFECTS,
    PHYSICAL_MACHINE_TRACE_EVIDENCE_DOMAIN_V1 as TRACE, PhysicalMachineEffectEvidenceV1,
    PhysicalMachineEffectRequestV1, PhysicalMachineInstructionTraceV1,
    PhysicalMachineOperandValueV1 as Operand,
};

const REQUEST: &[u8] = include_bytes!("fill.request");
const ANALYSIS: &[u8] = include_bytes!("fill.bundle");

fn captured() -> (
    PhysicalMachineEffectRequestV1,
    PhysicalMachineAnalysisEvidenceV1,
) {
    let request = PhysicalMachineEffectRequestV1::decode_canonical(REQUEST).unwrap();
    let analysis =
        PhysicalMachineAnalysisEvidenceV1::decode_canonical_for(&request, ANALYSIS).unwrap();
    (request, analysis)
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Mutation {
    None,
    Descriptor,
    ReadWidth,
    WriteWidth,
    Branch,
    Split,
    Flag(u16),
    ClearEndBarrier,
}

// Test-only structured serializer: no authenticated execution can be made from it.
struct Wire {
    bytes: Vec<u8>,
    length: usize,
}
impl Wire {
    fn new(domain: &[u8]) -> Self {
        let mut value = Self {
            bytes: domain.to_vec(),
            length: domain.len(),
        };
        value.u32(0);
        value.u16(1);
        value
    }
    fn u8(&mut self, v: u8) {
        self.bytes.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.raw(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.raw(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.raw(&v.to_le_bytes());
    }
    fn raw(&mut self, v: &[u8]) {
        self.bytes.extend_from_slice(v);
    }
    fn text(&mut self, v: &str) {
        self.u16(v.len().try_into().unwrap());
        self.raw(v.as_bytes());
    }
    fn identity(&mut self, hash: [u8; 32], len: u64) {
        self.raw(&hash);
        self.u64(len);
    }
    fn finish(mut self) -> Vec<u8> {
        let len = u32::try_from(self.bytes.len()).unwrap();
        self.bytes[self.length..self.length + 4].copy_from_slice(&len.to_le_bytes());
        self.bytes
    }
    fn request(&mut self, request: &PhysicalMachineEffectRequestV1) {
        self.raw(&request.execution_challenge().as_bytes());
        self.identity(request.identity().sha256(), request.identity().byte_len());
    }
    fn payload(&mut self, request: &PhysicalMachineEffectRequestV1) {
        self.identity(
            request.payload_identity().sha256(),
            request.payload_identity().byte_len(),
        );
        self.raw(&request.analyzer_identity().as_bytes());
        self.raw(&request.toolchain_identity().as_bytes());
        self.u16(1);
    }
}

fn encode(
    request: &PhysicalMachineEffectRequestV1,
    analysis: &PhysicalMachineAnalysisEvidenceV1,
    mutation: Mutation,
) -> Vec<u8> {
    let mut effects = Wire::new(EFFECTS);
    effects.request(request);
    effects.payload(request);
    let original = analysis.effects();
    effects.u16(original.entry_points().len().try_into().unwrap());
    for entry in original.entry_points() {
        effects.text(entry.symbol());
        let mut descriptor = entry.descriptor_identity().as_bytes();
        if mutation == Mutation::Descriptor {
            descriptor[0] ^= 1;
        }
        effects.raw(&descriptor);
        effects.u64(entry.code_offset());
        effects.u64(entry.code_size());
    }
    effects.u32(original.functions().len().try_into().unwrap());
    for function in original.functions() {
        effects.text(function.symbol());
        effects.u64(function.code_offset());
        effects.u64(function.code_size());
        effects.u16(function.direct_callees().len().try_into().unwrap());
        for callee in function.direct_callees() {
            effects.text(callee);
        }
    }
    effects.u32(original.effects().len().try_into().unwrap());
    for effect in original.effects() {
        effects.text(effect.entry_symbol());
        effects.text(effect.function_symbol());
        effects.u64(effect.instruction_offset());
        effects.u8(match effect.kind() {
            Effect::GlobalAddress => 1,
            Effect::GlobalRead => 2,
            Effect::GlobalWrite => 3,
            Effect::Return => 4,
        });
        let width = match (mutation, effect.kind()) {
            (Mutation::ReadWidth, Effect::GlobalRead)
            | (Mutation::WriteWidth, Effect::GlobalWrite) => 8,
            _ => effect.byte_width(),
        };
        effects.u16(width);
    }
    let effects = effects.finish();
    let decoded = PhysicalMachineEffectEvidenceV1::decode_canonical_for(request, &effects).unwrap();
    let mut trace = Wire::new(TRACE);
    trace.request(request);
    trace.identity(decoded.identity().sha256(), decoded.identity().byte_len());
    trace.payload(request);
    let original = analysis.trace();
    let start = analysis.effects().entry_points()[0].code_offset();
    trace.u32(original.blocks().len().try_into().unwrap());
    for block in original.blocks() {
        trace.text(block.function_symbol());
        trace.u32(block.ordinal());
        trace.u64(block.first_instruction_offset());
        trace.u32(
            block.instruction_count()
                + u32::from(mutation == Mutation::Split && block.ordinal() == 1),
        );
        let successors = if mutation == Mutation::Branch && block.ordinal() == 0 {
            &[1][..]
        } else {
            block.successors()
        };
        trace.u16(successors.len().try_into().unwrap());
        for successor in successors {
            trace.u32(*successor);
        }
    }
    trace.u32(
        (original.instructions().len() + usize::from(mutation == Mutation::Split))
            .try_into()
            .unwrap(),
    );
    for instruction in original.instructions() {
        let offset = instruction.instruction_offset() - start;
        if mutation == Mutation::Split && offset == 48 {
            instruction_wire(&mut trace, instruction, start, mutation, Some(0));
            instruction_wire(&mut trace, instruction, start, mutation, Some(4));
        } else {
            instruction_wire(&mut trace, instruction, start, mutation, None);
        }
    }
    let trace = trace.finish();
    let mut bundle = Wire::new(BUNDLE);
    bundle.u32(effects.len().try_into().unwrap());
    bundle.raw(&effects);
    bundle.u32(trace.len().try_into().unwrap());
    bundle.raw(&trace);
    bundle.finish()
}

fn instruction_wire(
    out: &mut Wire,
    instruction: &PhysicalMachineInstructionTraceV1,
    start: u64,
    mutation: Mutation,
    split: Option<usize>,
) {
    let offset = instruction.instruction_offset() - start;
    out.text(instruction.function_symbol());
    out.u64(instruction.instruction_offset() + split.unwrap_or(0) as u64);
    out.u32(instruction.block_ordinal());
    out.text(instruction.opcode());
    let encoding = split.map_or(instruction.encoding(), |part| {
        &instruction.encoding()[part..part + 4]
    });
    out.u16(encoding.len().try_into().unwrap());
    out.raw(encoding);
    out.u16(instruction.explicit_definition_count());
    out.u16(instruction.operands().len().try_into().unwrap());
    for operand in instruction.operands() {
        out.u8(match operand.value() {
            Operand::Register(_) => 1,
            Operand::SignedImmediate(_) => 2,
            Operand::SingleFloatImmediate(_) => 3,
            Operand::DoubleFloatImmediate(_) => 4,
            Operand::AbsoluteExpression(_) => 5,
        });
        out.u16(operand.tied_to().unwrap_or(u16::MAX));
        match operand.value() {
            Operand::Register(value) => out.text(value),
            Operand::SignedImmediate(value) | Operand::AbsoluteExpression(value) => {
                out.u64(*value as u64)
            }
            Operand::SingleFloatImmediate(value) => out.u32(*value),
            Operand::DoubleFloatImmediate(value) => out.u64(*value),
        }
    }
    for registers in [
        instruction.implicit_definitions(),
        instruction.implicit_uses(),
    ] {
        out.u16(registers.len().try_into().unwrap());
        for register in registers {
            out.text(register);
        }
    }
    out.u8(match instruction.branch_kind() {
        Branch::None => 0,
        Branch::ConditionalDirect => 1,
        Branch::UnconditionalDirect => 2,
        Branch::DirectCall => 3,
        Branch::Return => 4,
    });
    out.u64(if mutation == Mutation::Branch && offset == 36 {
        start + 40
    } else {
        instruction.branch_target().unwrap_or(0)
    });
    let flags = match mutation {
        Mutation::Flag(bits) if offset == 8 => instruction.flags().bits() | bits,
        Mutation::ClearEndBarrier if offset == 64 => instruction.flags().bits() & !8,
        _ => instruction.flags().bits(),
    };
    out.u16(flags);
    out.u8(match instruction.memory_access() {
        Memory::None => 0,
        Memory::Read { .. } => 1,
        Memory::Write { .. } => 2,
        Memory::ReadWrite { .. } => 3,
        Memory::WorkgroupRead { .. } => 4,
        Memory::WorkgroupWrite { .. } => 5,
        Memory::WorkgroupReadWrite { .. } => 6,
    });
    out.u16(match (mutation, offset) {
        (Mutation::ReadWidth, 0) | (Mutation::WriteWidth, 56) => 8,
        _ => instruction.memory_access().byte_width(),
    });
}

#[test]
fn captured_native_bundle_replays_without_authenticating_an_execution() {
    let (request, analysis) = captured();
    assert_eq!(encode(&request, &analysis, Mutation::None), ANALYSIS);
    let kernel = Gfx942FillKernelV1::inspect(request.exact_payload_bytes(), 0).unwrap();
    check_profile(&analysis, "fill_write_only", &kernel).unwrap();
    assert!(!analysis.establishes_compiler_refinement());
    assert!(!analysis.grants_launch_authority());
    assert_eq!(
        check_profile(&analysis, "other", &kernel),
        Err(Gfx942FillAnalysisErrorV1::Entry)
    );
}

#[test]
fn valid_structured_metadata_mutations_do_not_pass_the_closed_profile() {
    use Gfx942FillAnalysisErrorV1 as E;
    let (request, analysis) = captured();
    let kernel = Gfx942FillKernelV1::inspect(request.exact_payload_bytes(), 0).unwrap();
    for (mutation, expected) in [
        (Mutation::Descriptor, E::Descriptor),
        (Mutation::ReadWidth, E::Effects),
        (Mutation::WriteWidth, E::Effects),
        (Mutation::Branch, E::ControlFlow),
        (Mutation::Split, E::ControlFlow),
        (Mutation::Flag(8), E::Instructions),
        (Mutation::Flag(16), E::Instructions),
        (Mutation::Flag(32), E::Instructions),
        (Mutation::ClearEndBarrier, E::Instructions),
    ] {
        let changed = encode(&request, &analysis, mutation);
        let decoded = PhysicalMachineAnalysisEvidenceV1::decode_canonical_for(&request, &changed)
            .unwrap_or_else(|error| panic!("{mutation:?} must remain decodable: {error}"));
        assert_eq!(
            check_profile(&decoded, "fill_write_only", &kernel),
            Err(expected),
            "{mutation:?}"
        );
    }
}

#[test]
fn request_payload_or_challenge_substitution_cannot_reuse_captured_analysis() {
    let (request, _) = captured();
    for change_payload in [false, true] {
        let mut payload = request.exact_payload_bytes().to_vec();
        let mut challenge = request.execution_challenge().as_bytes();
        if change_payload {
            *payload.last_mut().unwrap() ^= 1;
        } else {
            challenge[0] ^= 1;
        }
        let changed = PhysicalMachineEffectRequestV1::new(
            crate::PhysicalMachineExecutionChallengeV1::from_sha256_bytes(challenge),
            request.analyzer_identity(),
            request.toolchain_identity(),
            payload,
            request.entries().to_vec(),
        )
        .unwrap();
        assert!(
            PhysicalMachineAnalysisEvidenceV1::decode_canonical_for(&changed, ANALYSIS).is_err()
        );
    }
}

#[test]
fn changed_instruction_bytes_still_fail_model_inspection() {
    let (request, analysis) = captured();
    let start = analysis.effects().entry_points()[0].code_offset() as usize;
    let mut payload = request.exact_payload_bytes().to_vec();
    payload[start + 8] ^= 1;
    assert!(matches!(
        Gfx942FillKernelV1::inspect(&payload, 0),
        Err(Gfx942FillErrorV1::Instruction { word: 2 })
    ));
}

#[test]
fn complete_implicit_kernarg_storage_is_bounded_in_both_execution_apis() {
    let (request, _) = captured();
    let kernel = Gfx942FillKernelV1::inspect(request.exact_payload_bytes(), 0).unwrap();
    assert_eq!(kernel.kernarg_storage_bytes(), 272);
    let short = Gfx942FillKernelV1::inspect(
        include_bytes!(
            "../../../fe2o3-hsaco/tests/fixtures/rust-fill-write-only-gfx942/kernel.hsaco"
        ),
        0,
    )
    .unwrap();
    assert_eq!(short.kernarg_storage_bytes(), 16);
    for (address, output, count, accepted) in [
        (0x1000u64, 0x1010u64, 1u64, false),
        (0x1000, 0x110c, 1, false),
        (0x1000, 0x1110, 1, true),
        (0x1000, 0xffc, 1, true),
        (0x1000, 0x1010, 0, true),
        (u64::MAX - 23, 0x8000, 1, false),
        (u64::MAX - 279, 0x8000, 1, true),
    ] {
        let mut kernarg = [0u8; 16];
        kernarg[..8].copy_from_slice(&output.to_le_bytes());
        kernarg[8..].copy_from_slice(&count.to_le_bytes());
        let state = crate::Gfx942FillWaveStateV1 {
            sgprs: [address as u32, (address >> 32) as u32, 0, 0, 0, 0, 0, 0],
            vgprs: std::array::from_fn(|lane| [lane as u32, 0, 0, 0]),
            exec_mask: u64::MAX,
            vcc: 0,
            scc: false,
        };
        let wave = kernel.execute(state, &kernarg, output, count * 4, [64, 1, 1], [0, 0]);
        let dispatch =
            kernel.check_dispatch(kernarg, address, output, count * 4, [64, 1, 1], [64, 1, 1]);
        assert_eq!(
            wave.is_ok(),
            accepted,
            "wave: {address:#x}/{output:#x}/{count}"
        );
        assert_eq!(
            dispatch.is_ok(),
            accepted,
            "dispatch: {address:#x}/{output:#x}/{count}"
        );
        if accepted {
            let dispatch = dispatch.unwrap();
            assert_eq!(dispatch.byte_after(address + 16, 0xa5), 0xa5);
            assert_eq!(dispatch.byte_after(address + 271, 0xa5), 0xa5);
        }
        assert!(
            short
                .execute(state, &kernarg, output, count * 4, [64, 1, 1], [0, 0])
                .is_ok()
        );
        assert!(
            short
                .check_dispatch(kernarg, address, output, count * 4, [64, 1, 1], [64, 1, 1])
                .is_ok()
        );
    }
}

fn field<'a>(value: &'a mut rmpv::Value, key: &str) -> &'a mut rmpv::Value {
    let rmpv::Value::Map(entries) = value else {
        panic!("expected metadata map")
    };
    &mut entries
        .iter_mut()
        .find(|(name, _)| name.as_str() == Some(key))
        .unwrap()
        .1
}

fn array(value: &mut rmpv::Value) -> &mut Vec<rmpv::Value> {
    let rmpv::Value::Array(entries) = value else {
        panic!("expected metadata array")
    };
    entries
}

#[test]
fn substituted_storage_alignment_and_argument_metadata_reject() {
    let (request, _) = captured();
    let payload = request.exact_payload_bytes();
    let inspected = inspect_and_bind_kernel_descriptors(payload).unwrap();
    let range = inspected.inspection().metadata_descriptor_range();
    let start = range.file_offset() as usize;
    let end = start + range.byte_len() as usize;
    let metadata = rmpv::decode::read_value(&mut &payload[start..end]).unwrap();
    for case in 0..7 {
        let mut metadata = metadata.clone();
        let kernel = &mut array(field(&mut metadata, "amdhsa.kernels"))[0];
        match case {
            0 => *field(kernel, ".kernarg_segment_align") = 16u32.into(),
            1 => *field(kernel, ".kernarg_segment_size") = 280u32.into(),
            _ => {
                let args = array(field(kernel, ".args"));
                let (row, key, value) = match case {
                    2 => (0, ".offset", 4u32),
                    3 => (0, ".size", 4),
                    4 => (1, ".offset", 4),
                    5 => (2, ".offset", 17),
                    6 => (2, ".size", 8),
                    _ => unreachable!(),
                };
                *field(&mut args[row], key) = value.into();
            }
        }
        let mut encoded = Vec::new();
        rmpv::encode::write_value(&mut encoded, &metadata).unwrap();
        assert_eq!(
            encoded.len(),
            end - start,
            "fixed-size metadata mutant {case}"
        );
        let mut changed = payload.to_vec();
        changed[start..end].copy_from_slice(&encoded);
        assert!(
            Gfx942FillKernelV1::inspect(&changed, 0).is_err(),
            "case {case}"
        );
    }
}
