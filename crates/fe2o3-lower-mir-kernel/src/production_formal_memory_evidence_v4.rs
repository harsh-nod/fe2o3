//! Canonical formal-memory custody bound to the current production KIR.

use std::{error::Error, fmt};

use fe2o3_kernel_ir::{
    FormalIndexWidth, FormalMemoryAnalysisBasis, FormalMemoryReceiptEncodingV3,
    FormalMemoryReceiptErrorV1, FormalMemoryReceiptMetadataV3, InertFormalMemoryReceiptFormatV3,
};
use sha2::{Digest, Sha256};

use crate::{
    ProductionCanonicalKernelIrIdentityV1, ProductionCanonicalKernelIrVersionV1,
    ProductionFormalMemoryOwnerV1,
};

/// Current wire version for formal-memory admission custody.
pub const FORMAL_MEMORY_ADMISSION_EVIDENCE_VERSION_V4: u16 = 4;
/// Closed validation policy for formal-memory admission custody.
pub const FORMAL_MEMORY_ADMISSION_EVIDENCE_POLICY_V4: u16 = 1;
/// Domain-aware validation policy paired only with obligation receipt V3/policy 2.
pub const FORMAL_MEMORY_ADMISSION_EVIDENCE_GUARDED_POLICY_V4: u16 = 2;
/// Maximum exact bytes accepted by the outer compiler-lineage receipt.
pub const MAX_FORMAL_MEMORY_ADMISSION_EVIDENCE_BYTES_V4: usize = 4 * 1024 * 1024;

const MAGIC_V4: [u8; 8] = *b"F2FMA4\0\0";
const IDENTITY_DOMAIN_V4: &[u8] = b"FE2O3/FORMAL-MEMORY-ADMISSION-EVIDENCE/V4\0";
const HEADER_BYTES_V4: usize = 120;

/// Closed nested-format and witness policy committed by V4 evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum FormalMemoryAdmissionValidationPolicyV4 {
    /// Frozen policy 1 accepts only legacy V1/extraction-policy 1.
    LegacyV1 = FORMAL_MEMORY_ADMISSION_EVIDENCE_POLICY_V4,
    /// Policy 2 accepts only V3/extraction-policy 2 with the current Bits64 witness.
    GuardedV2 = FORMAL_MEMORY_ADMISSION_EVIDENCE_GUARDED_POLICY_V4,
}

/// Exact completeness policy committed by V4 formal-memory evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum FormalMemoryCompletenessPolicyV4 {
    /// Require complete extraction with no unresolved static or cross-invocation conflicts.
    RequireCompleteConflictFree = 1,
}

/// Exact completeness result committed by V4 formal-memory evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum FormalMemoryCompletenessStatusV4 {
    /// The production owner re-derived complete, conflict-free obligations.
    Complete = 1,
}

/// Authority-free canonical formal-memory evidence bound to versioned production KIR custody.
#[derive(Debug, Eq, PartialEq)]
pub struct InertCanonicalFormalMemoryAdmissionEvidenceV4 {
    canonical_bytes: Box<[u8]>,
    identity: [u8; 32],
    canonical_kernel_ir: ProductionCanonicalKernelIrIdentityV1,
    formal_obligation_receipt_identity: [u8; 32],
    witness_invocation_count: u64,
    validation_policy: FormalMemoryAdmissionValidationPolicyV4,
    completeness_policy: FormalMemoryCompletenessPolicyV4,
    completeness_status: FormalMemoryCompletenessStatusV4,
    static_conflict_count: u32,
    inter_invocation_conflict_count: u32,
    formal_obligation_receipt_offset: usize,
}

