//! Inert execution-discharge custody. Raw obligations are never rewritten.

use crate::production_formal_memory_execution_discharge_v1::{
    MAX_EXECUTION_DISCHARGES_V1, derive_execution_discharges_v1,
};
use crate::production_formal_memory_v1::witness_extents;
use crate::{
    InertCanonicalFormalMemoryAdmissionEvidenceV4, MAX_FORMAL_MEMORY_ADMISSION_EVIDENCE_BYTES_V4,
    ProductionCanonicalKernelIrIdentityV1, ProductionCanonicalKernelIrVersionV1,
    ProductionFormalMemoryEvidenceErrorV4, ProductionFormalMemoryExecutionDischargeV1 as Discharge,
    ProductionFormalMemoryExecutionWitnessV1 as Witness, ProductionFormalMemoryOwnerV1,
};
use fe2o3_kernel_ir::{
    BlockId, ExplicitLaunchExtent, FormalGuardedPathV1, FormalIndexWidth,
    FormalMemoryAnalysisBasis, FormalMemoryObligationAnalysis, FormalMemoryReceiptEncodingV4,
    FunctionOperationLocation, InertFormalMemoryReceiptFormatV4, Module, ValueId,
    VerifiedCanonicalKernelIrV8, VerifiedCanonicalKernelIrV9, VerifiedCanonicalKernelIrV11,
    VerifiedKernelIrModuleV1, derive_kernel_memory_obligations_for_launch, encode_module_v8,
    encode_module_v9, encode_module_v11,
};
use sha2::{Digest, Sha256};
use std::{error::Error, fmt};

pub const FORMAL_MEMORY_ADMISSION_EVIDENCE_VERSION_V5: u16 = 5;
pub const FORMAL_MEMORY_ADMISSION_EVIDENCE_EXECUTION_POLICY_V5: u16 = 1;
const MAGIC: [u8; 8] = *b"F2FMA5\0\0";
const DOMAIN: &[u8] = b"FE2O3/FORMAL-MEMORY-ADMISSION-EVIDENCE/V5\0";
const HEADER_BYTES: usize = 160;
const ROW_BYTES: usize = 96;

#[derive(Debug)]
pub enum ProductionFormalMemoryEvidenceErrorV5 {
    Legacy(ProductionFormalMemoryEvidenceErrorV4),
    LiveOwner(String),
    Invalid(&'static str),
}
impl fmt::Display for ProductionFormalMemoryEvidenceErrorV5 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Legacy(error) => write!(f, "legacy formal evidence: {error}"),
            Self::LiveOwner(error) => write!(f, "formal evidence owner: {error}"),
            Self::Invalid(reason) => write!(f, "invalid execution-discharge evidence: {reason}"),
        }
    }
}
impl Error for ProductionFormalMemoryEvidenceErrorV5 {}
type Result<T> = std::result::Result<T, ProductionFormalMemoryEvidenceErrorV5>;
fn invalid(reason: &'static str) -> ProductionFormalMemoryEvidenceErrorV5 {
    ProductionFormalMemoryEvidenceErrorV5::Invalid(reason)
}
fn external(error: impl fmt::Display) -> ProductionFormalMemoryEvidenceErrorV5 {
    ProductionFormalMemoryEvidenceErrorV5::LiveOwner(error.to_string())
}

/// A decoded value is observation-only; source admission remains with its live owner.
#[derive(Debug, Eq, PartialEq)]
pub struct InertCanonicalFormalMemoryAdmissionEvidenceV5 {
    bytes: Box<[u8]>,
    identity: [u8; 32],
    kir: ProductionCanonicalKernelIrIdentityV1,
    kernel_ordinal: usize,
    kernel: Box<str>,
    entry: Box<str>,
    extents: [u64; 3],
    receipt_offset: usize,
    receipt_length: usize,
    discharges: Box<[Discharge]>,
}

