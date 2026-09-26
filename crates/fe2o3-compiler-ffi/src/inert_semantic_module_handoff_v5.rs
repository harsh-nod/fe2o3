//! Conditional capsule/V2 transport over the unchanged native outer wire engine.
use super::*;
use fe2o3_compiler_lineage::{
    INERT_PRODUCTION_SEMANTIC_CAPSULE_WORKING_STORAGE_V5, InertProductionSemanticCapsuleErrorV5,
    InertProductionSemanticCapsuleIdentityV5, InertProductionSemanticCapsuleV5,
    MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V5,
};
use std::{convert::Infallible, mem::size_of, ops::Range};

/// Distinct conditional outer discriminator.
pub const INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_MAGIC_V5: [u8; 8] = *b"F2O3IHV5";
/// Closed outer version, with no legacy fallback.
pub const INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_VERSION_V5: u16 = 5;
/// Distinct conditional capsule/module pair discriminator.
pub const INERT_COMPILER_MODULE_PAIR_BINDING_MAGIC_V5: [u8; 8] = *b"F2O3PBV5";
/// Closed pair version.
pub const INERT_COMPILER_MODULE_PAIR_BINDING_VERSION_V5: u16 = 5;
/// Fixed pair segment size; only discriminator/version/domain differ.
pub const INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V5: usize =
    INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V3;
/// Unchanged complete outer independent and aggregate policy.
pub const MAX_INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_BYTES_V5: usize = OUTER_FIXED_BYTES_V3
    + MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V5
    + MAX_COMPILER_MODULE_HANDOFF_BYTES_V2;
const WIRE_V5: WireSchema = WireSchema {
    magic: INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_MAGIC_V5,
    version: INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_VERSION_V5,
    pair_magic: INERT_COMPILER_MODULE_PAIR_BINDING_MAGIC_V5,
    pair_version: INERT_COMPILER_MODULE_PAIR_BINDING_VERSION_V5,
    pair_domain: b"FE2O3/INERT-COMPILER-MODULE-PAIR-BINDING/V5\0",
    outer_domain: b"FE2O3/INERT-SEMANTIC-COMPILER-MODULE-HANDOFF/V5\0",
};
const _: () = assert!(
    MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V5
        == MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V3
);

/// Fixed sealer scratch, separate from actual live backing and payload writes.
pub const INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_SEAL_STORAGE_V5: usize =
    size_of::<InertSemanticCompilerModuleHandoffLayoutV5>()
        + size_of::<InertProductionSemanticCapsuleIdentityV5>()
        + size_of::<CompilerModuleHandoffIdentityV2>()
        + size_of::<InertSemanticCompilerModuleHandoffIdentityV5>()
        + size_of::<[&[u8]; 9]>()
        + size_of::<Sha256>()
        + 3 * INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V5
        + HEADER_BYTES_V3
        + 256;

/// Conservative versioned logical decode allowance, NOT measured allocator/RSS
/// usage. Reserve alongside the ENTIRE backing capacity before unmetered decode;
/// retain while decoded metadata lives. No graph/source/descriptor admission is included.
pub const INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DECODE_METADATA_STORAGE_V5: usize =
    INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DECODE_METADATA_STORAGE_V3
        + INERT_PRODUCTION_SEMANTIC_CAPSULE_WORKING_STORAGE_V5
        + INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_SEAL_STORAGE_V5
        + size_of::<InertSemanticCompilerModuleHandoffV5>()
        + size_of::<native::Finished>();

/// Prepay before content-decoder entry; never refund accepted work on failure.
/// The inherited V4 schedule is unchanged. Four additional full-image traversals
/// cover conditional output/auxiliary hashing and metadata/layout receipt visits.
/// Total12 visits plus inherited invocation128/envelope128/manifest320/row4096
/// terms and fixed allowance. Pinned parser/toolchain changes require a new audit;
/// this is a logical visit bound, not proof of runtime or semantic correctness.
pub fn inert_semantic_compiler_module_handoff_decode_work_v5(n: usize) -> Result<usize, Failure> {
    let base = native::decode_work(n)?;
    Ok(n.checked_mul(4)
        .and_then(|extra| base.checked_add(extra))
        .ok_or(InertSemanticCompilerModuleHandoffErrorV3::LengthOverflow)?)
}

