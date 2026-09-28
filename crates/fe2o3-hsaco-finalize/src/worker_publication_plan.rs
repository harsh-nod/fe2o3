//! Domain-separated durable coordinates shared by typed native source adapters.
use super::*;

pub(super) struct Domains {
    pub(super) context: &'static [u8],
    pub(super) request: &'static [u8],
    pub(super) plan: &'static [u8],
    pub(super) intent: &'static [u8],
    pub(super) kernel: &'static [u8],
    pub(super) target: &'static [u8],
    pub(super) worker: &'static [u8],
    pub(super) response: &'static [u8],
    pub(super) finalization: &'static [u8],
    pub(super) publication: &'static [u8],
}

pub(super) struct Inputs {
    pub(super) package: PackageIdentityV1,
    pub(super) attempt: BuildAttempt,
    pub(super) slot: u8,
    pub(super) transaction: [u8; 32],
    pub(super) outer: ContentIdentityV1,
    pub(super) binding: [u8; 32],
    pub(super) source: [u8; 32],
    pub(super) worker: [u8; 32],
    pub(super) finalized: [u8; 32],
    pub(super) transcript: [u8; 32],
    pub(super) link_plan: [u8; 32],
    pub(super) manifest: ContentIdentityV1,
    pub(super) policy: [u8; 32],
    pub(super) raw: ContentIdentityV1,
    pub(super) output: ContentIdentityV1,
    pub(super) descriptor: ContentIdentityV1,
    pub(super) canonical_digest: [u8; 32],
}

pub(super) fn derive(
    input: Inputs,
    domains: &Domains,
) -> ([u8; 32], [u8; 32], DurableLinkPublicationPlanV1) {
    // Complete native source/Worker identity already commits all response,
    // measurement, provider and option axes. Include it with the finalization,
    // transcript, producer and full occurrence in every new native identity.
    let mut hash = Sha256::new();
    hash.update(domains.context);
    hash.update(input.package.as_bytes());
    hash_attempt(&mut hash, input.attempt);
    hash.update([input.slot]);
    for identity in [
        input.transaction,
        input.binding,
        input.source,
        input.worker,
        input.finalized,
        input.transcript,
        input.link_plan,
        input.policy,
        input.canonical_digest,
    ] {
        hash.update(identity);
    }
    for content in [
        input.outer,
        input.manifest,
        input.raw,
        input.output,
        input.descriptor,
    ] {
        hash.update(content.sha256());
        hash.update(content.byte_len().to_le_bytes());
    }
    let context: [u8; 32] = hash.finalize().into();
    let domain = |domain: &[u8]| hash_parts(domain, &[&context]);
    // Scope and worker coordinates keep their meaning across attempts; the
    // complete request/plan/intent identities, not the scope, bind occurrence.
    let scope = LinkPublicationScopeV1::new(
        input.package,
        KernelSetIdentityV1::from_bytes(hash_parts(
            domains.kernel,
            &[
                input.manifest.sha256(),
                &input.manifest.byte_len().to_le_bytes(),
            ],
        )),
        TargetIdentityV1::from_bytes(hash_parts(domains.target, &[&input.policy])),
    );
    let plan = DurableLinkPublicationPlanV1::new(
        input.attempt,
        scope,
        CanonicalLinkRequestIdentityV1::from_bytes(domain(domains.request)),
        PinnedWorkerIdentityV1::from_bytes(input.worker),
        ValidatedResponseIdentityV1::from_bytes(hash_parts(
            domains.response,
            &[
                &input.source,
                input.raw.sha256(),
                &input.raw.byte_len().to_le_bytes(),
            ],
        )),
        LinkedOutputIdentityV1::from_bytes(*input.raw.sha256()),
        FinalizationIdentityV1::from_bytes(hash_parts(
            domains.finalization,
            &[
                &input.finalized,
                &input.canonical_digest,
                input.descriptor.sha256(),
                &input.descriptor.byte_len().to_le_bytes(),
                input.output.sha256(),
                &input.output.byte_len().to_le_bytes(),
            ],
        )),
        FinalizedOutputIdentityV1::from_bytes(*input.output.sha256()),
        AtomicPublicationIdentityV1::from_bytes(domain(domains.publication)),
    );
    let plan_identity = hash_parts(
        domains.plan,
        &[
            &context,
            scope.package().as_bytes(),
            scope.kernel_set().as_bytes(),
            scope.target().as_bytes(),
            plan.request().as_bytes(),
            plan.worker().as_bytes(),
            plan.response().as_bytes(),
            plan.linked_output().as_bytes(),
            plan.finalization().as_bytes(),
            plan.finalized_output().as_bytes(),
            plan.publication().as_bytes(),
        ],
    );
    let identity = hash_parts(domains.intent, &[&context, &plan_identity]);
    (identity, plan_identity, plan)
}
