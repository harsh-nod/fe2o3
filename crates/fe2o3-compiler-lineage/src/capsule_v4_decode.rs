//! Borrowed V4 decoder. No history graph or optimizer witness is materialized.
use crate::{
    EXPANDED_HISTORY_DIRECTORY_STORAGE_V4, EXPANDED_OUTPUT_ASSOCIATION_READ_STORAGE_V1,
    EXPANDED_PUBLICATION_HASH_STORAGE_V4, EXPANDED_SEMANTIC_TO_LLVM_READ_STORAGE_V1,
    ExpandedContentIdentityV4, ExpandedOutputAssociationRefV1, ExpandedOutputAxisV1,
    ExpandedPublicationErrorV4, ExpandedReceiptRefV4, ExpandedReceiptSlotV4,
    ExpandedSemanticToLlvmRefV1, INERT_PRODUCTION_SEMANTIC_CAPSULE_DOMAIN_V4,
    MAX_EXPANDED_PUBLICATION_CAPSULE_BYTES_V4, expanded_content_identity_v4,
    expanded_history_final_graph_range_v4,
};
use crate::{
    capsule_v4::{DIRECTORY, HEADER},
    expanded_history_receipt_v4::{Reader, storage, work},
};
use fe2o3_kernel_descriptor::{
    CodeObjectVersion, DESCRIPTOR_READER_SCRATCH_STORAGE_V3, DESCRIPTOR_TABLE_VIEW_STORAGE_V3,
    DeviceTargetV1, decode_device_descriptor_table_v3,
};
use fe2o3_rustc_invocation::{
    InvocationDigestV3, MAX_COMPILE_ENVIRONMENT_ENTRIES_V2, MAX_DESCRIPTOR_BYTES_V2,
    MAX_DESCRIPTOR_BYTES_V3, MAX_PATH_BYTES_V2, MAX_RUSTC_ARGUMENTS_V2,
    RustcInvocationDescriptorV3, decode_descriptor_v3,
};
use std::mem::size_of;

/// Bounded legacy-child reservation, not legacy allocation/work telemetry.
/// The unchanged V3 invocation decoder can retain a V2 reconstruction, a V2
/// re-encoding, a V3 re-encoding, aggregate decoded string payload bounded by its
/// descriptor wire cap, source/destination argument String arrays, environment
/// String headers, three separately validated path copies, and fixed owners.
/// Argument count is capped at 4096 (a power of two); even a non-reusing consuming
/// collect fits one source and one destination array at that maximum.
/// The enclosing legacy decoder's canonical-byte/count limits remain in force.
pub const EXPANDED_INVOCATION_CHILD_STORAGE_V4: usize = MAX_DESCRIPTOR_BYTES_V2
    + MAX_DESCRIPTOR_BYTES_V2
    + MAX_DESCRIPTOR_BYTES_V3
    + MAX_DESCRIPTOR_BYTES_V3
    + 2 * MAX_RUSTC_ARGUMENTS_V2 * size_of::<String>()
    + MAX_COMPILE_ENVIRONMENT_ENTRIES_V2 * 2 * size_of::<String>()
    + size_of::<RustcInvocationDescriptorV3>()
    + 3 * size_of::<Vec<u8>>()
    + 3 * size_of::<String>()
    + 3 * MAX_PATH_BYTES_V2
    + size_of::<fe2o3_build_authority::CompilerClosureV2>()
    + 7 * size_of::<[u8; 32]>()
    + 3 * size_of::<Reader<'static>>()
    + EXPANDED_PUBLICATION_HASH_STORAGE_V4;
/// Simultaneous framing views/directories plus pre-reserved legacy and descriptor
/// child domains. The invocation domain is intentionally not called exact telemetry.
pub const EXPANDED_CAPSULE_READ_STORAGE_V4: usize = 2 * size_of::<ExpandedCapsuleRefV4<'static>>()
    + size_of::<Reader<'static>>()
    + 16 * size_of::<usize>()
    + 16 * size_of::<ExpandedContentIdentityV4>()
    + EXPANDED_INVOCATION_CHILD_STORAGE_V4
    + EXPANDED_OUTPUT_ASSOCIATION_READ_STORAGE_V1
    + EXPANDED_SEMANTIC_TO_LLVM_READ_STORAGE_V1
    + EXPANDED_HISTORY_DIRECTORY_STORAGE_V4
    + DESCRIPTOR_READER_SCRATCH_STORAGE_V3
    + DESCRIPTOR_TABLE_VIEW_STORAGE_V3
    + EXPANDED_PUBLICATION_HASH_STORAGE_V4;

