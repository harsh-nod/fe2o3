//! Native target-binding framing. Legacy V2 admission is deliberately unchanged.
//! Workgroup rows are in semantic-root order, not descriptor or graph order.
//! This bounded association codec grants no source, target, or runtime authority.

use std::{collections::BTreeSet, ops::Range, str};

use fe2o3_amd_target::ProductionAmdTargetProfileV1;
use fe2o3_kernel_descriptor::{MAX_KERNELS, MAX_NAME_BYTES};

use crate::{
    ASSOCIATION_ONLY_NO_REFINEMENT_PROOF_POLICY_V3, InertNativeNeutralSubjectV1,
    MAX_PRODUCTION_TARGET_LINEAGE_TRANSCRIPT_BYTES_V3, NATIVE_NEUTRAL_SUBJECT_BYTES_V1,
    ProductionTargetLineageErrorV3, TargetLineageClaimV3, TargetLineageIdentityV3,
};

/// Magic prefix for the canonical multi-root target-binding transcript.
pub const MULTI_ROOT_TARGET_BINDING_MAGIC_V3: [u8; 8] = *b"F2MRTGT3";
/// Wire version for the canonical multi-root target-binding transcript.
pub const MULTI_ROOT_TARGET_BINDING_VERSION_V3: u16 = 3;
/// Maximum number of roots admitted by one multi-root target transcript.
pub const MAX_MULTI_ROOT_TARGET_BINDING_ROOTS_V3: usize = MAX_KERNELS;

const HEADER_BYTES_V3: usize = 16;
const IDENTITY_BYTES_V3: usize = 40;
const CODE_OBJECT_VERSION_V3: u16 = 6;
const WAVE_WIDTH_BITS_V3: u16 = 64;
const MAX_TARGET_TEXT_BYTES_V3: usize = 256;
const MAX_TARGET_FEATURES_BYTES_V3: usize = 4 * 1024;

#[derive(Clone, Copy, Debug)]
/// One borrowed kernel/workgroup entry supplied to the canonical encoder.
pub struct MultiRootTargetWorkgroupInputV3<'a> {
    /// Stable kernel identifier in semantic-root order.
    pub kernel: &'a str,
    /// Exact default workgroup dimensions for this kernel.
    pub workgroup: [u32; 3],
}

#[derive(Clone, Copy, Debug)]
/// Borrowed inputs for a canonical multi-root target-binding transcript.
pub struct MultiRootTargetBindingInputsV3<'a> {
    /// Protected rustc invocation identity.
    pub protected_rustc_invocation: TargetLineageIdentityV3,
    /// Canonical semantic MIR identity shared by every root.
    pub semantic_mir: TargetLineageIdentityV3,
    /// Full native V12 graph-plus-catalog subject, never a legacy graph identity.
    pub native_neutral_subject: InertNativeNeutralSubjectV1,
    /// Target-bound canonical Kernel IR identity.
    pub target_bound_kir: TargetLineageIdentityV3,
    /// Exact configured AMDHSA target ID.
    pub configured_target: &'a str,
    /// Exact rustc LLVM target triple.
    pub rustc_llvm_target: &'a str,
    /// Exact LLVM processor.
    pub target_cpu: &'a str,
    /// Exact active target-feature string.
    pub target_features: &'a str,
    /// Canonical identity of the ordered compiler roster.
    pub roster_identity: [u8; 32],
    /// AMDHSA code-object version.
    pub code_object_version: u16,
    /// Required wavefront width in bits.
    pub wave_width_bits: u16,
    /// Per-root workgroups in semantic-root order.
    pub workgroups: &'a [MultiRootTargetWorkgroupInputV3<'a>],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// One decoded kernel/workgroup entry borrowed from a canonical transcript.
pub struct MultiRootTargetWorkgroupV3<'a> {
    kernel: &'a str,
    workgroup: [u32; 3],
}

