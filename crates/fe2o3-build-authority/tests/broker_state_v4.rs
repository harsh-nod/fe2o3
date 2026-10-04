use fe2o3_build_authority::{
    BROKER_V4_AUTHORITY, BrokerAuthorityV4, BrokerReplayRegistryV4, BrokerSessionClaimV4,
    BrokerStateErrorV4, BrokerTranscriptFieldV4, BrokerTranscriptValidatorV4, CapabilityBindingV4,
    CompletedBrokerTranscriptV4, GrantedHostLinkTranscriptV4, HOST_LINK_OUTPUT_MODE_V4,
    HostLinkCommitV4, HostLinkGrantV4, HostLinkPrepareV4, PreparedHostLinkTranscriptV4,
    ProcessIdentityV4,
};

fn digest(seed: u8) -> [u8; 32] {
    let mut value = [0_u8; 32];
    for (index, byte) in value.iter_mut().enumerate() {
        *byte = seed.wrapping_mul(47).wrapping_add(index as u8 + 1);
    }
    value
}

#[derive(Clone, Copy)]
struct Transcript {
    binding: CapabilityBindingV4,
    process: ProcessIdentityV4,
    request: [u8; 32],
    plan: [u8; 32],
    closure: [u8; 32],
    grant: [u8; 32],
    output: [u8; 32],
    durable: [u8; 32],
}

impl Transcript {
    fn new(seed: u8) -> Self {
        Self {
            binding: CapabilityBindingV4::new(digest(1), digest(2), digest(3)).unwrap(),
            process: ProcessIdentityV4::new(4_000, 9_000_000).unwrap(),
            request: digest(seed),
            plan: digest(seed + 1),
            closure: digest(seed + 2),
            grant: digest(seed + 3),
            output: digest(seed + 4),
            durable: digest(seed + 5),
        }
    }

    fn state(self) -> BrokerTranscriptValidatorV4 {
        BrokerTranscriptValidatorV4::new(self.binding, self.process)
    }

    fn prepare(self) -> HostLinkPrepareV4 {
        HostLinkPrepareV4::new(
            self.process,
            self.binding.identity_sha256(),
            self.request,
            self.plan,
            self.closure,
        )
        .unwrap()
    }

    fn grant(self) -> HostLinkGrantV4 {
        HostLinkGrantV4::new(
            self.process,
            self.binding.identity_sha256(),
            self.request,
            self.plan,
            self.closure,
            self.grant,
        )
        .unwrap()
    }

