use std::{error::Error, fmt};

use fe2o3_compiler_lineage::{
    InertProductionSemanticCapsuleIdentityV3, InertProductionSemanticCapsuleV3,
    LineageDecodeErrorV3, MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V3,
};
use sha2::{Digest, Sha256};

use super::{
    CompilerModuleHandoffErrorV2, CompilerModuleHandoffIdentityV2, CompilerModuleHandoffV2,
    MAX_COMPILER_MODULE_HANDOFF_BYTES_V2,
};

/// Fixed magic at the start of every inert semantic compiler module handoff V3.
pub const INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_MAGIC_V3: [u8; 8] = *b"F2O3IHV3";

/// The only inert semantic compiler module handoff version implemented by this crate.
pub const INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_VERSION_V3: u16 = 3;

/// Fixed magic at the start of the embedded inert pair-binding segment V3.
pub const INERT_COMPILER_MODULE_PAIR_BINDING_MAGIC_V3: [u8; 8] = *b"F2O3PBV3";

/// The only inert compiler module pair-binding version implemented by this crate.
pub const INERT_COMPILER_MODULE_PAIR_BINDING_VERSION_V3: u16 = 3;

const PAIR_BINDING_IDENTITY_DOMAIN_V3: &[u8] = b"FE2O3/INERT-COMPILER-MODULE-PAIR-BINDING/V3\0";
const OUTER_IDENTITY_DOMAIN_V3: &[u8] = b"FE2O3/INERT-SEMANTIC-COMPILER-MODULE-HANDOFF/V3\0";
const SHA256_BYTES: usize = 32;
const INNER_IDENTITY_BYTES: usize = SHA256_BYTES + 8;
const HEADER_BYTES_V3: usize = 8 + 2 + 2 + 8 + 4 + 8 + 8;
const PAIR_BINDING_PREIMAGE_BYTES_V3: usize =
    8 + 2 + 2 + 4 + 4 + INNER_IDENTITY_BYTES + INNER_IDENTITY_BYTES;
/// Exact canonical byte length of the fixed inert pair-binding segment V3.
pub const INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V3: usize =
    PAIR_BINDING_PREIMAGE_BYTES_V3 + SHA256_BYTES;
const OUTER_FIXED_BYTES_V3: usize =
    HEADER_BYTES_V3 + INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V3 + SHA256_BYTES;
const MIN_OUTER_BYTES_V3: usize = OUTER_FIXED_BYTES_V3 + 2;

/// Maximum complete canonical bytes in one inert semantic compiler module handoff V3.
pub const MAX_INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_BYTES_V3: usize = OUTER_FIXED_BYTES_V3
    + MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V3
    + MAX_COMPILER_MODULE_HANDOFF_BYTES_V2;

/// Domain-separated identity of the fixed pair-binding segment.
///
/// The segment commits only to the already-complete native identities of the
/// semantic capsule and V2 module handoff. It never contains or derives from
/// the outer identity.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InertCompilerModulePairBindingIdentityV3 {
    sha256: [u8; SHA256_BYTES],
    byte_len: u64,
}

impl InertCompilerModulePairBindingIdentityV3 {
    /// Returns the pair-binding segment's domain-separated SHA-256 bytes.
    pub const fn sha256(&self) -> &[u8; SHA256_BYTES] {
        &self.sha256
    }

    /// Returns the complete canonical pair-binding segment length.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    /// Checks exact pair-binding bytes without granting authority.
    pub fn matches_canonical_bytes(self, bytes: &[u8]) -> bool {
        self.byte_len == bytes.len() as u64
            && bytes.len() == INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V3
            && bytes[PAIR_BINDING_PREIMAGE_BYTES_V3..] == self.sha256
            && derive_identity_sha256(
                PAIR_BINDING_IDENTITY_DOMAIN_V3,
                &bytes[..PAIR_BINDING_PREIMAGE_BYTES_V3],
            )
            .is_some_and(|actual| actual == self.sha256)
    }
}

impl fmt::Debug for InertCompilerModulePairBindingIdentityV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InertCompilerModulePairBindingIdentityV3")
            .field("sha256", &self.sha256)
            .field("byte_len", &self.byte_len)
            .finish()
    }
}

/// Inert fixed segment binding one capsule identity to one V2 handoff identity.
///
/// This is a content association, not producer authentication or authority.
#[derive(Eq, PartialEq)]
pub struct InertCompilerModulePairBindingV3 {
    capsule_identity: InertProductionSemanticCapsuleIdentityV3,
    module_handoff_identity: CompilerModuleHandoffIdentityV2,
    identity: InertCompilerModulePairBindingIdentityV3,
    canonical_bytes: [u8; INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V3],
}

impl InertCompilerModulePairBindingV3 {
    fn new(
        capsule_identity: InertProductionSemanticCapsuleIdentityV3,
        module_handoff_identity: CompilerModuleHandoffIdentityV2,
    ) -> Result<Self, InertSemanticCompilerModuleHandoffErrorV3> {
        let mut canonical_bytes = [0_u8; INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V3];
        let mut offset = 0;
        put_slice(
            &mut canonical_bytes,
            &mut offset,
            &INERT_COMPILER_MODULE_PAIR_BINDING_MAGIC_V3,
        );
        put_slice(
            &mut canonical_bytes,
            &mut offset,
            &INERT_COMPILER_MODULE_PAIR_BINDING_VERSION_V3.to_le_bytes(),
        );
        put_slice(&mut canonical_bytes, &mut offset, &0_u16.to_le_bytes());
        put_slice(
            &mut canonical_bytes,
            &mut offset,
            &(INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V3 as u32).to_le_bytes(),
        );
        put_slice(&mut canonical_bytes, &mut offset, &0_u32.to_le_bytes());
        put_slice(&mut canonical_bytes, &mut offset, capsule_identity.sha256());
        put_slice(
            &mut canonical_bytes,
            &mut offset,
            &capsule_identity.byte_len().to_le_bytes(),
        );
        put_slice(
            &mut canonical_bytes,
            &mut offset,
            module_handoff_identity.sha256(),
        );
        put_slice(
            &mut canonical_bytes,
            &mut offset,
            &module_handoff_identity.byte_len().to_le_bytes(),
        );
        debug_assert_eq!(offset, PAIR_BINDING_PREIMAGE_BYTES_V3);
        let sha256 = derive_identity_sha256(
            PAIR_BINDING_IDENTITY_DOMAIN_V3,
            &canonical_bytes[..PAIR_BINDING_PREIMAGE_BYTES_V3],
        )
        .ok_or(InertSemanticCompilerModuleHandoffErrorV3::ZeroIdentity {
            field: "inert compiler module pair binding",
        })?;
        put_slice(&mut canonical_bytes, &mut offset, &sha256);
        debug_assert_eq!(offset, INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V3);
        let identity = InertCompilerModulePairBindingIdentityV3 {
            sha256,
            byte_len: INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V3 as u64,
        };
        Ok(Self {
            capsule_identity,
            module_handoff_identity,
            identity,
            canonical_bytes,
        })
    }

    /// Returns the exact native semantic-capsule identity in this binding.
    pub const fn capsule_identity(&self) -> InertProductionSemanticCapsuleIdentityV3 {
        self.capsule_identity
    }

    /// Returns the exact native V2 module-handoff identity in this binding.
    pub const fn module_handoff_identity(&self) -> CompilerModuleHandoffIdentityV2 {
        self.module_handoff_identity
    }

    /// Returns the domain-separated identity of this complete segment.
    pub const fn identity(&self) -> InertCompilerModulePairBindingIdentityV3 {
        self.identity
    }

    /// Returns the complete canonical pair-binding segment.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Reports that the pair-binding segment does not authenticate a producer.
    pub const fn authenticates_producer(&self) -> bool {
        false
    }

    /// Reports that the pair-binding segment grants no publication authority.
    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    /// Reports that the pair-binding segment grants no load authority.
    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    /// Reports that the pair-binding segment grants no launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

impl fmt::Debug for InertCompilerModulePairBindingV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InertCompilerModulePairBindingV3")
            .field("capsule_identity", &self.capsule_identity)
            .field("module_handoff_identity", &self.module_handoff_identity)
            .field("identity", &self.identity)
            .finish()
    }
}

/// Domain-separated identity of one complete outer V3 handoff.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InertSemanticCompilerModuleHandoffIdentityV3 {
    sha256: [u8; SHA256_BYTES],
    byte_len: u64,
}

impl InertSemanticCompilerModuleHandoffIdentityV3 {
    /// Returns the outer handoff's domain-separated SHA-256 bytes.
    pub const fn sha256(&self) -> &[u8; SHA256_BYTES] {
        &self.sha256
    }

    /// Returns the complete canonical outer handoff length.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    /// Checks exact outer bytes without granting authority.
    pub fn matches_canonical_bytes(self, bytes: &[u8]) -> bool {
        if self.byte_len != bytes.len() as u64 || bytes.len() < SHA256_BYTES {
            return false;
        }
        let preimage_len = bytes.len() - SHA256_BYTES;
        bytes[preimage_len..] == self.sha256
            && derive_identity_sha256(OUTER_IDENTITY_DOMAIN_V3, &bytes[..preimage_len])
                .is_some_and(|actual| actual == self.sha256)
    }
}

