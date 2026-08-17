use std::cell::Cell;
use std::rc::Rc;

use ed25519_dalek::{Signer, SigningKey};
use fe2o3_build_authority::{
    BrokerTranscriptValidatorV4, CapabilityBindingV4, CompletedBrokerTranscriptV4,
    HOST_LINK_OUTPUT_MODE_V4, HostLinkCommitV4, HostLinkGrantV4, HostLinkPrepareV4,
    ProcessIdentityV4,
};
use fe2o3_external_anchor_protocol::{
    ANCHOR_CHALLENGE_WIRE_LEN_V1, ANCHOR_OBSERVATION_WIRE_LEN_V1, AnchorChallengeV1,
    AnchorPositionV1, AnchoredStateV1, HashChainHeadV1, PinnedAnchorKeyV1,
    UnsignedAnchorObservationV1,
};

use super::*;

#[derive(Clone, Copy)]
struct TranscriptFixture {
    process: ProcessIdentityV4,
    binding: CapabilityBindingV4,
    request: [u8; 32],
    plan: [u8; 32],
    closure: [u8; 32],
    grant: [u8; 32],
    output: [u8; 32],
    output_length: u64,
    durable_plan: [u8; 32],
}

impl TranscriptFixture {
    fn new(seed: u8) -> Self {
        Self {
            process: ProcessIdentityV4::new(u32::from(seed) + 100, u64::from(seed) + 1_000)
                .unwrap(),
            binding: CapabilityBindingV4::new(
                digest(seed.wrapping_add(1)),
                digest(seed.wrapping_add(2)),
                digest(seed.wrapping_add(3)),
            )
            .unwrap(),
            request: digest(seed.wrapping_add(4)),
            plan: digest(seed.wrapping_add(5)),
            closure: digest(seed.wrapping_add(6)),
            grant: digest(seed.wrapping_add(7)),
            output: digest(seed.wrapping_add(8)),
            output_length: u64::from(seed) + 4_096,
            durable_plan: digest(seed.wrapping_add(9)),
        }
    }

    fn completed(self) -> CompletedBrokerTranscriptV4 {
        let binding = self.binding.identity_sha256();
        let prepare =
            HostLinkPrepareV4::new(self.process, binding, self.request, self.plan, self.closure)
                .unwrap();
        let grant = HostLinkGrantV4::new(
            self.process,
            binding,
            self.request,
            self.plan,
            self.closure,
            self.grant,
        )
        .unwrap();
        let commit = HostLinkCommitV4::new(
            self.process,
            binding,
            self.request,
            self.plan,
            self.closure,
            self.grant,
            self.output,
            self.output_length,
            HOST_LINK_OUTPUT_MODE_V4,
            self.durable_plan,
        )
        .unwrap();
        BrokerTranscriptValidatorV4::new(self.binding, self.process)
            .validate_prepare(prepare)
            .unwrap()
            .validate_grant(grant)
            .unwrap()
            .validate_commit(commit)
            .unwrap()
    }
}

fn digest(seed: u8) -> [u8; 32] {
    [seed.max(1); 32]
}

fn session_id(seed: u8) -> BrokerSessionIdV1 {
    BrokerSessionIdV1::from_bytes(digest(seed)).unwrap()
}

fn nonce(seed: u8) -> BrokerSessionNonceV1 {
    BrokerSessionNonceV1::from_bytes(digest(seed)).unwrap()
}

fn durable(seed: u8) -> DurablePublicationPlanIdentityV1 {
    DurablePublicationPlanIdentityV1::from_bytes(digest(seed)).unwrap()
}

fn reservation(
    transcript: &CompletedBrokerTranscriptV4,
    id_seed: u8,
    nonce_seed: u8,
) -> BrokerSessionReservationV1 {
    BrokerSessionReservationV1::new(
        session_id(id_seed),
        nonce(nonce_seed),
        transcript.session_claim(),
        DurablePublicationPlanIdentityV1::from_bytes(transcript.durable_plan_identity()).unwrap(),
    )
    .unwrap()
}

