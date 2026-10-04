//! Inert AMD native-F/text-descriptor association, not a refinement theorem.
use fe2o3_amd_target::ProductionAmdTargetProfileV1;
use sha2::{Digest, Sha256};

use crate::{
    InertNativeNeutralSubjectV1, NativeNeutralSubjectErrorV1, ProductionTargetLineageErrorV3,
    TargetLineageIdentityV3,
};

/// Fixed, allocation-free size of the native lowering association.
pub const NATIVE_LOWERING_ASSOCIATION_BYTES_V1: usize = 352;
/// Conservative fixed byte-work prepayment for one build or decode, including
/// nested subject re-encoding/hashing and all fixed staging copies. The codec
/// has no ledger; its production caller prepays this before invoking it.
pub const NATIVE_LOWERING_ASSOCIATION_WORK_V1: usize = 16 * NATIVE_LOWERING_ASSOCIATION_BYTES_V1;

const MAGIC: [u8; 8] = *b"F2NLOW1\0";
const DOMAIN: &[u8] = b"FE2O3/NATIVE-F-TEXT-DESCRIPTOR-ASSOCIATION/V1\0";
const HEADER: usize = 24;
const SUBJECT_END: usize = HEADER + 96;
const PREIMAGE: usize = NATIVE_LOWERING_ASSOCIATION_BYTES_V1 - 32;

/// Distinct coordinates which an independent native lowering replay must join.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeLoweringAssociationInputsV1 {
    /// Exact final-F graph and contract catalog, not original N or later U.
    pub final_native: InertNativeNeutralSubjectV1,
    /// Domain-separated complete paired source/output carrier identity, not F2RFO1.
    pub carrier: TargetLineageIdentityV3,
    /// Raw SHA256 of the complete descriptor preimage (not the ABI receipt).
    pub descriptor: TargetLineageIdentityV3,
    /// Raw SHA256 of LLVM emitted before descriptor embedding.
    pub pre_descriptor_llvm: TargetLineageIdentityV3,
    /// Raw SHA256 of final LLVM after descriptor embedding (`module_identity`).
    pub final_llvm: TargetLineageIdentityV3,
    /// Raw SHA256 of the complete V2 handoff, distinct from LLVM and commitment.
    pub module_handoff: TargetLineageIdentityV3,
    /// Explicit owning AMD backend profile, never a neutral-IR assumption.
    pub profile: ProductionAmdTargetProfileV1,
}

#[cfg(test)]
#[path = "native_lowering_association_v1_tests.rs"]
mod tests;

/// Strict fixed-record refusal; older lowering receipts are not retried.
#[derive(Debug, Eq, PartialEq)]
pub enum NativeLoweringAssociationErrorV1 {
    /// Complete byte length differs from the one supported schema.
    Length,
    /// Magic, version, policy, profile, COV6 or reserved fields differ.
    Header,
    /// The graph/catalog subject failed its own strict decoder.
    Subject(NativeNeutralSubjectErrorV1),
    /// A content identity has invalid length or zero coordinates.
    Coordinate(ProductionTargetLineageErrorV3),
    /// Terminal domain-separated digest or canonical bytes differ.
    Identity,
}

/// Exact content record. Public construction/decoding cannot establish that
/// these inputs share a producer or that LLVM refines the graph. Consumers must
/// recover F, rerun native text/descriptor replay, and compare every coordinate.
#[derive(Debug, Eq, PartialEq)]
pub struct InertNativeLoweringAssociationV1 {
    inputs: NativeLoweringAssociationInputsV1,
    bytes: [u8; NATIVE_LOWERING_ASSOCIATION_BYTES_V1],
}
impl InertNativeLoweringAssociationV1 {
    /// Frames already identified inputs without allocation or authority.
    pub fn new(inputs: NativeLoweringAssociationInputsV1) -> Self {
        let mut bytes = [0; NATIVE_LOWERING_ASSOCIATION_BYTES_V1];
        bytes[..8].copy_from_slice(&MAGIC);
        bytes[8..12].copy_from_slice(&[1, 0, 1, 0]);
        bytes[12..16].copy_from_slice(&(NATIVE_LOWERING_ASSOCIATION_BYTES_V1 as u32).to_le_bytes());
        let tag: u16 = match inputs.profile {
            ProductionAmdTargetProfileV1::Gfx942 => 1,
            ProductionAmdTargetProfileV1::Gfx950 => 2,
        };
        bytes[16..18].copy_from_slice(&tag.to_le_bytes());
        bytes[18..20].copy_from_slice(&12_u16.to_le_bytes());
        bytes[20..22].copy_from_slice(&6_u16.to_le_bytes());
        bytes[HEADER..SUBJECT_END].copy_from_slice(inputs.final_native.canonical_bytes());
        for (slot, value) in bytes[SUBJECT_END..PREIMAGE].chunks_exact_mut(40).zip([
            inputs.carrier,
            inputs.descriptor,
            inputs.pre_descriptor_llvm,
            inputs.final_llvm,
            inputs.module_handoff,
        ]) {
            slot.copy_from_slice(&value.encode());
        }
        let mut hash = Sha256::new();
        hash.update(DOMAIN);
        hash.update((PREIMAGE as u64).to_le_bytes());
        hash.update(&bytes[..PREIMAGE]);
        bytes[PREIMAGE..].copy_from_slice(&hash.finalize());
        Self { inputs, bytes }
    }
    /// Strictly decodes this fixed schema, with no legacy or stage fallback.
    pub fn decode(bytes: &[u8]) -> Result<Self, NativeLoweringAssociationErrorV1> {
        use NativeLoweringAssociationErrorV1 as E;
        if bytes.len() != NATIVE_LOWERING_ASSOCIATION_BYTES_V1 {
            return Err(E::Length);
        }
        if bytes[..8] != MAGIC
            || bytes[8..12] != [1, 0, 1, 0]
            || bytes[12..16] != (NATIVE_LOWERING_ASSOCIATION_BYTES_V1 as u32).to_le_bytes()
            || bytes[18..24] != [12, 0, 6, 0, 0, 0]
        {
            return Err(E::Header);
        }
        let profile = match bytes[16..18] {
            [1, 0] => ProductionAmdTargetProfileV1::Gfx942,
            [2, 0] => ProductionAmdTargetProfileV1::Gfx950,
            _ => return Err(E::Header),
        };
        let axis = |index: usize| {
            TargetLineageIdentityV3::decode(
                "native lowering coordinate",
                &bytes[SUBJECT_END + 40 * index..SUBJECT_END + 40 * (index + 1)],
            )
            .map_err(E::Coordinate)
        };
        let value = Self::new(NativeLoweringAssociationInputsV1 {
            final_native: InertNativeNeutralSubjectV1::decode(&bytes[HEADER..SUBJECT_END])
                .map_err(E::Subject)?,
            carrier: axis(0)?,
            descriptor: axis(1)?,
            pre_descriptor_llvm: axis(2)?,
            final_llvm: axis(3)?,
            module_handoff: axis(4)?,
            profile,
        });
        if value.canonical_bytes() != bytes {
            return Err(E::Identity);
        }
        Ok(value)
    }
    /// Complete canonical fixed encoding, including its terminal digest.
    pub const fn canonical_bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// All exact coordinates, not evidence that any coordinate was truthful.
    pub const fn inputs(&self) -> NativeLoweringAssociationInputsV1 {
        self.inputs
    }
    /// Public content association proves no compiler or machine refinement.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}
