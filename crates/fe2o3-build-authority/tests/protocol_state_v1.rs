use fe2o3_build_authority::{
    AcceptV1, AttestV1, ChallengeV1, CompilerClosureV1, DenyReasonV1, DenyV1, FrameKindV1, GrantV1,
    PipelineAllowlistV1, PipelineV1, PolicyV1, ProtocolFrameV1, ProtocolPhaseV1,
    ProtocolStateErrorV1, ProtocolStateV1, TranscriptFieldV1,
};

fn digest(seed: u8) -> [u8; 32] {
    let mut value = [0_u8; 32];
    for (index, byte) in value.iter_mut().enumerate() {
        *byte = seed.wrapping_mul(31).wrapping_add(index as u8 + 1);
    }
    value
}

fn compiler(seed: u8) -> CompilerClosureV1 {
    CompilerClosureV1::new(
        digest(seed),
        digest(seed + 1),
        digest(seed + 2),
        digest(seed + 3),
    )
    .unwrap()
}

fn policy(seed: u8, pipeline: PipelineV1) -> PolicyV1 {
    PolicyV1::new(
        100 + u64::from(seed),
        digest(seed + 4),
        digest(seed + 5),
        compiler(seed + 6),
        PipelineAllowlistV1::ALL,
        pipeline,
        digest(seed + 10),
    )
    .unwrap()
}

#[derive(Clone, Copy)]
struct Transcript {
    policy: PolicyV1,
    challenge: ChallengeV1,
    attest: AttestV1,
    grant: GrantV1,
    accept: AcceptV1,
}

fn transcript(seed: u8, pipeline: PipelineV1) -> Transcript {
    let policy = policy(seed, pipeline);
    let nonce = digest(seed + 20);
    let challenge = ChallengeV1::for_policy(nonce, policy).unwrap();
    let attest = AttestV1::for_policy(nonce, policy).unwrap();
    let grant = GrantV1::for_attestation(attest, digest(seed + 21)).unwrap();
    let accept = AcceptV1::for_grant(grant);
    Transcript {
        policy,
        challenge,
        attest,
        grant,
        accept,
    }
}

fn advance_to_attest(state: &mut ProtocolStateV1, transcript: Transcript) {
    state
        .advance(ProtocolFrameV1::Challenge(transcript.challenge))
        .unwrap();
}

fn advance_to_decision(state: &mut ProtocolStateV1, transcript: Transcript) {
    advance_to_attest(state, transcript);
    state
        .advance(ProtocolFrameV1::Attest(transcript.attest))
        .unwrap();
}

#[test]
fn matching_grant_transcript_reaches_complete_without_publication_authority() {
    for pipeline in [
        PipelineV1::CollectedRowSoftmax,
        PipelineV1::CollectedTiledGemm,
    ] {
        let transcript = transcript(1, pipeline);
        let mut state = ProtocolStateV1::new(transcript.policy);
        assert_eq!(state.phase(), ProtocolPhaseV1::AwaitChallenge);
        assert_eq!(state.accepted_admission_identity(), None);

        state
            .advance(ProtocolFrameV1::Challenge(transcript.challenge))
            .unwrap();
        assert_eq!(state.phase(), ProtocolPhaseV1::AwaitAttest);
        state
            .advance(ProtocolFrameV1::Attest(transcript.attest))
            .unwrap();
        assert_eq!(state.phase(), ProtocolPhaseV1::AwaitDecision);
        state
            .advance(ProtocolFrameV1::Grant(transcript.grant))
            .unwrap();
        assert_eq!(state.phase(), ProtocolPhaseV1::AwaitAccept);
        assert_eq!(state.accepted_admission_identity(), None);
        state
            .advance(ProtocolFrameV1::Accept(transcript.accept))
            .unwrap();
        assert_eq!(state.phase(), ProtocolPhaseV1::Complete);
        assert_eq!(
            state.accepted_admission_identity(),
            Some(transcript.grant.admission_identity())
        );
        assert_eq!(state.denial_reason(), None);
    }
}