fn completion(transcript: &CompletedBrokerTranscriptV4) -> CompletionBindingV1 {
    CompletionBindingV1 {
        claim_digest: broker_session_claim_digest_v1(transcript.session_claim()),
        transcript_digest: completed_broker_transcript_digest_v1(transcript),
        output_digest: transcript.output_sha256(),
        durable_plan: DurablePublicationPlanIdentityV1::from_bytes(
            transcript.durable_plan_identity(),
        )
        .unwrap(),
        broker_reservation: None,
        request_nonce_sha256: [0; 32],
    }
}

fn linked_completion(
    core: &SessionCoreV1<(), ()>,
    transcript: &CompletedBrokerTranscriptV4,
) -> CompletionBindingV1 {
    let link = core.link.expect("test core must retain a link binding");
    CompletionBindingV1 {
        broker_reservation: Some(link.broker_reservation),
        request_nonce_sha256: link.request_nonce_sha256,
        ..completion(transcript)
    }
}

fn begin_test_link(
    core: &mut SessionCoreV1<(), ()>,
    permit: &mut BrokerHostLinkPermitV1,
    transcript: &CompletedBrokerTranscriptV4,
    request_nonce_seed: u8,
) {
    core.validate_link_start(
        permit,
        transcript.plan_identity(),
        transcript.closure_identity(),
    )
    .unwrap();
    let reservation_digest = core.reservation_digest().unwrap();
    permit.consume_for(reservation_digest).unwrap();
    core.commit_link_start(reservation_digest, digest(request_nonce_seed));
}

fn signing_key(seed: u8) -> (SigningKey, PinnedAnchorKeyV1) {
    let signing = SigningKey::from_bytes(&digest(seed));
    let pinned = PinnedAnchorKeyV1::from_bytes(signing.verifying_key().to_bytes()).unwrap();
    (signing, pinned)
}

fn signed_observation(
    challenge: &BrokerAnchorChallengeObservationV1,
    position: AnchorPositionV1,
    signing: &SigningKey,
) -> [u8; ANCHOR_OBSERVATION_WIRE_LEN_V1] {
    let decoded = AnchorChallengeV1::decode(challenge.as_bytes()).unwrap();
    let unsigned = UnsignedAnchorObservationV1::from_challenge(&decoded, position);
    let signature = signing.sign(&unsigned.signing_bytes()).to_bytes();
    unsigned.attach_signature(signature)
}

fn completed_core(
    fixture: TranscriptFixture,
    id_seed: u8,
    nonce_seed: u8,
) -> (
    SessionCoreV1<(), ()>,
    CompletedBrokerTranscriptV4,
    SigningKey,
    PinnedAnchorKeyV1,
) {
    let transcript = fixture.completed();
    let mut core = SessionCoreV1::new();
    let mut permit = core
        .reserve((), reservation(&transcript, id_seed, nonce_seed), true)
        .unwrap();
    begin_test_link(&mut core, &mut permit, &transcript, 92);
    core.complete((), linked_completion(&core, &transcript))
        .unwrap();
    let (signing, key) = signing_key(91);
    (core, transcript, signing, key)
}

fn anchor_pending(
    mode: BrokerAnchorModeV1,
    fixture: TranscriptFixture,
) -> (
    SessionCoreV1<(), ()>,
    CompletedBrokerTranscriptV4,
    SigningKey,
    BrokerAnchorChallengeObservationV1,
) {
    let (mut core, transcript, signing, key) = completed_core(fixture, 80, 81);
    core.prepare_anchor(
        AnchoredStateV1::from_local_state(7, HashChainHeadV1::from_bytes(digest(82))),
        &key,
    )
    .unwrap();
    let challenge = core.begin_anchor(mode, &key).unwrap();
    (core, transcript, signing, challenge)
}