impl fmt::Debug for InertSemanticCompilerModuleHandoffIdentityV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InertSemanticCompilerModuleHandoffIdentityV3")
            .field("sha256", &self.sha256)
            .field("byte_len", &self.byte_len)
            .finish()
    }
}

/// Opaque proof that exact V3 inner and aggregate byte bounds were checked.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InertSemanticCompilerModuleHandoffPreflightV3 {
    capsule_identity: InertProductionSemanticCapsuleIdentityV3,
    module_handoff_identity: CompilerModuleHandoffIdentityV2,
    capsule_bytes: usize,
    module_handoff_bytes: usize,
    exact_outer_bytes: usize,
}

impl InertSemanticCompilerModuleHandoffPreflightV3 {
    /// Returns the checked semantic-capsule byte length.
    pub const fn capsule_bytes(self) -> usize {
        self.capsule_bytes
    }

    /// Returns the checked V2 module-handoff byte length.
    pub const fn module_handoff_bytes(self) -> usize {
        self.module_handoff_bytes
    }

    /// Returns the checked exact outer canonical byte length.
    pub const fn exact_outer_bytes(self) -> usize {
        self.exact_outer_bytes
    }
}

/// Checks native inner identities, target agreement, and exact aggregate bounds
/// before outer canonical-byte allocation.
pub fn preflight_inert_semantic_compiler_module_handoff_v3(
    capsule: &InertProductionSemanticCapsuleV3,
    module_handoff: &CompilerModuleHandoffV2,
) -> Result<InertSemanticCompilerModuleHandoffPreflightV3, InertSemanticCompilerModuleHandoffErrorV3>
{
    let capsule_bytes = capsule.canonical_bytes();
    let module_handoff_bytes = module_handoff.canonical_bytes();
    let capsule_identity = capsule.identity();
    let module_handoff_identity = module_handoff.identity();

    validate_inner_lengths(capsule_bytes.len(), module_handoff_bytes.len())?;
    if !capsule_identity.matches_canonical_bytes(capsule_bytes) {
        return Err(InertSemanticCompilerModuleHandoffErrorV3::CapsuleIdentityMismatch);
    }
    if !module_handoff_identity.matches(module_handoff_bytes) {
        return Err(InertSemanticCompilerModuleHandoffErrorV3::ModuleHandoffIdentityMismatch);
    }
    if capsule.target() != module_handoff.target() {
        return Err(InertSemanticCompilerModuleHandoffErrorV3::TargetMismatch);
    }
    let exact_outer_bytes = exact_outer_len(capsule_bytes.len(), module_handoff_bytes.len())?;

    Ok(InertSemanticCompilerModuleHandoffPreflightV3 {
        capsule_identity,
        module_handoff_identity,
        capsule_bytes: capsule_bytes.len(),
        module_handoff_bytes: module_handoff_bytes.len(),
        exact_outer_bytes,
    })
}

/// Strict inert content owner for one exact semantic capsule and one exact V2
/// compiler module handoff.
///
/// Public construction and decoding establish only canonical content identity.
/// They do not authenticate a producer, prove stage derivation or compiler
/// origin, establish freshness, or grant compiler, artifact, worker, link,
/// publication, load, or launch authority.
#[derive(Eq, PartialEq)]
pub struct InertSemanticCompilerModuleHandoffV3 {
    capsule: InertProductionSemanticCapsuleV3,
    module_handoff: CompilerModuleHandoffV2,
    pair_binding: InertCompilerModulePairBindingV3,
    identity: InertSemanticCompilerModuleHandoffIdentityV3,
    canonical_bytes: Box<[u8]>,
}

impl InertSemanticCompilerModuleHandoffV3 {
    /// Constructs one canonical inert outer handoff from complete inner owners.
    pub fn new(
        capsule: InertProductionSemanticCapsuleV3,
        module_handoff: CompilerModuleHandoffV2,
    ) -> Result<Self, InertSemanticCompilerModuleHandoffErrorV3> {
        let preflight =
            preflight_inert_semantic_compiler_module_handoff_v3(&capsule, &module_handoff)?;
        let pair_binding = InertCompilerModulePairBindingV3::new(
            preflight.capsule_identity,
            preflight.module_handoff_identity,
        )?;
        let total_len = u64::try_from(preflight.exact_outer_bytes)
            .map_err(|_| InertSemanticCompilerModuleHandoffErrorV3::LengthOverflow)?;
        let capsule_len = u64::try_from(preflight.capsule_bytes)
            .map_err(|_| InertSemanticCompilerModuleHandoffErrorV3::LengthOverflow)?;
        let module_handoff_len = u64::try_from(preflight.module_handoff_bytes)
            .map_err(|_| InertSemanticCompilerModuleHandoffErrorV3::LengthOverflow)?;

        let mut canonical = Vec::with_capacity(preflight.exact_outer_bytes);
        canonical.extend_from_slice(&INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_MAGIC_V3);
        canonical
            .extend_from_slice(&INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_VERSION_V3.to_le_bytes());
        canonical.extend_from_slice(&0_u16.to_le_bytes());
        canonical.extend_from_slice(&total_len.to_le_bytes());
        canonical.extend_from_slice(&0_u32.to_le_bytes());
        canonical.extend_from_slice(&capsule_len.to_le_bytes());
        canonical.extend_from_slice(&module_handoff_len.to_le_bytes());
        canonical.extend_from_slice(capsule.canonical_bytes());
        canonical.extend_from_slice(module_handoff.canonical_bytes());
        canonical.extend_from_slice(pair_binding.canonical_bytes());
        let sha256 = derive_identity_sha256(OUTER_IDENTITY_DOMAIN_V3, &canonical).ok_or(
            InertSemanticCompilerModuleHandoffErrorV3::ZeroIdentity {
                field: "inert semantic compiler module handoff",
            },
        )?;
        canonical.extend_from_slice(&sha256);
        debug_assert_eq!(canonical.len(), preflight.exact_outer_bytes);
        let identity = InertSemanticCompilerModuleHandoffIdentityV3 {
            sha256,
            byte_len: total_len,
        };

        Ok(Self {
            capsule,
            module_handoff,
            pair_binding,
            identity,
            canonical_bytes: canonical.into_boxed_slice(),
        })
    }

