//! Complete inert source transport feeding the existing consuming recipe replay.
use super::*;
use crate::InertFunctionalRefinementReceiptSignatureV2 as Signature;
use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12;
use std::mem::size_of;

#[path = "compiler_native_source_packet_decode_v1.rs"]
mod decode;
#[cfg(test)]
#[path = "compiler_native_source_packet_v1_tests.rs"]
mod tests;
#[path = "compiler_native_source_packet_wire_v1.rs"]
mod wire;

const MAGIC: &[u8; 8] = b"F2NSRC1\0";
const VERSION: u16 = 1;
const MAX_ROOTS: usize = fe2o3_compiler_lineage::MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3;

/// Aggregate transport limit, not the sum of the nested codecs' larger limits.
/// A containing ProofBinding envelope must additionally fit its own overhead
/// within the existing receipt cap. No historical receipt schema is widened.
pub const MAX_NATIVE_COMPILER_SOURCE_PACKET_BYTES_V1: usize =
    fe2o3_compiler_lineage::MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3;

/// Output byte capacity, excluding the caller's inline Vec header. Reserve this
/// receipt before further controlled allocations while retaining encoded bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeCompilerSourcePacketStorageV1(usize);
impl NativeCompilerSourcePacketStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Encodes all source inputs without importing signatures or validating graphs.
/// `None` selects Direct; `Some` selects UnitLocal and must contain actual E bytes.
/// Launch/staging/ranked lists and both ranks are independent and unsorted.
/// Names, diagnostic text, signatures, keys and all nine staging digests are
/// lossless. Decoding cannot authenticate launch origin or embedded signer keys.
///
/// The complete packet is capped at 4 MiB, including all repeated nested bytes;
/// this explicitly bounds otherwise unbounded detached logical names. Nested
/// admission keeps its own component limits. Work includes both count/fill
/// visits; success, error and unwind restore the incoming storage floor.
pub fn encode_native_compiler_source_packet_v1(
    inputs: NativeCompilerRankedRecipeSourceProofInputsV1<'_>,
    erased: Option<&[u8]>,
    budget: &mut Budget<'_>,
) -> Result<(Vec<u8>, NativeCompilerSourcePacketStorageV1), E> {
    recipe_source::recipe_scope(budget, |budget| {
        let length = wire::encode(inputs, erased, 0, None, budget)?;
        budget.reserve_storage(length)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| Resource::Allocation)?;
        if bytes.capacity() != length {
            return Err(Resource::Allocation.into());
        }
        wire::encode(inputs, erased, length, Some(&mut bytes), budget)?;
        if bytes.len() != length {
            return Err(Resource::Accounting.into());
        }
        Ok((bytes, NativeCompilerSourcePacketStorageV1(length)))
    })
}

struct StagingRoot {
    semantic_root: u32,
    commitments: Vec<NativeCompilerStagingCommitmentV1>,
}
struct RankedRoot<'a> {
    semantic_root: u32,
    launch_rank: u8,
    recipe: &'a [u8],
    source_rows: &'a [u8],
    text: &'a str,
    signatures: Vec<Signature>,
}
struct Packet<'a> {
    frames: [&'a [u8]; 5],
    erased: Option<&'a [u8]>,
    launches: Vec<ProductionSourceLaunchRootInputV1<'a>>,
    staging: Vec<StagingRoot>,
    ranked: Vec<RankedRoot<'a>>,
}

impl Packet<'_> {
    fn replay<T>(
        &self,
        budget: &mut Budget<'_>,
        replay: impl FnOnce(
            NativeCompilerRankedRecipeSourceProofInputsV1<'_>,
            &mut Budget<'_>,
        ) -> Result<T, E>,
    ) -> Result<T, E> {
        budget.reserve_storage(
            size_of::<Vec<NativeCompilerRootStagingV1<'_>>>()
                + size_of::<Vec<NativeCompilerRankedRecipeRootV1<'_>>>(),
        )?;
        let (mut staging, _) = ranked_source::reserve_vec(self.staging.len(), budget)?;
        for root in &self.staging {
            budget.charge_work(1)?;
            staging.push(NativeCompilerRootStagingV1 {
                semantic_root: root.semantic_root,
                commitments: &root.commitments,
            });
        }
        let (mut ranked, _) = ranked_source::reserve_vec(self.ranked.len(), budget)?;
        for root in &self.ranked {
            budget.charge_work(1)?;
            ranked.push(NativeCompilerRankedRecipeRootV1 {
                semantic_root: root.semantic_root,
                launch_rank: root.launch_rank,
                recipe_bytes: root.recipe,
                source_rows_bytes: root.source_rows,
                ranked_ir: root.text,
                effect_receipts: &root.signatures,
            });
        }
        replay(
            NativeCompilerRankedRecipeSourceProofInputsV1 {
                source: NativeCompilerSourceProofInputsV1 {
                    semantic_mir: self.frames[0],
                    native_module: self.frames[1],
                    middle_end_roster: self.frames[2],
                    correspondence_roster: self.frames[3],
                    verus_roster: self.frames[4],
                    launch_inputs: &self.launches,
                    staging_roots: &staging,
                },
                ranked_roots: &ranked,
            },
            budget,
        )
    }
}

/// Recovers Direct from bytes alone through the same signed recipe validator.
/// Borrowed packet bytes are caller-owned; scratch metadata remains reserved
/// throughout replay and is excluded from the unchanged retained proof receipt.
pub fn validate_native_compiler_ranked_source_packet_v1(
    bytes: &[u8],
    budget: &mut Budget<'_>,
) -> Result<
    (
        ValidatedNativeCompilerRankedSourceProofV1,
        NativeCompilerRankedSourceProofStorageV1,
    ),
    E,
> {
    recipe_source::recipe_scope(budget, |budget| {
        let packet = decode::decode(bytes, budget)?;
        if packet.erased.is_some() {
            return Err(E::PacketWire("expected Direct source packet"));
        }
        packet.replay(
            budget,
            validate_native_compiler_ranked_recipe_source_proof_v1,
        )
    })
}

/// Recovers UnitLocal from bytes, freshly admitting actual E through V12 and
/// checking the same original N/source/ranked/erasure relation. No producer or
/// serialized verified flag supplies E custody. This grants no launch authority.
pub fn validate_native_compiler_unit_local_erased_source_packet_v1(
    bytes: &[u8],
    budget: &mut Budget<'_>,
) -> Result<
    (
        ValidatedNativeCompilerUnitLocalErasedSourceProofV1,
        NativeCompilerUnitLocalErasedSourceProofStorageV1,
    ),
    E,
> {
    recipe_source::recipe_scope(budget, |budget| {
        let packet = decode::decode(bytes, budget)?;
        let bytes = packet
            .erased
            .ok_or(E::PacketWire("expected UnitLocal source packet"))?;
        let (erased, storage) =
            VerifiedCanonicalKernelIrModuleV12::from_canonical_bytes_with_verification_budget_v12(
                bytes, budget,
            )
            .map_err(E::ErasedAdmission)?;
        budget.reserve_storage(storage.retained_storage())?;
        packet.replay(budget, |inputs, budget| {
            validate_native_compiler_unit_local_erased_recipe_source_proof_v1(
                NativeCompilerUnitLocalErasedRecipeSourceProofInputsV1 {
                    original: inputs.source,
                    ranked_roots: inputs.ranked_roots,
                    erased: &erased,
                },
                budget,
            )
        })
    })
}
