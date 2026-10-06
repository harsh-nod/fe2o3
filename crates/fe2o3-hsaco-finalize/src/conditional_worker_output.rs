//! Exact conditional V5 output through the existing physical publication engine.
use super::{
    AttemptScopedHsacoPublicationResultV3 as Output, CompilerClosureV2 as Closure,
    DurableLinkPublicationPlanV1 as Plan, ProducerIdentity, PublicationSource,
    UpstreamCodeObjectEvidenceIdentityV1 as Upstream,
    WorkerV3HsacoPublicationErrorV1 as Transaction, WorkerV3PublicationBindingV1 as Binding,
    publish_retained_finalizer,
};
use crate::{
    ContentIdentityV1, NativeFirstBuildWorkerErrorV1, NativeWorkerCompactReplayErrorV1,
    RecoveredConditionalWorkerHsacoPublicationV5 as Recovered,
    first_build_worker_conditional_binding::AccountMode,
};
use fe2o3_artifact_transaction::{
    DurableLinkPublicationError, DurablePublishedHsacoClaimV3, WorkerV3PublicationBindingErrorV1,
    producer_package_identity_v1, validate_backend_publication_receipt_v3,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use sha2::{Digest, Sha256};
use std::{fmt, mem::size_of, path::Path};

#[path = "conditional_worker_output_preimages.rs"]
mod preimages;
pub use preimages::ConditionalWorkerReplayPreimagesV5;

const DOMAIN: &[u8] = b"FE2O3/CONDITIONAL-WORKER-PUBLISHED-UPSTREAM/V5\0";
const FRAME: usize = 4 * size_of::<PublishedConditionalWorkerHsacoV5>() + 8192;
const WORK: usize = 4096;

#[derive(Debug)]
pub enum ConditionalWorkerOutputErrorV5 {
    Resource(Resource),
    Source(NativeFirstBuildWorkerErrorV1),
    Transcript(NativeWorkerCompactReplayErrorV1),
    Binding(WorkerV3PublicationBindingErrorV1),
    Transaction(Transaction),
    Currentness(DurableLinkPublicationError),
    Mismatch(&'static str),
}
type Error = ConditionalWorkerOutputErrorV5;
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => write!(f, "conditional output resources: {e:?}"),
            Self::Source(e) => write!(f, "conditional output source: {e}"),
            Self::Transcript(e) => write!(f, "conditional output transcript: {e}"),
            Self::Binding(e) => write!(f, "conditional output binding: {e}"),
            Self::Transaction(e) => write!(f, "conditional output publication: {e}"),
            Self::Currentness(e) => write!(f, "conditional output currentness: {e}"),
            Self::Mismatch(e) => write!(f, "conditional output mismatch: {e}"),
        }
    }
}
impl std::error::Error for Error {}

/// Additional complete result backing to reserve while retaining this owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConditionalWorkerOutputStorageV5(usize);
impl ConditionalWorkerOutputStorageV5 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Actual conditional source/transcript plus the exact published-file lease.
/// Its public borrows are inert; it is not a V3/V89 semantic owner and cannot
/// produce compiler, proof, host load or GPU launch authority.
/// ```compile_fail
/// use fe2o3_hsaco_finalize::PublishedConditionalWorkerHsacoV5 as P;
/// fn clone(value: P) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::{PublishedConditionalWorkerHsacoV5 as C, PublishedMixedWorkerHsacoV89 as M};
/// fn downgrade(value: C) -> M { value.into() }
/// ```
pub struct PublishedConditionalWorkerHsacoV5 {
    recovered: Recovered,
    publication: Output,
    account: AccountMode,
    retained_storage: usize,
}
type Published = PublishedConditionalWorkerHsacoV5;
impl Published {
    pub fn recovered_evidence(&self) -> &Recovered {
        &self.recovered
    }
    pub fn publication_result(&self) -> &Output {
        &self.publication
    }
    pub fn published_claim(&self) -> &DurablePublishedHsacoClaimV3 {
        self.publication.published_claim()
    }
    pub const fn required_retained_storage(&self) -> usize {
        self.retained_storage
    }
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }
    pub const fn grants_proof_authority(&self) -> bool {
        false
    }
    pub const fn grants_load_authority(&self) -> bool {
        false
    }
    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    /// Rechecks the retained occurrence while holding its actual currentness
    /// token. The source account and complete nominal owner remain unchanged.
    pub fn revalidate(&self, producer: &ProducerIdentity, b: &mut Budget<'_>) -> Result<()> {
        self.account.run(b, self.retained_storage, |b| {
            b.with_prepaid_scope(self.retained_storage, 8, WORK, FRAME, |b| {
                let current = self
                    .publication
                    .current_publication_lease()
                    .acquire_current_token()
                    .map_err(Error::Currentness)?;
                let retained = Retained::new(&self.recovered, producer, b)?;
                validate_result(&retained, producer, &self.publication, b)?;
                current
                    .revalidate_locked_currentness()
                    .map_err(Error::Currentness)
            })
        })
    }
}