    /// Strictly decodes one complete canonical outer V3 handoff with no fallback.
    ///
    /// The complete outer and inner lengths are checked before either inner
    /// decoder runs or the outer canonical buffer is allocated.
    pub fn decode(bytes: &[u8]) -> Result<Self, InertSemanticCompilerModuleHandoffErrorV3> {
        if bytes.len() > MAX_INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_BYTES_V3 {
            return Err(InertSemanticCompilerModuleHandoffErrorV3::OuterByteBoundExceeded);
        }
        let mut reader = Reader::new(bytes);
        if reader.fixed::<8>()? != INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_MAGIC_V3 {
            return Err(InertSemanticCompilerModuleHandoffErrorV3::InvalidMagic);
        }
        let version = reader.u16()?;
        if version != INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_VERSION_V3 {
            return Err(InertSemanticCompilerModuleHandoffErrorV3::UnsupportedVersion(version));
        }
        let flags = reader.u16()?;
        if flags != 0 {
            return Err(InertSemanticCompilerModuleHandoffErrorV3::UnsupportedFlags(
                flags,
            ));
        }
        let declared_total_len = reader.u64()?;
        let declared_total_len_usize = usize::try_from(declared_total_len).map_err(|_| {
            InertSemanticCompilerModuleHandoffErrorV3::InvalidLength(declared_total_len)
        })?;
        if declared_total_len_usize < MIN_OUTER_BYTES_V3 {
            return Err(InertSemanticCompilerModuleHandoffErrorV3::InvalidLength(
                declared_total_len,
            ));
        }
        if declared_total_len_usize > MAX_INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_BYTES_V3 {
            return Err(InertSemanticCompilerModuleHandoffErrorV3::OuterByteBoundExceeded);
        }
        if declared_total_len_usize > bytes.len() {
            return Err(InertSemanticCompilerModuleHandoffErrorV3::Truncated);
        }
        if declared_total_len_usize < bytes.len() {
            return Err(InertSemanticCompilerModuleHandoffErrorV3::TrailingBytes);
        }
        if reader.u32()? != 0 {
            return Err(InertSemanticCompilerModuleHandoffErrorV3::NonzeroReserved);
        }
        let capsule_len_u64 = reader.u64()?;
        let module_handoff_len_u64 = reader.u64()?;
        let capsule_len = checked_inner_len(
            capsule_len_u64,
            MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V3,
            InertSemanticCompilerModuleHandoffErrorV3::CapsuleByteBoundExceeded,
        )?;
        let module_handoff_len = checked_inner_len(
            module_handoff_len_u64,
            MAX_COMPILER_MODULE_HANDOFF_BYTES_V2,
            InertSemanticCompilerModuleHandoffErrorV3::ModuleHandoffByteBoundExceeded,
        )?;
        let expected_total_len = exact_outer_len(capsule_len, module_handoff_len)?;
        if expected_total_len != declared_total_len_usize {
            return Err(InertSemanticCompilerModuleHandoffErrorV3::InvalidLength(
                declared_total_len,
            ));
        }

        let capsule_bytes = reader.take(capsule_len)?;
        let module_handoff_bytes = reader.take(module_handoff_len)?;
        let pair_binding_bytes = reader.take(INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V3)?;
        let outer_preimage_len = reader.offset();
        let declared_outer_sha256 = reader.fixed::<SHA256_BYTES>()?;
        if !reader.is_empty() {
            return Err(InertSemanticCompilerModuleHandoffErrorV3::TrailingBytes);
        }
        if declared_outer_sha256 == [0; SHA256_BYTES] {
            return Err(InertSemanticCompilerModuleHandoffErrorV3::ZeroIdentity {
                field: "inert semantic compiler module handoff",
            });
        }
        if derive_identity_sha256(OUTER_IDENTITY_DOMAIN_V3, &bytes[..outer_preimage_len])
            != Some(declared_outer_sha256)
        {
            return Err(InertSemanticCompilerModuleHandoffErrorV3::OuterIdentityMismatch);
        }

        let parsed_pair_binding = ParsedPairBindingV3::decode(pair_binding_bytes)?;
        if parsed_pair_binding.capsule_len != capsule_len_u64
            || parsed_pair_binding.module_handoff_len != module_handoff_len_u64
        {
            return Err(InertSemanticCompilerModuleHandoffErrorV3::PairBindingInnerMismatch);
        }
        if capsule_bytes.len() < SHA256_BYTES
            || capsule_bytes[capsule_bytes.len() - SHA256_BYTES..]
                != parsed_pair_binding.capsule_sha256
        {
            return Err(InertSemanticCompilerModuleHandoffErrorV3::CapsuleIdentityMismatch);
        }
        let actual_module_handoff_sha256: [u8; SHA256_BYTES] =
            Sha256::digest(module_handoff_bytes).into();
        if actual_module_handoff_sha256 != parsed_pair_binding.module_handoff_sha256 {
            return Err(InertSemanticCompilerModuleHandoffErrorV3::ModuleHandoffIdentityMismatch);
        }

        let capsule = InertProductionSemanticCapsuleV3::decode(capsule_bytes)
            .map_err(InertSemanticCompilerModuleHandoffErrorV3::Capsule)?;
        let module_handoff = CompilerModuleHandoffV2::decode(module_handoff_bytes)
            .map_err(InertSemanticCompilerModuleHandoffErrorV3::ModuleHandoff)?;
        if capsule.identity().sha256() != &parsed_pair_binding.capsule_sha256
            || capsule.identity().byte_len() != parsed_pair_binding.capsule_len
        {
            return Err(InertSemanticCompilerModuleHandoffErrorV3::CapsuleIdentityMismatch);
        }
        if module_handoff.identity().sha256() != &parsed_pair_binding.module_handoff_sha256
            || module_handoff.identity().byte_len() != parsed_pair_binding.module_handoff_len
        {
            return Err(InertSemanticCompilerModuleHandoffErrorV3::ModuleHandoffIdentityMismatch);
        }

        let decoded = Self::new(capsule, module_handoff)?;
        if decoded.pair_binding.canonical_bytes() != pair_binding_bytes
            || decoded.pair_binding.identity().sha256() != &parsed_pair_binding.binding_sha256
            || decoded.canonical_bytes() != bytes
        {
            return Err(InertSemanticCompilerModuleHandoffErrorV3::NonCanonicalEncoding);
        }
        Ok(decoded)
    }

    /// Returns the exact retained inert semantic capsule.
    pub const fn capsule(&self) -> &InertProductionSemanticCapsuleV3 {
        &self.capsule
    }

    /// Returns the exact retained V2 compiler module handoff.
    pub const fn module_handoff(&self) -> &CompilerModuleHandoffV2 {
        &self.module_handoff
    }

    /// Returns the fixed pair-binding segment.
    pub const fn pair_binding(&self) -> &InertCompilerModulePairBindingV3 {
        &self.pair_binding
    }

    /// Returns the terminal domain-separated outer identity.
    pub const fn identity(&self) -> InertSemanticCompilerModuleHandoffIdentityV3 {
        self.identity
    }

    /// Returns the complete exact canonical outer encoding.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Moves both exact inner owners out of this inert outer owner.
    pub fn into_capsule_and_module_handoff(
        self,
    ) -> (InertProductionSemanticCapsuleV3, CompilerModuleHandoffV2) {
        (self.capsule, self.module_handoff)
    }

    /// Reports that this inert content does not authenticate a producer.
    pub const fn authenticates_producer(&self) -> bool {
        false
    }

    /// Reports that this inert content does not authenticate compiler origin.
    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    /// Reports that this inert content grants no compiler authority.
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    /// Reports that this inert content grants no artifact authority.
    pub const fn grants_artifact_authority(&self) -> bool {
        false
    }

    /// Reports that this inert content grants no worker authority.
    pub const fn grants_worker_authority(&self) -> bool {
        false
    }

    /// Reports that this inert content grants no link authority.
    pub const fn grants_link_authority(&self) -> bool {
        false
    }

    /// Reports that this inert content grants no publication authority.
    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    /// Reports that this inert content grants no load authority.
    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    /// Reports that this inert content grants no launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

impl fmt::Debug for InertSemanticCompilerModuleHandoffV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InertSemanticCompilerModuleHandoffV3")
            .field("capsule_identity", &self.capsule.identity())
            .field("module_handoff_identity", &self.module_handoff.identity())
            .field("pair_binding_identity", &self.pair_binding.identity())
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

impl<'a> TryFrom<&'a [u8]> for InertSemanticCompilerModuleHandoffV3 {
    type Error = InertSemanticCompilerModuleHandoffErrorV3;

    fn try_from(bytes: &'a [u8]) -> Result<Self, Self::Error> {
        Self::decode(bytes)
    }
}

/// Failure to construct or strictly decode an inert outer V3 handoff.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum InertSemanticCompilerModuleHandoffErrorV3 {
    /// The complete outer input exceeds its exported byte bound.
    OuterByteBoundExceeded,
    /// The semantic capsule is empty or exceeds its exported byte bound.
    CapsuleByteBoundExceeded,
    /// The V2 module handoff is empty or exceeds its exported byte bound.
    ModuleHandoffByteBoundExceeded,
    /// A construction-time length cannot be represented by the wire schema.
    LengthOverflow,
    /// The outer magic does not identify this exact V3 schema.
    InvalidMagic,
    /// The outer version is not the one exact supported V3 version.
    UnsupportedVersion(u16),
    /// Unsupported outer flags were present.
    UnsupportedFlags(u16),
    /// A reserved outer field was nonzero.
    NonzeroReserved,
    /// The declared complete outer length is impossible or noncanonical.
    InvalidLength(u64),
    /// The physical input ends before its complete declared encoding.
    Truncated,
    /// Bytes remain after the complete declared encoding.
    TrailingBytes,
    /// A required digest was all zeroes.
    ZeroIdentity {
        /// Name of the identity field.
        field: &'static str,
    },
    /// The pair-binding segment magic is invalid.
    InvalidPairBindingMagic,
    /// The pair-binding segment version is unsupported.
    UnsupportedPairBindingVersion(u16),
    /// Unsupported pair-binding flags were present.
    UnsupportedPairBindingFlags(u16),
    /// The pair-binding segment length is not the canonical fixed length.
    InvalidPairBindingLength(u32),
    /// A reserved pair-binding field was nonzero.
    NonzeroPairBindingReserved,
    /// The pair-binding segment's terminal identity does not match its preimage.
    PairBindingIdentityMismatch,
    /// Pair-binding inner lengths disagree with the outer preflight lengths.
    PairBindingInnerMismatch,
    /// Exact semantic-capsule bytes and the bound native identity disagree.
    CapsuleIdentityMismatch,
    /// Exact V2 module-handoff bytes and the bound native identity disagree.
    ModuleHandoffIdentityMismatch,
    /// The complete capsule and module handoff name different canonical targets.
    TargetMismatch,
    /// The terminal outer identity does not match the complete outer preimage.
    OuterIdentityMismatch,
    /// Decoding and reconstruction did not reproduce the exact input bytes.
    NonCanonicalEncoding,
    /// Strict decoding of the inner semantic capsule failed.
    Capsule(LineageDecodeErrorV3),
    /// Strict decoding of the inner V2 module handoff failed.
    ModuleHandoff(CompilerModuleHandoffErrorV2),
}