#[test]
fn authority_capacity_and_observations_are_frozen() {
    assert_eq!(BROKER_SESSION_MACHINE_AUTHORITY_V1, "none");
    assert_eq!(BROKER_SESSION_CAPACITY_V1, 1);
    assert_eq!(ANCHOR_CHALLENGE_WIRE_LEN_V1, 184);
    let core = SessionCoreV1::<(), ()>::new();
    let observation = core.observation();
    assert_eq!(observation.stage(), BrokerSessionStageV1::Vacant);
    assert_eq!(observation.authority(), "none");
}

#[test]
fn fixed_width_session_inputs_reject_zero_and_ambiguous_domains() {
    assert_eq!(
        BrokerSessionIdV1::from_bytes([0; 32]).unwrap_err().kind(),
        BrokerSessionErrorKindV1::ZeroSessionId
    );
    assert_eq!(
        BrokerSessionNonceV1::from_bytes([0; 32])
            .unwrap_err()
            .kind(),
        BrokerSessionErrorKindV1::ZeroSessionNonce
    );
    assert_eq!(
        DurablePublicationPlanIdentityV1::from_bytes([0; 32])
            .unwrap_err()
            .kind(),
        BrokerSessionErrorKindV1::ZeroDurablePublicationPlan
    );
    let transcript = TranscriptFixture::new(1).completed();
    assert_eq!(
        BrokerSessionReservationV1::new(
            session_id(50),
            nonce(50),
            transcript.session_claim(),
            durable(51),
        )
        .unwrap_err()
        .kind(),
        BrokerSessionErrorKindV1::SessionIdNonceCollision
    );
}

#[test]
fn completed_transcript_digest_binds_every_constructible_terminal_field() {
    let baseline = TranscriptFixture::new(10);
    let expected = completed_broker_transcript_digest_v1(&baseline.completed());
    assert_eq!(
        expected,
        [
            249, 189, 26, 90, 171, 242, 170, 51, 141, 1, 143, 35, 85, 232, 111, 212, 247, 194, 128,
            42, 191, 3, 200, 138, 178, 94, 134, 42, 151, 115, 214, 48,
        ]
    );
    let mut variants = Vec::new();

    let mut value = baseline;
    value.process = ProcessIdentityV4::new(999, value.process.start_time_ticks()).unwrap();
    variants.push(value);
    let mut value = baseline;
    value.process = ProcessIdentityV4::new(value.process.pid(), 999_999).unwrap();
    variants.push(value);
    let mut value = baseline;
    value.binding = TranscriptFixture::new(30).binding;
    variants.push(value);
    let mut value = baseline;
    value.request = digest(31);
    variants.push(value);
    let mut value = baseline;
    value.plan = digest(32);
    variants.push(value);
    let mut value = baseline;
    value.closure = digest(33);
    variants.push(value);
    let mut value = baseline;
    value.grant = digest(34);
    variants.push(value);
    let mut value = baseline;
    value.output = digest(35);
    variants.push(value);
    let mut value = baseline;
    value.output_length += 1;
    variants.push(value);
    let mut value = baseline;
    value.durable_plan = digest(36);
    variants.push(value);

    for variant in variants {
        assert_ne!(
            completed_broker_transcript_digest_v1(&variant.completed()),
            expected
        );
    }
    assert_eq!(
        completed_broker_transcript_digest_v1(&baseline.completed()),
        expected
    );
}

