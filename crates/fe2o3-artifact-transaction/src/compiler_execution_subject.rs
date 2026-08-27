use std::{error::Error, fmt};

use fe2o3_build_authority::{CompilerClosureErrorV2, CompilerClosureV2};
use fe2o3_compiler_ffi::{
    InertSemanticCompilerModuleHandoffErrorV3, InertSemanticCompilerModuleHandoffV3,
    preflight_inert_semantic_compiler_module_handoff_v3,
};
use sha2::{Digest, Sha256};

use crate::{
    AttemptCodecError, BuildAttempt, BuildInvocation, BuildSession, CompilerModuleHandoffReceiptV3,
    CompilerModuleHandoffSlotV3, CompilerModuleHandoffTransactionIdentityV3,
    ConsumedCompilerModuleHandoffV3,
};

/// Fixed magic at the start of every inert compiler-execution subject V1.
pub const INERT_COMPILER_EXECUTION_SUBJECT_MAGIC_V1: [u8; 8] = *b"F2O3CES1";

/// The only inert compiler-execution subject version implemented by this crate.
pub const INERT_COMPILER_EXECUTION_SUBJECT_VERSION_V1: u16 = 1;

const SUBJECT_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/INERT-COMPILER-EXECUTION-SUBJECT/V1\0";
const SHA256_BYTES: usize = 32;
const HEADER_BYTES: usize = 8 + 2 + 2 + 8 + 4;
const BUILD_ATTEMPT_BYTES: usize = 8 + 16 + SHA256_BYTES;
const SLOT_BYTES: usize = 1 + 7;
const COMPILER_CLOSURE_BYTES: usize = (6 * SHA256_BYTES) + 2 + SHA256_BYTES;
const CONTENT_BINDING_BYTES: usize = SHA256_BYTES + 8;
const CONTENT_BINDING_COUNT: usize = 7;
const SUBJECT_PREIMAGE_BYTES: usize = HEADER_BYTES
    + BUILD_ATTEMPT_BYTES
    + SLOT_BYTES
    + SHA256_BYTES
    + SHA256_BYTES
    + COMPILER_CLOSURE_BYTES
    + (CONTENT_BINDING_COUNT * CONTENT_BINDING_BYTES);

/// Exact canonical byte length of one inert compiler-execution subject V1.
pub const INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1: usize = SUBJECT_PREIMAGE_BYTES + SHA256_BYTES;

/// Domain-separated identity of one complete canonical execution subject.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InertCompilerExecutionSubjectIdentityV1 {
    sha256: [u8; SHA256_BYTES],
    byte_len: u64,
}

impl InertCompilerExecutionSubjectIdentityV1 {
    /// Returns the domain-separated subject digest.
    pub const fn sha256(&self) -> &[u8; SHA256_BYTES] {
        &self.sha256
    }

    /// Returns the complete canonical subject length.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    /// Checks exact canonical bytes without granting authority.
    pub fn matches_canonical_bytes(self, bytes: &[u8]) -> bool {
        self.byte_len == bytes.len() as u64
            && bytes.len() == INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1
            && bytes[SUBJECT_PREIMAGE_BYTES..] == self.sha256
            && derive_subject_identity(&bytes[..SUBJECT_PREIMAGE_BYTES]) == self.sha256
    }
}

impl fmt::Debug for InertCompilerExecutionSubjectIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InertCompilerExecutionSubjectIdentityV1")
            .field("sha256", &self.sha256)
            .field("byte_len", &self.byte_len)
            .finish()
    }
}

/// One exact content identity and canonical byte length retained by an execution subject.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InertCompilerExecutionContentBindingV1 {
    sha256: [u8; SHA256_BYTES],
    byte_len: u64,
}

impl InertCompilerExecutionContentBindingV1 {
    fn new(
        sha256: [u8; SHA256_BYTES],
        byte_len: u64,
        field: &'static str,
    ) -> Result<Self, CompilerExecutionSubjectErrorV1> {
        if sha256 == [0; SHA256_BYTES] {
            return Err(CompilerExecutionSubjectErrorV1::ZeroIdentity { field });
        }
        if byte_len == 0 {
            return Err(CompilerExecutionSubjectErrorV1::ZeroLength { field });
        }
        Ok(Self { sha256, byte_len })
    }

    /// Returns the exact domain-separated content digest.
    pub const fn sha256(&self) -> &[u8; SHA256_BYTES] {
        &self.sha256
    }

