//! Native execution subject: exact content binding, not execution authentication.
use super::{CompilerExecutionSubjectErrorV1, InertCompilerExecutionContentBindingV1, codec};
type Binding = InertCompilerExecutionContentBindingV1;

#[cfg(test)]
#[path = "compiler_execution_subject_v2_tests.rs"]
mod tests;
use crate::{
    BuildAttempt, CompilerModuleHandoffReceiptV4, CompilerModuleHandoffSlotV4,
    CompilerModuleHandoffTransactionIdentityV4, ConsumedCompilerModuleHandoffV4,
    MAX_COMPILER_MODULE_HANDOFF_STORAGE_V4,
};
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_ffi::{
    INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DECODE_METADATA_STORAGE_V4 as METADATA,
    InertSemanticCompilerModuleHandoffV4 as Handoff,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{fmt, mem::size_of};

pub const INERT_COMPILER_EXECUTION_SUBJECT_MAGIC_V2: [u8; 8] = *b"F2O3CES2";
pub const INERT_COMPILER_EXECUTION_SUBJECT_VERSION_V2: u16 = 2;
pub const INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V2: usize = codec::BYTES;
const SCHEMA: codec::Schema = codec::Schema {
    magic: INERT_COMPILER_EXECUTION_SUBJECT_MAGIC_V2,
    version: INERT_COMPILER_EXECUTION_SUBJECT_VERSION_V2,
    identity_domain: b"FE2O3/INERT-COMPILER-EXECUTION-SUBJECT/V2\0",
    transaction_label: "V4 handoff transaction",
    outer_label: "outer V4 handoff",
};
const SHA256_BYTES: usize = 32;
pub(crate) const RETAINED: usize = size_of::<InertCompilerExecutionSubjectV2>()
    + size_of::<InertCompilerExecutionSubjectStorageV2>();
/// Conservative fixed logical work for cached-field extraction, closure validation,
/// two subject hashes, reconstruction and comparison, including entry work.
/// No payload/invocation serialization is performed. This is not instruction count.
pub const INERT_COMPILER_EXECUTION_SUBJECT_WORK_V2: usize = 32 * codec::BYTES + 4096 + 8;
/// Fixed peak additional logical storage, including retained result/receipt,
/// staging, hash state and scalar/error scratch; not an allocator/RSS bound.
pub const INERT_COMPILER_EXECUTION_SUBJECT_STORAGE_V2: usize = 4 * RETAINED
    + 4 * size_of::<codec::Fields>()
    + 4 * codec::BYTES
    + 2 * size_of::<sha2::Sha256>()
    + 4096;
const _: () = assert!(codec::BYTES == 690);

const fn result_overhead<P>() -> usize {
    4 * (size_of::<Result<P>>() - size_of::<P>())
        + crate::compiler_module_handoff::resources::fixed_scope_overhead::<Result<P>>()
}
const SCALAR_STORAGE: usize = codec::SCALAR_STORAGE
    + 2 * size_of::<CompilerModuleHandoffReceiptV4>()
    + 64 * size_of::<usize>();
// Guard layout growth without counting full result payloads twice. The fixed
// word allowance covers offsets, references and captures, not generated stack.
const _: () = {
    type Output = (
        InertCompilerExecutionSubjectV2,
        InertCompilerExecutionSubjectStorageV2,
    );
    assert!(size_of::<Output>() <= RETAINED);
    assert!(SCALAR_STORAGE + result_overhead::<Output>() <= 4096);
    assert!(SCALAR_STORAGE + result_overhead::<bool>() <= 4096);
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InertCompilerExecutionSubjectIdentityV2 {
    sha256: [u8; 32],
}
impl InertCompilerExecutionSubjectIdentityV2 {
    pub const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
    pub const fn byte_len(self) -> u64 {
        codec::BYTES as u64
    }
    /// Checks fixed bytes, not their relationship to a payload or execution.
    /// Keep the complete borrowed input owner paid on the same ledger.
    pub fn matches_canonical_bytes(self, bytes: &[u8], budget: &mut Budget<'_>) -> Result<bool> {
        metered(budget, input_floor(bytes), || {
            Ok(bytes.len() == codec::BYTES
                && bytes[codec::SUBJECT_PREIMAGE_BYTES..] == self.sha256
                && SCHEMA.identity(&bytes[..codec::SUBJECT_PREIMAGE_BYTES]) == self.sha256)
        })
    }
}

/// Additional fixed retained subject/receipt storage, returned unreserved.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InertCompilerExecutionSubjectStorageV2(usize);
impl InertCompilerExecutionSubjectStorageV2 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Binds a complete native V4 capsule, including its mandatory source/F carrier.
/// Decoding validates canonical framing only; typed reconstruction is required
/// to establish agreement with an actual immutable handoff. Neither authenticates
/// protected compiler execution or grants publication/load/launch authority.
///
/// Keep input owners prepaid, then reserve the returned additional storage
/// before retaining/using a constructed or decoded subject.
///
/// The owner is move-only: another instance requires a metered construction.
/// ```compile_fail
/// use fe2o3_artifact_transaction::InertCompilerExecutionSubjectV2;
/// fn duplicate(subject: InertCompilerExecutionSubjectV2) {
///     let _ = subject.clone();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_artifact_transaction::{CompilerModuleHandoffReceiptV3, InertCompilerExecutionSubjectV2};
/// use fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV4;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// fn mix(r: CompilerModuleHandoffReceiptV3, h: &InertSemanticCompilerModuleHandoffV4,
///        b: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let _ = InertCompilerExecutionSubjectV2::from_publication(r, h, b);
/// }
/// ```
#[derive(Eq, PartialEq)]
pub struct InertCompilerExecutionSubjectV2 {
    fields: codec::Fields,
    encoded: codec::Encoded,
}

impl InertCompilerExecutionSubjectV2 {
    /// Uses only the exact receipt and cached fields of a strictly decoded owner.
    pub fn from_publication(
        receipt: CompilerModuleHandoffReceiptV4,
        handoff: &Handoff,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, InertCompilerExecutionSubjectStorageV2)> {
        subject(budget, handoff_floor(handoff)?, || {
            Self::from_receipt(receipt, handoff)
        })
    }

    /// Raw consumed transport convenience. For a concrete verified owner use
    /// from_publication(consumed.receipt(), owner.handoff(), budget). An arbitrary
    /// user-provided AsRef implementation is outside this fixed-cost codec.
    pub fn from_consumed(
        consumed: &ConsumedCompilerModuleHandoffV4,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, InertCompilerExecutionSubjectStorageV2)> {
        subject(budget, consumed.storage().retained_storage(), || {
            Self::from_receipt(consumed.receipt(), consumed.handoff())
        })
    }

    fn from_receipt(receipt: CompilerModuleHandoffReceiptV4, handoff: &Handoff) -> Result<Self> {
        if receipt.handoff_identity() != handoff.identity() {
            return Err(Failure::HandoffIdentityMismatch);
        }
        if receipt.length() != handoff.canonical_bytes().len() {
            return Err(Failure::HandoffLengthMismatch);
        }
        Self::from_exact(
            receipt.attempt(),
            receipt.slot(),
            receipt.transaction_identity(),
            handoff,
        )
    }

    /// Supplied occurrence axes are bound, not authenticated as a real execution.
    pub fn from_replay_evidence(
        attempt: BuildAttempt,
        slot: CompilerModuleHandoffSlotV4,
        transaction: CompilerModuleHandoffTransactionIdentityV4,
        handoff: &Handoff,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, InertCompilerExecutionSubjectStorageV2)> {
        subject(budget, handoff_floor(handoff)?, || {
            Self::from_exact(attempt, slot, transaction, handoff)
        })
    }

    fn from_exact(
        attempt: BuildAttempt,
        slot: CompilerModuleHandoffSlotV4,
        transaction: CompilerModuleHandoffTransactionIdentityV4,
        handoff: &Handoff,
    ) -> Result<Self> {
        let capsule = handoff.capsule();
        let base = capsule.base();
        let receipts = base.receipts();
        let inventory = receipts.rustc_identity_inventory().identity();
        let preflight = receipts.rustc_preflight_plan().identity();
        let commitment = receipts.final_compiler_module_commitment().identity();
        let capsule = capsule.identity();
        let module = handoff.module_handoff().identity();
        let pair = handoff.pair_binding_identity();
        let outer = handoff.identity();
        Self::from_fields(codec::Fields {
            attempt,
            slot: slot as u8,
            transaction_identity: *transaction.as_bytes(),
            rustc_invocation_sha256: base.invocation_digest().into_bytes(),
            compiler_closure: *base.compiler_closure(),
            rustc_identity_inventory: Binding::new(
                *inventory.sha256(),
                inventory.byte_len(),
                "rustc identity inventory",
            )?,
            rustc_preflight_plan: Binding::new(
                *preflight.sha256(),
                preflight.byte_len(),
                "rustc preflight plan",
            )?,
            semantic_capsule: Binding::new(
                *capsule.sha256(),
                capsule.byte_len(),
                "semantic capsule",
            )?,
            final_compiler_module_commitment: Binding::new(
                *commitment.sha256(),
                commitment.byte_len(),
                "final compiler module commitment",
            )?,
            compiler_module_handoff: Binding::new(
                *module.sha256(),
                module.byte_len(),
                "compiler module handoff",
            )?,
            compiler_module_pair_binding: Binding::new(
                *pair.sha256(),
                pair.byte_len(),
                "compiler module pair binding",
            )?,
            outer_handoff: Binding::new(*outer.sha256(), outer.byte_len(), "outer V4 handoff")?,
        })
    }

    fn from_fields(fields: codec::Fields) -> Result<Self> {
        let encoded = SCHEMA.encode(&fields)?;
        Ok(Self { fields, encoded })
    }

    /// Borrows a fixed wire record. Embedded content lengths never allocate.
    /// Caller-owned input capacity remains paid separately; invalid lengths are
    /// rejected after fixed entry prepayment, without traversing the input.
    pub fn decode(
        bytes: &[u8],
        budget: &mut Budget<'_>,
    ) -> Result<(Self, InertCompilerExecutionSubjectStorageV2)> {
        subject(budget, input_floor(bytes), || {
            let (fields, encoded) = SCHEMA.decode(bytes)?;
            Ok(Self { fields, encoded })
        })
    }

    /// Returns the exact durable build attempt.
    pub const fn attempt(&self) -> BuildAttempt {
        self.fields.attempt
    }

    /// Returns the exact V4 transaction slot.
    pub const fn slot(&self) -> CompilerModuleHandoffSlotV4 {
        CompilerModuleHandoffSlotV4::Production
    }

    /// Returns the exact V4 transaction identity.
    pub const fn transaction_identity(&self) -> CompilerModuleHandoffTransactionIdentityV4 {
        CompilerModuleHandoffTransactionIdentityV4::from_bytes(self.fields.transaction_identity)
    }

    /// Returns the V3 canonical rustc invocation digest.
    pub const fn rustc_invocation_sha256(&self) -> &[u8; SHA256_BYTES] {
        &self.fields.rustc_invocation_sha256
    }

    /// Returns the complete six-pin compiler closure.
    pub const fn compiler_closure(&self) -> CompilerClosureV2 {
        self.fields.compiler_closure
    }

    /// Returns the rustc identity-inventory binding.
    pub const fn rustc_identity_inventory(&self) -> InertCompilerExecutionContentBindingV1 {
        self.fields.rustc_identity_inventory
    }

    /// Returns the rustc preflight-plan binding.
    pub const fn rustc_preflight_plan(&self) -> InertCompilerExecutionContentBindingV1 {
        self.fields.rustc_preflight_plan
    }

    /// Returns the complete semantic-capsule binding.
    pub const fn semantic_capsule(&self) -> InertCompilerExecutionContentBindingV1 {
        self.fields.semantic_capsule
    }

    /// Returns the compact final compiler-module commitment binding.
    pub const fn final_compiler_module_commitment(&self) -> InertCompilerExecutionContentBindingV1 {
        self.fields.final_compiler_module_commitment
    }

    /// Returns the native V2 compiler-module handoff binding.
    pub const fn compiler_module_handoff(&self) -> InertCompilerExecutionContentBindingV1 {
        self.fields.compiler_module_handoff
    }

    /// Returns the V4 compiler-module pair-binding identity.
    pub const fn compiler_module_pair_binding(&self) -> InertCompilerExecutionContentBindingV1 {
        self.fields.compiler_module_pair_binding
    }

    /// Returns the exact outer V4 handoff binding.
    pub const fn outer_handoff(&self) -> InertCompilerExecutionContentBindingV1 {
        self.fields.outer_handoff
    }

    /// Returns the domain-separated identity of the complete subject.
    pub const fn identity(&self) -> InertCompilerExecutionSubjectIdentityV2 {
        InertCompilerExecutionSubjectIdentityV2 {
            sha256: self.encoded.sha256,
        }
    }

    /// Returns the exact canonical subject bytes.
    pub const fn canonical_bytes(&self) -> &[u8; INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V2] {
        &self.encoded.canonical_bytes
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
impl fmt::Debug for InertCompilerExecutionSubjectV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InertCompilerExecutionSubjectV2")
            .field("attempt", &self.attempt())
            .field("identity", &self.identity())
            .finish_non_exhaustive()
    }
}

/// Shared framing diagnostics never invoke a V1 decoder or legacy admission.
#[derive(Debug)]
pub enum CompilerExecutionSubjectErrorV2 {
    Framing(CompilerExecutionSubjectErrorV1),
    HandoffIdentityMismatch,
    HandoffLengthMismatch,
    Resource(Resource),
}
type Failure = CompilerExecutionSubjectErrorV2;
type Result<T> = std::result::Result<T, Failure>;
impl From<CompilerExecutionSubjectErrorV1> for Failure {
    fn from(e: CompilerExecutionSubjectErrorV1) -> Self {
        Self::Framing(e)
    }
}
impl From<Resource> for Failure {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Framing(e) => e.fmt(f),
            Self::Resource(e) => e.fmt(f),
            Self::HandoffIdentityMismatch => {
                f.write_str("publication binding and native V4 identities differ")
            }
            Self::HandoffLengthMismatch => {
                f.write_str("publication binding and native V4 lengths differ")
            }
        }
    }
}
impl std::error::Error for Failure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Framing(e) => Some(e),
            Self::Resource(e) => Some(e),
            _ => None,
        }
    }
}