#[test]
fn reserve_binds_client_once_and_never_reopens_capacity() {
    let transcript = TranscriptFixture::new(40).completed();
    let mut mismatch = SessionCoreV1::<(), ()>::new();
    assert_eq!(
        mismatch
            .reserve((), reservation(&transcript, 41, 42), false)
            .unwrap_err()
            .kind(),
        BrokerSessionErrorKindV1::ClientIdentityMismatch
    );
    assert_eq!(mismatch.stage, BrokerSessionStageV1::Vacant);

    let mut core = SessionCoreV1::<(), ()>::new();
    let permit = core
        .reserve((), reservation(&transcript, 41, 42), true)
        .unwrap();
    assert!(!permit.is_consumed());
    assert_eq!(permit.authority(), "none");
    assert_eq!(core.stage, BrokerSessionStageV1::Reserved);
    assert_eq!(
        core.reserve((), reservation(&transcript, 43, 44), true)
            .unwrap_err()
            .kind(),
        BrokerSessionErrorKindV1::CapacityOccupied
    );
    assert_eq!(core.stage, BrokerSessionStageV1::Reserved);
}

#[test]
fn completion_requires_reservation_claim_and_exact_durable_plan_once() {
    let first = TranscriptFixture::new(50).completed();
    let second = TranscriptFixture::new(60).completed();
    let mut empty = SessionCoreV1::<(), ()>::new();
    assert_eq!(
        empty.complete((), completion(&first)).unwrap_err().kind(),
        BrokerSessionErrorKindV1::TransitionOrder
    );

    let mut core = SessionCoreV1::<(), ()>::new();
    let mut permit = core.reserve((), reservation(&first, 70, 71), true).unwrap();
    assert_eq!(
        core.complete((), completion(&first)).unwrap_err().kind(),
        BrokerSessionErrorKindV1::TransitionOrder
    );
    assert_eq!(core.stage, BrokerSessionStageV1::Reserved);
    begin_test_link(&mut core, &mut permit, &first, 72);
    let mut wrong_claim = linked_completion(&core, &first);
    wrong_claim.claim_digest = completion(&second).claim_digest;
    assert_eq!(
        core.complete((), wrong_claim).unwrap_err().kind(),
        BrokerSessionErrorKindV1::TranscriptClaimMismatch
    );
    assert_eq!(core.stage, BrokerSessionStageV1::Linking);
    let mut wrong_plan = linked_completion(&core, &first);
    wrong_plan.durable_plan = durable(99);
    assert_eq!(
        core.complete((), wrong_plan).unwrap_err().kind(),
        BrokerSessionErrorKindV1::DurablePublicationPlanMismatch
    );
    assert_eq!(core.stage, BrokerSessionStageV1::Linking);
    core.complete((), linked_completion(&core, &first)).unwrap();
    assert_eq!(
        core.complete((), linked_completion(&core, &first))
            .unwrap_err()
            .kind(),
        BrokerSessionErrorKindV1::TransitionOrder
    );
}

#[test]
fn link_permit_rejects_substitution_duplicate_use_and_reordering_without_state_loss() {
    let transcript = TranscriptFixture::new(61).completed();
    let mut first = SessionCoreV1::<(), ()>::new();
    let mut first_permit = first
        .reserve((), reservation(&transcript, 62, 63), true)
        .unwrap();
    let mut second = SessionCoreV1::<(), ()>::new();
    let second_permit = second
        .reserve((), reservation(&transcript, 64, 65), true)
        .unwrap();

    assert_eq!(
        first
            .validate_link_start(
                &second_permit,
                transcript.plan_identity(),
                transcript.closure_identity(),
            )
            .unwrap_err()
            .kind(),
        BrokerSessionErrorKindV1::LinkPermitSubstitution
    );
    assert_eq!(first.stage, BrokerSessionStageV1::Reserved);
    assert!(!first_permit.is_consumed());
    assert!(!second_permit.is_consumed());

    assert_eq!(
        first
            .validate_link_start(&first_permit, digest(200), transcript.closure_identity(),)
            .unwrap_err()
            .kind(),
        BrokerSessionErrorKindV1::HostLinkPlanMismatch
    );
    assert_eq!(
        first
            .validate_link_start(&first_permit, transcript.plan_identity(), digest(201))
            .unwrap_err()
            .kind(),
        BrokerSessionErrorKindV1::HostLinkClosureMismatch
    );
    assert_eq!(first.stage, BrokerSessionStageV1::Reserved);
    assert!(!first_permit.is_consumed());

    begin_test_link(&mut first, &mut first_permit, &transcript, 68);
    assert!(first_permit.is_consumed());
    assert_eq!(first.stage, BrokerSessionStageV1::Linking);
    assert_eq!(
        first
            .validate_link_start(
                &first_permit,
                transcript.plan_identity(),
                transcript.closure_identity(),
            )
            .unwrap_err()
            .kind(),
        BrokerSessionErrorKindV1::LinkPermitAlreadyConsumed
    );
    assert_eq!(first.stage, BrokerSessionStageV1::Linking);
    assert_eq!(second.stage, BrokerSessionStageV1::Reserved);
}