/// Publishes only an independently replayed actual conditional owner. The
/// existing transaction engine bounds filesystem I/O, copies and snapshots;
/// this budget pays source/identity checks and the retained result's Rust
/// backing. Caller path/producer storage remains prepaid. No machine theorem
/// or compiler origin is inferred from deterministic artifact publication.
pub fn publish_recovered_conditional_worker_hsaco_v5(
    output: &Path,
    producer: &ProducerIdentity,
    recovered: Recovered,
    b: &mut Budget<'_>,
) -> Result<(Published, ConditionalWorkerOutputStorageV5)> {
    if b.storage_limit() > fe2o3_compiler_ffi::MAX_INERT_REFINED_FORWARDING_STORAGE_V1 {
        return Err(Resource::Accounting.into());
    }
    publish_using(output, producer, recovered, b, AccountMode::LEGACY)
}

/// Same physical engine on the original owned account, preserving its identity,
/// total affordability and existing non-widening <=256 MiB operation ceiling.
pub fn publish_recovered_conditional_worker_hsaco_in_original_account_v5(
    output: &Path,
    producer: &ProducerIdentity,
    recovered: Recovered,
    b: &mut Budget<'_>,
) -> Result<(Published, ConditionalWorkerOutputStorageV5)> {
    let account = AccountMode::original(b)?;
    publish_using(output, producer, recovered, b, account)
}

fn publish_using(
    output: &Path,
    producer: &ProducerIdentity,
    recovered: Recovered,
    b: &mut Budget<'_>,
    account: AccountMode,
) -> Result<(Published, ConditionalWorkerOutputStorageV5)> {
    let floor = recovered.required_retained_storage();
    account.run(b, floor, |b| {
        b.with_prepaid_scope(floor, 8, WORK, FRAME, |b| {
            let retained = Retained::new(&recovered, producer, b)?;
            let closure = retained.binding.compiler_closure();
            let publication = publish_retained_finalizer(
                output,
                producer,
                closure,
                PublicationSource::Conditional(retained),
            )
            .map_err(Error::Transaction)?;
            let retained = Retained::new(&recovered, producer, b)?;
            validate_result(&retained, producer, &publication, b)?;
            let extra = publication
                .retained_rust_storage()
                .ok_or(Resource::Arithmetic)?
                .checked_add(size_of::<Published>() - size_of::<Recovered>() - size_of::<Output>())
                .ok_or(Resource::Arithmetic)?;
            b.reserve_storage(extra)?;
            let retained_storage = floor.checked_add(extra).ok_or(Resource::Arithmetic)?;
            Ok((
                Published {
                    recovered,
                    publication,
                    account,
                    retained_storage,
                },
                ConditionalWorkerOutputStorageV5(extra),
            ))
        })
    })
}

// This constructor is private to this module. The bridge receives its exact
// borrowed replay owner, not free-standing scalar binding claims.
pub(super) struct Retained<'a> {
    owner: &'a Recovered,
    binding: Binding,
    upstream: Upstream,
}
impl<'a> Retained<'a> {
    fn new(owner: &'a Recovered, producer: &ProducerIdentity, b: &mut Budget<'_>) -> Result<Self> {
        b.charge_work(WORK)?;
        let finalized = owner.finalized();
        let source = finalized.source();
        source.revalidate_for_artifact(b).map_err(Error::Source)?;
        owner
            .transcript()
            .verify_finalized_coordinates(finalized)
            .map_err(Error::Transcript)?;
        let plan = owner.intent().durable_plan();
        if owner.record().plan() != plan
            || owner.record().attempt() != source.binding().receipt().attempt()
            || plan.scope().package() != producer_package_identity_v1(producer)
        {
            return Err(Error::Mismatch("retained producer/attempt/plan"));
        }
        b.charge_work(finalized.finalized().as_bytes().len())?;
        let final_bytes = ContentIdentityV1::calculate(finalized.finalized().as_bytes());
        let raw = source.output_identity();
        if plan.linked_output().as_bytes() != raw.sha256()
            || plan.finalized_output().as_bytes() != final_bytes.sha256()
        {
            return Err(Error::Mismatch("retained exact output"));
        }
        let binding = Binding::new(
            source.binding().compiler_closure(),
            owner.record().identity().as_bytes(),
            *finalized.identity(),
            *source.identity(),
            *source.binding().identity(),
            finalized.publication_inspection_identity(),
            *raw.sha256(),
            raw.byte_len(),
            *final_bytes.sha256(),
            final_bytes.byte_len(),
        )
        .map_err(Error::Binding)?;
        let upstream = upstream(
            owner.intent().identity(),
            owner.intent().plan_identity(),
            &owner.record().identity().as_bytes(),
            owner.transcript().identity().as_bytes(),
            &binding,
        );
        Ok(Self {
            owner,
            binding,
            upstream,
        })
    }