impl InertCanonicalFormalMemoryAdmissionEvidenceV4 {
    /// Revalidates a live owner and binds its exact current KIR and obligation receipt.
    pub fn from_live_owner(
        owner: &ProductionFormalMemoryOwnerV1,
    ) -> Result<Self, ProductionFormalMemoryEvidenceErrorV4> {
        owner
            .verify_equivalence()
            .map_err(|error| ProductionFormalMemoryEvidenceErrorV4::LiveOwner(error.to_string()))?;
        let [kernel] = owner.kernels() else {
            return Err(ProductionFormalMemoryEvidenceErrorV4::InvalidAdmission);
        };
        if !kernel.obligations().inter_invocation_conflicts().is_empty() {
            return Err(ProductionFormalMemoryEvidenceErrorV4::InvalidAdmission);
        }
        let receipt =
            InertFormalMemoryReceiptFormatV3::from_current_obligations(kernel.obligations())
                .map_err(ProductionFormalMemoryEvidenceErrorV4::FormalReceipt)?;
        receipt
            .revalidate()
            .map_err(ProductionFormalMemoryEvidenceErrorV4::FormalReceipt)?;
        let witness_invocation_count = kernel.witness_invocation_count();
        if witness_invocation_count == 0
            || kernel
                .witness_extents(owner.semantic_kir().module())
                .is_none_or(|extents| extents.contains(&0))
        {
            return Err(ProductionFormalMemoryEvidenceErrorV4::InvalidAdmission);
        }
        let validation_policy = match receipt.metadata().encoding() {
            FormalMemoryReceiptEncodingV3::LegacyV1 => {
                FormalMemoryAdmissionValidationPolicyV4::LegacyV1
            }
            FormalMemoryReceiptEncodingV3::GuardedV3 => {
                FormalMemoryAdmissionValidationPolicyV4::GuardedV2
            }
            FormalMemoryReceiptEncodingV3::LegacyV2 => {
                return Err(ProductionFormalMemoryEvidenceErrorV4::InvalidAdmission);
            }
        };
        let bytes = encode(
            validation_policy,
            owner.semantic_kir().canonical_kernel_ir_identity(),
            *receipt.identity_digest(),
            witness_invocation_count,
            FormalMemoryCompletenessPolicyV4::RequireCompleteConflictFree,
            FormalMemoryCompletenessStatusV4::Complete,
            0,
            0,
            &receipt,
        )?;
        Self::decode(&bytes)
    }