/// V5 typed failure without any legacy content-decoder fallback.
#[derive(Debug)]
pub enum InertSemanticCompilerModuleHandoffErrorV5<E = Infallible> {
    /// Original prepaid-work refusal.
    Charge(E),
    /// Invalid selected shared range.
    Range,
    /// Shared framing/module/target/commitment failure, not ordinary admission.
    Framing(InertSemanticCompilerModuleHandoffErrorV3),
    /// Mandatory conditional capsule content failed.
    Capsule(InertProductionSemanticCapsuleErrorV5),
}
type Failure<E = Infallible> = InertSemanticCompilerModuleHandoffErrorV5<E>;
impl<E> From<InertSemanticCompilerModuleHandoffErrorV3> for Failure<E> {
    fn from(e: InertSemanticCompilerModuleHandoffErrorV3) -> Self {
        Self::Framing(e)
    }
}

/// Bounded disjoint extents, not admission of either nested payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InertSemanticCompilerModuleHandoffLayoutV5 {
    capsule_len: usize,
    module_len: usize,
    total: usize,
}
impl InertSemanticCompilerModuleHandoffLayoutV5 {
    /// Checks independent lengths and unchanged total policy before allocation.
    pub fn new(capsule_len: usize, module_len: usize) -> Result<Self, Failure> {
        validate_inner_lengths(capsule_len, module_len)?;
        Ok(Self {
            capsule_len,
            module_len,
            total: exact_outer_len(capsule_len, module_len)?,
        })
    }
    /// Complete canonical allocation extent.
    pub const fn encoded_len(self) -> usize {
        self.total
    }
    /// Exact conditional capsule bytes.
    pub fn capsule_range(self) -> Range<usize> {
        HEADER_BYTES_V3..HEADER_BYTES_V3 + self.capsule_len
    }
    /// Exact unchanged V2 handoff bytes.
    pub fn module_handoff_range(self) -> Range<usize> {
        self.capsule_range().end..self.capsule_range().end + self.module_len
    }
}

