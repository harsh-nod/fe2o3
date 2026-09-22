//! Explicit expanded capsule/ModuleV2 join. No signing or worker admission lives here.
use crate::inert_semantic_module_handoff_v4_decode::{
    EXPANDED_HANDOFF_READ_STORAGE_V4, ExpandedCompilerModuleHandoffRefV4,
    preflight_expanded_compiler_handoff_v4, read_shared,
};
use crate::{
    CompilerModuleHandoffErrorV2, CompilerModuleHandoffV2, FinalCompilerModuleCommitmentErrorV3,
    MAX_COMPILER_MODULE_HANDOFF_BYTES_V2,
};
use fe2o3_compiler_lineage::{
    ExpandedContentIdentityV4, ExpandedPublicationErrorV4, InertProductionSemanticCapsuleV4,
    MAX_EXPANDED_PUBLICATION_CAPSULE_BYTES_V4,
};
use std::{
    error::Error,
    fmt,
    mem::size_of,
    sync::{Arc, atomic::AtomicUsize},
};

/// Distinct complete outer magic; V3 decoders cannot accept this schema.
pub const INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_MAGIC_V4: [u8; 8] = *b"F2O3IHV4";
/// Complete outer content identity domain.
pub const INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DOMAIN_V4: &[u8] =
    b"FE2O3/INERT-SEMANTIC-COMPILER-MODULE-HANDOFF/V4\0";
/// Exact pair content identity domain.
pub const INERT_COMPILER_MODULE_PAIR_BINDING_DOMAIN_V4: &[u8] =
    b"FE2O3/INERT-COMPILER-MODULE-PAIR-BINDING/V4\0";
/// Fixed pair header plus complete capsule and ModuleV2 identities.
pub const INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V4: usize = 16 + 2 * 40;
pub(crate) const HEADER: usize = 44;
/// Derived successor bound. Neither the old outer nor any old inner limit is changed.
pub const MAX_INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_BYTES_V4: usize = HEADER
    + MAX_EXPANDED_PUBLICATION_CAPSULE_BYTES_V4
    + MAX_COMPILER_MODULE_HANDOFF_BYTES_V2
    + INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V4;
/// Shared Vec/control representation, excluding the separately measured payload capacity.
/// The allocation model counts the Vec header and two Arc counters; allocator
/// implementation bookkeeping is not a framing byte-allocation charge.
pub const EXPANDED_HANDOFF_SHARED_VECTOR_STORAGE_V4: usize =
    size_of::<Vec<u8>>() + 2 * size_of::<AtomicUsize>();

/// Typed failure without erasing child-codec or caller-work errors.
#[derive(Debug)]
pub enum ExpandedCompilerHandoffErrorV4<E> {
    /// Bounded framing, invocation or descriptor failure.
    Lineage(ExpandedPublicationErrorV4<E>),
    /// Unchanged strict ModuleV2 child refusal.
    Module(CompilerModuleHandoffErrorV2),
    /// Unchanged strict compact-commitment child refusal.
    Commitment(FinalCompilerModuleCommitmentErrorV3),
}
impl<E: fmt::Display> fmt::Display for ExpandedCompilerHandoffErrorV4<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lineage(e) => e.fmt(f),
            Self::Module(e) => e.fmt(f),
            Self::Commitment(e) => e.fmt(f),
        }
    }
}
impl<E: Error + 'static> Error for ExpandedCompilerHandoffErrorV4<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Lineage(e) => Some(e),
            Self::Module(e) => Some(e),
            Self::Commitment(e) => Some(e),
        }
    }
}
impl<E> From<ExpandedPublicationErrorV4<E>> for ExpandedCompilerHandoffErrorV4<E> {
    fn from(e: ExpandedPublicationErrorV4<E>) -> Self {
        Self::Lineage(e)
    }
}
/// Actual output-owner header/control block and transferred payload capacity.
pub fn expanded_handoff_retained_storage_v4(capacity: usize) -> Option<usize> {
    size_of::<InertSemanticCompilerModuleHandoffV4>()
        .checked_add(EXPANDED_HANDOFF_SHARED_VECTOR_STORAGE_V4)?
        .checked_add(capacity)
}
/// Output owner plus the full simultaneous framing/borrowed/legacy-child domain.
pub fn expanded_handoff_validation_storage_v4(capacity: usize) -> Option<usize> {
    expanded_handoff_retained_storage_v4(capacity)?.checked_add(EXPANDED_HANDOFF_READ_STORAGE_V4)
}
/// Exact outer length from bounded inner byte lengths, before allocation.
pub fn expanded_handoff_length_v4(capsule: usize, module: usize) -> Option<usize> {
    if capsule == 0
        || capsule > MAX_EXPANDED_PUBLICATION_CAPSULE_BYTES_V4
        || module == 0
        || module > MAX_COMPILER_MODULE_HANDOFF_BYTES_V2
    {
        return None;
    }
    HEADER
        .checked_add(capsule)?
        .checked_add(module)?
        .checked_add(INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V4)
}