    /// Strictly decodes one complete canonical V4 formal-memory evidence value.
    pub fn decode(bytes: &[u8]) -> Result<Self, ProductionFormalMemoryEvidenceErrorV4> {
        if bytes.len() > MAX_FORMAL_MEMORY_ADMISSION_EVIDENCE_BYTES_V4 {
            return Err(ProductionFormalMemoryEvidenceErrorV4::TooLarge);
        }
        let mut reader = ReaderV4::new(bytes);
        if reader.fixed::<8>()? != MAGIC_V4
            || reader.u16()? != FORMAL_MEMORY_ADMISSION_EVIDENCE_VERSION_V4
        {
            return Err(ProductionFormalMemoryEvidenceErrorV4::InvalidHeader);
        }
        let validation_policy = match reader.u16()? {
            FORMAL_MEMORY_ADMISSION_EVIDENCE_POLICY_V4 => {
                FormalMemoryAdmissionValidationPolicyV4::LegacyV1
            }
            FORMAL_MEMORY_ADMISSION_EVIDENCE_GUARDED_POLICY_V4 => {
                FormalMemoryAdmissionValidationPolicyV4::GuardedV2
            }
            _ => return Err(ProductionFormalMemoryEvidenceErrorV4::InvalidHeader),
        };
        if reader.u32()? != 0 {
            return Err(ProductionFormalMemoryEvidenceErrorV4::InvalidHeader);
        }
        if reader.usize_u32()? != bytes.len() {
            return Err(ProductionFormalMemoryEvidenceErrorV4::InvalidLength);
        }
        let kernel_ir_version = match reader.u16()? {
            8 => ProductionCanonicalKernelIrVersionV1::V8,
            9 => ProductionCanonicalKernelIrVersionV1::V9,
            11 => ProductionCanonicalKernelIrVersionV1::V11,
            _ => return Err(ProductionFormalMemoryEvidenceErrorV4::InvalidHeader),
        };
        if reader.u16()? != 0 {
            return Err(ProductionFormalMemoryEvidenceErrorV4::InvalidHeader);
        }
        let kernel_ir_length = reader.u64()?;
        let kernel_ir_digest = reader.fixed::<32>()?;
        let formal_obligation_receipt_identity = reader.fixed::<32>()?;
        if kernel_ir_length == 0
            || kernel_ir_digest == [0; 32]
            || formal_obligation_receipt_identity == [0; 32]
        {
            return Err(ProductionFormalMemoryEvidenceErrorV4::ZeroIdentity);
        }
        let canonical_kernel_ir = ProductionCanonicalKernelIrIdentityV1::from_canonical_parts(
            kernel_ir_version,
            kernel_ir_digest,
            kernel_ir_length,
        );
        let witness_invocation_count = reader.u64()?;
        let completeness_policy = match reader.u16()? {
            1 => FormalMemoryCompletenessPolicyV4::RequireCompleteConflictFree,
            _ => return Err(ProductionFormalMemoryEvidenceErrorV4::InvalidAdmission),
        };
        let completeness_status = match reader.u16()? {
            1 => FormalMemoryCompletenessStatusV4::Complete,
            _ => return Err(ProductionFormalMemoryEvidenceErrorV4::InvalidAdmission),
        };
        let static_conflict_count = reader.u32()?;
        let inter_invocation_conflict_count = reader.u32()?;
        if witness_invocation_count == 0
            || static_conflict_count != 0
            || inter_invocation_conflict_count != 0
        {
            return Err(ProductionFormalMemoryEvidenceErrorV4::InvalidAdmission);
        }
        let receipt_len = reader.usize_u32()?;
        if receipt_len == 0
            || receipt_len
                > MAX_FORMAL_MEMORY_ADMISSION_EVIDENCE_BYTES_V4.saturating_sub(HEADER_BYTES_V4)
            || reader.remaining() != receipt_len
        {
            return Err(ProductionFormalMemoryEvidenceErrorV4::InvalidLength);
        }
        let formal_obligation_receipt_offset = reader.offset();
        let receipt =
            InertFormalMemoryReceiptFormatV3::decode_current(reader.take(receipt_len)?.to_vec())
                .map_err(ProductionFormalMemoryEvidenceErrorV4::FormalReceipt)?;
        reader.finish()?;
        receipt
            .revalidate()
            .map_err(ProductionFormalMemoryEvidenceErrorV4::FormalReceipt)?;
        if receipt.identity_digest() != &formal_obligation_receipt_identity {
            return Err(ProductionFormalMemoryEvidenceErrorV4::NestedIdentityMismatch);
        }
        let reencoded = encode(
            validation_policy,
            canonical_kernel_ir,
            formal_obligation_receipt_identity,
            witness_invocation_count,
            completeness_policy,
            completeness_status,
            static_conflict_count,
            inter_invocation_conflict_count,
            &receipt,
        )?;
        if reencoded != bytes {
            return Err(ProductionFormalMemoryEvidenceErrorV4::NonCanonical);
        }
        let identity = evidence_identity(&reencoded)?;
        Ok(Self {
            canonical_bytes: reencoded.into_boxed_slice(),
            identity,
            canonical_kernel_ir,
            formal_obligation_receipt_identity,
            witness_invocation_count,
            validation_policy,
            completeness_policy,
            completeness_status,
            static_conflict_count,
            inter_invocation_conflict_count,
            formal_obligation_receipt_offset,
        })
    }

    /// Re-decodes all retained bytes and identities.
    pub fn revalidate(&self) -> Result<(), ProductionFormalMemoryEvidenceErrorV4> {
        if Self::decode(&self.canonical_bytes)? != *self {
            return Err(ProductionFormalMemoryEvidenceErrorV4::IdentityMismatch);
        }
        Ok(())
    }

    /// Returns the complete canonical V4 bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Consumes the inert evidence and returns its canonical bytes.
    pub fn into_canonical_bytes(self) -> Vec<u8> {
        self.canonical_bytes.into_vec()
    }