    pub(super) fn parts(
        self,
        closure: Closure,
    ) -> std::result::Result<(Binding, Plan, Upstream, &'a [u8]), Transaction> {
        if closure != self.binding.compiler_closure() {
            return Err(Transaction::CompilerClosureMismatch);
        }
        Ok((
            self.binding,
            self.owner.intent().durable_plan(),
            self.upstream,
            self.owner.finalized().finalized().as_bytes(),
        ))
    }
}

fn validate_result(
    retained: &Retained<'_>,
    producer: &ProducerIdentity,
    publication: &Output,
    b: &mut Budget<'_>,
) -> Result<()> {
    b.charge_work(WORK)?;
    let plan = retained.owner.intent().durable_plan();
    validate_backend_publication_receipt_v3(
        producer,
        plan.attempt(),
        plan,
        retained.upstream,
        retained.binding,
        publication.receipt(),
    )
    .map_err(|_| Error::Mismatch("exact publication receipt"))?;
    let bytes = retained.owner.finalized().finalized().as_bytes();
    b.charge_work(bytes.len())?;
    if publication.snapshot().artifact().bytes() != bytes {
        return Err(Error::Mismatch("descriptor-derived artifact bytes"));
    }
    Ok(())
}

fn upstream(
    intent: &[u8; 32],
    plan: &[u8; 32],
    record: &[u8; 32],
    transcript: &[u8; 32],
    binding: &Binding,
) -> Upstream {
    let mut h = Sha256::new();
    h.update(DOMAIN);
    for id in [
        intent,
        plan,
        record,
        transcript,
        &binding.compiler_closure().identity_sha256(),
        &binding.publication_intent_record_identity(),
        &binding.finalization_identity(),
        &binding.source_evidence_identity(),
        &binding.compiler_handoff_binding_identity(),
        &binding.raw_inspection_identity(),
        &binding.raw_output_sha256(),
        &binding.finalized_output_sha256(),
    ] {
        h.update(id);
    }
    h.update(binding.raw_output_length().to_le_bytes());
    h.update(binding.finalized_output_length().to_le_bytes());
    Upstream::from_bytes(h.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Only inert coordinates: this never constructs a replay or publication owner.
    fn binding(changed: Option<usize>) -> Binding {
        let mut pins = [[1; 32], [2; 32], [3; 32], [4; 32], [5; 32], [6; 32]];
        let mut axes = [
            [7; 32], [8; 32], [9; 32], [10; 32], [11; 32], [12; 32], [13; 32],
        ];
        let mut lengths = [17, 23];
        if let Some(index) = changed {
            if index < 6 {
                pins[index][0] ^= 128;
            } else if index < 13 {
                axes[index - 6][0] ^= 128;
            } else {
                lengths[index - 13] += 1;
            }
        }
        let closure = Closure::new(pins[0], pins[1], pins[2], pins[3], pins[4], pins[5]).unwrap();
        Binding::new(
            closure, axes[0], axes[1], axes[2], axes[3], axes[4], axes[5], lengths[0], axes[6],
            lengths[1],
        )
        .unwrap()
    }

    #[test]
    fn conditional_upstream_binds_every_retained_record_and_finalizer_axis() {
        let coordinates = [[31; 32], [32; 32], [33; 32], [34; 32]];
        let derive = |values: &[[u8; 32]; 4], binding: &Binding| {
            upstream(&values[0], &values[1], &values[2], &values[3], binding)
        };
        let original = derive(&coordinates, &binding(None));
        assert_eq!(derive(&coordinates, &binding(None)), original);
        for index in 0..4 {
            let mut changed = coordinates;
            changed[index][0] ^= 128;
            assert_ne!(
                derive(&changed, &binding(None)),
                original,
                "coordinate {index}"
            );
        }
        for index in 0..15 {
            assert_ne!(
                derive(&coordinates, &binding(Some(index))),
                original,
                "axis {index}"
            );
        }
        assert_ne!(DOMAIN, super::super::UPSTREAM_DOMAIN_V1);
    }

    #[test]
    fn conditional_published_result_counts_full_actual_snapshot_not_only_header() {
        assert!(FRAME >= 4 * size_of::<Published>());
        assert!(size_of::<Published>() >= size_of::<Recovered>() + size_of::<Output>());
        assert!(!std::mem::needs_drop::<ConditionalWorkerOutputStorageV5>());
        // The actual descriptor/snapshot/lease quote is covered by the existing
        // attempt_scoped_hsaco_publication_v3 filesystem round-trip fixture.
    }
}
