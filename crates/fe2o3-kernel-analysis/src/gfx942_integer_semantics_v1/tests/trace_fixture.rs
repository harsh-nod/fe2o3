// Synthetic canonical extractor facts for unit tests, not authenticated worker
// output. The caller supplies a trace_api module reexporting kernel-analysis API.
use super::trace_api::*;

#[derive(Clone)]
pub struct TraceFixture {
    pub function: String,
    pub encoding: Vec<u8>,
    pub opcode: String,
    pub definitions: u16,
    pub operands: Vec<(PhysicalMachineOperandValueV1, Option<u16>)>,
    pub implicit_definitions: Vec<String>,
    pub implicit_uses: Vec<String>,
    pub flags: u16,
    pub memory: u8,
    pub width: u16,
    pub returning: bool,
}

impl TraceFixture {
    pub fn registers(destination: u8, left: u8, right: u8) -> Self {
        let word = 0x8000_0000
            | (u32::from(destination) << 16)
            | (u32::from(right) << 8)
            | u32::from(left);
        Self {
            function: "integer_step".into(),
            encoding: word.to_le_bytes().to_vec(),
            opcode: "S_ADD_U32_vi".into(),
            definitions: 1,
            operands: [destination, left, right]
                .map(|index| {
                    (
                        PhysicalMachineOperandValueV1::Register(format!("SGPR{index}")),
                        None,
                    )
                })
                .to_vec(),
            implicit_definitions: vec!["SCC".into()],
            implicit_uses: vec![],
            flags: 0,
            memory: 0,
            width: 0,
            returning: false,
        }
    }

    pub fn instruction(&self) -> PhysicalMachineInstructionTraceV1 {
        let (_, analysis) = self.analysis();
        analysis.trace().instructions()[0].clone()
    }

    pub fn analysis(
        &self,
    ) -> (
        PhysicalMachineEffectRequestV1,
        PhysicalMachineAnalysisEvidenceV1,
    ) {
        self.analysis_for_target(PhysicalMachineTargetV1::Gfx942XnackMinusCov6)
    }

