//! Canonical executable binding consumed by the scalar Worker V3 Verus proof.
//!
//! This record is deliberately inert. Its constructor validates the independently reviewed
//! scalar gfx942 machine profile and retains the complete authenticated analyzer evidence and
//! receipt bytes, but it does not authenticate their producer. The production authority performs
//! that authentication while retaining the move-only machine execution receipt.

use std::fmt::Write as _;
use std::{error::Error, fmt};

use sha2::{Digest as _, Sha256};

use crate::Digest;

const EXECUTABLE_BINDING_DOMAIN_V1: &[u8] = b"fe2o3-scalar-gemm-worker-v3-executable-binding-v1\0";

pub const SCALAR_GEMM_WORKER_V3_FINALIZED_HSACO_SHA256_V1: [u8; 32] = [
    0xf4, 0x15, 0xc0, 0x40, 0x60, 0x6b, 0x56, 0xcd, 0xbc, 0x14, 0x67, 0xab, 0x34, 0xb7, 0xd2, 0xda,
    0x7d, 0x99, 0xb5, 0x7b, 0x99, 0x97, 0xfe, 0xf9, 0xe4, 0x20, 0x0a, 0xc0, 0x3b, 0x36, 0x5a, 0x75,
];
pub const SCALAR_GEMM_WORKER_V3_FINALIZED_HSACO_BYTES_V1: u64 = 10_008;
pub const SCALAR_GEMM_WORKER_V3_LOGICAL_DESCRIPTOR_IDENTITY_V1: [u8; 32] = [
    0x78, 0x9a, 0xde, 0xdf, 0xdc, 0x3b, 0xe1, 0xfb, 0x60, 0x51, 0x8d, 0xd2, 0xc7, 0x46, 0x0c, 0x3e,
    0xf8, 0xe6, 0xb9, 0x00, 0x52, 0x7d, 0x1b, 0xcb, 0x22, 0x89, 0xba, 0xa1, 0xe0, 0x14, 0x69, 0x3e,
];
pub const SCALAR_GEMM_WORKER_V3_RAW_DESCRIPTOR_SHA256_V1: [u8; 32] = [
    0x01, 0xab, 0x64, 0x23, 0x92, 0xfb, 0xc7, 0x35, 0xe5, 0x2e, 0x9b, 0xa0, 0x0b, 0xbc, 0xa5, 0x41,
    0xd1, 0xb8, 0xa1, 0x19, 0xc1, 0xd1, 0x7d, 0x71, 0xe6, 0xe1, 0x42, 0x59, 0xa9, 0xa1, 0x52, 0xf6,
];
pub const SCALAR_GEMM_WORKER_V3_CODE_OFFSET_V1: u64 = 0x1b00;
pub const SCALAR_GEMM_WORKER_V3_CODE_SIZE_V1: u64 = 0x0ab0;
pub const SCALAR_GEMM_WORKER_V3_MACHINE_EFFECT_COUNT_V1: usize = 19;
pub const MAX_SCALAR_GEMM_WORKER_V3_MACHINE_BINDING_BLOB_BYTES_V1: usize = 128 * 1024;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ScalarGemmWorkerV3MeasuredIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl ScalarGemmWorkerV3MeasuredIdentityV1 {
    pub const fn new(sha256: [u8; 32], byte_len: u64) -> Self {
        Self { sha256, byte_len }
    }

    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    fn is_valid(self) -> bool {
        self.sha256 != [0; 32] && self.byte_len != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum ScalarGemmWorkerV3MachineEffectKindV1 {
    GlobalAddress = 1,
    GlobalRead = 2,
    GlobalWrite = 3,
    Return = 4,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ScalarGemmWorkerV3MachineEffectSiteV1 {
    instruction_offset: u64,
    kind: ScalarGemmWorkerV3MachineEffectKindV1,
    byte_width: u16,
}

impl ScalarGemmWorkerV3MachineEffectSiteV1 {
    pub const fn new(
        instruction_offset: u64,
        kind: ScalarGemmWorkerV3MachineEffectKindV1,
        byte_width: u16,
    ) -> Self {
        Self {
            instruction_offset,
            kind,
            byte_width,
        }
    }

    pub const fn instruction_offset(self) -> u64 {
        self.instruction_offset
    }

    pub const fn kind(self) -> ScalarGemmWorkerV3MachineEffectKindV1 {
        self.kind
    }

    pub const fn byte_width(self) -> u16 {
        self.byte_width
    }
}

pub const SCALAR_GEMM_WORKER_V3_MACHINE_EFFECTS_V1: [ScalarGemmWorkerV3MachineEffectSiteV1;
    SCALAR_GEMM_WORKER_V3_MACHINE_EFFECT_COUNT_V1] = [
    ScalarGemmWorkerV3MachineEffectSiteV1::new(
        0x1b0c,
        ScalarGemmWorkerV3MachineEffectKindV1::GlobalAddress,
        8,
    ),
    ScalarGemmWorkerV3MachineEffectSiteV1::new(
        0x1b0c,
        ScalarGemmWorkerV3MachineEffectKindV1::GlobalRead,
        8,
    ),
    ScalarGemmWorkerV3MachineEffectSiteV1::new(
        0x1b14,
        ScalarGemmWorkerV3MachineEffectKindV1::GlobalAddress,
        8,
    ),
    ScalarGemmWorkerV3MachineEffectSiteV1::new(
        0x1b14,
        ScalarGemmWorkerV3MachineEffectKindV1::GlobalRead,
        8,
    ),
    ScalarGemmWorkerV3MachineEffectSiteV1::new(
        0x1b1c,
        ScalarGemmWorkerV3MachineEffectKindV1::GlobalAddress,
        8,
    ),
    ScalarGemmWorkerV3MachineEffectSiteV1::new(
        0x1b1c,
        ScalarGemmWorkerV3MachineEffectKindV1::GlobalRead,
        8,
    ),
    ScalarGemmWorkerV3MachineEffectSiteV1::new(
        0x1b24,
        ScalarGemmWorkerV3MachineEffectKindV1::GlobalAddress,
        8,
    ),
    ScalarGemmWorkerV3MachineEffectSiteV1::new(
        0x1b24,
        ScalarGemmWorkerV3MachineEffectKindV1::GlobalRead,
        4,
    ),
    ScalarGemmWorkerV3MachineEffectSiteV1::new(
        0x1b2c,
        ScalarGemmWorkerV3MachineEffectKindV1::GlobalAddress,
        8,
    ),
    ScalarGemmWorkerV3MachineEffectSiteV1::new(
        0x1b2c,
        ScalarGemmWorkerV3MachineEffectKindV1::GlobalRead,
        4,
    ),
    ScalarGemmWorkerV3MachineEffectSiteV1::new(
        0x1b34,
        ScalarGemmWorkerV3MachineEffectKindV1::GlobalAddress,
        8,
    ),
    ScalarGemmWorkerV3MachineEffectSiteV1::new(
        0x1b34,
        ScalarGemmWorkerV3MachineEffectKindV1::GlobalRead,
        4,
    ),
    ScalarGemmWorkerV3MachineEffectSiteV1::new(
        0x2470,
        ScalarGemmWorkerV3MachineEffectKindV1::GlobalAddress,
        8,
    ),
    ScalarGemmWorkerV3MachineEffectSiteV1::new(
        0x2470,
        ScalarGemmWorkerV3MachineEffectKindV1::GlobalRead,
        4,
    ),
    ScalarGemmWorkerV3MachineEffectSiteV1::new(
        0x2484,
        ScalarGemmWorkerV3MachineEffectKindV1::GlobalAddress,
        8,
    ),
    ScalarGemmWorkerV3MachineEffectSiteV1::new(
        0x2484,
        ScalarGemmWorkerV3MachineEffectKindV1::GlobalRead,
        4,
    ),
    ScalarGemmWorkerV3MachineEffectSiteV1::new(
        0x25a0,
        ScalarGemmWorkerV3MachineEffectKindV1::GlobalAddress,
        8,
    ),
    ScalarGemmWorkerV3MachineEffectSiteV1::new(
        0x25a0,
        ScalarGemmWorkerV3MachineEffectKindV1::GlobalWrite,
        4,
    ),
    ScalarGemmWorkerV3MachineEffectSiteV1::new(
        0x25ac,
        ScalarGemmWorkerV3MachineEffectKindV1::Return,
        0,
    ),
];

/// Caller-supplied fields used to construct one inert executable binding.
///
/// Production callers must obtain these values from the retained authenticated machine worker.
/// Public construction is not authentication and never grants artifact or runtime authority.
#[derive(Debug)]
pub struct ScalarGemmWorkerV3ExecutableBindingComponentsV1 {
    pub finalized_hsaco: ScalarGemmWorkerV3MeasuredIdentityV1,
    pub logical_descriptor_identity: [u8; 32],
    pub raw_descriptor_identity: [u8; 32],
    pub machine_execution_challenge: [u8; 32],
    pub analyzer_identity: [u8; 32],
    pub toolchain_identity: [u8; 32],
    pub machine_request_identity: ScalarGemmWorkerV3MeasuredIdentityV1,
    pub machine_evidence_identity: ScalarGemmWorkerV3MeasuredIdentityV1,
    pub authenticated_receipt_identity: ScalarGemmWorkerV3MeasuredIdentityV1,
    pub worker_executable_identity: ScalarGemmWorkerV3MeasuredIdentityV1,
    pub machine_runtime_closure_identity: ScalarGemmWorkerV3MeasuredIdentityV1,
    pub machine_runtime_mapping_identity: ScalarGemmWorkerV3MeasuredIdentityV1,
    pub verus_runtime_closure_identity: [u8; 32],
    pub entry_code_offset: u64,
    pub entry_code_size: u64,
    pub effects: Vec<ScalarGemmWorkerV3MachineEffectSiteV1>,
    pub canonical_machine_request: Vec<u8>,
    pub canonical_machine_evidence: Vec<u8>,
    pub canonical_authenticated_receipt: Vec<u8>,
}

/// Lossless, exact-profile executable data embedded in retained Verus input.
#[derive(Debug)]
pub struct ScalarGemmWorkerV3ExecutableBindingV1 {
    components: ScalarGemmWorkerV3ExecutableBindingComponentsV1,
    machine_request_content_sha256: [u8; 32],
    machine_evidence_content_sha256: [u8; 32],
    authenticated_receipt_content_sha256: [u8; 32],
    canonical_bytes: Vec<u8>,
    identity: Digest,
}

impl ScalarGemmWorkerV3ExecutableBindingV1 {
    pub fn new(
        components: ScalarGemmWorkerV3ExecutableBindingComponentsV1,
    ) -> Result<Self, ScalarGemmWorkerV3ExecutableBindingErrorV1> {
        validate_components(&components)?;
        let machine_request_content_sha256 =
            Sha256::digest(&components.canonical_machine_request).into();
        let machine_evidence_content_sha256 =
            Sha256::digest(&components.canonical_machine_evidence).into();
        let authenticated_receipt_content_sha256 =
            Sha256::digest(&components.canonical_authenticated_receipt).into();
        let canonical_bytes = encode_canonical(
            &components,
            machine_request_content_sha256,
            machine_evidence_content_sha256,
            authenticated_receipt_content_sha256,
        );
        let mut digest = Sha256::new();
        digest.update(EXECUTABLE_BINDING_DOMAIN_V1);
        digest.update(&canonical_bytes);
        let identity = Digest::from_bytes(digest.finalize().into());
        Ok(Self {
            components,
            machine_request_content_sha256,
            machine_evidence_content_sha256,
            authenticated_receipt_content_sha256,
            canonical_bytes,
            identity,
        })
    }

    pub const fn finalized_hsaco(&self) -> ScalarGemmWorkerV3MeasuredIdentityV1 {
        self.components.finalized_hsaco
    }

    pub const fn logical_descriptor_identity(&self) -> [u8; 32] {
        self.components.logical_descriptor_identity
    }

    pub const fn raw_descriptor_identity(&self) -> [u8; 32] {
        self.components.raw_descriptor_identity
    }

    pub const fn machine_execution_challenge(&self) -> [u8; 32] {
        self.components.machine_execution_challenge
    }

    pub const fn analyzer_identity(&self) -> [u8; 32] {
        self.components.analyzer_identity
    }

    pub const fn toolchain_identity(&self) -> [u8; 32] {
        self.components.toolchain_identity
    }

    pub const fn machine_request_identity(&self) -> ScalarGemmWorkerV3MeasuredIdentityV1 {
        self.components.machine_request_identity
    }

    pub const fn machine_evidence_identity(&self) -> ScalarGemmWorkerV3MeasuredIdentityV1 {
        self.components.machine_evidence_identity
    }

    pub const fn authenticated_receipt_identity(&self) -> ScalarGemmWorkerV3MeasuredIdentityV1 {
        self.components.authenticated_receipt_identity
    }

    pub const fn worker_executable_identity(&self) -> ScalarGemmWorkerV3MeasuredIdentityV1 {
        self.components.worker_executable_identity
    }

    pub const fn machine_runtime_closure_identity(&self) -> ScalarGemmWorkerV3MeasuredIdentityV1 {
        self.components.machine_runtime_closure_identity
    }

    pub const fn machine_runtime_mapping_identity(&self) -> ScalarGemmWorkerV3MeasuredIdentityV1 {
        self.components.machine_runtime_mapping_identity
    }

    pub const fn verus_runtime_closure_identity(&self) -> [u8; 32] {
        self.components.verus_runtime_closure_identity
    }

    pub const fn entry_code_offset(&self) -> u64 {
        self.components.entry_code_offset
    }

    pub const fn entry_code_size(&self) -> u64 {
        self.components.entry_code_size
    }

    pub fn effects(&self) -> &[ScalarGemmWorkerV3MachineEffectSiteV1] {
        &self.components.effects
    }

    pub fn canonical_machine_evidence(&self) -> &[u8] {
        &self.components.canonical_machine_evidence
    }

    pub fn canonical_machine_request(&self) -> &[u8] {
        &self.components.canonical_machine_request
    }

    pub fn canonical_authenticated_receipt(&self) -> &[u8] {
        &self.components.canonical_authenticated_receipt
    }

    pub const fn machine_evidence_content_sha256(&self) -> [u8; 32] {
        self.machine_evidence_content_sha256
    }

    pub const fn machine_request_content_sha256(&self) -> [u8; 32] {
        self.machine_request_content_sha256
    }

    pub const fn authenticated_receipt_content_sha256(&self) -> [u8; 32] {
        self.authenticated_receipt_content_sha256
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn identity(&self) -> Digest {
        self.identity
    }

    pub const fn establishes_exact_reviewed_machine_profile(&self) -> bool {
        true
    }

    pub const fn authenticates_machine_execution(&self) -> bool {
        false
    }

    pub const fn grants_artifact_or_runtime_authority(&self) -> bool {
        false
    }

    pub(crate) fn append_generated_verus_source(
        &self,
        output: &mut Vec<u8>,
        worker_v3_challenge: [u8; 32],
        worker_v3_lineage_identity: [u8; 32],
        generated_host_contract_identity: [u8; 32],
    ) -> Result<(), ScalarGemmWorkerV3ExecutableBindingErrorV1> {
        let mut source = String::with_capacity(
            self.components.canonical_machine_request.len() * 6
                + self.components.canonical_machine_evidence.len() * 6
                + self.components.canonical_authenticated_receipt.len() * 6
                + 16 * 1024,
        );
        source.push_str("\npub mod scalar_gemm_worker_v3_executable_binding_generated_v1 {\n\n");
        source.push_str("use vstd::prelude::*;\n\n");
        source.push_str("verus! {\n\n");
        push_digest(&mut source, "FE2O3_ZERO_SHA256_V1", [0; 32])?;
        push_digest(
            &mut source,
            "FE2O3_BOUND_WORKER_V3_CHALLENGE_V1",
            worker_v3_challenge,
        )?;
        push_digest(
            &mut source,
            "FE2O3_BOUND_WORKER_V3_LINEAGE_IDENTITY_V1",
            worker_v3_lineage_identity,
        )?;
        push_digest(
            &mut source,
            "FE2O3_BOUND_GENERATED_HOST_CONTRACT_IDENTITY_V1",
            generated_host_contract_identity,
        )?;
        push_digest(
            &mut source,
            "FE2O3_FINALIZED_HSACO_SHA256_V1",
            self.finalized_hsaco().sha256(),
        )?;
        writeln!(
            source,
            "pub const FE2O3_FINALIZED_HSACO_BYTE_LEN_V1: u64 = {};",
            self.finalized_hsaco().byte_len()
        )?;
        push_digest(
            &mut source,
            "FE2O3_LOGICAL_DESCRIPTOR_IDENTITY_V1",
            self.logical_descriptor_identity(),
        )?;
        push_digest(
            &mut source,
            "FE2O3_RAW_AMDHSA_DESCRIPTOR_SHA256_V1",
            self.raw_descriptor_identity(),
        )?;
        push_digest(
            &mut source,
            "FE2O3_MACHINE_EXECUTION_CHALLENGE_V1",
            self.machine_execution_challenge(),
        )?;
        push_digest(
            &mut source,
            "FE2O3_MACHINE_ANALYZER_IDENTITY_V1",
            self.analyzer_identity(),
        )?;
        push_digest(
            &mut source,
            "FE2O3_MACHINE_TOOLCHAIN_IDENTITY_V1",
            self.toolchain_identity(),
        )?;
        for (name, identity) in [
            ("MACHINE_REQUEST", self.machine_request_identity()),
            ("MACHINE_EVIDENCE", self.machine_evidence_identity()),
            (
                "AUTHENTICATED_MACHINE_RECEIPT",
                self.authenticated_receipt_identity(),
            ),
            (
                "MACHINE_WORKER_EXECUTABLE",
                self.worker_executable_identity(),
            ),
            (
                "MACHINE_RUNTIME_CLOSURE",
                self.machine_runtime_closure_identity(),
            ),
            (
                "MACHINE_RUNTIME_MAPPING",
                self.machine_runtime_mapping_identity(),
            ),
        ] {
            push_digest(
                &mut source,
                &format!("FE2O3_{name}_SHA256_V1"),
                identity.sha256(),
            )?;
            writeln!(
                source,
                "pub const FE2O3_{name}_BYTE_LEN_V1: u64 = {};",
                identity.byte_len()
            )?;
        }
        push_digest(
            &mut source,
            "FE2O3_VERUS_RUNTIME_CLOSURE_IDENTITY_V1",
            self.verus_runtime_closure_identity(),
        )?;
        push_digest(
            &mut source,
            "FE2O3_MACHINE_REQUEST_CONTENT_SHA256_V1",
            self.machine_request_content_sha256,
        )?;
        push_digest(
            &mut source,
            "FE2O3_MACHINE_EVIDENCE_CONTENT_SHA256_V1",
            self.machine_evidence_content_sha256,
        )?;
        push_digest(
            &mut source,
            "FE2O3_AUTHENTICATED_MACHINE_RECEIPT_CONTENT_SHA256_V1",
            self.authenticated_receipt_content_sha256,
        )?;
        writeln!(
            source,
            "pub const FE2O3_MACHINE_ENTRY_CODE_OFFSET_V1: u64 = 0x{:x};",
            self.entry_code_offset()
        )?;
        writeln!(
            source,
            "pub const FE2O3_MACHINE_ENTRY_CODE_SIZE_V1: u64 = 0x{:x};",
            self.entry_code_size()
        )?;
        push_effect_arrays(&mut source, &self.components.effects, false)?;
        push_byte_array(
            &mut source,
            "FE2O3_CANONICAL_MACHINE_REQUEST_V1",
            &self.components.canonical_machine_request,
        )?;
        push_byte_array(
            &mut source,
            "FE2O3_CANONICAL_MACHINE_EVIDENCE_V1",
            &self.components.canonical_machine_evidence,
        )?;
        push_byte_array(
            &mut source,
            "FE2O3_CANONICAL_AUTHENTICATED_MACHINE_RECEIPT_V1",
            &self.components.canonical_authenticated_receipt,
        )?;

        push_digest(
            &mut source,
            "FE2O3_REVIEWED_FINALIZED_HSACO_SHA256_V1",
            SCALAR_GEMM_WORKER_V3_FINALIZED_HSACO_SHA256_V1,
        )?;
        push_digest(
            &mut source,
            "FE2O3_REVIEWED_LOGICAL_DESCRIPTOR_IDENTITY_V1",
            SCALAR_GEMM_WORKER_V3_LOGICAL_DESCRIPTOR_IDENTITY_V1,
        )?;
        push_digest(
            &mut source,
            "FE2O3_REVIEWED_RAW_AMDHSA_DESCRIPTOR_SHA256_V1",
            SCALAR_GEMM_WORKER_V3_RAW_DESCRIPTOR_SHA256_V1,
        )?;
        push_effect_arrays(&mut source, &SCALAR_GEMM_WORKER_V3_MACHINE_EFFECTS_V1, true)?;

        source.push_str(
            "pub proof fn generated_scalar_gemm_worker_v3_executable_binding_matches_reviewed_profile_v1()\n",
        );
        source.push_str("    ensures\n");
        source.push_str(
            "        FE2O3_BOUND_WORKER_V3_CHALLENGE_V1 != FE2O3_ZERO_SHA256_V1,\n        FE2O3_BOUND_WORKER_V3_LINEAGE_IDENTITY_V1 != FE2O3_ZERO_SHA256_V1,\n        FE2O3_BOUND_GENERATED_HOST_CONTRACT_IDENTITY_V1 != FE2O3_ZERO_SHA256_V1,\n",
        );
        source.push_str(
            "        FE2O3_FINALIZED_HSACO_SHA256_V1 == FE2O3_REVIEWED_FINALIZED_HSACO_SHA256_V1,\n",
        );
        writeln!(
            source,
            "        FE2O3_FINALIZED_HSACO_BYTE_LEN_V1 == {SCALAR_GEMM_WORKER_V3_FINALIZED_HSACO_BYTES_V1},"
        )?;
        source.push_str(
            "        FE2O3_LOGICAL_DESCRIPTOR_IDENTITY_V1 == FE2O3_REVIEWED_LOGICAL_DESCRIPTOR_IDENTITY_V1,\n        FE2O3_RAW_AMDHSA_DESCRIPTOR_SHA256_V1 == FE2O3_REVIEWED_RAW_AMDHSA_DESCRIPTOR_SHA256_V1,\n",
        );
        writeln!(
            source,
            "        FE2O3_MACHINE_ENTRY_CODE_OFFSET_V1 == 0x{SCALAR_GEMM_WORKER_V3_CODE_OFFSET_V1:x},"
        )?;
        writeln!(
            source,
            "        FE2O3_MACHINE_ENTRY_CODE_SIZE_V1 == 0x{SCALAR_GEMM_WORKER_V3_CODE_SIZE_V1:x},"
        )?;
        source.push_str(
            "        FE2O3_MACHINE_EFFECT_OFFSETS_V1 == FE2O3_REVIEWED_MACHINE_EFFECT_OFFSETS_V1,\n        FE2O3_MACHINE_EFFECT_KINDS_V1 == FE2O3_REVIEWED_MACHINE_EFFECT_KINDS_V1,\n        FE2O3_MACHINE_EFFECT_WIDTHS_V1 == FE2O3_REVIEWED_MACHINE_EFFECT_WIDTHS_V1,\n",
        );
        for name in [
            "FE2O3_MACHINE_EXECUTION_CHALLENGE_V1",
            "FE2O3_MACHINE_ANALYZER_IDENTITY_V1",
            "FE2O3_MACHINE_TOOLCHAIN_IDENTITY_V1",
            "FE2O3_MACHINE_REQUEST_SHA256_V1",
            "FE2O3_MACHINE_EVIDENCE_SHA256_V1",
            "FE2O3_AUTHENTICATED_MACHINE_RECEIPT_SHA256_V1",
            "FE2O3_MACHINE_WORKER_EXECUTABLE_SHA256_V1",
            "FE2O3_MACHINE_RUNTIME_CLOSURE_SHA256_V1",
            "FE2O3_MACHINE_RUNTIME_MAPPING_SHA256_V1",
            "FE2O3_VERUS_RUNTIME_CLOSURE_IDENTITY_V1",
            "FE2O3_MACHINE_REQUEST_CONTENT_SHA256_V1",
            "FE2O3_MACHINE_EVIDENCE_CONTENT_SHA256_V1",
            "FE2O3_AUTHENTICATED_MACHINE_RECEIPT_CONTENT_SHA256_V1",
        ] {
            writeln!(source, "        {name} != FE2O3_ZERO_SHA256_V1,")?;
        }
        source.push_str(
            "        FE2O3_CANONICAL_MACHINE_REQUEST_V1@.len() > 0,\n        FE2O3_CANONICAL_MACHINE_EVIDENCE_V1@.len() > 0,\n        FE2O3_CANONICAL_AUTHENTICATED_MACHINE_RECEIPT_V1@.len() > 0,\n",
        );
        source.push_str("{\n");
        source.push_str(
            "    assert(FE2O3_FINALIZED_HSACO_SHA256_V1 == FE2O3_REVIEWED_FINALIZED_HSACO_SHA256_V1);\n    assert(FE2O3_LOGICAL_DESCRIPTOR_IDENTITY_V1 == FE2O3_REVIEWED_LOGICAL_DESCRIPTOR_IDENTITY_V1);\n    assert(FE2O3_RAW_AMDHSA_DESCRIPTOR_SHA256_V1 == FE2O3_REVIEWED_RAW_AMDHSA_DESCRIPTOR_SHA256_V1);\n    assert(FE2O3_MACHINE_EFFECT_OFFSETS_V1 == FE2O3_REVIEWED_MACHINE_EFFECT_OFFSETS_V1);\n    assert(FE2O3_MACHINE_EFFECT_KINDS_V1 == FE2O3_REVIEWED_MACHINE_EFFECT_KINDS_V1);\n    assert(FE2O3_MACHINE_EFFECT_WIDTHS_V1 == FE2O3_REVIEWED_MACHINE_EFFECT_WIDTHS_V1);\n",
        );
        source.push_str("}\n\n} // verus!\n\n");
        source.push_str("} // mod scalar_gemm_worker_v3_executable_binding_generated_v1\n");
        output.extend_from_slice(source.as_bytes());
        Ok(())
    }
}

fn validate_components(
    components: &ScalarGemmWorkerV3ExecutableBindingComponentsV1,
) -> Result<(), ScalarGemmWorkerV3ExecutableBindingErrorV1> {
    if components.finalized_hsaco
        != ScalarGemmWorkerV3MeasuredIdentityV1::new(
            SCALAR_GEMM_WORKER_V3_FINALIZED_HSACO_SHA256_V1,
            SCALAR_GEMM_WORKER_V3_FINALIZED_HSACO_BYTES_V1,
        )
    {
        return Err(ScalarGemmWorkerV3ExecutableBindingErrorV1::FinalizedHsacoSubstitution);
    }
    if components.logical_descriptor_identity
        != SCALAR_GEMM_WORKER_V3_LOGICAL_DESCRIPTOR_IDENTITY_V1
    {
        return Err(ScalarGemmWorkerV3ExecutableBindingErrorV1::LogicalDescriptorSubstitution);
    }
    if components.raw_descriptor_identity != SCALAR_GEMM_WORKER_V3_RAW_DESCRIPTOR_SHA256_V1 {
        return Err(ScalarGemmWorkerV3ExecutableBindingErrorV1::RawDescriptorSubstitution);
    }
    if components.entry_code_offset != SCALAR_GEMM_WORKER_V3_CODE_OFFSET_V1
        || components.entry_code_size != SCALAR_GEMM_WORKER_V3_CODE_SIZE_V1
    {
        return Err(ScalarGemmWorkerV3ExecutableBindingErrorV1::EntryRangeSubstitution);
    }
    if components.effects.as_slice() != SCALAR_GEMM_WORKER_V3_MACHINE_EFFECTS_V1 {
        return Err(ScalarGemmWorkerV3ExecutableBindingErrorV1::MachineEffectSubstitution);
    }
    for (field, identity) in [
        (
            "machine execution challenge",
            components.machine_execution_challenge,
        ),
        ("machine analyzer", components.analyzer_identity),
        ("machine toolchain", components.toolchain_identity),
        (
            "Verus runtime closure",
            components.verus_runtime_closure_identity,
        ),
    ] {
        if identity == [0; 32] {
            return Err(ScalarGemmWorkerV3ExecutableBindingErrorV1::ZeroIdentity(
                field,
            ));
        }
    }
    for (field, identity) in [
        ("machine request", components.machine_request_identity),
        ("machine evidence", components.machine_evidence_identity),
        (
            "authenticated machine receipt",
            components.authenticated_receipt_identity,
        ),
        (
            "machine worker executable",
            components.worker_executable_identity,
        ),
        (
            "machine runtime closure",
            components.machine_runtime_closure_identity,
        ),
        (
            "machine runtime mapping",
            components.machine_runtime_mapping_identity,
        ),
    ] {
        if !identity.is_valid() {
            return Err(ScalarGemmWorkerV3ExecutableBindingErrorV1::InvalidMeasuredIdentity(field));
        }
    }
    validate_blob(
        "machine request",
        &components.canonical_machine_request,
        components.machine_request_identity,
    )?;
    validate_blob(
        "machine evidence",
        &components.canonical_machine_evidence,
        components.machine_evidence_identity,
    )?;
    validate_blob(
        "authenticated machine receipt",
        &components.canonical_authenticated_receipt,
        components.authenticated_receipt_identity,
    )?;
    Ok(())
}

fn validate_blob(
    field: &'static str,
    bytes: &[u8],
    identity: ScalarGemmWorkerV3MeasuredIdentityV1,
) -> Result<(), ScalarGemmWorkerV3ExecutableBindingErrorV1> {
    if bytes.is_empty() || bytes.len() > MAX_SCALAR_GEMM_WORKER_V3_MACHINE_BINDING_BLOB_BYTES_V1 {
        return Err(ScalarGemmWorkerV3ExecutableBindingErrorV1::InvalidBlobSize(
            field,
        ));
    }
    if u64::try_from(bytes.len()).ok() != Some(identity.byte_len()) {
        return Err(ScalarGemmWorkerV3ExecutableBindingErrorV1::BlobIdentityLengthMismatch(field));
    }
    Ok(())
}

fn encode_canonical(
    components: &ScalarGemmWorkerV3ExecutableBindingComponentsV1,
    request_content_sha256: [u8; 32],
    evidence_content_sha256: [u8; 32],
    receipt_content_sha256: [u8; 32],
) -> Vec<u8> {
    let mut output = Vec::with_capacity(
        components.canonical_machine_request.len()
            + components.canonical_machine_evidence.len()
            + components.canonical_authenticated_receipt.len()
            + 1024,
    );
    output.extend_from_slice(EXECUTABLE_BINDING_DOMAIN_V1);
    put_measured(&mut output, components.finalized_hsaco);
    for digest in [
        components.logical_descriptor_identity,
        components.raw_descriptor_identity,
        components.machine_execution_challenge,
        components.analyzer_identity,
        components.toolchain_identity,
    ] {
        output.extend_from_slice(&digest);
    }
    for identity in [
        components.machine_request_identity,
        components.machine_evidence_identity,
        components.authenticated_receipt_identity,
        components.worker_executable_identity,
        components.machine_runtime_closure_identity,
        components.machine_runtime_mapping_identity,
    ] {
        put_measured(&mut output, identity);
    }
    output.extend_from_slice(&components.verus_runtime_closure_identity);
    output.extend_from_slice(&components.entry_code_offset.to_le_bytes());
    output.extend_from_slice(&components.entry_code_size.to_le_bytes());
    output.extend_from_slice(&(components.effects.len() as u32).to_le_bytes());
    for effect in &components.effects {
        output.extend_from_slice(&effect.instruction_offset.to_le_bytes());
        output.push(effect.kind as u8);
        output.extend_from_slice(&effect.byte_width.to_le_bytes());
    }
    output.extend_from_slice(&request_content_sha256);
    put_blob(&mut output, &components.canonical_machine_request);
    output.extend_from_slice(&evidence_content_sha256);
    put_blob(&mut output, &components.canonical_machine_evidence);
    output.extend_from_slice(&receipt_content_sha256);
    put_blob(&mut output, &components.canonical_authenticated_receipt);
    output
}

fn put_measured(output: &mut Vec<u8>, identity: ScalarGemmWorkerV3MeasuredIdentityV1) {
    output.extend_from_slice(&identity.sha256);
    output.extend_from_slice(&identity.byte_len.to_le_bytes());
}

fn put_blob(output: &mut Vec<u8>, bytes: &[u8]) {
    output.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    output.extend_from_slice(bytes);
}

fn push_digest(
    output: &mut String,
    name: &str,
    digest: [u8; 32],
) -> Result<(), ScalarGemmWorkerV3ExecutableBindingErrorV1> {
    write!(output, "pub const {name}: [u8; 32] = [")?;
    for (index, byte) in digest.into_iter().enumerate() {
        if index != 0 {
            output.push_str(", ");
        }
        write!(output, "0x{byte:02x}")?;
    }
    output.push_str("];\n");
    Ok(())
}

fn push_byte_array(
    output: &mut String,
    name: &str,
    bytes: &[u8],
) -> Result<(), ScalarGemmWorkerV3ExecutableBindingErrorV1> {
    writeln!(output, "pub const {name}: [u8; {}] = [", bytes.len())?;
    for chunk in bytes.chunks(24) {
        output.push_str("    ");
        for (index, byte) in chunk.iter().enumerate() {
            if index != 0 {
                output.push_str(", ");
            }
            write!(output, "{byte}")?;
        }
        output.push_str(",\n");
    }
    output.push_str("];\n");
    Ok(())
}

fn push_effect_arrays(
    output: &mut String,
    effects: &[ScalarGemmWorkerV3MachineEffectSiteV1],
    reviewed: bool,
) -> Result<(), ScalarGemmWorkerV3ExecutableBindingErrorV1> {
    let prefix = if reviewed { "REVIEWED_" } else { "" };
    write!(
        output,
        "pub const FE2O3_{prefix}MACHINE_EFFECT_OFFSETS_V1: [u64; {}] = [",
        effects.len()
    )?;
    for (index, effect) in effects.iter().enumerate() {
        if index != 0 {
            output.push_str(", ");
        }
        write!(output, "0x{:x}", effect.instruction_offset)?;
    }
    output.push_str("];\n");
    write!(
        output,
        "pub const FE2O3_{prefix}MACHINE_EFFECT_KINDS_V1: [u8; {}] = [",
        effects.len()
    )?;
    for (index, effect) in effects.iter().enumerate() {
        if index != 0 {
            output.push_str(", ");
        }
        write!(output, "{}", effect.kind as u8)?;
    }
    output.push_str("];\n");
    write!(
        output,
        "pub const FE2O3_{prefix}MACHINE_EFFECT_WIDTHS_V1: [u16; {}] = [",
        effects.len()
    )?;
    for (index, effect) in effects.iter().enumerate() {
        if index != 0 {
            output.push_str(", ");
        }
        write!(output, "{}", effect.byte_width)?;
    }
    output.push_str("];\n");
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ScalarGemmWorkerV3ExecutableBindingErrorV1 {
    FinalizedHsacoSubstitution,
    LogicalDescriptorSubstitution,
    RawDescriptorSubstitution,
    EntryRangeSubstitution,
    MachineEffectSubstitution,
    ZeroIdentity(&'static str),
    InvalidMeasuredIdentity(&'static str),
    InvalidBlobSize(&'static str),
    BlobIdentityLengthMismatch(&'static str),
    Formatting,
}

impl From<fmt::Error> for ScalarGemmWorkerV3ExecutableBindingErrorV1 {
    fn from(_: fmt::Error) -> Self {
        Self::Formatting
    }
}

impl fmt::Display for ScalarGemmWorkerV3ExecutableBindingErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid scalar Worker V3 executable binding: {self:?}"
        )
    }
}

impl Error for ScalarGemmWorkerV3ExecutableBindingErrorV1 {}

#[cfg(test)]
mod tests {
    use super::*;

    fn measured(seed: u8, byte_len: usize) -> ScalarGemmWorkerV3MeasuredIdentityV1 {
        ScalarGemmWorkerV3MeasuredIdentityV1::new([seed; 32], byte_len as u64)
    }

    fn components() -> ScalarGemmWorkerV3ExecutableBindingComponentsV1 {
        let request = vec![1, 2];
        let evidence = vec![3, 4, 5];
        let receipt = vec![6, 7, 8, 9];
        ScalarGemmWorkerV3ExecutableBindingComponentsV1 {
            finalized_hsaco: ScalarGemmWorkerV3MeasuredIdentityV1::new(
                SCALAR_GEMM_WORKER_V3_FINALIZED_HSACO_SHA256_V1,
                SCALAR_GEMM_WORKER_V3_FINALIZED_HSACO_BYTES_V1,
            ),
            logical_descriptor_identity: SCALAR_GEMM_WORKER_V3_LOGICAL_DESCRIPTOR_IDENTITY_V1,
            raw_descriptor_identity: SCALAR_GEMM_WORKER_V3_RAW_DESCRIPTOR_SHA256_V1,
            machine_execution_challenge: [10; 32],
            analyzer_identity: [11; 32],
            toolchain_identity: [12; 32],
            machine_request_identity: measured(13, request.len()),
            machine_evidence_identity: measured(14, evidence.len()),
            authenticated_receipt_identity: measured(15, receipt.len()),
            worker_executable_identity: measured(16, 100),
            machine_runtime_closure_identity: measured(17, 200),
            machine_runtime_mapping_identity: measured(18, 300),
            verus_runtime_closure_identity: [19; 32],
            entry_code_offset: SCALAR_GEMM_WORKER_V3_CODE_OFFSET_V1,
            entry_code_size: SCALAR_GEMM_WORKER_V3_CODE_SIZE_V1,
            effects: SCALAR_GEMM_WORKER_V3_MACHINE_EFFECTS_V1.to_vec(),
            canonical_machine_request: request,
            canonical_machine_evidence: evidence,
            canonical_authenticated_receipt: receipt,
        }
    }

    fn assert_error(
        mutate: impl FnOnce(&mut ScalarGemmWorkerV3ExecutableBindingComponentsV1),
        expected: ScalarGemmWorkerV3ExecutableBindingErrorV1,
    ) {
        let mut value = components();
        mutate(&mut value);
        assert_eq!(
            ScalarGemmWorkerV3ExecutableBindingV1::new(value).unwrap_err(),
            expected
        );
    }

    fn assert_identity_changes(
        mutate: impl FnOnce(&mut ScalarGemmWorkerV3ExecutableBindingComponentsV1),
    ) {
        let exact = ScalarGemmWorkerV3ExecutableBindingV1::new(components()).unwrap();
        let mut substituted = components();
        mutate(&mut substituted);
        let substituted = ScalarGemmWorkerV3ExecutableBindingV1::new(substituted).unwrap();
        assert_ne!(substituted.identity(), exact.identity());
        assert_ne!(substituted.canonical_bytes(), exact.canonical_bytes());
    }

    #[test]
    fn exact_reviewed_scalar_machine_profile_is_losslessly_retained() {
        let binding = ScalarGemmWorkerV3ExecutableBindingV1::new(components()).unwrap();
        assert_eq!(
            binding.finalized_hsaco().sha256(),
            SCALAR_GEMM_WORKER_V3_FINALIZED_HSACO_SHA256_V1
        );
        assert_eq!(
            binding.logical_descriptor_identity(),
            SCALAR_GEMM_WORKER_V3_LOGICAL_DESCRIPTOR_IDENTITY_V1
        );
        assert_eq!(
            binding.raw_descriptor_identity(),
            SCALAR_GEMM_WORKER_V3_RAW_DESCRIPTOR_SHA256_V1
        );
        assert_eq!(binding.effects(), SCALAR_GEMM_WORKER_V3_MACHINE_EFFECTS_V1);
        assert_eq!(binding.canonical_machine_request(), [1, 2]);
        assert_eq!(binding.canonical_machine_evidence(), [3, 4, 5]);
        assert_eq!(binding.canonical_authenticated_receipt(), [6, 7, 8, 9]);
        assert!(binding.establishes_exact_reviewed_machine_profile());
        assert!(!binding.authenticates_machine_execution());
        assert!(!binding.grants_artifact_or_runtime_authority());
    }

    #[test]
    fn every_fixed_executable_profile_axis_fails_closed() {
        assert_error(
            |value| value.finalized_hsaco = measured(99, 10_008),
            ScalarGemmWorkerV3ExecutableBindingErrorV1::FinalizedHsacoSubstitution,
        );
        assert_error(
            |value| {
                value.finalized_hsaco = ScalarGemmWorkerV3MeasuredIdentityV1::new(
                    SCALAR_GEMM_WORKER_V3_FINALIZED_HSACO_SHA256_V1,
                    10_009,
                )
            },
            ScalarGemmWorkerV3ExecutableBindingErrorV1::FinalizedHsacoSubstitution,
        );
        assert_error(
            |value| value.logical_descriptor_identity[0] ^= 1,
            ScalarGemmWorkerV3ExecutableBindingErrorV1::LogicalDescriptorSubstitution,
        );
        assert_error(
            |value| value.raw_descriptor_identity[0] ^= 1,
            ScalarGemmWorkerV3ExecutableBindingErrorV1::RawDescriptorSubstitution,
        );
        assert_error(
            |value| value.entry_code_offset += 4,
            ScalarGemmWorkerV3ExecutableBindingErrorV1::EntryRangeSubstitution,
        );
        assert_error(
            |value| value.entry_code_size -= 4,
            ScalarGemmWorkerV3ExecutableBindingErrorV1::EntryRangeSubstitution,
        );
        assert_error(
            |value| value.effects[0].instruction_offset += 4,
            ScalarGemmWorkerV3ExecutableBindingErrorV1::MachineEffectSubstitution,
        );
        assert_error(
            |value| value.effects[0].kind = ScalarGemmWorkerV3MachineEffectKindV1::GlobalRead,
            ScalarGemmWorkerV3ExecutableBindingErrorV1::MachineEffectSubstitution,
        );
        assert_error(
            |value| value.effects[0].byte_width = 4,
            ScalarGemmWorkerV3ExecutableBindingErrorV1::MachineEffectSubstitution,
        );
        assert_error(
            |value| {
                value.effects.pop();
            },
            ScalarGemmWorkerV3ExecutableBindingErrorV1::MachineEffectSubstitution,
        );
    }

    #[test]
    fn zero_and_malformed_authenticated_axes_fail_closed() {
        for (mutate, field) in [
            (
                (|value: &mut ScalarGemmWorkerV3ExecutableBindingComponentsV1| {
                    value.machine_execution_challenge = [0; 32]
                }) as fn(&mut ScalarGemmWorkerV3ExecutableBindingComponentsV1),
                "machine execution challenge",
            ),
            (
                |value| value.analyzer_identity = [0; 32],
                "machine analyzer",
            ),
            (
                |value| value.toolchain_identity = [0; 32],
                "machine toolchain",
            ),
            (
                |value| value.verus_runtime_closure_identity = [0; 32],
                "Verus runtime closure",
            ),
        ] {
            assert_error(
                mutate,
                ScalarGemmWorkerV3ExecutableBindingErrorV1::ZeroIdentity(field),
            );
        }
        assert_error(
            |value| value.machine_request_identity = measured(0, 2),
            ScalarGemmWorkerV3ExecutableBindingErrorV1::InvalidMeasuredIdentity("machine request"),
        );
        assert_error(
            |value| value.canonical_machine_evidence.clear(),
            ScalarGemmWorkerV3ExecutableBindingErrorV1::InvalidBlobSize("machine evidence"),
        );
        assert_error(
            |value| value.machine_evidence_identity = measured(14, 4),
            ScalarGemmWorkerV3ExecutableBindingErrorV1::BlobIdentityLengthMismatch(
                "machine evidence",
            ),
        );
    }

    #[test]
    fn every_dynamic_identity_and_lossless_blob_changes_the_binding() {
        assert_identity_changes(|value| value.machine_execution_challenge[0] ^= 1);
        assert_identity_changes(|value| value.analyzer_identity[0] ^= 1);
        assert_identity_changes(|value| value.toolchain_identity[0] ^= 1);
        assert_identity_changes(|value| value.machine_request_identity.sha256[0] ^= 1);
        assert_identity_changes(|value| value.machine_evidence_identity.sha256[0] ^= 1);
        assert_identity_changes(|value| value.authenticated_receipt_identity.sha256[0] ^= 1);
        assert_identity_changes(|value| value.worker_executable_identity.sha256[0] ^= 1);
        assert_identity_changes(|value| value.machine_runtime_closure_identity.sha256[0] ^= 1);
        assert_identity_changes(|value| value.machine_runtime_mapping_identity.sha256[0] ^= 1);
        assert_identity_changes(|value| value.verus_runtime_closure_identity[0] ^= 1);
        assert_identity_changes(|value| value.canonical_machine_request[0] ^= 1);
        assert_identity_changes(|value| value.canonical_machine_evidence[0] ^= 1);
        assert_identity_changes(|value| value.canonical_authenticated_receipt[0] ^= 1);
    }
}
