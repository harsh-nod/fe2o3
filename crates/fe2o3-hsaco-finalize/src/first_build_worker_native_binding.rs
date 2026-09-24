//! Inert Worker binding to the complete recovered native V4 occurrence.

use std::{error::Error, fmt, mem::size_of};

use fe2o3_amd_target::{AmdTargetId, ProductionAmdTargetProfileV1};
use fe2o3_artifact_transaction::{CompilerModuleHandoffReceiptV4, ConsumedCompilerModuleHandoffV4};
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_ffi::{
    CodeObjectVersion, CompilerModuleHandoffIdentityV2,
    INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DECODE_METADATA_STORAGE_V4,
    InertCompilerModulePairBindingIdentityV4, InertRefinedForwardingContentIdentityV1,
    MAX_INERT_REFINED_FORWARDING_STORAGE_V1,
};
use fe2o3_compiler_lineage::{
    InertNativeLoweringAssociationV1, InertNativeNeutralSubjectV1,
    InertProductionSemanticCapsuleIdentityV4, NATIVE_LOWERING_ASSOCIATION_WORK_V1,
    NativeLoweringAssociationErrorV1, NativeRefinedForwardingCarrierIdentityV1,
    TargetLineageIdentityV3,
};
use fe2o3_kernel_descriptor::MAX_DESCRIPTOR_TABLE_BYTES;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, VerifiedCanonicalKernelIrIdentityV12,
};
use fe2o3_verifier::RecoveredCompilerNativeSemanticHandoffV4;
use sha2::{Digest, Sha256};

const BINDING_DOMAIN: &[u8] = b"FE2O3/PROTECTED-WORKER-COMPILER-NATIVE-HANDOFF-BINDING/V1\0";

// Prepay the codec's fixed staging/hash schedule here. The additional 8 KiB
// covers fixed identity/closure comparisons, copies, target parsing, and the
// sub-1-KiB binding preimage. Only descriptor hashing traverses variable bytes;
// it is charged separately before hashing. No graph or transport is copied.
const FIXED_WORK: usize = NATIVE_LOWERING_ASSOCIATION_WORK_V1 + 8 * 1024;
const SCRATCH: usize = 3 * size_of::<ProtectedCompilerNativeHandoffBindingV1>()
    + 3 * size_of::<InertNativeLoweringAssociationV1>()
    + 2 * size_of::<Sha256>()
    + 1024;

/// Domain-separated identity of one complete native Worker binding.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProtectedCompilerNativeHandoffBindingIdentityV1([u8; 32]);

impl ProtectedCompilerNativeHandoffBindingIdentityV1 {
    /// Returns the exact binding digest.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Copyable, inert coordinates derived only from an immutable recovered owner.
///
/// The receipt names the complete V4 transport, including its native carrier.
/// The actual F identity comes from the retained verified graph. These values
/// do not replace that owner, authenticate compiler origin, establish currentness,
/// or grant compiler, link, publication, load, or launch authority.
///
/// ```compile_fail
/// use fe2o3_hsaco_finalize::ProtectedCompilerNativeHandoffBindingV1 as Binding;
/// fn manufacture() -> Binding { Binding::default() }
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedCompilerNativeHandoffBindingV1 {
    receipt: CompilerModuleHandoffReceiptV4,
    compiler_closure: CompilerClosureV2,
    capsule_identity: InertProductionSemanticCapsuleIdentityV4,
    carrier_identity: NativeRefinedForwardingCarrierIdentityV1,
    pair_binding_identity: InertCompilerModulePairBindingIdentityV4,
    invocation_digest: [u8; 32],
    module_handoff_identity: CompilerModuleHandoffIdentityV2,
    actual_f_identity: VerifiedCanonicalKernelIrIdentityV12,
    output_identity: InertRefinedForwardingContentIdentityV1,
    history_identity: InertRefinedForwardingContentIdentityV1,
    descriptor_identity: TargetLineageIdentityV3,
    profile: ProductionAmdTargetProfileV1,
    identity: ProtectedCompilerNativeHandoffBindingIdentityV1,
}

impl ProtectedCompilerNativeHandoffBindingV1 {
    /// Rechecks the exact receipt of the consumed occurrence before deriving
    /// coordinates from its still-retained recovered native owner.
    pub(crate) fn from_consumed(
        consumed: &ConsumedCompilerModuleHandoffV4<RecoveredCompilerNativeSemanticHandoffV4>,
        expected_receipt: CompilerModuleHandoffReceiptV4,
        expected_compiler_closure: CompilerClosureV2,
        budget: &mut Budget<'_>,
    ) -> Result<Self, ProtectedCompilerNativeHandoffBindingErrorV1> {
        budget.with_prepaid_scope(
            consumed.storage().retained_storage(),
            8,
            256,
            size_of::<Self>() + 2 * size_of::<CompilerModuleHandoffReceiptV4>(),
            |budget| {
                // Equality includes attempt, slot, transaction, outer identity
                // and receipt length. The transaction crate owns its derivation.
                if consumed.receipt() != expected_receipt {
                    return Err(mismatch("consumed V4 receipt"));
                }
                Self::from_handoff(
                    consumed.content(),
                    expected_receipt,
                    expected_compiler_closure,
                    budget,
                )
            },
        )
    }

