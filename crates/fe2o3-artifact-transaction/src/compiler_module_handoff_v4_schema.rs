//! Native V4 schema, strictly isolated from legacy transaction domains.
use super::*;
use fe2o3_compiler_ffi::{
    INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DECODE_METADATA_STORAGE_V4 as METADATA,
    inert_semantic_compiler_module_handoff_decode_work_v4,
};

pub(in crate::compiler_module_handoff) struct Schema;
const MAGIC: &[u8] = b"FE2O3-COMPILER-MODULE-HANDOFF-V4\0";
pub(super) const RECORD_BYTES: usize =
    MAGIC.len() + 2 + 32 + 8 + 16 + 32 + 32 + 40 + 32 + 8 + 7 * 8 + 32;

impl From<Identity> for currentness::Binding {
    fn from(identity: Identity) -> Self {
        Self {
            sha256: *identity.sha256(),
            byte_len: identity.byte_len(),
        }
    }
}

impl currentness::Schema for Schema {
    type Receipt = CompilerModuleHandoffReceiptV4;
    const TRANSACTION_DOMAIN: &'static [u8] =
        b"fe2o3.compiler-module-handoff.transaction-identity.v4\0";

    fn receipt_fields(receipt: Self::Receipt) -> PublishedHandoff<Self> {
        PublishedHandoff {
            attempt: receipt.attempt,
            slot: receipt.slot,
            binding: receipt.handoff_identity.into(),
            identity: receipt.transaction_identity.0,
            length: receipt.length,
        }
    }

    fn receipt(fields: PublishedHandoff<Self>, payload: &Handoff) -> Self::Receipt {
        CompilerModuleHandoffReceiptV4 {
            attempt: fields.attempt,
            slot: fields.slot,
            handoff_identity: payload.identity(),
            transaction_identity: CompilerModuleHandoffTransactionIdentityV4(fields.identity),
            length: fields.length,
        }
    }

    fn payload_binding(payload: &Handoff) -> currentness::Binding {
        payload.identity().into()
    }
}

impl HandoffSchema for Schema {
    type Slot = CompilerModuleHandoffSlotV4;
    type Binding = currentness::Binding;
    type Payload = Handoff;
    const METERED: bool = true;
    const PARENT_PREFIX: &'static str = ".fe2o3-compiler-module-handoff-v4-";
    const SLOT_PREFIX: &'static str = "attempt-";
    const RECORD_MAGIC: &'static [u8] = MAGIC;
    const RECORD_VERSION: u16 = 4;
    const PRODUCER_DOMAIN: &'static [u8] = b"fe2o3.compiler-module-handoff.producer.v4\0";
    const SLOT_DOMAIN: &'static [u8] = b"fe2o3.compiler-module-handoff.slot.v4\0";
    const NAMED_SLOT_DOMAIN: &'static [u8] = b"fe2o3.compiler-module-handoff.named-slot.v4\0";
    const RECORD_DOMAIN: &'static [u8] = b"fe2o3.compiler-module-handoff.record.v4\0";
    const RECORD_BYTES: usize = RECORD_BYTES;
    const MAX_HANDOFF_BYTES: usize = MAX_COMPILER_MODULE_HANDOFF_BYTES_V4;
    const DECODE_WORKING_SET_MULTIPLIER: usize = 1;
    const DECODE_WORKING_SET_FIXED_BYTES: usize = METADATA;
    const MAX_DECODE_WORKING_SET_BYTES: usize = MAX_COMPILER_MODULE_HANDOFF_BYTES_V4 + METADATA;
    const VALIDATE_RECORD_DURING_RECOVERY: bool = true;
    const COMMITTED_SIDECAR_ENTRY: Option<&'static str> = Some(receipt_transport_v2::ENTRY);
    const ALL_SLOTS: &'static [Self::Slot] = &[CompilerModuleHandoffSlotV4::Production];

