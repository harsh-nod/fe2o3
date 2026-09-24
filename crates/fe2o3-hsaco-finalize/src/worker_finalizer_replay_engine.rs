//! Canonical request/response reconstruction shared by typed finalizer adapters.

use crate::{
    ContentIdentityV1, LinkInputKindClosureV1, LinkInputV1, LinkOutputV1, MultiInputLinkPlanV1,
    ProvenanceNodeV1, WorkerInputV1, WorkerOutputConstraintsV1, WorkerResponseV2,
    WorkerV3HsacoPublicationErrorV1 as Error,
    first_build_worker_binding::WorkerCompilerBinding,
    request_construction::{
        DecodedCompilerModuleHandoffV2, construct_first_build_worker_request_from_decoded,
        construct_plan_worker_request_from_decoded, decode_link_options,
    },
    worker_protocol_v2::reconstruct_complete_worker_response_v2,
    worker_v3_compact_finalizer_replay::ProtectedWorkerV3CompactFinalizerReplayViewV2,
    worker_v3_hsaco_publication::{try_copy_bytes, try_vec},
};

pub(crate) struct ReconstructedWorkerExchanges {
    pub(crate) plan: MultiInputLinkPlanV1,
    pub(crate) bootstrap_request_bytes: Vec<u8>,
    pub(crate) bootstrap_response: WorkerResponseV2,
    pub(crate) replay_request_bytes: Vec<u8>,
    pub(crate) replay_response: WorkerResponseV2,
}

pub(crate) fn reconstruct_worker_exchanges(
    binding: WorkerCompilerBinding<'_>,
    decoded: &DecodedCompilerModuleHandoffV2,
    providers: Vec<WorkerInputV1>,
    replay: &ProtectedWorkerV3CompactFinalizerReplayViewV2<'_>,
    raw_hsaco: &[u8],
) -> Result<ReconstructedWorkerExchanges, Error> {
    let (_, worker_options) = decode_link_options(replay.link_options)?;
    let raw_identity = ContentIdentityV1::calculate(raw_hsaco);
    let plan = derive_link_plan(decoded, &providers, replay.link_options, raw_identity)?;
    let input_kinds =
        LinkInputKindClosureV1::new(&plan, plan_inputs_with_kinds(decoded, &providers)?)?;
    let bootstrap_output = WorkerOutputConstraintsV1::new(replay.bootstrap_output_bound)?;
    let bootstrap = construct_first_build_worker_request_from_decoded(
        binding,
        replay.worker,
        decoded,
        providers,
        worker_options,
        bootstrap_output,
    )?;
    let bootstrap_request_bytes = try_copy_bytes(
        bootstrap.sealed_request().canonical_bytes(),
        "bootstrap request wire",
    )?;
    let bootstrap_response = reconstruct_complete_worker_response_v2(
        bootstrap.sealed_request(),
        raw_hsaco,
        replay.bootstrap_metadata,
    )?;
    let providers = bootstrap.into_external_providers();
    let replay_output = WorkerOutputConstraintsV1::new(raw_identity.byte_len())?;
    let replay_request = construct_plan_worker_request_from_decoded(
        binding,
        &plan,
        replay.worker,
        decoded,
        providers,
        &input_kinds,
        replay_output,
    )?;
    let replay_request_bytes = try_copy_bytes(
        replay_request.sealed_request().canonical_bytes(),
        "replay request wire",
    )?;
    let replay_response = reconstruct_complete_worker_response_v2(
        replay_request.sealed_request(),
        raw_hsaco,
        replay.replay_metadata,
    )?;
    Ok(ReconstructedWorkerExchanges {
        plan,
        bootstrap_request_bytes,
        bootstrap_response,
        replay_request_bytes,
        replay_response,
    })
}

fn derive_link_plan(
    decoded: &crate::request_construction::DecodedCompilerModuleHandoffV2,
    providers: &[WorkerInputV1],
    options: &[crate::LinkOptionV1],
    output_identity: ContentIdentityV1,
) -> Result<MultiInputLinkPlanV1, Error> {
    let mut link_inputs = try_vec(providers.len() + 1, "link plan inputs")?;
    for provider in providers {
        link_inputs.push(LinkInputV1::new(provider.identity(), decoded.target()));
    }
    link_inputs.push(LinkInputV1::new(
        ContentIdentityV1::calculate(decoded.compiler_module_bytes()),
        decoded.target(),
    ));
    link_inputs.sort_by_key(|input| input.identity());
    let mut provenance = try_vec(link_inputs.len() + 1, "link provenance")?;
    for input in &link_inputs {
        provenance.push(ProvenanceNodeV1::new(input.identity(), vec![])?);
    }
    let mut output_parents = try_vec(link_inputs.len(), "output provenance")?;
    for input in &link_inputs {
        output_parents.push(input.identity());
    }
    provenance.push(ProvenanceNodeV1::new(output_identity, output_parents)?);
    let mut canonical_options = try_vec(options.len(), "link options")?;
    canonical_options.extend_from_slice(options);
    Ok(MultiInputLinkPlanV1::canonicalized(
        decoded.target(),
        link_inputs,
        canonical_options,
        LinkOutputV1::new(output_identity, decoded.target()),
        provenance,
    )?)
}

fn plan_inputs_with_kinds(
    decoded: &crate::request_construction::DecodedCompilerModuleHandoffV2,
    providers: &[WorkerInputV1],
) -> Result<Vec<crate::WorkerInputKindV1>, Error> {
    let mut inputs = try_vec(providers.len() + 1, "input-kind closure")?;
    inputs.extend(
        providers
            .iter()
            .map(|input| (input.identity(), input.kind())),
    );
    let compiler_identity = ContentIdentityV1::calculate(decoded.compiler_module_bytes());
    inputs.push((compiler_identity, decoded.compiler_module_kind()));
    inputs.sort_by_key(|input| *input);
    let mut kinds = try_vec(inputs.len(), "input-kind values")?;
    for (_, kind) in inputs {
        kinds.push(kind);
    }
    Ok(kinds)
}