    /// Derives coordinates without copying or re-admitting the source/F graph.
    ///
    /// The caller keeps the complete backing, decoded metadata and recovered
    /// owner's returned storage reserved on this ledger. All temporary storage
    /// is restored on return or unwind; work, peaks and denials are preserved.
    /// The returned fixed-size binding is unreserved, so its retaining caller
    /// must include it in that caller's owner/header reservation.
    pub(crate) fn from_handoff(
        owner: &RecoveredCompilerNativeSemanticHandoffV4,
        expected_receipt: CompilerModuleHandoffReceiptV4,
        expected_compiler_closure: CompilerClosureV2,
        budget: &mut Budget<'_>,
    ) -> Result<Self, ProtectedCompilerNativeHandoffBindingErrorV1> {
        budget.charge_work(8)?;
        let handoff = owner.handoff();
        let floor = native_handoff_storage_floor(owner)?;
        budget.with_prepaid_scope(floor, 0, FIXED_WORK, SCRATCH, |budget| {
            if budget.storage_limit() > MAX_INERT_REFINED_FORWARDING_STORAGE_V1 {
                return Err(mismatch("bounded native storage cap"));
            }
            let byte_len =
                u64::try_from(handoff.canonical_bytes().len()).map_err(|_| Resource::Arithmetic)?;
            // V4's private-construction identity commits all immutable backing
            // bytes. Its decoder already checked the V4 pair, target and final
            // commitment; recovery joined the same transport to source and F.
            if handoff.identity() != expected_receipt.handoff_identity()
                || handoff.identity().byte_len() != byte_len
                || expected_receipt.length() != handoff.canonical_bytes().len()
            {
                return Err(mismatch("complete outer V4 handoff identity"));
            }

            let capsule = handoff.capsule();
            let base = capsule.base();
            let module = handoff.module_handoff();
            check_compiler_closure(
                *base.compiler_closure(),
                *base.invocation().compiler_closure(),
                expected_compiler_closure,
            )?;
            if base.target() != module.target() {
                return Err(mismatch("capsule and module target"));
            }

            let lowering = InertNativeLoweringAssociationV1::decode(
                base.receipts().amdgpu_lowering().canonical_preimage(),
            )?;
            let inputs = lowering.inputs();
            let recovered = owner.recovered();
            let actual_f = recovered.output().canonical().identity();
            check_actual_f(actual_f, &inputs.final_native)?;
            let carrier = capsule.carrier_identity();
            if inputs.carrier.sha256() != *carrier.sha256()
                || inputs.carrier.byte_len() != carrier.byte_len()
                || u64::try_from(capsule.carrier_bytes().len()).ok() != Some(carrier.byte_len())
            {
                return Err(mismatch("exact native source/F carrier"));
            }
            if inputs.module_handoff.sha256() != *module.identity().sha256()
                || inputs.module_handoff.byte_len() != module.identity().byte_len()
                || inputs.final_llvm.sha256() != *module.module_identity().sha256()
                || inputs.final_llvm.byte_len() != module.module_identity().byte_len()
            {
                return Err(mismatch("native lowering and retained module"));
            }
            if ProductionAmdTargetProfileV1::from_device_target(base.invocation().amd_target())
                != Some(inputs.profile)
                || AmdTargetId::parse(inputs.profile.device_target()).ok()
                    != Some(module.target().as_amd_target_id())
                || module.code_object_version() != CodeObjectVersion::V6
            {
                return Err(mismatch("native lowering target profile"));
            }
            // Recovery already checked the ABI bytes against the descriptor,
            // actual-F capability projection and native text relation. Here the
            // descriptor coordinate is raw SHA256, not the ABI receipt digest.
            check_descriptor(
                base.receipts().abi().canonical_preimage(),
                inputs.descriptor,
                budget,
            )?;

            let mut binding = Self {
                receipt: expected_receipt,
                compiler_closure: expected_compiler_closure,
                capsule_identity: capsule.identity(),
                carrier_identity: carrier,
                pair_binding_identity: handoff.pair_binding_identity(),
                invocation_digest: *base.invocation_digest().as_bytes(),
                module_handoff_identity: module.identity(),
                actual_f_identity: *actual_f,
                output_identity: recovered.identity(),
                history_identity: recovered.history_identity(),
                descriptor_identity: inputs.descriptor,
                profile: inputs.profile,
                identity: ProtectedCompilerNativeHandoffBindingIdentityV1([0; 32]),
            };
            let mut hasher = Sha256::new();
            binding.hash_identity_preimage(&mut hasher);
            binding.identity =
                ProtectedCompilerNativeHandoffBindingIdentityV1(hasher.finalize().into());
            Ok(binding)
        })
    }

