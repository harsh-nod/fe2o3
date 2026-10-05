// Synthetic canonical extractor facts, never an authenticated LLVM invocation.
use super::trace_api::*;

#[derive(Clone)]
pub struct Instruction {
    pub encoding: Vec<u8>,
    pub opcode: String,
    pub destination: Option<u8>,
    pub sources: Vec<PhysicalMachineOperandValueV1>,
    pub scc: bool,
}

impl Instruction {
    pub fn mov(destination: u8, source: u8) -> Self {
        Self::selected(true, destination, source, 0, 0)
    }

    pub fn add(destination: u8, left: u8, right: u8) -> Self {
        Self::selected(false, destination, left, right, 0)
    }

    pub fn selected(mov: bool, destination: u8, left: u8, right: u8, literal: u32) -> Self {
        let word = if mov { 0xbe80_0000 } else { 0x8000_0000 }
            | (u32::from(destination) << 16)
            | u32::from(left)
            | if mov { 0 } else { u32::from(right) << 8 };
        let selectors = if mov { vec![left] } else { vec![left, right] };
        let mut encoding = word.to_le_bytes().to_vec();
        if selectors.contains(&255) {
            encoding.extend_from_slice(&literal.to_le_bytes());
        }
        let sources = selectors
            .into_iter()
            .map(|source| match source {
                0..=101 => PhysicalMachineOperandValueV1::Register(format!("SGPR{source}")),
                128..=192 => {
                    PhysicalMachineOperandValueV1::SignedImmediate(i64::from(source - 128))
                }
                193..=208 => {
                    PhysicalMachineOperandValueV1::SignedImmediate(192 - i64::from(source))
                }
                255 => PhysicalMachineOperandValueV1::SignedImmediate(i64::from(literal)),
                _ => PhysicalMachineOperandValueV1::SignedImmediate(0),
            })
            .collect();
        Self {
            encoding,
            opcode: if mov { "S_MOV_B32_vi" } else { "S_ADD_U32_vi" }.into(),
            destination: Some(destination),
            sources,
            scc: !mov,
        }
    }
}

pub struct Fixture {
    pub function: String,
    pub instructions: Vec<Instruction>,
    pub split_at: Option<usize>,
    pub backedge: bool,
}

impl Fixture {
    pub fn new(instructions: Vec<Instruction>) -> Self {
        Self {
            function: "integer_step".into(),
            instructions,
            split_at: None,
            backedge: false,
        }
    }

    pub fn analysis(
        &self,
    ) -> (
        PhysicalMachineEffectRequestV1,
        PhysicalMachineAnalysisEvidenceV1,
    ) {
        self.analysis_for_target(PhysicalMachineTargetV1::Gfx942XnackMinusCov6)
    }

