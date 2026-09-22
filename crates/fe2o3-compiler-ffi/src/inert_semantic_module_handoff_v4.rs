//! V4 native capsule transport using the unchanged bounded handoff layout.
use std::{convert::Infallible, ops::Range};

use fe2o3_compiler_lineage::{
    InertProductionSemanticCapsuleErrorV4, InertProductionSemanticCapsuleIdentityV4,
    InertProductionSemanticCapsuleV4, MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V4,
};

use super::*;

/// Distinct native handoff discriminator.
pub const INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_MAGIC_V4: [u8; 8] = *b"F2O3IHV4";
/// Native handoff version; no V3 fallback is permitted.
pub const INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_VERSION_V4: u16 = 4;
/// Distinct V4 capsule/module pair discriminator.
pub const INERT_COMPILER_MODULE_PAIR_BINDING_MAGIC_V4: [u8; 8] = *b"F2O3PBV4";
/// Native capsule/module pair version.
pub const INERT_COMPILER_MODULE_PAIR_BINDING_VERSION_V4: u16 = 4;
/// Fixed segment size is unchanged; its schema and identity domain differ.
pub const INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V4: usize =
    INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V3;
/// Complete handoff ceiling, including a complete (not enlarged) 160 MiB capsule.
pub const MAX_INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_BYTES_V4: usize = OUTER_FIXED_BYTES_V3
    + MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V4
    + MAX_COMPILER_MODULE_HANDOFF_BYTES_V2;
const WIRE_V4: WireSchema = WireSchema {
    magic: INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_MAGIC_V4,
    version: INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_VERSION_V4,
    pair_magic: INERT_COMPILER_MODULE_PAIR_BINDING_MAGIC_V4,
    pair_version: INERT_COMPILER_MODULE_PAIR_BINDING_VERSION_V4,
    pair_domain: b"FE2O3/INERT-COMPILER-MODULE-PAIR-BINDING/V4\0",
    outer_domain: b"FE2O3/INERT-SEMANTIC-COMPILER-MODULE-HANDOFF/V4\0",
};
// Sharing the historical length engine is valid only while these policies agree.
const _: () = assert!(
    MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V4
        == MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V3
);

/// Conservative fixed logical sealer scratch, including pair staging/results,
/// slice-reference fields, hash state and scalar/digest/header temporaries.
/// Reserve this and actual live backing before sealing. It excludes decoded
/// owners, nested decoders and caller payload-writing storage; it is not RSS.
pub const INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_SEAL_STORAGE_V4: usize =
    std::mem::size_of::<InertSemanticCompilerModuleHandoffLayoutV4>()
        + std::mem::size_of::<InertProductionSemanticCapsuleIdentityV4>()
        + std::mem::size_of::<CompilerModuleHandoffIdentityV2>()
        + std::mem::size_of::<InertSemanticCompilerModuleHandoffIdentityV4>()
        + std::mem::size_of::<[&[u8]; 9]>()
        + std::mem::size_of::<Sha256>()
        + 3 * INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V4
        + HEADER_BYTES_V3
        + 256;

/// V4-specific failure. Shared framing errors retain their existing diagnostics;
/// carrying that error type never invokes the legacy capsule decoder.
#[derive(Debug, Eq, PartialEq)]
pub enum InertSemanticCompilerModuleHandoffErrorV4<E = Infallible> {
    /// Work refusal before any sealer write.
    Charge(E),
    /// Invalid selected range in shared backing.
    Range,
    /// Shared wire, V2 module, target or final-commitment check failed.
    Framing(InertSemanticCompilerModuleHandoffErrorV3),
    /// The mandatory V4 capsule failed its own content decoder.
    Capsule(InertProductionSemanticCapsuleErrorV4),
}
type Failure<E = Infallible> = InertSemanticCompilerModuleHandoffErrorV4<E>;
impl<E> From<InertSemanticCompilerModuleHandoffErrorV3> for Failure<E> {
    fn from(error: InertSemanticCompilerModuleHandoffErrorV3) -> Self {
        Self::Framing(error)
    }
}