    /// Returns the exact evidence identity.
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }

    /// Returns the exact versioned canonical production-KIR identity.
    pub const fn canonical_kernel_ir_identity(&self) -> ProductionCanonicalKernelIrIdentityV1 {
        self.canonical_kernel_ir
    }

    /// Returns the embedded formal-obligation receipt identity.
    pub const fn formal_obligation_receipt_identity(&self) -> &[u8; 32] {
        &self.formal_obligation_receipt_identity
    }

    /// Returns the embedded exact canonical formal-obligation receipt bytes.
    pub fn formal_obligation_receipt_bytes(&self) -> &[u8] {
        &self.canonical_bytes[self.formal_obligation_receipt_offset..]
    }

    /// Returns the flattened invocation count represented by the structural witness.
    pub const fn witness_invocation_count(&self) -> u64 {
        self.witness_invocation_count
    }

    /// Returns the exact nested-format and witness validation policy.
    pub const fn validation_policy(&self) -> FormalMemoryAdmissionValidationPolicyV4 {
        self.validation_policy
    }

    /// Returns the exact completeness policy.
    pub const fn completeness_policy(&self) -> FormalMemoryCompletenessPolicyV4 {
        self.completeness_policy
    }

    /// Returns the exact completeness result.
    pub const fn completeness_status(&self) -> FormalMemoryCompletenessStatusV4 {
        self.completeness_status
    }

    /// Returns the unresolved static conflict count, always zero for admitted evidence.
    pub const fn static_conflict_count(&self) -> u32 {
        self.static_conflict_count
    }

    /// Returns the cross-invocation conflict count, always zero for admitted evidence.
    pub const fn inter_invocation_conflict_count(&self) -> u32 {
        self.inter_invocation_conflict_count
    }

    /// Inert formal-memory custody grants no compiler, artifact, proof, or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

impl TryFrom<&[u8]> for InertCanonicalFormalMemoryAdmissionEvidenceV4 {
    type Error = ProductionFormalMemoryEvidenceErrorV4;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        Self::decode(bytes)
    }
}

/// Fail-closed V4 formal-memory evidence error.
#[derive(Debug)]
pub enum ProductionFormalMemoryEvidenceErrorV4 {
    /// Live owner equivalence replay failed.
    LiveOwner(String),
    /// The nested canonical obligation receipt failed validation.
    FormalReceipt(FormalMemoryReceiptErrorV1),
    /// Input exceeds the fixed outer receipt budget.
    TooLarge,
    /// Magic, version, policy, flags, reserved fields, or KIR version are invalid.
    InvalidHeader,
    /// Declared, computed, and available lengths differ.
    InvalidLength,
    /// A checked length calculation overflowed.
    Overflow,
    /// A required digest or canonical length is zero.
    ZeroIdentity,
    /// Completeness, witness, or conflict fields violate production admission.
    InvalidAdmission,
    /// The nested receipt identity differs from its exact bytes.
    NestedIdentityMismatch,
    /// Input ended before a complete field was available.
    Truncated,
    /// Structured decoding and canonical re-encoding differ.
    NonCanonical,
    /// Retained bytes or identity changed during revalidation.
    IdentityMismatch,
}

impl fmt::Display for ProductionFormalMemoryEvidenceErrorV4 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LiveOwner(error) => write!(formatter, "live formal-memory owner failed: {error}"),
            Self::FormalReceipt(error) => {
                write!(formatter, "formal-memory receipt failed: {error}")
            }
            Self::TooLarge => formatter.write_str("formal-memory evidence exceeds its byte limit"),
            Self::InvalidHeader => formatter.write_str("formal-memory evidence header is invalid"),
            Self::InvalidLength => formatter.write_str("formal-memory evidence length is invalid"),
            Self::Overflow => formatter.write_str("formal-memory evidence arithmetic overflowed"),
            Self::ZeroIdentity => formatter.write_str("formal-memory evidence identity is zero"),
            Self::InvalidAdmission => {
                formatter.write_str("formal-memory evidence violates production admission")
            }
            Self::NestedIdentityMismatch => {
                formatter.write_str("formal-memory receipt identity differs from its bytes")
            }
            Self::Truncated => formatter.write_str("formal-memory evidence is truncated"),
            Self::NonCanonical => formatter.write_str("formal-memory evidence is not canonical"),
            Self::IdentityMismatch => {
                formatter.write_str("formal-memory evidence identity changed")
            }
        }
    }
}