#[test]
fn every_typed_denial_is_terminal_and_needs_no_grant() {
    let reasons = [
        DenyReasonV1::PolicyRejected,
        DenyReasonV1::ExecutableIdentityMismatch,
        DenyReasonV1::ArgumentVectorMismatch,
        DenyReasonV1::CompilerClosureMismatch,
        DenyReasonV1::TargetNotPermitted,
        DenyReasonV1::PipelineNotPermitted,
        DenyReasonV1::RightsNotPermitted,
        DenyReasonV1::ProtocolViolation,
        DenyReasonV1::InternalFailure,
    ];
    for reason in reasons {
        let transcript = transcript(2, PipelineV1::CollectedRowSoftmax);
        let mut state = ProtocolStateV1::new(transcript.policy);
        advance_to_decision(&mut state, transcript);
        state
            .advance(ProtocolFrameV1::Deny(DenyV1::for_attestation(
                transcript.attest,
                reason,
            )))
            .unwrap();
        assert_eq!(state.phase(), ProtocolPhaseV1::Denied);
        assert_eq!(state.denial_reason(), Some(reason));
        assert_eq!(state.accepted_admission_identity(), None);
        assert_eq!(
            state.advance(ProtocolFrameV1::Accept(transcript.accept)),
            Err(ProtocolStateErrorV1::TerminalState {
                phase: ProtocolPhaseV1::Denied,
            })
        );
    }
}

#[test]
fn out_of_order_frames_and_replays_leave_state_unchanged() {
    let transcript = transcript(3, PipelineV1::CollectedTiledGemm);
    let frames = [
        ProtocolFrameV1::Attest(transcript.attest),
        ProtocolFrameV1::Grant(transcript.grant),
        ProtocolFrameV1::Deny(DenyV1::for_attestation(
            transcript.attest,
            DenyReasonV1::ProtocolViolation,
        )),
        ProtocolFrameV1::Accept(transcript.accept),
    ];
    for frame in frames {
        let mut state = ProtocolStateV1::new(transcript.policy);
        let before = state;
        assert_eq!(
            state.advance(frame),
            Err(ProtocolStateErrorV1::UnexpectedFrame {
                phase: ProtocolPhaseV1::AwaitChallenge,
                actual: frame.kind(),
            })
        );
        assert_eq!(state, before);
    }

    let mut state = ProtocolStateV1::new(transcript.policy);
    state
        .advance(ProtocolFrameV1::Challenge(transcript.challenge))
        .unwrap();
    let before = state;
    assert_eq!(
        state.advance(ProtocolFrameV1::Challenge(transcript.challenge)),
        Err(ProtocolStateErrorV1::UnexpectedFrame {
            phase: ProtocolPhaseV1::AwaitAttest,
            actual: FrameKindV1::Challenge,
        })
    );
    assert_eq!(state, before);

    state
        .advance(ProtocolFrameV1::Attest(transcript.attest))
        .unwrap();
    let before = state;
    assert_eq!(
        state.advance(ProtocolFrameV1::Attest(transcript.attest)),
        Err(ProtocolStateErrorV1::UnexpectedFrame {
            phase: ProtocolPhaseV1::AwaitDecision,
            actual: FrameKindV1::Attest,
        })
    );
    assert_eq!(state, before);

    state
        .advance(ProtocolFrameV1::Grant(transcript.grant))
        .unwrap();
    let before = state;
    assert_eq!(
        state.advance(ProtocolFrameV1::Grant(transcript.grant)),
        Err(ProtocolStateErrorV1::UnexpectedFrame {
            phase: ProtocolPhaseV1::AwaitAccept,
            actual: FrameKindV1::Grant,
        })
    );
    assert_eq!(state, before);

    state
        .advance(ProtocolFrameV1::Accept(transcript.accept))
        .unwrap();
    assert_eq!(
        state.advance(ProtocolFrameV1::Accept(transcript.accept)),
        Err(ProtocolStateErrorV1::TerminalState {
            phase: ProtocolPhaseV1::Complete,
        })
    );
}