/// Checked disjoint extents for direct encoding into one final handoff buffer.
/// Extents alone establish no cross-member or semantic agreement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InertSemanticCompilerModuleHandoffLayoutV4 {
    capsule_len: usize,
    module_len: usize,
    total: usize,
}
impl InertSemanticCompilerModuleHandoffLayoutV4 {
    /// Checks unchanged independent member bounds and the complete outer extent.
    pub fn new(capsule_len: usize, module_len: usize) -> Result<Self, Failure> {
        validate_inner_lengths(capsule_len, module_len)?;
        Ok(Self {
            capsule_len,
            module_len,
            total: exact_outer_len(capsule_len, module_len)?,
        })
    }
    /// Complete canonical buffer length.
    pub const fn encoded_len(self) -> usize {
        self.total
    }
    /// Region for unchanged V4 capsule bytes.
    pub fn capsule_range(self) -> Range<usize> {
        HEADER_BYTES_V3..HEADER_BYTES_V3 + self.capsule_len
    }
    /// Region for unchanged V2 compiler module handoff bytes.
    pub fn module_handoff_range(self) -> Range<usize> {
        self.capsule_range().end..self.capsule_range().end + self.module_len
    }
    fn pair_range(self) -> Range<usize> {
        self.module_handoff_range().end..self.total - SHA256_BYTES
    }
}

/// V4-only pair-binding identity; never interchangeable with the V3 binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InertCompilerModulePairBindingIdentityV4 {
    sha256: [u8; 32],
}
impl InertCompilerModulePairBindingIdentityV4 {
    /// Domain-separated binding digest.
    pub const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
    /// Complete canonical fixed segment length.
    pub const fn byte_len(self) -> u64 {
        INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V4 as u64
    }
}