impl Error for ProductionFormalMemoryEvidenceErrorV4 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::FormalReceipt(error) => Some(error),
            _ => None,
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn encode(
    validation_policy: FormalMemoryAdmissionValidationPolicyV4,
    canonical_kernel_ir: ProductionCanonicalKernelIrIdentityV1,
    formal_obligation_receipt_identity: [u8; 32],
    witness_invocation_count: u64,
    completeness_policy: FormalMemoryCompletenessPolicyV4,
    completeness_status: FormalMemoryCompletenessStatusV4,
    static_conflict_count: u32,
    inter_invocation_conflict_count: u32,
    receipt: &InertFormalMemoryReceiptFormatV3,
) -> Result<Vec<u8>, ProductionFormalMemoryEvidenceErrorV4> {
    if canonical_kernel_ir.digest() == &[0; 32]
        || canonical_kernel_ir.canonical_length() == 0
        || formal_obligation_receipt_identity == [0; 32]
    {
        return Err(ProductionFormalMemoryEvidenceErrorV4::ZeroIdentity);
    }
    if witness_invocation_count == 0
        || completeness_policy != FormalMemoryCompletenessPolicyV4::RequireCompleteConflictFree
        || completeness_status != FormalMemoryCompletenessStatusV4::Complete
        || static_conflict_count != 0
        || inter_invocation_conflict_count != 0
        || receipt.canonical_bytes().is_empty()
    {
        return Err(ProductionFormalMemoryEvidenceErrorV4::InvalidAdmission);
    }
    validate_receipt_witness(
        validation_policy,
        receipt.metadata(),
        witness_invocation_count,
    )?;
    let receipt = receipt.canonical_bytes();
    let exact_size = HEADER_BYTES_V4
        .checked_add(receipt.len())
        .ok_or(ProductionFormalMemoryEvidenceErrorV4::Overflow)?;
    if exact_size > MAX_FORMAL_MEMORY_ADMISSION_EVIDENCE_BYTES_V4 {
        return Err(ProductionFormalMemoryEvidenceErrorV4::TooLarge);
    }
    let mut bytes = Vec::with_capacity(exact_size);
    bytes.extend_from_slice(&MAGIC_V4);
    push_u16(&mut bytes, FORMAL_MEMORY_ADMISSION_EVIDENCE_VERSION_V4);
    push_u16(&mut bytes, validation_policy as u16);
    push_u32(&mut bytes, 0);
    push_usize(&mut bytes, exact_size)?;
    push_u16(
        &mut bytes,
        match canonical_kernel_ir.version() {
            ProductionCanonicalKernelIrVersionV1::V8 => 8,
            ProductionCanonicalKernelIrVersionV1::V9 => 9,
            ProductionCanonicalKernelIrVersionV1::V11 => 11,
        },
    );
    push_u16(&mut bytes, 0);
    push_u64(&mut bytes, canonical_kernel_ir.canonical_length());
    bytes.extend_from_slice(canonical_kernel_ir.digest());
    bytes.extend_from_slice(&formal_obligation_receipt_identity);
    push_u64(&mut bytes, witness_invocation_count);
    push_u16(&mut bytes, completeness_policy as u16);
    push_u16(&mut bytes, completeness_status as u16);
    push_u32(&mut bytes, static_conflict_count);
    push_u32(&mut bytes, inter_invocation_conflict_count);
    push_usize(&mut bytes, receipt.len())?;
    bytes.extend_from_slice(receipt);
    if bytes.len() != exact_size {
        return Err(ProductionFormalMemoryEvidenceErrorV4::InvalidLength);
    }
    Ok(bytes)
}

fn validate_receipt_witness(
    validation_policy: FormalMemoryAdmissionValidationPolicyV4,
    metadata: FormalMemoryReceiptMetadataV3,
    witness_invocation_count: u64,
) -> Result<(), ProductionFormalMemoryEvidenceErrorV4> {
    let expected = match validation_policy {
        FormalMemoryAdmissionValidationPolicyV4::LegacyV1 => {
            FormalMemoryReceiptEncodingV3::LegacyV1
        }
        FormalMemoryAdmissionValidationPolicyV4::GuardedV2 => {
            FormalMemoryReceiptEncodingV3::GuardedV3
        }
    };
    if metadata.encoding() != expected {
        return Err(ProductionFormalMemoryEvidenceErrorV4::InvalidAdmission);
    }
    if validation_policy == FormalMemoryAdmissionValidationPolicyV4::GuardedV2
        && (metadata.index_width() != FormalIndexWidth::Bits64
            || metadata.analysis_basis()
                != FormalMemoryAnalysisBasis::CompilerDerivedIrWithUnauthenticatedLaunchInputs)
    {
        return Err(ProductionFormalMemoryEvidenceErrorV4::InvalidAdmission);
    }
    if witness_invocation_count == 0
        || metadata.invocations().is_none_or(|range| {
            range.start() != 0 || range.end_exclusive() != witness_invocation_count
        })
    {
        return Err(ProductionFormalMemoryEvidenceErrorV4::InvalidAdmission);
    }
    Ok(())
}