fn handoff_floor(handoff: &Handoff) -> Result<usize> {
    handoff
        .backing_capacity()
        .checked_add(METADATA)
        .ok_or(Resource::Arithmetic.into())
}
fn input_floor(bytes: &[u8]) -> usize {
    if bytes.len() == codec::BYTES {
        codec::BYTES
    } else {
        0
    }
}
fn subject(
    budget: &mut Budget<'_>,
    floor: usize,
    f: impl FnOnce() -> Result<InertCompilerExecutionSubjectV2>,
) -> Result<(
    InertCompilerExecutionSubjectV2,
    InertCompilerExecutionSubjectStorageV2,
)> {
    metered(budget, floor, || {
        Ok((f()?, InertCompilerExecutionSubjectStorageV2(RETAINED)))
    })
}
fn metered<T>(budget: &mut Budget<'_>, floor: usize, f: impl FnOnce() -> Result<T>) -> Result<T> {
    crate::compiler_module_handoff::resources::with_budget(budget, |budget| {
        budget.charge_work(8)?;
        if budget.storage_limit() > MAX_COMPILER_MODULE_HANDOFF_STORAGE_V4
            || budget.storage() < floor
        {
            return Err(Resource::Accounting.into());
        }
        budget.reserve_storage(INERT_COMPILER_EXECUTION_SUBJECT_STORAGE_V2)?;
        budget.charge_work(INERT_COMPILER_EXECUTION_SUBJECT_WORK_V2 - 8)?;
        f()
    })?
}
