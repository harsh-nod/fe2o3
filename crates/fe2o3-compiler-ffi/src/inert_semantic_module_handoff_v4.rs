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

/// Inherited shared-decoder metadata policy plus native framing/owner scratch.
/// Reserve separately from the entire backing's actual capacity, and retain
/// this reservation while decoded metadata lives. Valid wire size alone does
/// not imply that a caller's replay budget can admit it.
pub const INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DECODE_METADATA_STORAGE_V4: usize =
    INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DECODE_METADATA_STORAGE_V3
        + fe2o3_compiler_lineage::INERT_PRODUCTION_SEMANTIC_CAPSULE_WORKING_STORAGE_V4
        + INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_SEAL_STORAGE_V4
        + std::mem::size_of::<InertSemanticCompilerModuleHandoffV4>();

/// Conservative versioned logical byte-work prepayment for one V4 content
/// decode. Charge the returned amount *before* entering either decoder, without
/// refunding work on failure. Storage and semantic/engine replay are separate.
///
/// Eight full-image traversals cover nested hashes, receipt visits and LLVM
/// UTF-8 checks. Bounded invocation/envelope reconstruction receives 128 visits;
/// manifest reconstruction receives 320 visits and 4096 units per potential
/// row for its two BTreeMap searches. The comparison bound was audited against
/// the pinned nightly-2026-04-03 implementation (at most 16384 rows), not an
/// abstract guarantee about future standard-library implementations. Duplicate
/// envelope checks are quadratic but capped at 128 contracts. Fixed metadata
/// and error paths receive a further 4 MiB allowance. Reaudit this schedule if
/// any parser, toolchain, nesting or limit changes; this is not an instruction
/// count, a measured runtime bound, or a substitute for semantic replay.
pub fn inert_semantic_compiler_module_handoff_decode_work_v4(n: usize) -> Result<usize, Failure> {
    Ok(native::decode_work(n)?)
}
const _: () = {
    assert!(fe2o3_rustc_invocation::MAX_DESCRIPTOR_BYTES_V3 == 262338);
    assert!(crate::MAX_COMPILER_FFI_ENVELOPE_BYTES_V1 == 524288);
    assert!(crate::MAX_COMPILER_MODULE_SYMBOL_MANIFEST_BYTES_V1 == 16777216);
    assert!(crate::MAX_COMPILER_MODULE_SYMBOLS_V1 == 16384);
    assert!(crate::MAX_COMPILER_MODULE_SYMBOL_BYTES_V1 == 1024);
    assert!(crate::MAX_COMPILER_FFI_CONTRACTS_V1 == 128);
    assert!(crate::MAX_COMPILER_FFI_CRATE_LABEL_BYTES_V1 == 128);
    assert!(crate::MAX_DEVICE_FFI_SYMBOL_BYTES_V1 == 128);
};

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
    charge_work: impl FnMut(usize) -> Result<(), E>,
) -> Result<InertSemanticCompilerModuleHandoffIdentityV4, Failure<E>> {
    let sha256 = native::seal(
        &WIRE_V4,
        (layout.capsule_len, layout.module_len, layout.total),
        bytes,
        (capsule.sha256(), capsule.byte_len()),
        (module.sha256(), module.byte_len()),
        charge_work,
    )
    .map_err(|e| match e {
        native::SealError::Charge(e) => Failure::Charge(e),
        native::SealError::Wire(e) => Failure::Framing(e),
    })?;
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
impl AsRef<InertSemanticCompilerModuleHandoffV4> for InertSemanticCompilerModuleHandoffV4 {
    fn as_ref(&self) -> &Self {
        self
    }
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
        let finished = native::finish(
            &WIRE_V4,
            &backing,
            &range,
            &wire,
            (capsule.identity().sha256(), capsule.identity().byte_len()),
            |module| {
                preflight_inert_semantic_compiler_module_handoff_v4(&capsule, module)
                    .map(|layout| layout.encoded_len())
            },
        )?;
        Ok(Self {
            capsule,
            module: finished.module,
            backing,
            range,
            pair_range: finished.pair_range,
            pair_identity: InertCompilerModulePairBindingIdentityV4 {
                sha256: finished.pair_sha256,
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
    /// Entire retained allocation capacity, including unselected prefix/suffix
    /// bytes and spare capacity. Semantic admission must prepay this, not just
    /// the selected wire length, plus the separate decoded-metadata allowance.
    pub fn backing_capacity(&self) -> usize {
        self.backing.capacity()
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