    /// Returns the exact complete V4 occurrence receipt.
    pub const fn receipt(&self) -> CompilerModuleHandoffReceiptV4 {
        self.receipt
    }

    /// Returns the closure checked against the retained invocation.
    pub const fn compiler_closure(&self) -> CompilerClosureV2 {
        self.compiler_closure
    }

    /// Returns the identity used to compare preflight and consumed bindings.
    pub const fn identity(&self) -> ProtectedCompilerNativeHandoffBindingIdentityV1 {
        self.identity
    }

    /// Returns the actual recovered F identity, not an original-N receipt.
    pub const fn actual_f_identity(&self) -> VerifiedCanonicalKernelIrIdentityV12 {
        self.actual_f_identity
    }

    /// Returns the mandatory paired source/output carrier identity.
    pub const fn carrier_identity(&self) -> NativeRefinedForwardingCarrierIdentityV1 {
        self.carrier_identity
    }

    /// Appends the domain and fixed-size canonical binding preimage. The common
    /// request engine must prepay this work in its own request-hashing schedule.
    /// No retained variable-length bytes are traversed by this method.
    pub(crate) fn hash_identity_preimage(&self, hasher: &mut Sha256) {
        hasher.update(BINDING_DOMAIN);
        let attempt = self.receipt.attempt();
        hasher.update(attempt.generation().to_le_bytes());
        hasher.update(attempt.session().as_bytes());
        hasher.update(attempt.invocation().as_bytes());
        hasher.update([self.receipt.slot() as u8]);
        hasher.update(self.receipt.transaction_identity().as_bytes());
        // Receipt length was checked equal to the complete outer byte length.
        hash_coordinate(
            hasher,
            self.receipt.handoff_identity().sha256(),
            self.receipt.handoff_identity().byte_len(),
        );
        hash_coordinate(
            hasher,
            self.capsule_identity.sha256(),
            self.capsule_identity.byte_len(),
        );
        hash_coordinate(
            hasher,
            self.carrier_identity.sha256(),
            self.carrier_identity.byte_len(),
        );
        hash_coordinate(
            hasher,
            self.pair_binding_identity.sha256(),
            self.pair_binding_identity.byte_len(),
        );
        hasher.update(self.invocation_digest);
        hash_coordinate(
            hasher,
            self.module_handoff_identity.sha256(),
            self.module_handoff_identity.byte_len(),
        );
        hash_coordinate(
            hasher,
            self.actual_f_identity.digest(),
            self.actual_f_identity.canonical_length(),
        );
        hash_coordinate(
            hasher,
            &self.output_identity.sha256,
            self.output_identity.byte_len,
        );
        hash_coordinate(
            hasher,
            &self.history_identity.sha256,
            self.history_identity.byte_len,
        );
        hash_coordinate(
            hasher,
            &self.descriptor_identity.sha256(),
            self.descriptor_identity.byte_len(),
        );
        let profile: u16 = match self.profile {
            ProductionAmdTargetProfileV1::Gfx942 => 1,
            ProductionAmdTargetProfileV1::Gfx950 => 2,
        };
        hasher.update(profile.to_le_bytes());
        let closure = self.compiler_closure;
        hasher.update(closure.cargo_executable_sha256());
        hasher.update(closure.cargo_binding_trampoline_sha256());
        hasher.update(closure.cargo_fe2o3_binding_wrapper_sha256());
        hasher.update(closure.rustc_executable_sha256());
        hasher.update(closure.rustc_runtime_tree_sha256());
        hasher.update(closure.codegen_backend_sha256());
        hasher.update(
            closure
                .cargo_binding_transition_protocol_version()
                .to_le_bytes(),
        );
        hasher.update(closure.identity_sha256());
    }