    fn commit(self) -> HostLinkCommitV4 {
        self.commit_with(
            self.process,
            self.binding.identity_sha256(),
            self.request,
            self.plan,
            self.closure,
            self.grant,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn commit_with(
        self,
        process: ProcessIdentityV4,
        binding: [u8; 32],
        request: [u8; 32],
        plan: [u8; 32],
        closure: [u8; 32],
        grant: [u8; 32],
    ) -> HostLinkCommitV4 {
        HostLinkCommitV4::new(
            process,
            binding,
            request,
            plan,
            closure,
            grant,
            self.output,
            85_597_472,
            HOST_LINK_OUTPUT_MODE_V4,
            self.durable,
        )
        .unwrap()
    }
}

fn prepared(transcript: Transcript) -> PreparedHostLinkTranscriptV4 {
    transcript
        .state()
        .validate_prepare(transcript.prepare())
        .unwrap()
}

fn validated_grant(transcript: Transcript) -> GrantedHostLinkTranscriptV4 {
    prepared(transcript)
        .validate_grant(transcript.grant())
        .unwrap()
}

fn completed(transcript: Transcript) -> CompletedBrokerTranscriptV4 {
    validated_grant(transcript)
        .validate_commit(transcript.commit())
        .unwrap()
}

fn mismatch(field: BrokerTranscriptFieldV4) -> BrokerStateErrorV4 {
    BrokerStateErrorV4::TranscriptMismatch { field }
}

#[test]
fn transcript_validation_consumes_prepare_grant_and_commit_in_order() {
    let transcript = Transcript::new(10);
    let prepared = transcript
        .state()
        .validate_prepare(transcript.prepare())
        .unwrap();
    assert_eq!(prepared.request_identity(), transcript.request);
    assert_eq!(prepared.plan_identity(), transcript.plan);
    assert_eq!(prepared.closure_identity(), transcript.closure);
    assert_eq!(prepared.expected_binding(), transcript.binding);
    assert_eq!(prepared.expected_process(), transcript.process);

    let validated_grant = prepared.validate_grant(transcript.grant()).unwrap();
    assert_eq!(validated_grant.grant_identity(), transcript.grant);
    assert_eq!(validated_grant.authority(), BrokerAuthorityV4::None);
    let complete = validated_grant
        .validate_commit(transcript.commit())
        .unwrap();
    assert_eq!(
        complete.binding_identity(),
        transcript.binding.identity_sha256()
    );
    assert_eq!(complete.process(), transcript.process);
    assert_eq!(complete.request_identity(), transcript.request);
    assert_eq!(complete.plan_identity(), transcript.plan);
    assert_eq!(complete.closure_identity(), transcript.closure);
    assert_eq!(complete.grant_identity(), transcript.grant);
    assert_eq!(complete.output_sha256(), transcript.output);
    assert_eq!(complete.output_length(), 85_597_472);
    assert_eq!(complete.output_mode(), HOST_LINK_OUTPUT_MODE_V4);
    assert_eq!(complete.durable_plan_identity(), transcript.durable);
    assert_eq!(complete.authority(), BrokerAuthorityV4::None);
    assert_eq!(complete.authority(), BROKER_V4_AUTHORITY);
}

#[test]
fn rejected_prepare_returns_the_original_move_only_validator() {
    let transcript = Transcript::new(20);
    let wrong_process = ProcessIdentityV4::new(
        transcript.process.pid() + 1,
        transcript.process.start_time_ticks(),
    )
    .unwrap();
    let wrong = HostLinkPrepareV4::new(
        wrong_process,
        transcript.binding.identity_sha256(),
        transcript.request,
        transcript.plan,
        transcript.closure,
    )
    .unwrap();
    let rejected = transcript.state().validate_prepare(wrong).unwrap_err();
    assert_eq!(
        rejected.error(),
        mismatch(BrokerTranscriptFieldV4::ProcessIdentity)
    );
    let (state, error) = (*rejected).into_parts();
    assert_eq!(error, mismatch(BrokerTranscriptFieldV4::ProcessIdentity));
    assert_eq!(state.expected_binding(), transcript.binding);
    assert_eq!(state.expected_process(), transcript.process);
    assert!(state.validate_prepare(transcript.prepare()).is_ok());

    let wrong = HostLinkPrepareV4::new(
        transcript.process,
        digest(90),
        transcript.request,
        transcript.plan,
        transcript.closure,
    )
    .unwrap();
    let rejected = transcript.state().validate_prepare(wrong).unwrap_err();
    assert_eq!(
        rejected.error(),
        mismatch(BrokerTranscriptFieldV4::CapabilityBindingIdentity)
    );
}

#[test]
fn grant_substitution_rejection_preserves_the_prepared_validator() {
    let transcript = Transcript::new(30);
    let binding_identity = transcript.binding.identity_sha256();
    let cases = [
        (
            HostLinkGrantV4::new(
                ProcessIdentityV4::new(
                    transcript.process.pid(),
                    transcript.process.start_time_ticks() + 1,
                )
                .unwrap(),
                binding_identity,
                transcript.request,
                transcript.plan,
                transcript.closure,
                transcript.grant,
            )
            .unwrap(),
            BrokerTranscriptFieldV4::ProcessIdentity,
        ),
        (
            HostLinkGrantV4::new(
                transcript.process,
                digest(91),
                transcript.request,
                transcript.plan,
                transcript.closure,
                transcript.grant,
            )
            .unwrap(),
            BrokerTranscriptFieldV4::CapabilityBindingIdentity,
        ),
        (
            HostLinkGrantV4::new(
                transcript.process,
                binding_identity,
                digest(92),
                transcript.plan,
                transcript.closure,
                transcript.grant,
            )
            .unwrap(),
            BrokerTranscriptFieldV4::HostLinkRequestIdentity,
        ),
        (
            HostLinkGrantV4::new(
                transcript.process,
                binding_identity,
                transcript.request,
                digest(93),
                transcript.closure,
                transcript.grant,
            )
            .unwrap(),
            BrokerTranscriptFieldV4::HostLinkPlanIdentity,
        ),
        (
            HostLinkGrantV4::new(
                transcript.process,
                binding_identity,
                transcript.request,
                transcript.plan,
                digest(94),
                transcript.grant,
            )
            .unwrap(),
            BrokerTranscriptFieldV4::HostLinkClosureIdentity,
        ),
    ];
    for (wrong, field) in cases {
        let rejected = prepared(transcript).validate_grant(wrong).unwrap_err();
        assert_eq!(rejected.error(), mismatch(field));
        let (state, error) = (*rejected).into_parts();
        assert_eq!(error, mismatch(field));
        assert_eq!(state.request_identity(), transcript.request);
        assert!(state.validate_grant(transcript.grant()).is_ok());
    }
}

#[test]
fn commit_substitution_rejection_preserves_the_granted_validator() {
    let transcript = Transcript::new(40);
    let binding_identity = transcript.binding.identity_sha256();
    let make = |process, binding, request, plan, closure, grant| {
        transcript.commit_with(process, binding, request, plan, closure, grant)
    };
    let cases = [
        (
            make(
                ProcessIdentityV4::new(
                    transcript.process.pid() + 1,
                    transcript.process.start_time_ticks(),
                )
                .unwrap(),
                binding_identity,
                transcript.request,
                transcript.plan,
                transcript.closure,
                transcript.grant,
            ),
            BrokerTranscriptFieldV4::ProcessIdentity,
        ),
        (
            make(
                transcript.process,
                digest(95),
                transcript.request,
                transcript.plan,
                transcript.closure,
                transcript.grant,
            ),
            BrokerTranscriptFieldV4::CapabilityBindingIdentity,
        ),
        (
            make(
                transcript.process,
                binding_identity,
                digest(96),
                transcript.plan,
                transcript.closure,
                transcript.grant,
            ),
            BrokerTranscriptFieldV4::HostLinkRequestIdentity,
        ),
        (
            make(
                transcript.process,
                binding_identity,
                transcript.request,
                digest(97),
                transcript.closure,
                transcript.grant,
            ),
            BrokerTranscriptFieldV4::HostLinkPlanIdentity,
        ),
        (
            make(
                transcript.process,
                binding_identity,
                transcript.request,
                transcript.plan,
                digest(98),
                transcript.grant,
            ),
            BrokerTranscriptFieldV4::HostLinkClosureIdentity,
        ),
        (
            make(
                transcript.process,
                binding_identity,
                transcript.request,
                transcript.plan,
                transcript.closure,
                digest(99),
            ),
            BrokerTranscriptFieldV4::HostLinkGrantIdentity,
        ),
    ];
    for (wrong, field) in cases {
        let rejected = validated_grant(transcript)
            .validate_commit(wrong)
            .unwrap_err();
        assert_eq!(rejected.error(), mismatch(field));
        let (validated_grant, error) = (*rejected).into_parts();
        assert_eq!(error, mismatch(field));
        assert_eq!(validated_grant.grant_identity(), transcript.grant);
        assert!(validated_grant.validate_commit(transcript.commit()).is_ok());
    }
}

#[test]
fn cross_transcript_grant_and_commit_substitutions_fail_continuity() {
    let first = Transcript::new(50);
    let second = Transcript::new(70);

    let rejected = prepared(second).validate_grant(first.grant()).unwrap_err();
    assert_eq!(
        rejected.error(),
        mismatch(BrokerTranscriptFieldV4::HostLinkRequestIdentity)
    );
    let (second_prepared, _) = (*rejected).into_parts();
    let second_grant = second_prepared.validate_grant(second.grant()).unwrap();

    let rejected = second_grant.validate_commit(first.commit()).unwrap_err();
    assert_eq!(
        rejected.error(),
        mismatch(BrokerTranscriptFieldV4::HostLinkRequestIdentity)
    );
    let (second_grant, _) = (*rejected).into_parts();
    assert!(second_grant.validate_commit(second.commit()).is_ok());
}

#[test]
fn equivalent_validators_can_validate_the_same_transcript_and_grant_no_authority() {
    let transcript = Transcript::new(90);
    let first_validator = transcript.state();
    let second_validator = transcript.state();

    assert_eq!(first_validator, second_validator);
    assert_eq!(first_validator.authority(), BrokerAuthorityV4::None);
    assert_eq!(second_validator.authority(), BrokerAuthorityV4::None);

    let first = first_validator
        .validate_prepare(transcript.prepare())
        .unwrap()
        .validate_grant(transcript.grant())
        .unwrap()
        .validate_commit(transcript.commit())
        .unwrap();
    let second = second_validator
        .validate_prepare(transcript.prepare())
        .unwrap()
        .validate_grant(transcript.grant())
        .unwrap()
        .validate_commit(transcript.commit())
        .unwrap();

    assert_eq!(first, second);
    assert_eq!(first.session_claim(), second.session_claim());
    assert_eq!(first.authority(), BrokerAuthorityV4::None);
    assert_eq!(second.authority(), BrokerAuthorityV4::None);
    assert_eq!(transcript.binding.authority(), BrokerAuthorityV4::None);
    assert_eq!(transcript.grant().authority(), BrokerAuthorityV4::None);
}

#[derive(Debug, Eq, PartialEq)]
struct TestSessionCapability;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TestRegistryError {
    Denied,
}

struct DenyAllReplayRegistry;

impl BrokerReplayRegistryV4 for DenyAllReplayRegistry {
    type SessionCapability = TestSessionCapability;
    type Error = TestRegistryError;

    fn reserve_session(
        &mut self,
        _claim: BrokerSessionClaimV4,
    ) -> Result<Self::SessionCapability, Self::Error> {
        Err(TestRegistryError::Denied)
    }

    fn commit_session(
        &mut self,
        _capability: Self::SessionCapability,
        _transcript: &CompletedBrokerTranscriptV4,
    ) -> Result<(), Self::Error> {
        Err(TestRegistryError::Denied)
    }
}

#[test]
fn replay_registry_seam_supplies_no_default_or_allow_all_authority() {
    let transcript = Transcript::new(110);
    let claim = prepared(transcript).session_claim();
    assert_eq!(claim.authority(), BrokerAuthorityV4::None);
    assert_eq!(
        claim.binding_identity(),
        transcript.binding.identity_sha256()
    );
    assert_eq!(claim.process(), transcript.process);
    assert_eq!(claim.request_identity(), transcript.request);
    assert_eq!(claim.plan_identity(), transcript.plan);
    assert_eq!(claim.closure_identity(), transcript.closure);

    let mut registry = DenyAllReplayRegistry;
    assert_eq!(
        registry.reserve_session(claim),
        Err(TestRegistryError::Denied)
    );
    assert_eq!(
        registry.commit_session(TestSessionCapability, &completed(transcript)),
        Err(TestRegistryError::Denied)
    );
}

#[test]
fn deterministic_transcripts_complete_with_distinct_terminal_evidence() {
    let mut previous = None;
    for seed in 1_u8..=64 {
        let transcript = Transcript::new(seed);
        let complete = completed(transcript);
        let identity_tuple = (
            complete.request_identity(),
            complete.plan_identity(),
            complete.closure_identity(),
            complete.grant_identity(),
            complete.output_sha256(),
            complete.durable_plan_identity(),
        );
        assert_ne!(previous, Some(identity_tuple));
        previous = Some(identity_tuple);
    }
}
