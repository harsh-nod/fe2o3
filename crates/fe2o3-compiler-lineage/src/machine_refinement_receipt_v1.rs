//! Canonical target-specific transport for independently checked machine refinement.

use core::fmt;

use sha2::{Digest as _, Sha256};

use crate::{
    CompilerInstructionSelectionCorrespondenceIdentityV1, ExactCompilerStageContentIdentityV1,
    PostLlvmStageCustodyIdentityV1,
};

/// Canonical receipt magic.
pub const TARGET_MACHINE_REFINEMENT_RECEIPT_MAGIC_V1: [u8; 8] = *b"F2MREFV1";
/// Canonical receipt version.
pub const TARGET_MACHINE_REFINEMENT_RECEIPT_VERSION_V1: u16 = 1;
/// Exact canonical receipt length.
pub const TARGET_MACHINE_REFINEMENT_RECEIPT_BYTES_V1: usize = 260;

const POLICY_V1: u16 = 1;
const HEADER_BYTES_V1: usize = 20;
const IDENTITY_BYTES_V1: usize = 40;
const TERMINAL_BYTES_V1: usize = 32;
const IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/TARGET-MACHINE-REFINEMENT-RECEIPT/V1\0";

/// Target selected by authenticated compiler target data.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum TargetMachineRefinementTargetV1 {
    /// The exact production gfx942 target profile.
    Gfx942,
    /// The exact production gfx950 target profile.
    Gfx950,
}

/// One bounded content coordinate from a target-specific decoder.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MachineRefinementContentIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl MachineRefinementContentIdentityV1 {
    /// Constructs one nonzero bounded identity.
    pub fn new(
        sha256: [u8; 32],
        byte_len: u64,
    ) -> Result<Self, TargetMachineRefinementReceiptErrorV1> {
        if sha256 == [0; 32] || byte_len == 0 {
            return Err(TargetMachineRefinementReceiptErrorV1::InvalidIdentity);
        }
        Ok(Self { sha256, byte_len })
    }

    /// Returns the digest.
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    /// Returns the exact content length.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

/// Bit positions for independently decoded machine families.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum MachineRefinementFamilyV1 {
    /// Scalar arithmetic and bitwise instructions.
    ScalarAlu = 0,
    /// Vector arithmetic instructions.
    VectorAlu = 1,
    /// Direct control-flow instructions and targets.
    ControlFlow = 2,
    /// EXEC save, update, branch, and restore instructions.
    ExecutionMask = 3,
    /// Global-memory instructions and effective addresses.
    GlobalMemory = 4,
    /// LDS instructions and effective addresses.
    LocalMemory = 5,
    /// Workgroup barrier instructions.
    Barrier = 6,
    /// Scoped atomic instructions.
    Atomic = 7,
    /// DPP lane-permutation instructions.
    Dpp = 8,
    /// Matrix instructions.
    Matrix = 9,
    /// Kernel MODE and numerical environment.
    FloatingMode = 10,
    /// Object-to-HSACO section, symbol, and relocation custody.
    ObjectToHsaco = 11,
}

impl MachineRefinementFamilyV1 {
    /// Returns this family bit.
    pub const fn bit(self) -> u64 {
        1_u64 << self as u8
    }
}

const KNOWN_FAMILY_MASK_V1: u64 = (1_u64 << 12) - 1;

/// Exact target-specific coordinates carried by one #214 machine-refinement receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TargetMachineRefinementReceiptPartsV1 {
    /// Authenticated target selected by the producer/checker boundary.
    pub target: TargetMachineRefinementTargetV1,
    /// Exact post-LLVM custody record.
    pub post_llvm_custody: PostLlvmStageCustodyIdentityV1,
    /// Exact compiler-emitted instruction-selection correspondence.
    pub instruction_selection: CompilerInstructionSelectionCorrespondenceIdentityV1,
    /// Exact independently decoded ISA transcript.
    pub decoded_isa: MachineRefinementContentIdentityV1,
    /// Exact raw final code object checked by all three records.
    pub final_code_object: ExactCompilerStageContentIdentityV1,
    /// Target-adapter conjunction identity.
    pub machine_refinement_sha256: [u8; 32],
    /// Families required by this exact correspondence.
    pub required_families: u64,
    /// Families independently established by the target adapter.
    pub established_families: u64,
}