/// Distinct V5 pair identity, not a V3/V4 identity or authority receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InertCompilerModulePairBindingIdentityV5 {
    sha256: [u8; 32],
}
impl InertCompilerModulePairBindingIdentityV5 {
    /// Domain-separated pair digest.
    pub const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
    /// Complete fixed segment length.
    pub const fn byte_len(self) -> u64 {
        INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V5 as u64
    }
}
/// Exact conditional outer identity, not source or publication custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InertSemanticCompilerModuleHandoffIdentityV5 {
    sha256: [u8; 32],
    byte_len: u64,
}
impl InertSemanticCompilerModuleHandoffIdentityV5 {
    /// Domain-separated outer digest.
    pub const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
    /// Complete canonical outer length.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

/// Content preflight with unchanged final-module codec owned by FFI. The capsule
/// has already checked invocation/lowering profile. Independent recovery must
/// still join every source/F/layout/descriptor/manifest/native-text subject.
pub fn preflight_inert_semantic_compiler_module_handoff_v5(
    capsule: &InertProductionSemanticCapsuleV5,
    module: &CompilerModuleHandoffV2,
) -> Result<InertSemanticCompilerModuleHandoffLayoutV5, Failure> {
    validate_inner_lengths(
        capsule.canonical_bytes().len(),
        module.canonical_bytes().len(),
    )?;
    if !module.identity().matches(module.canonical_bytes()) {
        return Err(
            InertSemanticCompilerModuleHandoffErrorV3::ModuleHandoffIdentityMismatch.into(),
        );
    }
    native::target_commitment(
        capsule.target(),
        capsule.final_module_commitment_bytes(),
        module,
    )?;
    InertSemanticCompilerModuleHandoffLayoutV5::new(
        capsule.canonical_bytes().len(),
        module.canonical_bytes().len(),
    )
}

/// Seals only outer header/pair/hash. Payloads are already written and separately
/// prepaid. All work and caller closure destruction precede mutation. Supplied
/// identities are not a substitute for subsequent content decode.
pub fn seal_inert_semantic_compiler_module_handoff_v5<E>(
    layout: InertSemanticCompilerModuleHandoffLayoutV5,
    bytes: &mut [u8],
    capsule: InertProductionSemanticCapsuleIdentityV5,
    module: CompilerModuleHandoffIdentityV2,
    charge: impl FnMut(usize) -> Result<(), E>,
) -> Result<InertSemanticCompilerModuleHandoffIdentityV5, Failure<E>> {
    let sha256 = native::seal(
        &WIRE_V5,
        (layout.capsule_len, layout.module_len, layout.total),
        bytes,
        (capsule.sha256(), capsule.byte_len()),
        (module.sha256(), module.byte_len()),
        charge,
    )
    .map_err(|e| match e {
        native::SealError::Charge(e) => Failure::Charge(e),
        native::SealError::Wire(e) => Failure::Framing(e),
    })?;
    Ok(InertSemanticCompilerModuleHandoffIdentityV5 {
        sha256,
        byte_len: bytes.len() as u64,
    })
}

/// Move-only conditional capsule/module content sharing one immutable allocation.
/// No source, original compiler, currentness, native machine or launch authority.
///
/// ```compile_fail
/// use fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV5;
/// fn duplicate(v:InertSemanticCompilerModuleHandoffV5) {let _=v.clone();}
/// ```
/// ```compile_fail
/// use fe2o3_compiler_ffi::{InertSemanticCompilerModuleHandoffV5,InertSemanticCompilerModuleHandoffV4};
/// fn downgrade(v:InertSemanticCompilerModuleHandoffV5)->InertSemanticCompilerModuleHandoffV4 {v.into()}
/// ```
pub struct InertSemanticCompilerModuleHandoffV5 {
    capsule: InertProductionSemanticCapsuleV5,
    module: CompilerModuleHandoffV2,
    backing: Arc<Vec<u8>>,
    range: Range<usize>,
    pair_range: Range<usize>,
    pair_identity: InertCompilerModulePairBindingIdentityV5,
    identity: InertSemanticCompilerModuleHandoffIdentityV5,
}
impl AsRef<InertSemanticCompilerModuleHandoffV5> for InertSemanticCompilerModuleHandoffV5 {
    fn as_ref(&self) -> &Self {
        self
    }
}
impl InertSemanticCompilerModuleHandoffV5 {
    /// Transfers exact bytes without a payload copy. Caller prepays whole capacity,
    /// quoted metadata and decode work BEFORE this unmetered content entry.
    pub fn decode_owned(bytes: Vec<u8>) -> Result<Self, Failure> {
        let wire = ValidatedOuterWireV3::decode_for_schema(&bytes, &WIRE_V5)?;
        let len = bytes.len();
        Self::from_validated(Arc::new(bytes), 0..len, wire)
    }
    /// All nested views retain the same allocation. Prepay the entire backing,
    /// including unselected/spare bytes, plus metadata; no ledger reset/refund.
    pub fn decode_shared_vec(backing: Arc<Vec<u8>>, range: Range<usize>) -> Result<Self, Failure> {
        let bytes = backing.get(range.clone()).ok_or(Failure::Range)?;
        let wire = ValidatedOuterWireV3::decode_for_schema(bytes, &WIRE_V5)?;
        Self::from_validated(backing, range, wire)
    }
    fn from_validated(
        backing: Arc<Vec<u8>>,
        range: Range<usize>,
        wire: ValidatedOuterWireV3,
    ) -> Result<Self, Failure> {
        let capsule = InertProductionSemanticCapsuleV5::decode_shared_vec(
            backing.clone(),
            range.start + wire.capsule_range.start..range.start + wire.capsule_range.end,
        )
        .map_err(Failure::Capsule)?;
        let finished = native::finish(
            &WIRE_V5,
            &backing,
            &range,
            &wire,
            (capsule.identity().sha256(), capsule.identity().byte_len()),
            |module| {
                preflight_inert_semantic_compiler_module_handoff_v5(&capsule, module)
                    .map(|l| l.encoded_len())
            },
        )?;
        Ok(Self {
            capsule,
            module: finished.module,
            backing,
            range,
            pair_range: finished.pair_range,
            pair_identity: InertCompilerModulePairBindingIdentityV5 {
                sha256: finished.pair_sha256,
            },
            identity: InertSemanticCompilerModuleHandoffIdentityV5 {
                sha256: wire.outer_sha256,
                byte_len: wire.total_len,
            },
        })
    }
    /// Exact conditional capsule, not an ordinary V3 base.
    pub const fn capsule(&self) -> &InertProductionSemanticCapsuleV5 {
        &self.capsule
    }
    /// Exact unchanged V2 module/envelope/manifest.
    pub const fn module_handoff(&self) -> &CompilerModuleHandoffV2 {
        &self.module
    }
    /// Complete selected canonical encoding.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.backing[self.range.clone()]
    }
    /// Entire retained capacity, including unselected and spare bytes.
    pub fn backing_capacity(&self) -> usize {
        self.backing.capacity()
    }
    /// Exact pair binding in shared backing.
    pub fn pair_binding_bytes(&self) -> &[u8] {
        &self.backing[self.pair_range.clone()]
    }
    /// V5-only pair identity.
    pub const fn pair_binding_identity(&self) -> InertCompilerModulePairBindingIdentityV5 {
        self.pair_identity
    }
    /// Exact complete V5 identity.
    pub const fn identity(&self) -> InertSemanticCompilerModuleHandoffIdentityV5 {
        self.identity
    }
    /// Framing/content checks grant no authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}