    #[allow(
        dead_code,
        reason = "used by cross-crate target-substitution regressions"
    )]
    pub fn analysis_for_target(
        &self,
        target: PhysicalMachineTargetV1,
    ) -> (
        PhysicalMachineEffectRequestV1,
        PhysicalMachineAnalysisEvidenceV1,
    ) {
        self.analysis_inner(false, target)
    }

    #[allow(
        dead_code,
        reason = "used by the verifier's cross-crate dataflow regression"
    )]
    pub fn analysis_with_backedge(
        &self,
    ) -> (
        PhysicalMachineEffectRequestV1,
        PhysicalMachineAnalysisEvidenceV1,
    ) {
        assert!(!self.returning && self.encoding.len() == 4);
        self.analysis_inner(true, PhysicalMachineTargetV1::Gfx942XnackMinusCov6)
    }

    fn analysis_inner(
        &self,
        backedge: bool,
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
        // A separate entry predecessor preserves the initial live-in alongside the loop value.
        let instruction_offset = if backedge { 8 } else { 4 };
        let mut payload = vec![0; 4];
        if backedge {
            payload.extend_from_slice(&[0, 0, 0x82, 0xbf]);
        }
        payload.extend_from_slice(&self.encoding);
        if backedge {
            payload.extend_from_slice(&[0xfe, 0xff, 0x85, 0xbf]);
        }
        if !self.returning {
            payload.extend_from_slice(&[0, 0, 0x81, 0xbf]);
        }
        let request = PhysicalMachineEffectRequestV1::new_for_target(
            target,
            PhysicalMachineExecutionChallengeV1::from_sha256_bytes([0x10; 32]),
            PhysicalMachineAnalyzerIdentityV1::from_sha256_bytes([0x11; 32]),
            PhysicalMachineToolchainIdentityV1::from_sha256_bytes([0x22; 32]),
            payload.clone(),
            vec![
                PhysicalMachineEffectEntryRequestV1::new(
                    &self.function,
                    PhysicalMachineEffectBudgetV1::new(8, 4, 4, 2, 2),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        let mut effects = Vec::from(effect_domain);
        push_u32(&mut effects, 0);
        push_u16(&mut effects, PHYSICAL_MACHINE_EFFECT_SCHEMA_VERSION_V1);
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
        push_u16(&mut effects, tag);
        push_u16(&mut effects, 1);
        text(&mut effects, &self.function);
        effects.extend_from_slice(&[0x33; 32]);
        push_u64(&mut effects, 4);
        push_u64(&mut effects, payload.len() as u64 - 4);
        push_u32(&mut effects, 1);
        text(&mut effects, &self.function);
        push_u64(&mut effects, 4);
        push_u64(&mut effects, payload.len() as u64 - 4);
        push_u16(&mut effects, 0);
        let mut accesses = vec![];
        if (1..=3).contains(&self.memory) {
            accesses.push((instruction_offset, 1, 8));
            if self.memory == 1 || self.memory == 3 {
                accesses.push((instruction_offset, 2, self.width));
            }
            if self.memory == 2 || self.memory == 3 {
                accesses.push((instruction_offset, 3, self.width));
            }
        }
        accesses.push((
            if backedge {
                16
            } else if self.returning {
                4
            } else {
                4 + self.encoding.len() as u64
            },
            4,
            0,
        ));
        push_u32(&mut effects, accesses.len() as u32);
        for (offset, kind, width) in accesses {
            text(&mut effects, &self.function);
            text(&mut effects, &self.function);
            push_u64(&mut effects, offset);
            effects.push(kind);
            push_u16(&mut effects, width);
        }
        finish(&mut effects, effect_domain);
        let effects =
            PhysicalMachineEffectEvidenceV1::decode_canonical_for(&request, &effects).unwrap();
        let mut trace = Vec::from(trace_domain);
        push_u32(&mut trace, 0);
        push_u16(&mut trace, PHYSICAL_MACHINE_TRACE_SCHEMA_VERSION_V1);
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
        push_u16(&mut trace, tag);
        push_u32(&mut trace, if backedge { 3 } else { 1 });
        text(&mut trace, &self.function);
        push_u32(&mut trace, 0);
        push_u64(&mut trace, 4);
        let count = if self.returning { 1 } else { 2 };
        push_u32(&mut trace, if backedge { 1 } else { count });
        push_u16(&mut trace, u16::from(backedge));
        if backedge {
            push_u32(&mut trace, 1);
            text(&mut trace, &self.function);
            push_u32(&mut trace, 1);
            push_u64(&mut trace, instruction_offset);
            push_u32(&mut trace, 2);
            push_u16(&mut trace, 2);
            push_u32(&mut trace, 1);
            push_u32(&mut trace, 2);
            text(&mut trace, &self.function);
            push_u32(&mut trace, 2);
            push_u64(&mut trace, 16);
            push_u32(&mut trace, 1);
            push_u16(&mut trace, 0);
        }
        push_u32(&mut trace, if backedge { 4 } else { count });
        if backedge {
            self.push_branch(&mut trace, 4, 0, 0, false);
        }
        self.push_instruction(&mut trace, instruction_offset, u32::from(backedge));
        if backedge {
            self.push_branch(&mut trace, 12, 1, -2, true);
        }
        if !self.returning {
            let mut end = Self::registers(0, 0, 0);
            end.function.clone_from(&self.function);
            end.encoding = vec![0, 0, 0x81, 0xbf];
            end.opcode = "S_ENDPGM_vi".into();
            end.definitions = 0;
            end.operands.clear();
            end.implicit_definitions.clear();
            end.flags = 4;
            end.returning = true;
            end.push_instruction(
                &mut trace,
                if backedge {
                    16
                } else {
                    4 + self.encoding.len() as u64
                },
                if backedge { 2 } else { 0 },
            );
        }
        finish(&mut trace, trace_domain);
        let mut bundle = Vec::from(bundle_domain);
        push_u32(&mut bundle, 0);
        push_u16(
            &mut bundle,
            PHYSICAL_MACHINE_ANALYSIS_BUNDLE_SCHEMA_VERSION_V1,
        );
        push_u32(&mut bundle, effects.canonical_bytes().len() as u32);
        bundle.extend_from_slice(effects.canonical_bytes());
        push_u32(&mut bundle, trace.len() as u32);
        bundle.extend_from_slice(&trace);
        finish(&mut bundle, bundle_domain);
        let analysis =
            PhysicalMachineAnalysisEvidenceV1::decode_canonical_for(&request, &bundle).unwrap();
        (request, analysis)
    }

    fn push_branch(
        &self,
        out: &mut Vec<u8>,
        offset: u64,
        block: u32,
        displacement: i16,
        conditional: bool,
    ) {
        text(out, &self.function);
        push_u64(out, offset);
        push_u32(out, block);
        text(
            out,
            if conditional {
                "S_CBRANCH_SCC1"
            } else {
                "S_BRANCH"
            },
        );
        push_u16(out, 4);
        let opcode = if conditional {
            0xbf85_0000
        } else {
            0xbf82_0000
        };
        out.extend_from_slice(&(opcode | u32::from(displacement as u16)).to_le_bytes());
        push_u16(out, 0);
        push_u16(out, 1);
        out.push(2);
        push_u16(out, u16::MAX);
        push_u64(out, i64::from(displacement) as u64);
        push_u16(out, 0);
        push_u16(out, u16::from(conditional));
        if conditional {
            text(out, "SCC");
        }
        out.push(if conditional { 1 } else { 2 });
        push_u64(out, 8);
        push_u16(out, 4);
        out.push(0);
        push_u16(out, 0);
    }

    fn push_instruction(&self, out: &mut Vec<u8>, offset: u64, block: u32) {
        text(out, &self.function);
        push_u64(out, offset);
        push_u32(out, block);
        text(out, &self.opcode);
        push_u16(out, self.encoding.len() as u16);
        out.extend_from_slice(&self.encoding);
        push_u16(out, self.definitions);
        push_u16(out, self.operands.len() as u16);
        for (value, tied) in &self.operands {
            out.push(match value {
                PhysicalMachineOperandValueV1::Register(_) => 1,
                PhysicalMachineOperandValueV1::SignedImmediate(_) => 2,
                PhysicalMachineOperandValueV1::SingleFloatImmediate(_) => 3,
                PhysicalMachineOperandValueV1::DoubleFloatImmediate(_) => 4,
                PhysicalMachineOperandValueV1::AbsoluteExpression(_) => 5,
            });
            push_u16(out, tied.unwrap_or(u16::MAX));
            match value {
                PhysicalMachineOperandValueV1::Register(name) => text(out, name),
                PhysicalMachineOperandValueV1::SignedImmediate(value)
                | PhysicalMachineOperandValueV1::AbsoluteExpression(value) => {
                    push_u64(out, *value as u64)
                }
                PhysicalMachineOperandValueV1::SingleFloatImmediate(value) => push_u32(out, *value),
                PhysicalMachineOperandValueV1::DoubleFloatImmediate(value) => push_u64(out, *value),
            }
        }
        for registers in [&self.implicit_definitions, &self.implicit_uses] {
            push_u16(out, registers.len() as u16);
            for register in registers {
                text(out, register);
            }
        }
        out.push(if self.returning { 4 } else { 0 });
        push_u64(out, 0);
        push_u16(out, self.flags);
        out.push(self.memory);
        push_u16(out, self.width);
    }
}

fn identity(out: &mut Vec<u8>, hash: [u8; 32], len: u64) {
    out.extend_from_slice(&hash);
    push_u64(out, len);
}

fn finish(out: &mut [u8], domain: &[u8]) {
    let len = out.len() as u32;
    out[domain.len()..domain.len() + 4].copy_from_slice(&len.to_le_bytes());
}

fn push_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn push_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn text(out: &mut Vec<u8>, value: &str) {
    push_u16(out, value.len() as u16);
    out.extend_from_slice(value.as_bytes());
}