/// Canonical, typed, authority-free target machine-refinement receipt.
///
/// Parsing rejects opaque or malformed bytes. Issuer authentication remains the sealed verifier's
/// responsibility; this type must not be treated as a verifier seal.
#[derive(Debug, Eq, PartialEq)]
pub struct TargetMachineRefinementReceiptV1 {
    parts: TargetMachineRefinementReceiptPartsV1,
    canonical_bytes: Box<[u8]>,
    terminal_sha256: [u8; 32],
}

impl TargetMachineRefinementReceiptV1 {
    /// Constructs and canonically encodes exact checked coordinates.
    pub fn from_parts(
        parts: TargetMachineRefinementReceiptPartsV1,
    ) -> Result<Self, TargetMachineRefinementReceiptErrorV1> {
        validate_parts(parts)?;
        let mut bytes = Vec::with_capacity(TARGET_MACHINE_REFINEMENT_RECEIPT_BYTES_V1);
        bytes.extend_from_slice(&TARGET_MACHINE_REFINEMENT_RECEIPT_MAGIC_V1);
        bytes.extend_from_slice(&TARGET_MACHINE_REFINEMENT_RECEIPT_VERSION_V1.to_le_bytes());
        bytes.extend_from_slice(&POLICY_V1.to_le_bytes());
        bytes.push(target_tag(parts.target));
        bytes.extend_from_slice(&[0; 3]);
        bytes.extend_from_slice(&(TARGET_MACHINE_REFINEMENT_RECEIPT_BYTES_V1 as u32).to_le_bytes());
        encode_identity(
            &mut bytes,
            parts.post_llvm_custody.sha256(),
            parts.post_llvm_custody.byte_len(),
        );
        encode_identity(
            &mut bytes,
            parts.instruction_selection.sha256(),
            parts.instruction_selection.byte_len(),
        );
        encode_identity(
            &mut bytes,
            parts.decoded_isa.sha256(),
            parts.decoded_isa.byte_len(),
        );
        encode_identity(
            &mut bytes,
            parts.final_code_object.sha256(),
            parts.final_code_object.byte_len(),
        );
        bytes.extend_from_slice(&parts.machine_refinement_sha256);
        bytes.extend_from_slice(&parts.required_families.to_le_bytes());
        bytes.extend_from_slice(&parts.established_families.to_le_bytes());
        let terminal_sha256 = receipt_identity(&bytes);
        bytes.extend_from_slice(&terminal_sha256);
        debug_assert_eq!(bytes.len(), TARGET_MACHINE_REFINEMENT_RECEIPT_BYTES_V1);
        Ok(Self {
            parts,
            canonical_bytes: bytes.into_boxed_slice(),
            terminal_sha256,
        })
    }

    /// Decodes exactly one complete canonical receipt.
    pub fn decode(bytes: &[u8]) -> Result<Self, TargetMachineRefinementReceiptErrorV1> {
        if bytes.len() != TARGET_MACHINE_REFINEMENT_RECEIPT_BYTES_V1 {
            return Err(TargetMachineRefinementReceiptErrorV1::Length);
        }
        let mut reader = Reader::new(bytes);
        if reader.fixed::<8>()? != TARGET_MACHINE_REFINEMENT_RECEIPT_MAGIC_V1 {
            return Err(TargetMachineRefinementReceiptErrorV1::Magic);
        }
        if reader.u16()? != TARGET_MACHINE_REFINEMENT_RECEIPT_VERSION_V1 {
            return Err(TargetMachineRefinementReceiptErrorV1::Version);
        }
        if reader.u16()? != POLICY_V1 {
            return Err(TargetMachineRefinementReceiptErrorV1::Policy);
        }
        let target = decode_target(reader.u8()?)?;
        if reader.fixed::<3>()? != [0; 3]
            || reader.u32()? as usize != TARGET_MACHINE_REFINEMENT_RECEIPT_BYTES_V1
        {
            return Err(TargetMachineRefinementReceiptErrorV1::NonCanonical);
        }
        let post_llvm_custody = reader.post_llvm_identity()?;
        let instruction_selection = reader.instruction_selection_identity()?;
        let decoded_isa = reader.content_identity()?;
        let final_code_object = reader.stage_identity()?;
        let machine_refinement_sha256 = reader.fixed::<32>()?;
        let required_families = reader.u64()?;
        let established_families = reader.u64()?;
        let terminal_sha256 = reader.fixed::<TERMINAL_BYTES_V1>()?;
        reader.finish()?;
        if terminal_sha256 != receipt_identity(&bytes[..bytes.len() - TERMINAL_BYTES_V1]) {
            return Err(TargetMachineRefinementReceiptErrorV1::Checksum);
        }
        let decoded = Self::from_parts(TargetMachineRefinementReceiptPartsV1 {
            target,
            post_llvm_custody,
            instruction_selection,
            decoded_isa,
            final_code_object,
            machine_refinement_sha256,
            required_families,
            established_families,
        })?;
        if decoded.canonical_bytes.as_ref() != bytes {
            return Err(TargetMachineRefinementReceiptErrorV1::NonCanonical);
        }
        Ok(decoded)
    }

