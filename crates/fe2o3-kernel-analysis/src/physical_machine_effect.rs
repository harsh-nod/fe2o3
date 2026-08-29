//! Payload-derived machine effects for bounded gfx942 entry-point sets.
//!
//! The native worker derives this record from finalized HSACO with LLVM
//! Object/MC APIs. This crate binds and validates the record but grants no
//! load or launch authority.

use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

pub const PHYSICAL_MACHINE_EFFECT_REQUEST_DOMAIN_V1: &[u8] =
    b"FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-REQUEST/V1\0";
pub const PHYSICAL_MACHINE_EFFECT_EVIDENCE_DOMAIN_V1: &[u8] =
    b"FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-EVIDENCE/V1\0";
const REQUEST_IDENTITY_DOMAIN: &[u8] =
    b"FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-REQUEST-IDENTITY/V1\0";
const EVIDENCE_IDENTITY_DOMAIN: &[u8] =
    b"FE2O3/GFX942-PHYSICAL-MACHINE-EFFECT-EVIDENCE-IDENTITY/V1\0";

pub const PHYSICAL_MACHINE_EFFECT_SCHEMA_VERSION_V1: u16 = 1;
pub const MAX_PHYSICAL_MACHINE_EFFECT_PAYLOAD_BYTES_V1: usize = 64 * 1024 * 1024;
pub const MAX_PHYSICAL_MACHINE_EFFECT_REQUEST_BYTES_V1: usize =
    MAX_PHYSICAL_MACHINE_EFFECT_PAYLOAD_BYTES_V1 + 1024;
pub const MAX_PHYSICAL_MACHINE_EFFECT_EVIDENCE_BYTES_V1: usize = 8 * 1024 * 1024;
pub const MAX_PHYSICAL_MACHINE_EFFECT_FUNCTIONS_V1: usize = 64;
pub const MAX_PHYSICAL_MACHINE_EFFECT_EFFECTS_V1: usize = 16_384;
const MAX_ENTRIES: usize = 2;
const MAX_EDGES: usize = 256;
const MAX_SYMBOL_BYTES: usize = 256;

macro_rules! digest_identity {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; 32]);

        impl $name {
            pub const fn from_sha256_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            pub const fn as_bytes(self) -> [u8; 32] {
                self.0
            }
        }
    };
}