/// Move-only inert outer V4. There is deliberately no conversion to the legacy
/// publication input, compiler-execution subject, worker authority or load owner.
/// ```compile_fail
/// use fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV4;
/// fn clone(x: InertSemanticCompilerModuleHandoffV4) { let _ = x.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_ffi::{InertSemanticCompilerModuleHandoffV3, InertSemanticCompilerModuleHandoffV4};
/// fn launder(x: InertSemanticCompilerModuleHandoffV4) -> InertSemanticCompilerModuleHandoffV3 { x.into() }
/// ```
pub struct InertSemanticCompilerModuleHandoffV4 {
    pub(crate) bytes: Arc<Vec<u8>>,
    identity: ExpandedContentIdentityV4,
}
impl InertSemanticCompilerModuleHandoffV4 {
    /// Borrows prepaid complete inner owners and allocates exactly one outer buffer.
    /// `available` is additional storage while both original owners stay alive.
    pub fn new<E>(
        capsule: &InertProductionSemanticCapsuleV4,
        module: &CompilerModuleHandoffV2,
        available: usize,
        charge: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, ExpandedCompilerHandoffErrorV4<E>> {
        let length = expanded_handoff_length_v4(
            capsule.canonical_bytes().len(),
            module.canonical_bytes().len(),
        )
        .ok_or_else(|| bad("outer inner extents"))?;
        ensure(
            expanded_handoff_validation_storage_v4(length).ok_or_else(overflow)?,
            available,
        )?;
        let view_extent = capsule
            .retained_storage()
            .checked_add(fe2o3_compiler_lineage::EXPANDED_CAPSULE_READ_STORAGE_V4)
            .ok_or_else(overflow)?;
        let view = capsule.view(view_extent, charge)?;
        preflight_expanded_compiler_handoff_v4(
            &view,
            module,
            EXPANDED_HANDOFF_READ_STORAGE_V4,
            charge,
        )?;
        drop(view);
        pay(length, charge)?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(length).map_err(|_| {
            ExpandedCompilerHandoffErrorV4::Lineage(ExpandedPublicationErrorV4::Allocation {
                requested: length,
            })
        })?;
        ensure(
            expanded_handoff_validation_storage_v4(bytes.capacity()).ok_or_else(overflow)?,
            available,
        )?;
        bytes.extend_from_slice(&INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_MAGIC_V4);
        bytes.extend_from_slice(&4u16.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&(length as u64).to_le_bytes());
        bytes.extend_from_slice(&(capsule.canonical_bytes().len() as u64).to_le_bytes());
        bytes.extend_from_slice(&(module.canonical_bytes().len() as u64).to_le_bytes());
        bytes
            .extend_from_slice(&(INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V4 as u32).to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(capsule.canonical_bytes());
        bytes.extend_from_slice(module.canonical_bytes());
        bytes.extend_from_slice(b"F2O3PBV4");
        bytes.extend_from_slice(&4u16.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes
            .extend_from_slice(&(INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V4 as u32).to_le_bytes());
        bytes.extend_from_slice(capsule.identity().sha256());
        bytes.extend_from_slice(&capsule.identity().byte_len().to_le_bytes());
        bytes.extend_from_slice(module.identity().sha256());
        bytes.extend_from_slice(&module.identity().byte_len().to_le_bytes());
        Self::decode_owned(bytes, available, charge)
    }
    /// Consumes the actual Vec backing without cloning capsule/history/module bytes.
    /// The complete owner and temporary readers are included in `available`.
    pub fn decode_owned<E>(
        bytes: Vec<u8>,
        available: usize,
        charge: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, ExpandedCompilerHandoffErrorV4<E>> {
        ensure(
            expanded_handoff_validation_storage_v4(bytes.capacity()).ok_or_else(overflow)?,
            available,
        )?;
        if bytes.len() > MAX_INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_BYTES_V4 {
            return Err(bad("outer aggregate"));
        }
        let bytes = Arc::new(bytes);
        let view = read_shared(&bytes, EXPANDED_HANDOFF_READ_STORAGE_V4, charge)?;
        let identity = view.identity();
        drop(view);
        Ok(Self { bytes, identity })
    }
    /// Rechecks the immutable owner and returns paid borrowed ranges and ModuleV2 metadata.
    /// The full owner stays included until every returned view/child has dropped.
    pub fn view<E>(
        &self,
        available: usize,
        charge: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<ExpandedCompilerModuleHandoffRefV4<'_>, ExpandedCompilerHandoffErrorV4<E>> {
        ensure(
            expanded_handoff_validation_storage_v4(self.bytes.capacity()).ok_or_else(overflow)?,
            available,
        )?;
        read_shared(&self.bytes, EXPANDED_HANDOFF_READ_STORAGE_V4, charge)
    }
    /// Complete exact canonical outer bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Domain/length/content identity of the full outer, not a signed occurrence.
    pub const fn identity(&self) -> ExpandedContentIdentityV4 {
        self.identity
    }
    /// Complete owner/control block and actual Vec capacity.
    pub fn retained_storage(&self) -> usize {
        expanded_handoff_retained_storage_v4(self.bytes.capacity()).expect("validated extent")
    }
    /// Framing never grants compiler, worker, publication, load or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}
pub(crate) fn bad<E>(field: &'static str) -> ExpandedCompilerHandoffErrorV4<E> {
    ExpandedPublicationErrorV4::Format(field).into()
}
pub(crate) fn overflow<E>() -> ExpandedCompilerHandoffErrorV4<E> {
    ExpandedPublicationErrorV4::Overflow.into()
}
pub(crate) fn ensure<E>(
    required: usize,
    available: usize,
) -> Result<(), ExpandedCompilerHandoffErrorV4<E>> {
    if required > available {
        Err(ExpandedPublicationErrorV4::Storage {
            required,
            available,
        }
        .into())
    } else {
        Ok(())
    }
}
pub(crate) fn pay<E>(
    work: usize,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<(), ExpandedCompilerHandoffErrorV4<E>> {
    charge(work).map_err(|e| ExpandedPublicationErrorV4::Work(e).into())
}