fn evidence_identity(bytes: &[u8]) -> Result<[u8; 32], ProductionFormalMemoryEvidenceErrorV4> {
    let length =
        u64::try_from(bytes.len()).map_err(|_| ProductionFormalMemoryEvidenceErrorV4::Overflow)?;
    let mut digest = Sha256::new();
    digest.update(IDENTITY_DOMAIN_V4);
    digest.update(length.to_le_bytes());
    digest.update(bytes);
    let identity = digest.finalize().into();
    if identity == [0; 32] {
        Err(ProductionFormalMemoryEvidenceErrorV4::ZeroIdentity)
    } else {
        Ok(identity)
    }
}

fn push_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_usize(
    bytes: &mut Vec<u8>,
    value: usize,
) -> Result<(), ProductionFormalMemoryEvidenceErrorV4> {
    push_u32(
        bytes,
        u32::try_from(value).map_err(|_| ProductionFormalMemoryEvidenceErrorV4::Overflow)?,
    );
    Ok(())
}

struct ReaderV4<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ReaderV4<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    const fn offset(&self) -> usize {
        self.offset
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], ProductionFormalMemoryEvidenceErrorV4> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(ProductionFormalMemoryEvidenceErrorV4::Overflow)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(ProductionFormalMemoryEvidenceErrorV4::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], ProductionFormalMemoryEvidenceErrorV4> {
        self.take(N)?
            .try_into()
            .map_err(|_| ProductionFormalMemoryEvidenceErrorV4::Truncated)
    }

    fn u16(&mut self) -> Result<u16, ProductionFormalMemoryEvidenceErrorV4> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, ProductionFormalMemoryEvidenceErrorV4> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn u64(&mut self) -> Result<u64, ProductionFormalMemoryEvidenceErrorV4> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }

    fn usize_u32(&mut self) -> Result<usize, ProductionFormalMemoryEvidenceErrorV4> {
        usize::try_from(self.u32()?).map_err(|_| ProductionFormalMemoryEvidenceErrorV4::Overflow)
    }

    fn finish(self) -> Result<(), ProductionFormalMemoryEvidenceErrorV4> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(ProductionFormalMemoryEvidenceErrorV4::InvalidLength)
        }
    }
}

#[cfg(test)]
mod guarded_policy_tests {
    use super::*;
    use fe2o3_kernel_ir::{
        AccessMode, AddressSpace, BasicBlock, BlockId, ExplicitLaunchExtent1d,
        FormalMemoryIncompleteReason, Function, Kernel, KernelId, LaunchDomain, LaunchExtent,
        Module, ScalarType, Signature, Terminator, Type, ValueId, VerifiedCanonicalKernelIrV8,
        VerifiedCanonicalKernelIrV9, derive_kernel_memory_obligations_from_verified,
        verify_module_ref,
    };