digest_identity!(PhysicalMachineAnalyzerIdentityV1);
digest_identity!(PhysicalMachineToolchainIdentityV1);
digest_identity!(PhysicalMachineDescriptorIdentityV1);
digest_identity!(PhysicalMachineExecutionChallengeV1);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PhysicalMachinePayloadIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl PhysicalMachinePayloadIdentityV1 {
    pub fn calculate(bytes: &[u8]) -> Self {
        Self {
            sha256: Sha256::digest(bytes).into(),
            byte_len: bytes.len() as u64,
        }
    }

    pub const fn from_parts(sha256: [u8; 32], byte_len: u64) -> Self {
        Self { sha256, byte_len }
    }

    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    pub fn matches(self, bytes: &[u8]) -> bool {
        self == Self::calculate(bytes)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PhysicalMachineEffectBudgetV1 {
    max_global_addresses: u32,
    max_global_reads: u32,
    max_global_writes: u32,
    max_returns: u32,
    max_direct_calls: u32,
}

impl PhysicalMachineEffectBudgetV1 {
    pub const fn new(
        max_global_addresses: u32,
        max_global_reads: u32,
        max_global_writes: u32,
        max_returns: u32,
        max_direct_calls: u32,
    ) -> Self {
        Self {
            max_global_addresses,
            max_global_reads,
            max_global_writes,
            max_returns,
            max_direct_calls,
        }
    }

    pub const fn max_global_addresses(self) -> u32 {
        self.max_global_addresses
    }

    pub const fn max_global_reads(self) -> u32 {
        self.max_global_reads
    }

    pub const fn max_global_writes(self) -> u32 {
        self.max_global_writes
    }

    pub const fn max_returns(self) -> u32 {
        self.max_returns
    }

    pub const fn max_direct_calls(self) -> u32 {
        self.max_direct_calls
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PhysicalMachineEffectEntryRequestV1 {
    symbol: String,
    budget: PhysicalMachineEffectBudgetV1,
}

impl PhysicalMachineEffectEntryRequestV1 {
    pub fn new(
        symbol: impl Into<String>,
        budget: PhysicalMachineEffectBudgetV1,
    ) -> Result<Self, PhysicalMachineEffectRequestErrorV1> {
        let symbol = symbol.into();
        if !valid_symbol(&symbol) {
            return Err(PhysicalMachineEffectRequestErrorV1::InvalidEntrySymbol {
                byte_len: symbol.len(),
            });
        }
        Ok(Self { symbol, budget })
    }

    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    pub const fn budget(&self) -> PhysicalMachineEffectBudgetV1 {
        self.budget
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PhysicalMachineEffectRequestIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl PhysicalMachineEffectRequestIdentityV1 {
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhysicalMachineEffectRequestV1 {
    execution_challenge: PhysicalMachineExecutionChallengeV1,
    analyzer_identity: PhysicalMachineAnalyzerIdentityV1,
    toolchain_identity: PhysicalMachineToolchainIdentityV1,
    payload_identity: PhysicalMachinePayloadIdentityV1,
    entries: Vec<PhysicalMachineEffectEntryRequestV1>,
    payload: Vec<u8>,
    canonical_bytes: Vec<u8>,
}

impl PhysicalMachineEffectRequestV1 {
    /// Reopens one exact canonical request retained at the authenticated worker boundary.
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, PhysicalMachineEffectRequestErrorV1> {
        decode_request(bytes)
    }

    pub fn new(
        execution_challenge: PhysicalMachineExecutionChallengeV1,
        analyzer_identity: PhysicalMachineAnalyzerIdentityV1,
        toolchain_identity: PhysicalMachineToolchainIdentityV1,
        payload: Vec<u8>,
        mut entries: Vec<PhysicalMachineEffectEntryRequestV1>,
    ) -> Result<Self, PhysicalMachineEffectRequestErrorV1> {
        if execution_challenge.0 == [0; 32] {
            return Err(PhysicalMachineEffectRequestErrorV1::ZeroIdentity(
                "execution challenge",
            ));
        }
        if analyzer_identity.0 == [0; 32] {
            return Err(PhysicalMachineEffectRequestErrorV1::ZeroIdentity(
                "analyzer",
            ));
        }
        if toolchain_identity.0 == [0; 32] {
            return Err(PhysicalMachineEffectRequestErrorV1::ZeroIdentity(
                "toolchain",
            ));
        }
        if payload.is_empty() || payload.len() > MAX_PHYSICAL_MACHINE_EFFECT_PAYLOAD_BYTES_V1 {
            return Err(PhysicalMachineEffectRequestErrorV1::PayloadSize {
                actual: payload.len(),
                maximum: MAX_PHYSICAL_MACHINE_EFFECT_PAYLOAD_BYTES_V1,
            });
        }
        if entries.is_empty() || entries.len() > MAX_ENTRIES {
            return Err(PhysicalMachineEffectRequestErrorV1::EntryCount(
                entries.len(),
            ));
        }
        entries.sort();
        if entries
            .windows(2)
            .any(|pair| pair[0].symbol == pair[1].symbol)
        {
            return Err(PhysicalMachineEffectRequestErrorV1::DuplicateEntry);
        }

        let payload_identity = PhysicalMachinePayloadIdentityV1::calculate(&payload);
        let mut result = Self {
            execution_challenge,
            analyzer_identity,
            toolchain_identity,
            payload_identity,
            entries,
            payload,
            canonical_bytes: Vec::new(),
        };
        result.canonical_bytes = encode_request(&result)?;
        Ok(result)
    }

    pub const fn execution_challenge(&self) -> PhysicalMachineExecutionChallengeV1 {
        self.execution_challenge
    }

    pub const fn analyzer_identity(&self) -> PhysicalMachineAnalyzerIdentityV1 {
        self.analyzer_identity
    }

    pub const fn toolchain_identity(&self) -> PhysicalMachineToolchainIdentityV1 {
        self.toolchain_identity
    }

    pub const fn payload_identity(&self) -> PhysicalMachinePayloadIdentityV1 {
        self.payload_identity
    }

    pub fn entries(&self) -> &[PhysicalMachineEffectEntryRequestV1] {
        &self.entries
    }

    pub fn exact_payload_bytes(&self) -> &[u8] {
        &self.payload
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub fn identity(&self) -> PhysicalMachineEffectRequestIdentityV1 {
        PhysicalMachineEffectRequestIdentityV1 {
            sha256: domain_hash(REQUEST_IDENTITY_DOMAIN, &self.canonical_bytes),
            byte_len: self.canonical_bytes.len() as u64,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PhysicalMachineTargetV1 {
    Gfx942XnackMinusCov6,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PhysicalMachineEntryEvidenceV1 {
    symbol: String,
    descriptor_identity: PhysicalMachineDescriptorIdentityV1,
    code_offset: u64,
    code_size: u64,
}

impl PhysicalMachineEntryEvidenceV1 {
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    pub const fn descriptor_identity(&self) -> PhysicalMachineDescriptorIdentityV1 {
        self.descriptor_identity
    }

    pub const fn code_offset(&self) -> u64 {
        self.code_offset
    }

    pub const fn code_size(&self) -> u64 {
        self.code_size
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PhysicalMachineFunctionEvidenceV1 {
    symbol: String,
    code_offset: u64,
    code_size: u64,
    direct_callees: Vec<String>,
}

impl PhysicalMachineFunctionEvidenceV1 {
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    pub const fn code_offset(&self) -> u64 {
        self.code_offset
    }

    pub const fn code_size(&self) -> u64 {
        self.code_size
    }

    pub fn direct_callees(&self) -> &[String] {
        &self.direct_callees
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PhysicalMachineEffectKindV1 {
    GlobalAddress,
    GlobalRead,
    GlobalWrite,
    Return,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PhysicalMachineEffectV1 {
    entry_symbol: String,
    function_symbol: String,
    instruction_offset: u64,
    kind: PhysicalMachineEffectKindV1,
    byte_width: u16,
}

impl PhysicalMachineEffectV1 {
    pub fn entry_symbol(&self) -> &str {
        &self.entry_symbol
    }

    pub fn function_symbol(&self) -> &str {
        &self.function_symbol
    }

    pub const fn instruction_offset(&self) -> u64 {
        self.instruction_offset
    }

    pub const fn kind(&self) -> PhysicalMachineEffectKindV1 {
        self.kind
    }

    pub const fn byte_width(&self) -> u16 {
        self.byte_width
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PhysicalMachineEffectAnalysisBasisV1 {
    FinalizedHsacoViaMeasuredLlvmObjectMc,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PhysicalMachineEffectEvidenceIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl PhysicalMachineEffectEvidenceIdentityV1 {
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

/// Canonical static machine-site evidence for one exact finalized HSACO.
///
/// Effects name reachable instruction sites and bounded access widths. They do
/// not report concrete runtime addresses or dynamic execution counts and do not
/// prove OOB absence, race freedom, compiler refinement, or source properties.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhysicalMachineEffectEvidenceV1 {
    execution_challenge: PhysicalMachineExecutionChallengeV1,
    request_identity: PhysicalMachineEffectRequestIdentityV1,
    payload_identity: PhysicalMachinePayloadIdentityV1,
    analyzer_identity: PhysicalMachineAnalyzerIdentityV1,
    toolchain_identity: PhysicalMachineToolchainIdentityV1,
    entries: Vec<PhysicalMachineEntryEvidenceV1>,
    functions: Vec<PhysicalMachineFunctionEvidenceV1>,
    effects: Vec<PhysicalMachineEffectV1>,
    canonical_bytes: Vec<u8>,
}

impl PhysicalMachineEffectEvidenceV1 {
    pub fn decode_canonical_for(
        request: &PhysicalMachineEffectRequestV1,
        bytes: &[u8],
    ) -> Result<Self, PhysicalMachineEffectEvidenceErrorV1> {
        decode_evidence_for(request, bytes)
    }

    pub const fn schema_version(&self) -> u16 {
        PHYSICAL_MACHINE_EFFECT_SCHEMA_VERSION_V1
    }

    pub const fn target(&self) -> PhysicalMachineTargetV1 {
        PhysicalMachineTargetV1::Gfx942XnackMinusCov6
    }

    pub const fn analysis_basis(&self) -> PhysicalMachineEffectAnalysisBasisV1 {
        PhysicalMachineEffectAnalysisBasisV1::FinalizedHsacoViaMeasuredLlvmObjectMc
    }

    pub const fn request_identity(&self) -> PhysicalMachineEffectRequestIdentityV1 {
        self.request_identity
    }

    pub const fn execution_challenge(&self) -> PhysicalMachineExecutionChallengeV1 {
        self.execution_challenge
    }

    pub const fn payload_identity(&self) -> PhysicalMachinePayloadIdentityV1 {
        self.payload_identity
    }

    pub const fn analyzer_identity(&self) -> PhysicalMachineAnalyzerIdentityV1 {
        self.analyzer_identity
    }

    pub const fn toolchain_identity(&self) -> PhysicalMachineToolchainIdentityV1 {
        self.toolchain_identity
    }

    pub fn entry_points(&self) -> &[PhysicalMachineEntryEvidenceV1] {
        &self.entries
    }

    pub fn functions(&self) -> &[PhysicalMachineFunctionEvidenceV1] {
        &self.functions
    }

    pub fn effects(&self) -> &[PhysicalMachineEffectV1] {
        &self.effects
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub fn identity(&self) -> PhysicalMachineEffectEvidenceIdentityV1 {
        PhysicalMachineEffectEvidenceIdentityV1 {
            sha256: domain_hash(EVIDENCE_IDENTITY_DOMAIN, &self.canonical_bytes),
            byte_len: self.canonical_bytes.len() as u64,
        }
    }

    pub const fn is_derived_from_exact_payload(&self) -> bool {
        true
    }

    pub const fn authenticates_analyzer(&self) -> bool {
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

fn encode_request(
    request: &PhysicalMachineEffectRequestV1,
) -> Result<Vec<u8>, PhysicalMachineEffectRequestErrorV1> {
    let mut output = Vec::with_capacity(
        PHYSICAL_MACHINE_EFFECT_REQUEST_DOMAIN_V1.len() + request.payload.len() + 256,
    );
    output.extend_from_slice(PHYSICAL_MACHINE_EFFECT_REQUEST_DOMAIN_V1);
    push_u32(&mut output, 0);
    push_u16(&mut output, PHYSICAL_MACHINE_EFFECT_SCHEMA_VERSION_V1);
    output.extend_from_slice(&request.execution_challenge.0);
    output.extend_from_slice(&request.analyzer_identity.0);
    output.extend_from_slice(&request.toolchain_identity.0);
    output.extend_from_slice(&request.payload_identity.sha256);
    push_u64(&mut output, request.payload_identity.byte_len);
    push_u16(&mut output, request.entries.len() as u16);
    for entry in &request.entries {
        push_text(&mut output, &entry.symbol);
        push_u32(&mut output, entry.budget.max_global_addresses);
        push_u32(&mut output, entry.budget.max_global_reads);
        push_u32(&mut output, entry.budget.max_global_writes);
        push_u32(&mut output, entry.budget.max_returns);
        push_u32(&mut output, entry.budget.max_direct_calls);
    }
    output.extend_from_slice(&request.payload);
    let length = u32::try_from(output.len())
        .map_err(|_| PhysicalMachineEffectRequestErrorV1::RecordTooLarge)?;
    let offset = PHYSICAL_MACHINE_EFFECT_REQUEST_DOMAIN_V1.len();
    output[offset..offset + 4].copy_from_slice(&length.to_le_bytes());
    Ok(output)
}

fn decode_request(
    bytes: &[u8],
) -> Result<PhysicalMachineEffectRequestV1, PhysicalMachineEffectRequestErrorV1> {
    if bytes.len() > MAX_PHYSICAL_MACHINE_EFFECT_REQUEST_BYTES_V1 {
        return Err(PhysicalMachineEffectRequestErrorV1::RecordTooLarge);
    }
    let mut input = RequestReader::new(bytes);
    input.expect(PHYSICAL_MACHINE_EFFECT_REQUEST_DOMAIN_V1)?;
    if input.u32()? as usize != bytes.len() {
        return Err(PhysicalMachineEffectRequestErrorV1::LengthMismatch);
    }
    if input.u16()? != PHYSICAL_MACHINE_EFFECT_SCHEMA_VERSION_V1 {
        return Err(PhysicalMachineEffectRequestErrorV1::UnsupportedVersion);
    }
    let execution_challenge = PhysicalMachineExecutionChallengeV1(input.array()?);
    let analyzer_identity = PhysicalMachineAnalyzerIdentityV1(input.array()?);
    let toolchain_identity = PhysicalMachineToolchainIdentityV1(input.array()?);
    let encoded_payload_identity = PhysicalMachinePayloadIdentityV1 {
        sha256: input.array()?,
        byte_len: input.u64()?,
    };
    let entry_count = input.u16()? as usize;
    if entry_count == 0 || entry_count > MAX_ENTRIES {
        return Err(PhysicalMachineEffectRequestErrorV1::EntryCount(entry_count));
    }
    let mut entries = Vec::with_capacity(entry_count);
    for _ in 0..entry_count {
        entries.push(PhysicalMachineEffectEntryRequestV1::new(
            input.symbol()?,
            PhysicalMachineEffectBudgetV1::new(
                input.u32()?,
                input.u32()?,
                input.u32()?,
                input.u32()?,
                input.u32()?,
            ),
        )?);
    }
    let payload_len = usize::try_from(encoded_payload_identity.byte_len)
        .map_err(|_| PhysicalMachineEffectRequestErrorV1::RecordTooLarge)?;
    if payload_len == 0 || payload_len > MAX_PHYSICAL_MACHINE_EFFECT_PAYLOAD_BYTES_V1 {
        return Err(PhysicalMachineEffectRequestErrorV1::PayloadSize {
            actual: payload_len,
            maximum: MAX_PHYSICAL_MACHINE_EFFECT_PAYLOAD_BYTES_V1,
        });
    }
    if input.remaining() != payload_len {
        return Err(PhysicalMachineEffectRequestErrorV1::LengthMismatch);
    }
    let payload = input.take(payload_len)?.to_vec();
    input.finish()?;

    let request = PhysicalMachineEffectRequestV1::new(
        execution_challenge,
        analyzer_identity,
        toolchain_identity,
        payload,
        entries,
    )?;
    if request.payload_identity != encoded_payload_identity {
        return Err(PhysicalMachineEffectRequestErrorV1::PayloadIdentityMismatch);
    }
    if request.canonical_bytes != bytes {
        return Err(PhysicalMachineEffectRequestErrorV1::NonCanonicalEncoding);
    }
    Ok(request)
}

fn decode_evidence_for(
    request: &PhysicalMachineEffectRequestV1,
    bytes: &[u8],
) -> Result<PhysicalMachineEffectEvidenceV1, PhysicalMachineEffectEvidenceErrorV1> {
    if bytes.len() > MAX_PHYSICAL_MACHINE_EFFECT_EVIDENCE_BYTES_V1 {
        return Err(PhysicalMachineEffectEvidenceErrorV1::RecordTooLarge);
    }
    let mut input = Reader::new(bytes);
    input.expect(PHYSICAL_MACHINE_EFFECT_EVIDENCE_DOMAIN_V1)?;
    if input.u32()? as usize != bytes.len() {
        return Err(PhysicalMachineEffectEvidenceErrorV1::LengthMismatch);
    }
    if input.u16()? != PHYSICAL_MACHINE_EFFECT_SCHEMA_VERSION_V1 {
        return Err(PhysicalMachineEffectEvidenceErrorV1::UnsupportedVersion);
    }
    let execution_challenge = PhysicalMachineExecutionChallengeV1(input.array()?);
    if execution_challenge != request.execution_challenge {
        return Err(PhysicalMachineEffectEvidenceErrorV1::ExecutionChallengeMismatch);
    }
    let request_identity = PhysicalMachineEffectRequestIdentityV1 {
        sha256: input.array()?,
        byte_len: input.u64()?,
    };
    if request_identity != request.identity() {
        return Err(PhysicalMachineEffectEvidenceErrorV1::RequestIdentityMismatch);
    }
    let payload_identity = PhysicalMachinePayloadIdentityV1 {
        sha256: input.array()?,
        byte_len: input.u64()?,
    };
    if payload_identity != request.payload_identity {
        return Err(PhysicalMachineEffectEvidenceErrorV1::PayloadIdentityMismatch);
    }
    let analyzer_identity = PhysicalMachineAnalyzerIdentityV1(input.array()?);
    let toolchain_identity = PhysicalMachineToolchainIdentityV1(input.array()?);
    if analyzer_identity != request.analyzer_identity {
        return Err(PhysicalMachineEffectEvidenceErrorV1::AnalyzerIdentityMismatch);
    }
    if toolchain_identity != request.toolchain_identity {
        return Err(PhysicalMachineEffectEvidenceErrorV1::ToolchainIdentityMismatch);
    }
    if input.u16()? != 1 {
        return Err(PhysicalMachineEffectEvidenceErrorV1::TargetMismatch);
    }

    let entry_count = input.u16()? as usize;
    if entry_count != request.entries.len() {
        return Err(PhysicalMachineEffectEvidenceErrorV1::EntrySetMismatch);
    }
    let mut entries = Vec::with_capacity(entry_count);
    for expected in &request.entries {
        let symbol = input.text()?;
        if symbol != expected.symbol {
            return Err(PhysicalMachineEffectEvidenceErrorV1::EntrySetMismatch);
        }
        let descriptor_identity = PhysicalMachineDescriptorIdentityV1(input.array()?);
        if descriptor_identity.0 == [0; 32] {
            return Err(PhysicalMachineEffectEvidenceErrorV1::ZeroDescriptorIdentity);
        }
        let code_offset = input.u64()?;
        let code_size = input.u64()?;
        checked_code_range_end(code_offset, code_size, payload_identity.byte_len)?;
        entries.push(PhysicalMachineEntryEvidenceV1 {
            symbol,
            descriptor_identity,
            code_offset,
            code_size,
        });
    }

    let function_count = input.u32()? as usize;
    if function_count == 0 || function_count > MAX_PHYSICAL_MACHINE_EFFECT_FUNCTIONS_V1 {
        return Err(PhysicalMachineEffectEvidenceErrorV1::FunctionCount);
    }
    let mut functions = Vec::with_capacity(function_count);
    let mut edge_count = 0usize;
    for _ in 0..function_count {
        let symbol = input.text()?;
        let code_offset = input.u64()?;
        let code_size = input.u64()?;
        checked_code_range_end(code_offset, code_size, payload_identity.byte_len)?;
        let count = input.u16()? as usize;
        edge_count = edge_count.saturating_add(count);
        if edge_count > MAX_EDGES {
            return Err(PhysicalMachineEffectEvidenceErrorV1::CallEdgeCount);
        }
        let mut direct_callees = Vec::with_capacity(count);
        for _ in 0..count {
            direct_callees.push(input.text()?);
        }
        if !strictly_sorted(&direct_callees) {
            return Err(PhysicalMachineEffectEvidenceErrorV1::NonCanonicalOrder);
        }
        functions.push(PhysicalMachineFunctionEvidenceV1 {
            symbol,
            code_offset,
            code_size,
            direct_callees,
        });
    }
    if !functions
        .windows(2)
        .all(|pair| pair[0].symbol < pair[1].symbol)
    {
        return Err(PhysicalMachineEffectEvidenceErrorV1::NonCanonicalOrder);
    }
    let closures = validate_graph(request, &entries, &functions)?;

    let effect_count = input.u32()? as usize;
    if effect_count > MAX_PHYSICAL_MACHINE_EFFECT_EFFECTS_V1 {
        return Err(PhysicalMachineEffectEvidenceErrorV1::EffectCount);
    }
    let mut effects = Vec::with_capacity(effect_count);
    for _ in 0..effect_count {
        let entry_symbol = input.text()?;
        let function_symbol = input.text()?;
        let instruction_offset = input.u64()?;
        let kind = match input.u8()? {
            1 => PhysicalMachineEffectKindV1::GlobalAddress,
            2 => PhysicalMachineEffectKindV1::GlobalRead,
            3 => PhysicalMachineEffectKindV1::GlobalWrite,
            4 => PhysicalMachineEffectKindV1::Return,
            _ => return Err(PhysicalMachineEffectEvidenceErrorV1::UnknownEffectKind),
        };
        let byte_width = input.u16()?;
        if matches!(kind, PhysicalMachineEffectKindV1::Return) != (byte_width == 0) {
            return Err(PhysicalMachineEffectEvidenceErrorV1::InvalidEffectWidth);
        }
        effects.push(PhysicalMachineEffectV1 {
            entry_symbol,
            function_symbol,
            instruction_offset,
            kind,
            byte_width,
        });
    }
    input.finish()?;
    if !strictly_sorted(&effects) {
        return Err(PhysicalMachineEffectEvidenceErrorV1::NonCanonicalOrder);
    }
    validate_effects(request, &functions, &closures, &effects)?;

    Ok(PhysicalMachineEffectEvidenceV1 {
        execution_challenge,
        request_identity,
        payload_identity,
        analyzer_identity,
        toolchain_identity,
        entries,
        functions,
        effects,
        canonical_bytes: bytes.to_vec(),
    })
}

fn validate_graph(
    request: &PhysicalMachineEffectRequestV1,
    entries: &[PhysicalMachineEntryEvidenceV1],
    functions: &[PhysicalMachineFunctionEvidenceV1],
) -> Result<BTreeMap<String, BTreeSet<String>>, PhysicalMachineEffectEvidenceErrorV1> {
    let by_name = functions
        .iter()
        .map(|function| (function.symbol.as_str(), function))
        .collect::<BTreeMap<_, _>>();
    if entries
        .iter()
        .any(|entry| !by_name.contains_key(entry.symbol.as_str()))
    {
        return Err(PhysicalMachineEffectEvidenceErrorV1::EntryFunctionMissing);
    }
    for function in functions {
        if function
            .direct_callees
            .iter()
            .any(|callee| !by_name.contains_key(callee.as_str()))
        {
            return Err(PhysicalMachineEffectEvidenceErrorV1::OpenCallGraph);
        }
    }

    let mut all_reachable = BTreeSet::new();
    let mut closures = BTreeMap::new();
    for entry in &request.entries {
        let mut reachable = BTreeSet::new();
        let mut pending = vec![entry.symbol.as_str()];
        let mut calls = 0usize;
        while let Some(symbol) = pending.pop() {
            if reachable.insert(symbol.to_string()) {
                calls += by_name[symbol].direct_callees.len();
                pending.extend(by_name[symbol].direct_callees.iter().map(String::as_str));
            }
        }
        if calls > entry.budget.max_direct_calls as usize {
            return Err(PhysicalMachineEffectEvidenceErrorV1::EffectExpansion);
        }
        all_reachable.extend(reachable.iter().cloned());
        closures.insert(entry.symbol.clone(), reachable);
    }
    if all_reachable.len() != functions.len() {
        return Err(PhysicalMachineEffectEvidenceErrorV1::UnreachableFunction);
    }
    Ok(closures)
}

fn validate_effects(
    request: &PhysicalMachineEffectRequestV1,
    functions: &[PhysicalMachineFunctionEvidenceV1],
    closures: &BTreeMap<String, BTreeSet<String>>,
    effects: &[PhysicalMachineEffectV1],
) -> Result<(), PhysicalMachineEffectEvidenceErrorV1> {
    let by_name = functions
        .iter()
        .map(|function| (function.symbol.as_str(), function))
        .collect::<BTreeMap<_, _>>();
    for entry in &request.entries {
        let mut counts = [0usize; 4];
        for effect in effects
            .iter()
            .filter(|effect| effect.entry_symbol == entry.symbol)
        {
            let function = by_name
                .get(effect.function_symbol.as_str())
                .ok_or(PhysicalMachineEffectEvidenceErrorV1::UnknownEffectFunction)?;
            let code_end = checked_code_range_end(
                function.code_offset,
                function.code_size,
                request.payload_identity.byte_len,
            )?;
            if !closures[&entry.symbol].contains(&effect.function_symbol)
                || effect.instruction_offset < function.code_offset
                || effect.instruction_offset >= code_end
            {
                return Err(PhysicalMachineEffectEvidenceErrorV1::EffectOutsideClosure);
            }
            counts[effect_kind_index(effect.kind)] += 1;
        }
        let maxima = [
            entry.budget.max_global_addresses,
            entry.budget.max_global_reads,
            entry.budget.max_global_writes,
            entry.budget.max_returns,
        ];
        if counts
            .into_iter()
            .zip(maxima)
            .any(|(actual, maximum)| actual > maximum as usize)
        {
            return Err(PhysicalMachineEffectEvidenceErrorV1::EffectExpansion);
        }
    }
    if effects.iter().any(|effect| {
        !request
            .entries
            .iter()
            .any(|entry| entry.symbol == effect.entry_symbol)
    }) {
        return Err(PhysicalMachineEffectEvidenceErrorV1::UnknownEffectEntry);
    }
    Ok(())
}

fn checked_code_range_end(
    code_offset: u64,
    code_size: u64,
    payload_byte_len: u64,
) -> Result<u64, PhysicalMachineEffectEvidenceErrorV1> {
    if code_size == 0 {
        return Err(PhysicalMachineEffectEvidenceErrorV1::InvalidFunctionRange);
    }
    let code_end = code_offset
        .checked_add(code_size)
        .ok_or(PhysicalMachineEffectEvidenceErrorV1::InvalidFunctionRange)?;
    if code_end > payload_byte_len {
        return Err(PhysicalMachineEffectEvidenceErrorV1::InvalidFunctionRange);
    }
    Ok(code_end)
}

const fn effect_kind_index(kind: PhysicalMachineEffectKindV1) -> usize {
    match kind {
        PhysicalMachineEffectKindV1::GlobalAddress => 0,
        PhysicalMachineEffectKindV1::GlobalRead => 1,
        PhysicalMachineEffectKindV1::GlobalWrite => 2,
        PhysicalMachineEffectKindV1::Return => 3,
    }
}

fn domain_hash(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(bytes);
    digest.finalize().into()
}

fn strictly_sorted<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn valid_symbol(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= MAX_SYMBOL_BYTES
        && (bytes[0].is_ascii_alphabetic() || matches!(bytes[0], b'_' | b'.' | b'$'))
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'$'))
}

fn push_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_text(output: &mut Vec<u8>, value: &str) {
    push_u16(output, value.len() as u16);
    output.extend_from_slice(value.as_bytes());
}

struct RequestReader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> RequestReader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], PhysicalMachineEffectRequestErrorV1> {
        let end = self
            .position
            .checked_add(count)
            .ok_or(PhysicalMachineEffectRequestErrorV1::Truncated)?;
        let result = self
            .bytes
            .get(self.position..end)
            .ok_or(PhysicalMachineEffectRequestErrorV1::Truncated)?;
        self.position = end;
        Ok(result)
    }

    fn expect(&mut self, expected: &[u8]) -> Result<(), PhysicalMachineEffectRequestErrorV1> {
        if self.take(expected.len())? != expected {
            return Err(PhysicalMachineEffectRequestErrorV1::InvalidDomain);
        }
        Ok(())
    }

    fn u16(&mut self) -> Result<u16, PhysicalMachineEffectRequestErrorV1> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, PhysicalMachineEffectRequestErrorV1> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, PhysicalMachineEffectRequestErrorV1> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], PhysicalMachineEffectRequestErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| PhysicalMachineEffectRequestErrorV1::Truncated)
    }

    fn symbol(&mut self) -> Result<String, PhysicalMachineEffectRequestErrorV1> {
        let length = self.u16()? as usize;
        if length == 0 || length > MAX_SYMBOL_BYTES {
            return Err(PhysicalMachineEffectRequestErrorV1::InvalidEntrySymbol {
                byte_len: length,
            });
        }
        let bytes = self.take(length)?;
        let value = std::str::from_utf8(bytes).map_err(|_| {
            PhysicalMachineEffectRequestErrorV1::InvalidEntrySymbol { byte_len: length }
        })?;
        if !valid_symbol(value) {
            return Err(PhysicalMachineEffectRequestErrorV1::InvalidEntrySymbol {
                byte_len: length,
            });
        }
        Ok(value.to_owned())
    }

    const fn remaining(&self) -> usize {
        self.bytes.len() - self.position
    }

    fn finish(self) -> Result<(), PhysicalMachineEffectRequestErrorV1> {
        if self.position != self.bytes.len() {
            return Err(PhysicalMachineEffectRequestErrorV1::TrailingBytes);
        }
        Ok(())
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], PhysicalMachineEffectEvidenceErrorV1> {
        let end = self
            .position
            .checked_add(count)
            .ok_or(PhysicalMachineEffectEvidenceErrorV1::Truncated)?;
        let result = self
            .bytes
            .get(self.position..end)
            .ok_or(PhysicalMachineEffectEvidenceErrorV1::Truncated)?;
        self.position = end;
        Ok(result)
    }

    fn expect(&mut self, expected: &[u8]) -> Result<(), PhysicalMachineEffectEvidenceErrorV1> {
        if self.take(expected.len())? != expected {
            return Err(PhysicalMachineEffectEvidenceErrorV1::InvalidDomain);
        }
        Ok(())
    }

    fn u8(&mut self) -> Result<u8, PhysicalMachineEffectEvidenceErrorV1> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, PhysicalMachineEffectEvidenceErrorV1> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, PhysicalMachineEffectEvidenceErrorV1> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, PhysicalMachineEffectEvidenceErrorV1> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], PhysicalMachineEffectEvidenceErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| PhysicalMachineEffectEvidenceErrorV1::Truncated)
    }

    fn text(&mut self) -> Result<String, PhysicalMachineEffectEvidenceErrorV1> {
        let length = self.u16()? as usize;
        if length == 0 || length > MAX_SYMBOL_BYTES {
            return Err(PhysicalMachineEffectEvidenceErrorV1::InvalidSymbol);
        }
        let value = std::str::from_utf8(self.take(length)?)
            .map_err(|_| PhysicalMachineEffectEvidenceErrorV1::InvalidUtf8)?
            .to_string();
        if !valid_symbol(&value) {
            return Err(PhysicalMachineEffectEvidenceErrorV1::InvalidSymbol);
        }
        Ok(value)
    }

    fn finish(self) -> Result<(), PhysicalMachineEffectEvidenceErrorV1> {
        if self.position != self.bytes.len() {
            return Err(PhysicalMachineEffectEvidenceErrorV1::TrailingBytes);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PhysicalMachineEffectRequestErrorV1 {
    InvalidDomain,
    Truncated,
    TrailingBytes,
    LengthMismatch,
    UnsupportedVersion,
    PayloadIdentityMismatch,
    NonCanonicalEncoding,
    ZeroIdentity(&'static str),
    PayloadSize { actual: usize, maximum: usize },
    EntryCount(usize),
    InvalidEntrySymbol { byte_len: usize },
    DuplicateEntry,
    RecordTooLarge,
}

impl fmt::Display for PhysicalMachineEffectRequestErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid physical machine-effect request: {self:?}"
        )
    }
}

impl Error for PhysicalMachineEffectRequestErrorV1 {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PhysicalMachineEffectEvidenceErrorV1 {
    RecordTooLarge,
    InvalidDomain,
    Truncated,
    TrailingBytes,
    InvalidUtf8,
    InvalidSymbol,
    LengthMismatch,
    UnsupportedVersion,
    ExecutionChallengeMismatch,
    RequestIdentityMismatch,
    PayloadIdentityMismatch,
    AnalyzerIdentityMismatch,
    ToolchainIdentityMismatch,
    TargetMismatch,
    EntrySetMismatch,
    ZeroDescriptorIdentity,
    FunctionCount,
    CallEdgeCount,
    EffectCount,
    InvalidFunctionRange,
    NonCanonicalOrder,
    EntryFunctionMissing,
    OpenCallGraph,
    UnreachableFunction,
    UnknownEffectKind,
    InvalidEffectWidth,
    UnknownEffectEntry,
    UnknownEffectFunction,
    EffectOutsideClosure,
    EffectExpansion,
}

impl fmt::Display for PhysicalMachineEffectEvidenceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid physical machine-effect evidence: {self:?}"
        )
    }
}

impl Error for PhysicalMachineEffectEvidenceErrorV1 {}