/// Domain-separated identity of the complete V4 outer transport.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InertSemanticCompilerModuleHandoffIdentityV4 {
    sha256: [u8; 32],
    byte_len: u64,
}
impl InertSemanticCompilerModuleHandoffIdentityV4 {
    /// Domain-separated outer digest.
    pub const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
    /// Exact complete canonical byte length.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

/// Checks target and final-commitment agreement before an outer allocation.
/// This reuses only the unchanged V3 base/V2 content joins, not legacy semantic
/// admission, execution evidence or authority. Native source/F joins remain required.
pub fn preflight_inert_semantic_compiler_module_handoff_v4(
    capsule: &InertProductionSemanticCapsuleV4,
    module: &CompilerModuleHandoffV2,
) -> Result<InertSemanticCompilerModuleHandoffLayoutV4, Failure> {
    let _ = preflight_inert_semantic_compiler_module_handoff_v3(capsule.base(), module)?;
    InertSemanticCompilerModuleHandoffLayoutV4::new(
        capsule.canonical_bytes().len(),
        module.canonical_bytes().len(),
    )
}

/// Writes only outer framing and the distinct V4 identity binding around
/// already encoded payloads. Lengths and all work debits precede mutation;
/// refusal or callback panic leaves bytes unchanged. Caller prepays payload
/// writes and reserves the live backing plus hash/header/pair scratch.
///
/// Sealing is not decoding: it does not inspect payloads or assert that they
/// match the supplied identities. The shared decoder performs those checks,
/// including target/final-commitment agreement, before returning an owner.
pub fn seal_inert_semantic_compiler_module_handoff_v4<E>(
    layout: InertSemanticCompilerModuleHandoffLayoutV4,
    bytes: &mut [u8],
    capsule: InertProductionSemanticCapsuleIdentityV4,
    module: CompilerModuleHandoffIdentityV2,
    mut charge_work: impl FnMut(usize) -> Result<(), E>,
) -> Result<InertSemanticCompilerModuleHandoffIdentityV4, Failure<E>> {
    if bytes.len() != layout.total
        || capsule.byte_len() != layout.capsule_len as u64
        || module.byte_len() != layout.module_len as u64
    {
        return Err(
            InertSemanticCompilerModuleHandoffErrorV3::InvalidLength(bytes.len() as u64).into(),
        );
    }
    // Hash both preimages and prepay fixed pair/header writes and digest copies.
    let work = layout
        .total
        .checked_add(
            WIRE_V4.outer_domain.len()
                + WIRE_V4.pair_domain.len()
                + 16
                + 256
                + PAIR_BINDING_PREIMAGE_BYTES_V3
                + 3 * INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V4
                + 2 * HEADER_BYTES_V3,
        )
        .ok_or(InertSemanticCompilerModuleHandoffErrorV3::LengthOverflow)?;
    charge_work(work).map_err(Failure::Charge)?;
    drop(charge_work);
    let (pair, _) = encode_pair_binding(
        &WIRE_V4,
        capsule.sha256(),
        capsule.byte_len(),
        module.sha256(),
        module.byte_len(),
    )?;
    bytes[..HEADER_BYTES_V3].fill(0);
    bytes[..8].copy_from_slice(&WIRE_V4.magic);
    bytes[8..10].copy_from_slice(&WIRE_V4.version.to_le_bytes());
    bytes[12..20].copy_from_slice(&(layout.total as u64).to_le_bytes());
    bytes[24..32].copy_from_slice(&(layout.capsule_len as u64).to_le_bytes());
    bytes[32..40].copy_from_slice(&(layout.module_len as u64).to_le_bytes());
    bytes[layout.pair_range()].copy_from_slice(&pair);
    let sha256 =
        derive_identity_sha256(WIRE_V4.outer_domain, &bytes[..layout.total - SHA256_BYTES]).ok_or(
            InertSemanticCompilerModuleHandoffErrorV3::ZeroIdentity {
                field: "inert semantic compiler module handoff",
            },
        )?;
    bytes[layout.total - SHA256_BYTES..].copy_from_slice(&sha256);
    Ok(InertSemanticCompilerModuleHandoffIdentityV4 {
        sha256,
        byte_len: layout.total as u64,
    })
}

/// Immutable native capsule/V2 transport in a single shared allocation.
/// Content checks do not authenticate a producer, validate source/F semantics,
/// establish currentness or grant compiler/publication/load/launch authority.
///
/// ```compile_fail
/// use fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV4;
/// fn duplicate(value: InertSemanticCompilerModuleHandoffV4) { let _ = value.clone(); }
/// ```
pub struct InertSemanticCompilerModuleHandoffV4 {
    capsule: InertProductionSemanticCapsuleV4,
    module: CompilerModuleHandoffV2,
    backing: Arc<Vec<u8>>,
    range: Range<usize>,
    pair_range: Range<usize>,
    pair_identity: InertCompilerModulePairBindingIdentityV4,
    identity: InertSemanticCompilerModuleHandoffIdentityV4,
}
impl InertSemanticCompilerModuleHandoffV4 {
    /// Transfers a complete canonical vector without another payload allocation.
    /// Framing is checked before allocating even the shared owner header.
    pub fn decode_owned(bytes: Vec<u8>) -> Result<Self, Failure> {
        let validated = ValidatedOuterWireV3::decode_for_schema(&bytes, &WIRE_V4)?;
        let len = bytes.len();
        Self::from_validated(Arc::new(bytes), 0..len, validated)
    }
    /// Decodes a checked range in shared custody. Capsule, base receipts,
    /// carrier and V2 module all retain subranges in this exact allocation.
    /// Like V3 this content decoder is unmetered; production must prepay work,
    /// decoded records and actual entire backing capacity before calling it.
    pub fn decode_shared_vec(backing: Arc<Vec<u8>>, range: Range<usize>) -> Result<Self, Failure> {
        let bytes = backing.get(range.clone()).ok_or(Failure::Range)?;
        let validated = ValidatedOuterWireV3::decode_for_schema(bytes, &WIRE_V4)?;
        Self::from_validated(backing, range, validated)
    }
    fn from_validated(
        backing: Arc<Vec<u8>>,
        range: Range<usize>,
        wire: ValidatedOuterWireV3,
    ) -> Result<Self, Failure> {
        let capsule = InertProductionSemanticCapsuleV4::decode_shared_vec(
            backing.clone(),
            range.start + wire.capsule_range.start..range.start + wire.capsule_range.end,
        )
        .map_err(Failure::Capsule)?;
        let module = CompilerModuleHandoffV2::decode_shared_vec_range(
            backing.clone(),
            range.start + wire.module_handoff_range.start,
            wire.module_handoff_range.len(),
        )
        .map_err(InertSemanticCompilerModuleHandoffErrorV3::ModuleHandoff)?;
        let pair = wire.parsed_pair_binding;
        if capsule.identity().sha256() != &pair.capsule_sha256
            || capsule.identity().byte_len() != pair.capsule_len
        {
            return Err(InertSemanticCompilerModuleHandoffErrorV3::CapsuleIdentityMismatch.into());
        }
        if module.identity().sha256() != &pair.module_handoff_sha256
            || module.identity().byte_len() != pair.module_handoff_len
        {
            return Err(
                InertSemanticCompilerModuleHandoffErrorV3::ModuleHandoffIdentityMismatch.into(),
            );
        }
        let layout = preflight_inert_semantic_compiler_module_handoff_v4(&capsule, &module)?;
        if layout.encoded_len() != range.len() {
            return Err(InertSemanticCompilerModuleHandoffErrorV3::NonCanonicalEncoding.into());
        }
        let (canonical_pair, binding_sha256) = encode_pair_binding(
            &WIRE_V4,
            capsule.identity().sha256(),
            capsule.identity().byte_len(),
            module.identity().sha256(),
            module.identity().byte_len(),
        )?;
        let pair_range =
            range.start + wire.pair_binding_range.start..range.start + wire.pair_binding_range.end;
        if backing[pair_range.clone()] != canonical_pair || binding_sha256 != pair.binding_sha256 {
            return Err(InertSemanticCompilerModuleHandoffErrorV3::NonCanonicalEncoding.into());
        }
        Ok(Self {
            capsule,
            module,
            backing,
            range,
            pair_range,
            pair_identity: InertCompilerModulePairBindingIdentityV4 {
                sha256: binding_sha256,
            },
            identity: InertSemanticCompilerModuleHandoffIdentityV4 {
                sha256: wire.outer_sha256,
                byte_len: wire.total_len,
            },
        })
    }
    /// Exact native capsule, including the mandatory source/F carrier.
    pub const fn capsule(&self) -> &InertProductionSemanticCapsuleV4 {
        &self.capsule
    }
    /// Exact outer V2 module. Native recovery must also join its embedded copy.
    pub const fn module_handoff(&self) -> &CompilerModuleHandoffV2 {
        &self.module
    }
    /// Complete canonical bytes in the selected shared range.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.backing[self.range.clone()]
    }
    /// Exact distinct V4 pair-binding segment, sharing the input allocation.
    pub fn pair_binding_bytes(&self) -> &[u8] {
        &self.backing[self.pair_range.clone()]
    }
    /// Pair identity binding complete V4 capsule and V2 module identities.
    pub const fn pair_binding_identity(&self) -> InertCompilerModulePairBindingIdentityV4 {
        self.pair_identity
    }
    /// Complete V4 outer identity.
    pub const fn identity(&self) -> InertSemanticCompilerModuleHandoffIdentityV4 {
        self.identity
    }
    /// Public content consistency grants no authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}