    /// This structural binding does not authenticate protected compiler origin.
    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }
    /// This binding grants no compiler authority.
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }
    /// This binding grants no link authority.
    pub const fn grants_link_authority(&self) -> bool {
        false
    }
    /// This binding grants no publication authority.
    pub const fn grants_publication_authority(&self) -> bool {
        false
    }
    /// This binding grants no load authority.
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    /// This binding grants no launch authority.
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// A bounded resource refusal, invalid native record, or mismatched owner axis.
#[derive(Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ProtectedCompilerNativeHandoffBindingErrorV1 {
    /// The original ledger could not fund the operation.
    Resource(Resource),
    /// The fixed native lowering association failed strict decoding.
    NativeLowering(NativeLoweringAssociationErrorV1),
    /// Two retained coordinates disagreed.
    RelationshipMismatch { field: &'static str },
}

impl From<Resource> for ProtectedCompilerNativeHandoffBindingErrorV1 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}

impl From<NativeLoweringAssociationErrorV1> for ProtectedCompilerNativeHandoffBindingErrorV1 {
    fn from(error: NativeLoweringAssociationErrorV1) -> Self {
        Self::NativeLowering(error)
    }
}

impl fmt::Display for ProtectedCompilerNativeHandoffBindingErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => write!(f, "native Worker binding resource refusal: {error}"),
            Self::NativeLowering(error) => {
                write!(f, "invalid native Worker lowering association: {error:?}")
            }
            Self::RelationshipMismatch { field } => {
                write!(f, "native Worker binding relationship mismatch: {field}")
            }
        }
    }
}

impl Error for ProtectedCompilerNativeHandoffBindingErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            // The fixed lowering error itself has no std::error::Error impl.
            Self::NativeLowering(NativeLoweringAssociationErrorV1::Subject(error)) => Some(error),
            Self::NativeLowering(NativeLoweringAssociationErrorV1::Coordinate(error)) => {
                Some(error)
            }
            Self::NativeLowering(_) | Self::RelationshipMismatch { .. } => None,
        }
    }
}

/// Shared minimum reservation for the immutable recovered owner. This includes
/// the entire backing capacity, decoded metadata and recovered source/F storage;
/// enclosing token, adapter, binding and request headers remain separately paid.
pub(crate) fn native_handoff_storage_floor(
    owner: &RecoveredCompilerNativeSemanticHandoffV4,
) -> Result<usize, Resource> {
    owner
        .handoff()
        .backing_capacity()
        .checked_add(INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DECODE_METADATA_STORAGE_V4)
        .and_then(|n| n.checked_add(owner.storage().retained_storage()))
        .ok_or(Resource::Arithmetic)
}

const fn mismatch(field: &'static str) -> ProtectedCompilerNativeHandoffBindingErrorV1 {
    ProtectedCompilerNativeHandoffBindingErrorV1::RelationshipMismatch { field }
}

fn check_compiler_closure(
    capsule: CompilerClosureV2,
    invocation: CompilerClosureV2,
    expected: CompilerClosureV2,
) -> Result<(), ProtectedCompilerNativeHandoffBindingErrorV1> {
    if capsule != invocation {
        return Err(mismatch("invocation compiler closure"));
    }
    if capsule != expected {
        return Err(mismatch("expected compiler closure"));
    }
    Ok(())
}

fn check_actual_f(
    actual: &VerifiedCanonicalKernelIrIdentityV12,
    declared: &InertNativeNeutralSubjectV1,
) -> Result<(), ProtectedCompilerNativeHandoffBindingErrorV1> {
    if actual.digest() != declared.graph_digest()
        || actual.canonical_length() != declared.graph_length()
    {
        return Err(mismatch("recovered actual F"));
    }
    Ok(())
}

fn check_descriptor(
    bytes: &[u8],
    expected: TargetLineageIdentityV3,
    budget: &mut Budget<'_>,
) -> Result<(), ProtectedCompilerNativeHandoffBindingErrorV1> {
    if bytes.len() > MAX_DESCRIPTOR_TABLE_BYTES
        || u64::try_from(bytes.len()).ok() != Some(expected.byte_len())
    {
        return Err(mismatch("native descriptor byte length"));
    }
    budget.charge_work(bytes.len().checked_add(64).ok_or(Resource::Arithmetic)?)?;
    let actual: [u8; 32] = Sha256::digest(bytes).into();
    if actual != expected.sha256() {
        return Err(mismatch("native descriptor identity"));
    }
    Ok(())
}

fn hash_coordinate(hasher: &mut Sha256, digest: &[u8; 32], length: u64) {
    hasher.update(digest);
    hasher.update(length.to_le_bytes());
}

#[cfg(test)]
#[path = "first_build_worker_native_binding_tests.rs"]
mod tests;
