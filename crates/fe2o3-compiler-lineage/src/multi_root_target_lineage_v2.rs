use std::{collections::BTreeSet, ops::Range, str};

use fe2o3_amd_target::ProductionAmdTargetProfileV1;
use fe2o3_kernel_descriptor::{MAX_KERNELS, MAX_NAME_BYTES};

use crate::{
    ASSOCIATION_ONLY_NO_REFINEMENT_PROOF_POLICY_V3,
    MAX_PRODUCTION_TARGET_LINEAGE_TRANSCRIPT_BYTES_V3, ProductionTargetLineageErrorV3,
    TargetLineageClaimV3, TargetLineageIdentityV3,
};

/// Magic prefix for the canonical multi-root target-binding transcript.
pub const MULTI_ROOT_TARGET_BINDING_MAGIC_V2: [u8; 8] = *b"F2MRTGT2";
/// Wire version for the canonical multi-root target-binding transcript.
pub const MULTI_ROOT_TARGET_BINDING_VERSION_V2: u16 = 2;
/// Maximum number of roots admitted by one multi-root target transcript.
pub const MAX_MULTI_ROOT_TARGET_BINDING_ROOTS_V2: usize = MAX_KERNELS;

const HEADER_BYTES_V2: usize = 16;
const IDENTITY_BYTES_V2: usize = 40;
const CODE_OBJECT_VERSION_V2: u16 = 6;
const WAVE_WIDTH_BITS_V2: u16 = 64;
const MAX_TARGET_TEXT_BYTES_V2: usize = 256;
const MAX_TARGET_FEATURES_BYTES_V2: usize = 4 * 1024;

#[derive(Clone, Copy, Debug)]
/// One borrowed kernel/workgroup entry supplied to the canonical encoder.
pub struct MultiRootTargetWorkgroupInputV2<'a> {
    /// Stable kernel identifier in semantic-root order.
    pub kernel: &'a str,
    /// Exact default workgroup dimensions for this kernel.
    pub workgroup: [u32; 3],
}

#[derive(Clone, Copy, Debug)]
/// Borrowed inputs for a canonical multi-root target-binding transcript.
pub struct MultiRootTargetBindingInputsV2<'a> {
    /// Protected rustc invocation identity.
    pub protected_rustc_invocation: TargetLineageIdentityV3,
    /// Canonical semantic MIR identity shared by every root.
    pub semantic_mir: TargetLineageIdentityV3,
    /// Target-neutral canonical Kernel IR identity.
    pub target_neutral_kir: TargetLineageIdentityV3,
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
    pub workgroups: &'a [MultiRootTargetWorkgroupInputV2<'a>],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// One decoded kernel/workgroup entry borrowed from a canonical transcript.
pub struct MultiRootTargetWorkgroupV2<'a> {
    kernel: &'a str,
    workgroup: [u32; 3],
}

