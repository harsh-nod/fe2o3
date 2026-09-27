//! Binding of an actual recovered conditional V5 owner to the shared worker.
use crate::NativeFirstBuildWorkerErrorV1 as Error;
use fe2o3_artifact_transaction::CompilerModuleHandoffReceiptV5 as Receipt;
use fe2o3_build_authority::CompilerClosureV2 as Closure;
use fe2o3_compiler_ffi::{
    INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DECODE_METADATA_STORAGE_V5 as METADATA,
    MAX_INERT_REFINED_FORWARDING_STORAGE_V1 as MAX_STORAGE,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    VerifiedCanonicalKernelIrIdentityV12 as GraphIdentity,
};
use fe2o3_verifier::RecoveredCompilerConditionalNativeSemanticHandoffV5 as Source;
use sha2::{Digest, Sha256};
use std::mem::size_of;

const DOMAIN: &[u8] = b"FE2O3/PROTECTED-WORKER-COMPILER-CONDITIONAL-HANDOFF-BINDING/V2\0";
const WORK: usize = 8192;
const SCRATCH: usize = 4 * size_of::<Binding>() + 2 * size_of::<Sha256>() + 4096;
type Binding = ProtectedCompilerConditionalHandoffBindingV2;

/// Inert coordinates, not source proof or protected compiler origin. The
/// complete recovered V5 owner must remain live; no V4 projection is exposed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProtectedCompilerConditionalHandoffBindingV2 {
    receipt: Receipt,
    compiler_closure: Closure,
    final_graph: GraphIdentity,
    catalog: [u8; 32],
    catalog_length: u64,
    identity: [u8; 32],
}
impl Binding {
    pub(crate) fn from_handoff(
        source: &Source,
        receipt: Receipt,
        compiler_closure: Closure,
        b: &mut Budget<'_>,
    ) -> Result<Self, Error> {
        let floor = storage_floor(source)?;
        b.with_prepaid_scope(floor, 8, WORK, SCRATCH, |b| {
            require_storage_limit(b)?;
            let handoff = source.handoff();
            let capsule = handoff.capsule();
            if handoff.identity() != receipt.handoff_identity()
                || receipt.length() != handoff.canonical_bytes().len()
                || u64::try_from(receipt.length()).ok() != Some(handoff.identity().byte_len())
            {
                return Err(Error::PreflightMismatch(
                    "complete conditional V5 occurrence",
                ));
            }
            if *capsule.invocation().compiler_closure() != compiler_closure {
                return Err(Error::PreflightMismatch("conditional compiler closure"));
            }
            let graph = source.output().canonical().identity();
            let declared = capsule.native_lowering().inputs();
            if graph.digest() != declared.final_native.graph_digest()
                || graph.canonical_length() != declared.final_native.graph_length()
                || source.catalog().digest() != declared.final_native.catalog_digest()
                || u64::try_from(source.catalog().canonical_bytes().len()).ok()
                    != Some(declared.final_native.catalog_length())
                || source.profile() != declared.profile
            {
                return Err(Error::PreflightMismatch(
                    "conditional final graph/catalog/profile",
                ));
            }
            // Recovery already joined exact carrier, descriptor, LLVM, module,
            // source policy and final graph. The private V5 identity commits all
            // those immutable bytes; do not decode or copy a second graph here.
            let mut value = Self {
                receipt,
                compiler_closure,
                final_graph: *graph,
                catalog: *source.catalog().digest(),
                catalog_length: declared.final_native.catalog_length(),
                identity: [0; 32],
            };
            let mut hash = Sha256::new();
            value.hash_identity_preimage(&mut hash);
            value.identity = hash.finalize().into();
            Ok(value)
        })
    }
    pub const fn receipt(&self) -> Receipt {
        self.receipt
    }
    pub const fn compiler_closure(&self) -> Closure {
        self.compiler_closure
    }
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }
    pub const fn actual_f_identity(&self) -> GraphIdentity {
        self.final_graph
    }
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    pub(crate) fn hash_identity_preimage(&self, h: &mut Sha256) {
        h.update(DOMAIN);
        let attempt = self.receipt.attempt();
        h.update(attempt.generation().to_le_bytes());
        h.update(attempt.session().as_bytes());
        h.update(attempt.invocation().as_bytes());
        h.update([self.receipt.slot() as u8]);
        h.update(self.receipt.transaction_identity().as_bytes());
        h.update(self.receipt.handoff_identity().sha256());
        h.update(self.receipt.handoff_identity().byte_len().to_le_bytes());
        h.update(self.compiler_closure.identity_sha256());
        h.update(self.final_graph.digest());
        h.update(self.final_graph.canonical_length().to_le_bytes());
        h.update(self.catalog);
        h.update(self.catalog_length.to_le_bytes());
    }
}

pub(crate) fn storage_floor(source: &Source) -> Result<usize, Resource> {
    source
        .handoff()
        .backing_capacity()
        .checked_add(METADATA)
        .and_then(|n| n.checked_add(source.storage().retained_storage()))
        .ok_or(Resource::Arithmetic)
}

pub(crate) fn require_storage_limit(b: &Budget<'_>) -> Result<(), Error> {
    if b.storage_limit() > MAX_STORAGE {
        return Err(Resource::Accounting.into());
    }
    Ok(())
}