    /// Returns exact typed coordinates.
    pub const fn parts(&self) -> TargetMachineRefinementReceiptPartsV1 {
        self.parts
    }

    /// Returns the canonical receipt bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Returns the terminal canonical receipt identity.
    pub const fn terminal_sha256(&self) -> [u8; 32] {
        self.terminal_sha256
    }

    /// Reports syntax/custody only; the sealed verifier must authenticate the #214 issuer.
    pub const fn authenticates_issuer(&self) -> bool {
        false
    }
}

/// Exact coordinates supplied by an independently checked target adapter.
pub type ExpectedTargetMachineRefinementReceiptV1 = TargetMachineRefinementReceiptPartsV1;

/// Move-only receipt whose complete coordinates matched an independently checked target adapter.
///
/// This check does not authenticate the receipt issuer. A protected consumer must retain this
/// value beside the sealed verifier owner that supplied the same canonical receipt.
#[derive(Debug, Eq, PartialEq)]
#[must_use = "dropping the checked target receipt abandons machine-refinement custody"]
pub struct CheckedTargetMachineRefinementReceiptV1 {
    receipt: TargetMachineRefinementReceiptV1,
}

impl CheckedTargetMachineRefinementReceiptV1 {
    /// Returns the exact decoded receipt.
    pub const fn receipt(&self) -> &TargetMachineRefinementReceiptV1 {
        &self.receipt
    }

    /// Returns the independently matched coordinates.
    pub const fn parts(&self) -> TargetMachineRefinementReceiptPartsV1 {
        self.receipt.parts()
    }

    /// Returns the canonical receipt identity bound by downstream owners.
    pub const fn terminal_sha256(&self) -> [u8; 32] {
        self.receipt.terminal_sha256()
    }

    /// Parsing plus coordinate equality does not authenticate the receipt issuer.
    pub const fn authenticates_issuer(&self) -> bool {
        false
    }
}

/// Requires every typed receipt coordinate to equal independently checked target-adapter facts.
pub fn check_target_machine_refinement_receipt_v1(
    canonical_receipt: &[u8],
    expected: ExpectedTargetMachineRefinementReceiptV1,
) -> Result<CheckedTargetMachineRefinementReceiptV1, TargetMachineRefinementReceiptErrorV1> {
    let receipt = TargetMachineRefinementReceiptV1::decode(canonical_receipt)?;
    if receipt.parts != expected {
        return Err(TargetMachineRefinementReceiptErrorV1::CoordinateMismatch);
    }
    Ok(CheckedTargetMachineRefinementReceiptV1 { receipt })
}

/// Closed canonical receipt failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetMachineRefinementReceiptErrorV1 {
    /// Receipt length differs from the fixed schema.
    Length,
    /// Magic differs.
    Magic,
    /// Version is unsupported.
    Version,
    /// Policy is unsupported.
    Policy,
    /// A target tag is unknown.
    Target,
    /// A typed identity is zero or malformed.
    InvalidIdentity,
    /// Family masks are unknown, empty, or leave a required family unproved.
    UnsupportedFamily,
    /// Reserved bytes or canonical re-encoding differ.
    NonCanonical,
    /// Receipt checksum differs.
    Checksum,
    /// A typed coordinate differs from independently checked facts.
    CoordinateMismatch,
    /// Receipt bytes ended early.
    Truncated,
}

impl fmt::Display for TargetMachineRefinementReceiptErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid target machine-refinement receipt: {self:?}"
        )
    }
}

impl std::error::Error for TargetMachineRefinementReceiptErrorV1 {}