#[test]
fn preexisting_output_and_request_binding_substitutions_are_rejected() {
    let transcript = TranscriptFixture::new(69).completed();
    let mut core = SessionCoreV1::<(), ()>::new();
    let mut permit = core
        .reserve((), reservation(&transcript, 70, 71), true)
        .unwrap();
    let preexisting_output = completion(&transcript);

    assert_eq!(
        core.complete((), preexisting_output).unwrap_err().kind(),
        BrokerSessionErrorKindV1::TransitionOrder
    );
    assert_eq!(core.stage, BrokerSessionStageV1::Reserved);
    assert!(!permit.is_consumed());

    begin_test_link(&mut core, &mut permit, &transcript, 72);
    assert_eq!(
        core.complete((), preexisting_output).unwrap_err().kind(),
        BrokerSessionErrorKindV1::HostLinkReservationMismatch
    );
    assert_eq!(core.stage, BrokerSessionStageV1::Linking);
    assert!(core.output.is_none());

    let exact = linked_completion(&core, &transcript);
    let mut wrong_reservation = exact;
    wrong_reservation.broker_reservation = Some(digest(73));
    assert_eq!(
        core.complete((), wrong_reservation).unwrap_err().kind(),
        BrokerSessionErrorKindV1::HostLinkReservationMismatch
    );
    let mut wrong_nonce = exact;
    wrong_nonce.request_nonce_sha256 = digest(74);
    assert_eq!(
        core.complete((), wrong_nonce).unwrap_err().kind(),
        BrokerSessionErrorKindV1::HostLinkRequestNonceMismatch
    );
    assert_eq!(core.stage, BrokerSessionStageV1::Linking);
    assert!(core.output.is_none());

    core.complete((), exact).unwrap();
    assert_eq!(core.stage, BrokerSessionStageV1::Completed);
}

#[test]
fn reservation_identity_binds_pid_and_start_time_independently() {
    let baseline = TranscriptFixture::new(75);
    let baseline_transcript = baseline.completed();
    let baseline_binding = ReservationBindingV1::from(reservation(&baseline_transcript, 76, 77));
    let baseline_digest = broker_link_reservation_digest_v1(baseline_binding);

    let mut reused_pid = baseline;
    reused_pid.process = ProcessIdentityV4::new(
        baseline.process.pid(),
        baseline.process.start_time_ticks() + 1,
    )
    .unwrap();
    let reused_binding = ReservationBindingV1::from(reservation(&reused_pid.completed(), 76, 77));
    assert_ne!(
        broker_link_reservation_digest_v1(reused_binding),
        baseline_digest
    );

    let mut substituted_pid = baseline;
    substituted_pid.process = ProcessIdentityV4::new(
        baseline.process.pid() + 1,
        baseline.process.start_time_ticks(),
    )
    .unwrap();
    let substituted_binding =
        ReservationBindingV1::from(reservation(&substituted_pid.completed(), 76, 77));
    assert_ne!(
        broker_link_reservation_digest_v1(substituted_binding),
        baseline_digest
    );

    for substituted in [reused_pid.completed(), substituted_pid.completed()] {
        let mut core = SessionCoreV1::<(), ()>::new();
        assert_eq!(
            core.reserve((), reservation(&substituted, 76, 77), false)
                .unwrap_err()
                .kind(),
            BrokerSessionErrorKindV1::ClientIdentityMismatch
        );
        assert_eq!(core.stage, BrokerSessionStageV1::Vacant);
        assert!(core.client.is_none());
        assert!(core.reservation.is_none());
        assert!(core.link.is_none());
    }
}