    /// Returns the exact canonical content length.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

/// Canonical authority-free subject for one published protected compiler occurrence.
///
/// The subject binds the durable build attempt, V3 invocation digest, complete compiler closure,
/// source inventory and preflight receipts, semantic capsule, final module commitment, native V2
/// module handoff, V3 pair binding, and exact outer V3 handoff transaction. Construction verifies
/// those axes against an actual strict publication or consumption result. Decoding verifies only
/// canonical bytes. Neither operation authenticates that the compiler occurrence happened.
#[derive(Clone, Eq, PartialEq)]
pub struct InertCompilerExecutionSubjectV1 {
    attempt: BuildAttempt,
    slot: CompilerModuleHandoffSlotV3,
    transaction_identity: CompilerModuleHandoffTransactionIdentityV3,
    rustc_invocation_sha256: [u8; SHA256_BYTES],
    compiler_closure: CompilerClosureV2,
    rustc_identity_inventory: InertCompilerExecutionContentBindingV1,
    rustc_preflight_plan: InertCompilerExecutionContentBindingV1,
    semantic_capsule: InertCompilerExecutionContentBindingV1,
    final_compiler_module_commitment: InertCompilerExecutionContentBindingV1,
    compiler_module_handoff: InertCompilerExecutionContentBindingV1,
    compiler_module_pair_binding: InertCompilerExecutionContentBindingV1,
    outer_handoff: InertCompilerExecutionContentBindingV1,
    identity: InertCompilerExecutionSubjectIdentityV1,
    canonical_bytes: [u8; INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1],
}

impl InertCompilerExecutionSubjectV1 {
    /// Derives one canonical subject from the exact durable publication receipt and handoff.
    pub fn from_publication(
        receipt: CompilerModuleHandoffReceiptV3,
        handoff: &InertSemanticCompilerModuleHandoffV3,
    ) -> Result<Self, CompilerExecutionSubjectErrorV1> {
        Self::from_exact_binding(
            receipt.attempt(),
            receipt.slot(),
            receipt.transaction_identity(),
            receipt.handoff_identity(),
            receipt.length(),
            handoff,
        )
    }

    /// Reconstructs the same subject from one strictly consumed V3 handoff.
    pub fn from_consumed(
        consumed: &ConsumedCompilerModuleHandoffV3,
    ) -> Result<Self, CompilerExecutionSubjectErrorV1> {
        Self::from_exact_binding(
            consumed.attempt(),
            consumed.slot(),
            consumed.transaction_identity(),
            consumed.handoff_identity(),
            consumed.bytes().len(),
            consumed.handoff(),
        )
    }

    fn from_exact_binding(
        attempt: BuildAttempt,
        slot: CompilerModuleHandoffSlotV3,
        transaction_identity: CompilerModuleHandoffTransactionIdentityV3,
        declared_handoff_identity: fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffIdentityV3,
        declared_handoff_len: usize,
        handoff: &InertSemanticCompilerModuleHandoffV3,
    ) -> Result<Self, CompilerExecutionSubjectErrorV1> {
        preflight_inert_semantic_compiler_module_handoff_v3(
            handoff.capsule(),
            handoff.module_handoff(),
        )
        .map_err(CompilerExecutionSubjectErrorV1::NonCanonicalHandoff)?;
        if !handoff
            .identity()
            .matches_canonical_bytes(handoff.canonical_bytes())
        {
            return Err(CompilerExecutionSubjectErrorV1::NonCanonicalOuterHandoff);
        }
        if declared_handoff_identity != handoff.identity() {
            return Err(CompilerExecutionSubjectErrorV1::HandoffIdentityMismatch);
        }
        if declared_handoff_len != handoff.canonical_bytes().len() {
            return Err(CompilerExecutionSubjectErrorV1::HandoffLengthMismatch);
        }
        if transaction_identity.as_bytes() == &[0; SHA256_BYTES] {
            return Err(CompilerExecutionSubjectErrorV1::ZeroIdentity {
                field: "V3 handoff transaction",
            });
        }

        let capsule = handoff.capsule();
        let receipts = capsule.receipts();
        let inventory = receipts.rustc_identity_inventory().identity();
        let preflight = receipts.rustc_preflight_plan().identity();
        let capsule_identity = capsule.identity();
        let final_commitment = receipts.final_compiler_module_commitment().identity();
        let module_handoff_identity = handoff.module_handoff().identity();
        let pair_binding_identity = handoff.pair_binding().identity();
        let outer_identity = handoff.identity();

        Self::from_fields(SubjectFieldsV1 {
            attempt,
            slot,
            transaction_identity,
            rustc_invocation_sha256: capsule.invocation_digest().into_bytes(),
            compiler_closure: *capsule.compiler_closure(),
            rustc_identity_inventory: InertCompilerExecutionContentBindingV1::new(
                *inventory.sha256(),
                inventory.byte_len(),
                "rustc identity inventory",
            )?,
            rustc_preflight_plan: InertCompilerExecutionContentBindingV1::new(
                *preflight.sha256(),
                preflight.byte_len(),
                "rustc preflight plan",
            )?,
            semantic_capsule: InertCompilerExecutionContentBindingV1::new(
                *capsule_identity.sha256(),
                capsule_identity.byte_len(),
                "semantic capsule",
            )?,
            final_compiler_module_commitment: InertCompilerExecutionContentBindingV1::new(
                *final_commitment.sha256(),
                final_commitment.byte_len(),
                "final compiler module commitment",
            )?,
            compiler_module_handoff: InertCompilerExecutionContentBindingV1::new(
                *module_handoff_identity.sha256(),
                module_handoff_identity.byte_len(),
                "compiler module handoff",
            )?,
            compiler_module_pair_binding: InertCompilerExecutionContentBindingV1::new(
                *pair_binding_identity.sha256(),
                pair_binding_identity.byte_len(),
                "compiler module pair binding",
            )?,
            outer_handoff: InertCompilerExecutionContentBindingV1::new(
                *outer_identity.sha256(),
                outer_identity.byte_len(),
                "outer V3 handoff",
            )?,
        })
    }

