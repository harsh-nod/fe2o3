//! One canonical response from the gfx942 physical-machine analyzer.
//!
//! The bundle keeps static effects and the exact LLVM/MC instruction trace
//! indivisible at the worker boundary. Decoding independently validates both
//! records against one request, including the trace's hash binding to the
//! canonical effect evidence. It remains inert evidence and grants no
//! publication, load, or launch authority.

use crate::{
    MAX_PHYSICAL_MACHINE_EFFECT_EVIDENCE_BYTES_V1, MAX_PHYSICAL_MACHINE_TRACE_BYTES_V1,
    PhysicalMachineEffectEvidenceErrorV1, PhysicalMachineEffectEvidenceV1,
    PhysicalMachineEffectRequestV1, PhysicalMachineTraceEvidenceErrorV1,
    PhysicalMachineTraceEvidenceV1,
};
use sha2::{Digest, Sha256};
use std::{error::Error, fmt};

pub const PHYSICAL_MACHINE_ANALYSIS_BUNDLE_DOMAIN_V1: &[u8] =
    b"FE2O3/GFX942-PHYSICAL-MACHINE-ANALYSIS-BUNDLE/V1\0";
const PHYSICAL_MACHINE_ANALYSIS_BUNDLE_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/GFX942-PHYSICAL-MACHINE-ANALYSIS-BUNDLE-IDENTITY/V1\0";
pub const PHYSICAL_MACHINE_ANALYSIS_BUNDLE_SCHEMA_VERSION_V1: u16 = 1;
pub const MAX_PHYSICAL_MACHINE_ANALYSIS_BUNDLE_BYTES_V1: usize =
    MAX_PHYSICAL_MACHINE_EFFECT_EVIDENCE_BYTES_V1 + MAX_PHYSICAL_MACHINE_TRACE_BYTES_V1 + 1024;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PhysicalMachineAnalysisBundleIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl PhysicalMachineAnalysisBundleIdentityV1 {
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhysicalMachineAnalysisEvidenceV1 {
    effects: PhysicalMachineEffectEvidenceV1,
    trace: PhysicalMachineTraceEvidenceV1,
    canonical_bytes: Vec<u8>,
}

impl PhysicalMachineAnalysisEvidenceV1 {
    pub fn decode_canonical_for(
        request: &PhysicalMachineEffectRequestV1,
        bytes: &[u8],
    ) -> Result<Self, PhysicalMachineAnalysisEvidenceErrorV1> {
        if bytes.len() > MAX_PHYSICAL_MACHINE_ANALYSIS_BUNDLE_BYTES_V1 {
            return Err(PhysicalMachineAnalysisEvidenceErrorV1::RecordTooLarge);
        }
        let mut input = BundleReader::new(bytes);
        input.expect(PHYSICAL_MACHINE_ANALYSIS_BUNDLE_DOMAIN_V1)?;
        if input.u32()? as usize != bytes.len() {
            return Err(PhysicalMachineAnalysisEvidenceErrorV1::LengthMismatch);
        }
        if input.u16()? != PHYSICAL_MACHINE_ANALYSIS_BUNDLE_SCHEMA_VERSION_V1 {
            return Err(PhysicalMachineAnalysisEvidenceErrorV1::UnsupportedVersion);
        }
        let effect_len = input.u32()? as usize;
        if effect_len == 0 || effect_len > MAX_PHYSICAL_MACHINE_EFFECT_EVIDENCE_BYTES_V1 {
            return Err(PhysicalMachineAnalysisEvidenceErrorV1::ComponentLength);
        }
        let effect_bytes = input.take(effect_len)?;
        let trace_len = input.u32()? as usize;
        if trace_len == 0 || trace_len > MAX_PHYSICAL_MACHINE_TRACE_BYTES_V1 {
            return Err(PhysicalMachineAnalysisEvidenceErrorV1::ComponentLength);
        }
        let trace_bytes = input.take(trace_len)?;
        input.finish()?;

        let effects = PhysicalMachineEffectEvidenceV1::decode_canonical_for(request, effect_bytes)
            .map_err(PhysicalMachineAnalysisEvidenceErrorV1::Effects)?;
        let trace =
            PhysicalMachineTraceEvidenceV1::decode_canonical_for(request, &effects, trace_bytes)
                .map_err(PhysicalMachineAnalysisEvidenceErrorV1::Trace)?;
        Ok(Self {
            effects,
            trace,
            canonical_bytes: bytes.to_vec(),
        })
    }

    pub const fn effects(&self) -> &PhysicalMachineEffectEvidenceV1 {
        &self.effects
    }

    pub const fn trace(&self) -> &PhysicalMachineTraceEvidenceV1 {
        &self.trace
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub fn identity(&self) -> PhysicalMachineAnalysisBundleIdentityV1 {
        PhysicalMachineAnalysisBundleIdentityV1 {
            sha256: domain_hash(
                PHYSICAL_MACHINE_ANALYSIS_BUNDLE_IDENTITY_DOMAIN_V1,
                &self.canonical_bytes,
            ),
            byte_len: self.canonical_bytes.len() as u64,
        }
    }

    pub const fn binds_exact_payload_instruction_bytes(&self) -> bool {
        true
    }

    pub const fn establishes_machine_semantics(&self) -> bool {
        false
    }

    pub const fn establishes_compiler_refinement(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PhysicalMachineAnalysisEvidenceErrorV1 {
    RecordTooLarge,
    Truncated,
    DomainMismatch,
    LengthMismatch,
    UnsupportedVersion,
    ComponentLength,
    TrailingBytes,
    Effects(PhysicalMachineEffectEvidenceErrorV1),
    Trace(PhysicalMachineTraceEvidenceErrorV1),
}

impl fmt::Display for PhysicalMachineAnalysisEvidenceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid physical machine analysis bundle: {self:?}"
        )
    }
}

impl Error for PhysicalMachineAnalysisEvidenceErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Effects(error) => Some(error),
            Self::Trace(error) => Some(error),
            _ => None,
        }
    }
}

struct BundleReader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> BundleReader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], PhysicalMachineAnalysisEvidenceErrorV1> {
        let end = self
            .position
            .checked_add(count)
            .ok_or(PhysicalMachineAnalysisEvidenceErrorV1::Truncated)?;
        let result = self
            .bytes
            .get(self.position..end)
            .ok_or(PhysicalMachineAnalysisEvidenceErrorV1::Truncated)?;
        self.position = end;
        Ok(result)
    }

    fn expect(&mut self, expected: &[u8]) -> Result<(), PhysicalMachineAnalysisEvidenceErrorV1> {
        if self.take(expected.len())? != expected {
            return Err(PhysicalMachineAnalysisEvidenceErrorV1::DomainMismatch);
        }
        Ok(())
    }

    fn u16(&mut self) -> Result<u16, PhysicalMachineAnalysisEvidenceErrorV1> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().map_err(
            |_| PhysicalMachineAnalysisEvidenceErrorV1::Truncated,
        )?))
    }

    fn u32(&mut self) -> Result<u32, PhysicalMachineAnalysisEvidenceErrorV1> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().map_err(
            |_| PhysicalMachineAnalysisEvidenceErrorV1::Truncated,
        )?))
    }

    fn finish(self) -> Result<(), PhysicalMachineAnalysisEvidenceErrorV1> {
        if self.position != self.bytes.len() {
            return Err(PhysicalMachineAnalysisEvidenceErrorV1::TrailingBytes);
        }
        Ok(())
    }
}

fn domain_hash(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(bytes);
    digest.finalize().into()
}