#[test]
fn transition_order_rejects_linkless_anchor_and_premature_consume() {
    let transcript = TranscriptFixture::new(70).completed();
    let (_, key) = signing_key(71);
    let stable = || AnchoredStateV1::from_local_state(1, HashChainHeadV1::from_bytes(digest(72)));
    let mut core = SessionCoreV1::<(), ()>::new();
    assert_eq!(
        core.prepare_anchor(stable(), &key).unwrap_err().kind(),
        BrokerSessionErrorKindV1::TransitionOrder
    );
    assert_eq!(
        core.begin_anchor(BrokerAnchorModeV1::Advance, &key)
            .unwrap_err()
            .kind(),
        BrokerSessionErrorKindV1::TransitionOrder
    );
    assert_eq!(
        core.observe_anchor(&[]).unwrap_err().kind(),
        BrokerSessionErrorKindV1::TransitionOrder
    );
    assert_eq!(
        core.consume_publication(&transcript).unwrap_err().kind(),
        BrokerSessionErrorKindV1::TransitionOrder
    );
}

#[test]
fn advance_requires_valid_proposed_observation_before_one_consume() {
    let (mut core, transcript, signing, challenge) =
        anchor_pending(BrokerAnchorModeV1::Advance, TranscriptFixture::new(80));
    assert_eq!(challenge.authority(), "none");
    let wire = signed_observation(&challenge, AnchorPositionV1::Proposed, &signing);
    assert_eq!(
        core.observe_anchor(&wire).unwrap().stage(),
        BrokerSessionStageV1::AnchorCommitted
    );
    assert_eq!(
        core.consume_publication(&transcript).unwrap().stage(),
        BrokerSessionStageV1::Consumed
    );
    assert_eq!(
        core.consume_publication(&transcript).unwrap_err().kind(),
        BrokerSessionErrorKindV1::TransitionOrder
    );
    assert_eq!(core.stage, BrokerSessionStageV1::Consumed);
}

#[test]
fn consume_rejects_claim_output_plan_and_other_transcript_substitutions() {
    let (mut core, transcript, signing, challenge) =
        anchor_pending(BrokerAnchorModeV1::Advance, TranscriptFixture::new(90));
    let wire = signed_observation(&challenge, AnchorPositionV1::Proposed, &signing);
    core.observe_anchor(&wire).unwrap();

    let mut wrong_claim = TranscriptFixture::new(90);
    wrong_claim.request = digest(201);
    assert_eq!(
        core.consume_publication(&wrong_claim.completed())
            .unwrap_err()
            .kind(),
        BrokerSessionErrorKindV1::TranscriptClaimMismatch
    );

    let mut wrong_output = TranscriptFixture::new(90);
    wrong_output.output = digest(202);
    assert_eq!(
        core.consume_publication(&wrong_output.completed())
            .unwrap_err()
            .kind(),
        BrokerSessionErrorKindV1::OutputDigestMismatch
    );

    let mut wrong_plan = TranscriptFixture::new(90);
    wrong_plan.durable_plan = digest(203);
    assert_eq!(
        core.consume_publication(&wrong_plan.completed())
            .unwrap_err()
            .kind(),
        BrokerSessionErrorKindV1::DurablePublicationPlanMismatch
    );

    let mut wrong_transcript = TranscriptFixture::new(90);
    wrong_transcript.grant = digest(204);
    assert_eq!(
        core.consume_publication(&wrong_transcript.completed())
            .unwrap_err()
            .kind(),
        BrokerSessionErrorKindV1::TranscriptDigestMismatch
    );
    assert_eq!(
        core.consume_publication(&transcript).unwrap().stage(),
        BrokerSessionStageV1::Consumed
    );
}