    fn from_fields(fields: SubjectFieldsV1) -> Result<Self, CompilerExecutionSubjectErrorV1> {
        if fields.rustc_invocation_sha256 == [0; SHA256_BYTES] {
            return Err(CompilerExecutionSubjectErrorV1::ZeroIdentity {
                field: "rustc invocation",
            });
        }
        if fields.transaction_identity.as_bytes() == &[0; SHA256_BYTES] {
            return Err(CompilerExecutionSubjectErrorV1::ZeroIdentity {
                field: "V3 handoff transaction",
            });
        }

        let mut canonical_bytes = [0_u8; INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1];
        let mut offset = 0;
        put_slice(
            &mut canonical_bytes,
            &mut offset,
            &INERT_COMPILER_EXECUTION_SUBJECT_MAGIC_V1,
        );
        put_slice(
            &mut canonical_bytes,
            &mut offset,
            &INERT_COMPILER_EXECUTION_SUBJECT_VERSION_V1.to_le_bytes(),
        );
        put_slice(&mut canonical_bytes, &mut offset, &0_u16.to_le_bytes());
        put_slice(
            &mut canonical_bytes,
            &mut offset,
            &(INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1 as u64).to_le_bytes(),
        );
        put_slice(&mut canonical_bytes, &mut offset, &0_u32.to_le_bytes());
        put_slice(
            &mut canonical_bytes,
            &mut offset,
            &fields.attempt.generation().to_le_bytes(),
        );
        put_slice(
            &mut canonical_bytes,
            &mut offset,
            fields.attempt.session().as_bytes(),
        );
        put_slice(
            &mut canonical_bytes,
            &mut offset,
            fields.attempt.invocation().as_bytes(),
        );
        canonical_bytes[offset] = fields.slot as u8;
        offset += SLOT_BYTES;
        put_slice(
            &mut canonical_bytes,
            &mut offset,
            fields.transaction_identity.as_bytes(),
        );
        put_slice(
            &mut canonical_bytes,
            &mut offset,
            &fields.rustc_invocation_sha256,
        );
        encode_compiler_closure(&mut canonical_bytes, &mut offset, fields.compiler_closure);
        for binding in fields.content_bindings() {
            put_slice(&mut canonical_bytes, &mut offset, binding.sha256());
            put_slice(
                &mut canonical_bytes,
                &mut offset,
                &binding.byte_len().to_le_bytes(),
            );
        }
        debug_assert_eq!(offset, SUBJECT_PREIMAGE_BYTES);
        let sha256 = derive_subject_identity(&canonical_bytes[..SUBJECT_PREIMAGE_BYTES]);
        if sha256 == [0; SHA256_BYTES] {
            return Err(CompilerExecutionSubjectErrorV1::ZeroIdentity {
                field: "compiler execution subject",
            });
        }
        put_slice(&mut canonical_bytes, &mut offset, &sha256);
        debug_assert_eq!(offset, INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1);

        Ok(Self {
            attempt: fields.attempt,
            slot: fields.slot,
            transaction_identity: fields.transaction_identity,
            rustc_invocation_sha256: fields.rustc_invocation_sha256,
            compiler_closure: fields.compiler_closure,
            rustc_identity_inventory: fields.rustc_identity_inventory,
            rustc_preflight_plan: fields.rustc_preflight_plan,
            semantic_capsule: fields.semantic_capsule,
            final_compiler_module_commitment: fields.final_compiler_module_commitment,
            compiler_module_handoff: fields.compiler_module_handoff,
            compiler_module_pair_binding: fields.compiler_module_pair_binding,
            outer_handoff: fields.outer_handoff,
            identity: InertCompilerExecutionSubjectIdentityV1 {
                sha256,
                byte_len: INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1 as u64,
            },
            canonical_bytes,
        })
    }