#[test]
fn challenge_must_match_every_policy_binding() {
    let good = transcript(4, PipelineV1::CollectedRowSoftmax);
    let other = policy(40, PipelineV1::CollectedTiledGemm);
    let cases = [
        (
            ChallengeV1::new(
                good.challenge.nonce(),
                other.identity_sha256(),
                good.challenge.launcher_executable_identity(),
                good.challenge.cargo_fe2o3_executable_identity(),
                good.challenge.child_argv_identity(),
                good.challenge.policy_serial(),
            )
            .unwrap(),
            TranscriptFieldV1::PolicyIdentity,
        ),
        (
            ChallengeV1::new(
                good.challenge.nonce(),
                good.challenge.policy_identity(),
                other.launcher_executable_sha256(),
                good.challenge.cargo_fe2o3_executable_identity(),
                good.challenge.child_argv_identity(),
                good.challenge.policy_serial(),
            )
            .unwrap(),
            TranscriptFieldV1::LauncherExecutableIdentity,
        ),
        (
            ChallengeV1::new(
                good.challenge.nonce(),
                good.challenge.policy_identity(),
                good.challenge.launcher_executable_identity(),
                other.cargo_fe2o3_executable_sha256(),
                good.challenge.child_argv_identity(),
                good.challenge.policy_serial(),
            )
            .unwrap(),
            TranscriptFieldV1::CargoFe2o3ExecutableIdentity,
        ),
        (
            ChallengeV1::new(
                good.challenge.nonce(),
                good.challenge.policy_identity(),
                good.challenge.launcher_executable_identity(),
                good.challenge.cargo_fe2o3_executable_identity(),
                other.child_argv_sha256(),
                good.challenge.policy_serial(),
            )
            .unwrap(),
            TranscriptFieldV1::ChildArgumentVectorIdentity,
        ),
        (
            ChallengeV1::new(
                good.challenge.nonce(),
                good.challenge.policy_identity(),
                good.challenge.launcher_executable_identity(),
                good.challenge.cargo_fe2o3_executable_identity(),
                good.challenge.child_argv_identity(),
                other.serial(),
            )
            .unwrap(),
            TranscriptFieldV1::PolicySerial,
        ),
    ];
    for (challenge, field) in cases {
        let mut state = ProtocolStateV1::new(good.policy);
        let before = state;
        assert_eq!(
            state.advance(ProtocolFrameV1::Challenge(challenge)),
            Err(ProtocolStateErrorV1::TranscriptMismatch { field })
        );
        assert_eq!(state, before);
    }
}

#[test]
fn attest_must_match_challenge_policy_compiler_and_pipeline() {
    let good = transcript(5, PipelineV1::CollectedRowSoftmax);
    let other = transcript(50, PipelineV1::CollectedTiledGemm);
    let cases = [
        (
            AttestV1::new(
                other.attest.nonce(),
                good.attest.policy_identity(),
                good.attest.launcher_executable_identity(),
                good.attest.cargo_fe2o3_executable_identity(),
                good.attest.child_argv_identity(),
                good.attest.compiler_closure(),
                good.attest.pipeline(),
            )
            .unwrap(),
            TranscriptFieldV1::Nonce,
        ),
        (
            AttestV1::new(
                good.attest.nonce(),
                other.attest.policy_identity(),
                good.attest.launcher_executable_identity(),
                good.attest.cargo_fe2o3_executable_identity(),
                good.attest.child_argv_identity(),
                good.attest.compiler_closure(),
                good.attest.pipeline(),
            )
            .unwrap(),
            TranscriptFieldV1::PolicyIdentity,
        ),
        (
            AttestV1::new(
                good.attest.nonce(),
                good.attest.policy_identity(),
                other.attest.launcher_executable_identity(),
                good.attest.cargo_fe2o3_executable_identity(),
                good.attest.child_argv_identity(),
                good.attest.compiler_closure(),
                good.attest.pipeline(),
            )
            .unwrap(),
            TranscriptFieldV1::LauncherExecutableIdentity,
        ),
        (
            AttestV1::new(
                good.attest.nonce(),
                good.attest.policy_identity(),
                good.attest.launcher_executable_identity(),
                other.attest.cargo_fe2o3_executable_identity(),
                good.attest.child_argv_identity(),
                good.attest.compiler_closure(),
                good.attest.pipeline(),
            )
            .unwrap(),
            TranscriptFieldV1::CargoFe2o3ExecutableIdentity,
        ),
        (
            AttestV1::new(
                good.attest.nonce(),
                good.attest.policy_identity(),
                good.attest.launcher_executable_identity(),
                good.attest.cargo_fe2o3_executable_identity(),
                other.attest.child_argv_identity(),
                good.attest.compiler_closure(),
                good.attest.pipeline(),
            )
            .unwrap(),
            TranscriptFieldV1::ChildArgumentVectorIdentity,
        ),
        (
            AttestV1::new(
                good.attest.nonce(),
                good.attest.policy_identity(),
                good.attest.launcher_executable_identity(),
                good.attest.cargo_fe2o3_executable_identity(),
                good.attest.child_argv_identity(),
                other.attest.compiler_closure(),
                good.attest.pipeline(),
            )
            .unwrap(),
            TranscriptFieldV1::CompilerClosure,
        ),
        (
            AttestV1::new(
                good.attest.nonce(),
                good.attest.policy_identity(),
                good.attest.launcher_executable_identity(),
                good.attest.cargo_fe2o3_executable_identity(),
                good.attest.child_argv_identity(),
                good.attest.compiler_closure(),
                PipelineV1::CollectedTiledGemm,
            )
            .unwrap(),
            TranscriptFieldV1::Pipeline,
        ),
    ];
    for (attest, field) in cases {
        let mut state = ProtocolStateV1::new(good.policy);
        advance_to_attest(&mut state, good);
        let before = state;
        assert_eq!(
            state.advance(ProtocolFrameV1::Attest(attest)),
            Err(ProtocolStateErrorV1::TranscriptMismatch { field })
        );
        assert_eq!(state, before);
    }
}