    pub fn analysis_for_target(
        &self,
        target: PhysicalMachineTargetV1,
    ) -> (
        PhysicalMachineEffectRequestV1,
        PhysicalMachineAnalysisEvidenceV1,
    ) {
        let (effect_domain, trace_domain, bundle_domain, tag): (&[u8], &[u8], &[u8], u16) =
            match target {
                PhysicalMachineTargetV1::Gfx942XnackMinusCov6 => (
                    PHYSICAL_MACHINE_EFFECT_EVIDENCE_DOMAIN_V1,
                    PHYSICAL_MACHINE_TRACE_EVIDENCE_DOMAIN_V1,
                    PHYSICAL_MACHINE_ANALYSIS_BUNDLE_DOMAIN_V1,
                    1,
                ),
                PhysicalMachineTargetV1::Gfx950XnackMinusCov6 => (
                    b"FE2O3/GFX950-PHYSICAL-MACHINE-EFFECT-EVIDENCE/V1\0",
                    b"FE2O3/GFX950-PHYSICAL-MACHINE-TRACE-EVIDENCE/V1\0",
                    b"FE2O3/GFX950-PHYSICAL-MACHINE-ANALYSIS-BUNDLE/V1\0",
                    2,
                ),
            };
        assert!(!self.instructions.is_empty());
        assert!(!(self.backedge && self.split_at.is_some()));
        let mut payload = vec![0; 4];
        if self.backedge {
            payload.extend_from_slice(&[0, 0, 0x82, 0xbf]);
        }
        let mut offsets = Vec::new();
        for instruction in &self.instructions {
            offsets.push(payload.len() as u64);
            payload.extend_from_slice(&instruction.encoding);
        }
        let branch_offset = payload.len() as u64;
        if self.backedge {
            let displacement = ((8i64 - branch_offset as i64 - 4) / 4) as i16;
            payload
                .extend_from_slice(&(0xbf85_0000 | u32::from(displacement as u16)).to_le_bytes());
        }
        let end_offset = payload.len() as u64;
        payload.extend_from_slice(&[0, 0, 0x81, 0xbf]);
        let request = PhysicalMachineEffectRequestV1::new_for_target(
            target,
            PhysicalMachineExecutionChallengeV1::from_sha256_bytes([0x10; 32]),
            PhysicalMachineAnalyzerIdentityV1::from_sha256_bytes([0x11; 32]),
            PhysicalMachineToolchainIdentityV1::from_sha256_bytes([0x22; 32]),
            payload.clone(),
            vec![
                PhysicalMachineEffectEntryRequestV1::new(
                    &self.function,
                    PhysicalMachineEffectBudgetV1::new(0, 0, 0, 1, 0),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let mut effects = Vec::from(effect_domain);
        u32v(&mut effects, 0);
        u16v(&mut effects, PHYSICAL_MACHINE_EFFECT_SCHEMA_VERSION_V1);
        effects.extend_from_slice(&request.execution_challenge().as_bytes());
        identity(
            &mut effects,
            request.identity().sha256(),
            request.identity().byte_len(),
        );
        identity(
            &mut effects,
            request.payload_identity().sha256(),
            request.payload_identity().byte_len(),
        );
        effects.extend_from_slice(&request.analyzer_identity().as_bytes());
        effects.extend_from_slice(&request.toolchain_identity().as_bytes());
        u16v(&mut effects, tag);
        u16v(&mut effects, 1);
        text(&mut effects, &self.function);
        effects.extend_from_slice(&[0x33; 32]);
        u64v(&mut effects, 4);
        u64v(&mut effects, payload.len() as u64 - 4);
        u32v(&mut effects, 1);
        text(&mut effects, &self.function);
        u64v(&mut effects, 4);
        u64v(&mut effects, payload.len() as u64 - 4);
        u16v(&mut effects, 0);
        u32v(&mut effects, 1);
        text(&mut effects, &self.function);
        text(&mut effects, &self.function);
        u64v(&mut effects, end_offset);
        effects.push(4);
        u16v(&mut effects, 0);
        finish(&mut effects, effect_domain);
        let effects =
            PhysicalMachineEffectEvidenceV1::decode_canonical_for(&request, &effects).unwrap();
        let mut trace = Vec::from(trace_domain);
        u32v(&mut trace, 0);
        u16v(&mut trace, PHYSICAL_MACHINE_TRACE_SCHEMA_VERSION_V1);
        trace.extend_from_slice(&request.execution_challenge().as_bytes());
        identity(
            &mut trace,
            request.identity().sha256(),
            request.identity().byte_len(),
        );
        identity(
            &mut trace,
            effects.identity().sha256(),
            effects.identity().byte_len(),
        );
        identity(
            &mut trace,
            request.payload_identity().sha256(),
            request.payload_identity().byte_len(),
        );
        trace.extend_from_slice(&request.analyzer_identity().as_bytes());
        trace.extend_from_slice(&request.toolchain_identity().as_bytes());
        u16v(&mut trace, tag);
        let split = self.split_at.unwrap_or(self.instructions.len());
        let two_blocks = self.split_at.is_some() || self.backedge;
        if self.backedge {
            u32v(&mut trace, 3);
            block(&mut trace, &self.function, 0, 4, 1, &[1]);
            block(&mut trace, &self.function, 1, 8, split + 1, &[1, 2]);
            block(&mut trace, &self.function, 2, end_offset, 1, &[]);
        } else if two_blocks {
            u32v(&mut trace, 2);
            block(&mut trace, &self.function, 0, 4, split, &[1]);
            block(
                &mut trace,
                &self.function,
                1,
                offsets[split],
                self.instructions.len() - split + 1,
                &[],
            );
        } else {
            u32v(&mut trace, 1);
            block(&mut trace, &self.function, 0, 4, split + 1, &[]);
        }
        u32v(
            &mut trace,
            (self.instructions.len() + 1 + 2 * usize::from(self.backedge)) as u32,
        );
        if self.backedge {
            header(
                &mut trace,
                &self.function,
                4,
                0,
                "S_BRANCH",
                &[0, 0, 0x82, 0xbf],
            );
            u16v(&mut trace, 0);
            u16v(&mut trace, 1);
            operand(
                &mut trace,
                &PhysicalMachineOperandValueV1::SignedImmediate(0),
            );
            u16v(&mut trace, 0);
            u16v(&mut trace, 0);
            tail(&mut trace, 2, 8, 4);
        }
        for (index, instruction) in self.instructions.iter().enumerate() {
            header(
                &mut trace,
                &self.function,
                offsets[index],
                u32::from(self.backedge || self.split_at.is_some_and(|split| index >= split)),
                &instruction.opcode,
                &instruction.encoding,
            );
            u16v(&mut trace, u16::from(instruction.destination.is_some()));
            u16v(
                &mut trace,
                (instruction.sources.len() + usize::from(instruction.destination.is_some())) as u16,
            );
            if let Some(destination) = instruction.destination {
                operand(
                    &mut trace,
                    &PhysicalMachineOperandValueV1::Register(format!("SGPR{destination}")),
                );
            }
            for source in &instruction.sources {
                operand(&mut trace, source);
            }
            u16v(&mut trace, u16::from(instruction.scc));
            if instruction.scc {
                text(&mut trace, "SCC");
            }
            u16v(&mut trace, 0);
            tail(&mut trace, 0, 0, 0);
        }
        if self.backedge {
            header(
                &mut trace,
                &self.function,
                branch_offset,
                1,
                "S_CBRANCH_SCC1",
                &payload[branch_offset as usize..end_offset as usize],
            );
            u16v(&mut trace, 0);
            u16v(&mut trace, 1);
            operand(
                &mut trace,
                &PhysicalMachineOperandValueV1::SignedImmediate((8 - branch_offset as i64 - 4) / 4),
            );
            u16v(&mut trace, 0);
            u16v(&mut trace, 1);
            text(&mut trace, "SCC");
            tail(&mut trace, 1, 8, 4);
        }
        header(
            &mut trace,
            &self.function,
            end_offset,
            if self.backedge {
                2
            } else {
                u32::from(two_blocks)
            },
            "S_ENDPGM_vi",
            &[0, 0, 0x81, 0xbf],
        );
        for _ in 0..4 {
            u16v(&mut trace, 0);
        }
        tail(&mut trace, 4, 0, 4);
        finish(&mut trace, trace_domain);
        let mut bundle = Vec::from(bundle_domain);
        u32v(&mut bundle, 0);
        u16v(
            &mut bundle,
            PHYSICAL_MACHINE_ANALYSIS_BUNDLE_SCHEMA_VERSION_V1,
        );
        u32v(&mut bundle, effects.canonical_bytes().len() as u32);
        bundle.extend_from_slice(effects.canonical_bytes());
        u32v(&mut bundle, trace.len() as u32);
        bundle.extend_from_slice(&trace);
        finish(&mut bundle, bundle_domain);
        let analysis =
            PhysicalMachineAnalysisEvidenceV1::decode_canonical_for(&request, &bundle).unwrap();
        (request, analysis)
    }
}

fn block(
    out: &mut Vec<u8>,
    function: &str,
    ordinal: u32,
    offset: u64,
    count: usize,
    successors: &[u32],
) {
    text(out, function);
    u32v(out, ordinal);
    u64v(out, offset);
    u32v(out, count as u32);
    u16v(out, successors.len() as u16);
    for successor in successors {
        u32v(out, *successor);
    }
}

fn header(
    out: &mut Vec<u8>,
    function: &str,
    offset: u64,
    block: u32,
    opcode: &str,
    encoding: &[u8],
) {
    text(out, function);
    u64v(out, offset);
    u32v(out, block);
    text(out, opcode);
    u16v(out, encoding.len() as u16);
    out.extend_from_slice(encoding);
}
fn operand(out: &mut Vec<u8>, value: &PhysicalMachineOperandValueV1) {
    out.push(match value {
        PhysicalMachineOperandValueV1::Register(_) => 1,
        PhysicalMachineOperandValueV1::SignedImmediate(_) => 2,
        _ => panic!("integer fixture"),
    });
    u16v(out, u16::MAX);
    match value {
        PhysicalMachineOperandValueV1::Register(value) => text(out, value),
        PhysicalMachineOperandValueV1::SignedImmediate(value) => u64v(out, *value as u64),
        _ => unreachable!(),
    }
}
fn tail(out: &mut Vec<u8>, branch: u8, target: u64, flags: u16) {
    out.push(branch);
    u64v(out, target);
    u16v(out, flags);
    out.push(0);
    u16v(out, 0);
}
fn identity(out: &mut Vec<u8>, hash: [u8; 32], len: u64) {
    out.extend_from_slice(&hash);
    u64v(out, len);
}
fn finish(out: &mut [u8], domain: &[u8]) {
    let len = out.len() as u32;
    out[domain.len()..domain.len() + 4].copy_from_slice(&len.to_le_bytes());
}
fn u16v(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn u32v(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn u64v(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn text(out: &mut Vec<u8>, value: &str) {
    u16v(out, value.len() as u16);
    out.extend_from_slice(value.as_bytes());
}