impl<'a> MultiRootTargetWorkgroupV2<'a> {
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
struct TextRangeV2 {
    start: usize,
    end: usize,
}

impl From<Range<usize>> for TextRangeV2 {
    fn from(range: Range<usize>) -> Self {
        Self {
            start: range.start,
            end: range.end,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StoredWorkgroupV2 {
    kernel: TextRangeV2,
    workgroup: [u32; 3],
}

#[derive(Debug, Eq, PartialEq)]
/// Canonical, bounded multi-root target-binding association transcript.
pub struct MultiRootTargetBindingTranscriptV2 {
    canonical_bytes: Box<[u8]>,
    protected_rustc_invocation: TargetLineageIdentityV3,
    semantic_mir: TargetLineageIdentityV3,
    target_neutral_kir: TargetLineageIdentityV3,
    target_bound_kir: TargetLineageIdentityV3,
    roster_identity: [u8; 32],
    code_object_version: u16,
    wave_width_bits: u16,
    configured_target: TextRangeV2,
    rustc_llvm_target: TextRangeV2,
    target_cpu: TextRangeV2,
    target_features: TextRangeV2,
    workgroups: Box<[StoredWorkgroupV2]>,
}

impl MultiRootTargetBindingTranscriptV2 {
    /// Builds and validates the exact canonical multi-root wire record.
    pub fn new(
        inputs: MultiRootTargetBindingInputsV2<'_>,
    ) -> Result<Self, ProductionTargetLineageErrorV3> {
        validate_inputs_v2(&inputs)?;

        let mut capacity = HEADER_BYTES_V2
            .checked_add(4 * IDENTITY_BYTES_V2)
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
        bytes.extend_from_slice(&MULTI_ROOT_TARGET_BINDING_MAGIC_V2);
        bytes.extend_from_slice(&MULTI_ROOT_TARGET_BINDING_VERSION_V2.to_le_bytes());
        bytes.extend_from_slice(&ASSOCIATION_ONLY_NO_REFINEMENT_PROOF_POLICY_V3.to_le_bytes());
        bytes.extend_from_slice(
            &u32::try_from(capacity)
                .map_err(|_| ProductionTargetLineageErrorV3::LengthOverflow)?
                .to_le_bytes(),
        );
        for identity in [
            inputs.protected_rustc_invocation,
            inputs.semantic_mir,
            inputs.target_neutral_kir,
            inputs.target_bound_kir,
        ] {
            bytes.extend_from_slice(&identity.encode());
        }
        bytes.extend_from_slice(&inputs.roster_identity);
        bytes.extend_from_slice(&inputs.code_object_version.to_le_bytes());
        bytes.extend_from_slice(&inputs.wave_width_bits.to_le_bytes());
        for text in [
            inputs.configured_target,
            inputs.rustc_llvm_target,
            inputs.target_cpu,
            inputs.target_features,
        ] {
            push_text_v2(&mut bytes, text)?;
        }
        bytes.extend_from_slice(
            &u32::try_from(inputs.workgroups.len())
                .map_err(|_| ProductionTargetLineageErrorV3::LengthOverflow)?
                .to_le_bytes(),
        );
        for entry in inputs.workgroups {
            push_text_v2(&mut bytes, entry.kernel)?;
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
        let decoded = DecodedTranscriptV2::decode(&bytes)?;
        Ok(Self {
            canonical_bytes: bytes.into_boxed_slice(),
            protected_rustc_invocation: decoded.protected_rustc_invocation,
            semantic_mir: decoded.semantic_mir,
            target_neutral_kir: decoded.target_neutral_kir,
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

    /// Returns the target-neutral canonical Kernel IR identity.
    pub const fn target_neutral_kir(&self) -> TargetLineageIdentityV3 {
        self.target_neutral_kir
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
    pub fn workgroup(&self, index: usize) -> Option<MultiRootTargetWorkgroupV2<'_>> {
        self.workgroups
            .get(index)
            .map(|entry| MultiRootTargetWorkgroupV2 {
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

    fn text(&self, range: TextRangeV2) -> &str {
        str::from_utf8(&self.canonical_bytes[range.start..range.end])
            .expect("strict decoder retained canonical ASCII text")
    }
}

struct DecodedTranscriptV2 {
    protected_rustc_invocation: TargetLineageIdentityV3,
    semantic_mir: TargetLineageIdentityV3,
    target_neutral_kir: TargetLineageIdentityV3,
    target_bound_kir: TargetLineageIdentityV3,
    roster_identity: [u8; 32],
    code_object_version: u16,
    wave_width_bits: u16,
    configured_target: TextRangeV2,
    rustc_llvm_target: TextRangeV2,
    target_cpu: TextRangeV2,
    target_features: TextRangeV2,
    workgroups: Box<[StoredWorkgroupV2]>,
}

impl DecodedTranscriptV2 {
    fn decode(bytes: &[u8]) -> Result<Self, ProductionTargetLineageErrorV3> {
        if bytes.len() > MAX_PRODUCTION_TARGET_LINEAGE_TRANSCRIPT_BYTES_V3 {
            return Err(ProductionTargetLineageErrorV3::TranscriptTooLarge {
                actual: bytes.len(),
                max: MAX_PRODUCTION_TARGET_LINEAGE_TRANSCRIPT_BYTES_V3,
            });
        }
        let mut reader = ReaderV2::new(bytes);
        if reader.take(8)? != MULTI_ROOT_TARGET_BINDING_MAGIC_V2 {
            return Err(ProductionTargetLineageErrorV3::InvalidMagic);
        }
        let version = reader.u16()?;
        if version != MULTI_ROOT_TARGET_BINDING_VERSION_V2 {
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
        let target_neutral_kir = reader.identity("target-neutral Kernel IR identity")?;
        let target_bound_kir = reader.identity("target-bound Kernel IR identity")?;
        if target_neutral_kir == target_bound_kir {
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
        if code_object_version != CODE_OBJECT_VERSION_V2 {
            return Err(ProductionTargetLineageErrorV3::InvalidInteger {
                field: "code object version",
                observed: u64::from(code_object_version),
            });
        }
        let wave_width_bits = reader.u16()?;
        if wave_width_bits != WAVE_WIDTH_BITS_V2 {
            return Err(ProductionTargetLineageErrorV3::InvalidInteger {
                field: "wave width",
                observed: u64::from(wave_width_bits),
            });
        }

        let configured_target = reader.text("configured target", MAX_TARGET_TEXT_BYTES_V2)?;
        let rustc_llvm_target = reader.text("rustc LLVM target", MAX_TARGET_TEXT_BYTES_V2)?;
        let target_cpu = reader.text("target CPU", MAX_TARGET_TEXT_BYTES_V2)?;
        let target_features = reader.text("target features", MAX_TARGET_FEATURES_BYTES_V2)?;
        validate_target_profile_v2(
            reader.text_at(configured_target),
            reader.text_at(rustc_llvm_target),
            reader.text_at(target_cpu),
            reader.text_at(target_features),
        )?;

        let root_count = reader.u32()? as usize;
        if !(2..=MAX_MULTI_ROOT_TARGET_BINDING_ROOTS_V2).contains(&root_count) {
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
            workgroups.push(StoredWorkgroupV2 { kernel, workgroup });
        }
        if !reader.is_finished() {
            return Err(ProductionTargetLineageErrorV3::TrailingBytes {
                trailing: bytes.len() - reader.offset,
            });
        }

        Ok(Self {
            protected_rustc_invocation,
            semantic_mir,
            target_neutral_kir,
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

struct ReaderV2<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ReaderV2<'a> {
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
        TargetLineageIdentityV3::decode(field, self.take(IDENTITY_BYTES_V2)?)
    }

    fn text(
        &mut self,
        field: &'static str,
        max: usize,
    ) -> Result<TextRangeV2, ProductionTargetLineageErrorV3> {
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
        validate_ascii_token_v2(field, bytes)?;
        Ok((start..self.offset).into())
    }

    fn text_at(&self, range: TextRangeV2) -> &'a str {
        str::from_utf8(&self.bytes[range.start..range.end])
            .expect("strict reader retained canonical ASCII text")
    }

    fn is_finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

fn validate_inputs_v2(
    inputs: &MultiRootTargetBindingInputsV2<'_>,
) -> Result<(), ProductionTargetLineageErrorV3> {
    if inputs.roster_identity == [0; 32] {
        return Err(ProductionTargetLineageErrorV3::ZeroIdentity {
            field: "compiler roster identity",
        });
    }
    if inputs.target_neutral_kir == inputs.target_bound_kir {
        return Err(ProductionTargetLineageErrorV3::AssociationInvariant {
            detail: "target-neutral and target-bound Kernel IR identities must differ",
        });
    }
    if inputs.code_object_version != CODE_OBJECT_VERSION_V2 {
        return Err(ProductionTargetLineageErrorV3::InvalidInteger {
            field: "code object version",
            observed: u64::from(inputs.code_object_version),
        });
    }
    if inputs.wave_width_bits != WAVE_WIDTH_BITS_V2 {
        return Err(ProductionTargetLineageErrorV3::InvalidInteger {
            field: "wave width",
            observed: u64::from(inputs.wave_width_bits),
        });
    }
    validate_bounded_ascii_token_v2(
        "configured target",
        inputs.configured_target,
        MAX_TARGET_TEXT_BYTES_V2,
    )?;
    validate_bounded_ascii_token_v2(
        "rustc LLVM target",
        inputs.rustc_llvm_target,
        MAX_TARGET_TEXT_BYTES_V2,
    )?;
    validate_bounded_ascii_token_v2("target CPU", inputs.target_cpu, MAX_TARGET_TEXT_BYTES_V2)?;
    validate_bounded_ascii_token_v2(
        "target features",
        inputs.target_features,
        MAX_TARGET_FEATURES_BYTES_V2,
    )?;
    validate_target_profile_v2(
        inputs.configured_target,
        inputs.rustc_llvm_target,
        inputs.target_cpu,
        inputs.target_features,
    )?;
    if !(2..=MAX_MULTI_ROOT_TARGET_BINDING_ROOTS_V2).contains(&inputs.workgroups.len()) {
        return Err(ProductionTargetLineageErrorV3::InvalidInteger {
            field: "multi-root target root count",
            observed: inputs.workgroups.len() as u64,
        });
    }
    let mut kernels = BTreeSet::new();
    for entry in inputs.workgroups {
        validate_bounded_ascii_token_v2("kernel identifier", entry.kernel, MAX_NAME_BYTES)?;
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

fn validate_target_profile_v2(
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

fn validate_bounded_ascii_token_v2(
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
    validate_ascii_token_v2(field, text.as_bytes())
}

fn validate_ascii_token_v2(
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

fn push_text_v2(bytes: &mut Vec<u8>, text: &str) -> Result<(), ProductionTargetLineageErrorV3> {
    bytes.extend_from_slice(
        &u32::try_from(text.len())
            .map_err(|_| ProductionTargetLineageErrorV3::LengthOverflow)?
            .to_le_bytes(),
    );
    bytes.extend_from_slice(text.as_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};

    use super::*;

    const GFX942: &str = "gfx942:xnack-";
    const LLVM_TARGET: &str = "amdgcn-amd-amdhsa";
    const CPU: &str = "gfx942";
    const FEATURES: &str = "-wavefrontsize32,+wavefrontsize64,-xnack";

    fn identity(seed: u8, byte_len: u64) -> TargetLineageIdentityV3 {
        TargetLineageIdentityV3::new([seed; 32], byte_len).unwrap()
    }

    fn transcript() -> MultiRootTargetBindingTranscriptV2 {
        let workgroups = [
            MultiRootTargetWorkgroupInputV2 {
                kernel: "crate::alpha",
                workgroup: [64, 1, 1],
            },
            MultiRootTargetWorkgroupInputV2 {
                kernel: "crate::bravo",
                workgroup: [128, 1, 1],
            },
        ];
        MultiRootTargetBindingTranscriptV2::new(MultiRootTargetBindingInputsV2 {
            protected_rustc_invocation: identity(1, 101),
            semantic_mir: identity(2, 202),
            target_neutral_kir: identity(3, 303),
            target_bound_kir: identity(4, 404),
            configured_target: GFX942,
            rustc_llvm_target: LLVM_TARGET,
            target_cpu: CPU,
            target_features: FEATURES,
            roster_identity: [5; 32],
            code_object_version: 6,
            wave_width_bits: 64,
            workgroups: &workgroups,
        })
        .unwrap()
    }

    #[test]
    fn exact_round_trip_retains_semantic_root_order() {
        let transcript = transcript();
        let decoded =
            MultiRootTargetBindingTranscriptV2::decode(transcript.canonical_bytes()).unwrap();
        assert_eq!(decoded, transcript);
        assert_eq!(decoded.root_count(), 2);
        assert_eq!(decoded.workgroup(0).unwrap().kernel(), "crate::alpha");
        assert_eq!(decoded.workgroup(0).unwrap().workgroup(), [64, 1, 1]);
        assert_eq!(decoded.workgroup(1).unwrap().kernel(), "crate::bravo");
        assert_eq!(decoded.workgroup(1).unwrap().workgroup(), [128, 1, 1]);
        assert_eq!(decoded.configured_target(), GFX942);
        assert_eq!(decoded.rustc_llvm_target(), LLVM_TARGET);
        assert_eq!(decoded.target_cpu(), CPU);
        assert_eq!(decoded.target_features(), FEATURES);
        assert_eq!(decoded.roster_identity(), [5; 32]);
        assert_eq!(
            decoded.claim(),
            TargetLineageClaimV3::AssociationOnlyNoRefinementProof
        );
        assert!(!decoded.establishes_refinement_proof());
        assert_eq!(decoded.canonical_bytes().len(), 364);
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(decoded.canonical_bytes())),
            [
                0xaf, 0xa6, 0xe7, 0xc4, 0x42, 0x8c, 0x7b, 0xbe, 0x6f, 0xe5, 0xf5, 0xd6, 0x14, 0xc0,
                0x14, 0xb0, 0x84, 0xeb, 0x5a, 0x64, 0x69, 0xaf, 0x46, 0x0a, 0x16, 0x37, 0x7f, 0x29,
                0x22, 0xa3, 0xa2, 0x19,
            ]
        );
    }

    #[test]
    fn decode_rejects_declared_length_and_trailing_bytes() {
        let transcript = transcript();
        let mut wrong_length = transcript.canonical_bytes().to_vec();
        wrong_length[12..16].copy_from_slice(&0_u32.to_le_bytes());
        assert!(matches!(
            MultiRootTargetBindingTranscriptV2::decode(&wrong_length),
            Err(ProductionTargetLineageErrorV3::DeclaredLengthMismatch { .. })
        ));

        let mut trailing = transcript.canonical_bytes().to_vec();
        trailing.extend_from_slice(&[0]);
        let declared = u32::try_from(trailing.len()).unwrap();
        trailing[12..16].copy_from_slice(&declared.to_le_bytes());
        assert!(matches!(
            MultiRootTargetBindingTranscriptV2::decode(&trailing),
            Err(ProductionTargetLineageErrorV3::TrailingBytes { trailing: 1 })
        ));
    }

    #[test]
    fn every_prefix_and_oversized_input_is_rejected() {
        let transcript = transcript();
        for length in 0..transcript.canonical_bytes().len() {
            assert!(
                MultiRootTargetBindingTranscriptV2::decode(&transcript.canonical_bytes()[..length])
                    .is_err(),
                "accepted truncated prefix of {length} bytes",
            );
        }
        let oversized = vec![0_u8; MAX_PRODUCTION_TARGET_LINEAGE_TRANSCRIPT_BYTES_V3 + 1];
        assert!(matches!(
            MultiRootTargetBindingTranscriptV2::decode(&oversized),
            Err(ProductionTargetLineageErrorV3::TranscriptTooLarge { .. })
        ));
    }

    #[test]
    fn decode_rejects_noncanonical_header_and_identity_axes() {
        let transcript = transcript();
        let bytes = transcript.canonical_bytes();

        let mut wrong_magic = bytes.to_vec();
        wrong_magic[0] ^= 1;
        assert!(matches!(
            MultiRootTargetBindingTranscriptV2::decode(&wrong_magic),
            Err(ProductionTargetLineageErrorV3::InvalidMagic)
        ));

        let mut wrong_version = bytes.to_vec();
        wrong_version[8..10].copy_from_slice(&3_u16.to_le_bytes());
        assert!(matches!(
            MultiRootTargetBindingTranscriptV2::decode(&wrong_version),
            Err(ProductionTargetLineageErrorV3::UnsupportedVersion { observed: 3 })
        ));

        let mut wrong_policy = bytes.to_vec();
        wrong_policy[10..12].copy_from_slice(&2_u16.to_le_bytes());
        assert!(matches!(
            MultiRootTargetBindingTranscriptV2::decode(&wrong_policy),
            Err(ProductionTargetLineageErrorV3::WrongPolicy { observed: 2, .. })
        ));

        let mut zero_invocation = bytes.to_vec();
        zero_invocation[16..48].fill(0);
        assert!(matches!(
            MultiRootTargetBindingTranscriptV2::decode(&zero_invocation),
            Err(ProductionTargetLineageErrorV3::ZeroIdentity {
                field: "protected rustc invocation identity"
            })
        ));

        let mut equal_kir = bytes.to_vec();
        equal_kir[136..176].copy_from_slice(&bytes[96..136]);
        assert!(matches!(
            MultiRootTargetBindingTranscriptV2::decode(&equal_kir),
            Err(ProductionTargetLineageErrorV3::AssociationInvariant { .. })
        ));
    }

    #[test]
    fn decode_rejects_target_and_roster_cross_splices_before_root_allocation() {
        let transcript = transcript();
        let bytes = transcript.canonical_bytes();

        let cpu_offset = bytes
            .windows(CPU.len())
            .position(|window| window == CPU.as_bytes())
            .unwrap();
        let mut wrong_cpu = bytes.to_vec();
        wrong_cpu[cpu_offset..cpu_offset + CPU.len()].copy_from_slice(b"gfx950");
        assert!(matches!(
            MultiRootTargetBindingTranscriptV2::decode(&wrong_cpu),
            Err(ProductionTargetLineageErrorV3::ExactValueMismatch {
                field: "configured target and target CPU"
            })
        ));

        let first_name = bytes
            .windows("crate::alpha".len())
            .position(|window| window == b"crate::alpha")
            .unwrap();
        let root_count_offset = first_name - 8;
        let mut too_many_roots = bytes.to_vec();
        too_many_roots[root_count_offset..root_count_offset + 4].copy_from_slice(
            &u32::try_from(MAX_MULTI_ROOT_TARGET_BINDING_ROOTS_V2 + 1)
                .unwrap()
                .to_le_bytes(),
        );
        assert!(matches!(
            MultiRootTargetBindingTranscriptV2::decode(&too_many_roots),
            Err(ProductionTargetLineageErrorV3::InvalidInteger {
                field: "multi-root target root count",
                ..
            })
        ));

        let mut zero_roster = bytes.to_vec();
        zero_roster[176..208].fill(0);
        assert!(matches!(
            MultiRootTargetBindingTranscriptV2::decode(&zero_roster),
            Err(ProductionTargetLineageErrorV3::ZeroIdentity {
                field: "compiler roster identity"
            })
        ));
    }

    #[test]
    fn decode_rejects_duplicate_kernel_and_zero_workgroup() {
        let transcript = transcript();
        let bytes = transcript.canonical_bytes();
        let second_name = bytes
            .windows("crate::bravo".len())
            .position(|window| window == b"crate::bravo")
            .unwrap();
        let first_name = bytes
            .windows("crate::alpha".len())
            .position(|window| window == b"crate::alpha")
            .unwrap();
        let mut duplicate = bytes.to_vec();
        duplicate[second_name..second_name + "crate::bravo".len()]
            .copy_from_slice(&bytes[first_name..first_name + "crate::alpha".len()]);
        assert!(matches!(
            MultiRootTargetBindingTranscriptV2::decode(&duplicate),
            Err(ProductionTargetLineageErrorV3::AssociationInvariant { .. })
        ));

        let mut zero = bytes.to_vec();
        let first_workgroup = first_name + "crate::alpha".len();
        zero[first_workgroup..first_workgroup + 4].copy_from_slice(&0_u32.to_le_bytes());
        assert!(matches!(
            MultiRootTargetBindingTranscriptV2::decode(&zero),
            Err(ProductionTargetLineageErrorV3::InvalidInteger {
                field: "default workgroup",
                observed: 0,
            })
        ));
    }

    #[test]
    fn construction_rejects_target_cross_splice_and_invalid_roster() {
        let workgroups = [
            MultiRootTargetWorkgroupInputV2 {
                kernel: "alpha",
                workgroup: [64, 1, 1],
            },
            MultiRootTargetWorkgroupInputV2 {
                kernel: "beta",
                workgroup: [64, 1, 1],
            },
        ];
        let inputs = MultiRootTargetBindingInputsV2 {
            protected_rustc_invocation: identity(1, 1),
            semantic_mir: identity(2, 2),
            target_neutral_kir: identity(3, 3),
            target_bound_kir: identity(4, 4),
            configured_target: GFX942,
            rustc_llvm_target: LLVM_TARGET,
            target_cpu: "gfx950",
            target_features: FEATURES,
            roster_identity: [5; 32],
            code_object_version: 6,
            wave_width_bits: 64,
            workgroups: &workgroups,
        };
        assert!(matches!(
            MultiRootTargetBindingTranscriptV2::new(inputs),
            Err(ProductionTargetLineageErrorV3::ExactValueMismatch {
                field: "configured target and target CPU"
            })
        ));
        assert!(matches!(
            MultiRootTargetBindingTranscriptV2::new(MultiRootTargetBindingInputsV2 {
                target_cpu: CPU,
                roster_identity: [0; 32],
                ..inputs
            }),
            Err(ProductionTargetLineageErrorV3::ZeroIdentity {
                field: "compiler roster identity"
            })
        ));
    }
}