#[test]
fn decision_and_accept_must_match_the_live_attestation() {
    let good = transcript(6, PipelineV1::CollectedRowSoftmax);
    let different_nonce = transcript(60, PipelineV1::CollectedRowSoftmax);
    let different_pipeline = transcript(61, PipelineV1::CollectedTiledGemm);
    let cases = [
        (different_nonce.grant, TranscriptFieldV1::Nonce),
        (different_pipeline.grant, TranscriptFieldV1::Nonce),
    ];
    for (grant, field) in cases {
        let mut state = ProtocolStateV1::new(good.policy);
        advance_to_decision(&mut state, good);
        let before = state;
        assert_eq!(
            state.advance(ProtocolFrameV1::Grant(grant)),
            Err(ProtocolStateErrorV1::TranscriptMismatch { field })
        );
        assert_eq!(state, before);
    }

    let mismatched_attest = AttestV1::new(
        good.attest.nonce(),
        good.attest.policy_identity(),
        good.attest.launcher_executable_identity(),
        good.attest.cargo_fe2o3_executable_identity(),
        good.attest.child_argv_identity(),
        good.attest.compiler_closure(),
        PipelineV1::CollectedTiledGemm,
    )
    .unwrap();
    let wrong_commitment_grant = GrantV1::for_attestation(mismatched_attest, digest(90)).unwrap();
    let mut state = ProtocolStateV1::new(good.policy);
    advance_to_decision(&mut state, good);
    assert_eq!(
        state.advance(ProtocolFrameV1::Grant(wrong_commitment_grant)),
        Err(ProtocolStateErrorV1::TranscriptMismatch {
            field: TranscriptFieldV1::AttestationIdentity,
        })
    );

    let mut state = ProtocolStateV1::new(good.policy);
    advance_to_decision(&mut state, good);
    state.advance(ProtocolFrameV1::Grant(good.grant)).unwrap();
    let wrong_accept = AcceptV1::for_grant(different_nonce.grant);
    let before = state;
    assert_eq!(
        state.advance(ProtocolFrameV1::Accept(wrong_accept)),
        Err(ProtocolStateErrorV1::TranscriptMismatch {
            field: TranscriptFieldV1::AdmissionIdentity,
        })
    );
    assert_eq!(state, before);
}

#[test]
fn deny_context_mismatches_fail_without_becoming_terminal() {
    let good = transcript(7, PipelineV1::CollectedTiledGemm);
    let other = transcript(70, PipelineV1::CollectedTiledGemm);
    let mut state = ProtocolStateV1::new(good.policy);
    advance_to_decision(&mut state, good);
    let before = state;
    assert_eq!(
        state.advance(ProtocolFrameV1::Deny(DenyV1::for_attestation(
            other.attest,
            DenyReasonV1::PolicyRejected,
        ))),
        Err(ProtocolStateErrorV1::TranscriptMismatch {
            field: TranscriptFieldV1::Nonce,
        })
    );
    assert_eq!(state, before);
    assert_eq!(state.phase(), ProtocolPhaseV1::AwaitDecision);
}