impl fmt::Display for InertSemanticCompilerModuleHandoffErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OuterByteBoundExceeded => {
                formatter.write_str("inert outer V3 handoff byte bound exceeded")
            }
            Self::CapsuleByteBoundExceeded => {
                formatter.write_str("inert semantic capsule byte bound exceeded")
            }
            Self::ModuleHandoffByteBoundExceeded => {
                formatter.write_str("V2 compiler module handoff byte bound exceeded")
            }
            Self::LengthOverflow => formatter.write_str("inert outer V3 handoff length overflow"),
            Self::InvalidMagic => formatter.write_str("invalid inert outer V3 handoff magic"),
            Self::UnsupportedVersion(version) => {
                write!(
                    formatter,
                    "unsupported inert outer handoff version {version}"
                )
            }
            Self::UnsupportedFlags(flags) => {
                write!(
                    formatter,
                    "unsupported inert outer handoff flags {flags:#x}"
                )
            }
            Self::NonzeroReserved => {
                formatter.write_str("nonzero inert outer handoff reserved field")
            }
            Self::InvalidLength(length) => {
                write!(formatter, "invalid inert outer handoff length {length}")
            }
            Self::Truncated => formatter.write_str("truncated inert outer V3 handoff"),
            Self::TrailingBytes => formatter.write_str("trailing inert outer V3 handoff bytes"),
            Self::ZeroIdentity { field } => write!(formatter, "zero identity for {field}"),
            Self::InvalidPairBindingMagic => {
                formatter.write_str("invalid inert pair-binding segment magic")
            }
            Self::UnsupportedPairBindingVersion(version) => {
                write!(
                    formatter,
                    "unsupported inert pair-binding version {version}"
                )
            }
            Self::UnsupportedPairBindingFlags(flags) => {
                write!(formatter, "unsupported inert pair-binding flags {flags:#x}")
            }
            Self::InvalidPairBindingLength(length) => {
                write!(formatter, "invalid inert pair-binding length {length}")
            }
            Self::NonzeroPairBindingReserved => {
                formatter.write_str("nonzero inert pair-binding reserved field")
            }
            Self::PairBindingIdentityMismatch => {
                formatter.write_str("inert pair-binding identity mismatch")
            }
            Self::PairBindingInnerMismatch => {
                formatter.write_str("inert pair-binding inner identity metadata mismatch")
            }
            Self::CapsuleIdentityMismatch => {
                formatter.write_str("inert semantic capsule identity mismatch")
            }
            Self::ModuleHandoffIdentityMismatch => {
                formatter.write_str("V2 compiler module handoff identity mismatch")
            }
            Self::TargetMismatch => {
                formatter.write_str("semantic capsule and module handoff target mismatch")
            }
            Self::OuterIdentityMismatch => {
                formatter.write_str("inert outer V3 handoff identity mismatch")
            }
            Self::NonCanonicalEncoding => {
                formatter.write_str("noncanonical inert outer V3 handoff encoding")
            }
            Self::Capsule(error) => write!(formatter, "invalid inert semantic capsule: {error}"),
            Self::ModuleHandoff(error) => {
                write!(formatter, "invalid V2 compiler module handoff: {error}")
            }
        }
    }
}