#[test]
fn exact_prior_observation_deterministically_aborts_and_cannot_consume() {
    let (mut core, transcript, signing, challenge) =
        anchor_pending(BrokerAnchorModeV1::Advance, TranscriptFixture::new(100));
    let wire = signed_observation(&challenge, AnchorPositionV1::Prior, &signing);
    assert_eq!(
        core.observe_anchor(&wire).unwrap().stage(),
        BrokerSessionStageV1::Aborted
    );
    assert_eq!(
        core.consume_publication(&transcript).unwrap_err().kind(),
        BrokerSessionErrorKindV1::TransitionOrder
    );
    assert_eq!(
        core.observe_anchor(&wire).unwrap_err().kind(),
        BrokerSessionErrorKindV1::TransitionOrder
    );
}

#[test]
fn recovery_mode_resolves_both_exact_anchor_positions() {
    for (position, expected_stage, seed) in [
        (AnchorPositionV1::Prior, BrokerSessionStageV1::Aborted, 110),
        (
            AnchorPositionV1::Proposed,
            BrokerSessionStageV1::AnchorCommitted,
            120,
        ),
    ] {
        let (mut core, _, signing, challenge) =
            anchor_pending(BrokerAnchorModeV1::Recovery, TranscriptFixture::new(seed));
        let decoded = AnchorChallengeV1::decode(challenge.as_bytes()).unwrap();
        assert_eq!(
            decoded.kind(),
            fe2o3_external_anchor_protocol::ChallengeKindV1::Recover
        );
        let wire = signed_observation(&challenge, position, &signing);
        assert_eq!(core.observe_anchor(&wire).unwrap().stage(), expected_stage);
    }
}

#[test]
fn substituted_or_invalid_anchor_observation_fails_closed_once() {
    let (mut core, _, _, challenge) =
        anchor_pending(BrokerAnchorModeV1::Advance, TranscriptFixture::new(130));
    let (other_signing, _) = signing_key(131);
    let wire = signed_observation(&challenge, AnchorPositionV1::Proposed, &other_signing);
    assert_eq!(
        core.observe_anchor(&wire).unwrap_err().kind(),
        BrokerSessionErrorKindV1::AnchorProtocol
    );
    assert_eq!(core.stage, BrokerSessionStageV1::Invalidated);
    assert_eq!(
        core.observe_anchor(&wire).unwrap_err().kind(),
        BrokerSessionErrorKindV1::TransitionOrder
    );
}

#[test]
fn every_single_byte_anchor_observation_mutation_fails_closed() {
    let (_, _, signing, canonical_challenge) =
        anchor_pending(BrokerAnchorModeV1::Advance, TranscriptFixture::new(132));
    let canonical = signed_observation(&canonical_challenge, AnchorPositionV1::Proposed, &signing);

    for offset in 0..ANCHOR_OBSERVATION_WIRE_LEN_V1 {
        let (mut core, _, loop_signing, challenge) =
            anchor_pending(BrokerAnchorModeV1::Advance, TranscriptFixture::new(132));
        let mut mutated = signed_observation(&challenge, AnchorPositionV1::Proposed, &loop_signing);
        assert_eq!(mutated, canonical);
        mutated[offset] ^= 1;
        assert_eq!(
            core.observe_anchor(&mutated).unwrap_err().kind(),
            BrokerSessionErrorKindV1::AnchorProtocol,
            "offset {offset}"
        );
        assert_eq!(
            core.stage,
            BrokerSessionStageV1::Invalidated,
            "offset {offset}"
        );
    }
}