/// Immutable range-backed capsule. All views are tied to the actual input backing.
/// ```compile_fail
/// use fe2o3_compiler_lineage::ExpandedCapsuleRefV4;
/// fn escape(bytes: Vec<u8>) -> ExpandedCapsuleRefV4<'static> {
///     ExpandedCapsuleRefV4::read(&bytes, usize::MAX, &mut |_| Ok::<(), ()>(())).unwrap()
/// }
/// ```
pub struct ExpandedCapsuleRefV4<'a> {
    bytes: &'a [u8],
    invocation: &'a [u8],
    invocation_digest: [u8; 32],
    target_text: &'a str,
    target: DeviceTargetV1,
    slots: [ExpandedReceiptRefV4<'a>; 16],
    identity: ExpandedContentIdentityV4,
}
impl<'a> ExpandedCapsuleRefV4<'a> {
    /// Strictly checks the new framing and its structural joins. Input backing is
    /// prepaid. This does not authenticate source or prove any graph transition.
    pub fn read<E>(
        bytes: &'a [u8],
        available: usize,
        charge: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, ExpandedPublicationErrorV4<E>> {
        storage(EXPANDED_CAPSULE_READ_STORAGE_V4, available)?;
        if bytes.len() > MAX_EXPANDED_PUBLICATION_CAPSULE_BYTES_V4 {
            return Err(ExpandedPublicationErrorV4::Format("capsule aggregate"));
        }
        work(HEADER + DIRECTORY, charge)?;
        let mut r = Reader::new(bytes);
        if r.take::<E>(8)? != b"F2O3ISV4"
            || r.u16::<E>()? != 4
            || r.u16::<E>()? != 0
            || r.u64::<E>()? != bytes.len() as u64
            || r.u16::<E>()? != 16
        {
            return Err(ExpandedPublicationErrorV4::Format("capsule header"));
        }
        let target_len = r.u16::<E>()? as usize;
        let invocation_len = r.u32::<E>()? as usize;
        let declared_invocation: [u8; 32] = r.array()?;
        if !(1..=128).contains(&target_len)
            || !(1..=MAX_DESCRIPTOR_BYTES_V3).contains(&invocation_len)
        {
            return Err(ExpandedPublicationErrorV4::Format(
                "invocation/target extent",
            ));
        }
        let dummy = ExpandedContentIdentityV4::from_declared([1; 32], 1).expect("nonzero literal");
        let mut lengths = [0usize; 16];
        let mut identities = [dummy; 16];
        for slot in ExpandedReceiptSlotV4::ALL {
            if r.u16::<E>()? as usize != slot as usize || r.u16::<E>()? != 0 {
                return Err(ExpandedPublicationErrorV4::Format(
                    "receipt directory order",
                ));
            }
            let n64 = r.u64::<E>()?;
            let n = usize::try_from(n64).map_err(|_| ExpandedPublicationErrorV4::Overflow)?;
            if n == 0 || n > slot.maximum_bytes() {
                return Err(ExpandedPublicationErrorV4::Format("receipt cap"));
            }
            lengths[slot as usize] = n;
            identities[slot as usize] =
                ExpandedContentIdentityV4::from_declared(r.array()?, n64)
                    .ok_or(ExpandedPublicationErrorV4::Identity("receipt declaration"))?;
        }
        let invocation = r.take::<E>(invocation_len)?;
        let target_text = std::str::from_utf8(r.take(target_len)?)
            .map_err(|_| ExpandedPublicationErrorV4::Format("target UTF-8"))?;
        let target = DeviceTargetV1::parse(target_text)
            .map_err(|_| ExpandedPublicationErrorV4::Format("canonical target"))?;
        let digest = invocation_digest(invocation, target_text, charge)?;
        if digest != declared_invocation {
            return Err(ExpandedPublicationErrorV4::Identity("invocation"));
        }
        let mut slots = [ExpandedReceiptRefV4 {
            bytes: &[],
            identity: dummy,
        }; 16];
        for slot in ExpandedReceiptSlotV4::ALL {
            let payload = r.take::<E>(lengths[slot as usize])?;
            let id = expanded_content_identity_v4(
                slot.identity_domain(),
                payload,
                EXPANDED_PUBLICATION_HASH_STORAGE_V4,
                charge,
            )?;
            if id != identities[slot as usize] {
                return Err(ExpandedPublicationErrorV4::Identity("receipt content"));
            }
            slots[slot as usize] = ExpandedReceiptRefV4 {
                bytes: payload,
                identity: id,
            };
        }
        r.finish()?;
        let association = ExpandedOutputAssociationRefV1::read(
            slots[7].bytes,
            EXPANDED_OUTPUT_ASSOCIATION_READ_STORAGE_V1,
            charge,
        )?;
        if association.target() != target_text {
            return Err(ExpandedPublicationErrorV4::Identity("association target"));
        }
        for (axis, slot) in [
            (ExpandedOutputAxisV1::SemanticMir, 2),
            (ExpandedOutputAxisV1::Correspondence, 5),
            (ExpandedOutputAxisV1::History, 15),
            (ExpandedOutputAxisV1::FinalGraph, 4),
            (ExpandedOutputAxisV1::FinalFormal, 6),
            (ExpandedOutputAxisV1::TargetBinding, 8),
            (ExpandedOutputAxisV1::DataLayout, 9),
            (ExpandedOutputAxisV1::AmdgpuLowering, 12),
            (ExpandedOutputAxisV1::FinalCommitment, 14),
        ] {
            work(40, charge)?;
            if association.axis(axis) != slots[slot].identity {
                return Err(ExpandedPublicationErrorV4::Identity(
                    "association receipt axis",
                ));
            }
        }
        let history = slots[15].bytes;
        let final_range = expanded_history_final_graph_range_v4(
            history,
            EXPANDED_HISTORY_DIRECTORY_STORAGE_V4,
            charge,
        )?;
        let final_graph = slots[4].bytes;
        work(final_graph.len(), charge)?;
        if final_graph.len() < 24
            || final_graph.get(..8) != Some(b"FE2O3KI\0")
            || final_graph[8..10] != 12u16.to_le_bytes()
            || history.get(final_range) != Some(final_graph)
        {
            return Err(ExpandedPublicationErrorV4::Format(
                "complete final V12 graph",
            ));
        }
        let descriptor = decode_device_descriptor_table_v3(slots[10].bytes, charge)
            .map_err(ExpandedPublicationErrorV4::Descriptor)?;
        if descriptor.device_target() != target
            || descriptor.code_object_version() != CodeObjectVersion::V6
            || descriptor.canonical_code_object_digest().as_bytes() != &[0; 32]
            || descriptor.kernel_count() != association.root_count()
        {
            return Err(ExpandedPublicationErrorV4::Identity(
                "nominal descriptor target/digest/roster",
            ));
        }
        let derivation = ExpandedSemanticToLlvmRefV1::read(
            slots[13].bytes,
            EXPANDED_SEMANTIC_TO_LLVM_READ_STORAGE_V1,
            charge,
        )?;
        derivation.check_association(&association, EXPANDED_PUBLICATION_HASH_STORAGE_V4, charge)?;
        drop(derivation);
        drop(descriptor);
        drop(association);
        let identity = expanded_content_identity_v4(
            INERT_PRODUCTION_SEMANTIC_CAPSULE_DOMAIN_V4,
            bytes,
            EXPANDED_PUBLICATION_HASH_STORAGE_V4,
            charge,
        )?;
        Ok(Self {
            bytes,
            invocation,
            invocation_digest: digest,
            target_text,
            target,
            slots,
            identity,
        })
    }
    /// Complete canonical bytes; all receipt ranges borrow this same backing.
    pub const fn canonical_bytes(&self) -> &'a [u8] {
        self.bytes
    }
    /// Complete capsule content identity.
    pub const fn identity(&self) -> ExpandedContentIdentityV4 {
        self.identity
    }
    /// Exact invocation bytes, independently strictly decoded during admission.
    pub const fn invocation_bytes(&self) -> &'a [u8] {
        self.invocation
    }
    /// Actual invocation V3 digest, not a producer attestation.
    pub const fn invocation_digest(&self) -> &[u8; 32] {
        &self.invocation_digest
    }
    /// Exact parsed target, including features.
    pub const fn target(&self) -> DeviceTargetV1 {
        self.target
    }
    /// Exact canonical target spelling.
    pub const fn target_text(&self) -> &'a str {
        self.target_text
    }
    /// Returns one already validated immutable receipt range.
    pub const fn receipt(&self, slot: ExpandedReceiptSlotV4) -> ExpandedReceiptRefV4<'a> {
        self.slots[slot as usize]
    }
}

pub(crate) fn invocation_digest<E>(
    bytes: &[u8],
    target: &str,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<[u8; 32], ExpandedPublicationErrorV4<E>> {
    // Framing charges the boundary and comparison. The unchanged child executes
    // in the separately prepaid fixed/count/shape-bounded legacy domain above.
    work(1, charge)?;
    let invocation = decode_descriptor_v3(bytes).map_err(|e| {
        ExpandedPublicationErrorV4::Legacy(crate::LineageDecodeErrorV3::Invocation(e))
    })?;
    work(target.len(), charge)?;
    if invocation.amd_target() != target {
        return Err(ExpandedPublicationErrorV4::Format(
            "actual invocation target",
        ));
    }
    let digest = InvocationDigestV3::calculate(&invocation).map_err(|_| {
        ExpandedPublicationErrorV4::Legacy(crate::LineageDecodeErrorV3::NonCanonical)
    })?;
    Ok(digest.into_bytes())
}