    fn default_slot() -> Self::Slot {
        CompilerModuleHandoffSlotV4::Production
    }
    fn slot_tag(slot: Self::Slot) -> u8 {
        slot as u8
    }
    fn encode_binding(binding: Self::Binding, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&binding.sha256);
        bytes.extend_from_slice(&binding.byte_len.to_le_bytes());
    }
    fn decode_binding(
        decoder: &mut Decoder<'_>,
    ) -> std::result::Result<Self::Binding, &'static str> {
        let sha256 = decoder.array()?;
        let byte_len = decoder.u64()?;
        if sha256 == [0; 32] {
            return Err("native V4 handoff identity is zero");
        }
        Ok(currentness::Binding { sha256, byte_len })
    }
    fn binding_matches_length(binding: Self::Binding, length: usize) -> bool {
        usize::try_from(binding.byte_len).ok() == Some(length) && length <= Self::MAX_HANDOFF_BYTES
    }
    fn decode_payload(
        binding: Self::Binding,
        bytes: Vec<u8>,
        resources: &mut Resources<'_, '_>,
    ) -> std::result::Result<Handoff, HandoffEngineError> {
        resources.require::<Self>()?;
        if resources.storage() < bytes.capacity() {
            return Err(Resource::Accounting.into());
        }
        let work = inert_semantic_compiler_module_handoff_decode_work_v4(bytes.len())
            .map_err(HandoffEngineError::InvalidCanonicalV4)?;
        resources.reserve(METADATA)?;
        resources.work(work)?;
        let handoff =
            Handoff::decode_owned(bytes).map_err(HandoffEngineError::InvalidCanonicalV4)?;
        if currentness::Binding::from(handoff.identity()) != binding {
            return Err(HandoffEngineError::PayloadBindingMismatch);
        }
        Ok(handoff)
    }
    fn derive_identity(
        producer: [u8; 32],
        slot: [u8; 32],
        attempt: BuildAttempt,
        binding: Self::Binding,
        bytes: &[u8],
    ) -> [u8; 32] {
        let mut digest = <Self as currentness::Schema>::transaction_hasher(
            producer,
            slot,
            attempt,
            binding,
            bytes.len(),
        );
        digest.update(bytes);
        digest.finalize().into()
    }
}

pub(super) fn payload_storage(handoff: &Handoff) -> std::result::Result<usize, HandoffEngineError> {
    handoff
        .backing_capacity()
        .checked_add(METADATA)
        .ok_or(Resource::Arithmetic.into())
}

/// V4 failures never dispatch to a legacy decoder or authority path.
#[derive(Debug)]
pub enum CompilerModuleHandoffErrorV4 {
    Coordination(CompilerModuleHandoffErrorV1),
    Busy,
    WrongHandoffIdentity,
    HandoffIdentityMismatch,
    NonCanonicalHandoff(fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffErrorV4),
    Resource(Resource),
    WorkingSetBudgetExceeded { required: usize, maximum: usize },
    PayloadAllocationFailed { requested: usize },
    MismatchedCurrentnessToken,
}

impl fmt::Display for CompilerModuleHandoffErrorV4 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Coordination(error) => error.fmt(f),
            Self::Resource(error) => error.fmt(f),
            other => write!(f, "native V4 handoff: {other:?}"),
        }
    }
}
impl std::error::Error for CompilerModuleHandoffErrorV4 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Coordination(error) => Some(error),
            Self::Resource(error) => Some(error),
            _ => None,
        }
    }
}
impl From<HandoffEngineError> for CompilerModuleHandoffErrorV4 {
    fn from(error: HandoffEngineError) -> Self {
        match error {
            HandoffEngineError::Common(error) => Self::Coordination(error),
            HandoffEngineError::WrongBinding => Self::WrongHandoffIdentity,
            HandoffEngineError::PayloadBindingMismatch => Self::HandoffIdentityMismatch,
            HandoffEngineError::InvalidCanonicalV4(error) => Self::NonCanonicalHandoff(error),
            HandoffEngineError::Resource(error) => Self::Resource(error),
            HandoffEngineError::Busy => Self::Busy,
            HandoffEngineError::WorkingSetBudgetExceeded { required, maximum } => {
                Self::WorkingSetBudgetExceeded { required, maximum }
            }
            HandoffEngineError::PayloadAllocationFailed { requested } => {
                Self::PayloadAllocationFailed { requested }
            }
            HandoffEngineError::InvalidCanonicalV3(_) => Self::Resource(Resource::Accounting),
        }
    }
}