#[test]
fn stale_nonce_or_transaction_challenge_is_rejected_and_invalidates() {
    let (mut first, _, _, _) =
        anchor_pending(BrokerAnchorModeV1::Advance, TranscriptFixture::new(140));
    let (_, _, second_signing, second_challenge) =
        anchor_pending(BrokerAnchorModeV1::Advance, TranscriptFixture::new(141));
    let stale = signed_observation(
        &second_challenge,
        AnchorPositionV1::Proposed,
        &second_signing,
    );
    assert_eq!(
        first.observe_anchor(&stale).unwrap_err().kind(),
        BrokerSessionErrorKindV1::AnchorProtocol
    );
    assert_eq!(first.stage, BrokerSessionStageV1::Invalidated);
}

#[test]
fn wrong_anchor_key_is_rejected_without_consuming_preparation() {
    let (mut core, _, signing, key) = completed_core(TranscriptFixture::new(150), 151, 152);
    core.prepare_anchor(
        AnchoredStateV1::from_local_state(5, HashChainHeadV1::from_bytes(digest(153))),
        &key,
    )
    .unwrap();
    let (_, wrong_key) = signing_key(154);
    assert_eq!(
        core.begin_anchor(BrokerAnchorModeV1::Advance, &wrong_key)
            .unwrap_err()
            .kind(),
        BrokerSessionErrorKindV1::AnchorKeyMismatch
    );
    assert_eq!(core.stage, BrokerSessionStageV1::AnchorPrepared);
    let challenge = core
        .begin_anchor(BrokerAnchorModeV1::Advance, &key)
        .unwrap();
    let wire = signed_observation(&challenge, AnchorPositionV1::Proposed, &signing);
    assert_eq!(
        core.observe_anchor(&wire).unwrap().stage(),
        BrokerSessionStageV1::AnchorCommitted
    );
}

#[test]
fn anchor_sequence_overflow_rejects_without_advancing_stage() {
    let (mut core, _, _, key) = completed_core(TranscriptFixture::new(160), 161, 162);
    assert_eq!(
        core.prepare_anchor(
            AnchoredStateV1::from_local_state(u64::MAX, HashChainHeadV1::from_bytes(digest(163)),),
            &key,
        )
        .unwrap_err()
        .kind(),
        BrokerSessionErrorKindV1::AnchorProtocol
    );
    assert_eq!(core.stage, BrokerSessionStageV1::Completed);
}

struct DropToken(Rc<Cell<usize>>);

impl Drop for DropToken {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

#[test]
fn client_and_output_tokens_remain_owned_until_machine_drop() {
    let transcript = TranscriptFixture::new(170).completed();
    let drops = Rc::new(Cell::new(0));
    {
        let mut core = SessionCoreV1::new();
        let mut permit = core
            .reserve(
                DropToken(Rc::clone(&drops)),
                reservation(&transcript, 171, 172),
                true,
            )
            .unwrap();
        core.validate_link_start(
            &permit,
            transcript.plan_identity(),
            transcript.closure_identity(),
        )
        .unwrap();
        let reservation_digest = core.reservation_digest().unwrap();
        permit.consume_for(reservation_digest).unwrap();
        core.commit_link_start(reservation_digest, digest(173));
        let completion = CompletionBindingV1 {
            broker_reservation: Some(reservation_digest),
            request_nonce_sha256: digest(173),
            ..completion(&transcript)
        };
        core.complete(DropToken(Rc::clone(&drops)), completion)
            .unwrap();
        assert_eq!(drops.get(), 0);
    }
    assert_eq!(drops.get(), 2);
}

#[test]
fn challenge_observation_is_exact_width_and_redacted_in_debug() {
    let (_, _, _, challenge) =
        anchor_pending(BrokerAnchorModeV1::Advance, TranscriptFixture::new(180));
    assert_eq!(challenge.as_bytes().len(), ANCHOR_CHALLENGE_WIRE_LEN_V1);
    let rendered = format!("{challenge:?}");
    assert!(rendered.contains("authority"));
    assert!(rendered.contains("length"));
    assert!(!rendered.contains(&format!("{:02x}", challenge.as_bytes()[32])));
}