    /// Strictly decodes one exact canonical subject without authenticating it.
    pub fn decode(bytes: &[u8]) -> Result<Self, CompilerExecutionSubjectErrorV1> {
        if bytes.len() != INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1 {
            return Err(CompilerExecutionSubjectErrorV1::InvalidLength {
                actual: bytes.len(),
                expected: INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1,
            });
        }
        let mut reader = Reader::new(bytes);
        if reader.fixed::<8>()? != INERT_COMPILER_EXECUTION_SUBJECT_MAGIC_V1 {
            return Err(CompilerExecutionSubjectErrorV1::InvalidMagic);
        }
        let version = reader.u16()?;
        if version != INERT_COMPILER_EXECUTION_SUBJECT_VERSION_V1 {
            return Err(CompilerExecutionSubjectErrorV1::UnsupportedVersion(version));
        }
        let flags = reader.u16()?;
        if flags != 0 {
            return Err(CompilerExecutionSubjectErrorV1::UnsupportedFlags(flags));
        }
        let declared_len = reader.u64()?;
        if declared_len != INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1 as u64 {
            return Err(CompilerExecutionSubjectErrorV1::InvalidDeclaredLength(
                declared_len,
            ));
        }
        if reader.u32()? != 0 {
            return Err(CompilerExecutionSubjectErrorV1::NonzeroReserved);
        }

        let generation = reader.u64()?;
        let session = BuildSession::from_bytes(reader.fixed::<16>()?);
        let invocation = BuildInvocation::from_bytes(reader.fixed::<32>()?);
        let attempt = BuildAttempt::new(generation, session, invocation)
            .map_err(CompilerExecutionSubjectErrorV1::Attempt)?;
        let slot_value = reader.u8()?;
        let slot = decode_slot(slot_value)?;
        if reader.fixed::<7>()? != [0; 7] {
            return Err(CompilerExecutionSubjectErrorV1::NonzeroReserved);
        }
        let transaction_identity_bytes = reader.fixed::<32>()?;
        if transaction_identity_bytes == [0; SHA256_BYTES] {
            return Err(CompilerExecutionSubjectErrorV1::ZeroIdentity {
                field: "V3 handoff transaction",
            });
        }
        let transaction_identity =
            CompilerModuleHandoffTransactionIdentityV3::from_bytes(transaction_identity_bytes);
        let rustc_invocation_sha256 = reader.fixed::<32>()?;
        if rustc_invocation_sha256 == [0; SHA256_BYTES] {
            return Err(CompilerExecutionSubjectErrorV1::ZeroIdentity {
                field: "rustc invocation",
            });
        }
        let compiler_closure = decode_compiler_closure(&mut reader)?;

        let mut binding = |field| {
            InertCompilerExecutionContentBindingV1::new(reader.fixed::<32>()?, reader.u64()?, field)
        };
        let fields = SubjectFieldsV1 {
            attempt,
            slot,
            transaction_identity,
            rustc_invocation_sha256,
            compiler_closure,
            rustc_identity_inventory: binding("rustc identity inventory")?,
            rustc_preflight_plan: binding("rustc preflight plan")?,
            semantic_capsule: binding("semantic capsule")?,
            final_compiler_module_commitment: binding("final compiler module commitment")?,
            compiler_module_handoff: binding("compiler module handoff")?,
            compiler_module_pair_binding: binding("compiler module pair binding")?,
            outer_handoff: binding("outer V3 handoff")?,
        };
        debug_assert_eq!(reader.offset, SUBJECT_PREIMAGE_BYTES);
        let declared_identity = reader.fixed::<32>()?;
        if declared_identity == [0; SHA256_BYTES] {
            return Err(CompilerExecutionSubjectErrorV1::ZeroIdentity {
                field: "compiler execution subject",
            });
        }
        if !reader.is_empty() {
            return Err(CompilerExecutionSubjectErrorV1::TrailingBytes);
        }
        if derive_subject_identity(&bytes[..SUBJECT_PREIMAGE_BYTES]) != declared_identity {
            return Err(CompilerExecutionSubjectErrorV1::SubjectIdentityMismatch);
        }

        let decoded = Self::from_fields(fields)?;
        if decoded.canonical_bytes.as_slice() != bytes {
            return Err(CompilerExecutionSubjectErrorV1::NonCanonical);
        }
        Ok(decoded)
    }

    /// Returns the exact durable build attempt.
    pub const fn attempt(&self) -> BuildAttempt {
        self.attempt
    }

    /// Returns the exact V3 transaction slot.
    pub const fn slot(&self) -> CompilerModuleHandoffSlotV3 {
        self.slot
    }

    /// Returns the exact V3 transaction identity.
    pub const fn transaction_identity(&self) -> CompilerModuleHandoffTransactionIdentityV3 {
        self.transaction_identity
    }

    /// Returns the V3 canonical rustc invocation digest.
    pub const fn rustc_invocation_sha256(&self) -> &[u8; SHA256_BYTES] {
        &self.rustc_invocation_sha256
    }

    /// Returns the complete six-pin compiler closure.
    pub const fn compiler_closure(&self) -> CompilerClosureV2 {
        self.compiler_closure
    }

    /// Returns the rustc identity-inventory binding.
    pub const fn rustc_identity_inventory(&self) -> InertCompilerExecutionContentBindingV1 {
        self.rustc_identity_inventory
    }

    /// Returns the rustc preflight-plan binding.
    pub const fn rustc_preflight_plan(&self) -> InertCompilerExecutionContentBindingV1 {
        self.rustc_preflight_plan
    }

    /// Returns the complete semantic-capsule binding.
    pub const fn semantic_capsule(&self) -> InertCompilerExecutionContentBindingV1 {
        self.semantic_capsule
    }

    /// Returns the compact final compiler-module commitment binding.
    pub const fn final_compiler_module_commitment(&self) -> InertCompilerExecutionContentBindingV1 {
        self.final_compiler_module_commitment
    }

    /// Returns the native V2 compiler-module handoff binding.
    pub const fn compiler_module_handoff(&self) -> InertCompilerExecutionContentBindingV1 {
        self.compiler_module_handoff
    }

    /// Returns the V3 compiler-module pair-binding identity.
    pub const fn compiler_module_pair_binding(&self) -> InertCompilerExecutionContentBindingV1 {
        self.compiler_module_pair_binding
    }

    /// Returns the exact outer V3 handoff binding.
    pub const fn outer_handoff(&self) -> InertCompilerExecutionContentBindingV1 {
        self.outer_handoff
    }

    /// Returns the domain-separated identity of the complete subject.
    pub const fn identity(&self) -> InertCompilerExecutionSubjectIdentityV1 {
        self.identity
    }

    /// Returns the exact canonical subject bytes.
    pub const fn canonical_bytes(&self) -> &[u8; INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1] {
        &self.canonical_bytes
    }

    /// Reports that canonical content does not authenticate compiler execution.
    pub const fn authenticates_compiler_execution(&self) -> bool {
        false
    }

    /// Reports that the subject still requires a protected execution attestation.
    pub const fn requires_protected_execution_attestation(&self) -> bool {
        true
    }

    /// Reports that the subject grants no compiler authority.
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }

    /// Reports that the subject grants no publication authority.
    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    /// Reports that the subject grants no load authority.
    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    /// Reports that the subject grants no launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

impl fmt::Debug for InertCompilerExecutionSubjectV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InertCompilerExecutionSubjectV1")
            .field("attempt", &self.attempt)
            .field("slot", &self.slot)
            .field("transaction_identity", &self.transaction_identity)
            .field("rustc_invocation_sha256", &self.rustc_invocation_sha256)
            .field("compiler_closure", &self.compiler_closure)
            .field("rustc_identity_inventory", &self.rustc_identity_inventory)
            .field("rustc_preflight_plan", &self.rustc_preflight_plan)
            .field("semantic_capsule", &self.semantic_capsule)
            .field(
                "final_compiler_module_commitment",
                &self.final_compiler_module_commitment,
            )
            .field("compiler_module_handoff", &self.compiler_module_handoff)
            .field(
                "compiler_module_pair_binding",
                &self.compiler_module_pair_binding,
            )
            .field("outer_handoff", &self.outer_handoff)
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

struct SubjectFieldsV1 {
    attempt: BuildAttempt,
    slot: CompilerModuleHandoffSlotV3,
    transaction_identity: CompilerModuleHandoffTransactionIdentityV3,
    rustc_invocation_sha256: [u8; SHA256_BYTES],
    compiler_closure: CompilerClosureV2,
    rustc_identity_inventory: InertCompilerExecutionContentBindingV1,
    rustc_preflight_plan: InertCompilerExecutionContentBindingV1,
    semantic_capsule: InertCompilerExecutionContentBindingV1,
    final_compiler_module_commitment: InertCompilerExecutionContentBindingV1,
    compiler_module_handoff: InertCompilerExecutionContentBindingV1,
    compiler_module_pair_binding: InertCompilerExecutionContentBindingV1,
    outer_handoff: InertCompilerExecutionContentBindingV1,
}

impl SubjectFieldsV1 {
    const fn content_bindings(
        &self,
    ) -> [InertCompilerExecutionContentBindingV1; CONTENT_BINDING_COUNT] {
        [
            self.rustc_identity_inventory,
            self.rustc_preflight_plan,
            self.semantic_capsule,
            self.final_compiler_module_commitment,
            self.compiler_module_handoff,
            self.compiler_module_pair_binding,
            self.outer_handoff,
        ]
    }
}

/// Failure to construct or strictly decode one compiler-execution subject.
#[derive(Debug)]
pub enum CompilerExecutionSubjectErrorV1 {
    /// The retained strict handoff fails its native inner preflight.
    NonCanonicalHandoff(InertSemanticCompilerModuleHandoffErrorV3),
    /// The retained outer handoff identity does not match its canonical bytes.
    NonCanonicalOuterHandoff,
    /// A publication or consumption binding names a different handoff identity.
    HandoffIdentityMismatch,
    /// A publication or consumption binding names a different handoff length.
    HandoffLengthMismatch,
    /// The subject magic is not canonical.
    InvalidMagic,
    /// The subject version is unsupported.
    UnsupportedVersion(u16),
    /// Nonzero flags are unsupported.
    UnsupportedFlags(u16),
    /// The exact input length differs from the fixed schema length.
    InvalidLength {
        /// Observed input length.
        actual: usize,
        /// Required fixed length.
        expected: usize,
    },
    /// The wire-declared total length is not canonical.
    InvalidDeclaredLength(u64),
    /// A reserved field is nonzero.
    NonzeroReserved,
    /// A required identity is the reserved all-zero value.
    ZeroIdentity {
        /// Identity field name.
        field: &'static str,
    },
    /// A required canonical content length is zero.
    ZeroLength {
        /// Content field name.
        field: &'static str,
    },
    /// A V3 transaction slot is outside the closed schema.
    InvalidSlot(u8),
    /// The embedded build attempt is invalid.
    Attempt(AttemptCodecError),
    /// The embedded complete compiler closure is invalid.
    CompilerClosure(CompilerClosureErrorV2),
    /// The terminal subject identity does not match the exact prefix.
    SubjectIdentityMismatch,
    /// Decoding left bytes beyond the fixed schema.
    TrailingBytes,
    /// Decoded fields do not re-encode to the exact input bytes.
    NonCanonical,
    /// Input ended before the fixed schema was complete.
    Truncated,
}

impl fmt::Display for CompilerExecutionSubjectErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonCanonicalHandoff(error) => {
                write!(formatter, "strict V3 handoff preflight failed: {error}")
            }
            Self::NonCanonicalOuterHandoff => {
                formatter.write_str("strict V3 outer handoff identity does not match its bytes")
            }
            Self::HandoffIdentityMismatch => {
                formatter.write_str("publication binding and strict V3 handoff identities differ")
            }
            Self::HandoffLengthMismatch => {
                formatter.write_str("publication binding and strict V3 handoff lengths differ")
            }
            Self::InvalidMagic => formatter.write_str("invalid compiler-execution subject magic"),
            Self::UnsupportedVersion(version) => {
                write!(
                    formatter,
                    "unsupported compiler-execution subject version {version}"
                )
            }
            Self::UnsupportedFlags(flags) => {
                write!(
                    formatter,
                    "unsupported compiler-execution subject flags {flags:#06x}"
                )
            }
            Self::InvalidLength { actual, expected } => write!(
                formatter,
                "compiler-execution subject has {actual} bytes; expected exactly {expected}"
            ),
            Self::InvalidDeclaredLength(length) => write!(
                formatter,
                "compiler-execution subject declares noncanonical length {length}"
            ),
            Self::NonzeroReserved => {
                formatter.write_str("compiler-execution subject reserved bytes must be zero")
            }
            Self::ZeroIdentity { field } => write!(formatter, "{field} identity must be nonzero"),
            Self::ZeroLength { field } => {
                write!(formatter, "{field} canonical byte length must be nonzero")
            }
            Self::InvalidSlot(slot) => {
                write!(
                    formatter,
                    "compiler-execution subject slot {slot} is not canonical"
                )
            }
            Self::Attempt(error) => write!(formatter, "invalid build attempt: {error}"),
            Self::CompilerClosure(error) => {
                write!(formatter, "invalid complete compiler closure: {error}")
            }
            Self::SubjectIdentityMismatch => formatter
                .write_str("compiler-execution subject identity does not match its exact prefix"),
            Self::TrailingBytes => {
                formatter.write_str("compiler-execution subject has trailing bytes")
            }
            Self::NonCanonical => {
                formatter.write_str("compiler-execution subject encoding is noncanonical")
            }
            Self::Truncated => formatter.write_str("compiler-execution subject is truncated"),
        }
    }
}

impl Error for CompilerExecutionSubjectErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::NonCanonicalHandoff(error) => Some(error),
            Self::Attempt(error) => Some(error),
            Self::CompilerClosure(error) => Some(error),
            _ => None,
        }
    }
}

fn encode_compiler_closure(output: &mut [u8], offset: &mut usize, closure: CompilerClosureV2) {
    for digest in [
        closure.cargo_executable_sha256(),
        closure.cargo_binding_trampoline_sha256(),
        closure.cargo_fe2o3_binding_wrapper_sha256(),
        closure.rustc_executable_sha256(),
        closure.rustc_runtime_tree_sha256(),
        closure.codegen_backend_sha256(),
    ] {
        put_slice(output, offset, &digest);
    }
    put_slice(
        output,
        offset,
        &closure
            .cargo_binding_transition_protocol_version()
            .to_le_bytes(),
    );
    put_slice(output, offset, &closure.identity_sha256());
}

fn decode_compiler_closure(
    reader: &mut Reader<'_>,
) -> Result<CompilerClosureV2, CompilerExecutionSubjectErrorV1> {
    let cargo_executable_sha256 = reader.fixed::<32>()?;
    let cargo_binding_trampoline_sha256 = reader.fixed::<32>()?;
    let cargo_fe2o3_binding_wrapper_sha256 = reader.fixed::<32>()?;
    let rustc_executable_sha256 = reader.fixed::<32>()?;
    let rustc_runtime_tree_sha256 = reader.fixed::<32>()?;
    let codegen_backend_sha256 = reader.fixed::<32>()?;
    let transition_version = reader.u16()?;
    let identity_sha256 = reader.fixed::<32>()?;
    CompilerClosureV2::from_pins_and_identity(
        cargo_executable_sha256,
        cargo_binding_trampoline_sha256,
        cargo_fe2o3_binding_wrapper_sha256,
        rustc_executable_sha256,
        rustc_runtime_tree_sha256,
        codegen_backend_sha256,
        transition_version,
        identity_sha256,
    )
    .map_err(CompilerExecutionSubjectErrorV1::CompilerClosure)
}

fn decode_slot(value: u8) -> Result<CompilerModuleHandoffSlotV3, CompilerExecutionSubjectErrorV1> {
    match value {
        0 => Ok(CompilerModuleHandoffSlotV3::Default),
        1 => Ok(CompilerModuleHandoffSlotV3::GeneralGemmReference),
        2 => Ok(CompilerModuleHandoffSlotV3::GeneralGemmVectorizedAOnly),
        _ => Err(CompilerExecutionSubjectErrorV1::InvalidSlot(value)),
    }
}