impl<'a> MultiRootTargetWorkgroupV3<'a> {
    /// Returns the stable kernel identifier.
    pub const fn kernel(self) -> &'a str {
        self.kernel
    }

    /// Returns the exact default workgroup dimensions.
    pub const fn workgroup(self) -> [u32; 3] {
        self.workgroup
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TextRangeV3 {
    start: usize,
    end: usize,
}

impl From<Range<usize>> for TextRangeV3 {
    fn from(range: Range<usize>) -> Self {
        Self {
            start: range.start,
            end: range.end,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StoredWorkgroupV3 {
    kernel: TextRangeV3,
    workgroup: [u32; 3],
}

#[derive(Debug, Eq, PartialEq)]
/// Canonical, bounded multi-root target-binding association transcript.
pub struct MultiRootTargetBindingTranscriptV3 {
    canonical_bytes: Box<[u8]>,
    protected_rustc_invocation: TargetLineageIdentityV3,
    semantic_mir: TargetLineageIdentityV3,
    native_neutral_subject: InertNativeNeutralSubjectV1,
    target_bound_kir: TargetLineageIdentityV3,
    roster_identity: [u8; 32],
    code_object_version: u16,
    wave_width_bits: u16,
    configured_target: TextRangeV3,
    rustc_llvm_target: TextRangeV3,
    target_cpu: TextRangeV3,
    target_features: TextRangeV3,
    workgroups: Box<[StoredWorkgroupV3]>,
}

impl MultiRootTargetBindingTranscriptV3 {
    /// Builds and validates the exact canonical multi-root wire record.
    pub fn new(
        inputs: MultiRootTargetBindingInputsV3<'_>,
    ) -> Result<Self, ProductionTargetLineageErrorV3> {
        validate_inputs_v3(&inputs)?;

        let mut capacity = HEADER_BYTES_V3
            .checked_add(3 * IDENTITY_BYTES_V3 + NATIVE_NEUTRAL_SUBJECT_BYTES_V1)
            .and_then(|value| value.checked_add(32 + 4))
            .ok_or(ProductionTargetLineageErrorV3::LengthOverflow)?;
        for text in [
            inputs.configured_target,
            inputs.rustc_llvm_target,
            inputs.target_cpu,
            inputs.target_features,
        ] {
            capacity = capacity
                .checked_add(4)
                .and_then(|value| value.checked_add(text.len()))
                .ok_or(ProductionTargetLineageErrorV3::LengthOverflow)?;
        }
        capacity = capacity
            .checked_add(4)
            .ok_or(ProductionTargetLineageErrorV3::LengthOverflow)?;
        for entry in inputs.workgroups {
            capacity = capacity
                .checked_add(4 + 12)
                .and_then(|value| value.checked_add(entry.kernel.len()))
                .ok_or(ProductionTargetLineageErrorV3::LengthOverflow)?;
        }
        if capacity > MAX_PRODUCTION_TARGET_LINEAGE_TRANSCRIPT_BYTES_V3 {
            return Err(ProductionTargetLineageErrorV3::TranscriptTooLarge {
                actual: capacity,
                max: MAX_PRODUCTION_TARGET_LINEAGE_TRANSCRIPT_BYTES_V3,
            });
        }

        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(capacity)
            .map_err(|_| ProductionTargetLineageErrorV3::AllocationFailed)?;
        bytes.extend_from_slice(&MULTI_ROOT_TARGET_BINDING_MAGIC_V3);
        bytes.extend_from_slice(&MULTI_ROOT_TARGET_BINDING_VERSION_V3.to_le_bytes());
        bytes.extend_from_slice(&ASSOCIATION_ONLY_NO_REFINEMENT_PROOF_POLICY_V3.to_le_bytes());
        bytes.extend_from_slice(
            &u32::try_from(capacity)
                .map_err(|_| ProductionTargetLineageErrorV3::LengthOverflow)?
                .to_le_bytes(),
        );
        bytes.extend_from_slice(&inputs.protected_rustc_invocation.encode());
        bytes.extend_from_slice(&inputs.semantic_mir.encode());
        bytes.extend_from_slice(inputs.native_neutral_subject.canonical_bytes());
        bytes.extend_from_slice(&inputs.target_bound_kir.encode());
        bytes.extend_from_slice(&inputs.roster_identity);
        bytes.extend_from_slice(&inputs.code_object_version.to_le_bytes());
        bytes.extend_from_slice(&inputs.wave_width_bits.to_le_bytes());
        for text in [
            inputs.configured_target,
            inputs.rustc_llvm_target,
            inputs.target_cpu,
            inputs.target_features,
        ] {
            push_text_v3(&mut bytes, text)?;
        }
        bytes.extend_from_slice(
            &u32::try_from(inputs.workgroups.len())
                .map_err(|_| ProductionTargetLineageErrorV3::LengthOverflow)?
                .to_le_bytes(),
        );
        for entry in inputs.workgroups {
            push_text_v3(&mut bytes, entry.kernel)?;
            for dimension in entry.workgroup {
                bytes.extend_from_slice(&dimension.to_le_bytes());
            }
        }
        debug_assert_eq!(bytes.len(), capacity);
        Self::decode_owned(bytes)
    }

    /// Strictly decodes, bounds, and revalidates an untrusted transcript.
    pub fn decode(bytes: &[u8]) -> Result<Self, ProductionTargetLineageErrorV3> {
        if bytes.len() > MAX_PRODUCTION_TARGET_LINEAGE_TRANSCRIPT_BYTES_V3 {
            return Err(ProductionTargetLineageErrorV3::TranscriptTooLarge {
                actual: bytes.len(),
                max: MAX_PRODUCTION_TARGET_LINEAGE_TRANSCRIPT_BYTES_V3,
            });
        }
        let mut owned = Vec::new();
        owned
            .try_reserve_exact(bytes.len())
            .map_err(|_| ProductionTargetLineageErrorV3::AllocationFailed)?;
        owned.extend_from_slice(bytes);
        Self::decode_owned(owned)
    }

    fn decode_owned(bytes: Vec<u8>) -> Result<Self, ProductionTargetLineageErrorV3> {
        let decoded = DecodedTranscriptV3::decode(&bytes)?;
        Ok(Self {
            canonical_bytes: bytes.into_boxed_slice(),
            protected_rustc_invocation: decoded.protected_rustc_invocation,
            semantic_mir: decoded.semantic_mir,
            native_neutral_subject: decoded.native_neutral_subject,
            target_bound_kir: decoded.target_bound_kir,
            roster_identity: decoded.roster_identity,
            code_object_version: decoded.code_object_version,
            wave_width_bits: decoded.wave_width_bits,
            configured_target: decoded.configured_target,
            rustc_llvm_target: decoded.rustc_llvm_target,
            target_cpu: decoded.target_cpu,
            target_features: decoded.target_features,
            workgroups: decoded.workgroups,
        })
    }

    /// Returns the exact canonical transcript bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Transfers the exact canonical transcript bytes without copying them.
    pub fn into_canonical_bytes(self) -> Vec<u8> {
        self.canonical_bytes.into_vec()
    }

    /// Returns the protected rustc invocation identity.
    pub const fn protected_rustc_invocation(&self) -> TargetLineageIdentityV3 {
        self.protected_rustc_invocation
    }

    /// Returns the shared canonical semantic MIR identity.
    pub const fn semantic_mir(&self) -> TargetLineageIdentityV3 {
        self.semantic_mir
    }

    /// Returns the complete native graph-plus-catalog subject.
    pub const fn native_neutral_subject(&self) -> &InertNativeNeutralSubjectV1 {
        &self.native_neutral_subject
    }

    /// Returns the target-bound canonical Kernel IR identity.
    pub const fn target_bound_kir(&self) -> TargetLineageIdentityV3 {
        self.target_bound_kir
    }

    /// Returns the canonical compiler-roster identity.
    pub const fn roster_identity(&self) -> [u8; 32] {
        self.roster_identity
    }

    /// Returns the AMDHSA code-object version.
    pub const fn code_object_version(&self) -> u16 {
        self.code_object_version
    }

    /// Returns the required wavefront width in bits.
    pub const fn wave_width_bits(&self) -> u16 {
        self.wave_width_bits
    }

    /// Returns the exact configured AMDHSA target ID.
    pub fn configured_target(&self) -> &str {
        self.text(self.configured_target)
    }

    /// Returns the exact rustc LLVM target triple.
    pub fn rustc_llvm_target(&self) -> &str {
        self.text(self.rustc_llvm_target)
    }

    /// Returns the exact LLVM processor.
    pub fn target_cpu(&self) -> &str {
        self.text(self.target_cpu)
    }

    /// Returns the exact active target-feature string.
    pub fn target_features(&self) -> &str {
        self.text(self.target_features)
    }

    /// Returns the number of semantic roots represented by the transcript.
    pub fn root_count(&self) -> usize {
        self.workgroups.len()
    }

    /// Returns one root by semantic-root ordinal.
    pub fn workgroup(&self, index: usize) -> Option<MultiRootTargetWorkgroupV3<'_>> {
        self.workgroups
            .get(index)
            .map(|entry| MultiRootTargetWorkgroupV3 {
                kernel: self.text(entry.kernel),
                workgroup: entry.workgroup,
            })
    }

    /// Returns the deliberately limited semantic claim carried by this record.
    pub const fn claim(&self) -> TargetLineageClaimV3 {
        TargetLineageClaimV3::AssociationOnlyNoRefinementProof
    }

    /// Reports that this association transcript is not a refinement proof.
    pub const fn establishes_refinement_proof(&self) -> bool {
        false
    }

    fn text(&self, range: TextRangeV3) -> &str {
        str::from_utf8(&self.canonical_bytes[range.start..range.end])
            .expect("strict decoder retained canonical ASCII text")
    }
}

struct DecodedTranscriptV3 {
    protected_rustc_invocation: TargetLineageIdentityV3,
    semantic_mir: TargetLineageIdentityV3,
    native_neutral_subject: InertNativeNeutralSubjectV1,
    target_bound_kir: TargetLineageIdentityV3,
    roster_identity: [u8; 32],
    code_object_version: u16,
    wave_width_bits: u16,
    configured_target: TextRangeV3,
    rustc_llvm_target: TextRangeV3,
    target_cpu: TextRangeV3,
    target_features: TextRangeV3,
    workgroups: Box<[StoredWorkgroupV3]>,
}

impl DecodedTranscriptV3 {
    fn decode(bytes: &[u8]) -> Result<Self, ProductionTargetLineageErrorV3> {
        if bytes.len() > MAX_PRODUCTION_TARGET_LINEAGE_TRANSCRIPT_BYTES_V3 {
            return Err(ProductionTargetLineageErrorV3::TranscriptTooLarge {
                actual: bytes.len(),
                max: MAX_PRODUCTION_TARGET_LINEAGE_TRANSCRIPT_BYTES_V3,
            });
        }
        let mut reader = ReaderV3::new(bytes);
        if reader.take(8)? != MULTI_ROOT_TARGET_BINDING_MAGIC_V3 {
            return Err(ProductionTargetLineageErrorV3::InvalidMagic);
        }
        let version = reader.u16()?;
        if version != MULTI_ROOT_TARGET_BINDING_VERSION_V3 {
            return Err(ProductionTargetLineageErrorV3::UnsupportedVersion { observed: version });
        }
        let policy = reader.u16()?;
        if policy != ASSOCIATION_ONLY_NO_REFINEMENT_PROOF_POLICY_V3 {
            return Err(ProductionTargetLineageErrorV3::WrongPolicy {
                expected: ASSOCIATION_ONLY_NO_REFINEMENT_PROOF_POLICY_V3,
                observed: policy,
            });
        }
        let declared = reader.u32()? as usize;
        if declared != bytes.len() {
            return Err(ProductionTargetLineageErrorV3::DeclaredLengthMismatch {
                declared,
                actual: bytes.len(),
            });
        }

        let protected_rustc_invocation = reader.identity("protected rustc invocation identity")?;
        let semantic_mir = reader.identity("semantic MIR identity")?;
        let native_neutral_subject =
            InertNativeNeutralSubjectV1::decode(reader.take(NATIVE_NEUTRAL_SUBJECT_BYTES_V1)?)
                .map_err(|_| ProductionTargetLineageErrorV3::AssociationInvariant {
                    detail: "invalid native graph-plus-catalog subject",
                })?;
        let target_bound_kir = reader.identity("target-bound Kernel IR identity")?;
        if native_neutral_subject.graph_digest() == &target_bound_kir.sha256()
            && native_neutral_subject.graph_length() == target_bound_kir.byte_len()
        {
            return Err(ProductionTargetLineageErrorV3::AssociationInvariant {
                detail: "target-neutral and target-bound Kernel IR identities must differ",
            });
        }

        let mut roster_identity = [0_u8; 32];
        roster_identity.copy_from_slice(reader.take(32)?);
        if roster_identity == [0; 32] {
            return Err(ProductionTargetLineageErrorV3::ZeroIdentity {
                field: "compiler roster identity",
            });
        }
        let code_object_version = reader.u16()?;
        if code_object_version != CODE_OBJECT_VERSION_V3 {
            return Err(ProductionTargetLineageErrorV3::InvalidInteger {
                field: "code object version",
                observed: u64::from(code_object_version),
            });
        }
        let wave_width_bits = reader.u16()?;
        if wave_width_bits != WAVE_WIDTH_BITS_V3 {
            return Err(ProductionTargetLineageErrorV3::InvalidInteger {
                field: "wave width",
                observed: u64::from(wave_width_bits),
            });
        }

        let configured_target = reader.text("configured target", MAX_TARGET_TEXT_BYTES_V3)?;
        let rustc_llvm_target = reader.text("rustc LLVM target", MAX_TARGET_TEXT_BYTES_V3)?;
        let target_cpu = reader.text("target CPU", MAX_TARGET_TEXT_BYTES_V3)?;
        let target_features = reader.text("target features", MAX_TARGET_FEATURES_BYTES_V3)?;
        validate_target_profile_v3(
            reader.text_at(configured_target),
            reader.text_at(rustc_llvm_target),
            reader.text_at(target_cpu),
            reader.text_at(target_features),
        )?;

        let root_count = reader.u32()? as usize;
        if !(1..=MAX_MULTI_ROOT_TARGET_BINDING_ROOTS_V3).contains(&root_count) {
            return Err(ProductionTargetLineageErrorV3::InvalidInteger {
                field: "multi-root target root count",
                observed: root_count as u64,
            });
        }
        let mut workgroups = Vec::new();
        workgroups
            .try_reserve_exact(root_count)
            .map_err(|_| ProductionTargetLineageErrorV3::AllocationFailed)?;
        let mut kernels = BTreeSet::new();
        for _ in 0..root_count {
            let kernel = reader.text("kernel identifier", MAX_NAME_BYTES)?;
            if !kernels.insert(reader.text_at(kernel)) {
                return Err(ProductionTargetLineageErrorV3::AssociationInvariant {
                    detail: "multi-root target kernel identifiers must be unique",
                });
            }
            let workgroup = [reader.u32()?, reader.u32()?, reader.u32()?];
            if workgroup.contains(&0) {
                return Err(ProductionTargetLineageErrorV3::InvalidInteger {
                    field: "default workgroup",
                    observed: 0,
                });
            }
            workgroups.push(StoredWorkgroupV3 { kernel, workgroup });
        }
        if !reader.is_finished() {
            return Err(ProductionTargetLineageErrorV3::TrailingBytes {
                trailing: bytes.len() - reader.offset,
            });
        }

        Ok(Self {
            protected_rustc_invocation,
            semantic_mir,
            native_neutral_subject,
            target_bound_kir,
            roster_identity,
            code_object_version,
            wave_width_bits,
            configured_target,
            rustc_llvm_target,
            target_cpu,
            target_features,
            workgroups: workgroups.into_boxed_slice(),
        })
    }
}

struct ReaderV3<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ReaderV3<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], ProductionTargetLineageErrorV3> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(ProductionTargetLineageErrorV3::LengthOverflow)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(ProductionTargetLineageErrorV3::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn u16(&mut self) -> Result<u16, ProductionTargetLineageErrorV3> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    fn u32(&mut self) -> Result<u32, ProductionTargetLineageErrorV3> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn identity(
        &mut self,
        field: &'static str,
    ) -> Result<TargetLineageIdentityV3, ProductionTargetLineageErrorV3> {
        TargetLineageIdentityV3::decode(field, self.take(IDENTITY_BYTES_V3)?)
    }

    fn text(
        &mut self,
        field: &'static str,
        max: usize,
    ) -> Result<TextRangeV3, ProductionTargetLineageErrorV3> {
        let length = self.u32()? as usize;
        if length == 0 {
            return Err(ProductionTargetLineageErrorV3::EmptyField { field });
        }
        if length > max {
            return Err(ProductionTargetLineageErrorV3::FieldTooLarge {
                field,
                actual: length,
                max,
            });
        }
        let start = self.offset;
        let bytes = self.take(length)?;
        validate_ascii_token_v3(field, bytes)?;
        Ok((start..self.offset).into())
    }

    fn text_at(&self, range: TextRangeV3) -> &'a str {
        str::from_utf8(&self.bytes[range.start..range.end])
            .expect("strict reader retained canonical ASCII text")
    }

    fn is_finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

fn validate_inputs_v3(
    inputs: &MultiRootTargetBindingInputsV3<'_>,
) -> Result<(), ProductionTargetLineageErrorV3> {
    if inputs.roster_identity == [0; 32] {
        return Err(ProductionTargetLineageErrorV3::ZeroIdentity {
            field: "compiler roster identity",
        });
    }
    if inputs.native_neutral_subject.graph_digest() == &inputs.target_bound_kir.sha256()
        && inputs.native_neutral_subject.graph_length() == inputs.target_bound_kir.byte_len()
    {
        return Err(ProductionTargetLineageErrorV3::AssociationInvariant {
            detail: "target-neutral and target-bound Kernel IR identities must differ",
        });
    }
    if inputs.code_object_version != CODE_OBJECT_VERSION_V3 {
        return Err(ProductionTargetLineageErrorV3::InvalidInteger {
            field: "code object version",
            observed: u64::from(inputs.code_object_version),
        });
    }
    if inputs.wave_width_bits != WAVE_WIDTH_BITS_V3 {
        return Err(ProductionTargetLineageErrorV3::InvalidInteger {
            field: "wave width",
            observed: u64::from(inputs.wave_width_bits),
        });
    }
    validate_bounded_ascii_token_v3(
        "configured target",
        inputs.configured_target,
        MAX_TARGET_TEXT_BYTES_V3,
    )?;
    validate_bounded_ascii_token_v3(
        "rustc LLVM target",
        inputs.rustc_llvm_target,
        MAX_TARGET_TEXT_BYTES_V3,
    )?;
    validate_bounded_ascii_token_v3("target CPU", inputs.target_cpu, MAX_TARGET_TEXT_BYTES_V3)?;
    validate_bounded_ascii_token_v3(
        "target features",
        inputs.target_features,
        MAX_TARGET_FEATURES_BYTES_V3,
    )?;
    validate_target_profile_v3(
        inputs.configured_target,
        inputs.rustc_llvm_target,
        inputs.target_cpu,
        inputs.target_features,
    )?;
    if !(1..=MAX_MULTI_ROOT_TARGET_BINDING_ROOTS_V3).contains(&inputs.workgroups.len()) {
        return Err(ProductionTargetLineageErrorV3::InvalidInteger {
            field: "multi-root target root count",
            observed: inputs.workgroups.len() as u64,
        });
    }
    let mut kernels = BTreeSet::new();
    for entry in inputs.workgroups {
        validate_bounded_ascii_token_v3("kernel identifier", entry.kernel, MAX_NAME_BYTES)?;
        if !kernels.insert(entry.kernel) {
            return Err(ProductionTargetLineageErrorV3::AssociationInvariant {
                detail: "multi-root target kernel identifiers must be unique",
            });
        }
        if entry.workgroup.contains(&0) {
            return Err(ProductionTargetLineageErrorV3::InvalidInteger {
                field: "default workgroup",
                observed: 0,
            });
        }
    }
    Ok(())
}

fn validate_target_profile_v3(
    configured_target: &str,
    rustc_llvm_target: &str,
    target_cpu: &str,
    target_features: &str,
) -> Result<(), ProductionTargetLineageErrorV3> {
    let profile = ProductionAmdTargetProfileV1::from_device_target(configured_target).ok_or(
        ProductionTargetLineageErrorV3::ExactValueMismatch {
            field: "configured target",
        },
    )?;
    if profile.rustc_target() != rustc_llvm_target {
        return Err(ProductionTargetLineageErrorV3::ExactValueMismatch {
            field: "rustc LLVM target",
        });
    }
    if profile.cpu() != target_cpu {
        return Err(ProductionTargetLineageErrorV3::ExactValueMismatch {
            field: "configured target and target CPU",
        });
    }
    if profile.rustc_features() != target_features {
        return Err(ProductionTargetLineageErrorV3::ExactValueMismatch {
            field: "target features",
        });
    }
    Ok(())
}

fn validate_bounded_ascii_token_v3(
    field: &'static str,
    text: &str,
    max: usize,
) -> Result<(), ProductionTargetLineageErrorV3> {
    if text.len() > max {
        return Err(ProductionTargetLineageErrorV3::FieldTooLarge {
            field,
            actual: text.len(),
            max,
        });
    }
    validate_ascii_token_v3(field, text.as_bytes())
}

fn validate_ascii_token_v3(
    field: &'static str,
    bytes: &[u8],
) -> Result<(), ProductionTargetLineageErrorV3> {
    let text =
        str::from_utf8(bytes).map_err(|_| ProductionTargetLineageErrorV3::InvalidText { field })?;
    if text.is_empty()
        || !text.is_ascii()
        || text
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
    {
        return Err(ProductionTargetLineageErrorV3::InvalidText { field });
    }
    Ok(())
}

fn push_text_v3(bytes: &mut Vec<u8>, text: &str) -> Result<(), ProductionTargetLineageErrorV3> {
    bytes.extend_from_slice(
        &u32::try_from(text.len())
            .map_err(|_| ProductionTargetLineageErrorV3::LengthOverflow)?
            .to_le_bytes(),
    );
    bytes.extend_from_slice(text.as_bytes());
    Ok(())
}

#[cfg(test)]
#[path = "multi_root_target_lineage_v3_tests.rs"]
mod tests;