    fn fixed_receipt(
        access: AccessMode,
        width: FormalIndexWidth,
    ) -> (
        ProductionCanonicalKernelIrIdentityV1,
        InertFormalMemoryReceiptFormatV3,
    ) {
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: vec![] });
        let mut module = Module::new("fixed-policy-component");
        module.functions.push(Function::kernel_entry(
            "entry",
            Signature::new(
                vec![Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    access,
                )],
                vec![],
            ),
            vec![ValueId(0)],
            vec![block],
        ));
        module.kernels.push(Kernel::new(
            "kernel",
            "entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(64),
            },
        ));
        let report = derive_kernel_memory_obligations_from_verified(
            verify_module_ref(&module).unwrap(),
            &KernelId::new("kernel"),
            ExplicitLaunchExtent1d::Exact(64),
            width,
        )
        .unwrap();
        if width == FormalIndexWidth::Bits32 {
            assert!(matches!(
                report.incomplete_reasons(),
                [FormalMemoryIncompleteReason::UnsupportedIndexWidth {
                    width: FormalIndexWidth::Bits32
                }]
            ));
        } else {
            assert!(report.is_complete());
        }
        let receipt =
            InertFormalMemoryReceiptFormatV3::from_current_obligations(report.obligations())
                .unwrap();
        let identity = if access == AccessMode::WriteOnly {
            let canonical = VerifiedCanonicalKernelIrV9::from_module(module).unwrap();
            ProductionCanonicalKernelIrIdentityV1::from_canonical_parts(
                ProductionCanonicalKernelIrVersionV1::V9,
                *canonical.identity().digest(),
                canonical.canonical_bytes().len() as u64,
            )
        } else {
            let canonical = VerifiedCanonicalKernelIrV8::from_module(module).unwrap();
            ProductionCanonicalKernelIrIdentityV1::from_canonical_parts(
                ProductionCanonicalKernelIrVersionV1::V8,
                *canonical.identity().digest(),
                canonical.canonical_bytes().len() as u64,
            )
        };
        (identity, receipt)
    }

    #[test]
    fn legacy_policy_preserves_bits32_exact_header_identity_and_v2_refusal() {
        let (identity, receipt) = fixed_receipt(AccessMode::ReadWrite, FormalIndexWidth::Bits32);
        let encoded = encode(
            FormalMemoryAdmissionValidationPolicyV4::LegacyV1,
            identity,
            *receipt.identity_digest(),
            64,
            FormalMemoryCompletenessPolicyV4::RequireCompleteConflictFree,
            FormalMemoryCompletenessStatusV4::Complete,
            0,
            0,
            &receipt,
        )
        .unwrap();
        // The old 120-byte header and nested bytes are unchanged, including Bits32.
        let mut expected = Vec::new();
        expected.extend_from_slice(&MAGIC_V4);
        for value in [4_u16, 1] {
            expected.extend_from_slice(&value.to_le_bytes());
        }
        expected.extend_from_slice(&0_u32.to_le_bytes());
        expected.extend_from_slice(&((120 + receipt.canonical_bytes().len()) as u32).to_le_bytes());
        expected.extend_from_slice(&8_u16.to_le_bytes());
        expected.extend_from_slice(&0_u16.to_le_bytes());
        expected.extend_from_slice(&identity.canonical_length().to_le_bytes());
        expected.extend_from_slice(identity.digest());
        expected.extend_from_slice(receipt.identity_digest());
        expected.extend_from_slice(&64_u64.to_le_bytes());
        expected.extend_from_slice(&[1, 0, 1, 0]);
        expected.extend_from_slice(&[0; 8]);
        expected.extend_from_slice(&(receipt.canonical_bytes().len() as u32).to_le_bytes());
        assert_eq!(expected.len(), 120);
        expected.extend_from_slice(receipt.canonical_bytes());
        assert_eq!(encoded, expected);
        let decoded = InertCanonicalFormalMemoryAdmissionEvidenceV4::decode(&expected).unwrap();
        assert_eq!(decoded.canonical_bytes(), expected);
        assert_eq!(decoded.identity(), &evidence_identity(&expected).unwrap());
        assert!(matches!(
            validate_receipt_witness(
                FormalMemoryAdmissionValidationPolicyV4::GuardedV2,
                receipt.metadata(),
                64
            ),
            Err(ProductionFormalMemoryEvidenceErrorV4::InvalidAdmission)
        ));
        let (_, write_only) = fixed_receipt(AccessMode::WriteOnly, FormalIndexWidth::Bits64);
        assert_eq!(
            write_only.metadata().encoding(),
            FormalMemoryReceiptEncodingV3::LegacyV2
        );
        for policy in [
            FormalMemoryAdmissionValidationPolicyV4::LegacyV1,
            FormalMemoryAdmissionValidationPolicyV4::GuardedV2,
        ] {
            assert!(matches!(
                validate_receipt_witness(policy, write_only.metadata(), 64),
                Err(ProductionFormalMemoryEvidenceErrorV4::InvalidAdmission)
            ));
        }
    }
}