fn derive_subject_identity(bytes: &[u8]) -> [u8; SHA256_BYTES] {
    let mut digest = Sha256::new();
    digest.update(SUBJECT_IDENTITY_DOMAIN_V1);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

fn put_slice(output: &mut [u8], offset: &mut usize, value: &[u8]) {
    let end = offset
        .checked_add(value.len())
        .expect("fixed compiler-execution subject offset cannot overflow");
    output[*offset..end].copy_from_slice(value);
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

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], CompilerExecutionSubjectErrorV1> {
        let end = self
            .offset
            .checked_add(N)
            .ok_or(CompilerExecutionSubjectErrorV1::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(CompilerExecutionSubjectErrorV1::Truncated)?;
        self.offset = end;
        value
            .try_into()
            .map_err(|_| CompilerExecutionSubjectErrorV1::Truncated)
    }

    fn u8(&mut self) -> Result<u8, CompilerExecutionSubjectErrorV1> {
        Ok(self.fixed::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, CompilerExecutionSubjectErrorV1> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, CompilerExecutionSubjectErrorV1> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn u64(&mut self) -> Result<u64, CompilerExecutionSubjectErrorV1> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }

    fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GENERATION_OFFSET: usize = HEADER_BYTES;
    const SESSION_OFFSET: usize = GENERATION_OFFSET + 8;
    const BUILD_INVOCATION_OFFSET: usize = SESSION_OFFSET + 16;
    const SLOT_OFFSET: usize = BUILD_INVOCATION_OFFSET + SHA256_BYTES;
    const TRANSACTION_OFFSET: usize = SLOT_OFFSET + SLOT_BYTES;
    const RUSTC_INVOCATION_OFFSET: usize = TRANSACTION_OFFSET + SHA256_BYTES;
    const CLOSURE_OFFSET: usize = RUSTC_INVOCATION_OFFSET + SHA256_BYTES;
    const CONTENT_BINDINGS_OFFSET: usize = CLOSURE_OFFSET + COMPILER_CLOSURE_BYTES;

    fn digest(seed: u8) -> [u8; SHA256_BYTES] {
        [seed; SHA256_BYTES]
    }

    fn closure(seed: u8) -> CompilerClosureV2 {
        CompilerClosureV2::new(
            digest(seed),
            digest(seed.wrapping_add(1)),
            digest(seed.wrapping_add(2)),
            digest(seed.wrapping_add(3)),
            digest(seed.wrapping_add(4)),
            digest(seed.wrapping_add(5)),
        )
        .unwrap()
    }

    fn content(seed: u8, byte_len: u64) -> InertCompilerExecutionContentBindingV1 {
        InertCompilerExecutionContentBindingV1::new(digest(seed), byte_len, "test").unwrap()
    }

    fn fields() -> SubjectFieldsV1 {
        SubjectFieldsV1 {
            attempt: BuildAttempt::new(
                7,
                BuildSession::from_bytes([0x08; 16]),
                BuildInvocation::from_bytes(digest(0x09)),
            )
            .unwrap(),
            slot: CompilerModuleHandoffSlotV3::Default,
            transaction_identity: CompilerModuleHandoffTransactionIdentityV3::from_bytes(digest(
                0x0a,
            )),
            rustc_invocation_sha256: digest(0x0b),
            compiler_closure: closure(0x10),
            rustc_identity_inventory: content(0x20, 101),
            rustc_preflight_plan: content(0x21, 102),
            semantic_capsule: content(0x22, 103),
            final_compiler_module_commitment: content(0x23, 104),
            compiler_module_handoff: content(0x24, 105),
            compiler_module_pair_binding: content(0x25, 106),
            outer_handoff: content(0x26, 107),
        }
    }

    fn subject() -> InertCompilerExecutionSubjectV1 {
        InertCompilerExecutionSubjectV1::from_fields(fields()).unwrap()
    }

    fn reseal(bytes: &mut [u8; INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1]) {
        let identity = derive_subject_identity(&bytes[..SUBJECT_PREIMAGE_BYTES]);
        bytes[SUBJECT_PREIMAGE_BYTES..].copy_from_slice(&identity);
    }

    #[test]
    fn canonical_subject_round_trips_every_field_without_authority() {
        let subject = subject();
        let decoded = InertCompilerExecutionSubjectV1::decode(subject.canonical_bytes()).unwrap();

        assert_eq!(decoded, subject);
        assert_eq!(decoded.attempt(), fields().attempt);
        assert_eq!(decoded.slot(), CompilerModuleHandoffSlotV3::Default);
        assert_eq!(
            decoded.transaction_identity(),
            fields().transaction_identity
        );
        assert_eq!(decoded.rustc_invocation_sha256(), &digest(0x0b));
        assert_eq!(decoded.compiler_closure(), closure(0x10));
        assert_eq!(decoded.rustc_identity_inventory(), content(0x20, 101));
        assert_eq!(decoded.rustc_preflight_plan(), content(0x21, 102));
        assert_eq!(decoded.semantic_capsule(), content(0x22, 103));
        assert_eq!(
            decoded.final_compiler_module_commitment(),
            content(0x23, 104)
        );
        assert_eq!(decoded.compiler_module_handoff(), content(0x24, 105));
        assert_eq!(decoded.compiler_module_pair_binding(), content(0x25, 106));
        assert_eq!(decoded.outer_handoff(), content(0x26, 107));
        assert!(
            decoded
                .identity()
                .matches_canonical_bytes(decoded.canonical_bytes())
        );
        assert!(!decoded.authenticates_compiler_execution());
        assert!(decoded.requires_protected_execution_attestation());
        assert!(!decoded.grants_compiler_authority());
        assert!(!decoded.grants_publication_authority());
        assert!(!decoded.grants_load_authority());
        assert!(!decoded.grants_launch_authority());
    }

    #[test]
    fn every_canonical_byte_is_identity_bound() {
        let original = subject();
        for index in 0..INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1 {
            let mut changed = *original.canonical_bytes();
            changed[index] ^= 1;
            assert!(
                InertCompilerExecutionSubjectV1::decode(&changed).is_err(),
                "byte {index} was not bound"
            );
        }
    }

    #[test]
    fn independently_resealed_noncanonical_axes_fail_closed() {
        let original = subject();

        let mut changed = *original.canonical_bytes();
        changed[SLOT_OFFSET] = 3;
        reseal(&mut changed);
        assert!(matches!(
            InertCompilerExecutionSubjectV1::decode(&changed),
            Err(CompilerExecutionSubjectErrorV1::InvalidSlot(3))
        ));

        let mut changed = *original.canonical_bytes();
        changed[SLOT_OFFSET + 1] = 1;
        reseal(&mut changed);
        assert!(matches!(
            InertCompilerExecutionSubjectV1::decode(&changed),
            Err(CompilerExecutionSubjectErrorV1::NonzeroReserved)
        ));

        for (offset, field) in [
            (TRANSACTION_OFFSET, "V3 handoff transaction"),
            (RUSTC_INVOCATION_OFFSET, "rustc invocation"),
            (CONTENT_BINDINGS_OFFSET, "rustc identity inventory"),
        ] {
            let mut changed = *original.canonical_bytes();
            changed[offset..offset + SHA256_BYTES].fill(0);
            reseal(&mut changed);
            assert!(matches!(
                InertCompilerExecutionSubjectV1::decode(&changed),
                Err(CompilerExecutionSubjectErrorV1::ZeroIdentity { field: actual })
                    if actual == field
            ));
        }

        let mut changed = *original.canonical_bytes();
        changed[CONTENT_BINDINGS_OFFSET + SHA256_BYTES
            ..CONTENT_BINDINGS_OFFSET + CONTENT_BINDING_BYTES]
            .fill(0);
        reseal(&mut changed);
        assert!(matches!(
            InertCompilerExecutionSubjectV1::decode(&changed),
            Err(CompilerExecutionSubjectErrorV1::ZeroLength {
                field: "rustc identity inventory"
            })
        ));

        let mut changed = *original.canonical_bytes();
        changed[CLOSURE_OFFSET..CLOSURE_OFFSET + SHA256_BYTES].fill(0);
        reseal(&mut changed);
        assert!(matches!(
            InertCompilerExecutionSubjectV1::decode(&changed),
            Err(CompilerExecutionSubjectErrorV1::CompilerClosure(_))
        ));

        let mut changed = *original.canonical_bytes();
        changed[SESSION_OFFSET..SESSION_OFFSET + 16].fill(0);
        reseal(&mut changed);
        assert!(matches!(
            InertCompilerExecutionSubjectV1::decode(&changed),
            Err(CompilerExecutionSubjectErrorV1::Attempt(_))
        ));
    }

    #[test]
    fn every_semantic_axis_changes_the_subject_identity() {
        let original = subject().identity();
        let assert_changed = |fields| {
            assert_ne!(
                InertCompilerExecutionSubjectV1::from_fields(fields)
                    .unwrap()
                    .identity(),
                original
            );
        };

        let mut changed = fields();
        changed.attempt =
            BuildAttempt::new(8, changed.attempt.session(), changed.attempt.invocation()).unwrap();
        assert_changed(changed);

        let mut changed = fields();
        changed.slot = CompilerModuleHandoffSlotV3::GeneralGemmReference;
        assert_changed(changed);

        let mut changed = fields();
        changed.transaction_identity =
            CompilerModuleHandoffTransactionIdentityV3::from_bytes(digest(0x30));
        assert_changed(changed);

        let mut changed = fields();
        changed.rustc_invocation_sha256 = digest(0x31);
        assert_changed(changed);

        let mut changed = fields();
        changed.compiler_closure = closure(0x32);
        assert_changed(changed);

        let mut changes: [fn(&mut SubjectFieldsV1); CONTENT_BINDING_COUNT] = [
            |value| value.rustc_identity_inventory = content(0x40, 201),
            |value| value.rustc_preflight_plan = content(0x41, 202),
            |value| value.semantic_capsule = content(0x42, 203),
            |value| value.final_compiler_module_commitment = content(0x43, 204),
            |value| value.compiler_module_handoff = content(0x44, 205),
            |value| value.compiler_module_pair_binding = content(0x45, 206),
            |value| value.outer_handoff = content(0x46, 207),
        ];
        for change in &mut changes {
            let mut changed = fields();
            change(&mut changed);
            assert_changed(changed);
        }
    }

    #[test]
    fn malformed_header_and_lengths_are_rejected_before_content_use() {
        let original = subject();
        assert!(matches!(
            InertCompilerExecutionSubjectV1::decode(
                &original.canonical_bytes()[..INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1 - 1]
            ),
            Err(CompilerExecutionSubjectErrorV1::InvalidLength { .. })
        ));

        let mut changed = *original.canonical_bytes();
        changed[0] ^= 1;
        reseal(&mut changed);
        assert!(matches!(
            InertCompilerExecutionSubjectV1::decode(&changed),
            Err(CompilerExecutionSubjectErrorV1::InvalidMagic)
        ));

        let mut changed = *original.canonical_bytes();
        changed[10] = 1;
        reseal(&mut changed);
        assert!(matches!(
            InertCompilerExecutionSubjectV1::decode(&changed),
            Err(CompilerExecutionSubjectErrorV1::UnsupportedFlags(1))
        ));

        let mut changed = *original.canonical_bytes();
        changed[12..20].copy_from_slice(&0_u64.to_le_bytes());
        reseal(&mut changed);
        assert!(matches!(
            InertCompilerExecutionSubjectV1::decode(&changed),
            Err(CompilerExecutionSubjectErrorV1::InvalidDeclaredLength(0))
        ));

        let mut changed = *original.canonical_bytes();
        changed[20] = 1;
        reseal(&mut changed);
        assert!(matches!(
            InertCompilerExecutionSubjectV1::decode(&changed),
            Err(CompilerExecutionSubjectErrorV1::NonzeroReserved)
        ));

        let mut changed = *original.canonical_bytes();
        changed[GENERATION_OFFSET..GENERATION_OFFSET + 8].fill(0);
        reseal(&mut changed);
        assert!(matches!(
            InertCompilerExecutionSubjectV1::decode(&changed),
            Err(CompilerExecutionSubjectErrorV1::Attempt(_))
        ));
    }
}