fn validate_parts(
    parts: TargetMachineRefinementReceiptPartsV1,
) -> Result<(), TargetMachineRefinementReceiptErrorV1> {
    if parts.machine_refinement_sha256 == [0; 32] {
        return Err(TargetMachineRefinementReceiptErrorV1::InvalidIdentity);
    }
    if parts.required_families == 0
        || parts.required_families & !KNOWN_FAMILY_MASK_V1 != 0
        || parts.established_families & !KNOWN_FAMILY_MASK_V1 != 0
        || parts.required_families & !parts.established_families != 0
    {
        return Err(TargetMachineRefinementReceiptErrorV1::UnsupportedFamily);
    }
    Ok(())
}

const fn target_tag(target: TargetMachineRefinementTargetV1) -> u8 {
    match target {
        TargetMachineRefinementTargetV1::Gfx942 => 1,
        TargetMachineRefinementTargetV1::Gfx950 => 2,
    }
}

fn decode_target(
    tag: u8,
) -> Result<TargetMachineRefinementTargetV1, TargetMachineRefinementReceiptErrorV1> {
    match tag {
        1 => Ok(TargetMachineRefinementTargetV1::Gfx942),
        2 => Ok(TargetMachineRefinementTargetV1::Gfx950),
        _ => Err(TargetMachineRefinementReceiptErrorV1::Target),
    }
}

fn encode_identity(output: &mut Vec<u8>, sha256: [u8; 32], byte_len: u64) {
    output.extend_from_slice(&sha256);
    output.extend_from_slice(&byte_len.to_le_bytes());
}

fn receipt_identity(bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(IDENTITY_DOMAIN_V1);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

struct Reader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], TargetMachineRefinementReceiptErrorV1> {
        let end = self
            .cursor
            .checked_add(count)
            .ok_or(TargetMachineRefinementReceiptErrorV1::Length)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(TargetMachineRefinementReceiptErrorV1::Truncated)?;
        self.cursor = end;
        Ok(value)
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], TargetMachineRefinementReceiptErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| TargetMachineRefinementReceiptErrorV1::Truncated)
    }

    fn u8(&mut self) -> Result<u8, TargetMachineRefinementReceiptErrorV1> {
        Ok(self.fixed::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, TargetMachineRefinementReceiptErrorV1> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, TargetMachineRefinementReceiptErrorV1> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn u64(&mut self) -> Result<u64, TargetMachineRefinementReceiptErrorV1> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }

    fn content_identity(
        &mut self,
    ) -> Result<MachineRefinementContentIdentityV1, TargetMachineRefinementReceiptErrorV1> {
        MachineRefinementContentIdentityV1::new(self.fixed()?, self.u64()?)
    }

    fn post_llvm_identity(
        &mut self,
    ) -> Result<PostLlvmStageCustodyIdentityV1, TargetMachineRefinementReceiptErrorV1> {
        crate::decode_post_llvm_stage_custody_identity_v1(self.fixed()?, self.u64()?)
            .ok_or(TargetMachineRefinementReceiptErrorV1::InvalidIdentity)
    }

    fn instruction_selection_identity(
        &mut self,
    ) -> Result<
        CompilerInstructionSelectionCorrespondenceIdentityV1,
        TargetMachineRefinementReceiptErrorV1,
    > {
        crate::decode_compiler_instruction_selection_correspondence_identity_v1(
            self.fixed()?,
            self.u64()?,
        )
        .ok_or(TargetMachineRefinementReceiptErrorV1::InvalidIdentity)
    }

    fn stage_identity(
        &mut self,
    ) -> Result<ExactCompilerStageContentIdentityV1, TargetMachineRefinementReceiptErrorV1> {
        crate::decode_exact_compiler_stage_content_identity_v1(self.fixed()?, self.u64()?)
            .ok_or(TargetMachineRefinementReceiptErrorV1::InvalidIdentity)
    }

    fn finish(self) -> Result<(), TargetMachineRefinementReceiptErrorV1> {
        if self.cursor == self.bytes.len() {
            Ok(())
        } else {
            Err(TargetMachineRefinementReceiptErrorV1::NonCanonical)
        }
    }
}

const _: () = assert!(
    HEADER_BYTES_V1 + IDENTITY_BYTES_V1 * 4 + 32 + 8 + 8 + TERMINAL_BYTES_V1
        == TARGET_MACHINE_REFINEMENT_RECEIPT_BYTES_V1
);