impl Error for InertSemanticCompilerModuleHandoffErrorV3 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Capsule(error) => Some(error),
            Self::ModuleHandoff(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ParsedPairBindingV3 {
    capsule_sha256: [u8; SHA256_BYTES],
    capsule_len: u64,
    module_handoff_sha256: [u8; SHA256_BYTES],
    module_handoff_len: u64,
    binding_sha256: [u8; SHA256_BYTES],
}

impl ParsedPairBindingV3 {
    fn decode(bytes: &[u8]) -> Result<Self, InertSemanticCompilerModuleHandoffErrorV3> {
        debug_assert_eq!(bytes.len(), INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V3);
        let mut reader = Reader::new(bytes);
        if reader.fixed::<8>()? != INERT_COMPILER_MODULE_PAIR_BINDING_MAGIC_V3 {
            return Err(InertSemanticCompilerModuleHandoffErrorV3::InvalidPairBindingMagic);
        }
        let version = reader.u16()?;
        if version != INERT_COMPILER_MODULE_PAIR_BINDING_VERSION_V3 {
            return Err(
                InertSemanticCompilerModuleHandoffErrorV3::UnsupportedPairBindingVersion(version),
            );
        }
        let flags = reader.u16()?;
        if flags != 0 {
            return Err(
                InertSemanticCompilerModuleHandoffErrorV3::UnsupportedPairBindingFlags(flags),
            );
        }
        let segment_len = reader.u32()?;
        if segment_len != INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V3 as u32 {
            return Err(
                InertSemanticCompilerModuleHandoffErrorV3::InvalidPairBindingLength(segment_len),
            );
        }
        if reader.u32()? != 0 {
            return Err(InertSemanticCompilerModuleHandoffErrorV3::NonzeroPairBindingReserved);
        }
        let capsule_sha256 = reader.fixed::<SHA256_BYTES>()?;
        let capsule_len = reader.u64()?;
        let module_handoff_sha256 = reader.fixed::<SHA256_BYTES>()?;
        let module_handoff_len = reader.u64()?;
        let preimage_len = reader.offset();
        debug_assert_eq!(preimage_len, PAIR_BINDING_PREIMAGE_BYTES_V3);
        let binding_sha256 = reader.fixed::<SHA256_BYTES>()?;
        if !reader.is_empty() {
            return Err(InertSemanticCompilerModuleHandoffErrorV3::TrailingBytes);
        }
        if capsule_sha256 == [0; SHA256_BYTES] {
            return Err(InertSemanticCompilerModuleHandoffErrorV3::ZeroIdentity {
                field: "inert semantic capsule pair member",
            });
        }
        if module_handoff_sha256 == [0; SHA256_BYTES] {
            return Err(InertSemanticCompilerModuleHandoffErrorV3::ZeroIdentity {
                field: "V2 compiler module handoff pair member",
            });
        }
        if binding_sha256 == [0; SHA256_BYTES] {
            return Err(InertSemanticCompilerModuleHandoffErrorV3::ZeroIdentity {
                field: "inert compiler module pair binding",
            });
        }
        if derive_identity_sha256(PAIR_BINDING_IDENTITY_DOMAIN_V3, &bytes[..preimage_len])
            != Some(binding_sha256)
        {
            return Err(InertSemanticCompilerModuleHandoffErrorV3::PairBindingIdentityMismatch);
        }
        Ok(Self {
            capsule_sha256,
            capsule_len,
            module_handoff_sha256,
            module_handoff_len,
            binding_sha256,
        })
    }
}

fn validate_inner_lengths(
    capsule_len: usize,
    module_handoff_len: usize,
) -> Result<(), InertSemanticCompilerModuleHandoffErrorV3> {
    if capsule_len == 0 || capsule_len > MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V3 {
        return Err(InertSemanticCompilerModuleHandoffErrorV3::CapsuleByteBoundExceeded);
    }
    if module_handoff_len == 0 || module_handoff_len > MAX_COMPILER_MODULE_HANDOFF_BYTES_V2 {
        return Err(InertSemanticCompilerModuleHandoffErrorV3::ModuleHandoffByteBoundExceeded);
    }
    Ok(())
}

fn checked_inner_len(
    encoded: u64,
    max: usize,
    error: InertSemanticCompilerModuleHandoffErrorV3,
) -> Result<usize, InertSemanticCompilerModuleHandoffErrorV3> {
    let len = usize::try_from(encoded).map_err(|_| error.clone())?;
    if len == 0 || len > max {
        return Err(error);
    }
    Ok(len)
}

fn exact_outer_len(
    capsule_len: usize,
    module_handoff_len: usize,
) -> Result<usize, InertSemanticCompilerModuleHandoffErrorV3> {
    let exact = OUTER_FIXED_BYTES_V3
        .checked_add(capsule_len)
        .and_then(|len| len.checked_add(module_handoff_len))
        .ok_or(InertSemanticCompilerModuleHandoffErrorV3::LengthOverflow)?;
    if exact > MAX_INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_BYTES_V3 {
        return Err(InertSemanticCompilerModuleHandoffErrorV3::OuterByteBoundExceeded);
    }
    Ok(exact)
}

fn derive_identity_sha256(domain: &[u8], preimage: &[u8]) -> Option<[u8; SHA256_BYTES]> {
    let preimage_len = u64::try_from(preimage.len()).ok()?;
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(preimage_len.to_le_bytes());
    digest.update(preimage);
    let sha256: [u8; SHA256_BYTES] = digest.finalize().into();
    (sha256 != [0; SHA256_BYTES]).then_some(sha256)
}

fn put_slice<const N: usize>(target: &mut [u8; N], offset: &mut usize, bytes: &[u8]) {
    let end = *offset + bytes.len();
    target[*offset..end].copy_from_slice(bytes);
    *offset = end;
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    const fn offset(&self) -> usize {
        self.offset
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], InertSemanticCompilerModuleHandoffErrorV3> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(InertSemanticCompilerModuleHandoffErrorV3::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(InertSemanticCompilerModuleHandoffErrorV3::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn fixed<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], InertSemanticCompilerModuleHandoffErrorV3> {
        self.take(N)?
            .try_into()
            .map_err(|_| InertSemanticCompilerModuleHandoffErrorV3::Truncated)
    }

    fn u16(&mut self) -> Result<u16, InertSemanticCompilerModuleHandoffErrorV3> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, InertSemanticCompilerModuleHandoffErrorV3> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn u64(&mut self) -> Result<u64, InertSemanticCompilerModuleHandoffErrorV3> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }

    fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

#[cfg(test)]
mod tests_wire_adversarial {
    use std::ffi::OsString;

    use fe2o3_build_authority::CompilerClosureV2;
    use fe2o3_compiler_lineage::{
        InertAbiReceiptV3, InertAmdgpuLoweringReceiptV3, InertCanonicalSemanticMirReceiptV3,
        InertDataLayoutReceiptV3, InertExportManifestReceiptV3, InertFormalMemoryReceiptV3,
        InertKernelIrReceiptV3, InertLlvmModuleReceiptV3, InertMiddleEndReceiptV3,
        InertMirToKirCorrespondenceReceiptV3, InertProofBindingReceiptV3,
        InertRustcIdentityInventoryReceiptV3, InertRustcPreflightPlanReceiptV3,
        InertSemanticToLlvmReceiptV3, InertTargetBindingReceiptV3,
        OrderedInertSemanticLineageReceiptsV3,
    };
    use fe2o3_rustc_invocation::{
        CompileEnvironmentV2, RustcInvocationDescriptorV2, RustcInvocationDescriptorV3, RustcUnitV2,
    };

    use super::*;
    use crate::{
        CodeObjectVersion, CompilerFfiEnvelopeV1, CompilerModuleHandoffV1, CompilerModuleKindV1,
        CompilerModuleSymbolManifestV1, CompilerModuleSymbolRoleV1, DeviceTargetV1,
    };

    const TARGET: &str = "gfx942:xnack-";
    const OTHER_TARGET: &str = "gfx942:sramecc+:xnack-";
    const OUTER_TOTAL_LEN_OFFSET: usize = 12;
    const OUTER_RESERVED_OFFSET: usize = 20;
    const CAPSULE_LEN_OFFSET: usize = 24;
    const MODULE_HANDOFF_LEN_OFFSET: usize = 32;
    const PAIR_VERSION_OFFSET: usize = 8;
    const PAIR_FLAGS_OFFSET: usize = 10;
    const PAIR_LENGTH_OFFSET: usize = 12;
    const PAIR_RESERVED_OFFSET: usize = 16;
    const PAIR_CAPSULE_IDENTITY_OFFSET: usize = 20;
    const PAIR_MODULE_IDENTITY_OFFSET: usize = 60;
    const PAIR_IDENTITY_OFFSET: usize = PAIR_BINDING_PREIMAGE_BYTES_V3;

    #[derive(Debug)]
    struct OuterLayout {
        capsule: std::ops::Range<usize>,
        module_handoff: std::ops::Range<usize>,
        pair_binding: std::ops::Range<usize>,
        outer_identity: std::ops::Range<usize>,
    }

    fn os_entries(entries: &[(&str, &str)]) -> Vec<(OsString, OsString)> {
        entries
            .iter()
            .map(|(key, value)| (OsString::from(key), OsString::from(value)))
            .collect()
    }

    fn target(text: &str) -> DeviceTargetV1 {
        DeviceTargetV1::parse(text).expect("canonical test target")
    }

    fn invocation(seed: u8, target: &str) -> RustcInvocationDescriptorV3 {
        let pins = [
            [seed.wrapping_add(1); 32],
            [seed.wrapping_add(2); 32],
            [seed.wrapping_add(3); 32],
            [seed.wrapping_add(4); 32],
            [seed.wrapping_add(5); 32],
            [seed.wrapping_add(6); 32],
        ];
        let closure = CompilerClosureV2::new(pins[0], pins[1], pins[2], pins[3], pins[4], pins[5])
            .expect("nonzero fixture closure");
        let rustc = RustcUnitV2::new(
            "/workspace/fe2o3",
            vec![
                "/opt/fe2o3/rustc".into(),
                "--crate-name".into(),
                format!("outer_v3_{seed:02x}"),
                "crates/outer-v3-fixture/src/lib.rs".into(),
                "--crate-type=lib".into(),
                "--edition=2024".into(),
                "-Zcodegen-backend=/opt/fe2o3/librustc_codegen_fe2o3.so".into(),
            ],
        )
        .expect("valid rustc fixture");
        let environment = CompileEnvironmentV2::from_child_environment(os_entries(&[
            ("CARGO_CFG_TARGET_ARCH", "amdgcn"),
            ("FE2O3_HSACO_DIR", "/workspace/fe2o3/target/fe2o3"),
            ("FE2O3_TARGET", target),
            ("FE2O3_VERIFY_KERNEL_IR", "1"),
        ]))
        .expect("valid exact environment");
        let v2 = RustcInvocationDescriptorV2::new(pins[3], pins[5], rustc, environment)
            .expect("valid V2 invocation");
        RustcInvocationDescriptorV3::new(v2, closure).expect("matching V3 compiler closure")
    }

    fn payload(label: &str, seed: u8) -> Vec<u8> {
        format!("fe2o3-outer-v3/{label}/seed-{seed:03}").into_bytes()
    }

    fn receipts(seed: u8, llvm_module: &[u8]) -> OrderedInertSemanticLineageReceiptsV3 {
        OrderedInertSemanticLineageReceiptsV3::new(
            InertRustcIdentityInventoryReceiptV3::from_canonical_preimage(payload(
                "inventory",
                seed,
            ))
            .unwrap(),
            InertRustcPreflightPlanReceiptV3::from_canonical_preimage(payload("preflight", seed))
                .unwrap(),
            InertCanonicalSemanticMirReceiptV3::from_canonical_preimage(payload(
                "semantic-mir",
                seed,
            ))
            .unwrap(),
            InertMiddleEndReceiptV3::from_canonical_preimage(payload("middle-end", seed)).unwrap(),
            InertKernelIrReceiptV3::from_canonical_preimage(payload("kernel-ir", seed)).unwrap(),
            InertMirToKirCorrespondenceReceiptV3::from_canonical_preimage(payload(
                "mir-to-kir",
                seed,
            ))
            .unwrap(),
            InertFormalMemoryReceiptV3::from_canonical_preimage(payload("formal-memory", seed))
                .unwrap(),
            InertProofBindingReceiptV3::from_canonical_preimage(payload("proof-binding", seed))
                .unwrap(),
            InertTargetBindingReceiptV3::from_canonical_preimage(payload("target-binding", seed))
                .unwrap(),
            InertDataLayoutReceiptV3::from_canonical_preimage(payload("data-layout", seed))
                .unwrap(),
            InertAbiReceiptV3::from_canonical_preimage(payload("abi", seed)).unwrap(),
            InertExportManifestReceiptV3::from_canonical_preimage(payload("export-manifest", seed))
                .unwrap(),
            InertAmdgpuLoweringReceiptV3::from_canonical_preimage(payload("amdgpu-lowering", seed))
                .unwrap(),
            InertSemanticToLlvmReceiptV3::from_canonical_preimage(payload(
                "semantic-to-llvm",
                seed,
            ))
            .unwrap(),
            InertLlvmModuleReceiptV3::from_canonical_preimage(llvm_module.to_vec()).unwrap(),
        )
    }

    fn llvm_module(seed: u8) -> Vec<u8> {
        format!(
            "; ModuleID = 'outer-v3-{seed:02x}'\ndefine amdgpu_kernel void @kernel() {{ ret void }}\n"
        )
        .into_bytes()
    }

    fn capsule(
        seed: u8,
        target_text: &str,
        llvm_module: &[u8],
    ) -> InertProductionSemanticCapsuleV3 {
        InertProductionSemanticCapsuleV3::new(
            invocation(seed, target_text),
            target(target_text),
            receipts(seed, llvm_module),
        )
        .expect("valid inert capsule fixture")
    }

    fn envelope(target_text: &str) -> CompilerFfiEnvelopeV1 {
        CompilerFfiEnvelopeV1::for_module_without_device_ffi(
            target(target_text),
            CodeObjectVersion::V5,
        )
        .expect("valid FFI-free envelope")
    }

    fn manifest() -> CompilerModuleSymbolManifestV1 {
        CompilerModuleSymbolManifestV1::new([
            (CompilerModuleSymbolRoleV1::KernelEntry, "kernel"),
            (CompilerModuleSymbolRoleV1::KernelDescriptor, "kernel.kd"),
        ])
        .expect("valid module symbol manifest")
    }

    fn module_handoff(seed: u8, target_text: &str) -> CompilerModuleHandoffV2 {
        let module = llvm_module(seed);
        CompilerModuleHandoffV2::new(
            CompilerModuleKindV1::LlvmTextIr,
            target(target_text),
            CodeObjectVersion::V5,
            envelope(target_text),
            manifest(),
            &module,
        )
        .expect("valid V2 module handoff fixture")
    }

    fn outer(seed: u8) -> InertSemanticCompilerModuleHandoffV3 {
        let module = llvm_module(seed);
        InertSemanticCompilerModuleHandoffV3::new(
            capsule(seed, TARGET, &module),
            module_handoff(seed, TARGET),
        )
        .expect("valid inert outer V3 fixture")
    }

    fn read_u64(bytes: &[u8], offset: usize) -> usize {
        usize::try_from(u64::from_le_bytes(
            bytes[offset..offset + 8].try_into().unwrap(),
        ))
        .unwrap()
    }

    fn layout(bytes: &[u8]) -> OuterLayout {
        let capsule_start = HEADER_BYTES_V3;
        let capsule_end = capsule_start + read_u64(bytes, CAPSULE_LEN_OFFSET);
        let module_handoff_end = capsule_end + read_u64(bytes, MODULE_HANDOFF_LEN_OFFSET);
        let pair_binding_end = module_handoff_end + INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V3;
        let outer_identity_end = pair_binding_end + SHA256_BYTES;
        assert_eq!(outer_identity_end, bytes.len());
        OuterLayout {
            capsule: capsule_start..capsule_end,
            module_handoff: capsule_end..module_handoff_end,
            pair_binding: module_handoff_end..pair_binding_end,
            outer_identity: pair_binding_end..outer_identity_end,
        }
    }

    fn rehash_pair_binding(bytes: &mut [u8], layout: &OuterLayout) {
        let pair = &mut bytes[layout.pair_binding.clone()];
        let sha256 = derive_identity_sha256(
            PAIR_BINDING_IDENTITY_DOMAIN_V3,
            &pair[..PAIR_BINDING_PREIMAGE_BYTES_V3],
        )
        .unwrap();
        pair[PAIR_IDENTITY_OFFSET..].copy_from_slice(&sha256);
    }

    fn rehash_outer(bytes: &mut [u8], layout: &OuterLayout) {
        let sha256 = derive_identity_sha256(
            OUTER_IDENTITY_DOMAIN_V3,
            &bytes[..layout.outer_identity.start],
        )
        .unwrap();
        bytes[layout.outer_identity.clone()].copy_from_slice(&sha256);
    }

    fn raw_outer(capsule_bytes: &[u8], module_handoff_bytes: &[u8]) -> Vec<u8> {
        let capsule_sha256: [u8; SHA256_BYTES] = capsule_bytes
            [capsule_bytes.len() - SHA256_BYTES..]
            .try_into()
            .unwrap();
        let module_handoff_sha256: [u8; SHA256_BYTES] = Sha256::digest(module_handoff_bytes).into();
        let total_len = exact_outer_len(capsule_bytes.len(), module_handoff_bytes.len()).unwrap();

        let mut pair = [0_u8; INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V3];
        let mut pair_offset = 0;
        put_slice(
            &mut pair,
            &mut pair_offset,
            &INERT_COMPILER_MODULE_PAIR_BINDING_MAGIC_V3,
        );
        put_slice(
            &mut pair,
            &mut pair_offset,
            &INERT_COMPILER_MODULE_PAIR_BINDING_VERSION_V3.to_le_bytes(),
        );
        put_slice(&mut pair, &mut pair_offset, &0_u16.to_le_bytes());
        put_slice(
            &mut pair,
            &mut pair_offset,
            &(INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V3 as u32).to_le_bytes(),
        );
        put_slice(&mut pair, &mut pair_offset, &0_u32.to_le_bytes());
        put_slice(&mut pair, &mut pair_offset, &capsule_sha256);
        put_slice(
            &mut pair,
            &mut pair_offset,
            &(capsule_bytes.len() as u64).to_le_bytes(),
        );
        put_slice(&mut pair, &mut pair_offset, &module_handoff_sha256);
        put_slice(
            &mut pair,
            &mut pair_offset,
            &(module_handoff_bytes.len() as u64).to_le_bytes(),
        );
        let binding_sha256 = derive_identity_sha256(
            PAIR_BINDING_IDENTITY_DOMAIN_V3,
            &pair[..PAIR_BINDING_PREIMAGE_BYTES_V3],
        )
        .unwrap();
        put_slice(&mut pair, &mut pair_offset, &binding_sha256);
        assert_eq!(pair_offset, INERT_COMPILER_MODULE_PAIR_BINDING_BYTES_V3);

        let mut bytes = Vec::with_capacity(total_len);
        bytes.extend_from_slice(&INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_MAGIC_V3);
        bytes.extend_from_slice(&INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_VERSION_V3.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&(total_len as u64).to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&(capsule_bytes.len() as u64).to_le_bytes());
        bytes.extend_from_slice(&(module_handoff_bytes.len() as u64).to_le_bytes());
        bytes.extend_from_slice(capsule_bytes);
        bytes.extend_from_slice(module_handoff_bytes);
        bytes.extend_from_slice(&pair);
        let outer_sha256 = derive_identity_sha256(OUTER_IDENTITY_DOMAIN_V3, &bytes).unwrap();
        bytes.extend_from_slice(&outer_sha256);
        assert_eq!(bytes.len(), total_len);
        bytes
    }

    #[test]
    fn round_trip_retains_exact_inner_bytes_and_native_identities() {
        let value = outer(0x10);
        let bytes = value.canonical_bytes();
        let wire = layout(bytes);
        let decoded = InertSemanticCompilerModuleHandoffV3::decode(bytes).unwrap();
        let via_try_from = InertSemanticCompilerModuleHandoffV3::try_from(bytes).unwrap();

        assert_eq!(decoded, value);
        assert_eq!(via_try_from, value);
        assert_eq!(
            &bytes[..8],
            &INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_MAGIC_V3
        );
        assert_eq!(read_u64(bytes, OUTER_TOTAL_LEN_OFFSET), bytes.len());
        assert_eq!(&bytes[wire.capsule], value.capsule().canonical_bytes());
        assert_eq!(
            &bytes[wire.module_handoff],
            value.module_handoff().canonical_bytes()
        );
        assert_eq!(
            value.pair_binding().capsule_identity(),
            value.capsule().identity()
        );
        assert_eq!(
            value.pair_binding().module_handoff_identity(),
            value.module_handoff().identity()
        );
        assert!(
            value
                .pair_binding()
                .identity()
                .matches_canonical_bytes(value.pair_binding().canonical_bytes())
        );
        assert!(value.identity().matches_canonical_bytes(bytes));
        assert_eq!(value.identity().byte_len(), bytes.len() as u64);
    }

    #[test]
    fn pair_binding_and_outer_identity_are_separate_and_acyclic() {
        let value = outer(0x11);
        let wire = layout(value.canonical_bytes());
        let pair_bytes = &value.canonical_bytes()[wire.pair_binding.clone()];
        let outer_identity = value.identity();

        assert_eq!(pair_bytes, value.pair_binding().canonical_bytes());
        assert_eq!(
            &pair_bytes[PAIR_CAPSULE_IDENTITY_OFFSET..PAIR_CAPSULE_IDENTITY_OFFSET + SHA256_BYTES],
            value.capsule().identity().sha256()
        );
        assert_eq!(
            &pair_bytes[PAIR_MODULE_IDENTITY_OFFSET..PAIR_MODULE_IDENTITY_OFFSET + SHA256_BYTES],
            value.module_handoff().identity().sha256()
        );
        assert_eq!(
            derive_identity_sha256(
                PAIR_BINDING_IDENTITY_DOMAIN_V3,
                &pair_bytes[..PAIR_BINDING_PREIMAGE_BYTES_V3]
            ),
            Some(*value.pair_binding().identity().sha256())
        );
        assert_eq!(
            derive_identity_sha256(
                OUTER_IDENTITY_DOMAIN_V3,
                &value.canonical_bytes()[..wire.outer_identity.start]
            ),
            Some(*outer_identity.sha256())
        );

        let mut only_outer_identity_changed = value.canonical_bytes().to_vec();
        only_outer_identity_changed[wire.outer_identity.start] ^= 1;
        assert!(
            value
                .pair_binding()
                .identity()
                .matches_canonical_bytes(&only_outer_identity_changed[wire.pair_binding])
        );
    }

    #[test]
    fn fully_rehashed_cross_producer_splice_is_accepted_only_as_inert_content() {
        let capsule_module = llvm_module(0x20);
        let capsule = capsule(0x20, TARGET, &capsule_module);
        let unrelated_handoff = module_handoff(0x21, TARGET);
        assert_ne!(
            capsule.receipts().llvm_module().canonical_preimage(),
            unrelated_handoff.module_bytes()
        );

        let splice = InertSemanticCompilerModuleHandoffV3::new(capsule, unrelated_handoff)
            .expect("public inert construction accepts an internally valid splice");
        let decoded = InertSemanticCompilerModuleHandoffV3::decode(splice.canonical_bytes())
            .expect("fully rehashed inert content remains structurally valid");

        assert_eq!(decoded, splice);
        assert!(!decoded.authenticates_producer());
        assert!(!decoded.authenticates_compiler_origin());
        assert!(!decoded.grants_compiler_authority());
        assert!(!decoded.grants_artifact_authority());
        assert!(!decoded.grants_worker_authority());
        assert!(!decoded.grants_link_authority());
        assert!(!decoded.grants_publication_authority());
        assert!(!decoded.grants_load_authority());
        assert!(!decoded.grants_launch_authority());
        assert!(!decoded.pair_binding().authenticates_producer());
        assert!(!decoded.pair_binding().grants_publication_authority());
        assert!(!decoded.pair_binding().grants_load_authority());
        assert!(!decoded.pair_binding().grants_launch_authority());
    }

    #[test]
    fn exact_member_substitution_is_rejected_even_after_rehashing_the_outer_identity() {
        let first = outer(0x30);
        let replacement = outer(0x31);
        let first_layout = layout(first.canonical_bytes());
        let replacement_layout = layout(replacement.canonical_bytes());
        assert_eq!(first_layout.capsule.len(), replacement_layout.capsule.len());
        assert_eq!(
            first_layout.module_handoff.len(),
            replacement_layout.module_handoff.len()
        );

        let mut capsule_substitution = first.canonical_bytes().to_vec();
        capsule_substitution[first_layout.capsule.clone()]
            .copy_from_slice(&replacement.canonical_bytes()[replacement_layout.capsule.clone()]);
        rehash_outer(&mut capsule_substitution, &first_layout);
        assert_eq!(
            InertSemanticCompilerModuleHandoffV3::decode(&capsule_substitution),
            Err(InertSemanticCompilerModuleHandoffErrorV3::CapsuleIdentityMismatch)
        );

        let mut module_substitution = first.canonical_bytes().to_vec();
        module_substitution[first_layout.module_handoff.clone()]
            .copy_from_slice(&replacement.canonical_bytes()[replacement_layout.module_handoff]);
        rehash_outer(&mut module_substitution, &first_layout);
        assert_eq!(
            InertSemanticCompilerModuleHandoffV3::decode(&module_substitution),
            Err(InertSemanticCompilerModuleHandoffErrorV3::ModuleHandoffIdentityMismatch)
        );
    }

    #[test]
    fn outer_header_and_declared_bounds_are_rejected_before_nested_decode() {
        let encoded = outer(0x40).canonical_bytes().to_vec();

        let mut bad_magic = encoded.clone();
        bad_magic[0] ^= 1;
        assert_eq!(
            InertSemanticCompilerModuleHandoffV3::decode(&bad_magic),
            Err(InertSemanticCompilerModuleHandoffErrorV3::InvalidMagic)
        );

        let mut bad_version = encoded.clone();
        bad_version[8..10].copy_from_slice(&2_u16.to_le_bytes());
        assert_eq!(
            InertSemanticCompilerModuleHandoffV3::decode(&bad_version),
            Err(InertSemanticCompilerModuleHandoffErrorV3::UnsupportedVersion(2))
        );

        let mut bad_flags = encoded.clone();
        bad_flags[10..12].copy_from_slice(&1_u16.to_le_bytes());
        assert_eq!(
            InertSemanticCompilerModuleHandoffV3::decode(&bad_flags),
            Err(InertSemanticCompilerModuleHandoffErrorV3::UnsupportedFlags(
                1
            ))
        );

        let mut bad_reserved = encoded.clone();
        bad_reserved[OUTER_RESERVED_OFFSET..OUTER_RESERVED_OFFSET + 4]
            .copy_from_slice(&1_u32.to_le_bytes());
        assert_eq!(
            InertSemanticCompilerModuleHandoffV3::decode(&bad_reserved),
            Err(InertSemanticCompilerModuleHandoffErrorV3::NonzeroReserved)
        );

        let mut capsule_too_large = encoded.clone();
        capsule_too_large[CAPSULE_LEN_OFFSET..CAPSULE_LEN_OFFSET + 8].copy_from_slice(
            &((MAX_INERT_PRODUCTION_SEMANTIC_CAPSULE_BYTES_V3 as u64) + 1).to_le_bytes(),
        );
        assert_eq!(
            InertSemanticCompilerModuleHandoffV3::decode(&capsule_too_large),
            Err(InertSemanticCompilerModuleHandoffErrorV3::CapsuleByteBoundExceeded)
        );

        let mut module_too_large = encoded.clone();
        module_too_large[MODULE_HANDOFF_LEN_OFFSET..MODULE_HANDOFF_LEN_OFFSET + 8]
            .copy_from_slice(&((MAX_COMPILER_MODULE_HANDOFF_BYTES_V2 as u64) + 1).to_le_bytes());
        assert_eq!(
            InertSemanticCompilerModuleHandoffV3::decode(&module_too_large),
            Err(InertSemanticCompilerModuleHandoffErrorV3::ModuleHandoffByteBoundExceeded)
        );

        let mut aggregate_too_large = encoded.clone();
        aggregate_too_large[OUTER_TOTAL_LEN_OFFSET..OUTER_TOTAL_LEN_OFFSET + 8].copy_from_slice(
            &((MAX_INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_BYTES_V3 as u64) + 1).to_le_bytes(),
        );
        assert_eq!(
            InertSemanticCompilerModuleHandoffV3::decode(&aggregate_too_large),
            Err(InertSemanticCompilerModuleHandoffErrorV3::OuterByteBoundExceeded)
        );
    }

    #[test]
    fn pair_binding_header_identity_and_member_metadata_are_strict() {
        let original = outer(0x50);
        let wire = layout(original.canonical_bytes());

        for (offset, expected) in [
            (
                0,
                InertSemanticCompilerModuleHandoffErrorV3::InvalidPairBindingMagic,
            ),
            (
                PAIR_VERSION_OFFSET,
                InertSemanticCompilerModuleHandoffErrorV3::UnsupportedPairBindingVersion(2),
            ),
            (
                PAIR_FLAGS_OFFSET,
                InertSemanticCompilerModuleHandoffErrorV3::UnsupportedPairBindingFlags(1),
            ),
            (
                PAIR_RESERVED_OFFSET,
                InertSemanticCompilerModuleHandoffErrorV3::NonzeroPairBindingReserved,
            ),
        ] {
            let mut bytes = original.canonical_bytes().to_vec();
            let absolute = wire.pair_binding.start + offset;
            match offset {
                PAIR_VERSION_OFFSET | PAIR_FLAGS_OFFSET => {
                    bytes[absolute..absolute + 2].copy_from_slice(
                        &(if offset == PAIR_VERSION_OFFSET {
                            2_u16
                        } else {
                            1_u16
                        })
                        .to_le_bytes(),
                    );
                }
                PAIR_RESERVED_OFFSET => {
                    bytes[absolute..absolute + 4].copy_from_slice(&1_u32.to_le_bytes());
                }
                _ => bytes[absolute] ^= 1,
            }
            rehash_pair_binding(&mut bytes, &wire);
            rehash_outer(&mut bytes, &wire);
            assert_eq!(
                InertSemanticCompilerModuleHandoffV3::decode(&bytes),
                Err(expected)
            );
        }

        let mut bad_length = original.canonical_bytes().to_vec();
        let length = wire.pair_binding.start + PAIR_LENGTH_OFFSET;
        bad_length[length..length + 4].copy_from_slice(&0_u32.to_le_bytes());
        rehash_pair_binding(&mut bad_length, &wire);
        rehash_outer(&mut bad_length, &wire);
        assert_eq!(
            InertSemanticCompilerModuleHandoffV3::decode(&bad_length),
            Err(InertSemanticCompilerModuleHandoffErrorV3::InvalidPairBindingLength(0))
        );

        let mut bad_binding_identity = original.canonical_bytes().to_vec();
        bad_binding_identity[wire.pair_binding.start + PAIR_IDENTITY_OFFSET] ^= 1;
        rehash_outer(&mut bad_binding_identity, &wire);
        assert_eq!(
            InertSemanticCompilerModuleHandoffV3::decode(&bad_binding_identity),
            Err(InertSemanticCompilerModuleHandoffErrorV3::PairBindingIdentityMismatch)
        );

        let mut bad_member = original.canonical_bytes().to_vec();
        bad_member[wire.pair_binding.start + PAIR_CAPSULE_IDENTITY_OFFSET] ^= 1;
        rehash_pair_binding(&mut bad_member, &wire);
        rehash_outer(&mut bad_member, &wire);
        assert_eq!(
            InertSemanticCompilerModuleHandoffV3::decode(&bad_member),
            Err(InertSemanticCompilerModuleHandoffErrorV3::CapsuleIdentityMismatch)
        );

        let mut bad_member_length = original.canonical_bytes().to_vec();
        let capsule_length = read_u64(&bad_member_length, CAPSULE_LEN_OFFSET) as u64;
        let pair_capsule_length = wire.pair_binding.start + 52;
        bad_member_length[pair_capsule_length..pair_capsule_length + 8]
            .copy_from_slice(&(capsule_length - 1).to_le_bytes());
        rehash_pair_binding(&mut bad_member_length, &wire);
        rehash_outer(&mut bad_member_length, &wire);
        assert_eq!(
            InertSemanticCompilerModuleHandoffV3::decode(&bad_member_length),
            Err(InertSemanticCompilerModuleHandoffErrorV3::PairBindingInnerMismatch)
        );
    }

    #[test]
    fn earlier_outer_and_inner_handoff_versions_are_rejected_without_fallback() {
        let v2 = module_handoff(0x60, TARGET);
        assert_eq!(
            InertSemanticCompilerModuleHandoffV3::decode(v2.canonical_bytes()),
            Err(InertSemanticCompilerModuleHandoffErrorV3::InvalidMagic)
        );

        let module = llvm_module(0x60);
        let capsule = capsule(0x60, TARGET, &module);
        let v1 = CompilerModuleHandoffV1::new(
            CompilerModuleKindV1::LlvmTextIr,
            target(TARGET),
            CodeObjectVersion::V5,
            envelope(TARGET),
            &module,
        )
        .unwrap();
        let cross_version = raw_outer(capsule.canonical_bytes(), v1.canonical_bytes());
        assert_eq!(
            InertSemanticCompilerModuleHandoffV3::decode(&cross_version),
            Err(InertSemanticCompilerModuleHandoffErrorV3::ModuleHandoff(
                CompilerModuleHandoffErrorV2::InvalidMagic
            ))
        );
    }

    #[test]
    fn target_disagreement_is_rejected_on_construction_and_strict_decode() {
        let module = llvm_module(0x70);
        let capsule = capsule(0x70, TARGET, &module);
        let other_handoff = module_handoff(0x70, OTHER_TARGET);
        assert_eq!(
            InertSemanticCompilerModuleHandoffV3::new(
                InertProductionSemanticCapsuleV3::decode(capsule.canonical_bytes()).unwrap(),
                CompilerModuleHandoffV2::decode(other_handoff.canonical_bytes()).unwrap(),
            ),
            Err(InertSemanticCompilerModuleHandoffErrorV3::TargetMismatch)
        );

        let raw = raw_outer(capsule.canonical_bytes(), other_handoff.canonical_bytes());
        assert_eq!(
            InertSemanticCompilerModuleHandoffV3::decode(&raw),
            Err(InertSemanticCompilerModuleHandoffErrorV3::TargetMismatch)
        );
    }

    #[test]
    fn every_truncation_trailing_byte_and_single_bit_mutation_is_rejected() {
        let encoded = outer(0x80).canonical_bytes().to_vec();
        for length in 0..encoded.len() {
            assert!(
                InertSemanticCompilerModuleHandoffV3::decode(&encoded[..length]).is_err(),
                "accepted prefix of length {length}"
            );
        }

        let mut trailing = encoded.clone();
        trailing.push(0);
        assert_eq!(
            InertSemanticCompilerModuleHandoffV3::decode(&trailing),
            Err(InertSemanticCompilerModuleHandoffErrorV3::TrailingBytes)
        );

        for index in 0..encoded.len() {
            for bit in 0..8 {
                let mut mutated = encoded.clone();
                mutated[index] ^= 1 << bit;
                assert!(
                    InertSemanticCompilerModuleHandoffV3::decode(&mutated).is_err(),
                    "accepted bit {bit} mutation at byte {index}"
                );
            }
        }
    }

    #[test]
    fn preflight_matches_exact_wire_size_and_move_out_preserves_inner_bytes() {
        let module = llvm_module(0x90);
        let capsule = capsule(0x90, TARGET, &module);
        let handoff = module_handoff(0x90, TARGET);
        let expected_capsule = capsule.canonical_bytes().to_vec();
        let expected_handoff = handoff.canonical_bytes().to_vec();
        let preflight =
            preflight_inert_semantic_compiler_module_handoff_v3(&capsule, &handoff).unwrap();
        assert_eq!(preflight.capsule_bytes(), expected_capsule.len());
        assert_eq!(preflight.module_handoff_bytes(), expected_handoff.len());
        assert_eq!(
            preflight.exact_outer_bytes(),
            OUTER_FIXED_BYTES_V3 + expected_capsule.len() + expected_handoff.len()
        );

        let outer = InertSemanticCompilerModuleHandoffV3::new(capsule, handoff).unwrap();
        assert_eq!(outer.canonical_bytes().len(), preflight.exact_outer_bytes());
        let (capsule, handoff) = outer.into_capsule_and_module_handoff();
        assert_eq!(capsule.canonical_bytes(), expected_capsule);
        assert_eq!(handoff.canonical_bytes(), expected_handoff);
    }

    #[test]
    fn impossible_empty_truncated_and_trailing_declared_lengths_fail_preflight() {
        let encoded = outer(0x91).canonical_bytes().to_vec();

        let mut impossible = encoded.clone();
        impossible[OUTER_TOTAL_LEN_OFFSET..OUTER_TOTAL_LEN_OFFSET + 8]
            .copy_from_slice(&1_u64.to_le_bytes());
        assert_eq!(
            InertSemanticCompilerModuleHandoffV3::decode(&impossible),
            Err(InertSemanticCompilerModuleHandoffErrorV3::InvalidLength(1))
        );

        let mut declared_trailing = encoded.clone();
        declared_trailing[OUTER_TOTAL_LEN_OFFSET..OUTER_TOTAL_LEN_OFFSET + 8]
            .copy_from_slice(&((encoded.len() - 1) as u64).to_le_bytes());
        assert_eq!(
            InertSemanticCompilerModuleHandoffV3::decode(&declared_trailing),
            Err(InertSemanticCompilerModuleHandoffErrorV3::TrailingBytes)
        );

        let mut declared_truncated = encoded.clone();
        declared_truncated[OUTER_TOTAL_LEN_OFFSET..OUTER_TOTAL_LEN_OFFSET + 8]
            .copy_from_slice(&((encoded.len() + 1) as u64).to_le_bytes());
        assert_eq!(
            InertSemanticCompilerModuleHandoffV3::decode(&declared_truncated),
            Err(InertSemanticCompilerModuleHandoffErrorV3::Truncated)
        );

        for (offset, error) in [
            (
                CAPSULE_LEN_OFFSET,
                InertSemanticCompilerModuleHandoffErrorV3::CapsuleByteBoundExceeded,
            ),
            (
                MODULE_HANDOFF_LEN_OFFSET,
                InertSemanticCompilerModuleHandoffErrorV3::ModuleHandoffByteBoundExceeded,
            ),
        ] {
            let mut empty = encoded.clone();
            empty[offset..offset + 8].copy_from_slice(&0_u64.to_le_bytes());
            assert_eq!(
                InertSemanticCompilerModuleHandoffV3::decode(&empty),
                Err(error)
            );
        }
    }

    #[test]
    fn zero_outer_pair_and_member_identities_fail_closed() {
        let value = outer(0x92);
        let wire = layout(value.canonical_bytes());

        let mut zero_outer = value.canonical_bytes().to_vec();
        zero_outer[wire.outer_identity.clone()].fill(0);
        assert_eq!(
            InertSemanticCompilerModuleHandoffV3::decode(&zero_outer),
            Err(InertSemanticCompilerModuleHandoffErrorV3::ZeroIdentity {
                field: "inert semantic compiler module handoff"
            })
        );

        let mut zero_pair = value.canonical_bytes().to_vec();
        zero_pair[wire.pair_binding.start + PAIR_IDENTITY_OFFSET..wire.pair_binding.end].fill(0);
        rehash_outer(&mut zero_pair, &wire);
        assert_eq!(
            InertSemanticCompilerModuleHandoffV3::decode(&zero_pair),
            Err(InertSemanticCompilerModuleHandoffErrorV3::ZeroIdentity {
                field: "inert compiler module pair binding"
            })
        );

        for (offset, field) in [
            (
                PAIR_CAPSULE_IDENTITY_OFFSET,
                "inert semantic capsule pair member",
            ),
            (
                PAIR_MODULE_IDENTITY_OFFSET,
                "V2 compiler module handoff pair member",
            ),
        ] {
            let mut zero_member = value.canonical_bytes().to_vec();
            let start = wire.pair_binding.start + offset;
            zero_member[start..start + SHA256_BYTES].fill(0);
            rehash_pair_binding(&mut zero_member, &wire);
            rehash_outer(&mut zero_member, &wire);
            assert_eq!(
                InertSemanticCompilerModuleHandoffV3::decode(&zero_member),
                Err(InertSemanticCompilerModuleHandoffErrorV3::ZeroIdentity { field })
            );
        }
    }

    #[test]
    fn terminal_outer_identity_and_debug_output_do_not_leak_payload_text() {
        let value = outer(0xa0);
        let wire = layout(value.canonical_bytes());
        let mut bad_identity = value.canonical_bytes().to_vec();
        bad_identity[wire.outer_identity.start] ^= 1;
        assert_eq!(
            InertSemanticCompilerModuleHandoffV3::decode(&bad_identity),
            Err(InertSemanticCompilerModuleHandoffErrorV3::OuterIdentityMismatch)
        );

        let debug = format!("{value:?}");
        for payload_text in ["outer-v3-a0", "semantic-mir", "kernel"] {
            assert!(
                !debug.contains(payload_text),
                "debug output leaked payload text `{payload_text}`: {debug}"
            );
        }
    }
}