impl InertCanonicalFormalMemoryAdmissionEvidenceV5 {
    pub fn from_live_owner_kernel(
        owner: &ProductionFormalMemoryOwnerV1,
        ordinal: usize,
    ) -> Result<Self> {
        owner.verify_equivalence().map_err(external)?;
        let retained = owner
            .kernels()
            .get(ordinal)
            .ok_or_else(|| invalid("kernel ordinal"))?;
        let module = owner.semantic_kir().module();
        let kernel = module
            .kernels
            .get(ordinal)
            .ok_or_else(|| invalid("kernel ordinal"))?;
        let receipt =
            InertFormalMemoryReceiptFormatV4::from_current_obligations(retained.obligations())
                .map_err(external)?;
        let bytes = encode(
            owner.semantic_kir().canonical_kernel_ir_identity(),
            ordinal,
            kernel.id.as_str(),
            kernel.entry.as_str(),
            witness_extents(&kernel.domain),
            &receipt,
            retained.execution_discharges(),
        )?;
        Self::decode(&bytes)
    }

    /// Checks canonical syntax and cross-field metadata, not raw-pair coverage.
    /// Consumers must replay against the actual graph before using a discharge.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > MAX_FORMAL_MEMORY_ADMISSION_EVIDENCE_BYTES_V4 {
            return Err(invalid("byte bound"));
        }
        let mut r = Reader { bytes, offset: 0 };
        if r.fixed::<8>()? != MAGIC
            || r.u16()? != 5
            || r.u16()? != 1
            || r.u32()? != 0
            || r.u32()? as usize != bytes.len()
        {
            return Err(invalid("header"));
        }
        let version = match r.u16()? {
            8 => ProductionCanonicalKernelIrVersionV1::V8,
            9 => ProductionCanonicalKernelIrVersionV1::V9,
            11 => ProductionCanonicalKernelIrVersionV1::V11,
            _ => return Err(invalid("KIR version")),
        };
        if r.u16()? != 0 {
            return Err(invalid("reserved"));
        }
        let length = r.u64()?;
        let digest = r.fixed::<32>()?;
        let receipt_digest = r.fixed::<32>()?;
        if length == 0 || digest == [0; 32] || receipt_digest == [0; 32] {
            return Err(invalid("zero identity"));
        }
        let kir =
            ProductionCanonicalKernelIrIdentityV1::from_canonical_parts(version, digest, length);
        let kernel_ordinal = r.u32()? as usize;
        if r.u16()? != 1 || r.u16()? != 64 {
            return Err(invalid("witness rank/width"));
        }
        let extents = [r.u64()?, r.u64()?, r.u64()?];
        if extents[0] == 0 || extents[1..] != [1, 1] || r.u64()? != extents[0] {
            return Err(invalid("witness extent"));
        }
        let raw_count = r.u32()? as usize;
        if r.u32()? != 0 {
            return Err(invalid("unresolved conflicts"));
        }
        let count = r.u32()? as usize;
        let receipt_length = r.u32()? as usize;
        if count == 0
            || count != raw_count
            || count > MAX_EXECUTION_DISCHARGES_V1
            || receipt_length == 0
        {
            return Err(invalid("discharge count"));
        }
        let kernel = r.string()?;
        let entry = r.string()?;
        if r.bytes.len().saturating_sub(r.offset)
            != receipt_length
                .checked_add(count * ROW_BYTES)
                .ok_or_else(|| invalid("length"))?
        {
            return Err(invalid("length"));
        }
        let receipt_offset = r.offset;
        let receipt =
            InertFormalMemoryReceiptFormatV4::decode_current(r.take(receipt_length)?.to_vec())
                .map_err(external)?;
        if receipt.identity_digest() != &receipt_digest
            || receipt.kernel_id() != kernel
            || receipt.entry_id() != entry
        {
            return Err(invalid("raw receipt association"));
        }
        validate_receipt_metadata(&receipt, extents[0])?;
        let mut discharges = Vec::new();
        discharges
            .try_reserve_exact(count)
            .map_err(|_| invalid("allocation"))?;
        let mut seen = Vec::new();
        seen.try_reserve_exact(count)
            .map_err(|_| invalid("allocation"))?;
        for ordinal in 0..count {
            let conflict_ordinal = r.u32()?;
            let allocation_parameter = r.u32()?;
            let left = r.location()?;
            let right = r.location()?;
            if conflict_ordinal as usize != ordinal {
                return Err(invalid("duplicate or reordered discharge"));
            }
            seen.push((allocation_parameter, left, right));
            discharges.push(Discharge {
                conflict_ordinal,
                allocation_parameter,
                left,
                right,
                left_witness: r.witness()?,
                right_witness: r.witness()?,
            });
        }
        seen.sort_unstable();
        if seen.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(invalid("duplicate discharge"));
        }
        if r.offset != bytes.len() {
            return Err(invalid("trailing bytes"));
        }
        let encoded = encode(
            kir,
            kernel_ordinal,
            &kernel,
            &entry,
            extents,
            &receipt,
            &discharges,
        )?;
        if encoded != bytes {
            return Err(invalid("noncanonical bytes"));
        }
        let mut hash = Sha256::new();
        hash.update(DOMAIN);
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
        Ok(Self {
            bytes: encoded.into_boxed_slice(),
            identity: hash.finalize().into(),
            kir,
            kernel_ordinal,
            kernel: kernel.into_boxed_str(),
            entry: entry.into_boxed_str(),
            extents,
            receipt_offset,
            receipt_length,
            discharges: discharges.into_boxed_slice(),
        })
    }

    pub fn revalidate(&self) -> Result<()> {
        if Self::decode(&self.bytes)? != *self {
            return Err(invalid("retained identity"));
        }
        Ok(())
    }
    /// Replays exact graph/raw-obligation/exclusion custody, not source or completeness admission.
    pub fn revalidate_against_verified_module(
        &self,
        verified: VerifiedKernelIrModuleV1<'_>,
    ) -> Result<()> {
        self.revalidate()?;
        let module = verified.module();
        if canonical_identity(module, self.kir.version())? != self.kir {
            return Err(invalid("current graph identity"));
        }
        let kernel = module
            .kernels
            .get(self.kernel_ordinal)
            .ok_or_else(|| invalid("kernel ordinal"))?;
        if kernel.id.as_str() != self.kernel.as_ref()
            || kernel.entry.as_str() != self.entry.as_ref()
            || kernel.domain.rank() != 1
            || witness_extents(&kernel.domain) != self.extents
        {
            return Err(invalid("current kernel witness"));
        }
        let analysis = replay_obligations(verified, self.kernel_ordinal)?;
        let obligations = analysis.obligations();
        let receipt = InertFormalMemoryReceiptFormatV4::from_current_obligations(obligations)
            .map_err(external)?;
        if receipt.canonical_bytes() != self.formal_obligation_receipt_bytes() {
            return Err(invalid("current raw obligations"));
        }
        let current =
            derive_execution_discharges_v1(module, kernel, obligations).map_err(external)?;
        if current != self.discharges {
            return Err(invalid("current exclusion roster"));
        }
        Ok(())
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
    pub const fn canonical_kernel_ir_identity(&self) -> ProductionCanonicalKernelIrIdentityV1 {
        self.kir
    }
    pub fn formal_obligation_receipt_bytes(&self) -> &[u8] {
        &self.bytes[self.receipt_offset..self.receipt_offset + self.receipt_length]
    }
    pub fn kernel_id(&self) -> &str {
        &self.kernel
    }
    pub fn entry_id(&self) -> &str {
        &self.entry
    }
    pub const fn kernel_ordinal(&self) -> usize {
        self.kernel_ordinal
    }
    pub fn discharges(&self) -> &[Discharge] {
        &self.discharges
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Current singleton custody without reinterpreting any legacy validation policy.
#[derive(Debug, Eq, PartialEq)]
pub enum InertFormalMemoryAdmissionEvidenceFormatV5 {
    Legacy(InertCanonicalFormalMemoryAdmissionEvidenceV4),
    ExecutionDischarged(InertCanonicalFormalMemoryAdmissionEvidenceV5),
}
impl InertFormalMemoryAdmissionEvidenceFormatV5 {
    pub fn from_live_owner(owner: &ProductionFormalMemoryOwnerV1) -> Result<Self> {
        let [kernel] = owner.kernels() else {
            return Err(invalid("singleton owner"));
        };
        if kernel.execution_discharges().is_empty() {
            InertCanonicalFormalMemoryAdmissionEvidenceV4::from_live_owner(owner)
                .map(Self::Legacy)
                .map_err(ProductionFormalMemoryEvidenceErrorV5::Legacy)
        } else {
            InertCanonicalFormalMemoryAdmissionEvidenceV5::from_live_owner_kernel(owner, 0)
                .map(Self::ExecutionDischarged)
        }
    }
    pub fn decode_current(bytes: &[u8]) -> Result<Self> {
        if bytes.starts_with(&MAGIC) {
            InertCanonicalFormalMemoryAdmissionEvidenceV5::decode(bytes)
                .map(Self::ExecutionDischarged)
        } else {
            InertCanonicalFormalMemoryAdmissionEvidenceV4::decode(bytes)
                .map(Self::Legacy)
                .map_err(ProductionFormalMemoryEvidenceErrorV5::Legacy)
        }
    }
    pub fn canonical_bytes(&self) -> &[u8] {
        match self {
            Self::Legacy(v) => v.canonical_bytes(),
            Self::ExecutionDischarged(v) => v.canonical_bytes(),
        }
    }
    pub fn identity(&self) -> &[u8; 32] {
        match self {
            Self::Legacy(v) => v.identity(),
            Self::ExecutionDischarged(v) => v.identity(),
        }
    }
    pub fn canonical_kernel_ir_identity(&self) -> ProductionCanonicalKernelIrIdentityV1 {
        match self {
            Self::Legacy(v) => v.canonical_kernel_ir_identity(),
            Self::ExecutionDischarged(v) => v.canonical_kernel_ir_identity(),
        }
    }
    pub fn formal_obligation_receipt_bytes(&self) -> &[u8] {
        match self {
            Self::Legacy(v) => v.formal_obligation_receipt_bytes(),
            Self::ExecutionDischarged(v) => v.formal_obligation_receipt_bytes(),
        }
    }
    pub const fn grants_authority(&self) -> bool {
        false
    }
    pub fn legacy_v4(&self) -> Option<&InertCanonicalFormalMemoryAdmissionEvidenceV4> {
        if let Self::Legacy(v) = self {
            Some(v)
        } else {
            None
        }
    }
    pub fn execution_discharged_v5(
        &self,
    ) -> Option<&InertCanonicalFormalMemoryAdmissionEvidenceV5> {
        if let Self::ExecutionDischarged(v) = self {
            Some(v)
        } else {
            None
        }
    }
    pub fn revalidate_against_verified_module(
        &self,
        verified: VerifiedKernelIrModuleV1<'_>,
    ) -> Result<()> {
        match self {
            Self::ExecutionDischarged(v) => v.revalidate_against_verified_module(verified),
            Self::Legacy(v) => {
                v.revalidate()
                    .map_err(ProductionFormalMemoryEvidenceErrorV5::Legacy)?;
                if canonical_identity(
                    verified.module(),
                    v.canonical_kernel_ir_identity().version(),
                )? != v.canonical_kernel_ir_identity()
                {
                    return Err(invalid("legacy current graph identity"));
                }
                revalidate_legacy_formal_memory_receipt_against_verified_module_v1(
                    verified,
                    0,
                    v.formal_obligation_receipt_bytes(),
                )
            }
        }
    }
}

/// Prevents a conflicting raw receipt stripped from V5 from taking a legacy path.
/// This checks graph custody/conflicts only, not the source-specific incomplete-reason proofs.
pub fn revalidate_legacy_formal_memory_receipt_against_verified_module_v1(
    verified: VerifiedKernelIrModuleV1<'_>,
    kernel_ordinal: usize,
    receipt_bytes: &[u8],
) -> Result<()> {
    let analysis = replay_obligations(verified, kernel_ordinal)?;
    let obligations = analysis.obligations();
    if !obligations.inter_invocation_conflicts().is_empty() {
        return Err(invalid("legacy raw conflicts"));
    }
    let receipt = InertFormalMemoryReceiptFormatV4::from_current_obligations(obligations)
        .map_err(external)?;
    if receipt.canonical_bytes() != receipt_bytes {
        return Err(invalid("legacy current raw obligations"));
    }
    Ok(())
}

fn replay_obligations(
    verified: VerifiedKernelIrModuleV1<'_>,
    ordinal: usize,
) -> Result<FormalMemoryObligationAnalysis> {
    let module = verified.module();
    let kernel = module
        .kernels
        .get(ordinal)
        .ok_or_else(|| invalid("kernel ordinal"))?;
    derive_kernel_memory_obligations_for_launch(
        module,
        &kernel.id,
        ExplicitLaunchExtent::Exact {
            rank: kernel.domain.rank(),
            extents: witness_extents(&kernel.domain),
        },
        FormalIndexWidth::Bits64,
    )
    .map_err(external)
}

fn validate_receipt_metadata(receipt: &InertFormalMemoryReceiptFormatV4, count: u64) -> Result<()> {
    let metadata = receipt.metadata();
    if metadata.encoding() == FormalMemoryReceiptEncodingV4::LegacyV2
        || metadata.index_width() != FormalIndexWidth::Bits64
        || metadata.analysis_basis()
            != FormalMemoryAnalysisBasis::CompilerDerivedIrWithUnauthenticatedLaunchInputs
        || metadata
            .invocations()
            .is_none_or(|range| range.start() != 0 || range.end_exclusive() != count)
    {
        return Err(invalid("raw witness metadata"));
    }
    Ok(())
}

fn canonical_identity(
    module: &Module,
    version: ProductionCanonicalKernelIrVersionV1,
) -> Result<ProductionCanonicalKernelIrIdentityV1> {
    // Each canonical codec applies its existing byte/record limits before decoding.
    let (digest, length) = match version {
        ProductionCanonicalKernelIrVersionV1::V8 => {
            let owner = VerifiedCanonicalKernelIrV8::from_canonical_bytes(
                encode_module_v8(module).map_err(external)?,
            )
            .map_err(external)?;
            (*owner.identity().digest(), owner.canonical_bytes().len())
        }
        ProductionCanonicalKernelIrVersionV1::V9 => {
            let owner = VerifiedCanonicalKernelIrV9::from_canonical_bytes(
                encode_module_v9(module).map_err(external)?,
            )
            .map_err(external)?;
            (*owner.identity().digest(), owner.canonical_bytes().len())
        }
        ProductionCanonicalKernelIrVersionV1::V11 => {
            let owner = VerifiedCanonicalKernelIrV11::from_canonical_bytes(
                encode_module_v11(module).map_err(external)?,
            )
            .map_err(external)?;
            (*owner.identity().digest(), owner.canonical_bytes().len())
        }
    };
    Ok(ProductionCanonicalKernelIrIdentityV1::from_canonical_parts(
        version,
        digest,
        length as u64,
    ))
}

fn encode(
    kir: ProductionCanonicalKernelIrIdentityV1,
    ordinal: usize,
    kernel: &str,
    entry: &str,
    extents: [u64; 3],
    receipt: &InertFormalMemoryReceiptFormatV4,
    rows: &[Discharge],
) -> Result<Vec<u8>> {
    validate_receipt_metadata(receipt, extents[0])?;
    let length = HEADER_BYTES
        .checked_add(kernel.len())
        .and_then(|n| n.checked_add(entry.len()))
        .and_then(|n| n.checked_add(receipt.canonical_bytes().len()))
        .and_then(|n| n.checked_add(rows.len().checked_mul(ROW_BYTES)?))
        .ok_or_else(|| invalid("length"))?;
    if length > MAX_FORMAL_MEMORY_ADMISSION_EVIDENCE_BYTES_V4
        || rows.is_empty()
        || rows.len() > MAX_EXECUTION_DISCHARGES_V1
        || kernel.is_empty()
        || entry.is_empty()
        || extents[0] == 0
        || extents[1..] != [1, 1]
    {
        return Err(invalid("encoding bounds"));
    }
    let mut out = Vec::new();
    out.try_reserve_exact(length)
        .map_err(|_| invalid("allocation"))?;
    out.extend_from_slice(&MAGIC);
    put16(&mut out, 5);
    put16(&mut out, 1);
    put32(&mut out, 0)?;
    put32(&mut out, length)?;
    put16(
        &mut out,
        match kir.version() {
            ProductionCanonicalKernelIrVersionV1::V8 => 8,
            ProductionCanonicalKernelIrVersionV1::V9 => 9,
            ProductionCanonicalKernelIrVersionV1::V11 => 11,
        },
    );
    put16(&mut out, 0);
    put64(&mut out, kir.canonical_length());
    out.extend_from_slice(kir.digest());
    out.extend_from_slice(receipt.identity_digest());
    put32(&mut out, ordinal)?;
    put16(&mut out, 1);
    put16(&mut out, 64);
    for value in extents {
        put64(&mut out, value);
    }
    put64(&mut out, extents[0]);
    put32(&mut out, rows.len())?;
    put32(&mut out, 0)?;
    put32(&mut out, rows.len())?;
    put32(&mut out, receipt.canonical_bytes().len())?;
    for name in [kernel, entry] {
        put32(&mut out, name.len())?;
        out.extend_from_slice(name.as_bytes());
    }
    out.extend_from_slice(receipt.canonical_bytes());
    for row in rows {
        put32(&mut out, row.conflict_ordinal as usize)?;
        put32(&mut out, row.allocation_parameter as usize)?;
        for location in [row.left, row.right] {
            put32(&mut out, location.block.0 as usize)?;
            put32(&mut out, location.operation_index)?;
        }
        for witness in [row.left_witness, row.right_witness] {
            let FormalGuardedPathV1::TrueEdge {
                source,
                ordinal,
                target,
            } = witness.path
            else {
                return Err(invalid("witness path"));
            };
            if witness.invocation != 0 {
                return Err(invalid("singleton invocation"));
            }
            put64(&mut out, witness.invocation);
            for value in [
                witness.index.0,
                witness.threshold.0,
                witness.predicate.0,
                1,
                source.0,
            ] {
                put32(&mut out, value as usize)?;
            }
            put32(&mut out, ordinal)?;
            put32(&mut out, target.0 as usize)?;
        }
    }
    if out.len() != length {
        return Err(invalid("encoded length"));
    }
    Ok(out)
}
fn put16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}
fn put32(out: &mut Vec<u8>, value: usize) -> Result<()> {
    out.extend_from_slice(
        &u32::try_from(value)
            .map_err(|_| invalid("u32 range"))?
            .to_le_bytes(),
    );
    Ok(())
}
fn put64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}
struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, length: usize) -> Result<&'a [u8]> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or_else(|| invalid("length"))?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| invalid("truncation"))?;
        self.offset = end;
        Ok(bytes)
    }
    fn fixed<const N: usize>(&mut self) -> Result<[u8; N]> {
        self.take(N)?.try_into().map_err(|_| invalid("fixed field"))
    }
    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }
    fn string(&mut self) -> Result<String> {
        let n = self.u32()? as usize;
        if n == 0 {
            return Err(invalid("empty name"));
        }
        Ok(std::str::from_utf8(self.take(n)?)
            .map_err(|_| invalid("UTF8"))?
            .to_owned())
    }
    fn location(&mut self) -> Result<FunctionOperationLocation> {
        Ok(FunctionOperationLocation::new(
            BlockId(self.u32()?),
            self.u32()? as usize,
        ))
    }
    fn witness(&mut self) -> Result<Witness> {
        let invocation = self.u64()?;
        let index = ValueId(self.u32()?);
        let threshold = ValueId(self.u32()?);
        let predicate = ValueId(self.u32()?);
        if invocation != 0 || self.u32()? != 1 {
            return Err(invalid("singleton witness"));
        }
        let path = FormalGuardedPathV1::TrueEdge {
            source: BlockId(self.u32()?),
            ordinal: self.u32()? as usize,
            target: BlockId(self.u32()?),
        };
        Ok(Witness {
            invocation,
            index,
            threshold,
            predicate,
            path,
        })
    }
}

#[cfg(test)]
#[path = "production_formal_memory_evidence_v5_tests.rs"]
mod tests;
