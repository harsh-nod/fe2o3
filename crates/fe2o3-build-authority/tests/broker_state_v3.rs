use fe2o3_build_authority::{
    BootstrapV3, BrokerDescriptorManifestV3, BrokerFrameV3, BrokerPhaseV3, BrokerStateErrorV3,
    BrokerStateV3, BrokerTranscriptFieldV3, CapabilitiesV3, CapabilityBindingV3, ConsumeV3,
    HelloV3, PipelineV1, PostExecV3, PrepareV3, ProcessIdentityV3, PublicationRightsV1,
    decode_broker_frame_v3,
};

fn digest(seed: u8) -> [u8; 32] {
    let mut value = [0_u8; 32];
    for (index, byte) in value.iter_mut().enumerate() {
        *byte = seed.wrapping_mul(41).wrapping_add(index as u8 + 1);
    }
    value
}

fn binding(seed: u8, pipeline: PipelineV1, worker: bool) -> CapabilityBindingV3 {
    CapabilityBindingV3::new(
        digest(seed),
        digest(seed + 1),
        digest(seed + 2),
        pipeline,
        digest(seed + 3),
        digest(seed + 4),
        digest(seed + 5),
        digest(seed + 6),
        digest(seed + 7),
        digest(seed + 8),
        worker.then(|| digest(seed + 9)),
    )
    .unwrap()
}

#[derive(Clone, Copy)]
struct Transcript {
    binding: CapabilityBindingV3,
    process: ProcessIdentityV3,
    bootstrap_identity: [u8; 32],
    capability_identity: [u8; 32],
    frames: [BrokerFrameV3; 6],
}

fn transcript(seed: u8, pipeline: PipelineV1, worker: bool) -> Transcript {
    let binding = binding(seed, pipeline, worker);
    let process =
        ProcessIdentityV3::new(4_000 + u32::from(seed), 9_000_000 + u64::from(seed)).unwrap();
    let bootstrap_identity = digest(seed + 10);
    let capability_identity = digest(seed + 11);
    let binding_identity = binding.identity_sha256();
    let frames = [
        BrokerFrameV3::Hello(HelloV3::for_binding(process, binding)),
        BrokerFrameV3::Bootstrap(
            BootstrapV3::new(
                process,
                binding_identity,
                bootstrap_identity,
                BrokerDescriptorManifestV3::Bootstrap,
            )
            .unwrap(),
        ),
        BrokerFrameV3::PostExec(
            PostExecV3::new(
                process,
                binding_identity,
                bootstrap_identity,
                binding.cargo_fe2o3_executable_identity(),
            )
            .unwrap(),
        ),
        BrokerFrameV3::Capabilities(
            CapabilitiesV3::new(
                process,
                binding_identity,
                bootstrap_identity,
                capability_identity,
                BrokerDescriptorManifestV3::CompilerCapabilities,
            )
            .unwrap(),
        ),
        BrokerFrameV3::Prepare(
            PrepareV3::new(
                process,
                binding_identity,
                capability_identity,
                binding.compiler_closure_identity(),
                binding.runtime_object_identity(),
                binding.codegen_backend_identity(),
            )
            .unwrap(),
        ),
        BrokerFrameV3::Consume(
            ConsumeV3::new(
                process,
                binding_identity,
                capability_identity,
                binding.compiler_closure_identity(),
                binding.runtime_object_identity(),
                binding.codegen_backend_identity(),
            )
            .unwrap(),
        ),
    ];
    Transcript {
        binding,
        process,
        bootstrap_identity,
        capability_identity,
        frames,
    }
}

fn state_before(transcript: Transcript, frame_index: usize) -> BrokerStateV3 {
    let mut state = BrokerStateV3::new(transcript.binding);
    for frame in &transcript.frames[..frame_index] {
        state.advance(*frame).unwrap();
    }
    state
}

fn assert_mismatch_without_mutation(
    state: BrokerStateV3,
    frame: BrokerFrameV3,
    field: BrokerTranscriptFieldV3,
) {
    let mut candidate = state;
    assert_eq!(
        candidate.advance(frame),
        Err(BrokerStateErrorV3::TranscriptMismatch { field })
    );
    assert_eq!(candidate, state);
}

#[test]
fn exact_two_exec_chain_completes_without_publication_authority() {
    for pipeline in [
        PipelineV1::CollectedRowSoftmax,
        PipelineV1::CollectedTiledGemm,
    ] {
        for worker in [false, true] {
            let transcript = transcript(1, pipeline, worker);
            let mut state = BrokerStateV3::new(transcript.binding);
            let phases = [
                BrokerPhaseV3::AwaitHello,
                BrokerPhaseV3::AwaitBootstrap,
                BrokerPhaseV3::AwaitPostExec,
                BrokerPhaseV3::AwaitCapabilities,
                BrokerPhaseV3::AwaitPrepare,
                BrokerPhaseV3::AwaitConsume,
            ];
            assert_eq!(state.process(), None);
            assert_eq!(state.completed_binding_identity(), None);
            for (frame, phase) in transcript.frames.into_iter().zip(phases) {
                assert_eq!(state.phase(), phase);
                state.advance(frame).unwrap();
            }
            assert_eq!(state.phase(), BrokerPhaseV3::Complete);
            assert_eq!(state.process(), Some(transcript.process));
            assert_eq!(
                state.completed_binding_identity(),
                Some(transcript.binding.identity_sha256())
            );
            assert_eq!(
                transcript.binding.publication_rights(),
                PublicationRightsV1::NONE
            );
        }
    }
}

#[test]
fn every_out_of_order_frame_and_replay_is_rejected_without_state_change() {
    let transcript = transcript(2, PipelineV1::CollectedTiledGemm, true);
    let phases = [
        BrokerPhaseV3::AwaitHello,
        BrokerPhaseV3::AwaitBootstrap,
        BrokerPhaseV3::AwaitPostExec,
        BrokerPhaseV3::AwaitCapabilities,
        BrokerPhaseV3::AwaitPrepare,
        BrokerPhaseV3::AwaitConsume,
    ];
    for (expected_index, phase) in phases.into_iter().enumerate() {
        let state = state_before(transcript, expected_index);
        assert_eq!(state.phase(), phase);
        for (actual_index, frame) in transcript.frames.into_iter().enumerate() {
            if actual_index == expected_index {
                continue;
            }
            let mut candidate = state;
            assert_eq!(
                candidate.advance(frame),
                Err(BrokerStateErrorV3::UnexpectedFrame {
                    phase,
                    actual: frame.kind(),
                })
            );
            assert_eq!(candidate, state);
        }
    }

    let complete = state_before(transcript, transcript.frames.len());
    for frame in transcript.frames {
        let mut candidate = complete;
        assert_eq!(
            candidate.advance(frame),
            Err(BrokerStateErrorV3::TerminalState)
        );
        assert_eq!(candidate, complete);
    }
}

#[test]
fn hello_binds_the_complete_expected_value_and_trampoline_image() {
    let current = transcript(3, PipelineV1::CollectedRowSoftmax, false);
    let state = BrokerStateV3::new(current.binding);
    let other = transcript(30, PipelineV1::CollectedRowSoftmax, false);
    assert_mismatch_without_mutation(
        state,
        BrokerFrameV3::Hello(HelloV3::for_binding(current.process, other.binding)),
        BrokerTranscriptFieldV3::CapabilityBinding,
    );
    assert_mismatch_without_mutation(
        state,
        BrokerFrameV3::Hello(HelloV3::new(current.process, current.binding, digest(90)).unwrap()),
        BrokerTranscriptFieldV3::TrampolineExecutableIdentity,
    );
}

#[test]
fn pid_start_time_binding_and_bootstrap_manifest_substitution_fail() {
    let transcript = transcript(4, PipelineV1::CollectedTiledGemm, false);
    let state = state_before(transcript, 1);
    let binding_identity = transcript.binding.identity_sha256();

    for process in [
        ProcessIdentityV3::new(
            transcript.process.pid() + 1,
            transcript.process.start_time_ticks(),
        )
        .unwrap(),
        ProcessIdentityV3::new(
            transcript.process.pid(),
            transcript.process.start_time_ticks() + 1,
        )
        .unwrap(),
    ] {
        assert_mismatch_without_mutation(
            state,
            BrokerFrameV3::Bootstrap(
                BootstrapV3::new(
                    process,
                    binding_identity,
                    transcript.bootstrap_identity,
                    BrokerDescriptorManifestV3::Bootstrap,
                )
                .unwrap(),
            ),
            BrokerTranscriptFieldV3::ProcessIdentity,
        );
    }
    assert_mismatch_without_mutation(
        state,
        BrokerFrameV3::Bootstrap(
            BootstrapV3::new(
                transcript.process,
                digest(91),
                transcript.bootstrap_identity,
                BrokerDescriptorManifestV3::Bootstrap,
            )
            .unwrap(),
        ),
        BrokerTranscriptFieldV3::CapabilityBindingIdentity,
    );
    assert_mismatch_without_mutation(
        state,
        BrokerFrameV3::Bootstrap(
            BootstrapV3::new(
                transcript.process,
                binding_identity,
                transcript.bootstrap_identity,
                BrokerDescriptorManifestV3::CompilerCapabilities,
            )
            .unwrap(),
        ),
        BrokerTranscriptFieldV3::BootstrapDescriptorManifest,
    );
}

#[test]
fn post_exec_rejects_process_bootstrap_and_wrapper_identity_substitution() {
    let transcript = transcript(5, PipelineV1::CollectedRowSoftmax, true);
    let state = state_before(transcript, 2);
    let binding_identity = transcript.binding.identity_sha256();
    let cases = [
        (
            PostExecV3::new(
                ProcessIdentityV3::new(
                    transcript.process.pid() + 1,
                    transcript.process.start_time_ticks(),
                )
                .unwrap(),
                binding_identity,
                transcript.bootstrap_identity,
                transcript.binding.cargo_fe2o3_executable_identity(),
            )
            .unwrap(),
            BrokerTranscriptFieldV3::ProcessIdentity,
        ),
        (
            PostExecV3::new(
                transcript.process,
                digest(92),
                transcript.bootstrap_identity,
                transcript.binding.cargo_fe2o3_executable_identity(),
            )
            .unwrap(),
            BrokerTranscriptFieldV3::CapabilityBindingIdentity,
        ),
        (
            PostExecV3::new(
                transcript.process,
                binding_identity,
                digest(93),
                transcript.binding.cargo_fe2o3_executable_identity(),
            )
            .unwrap(),
            BrokerTranscriptFieldV3::BootstrapIdentity,
        ),
        (
            PostExecV3::new(
                transcript.process,
                binding_identity,
                transcript.bootstrap_identity,
                digest(94),
            )
            .unwrap(),
            BrokerTranscriptFieldV3::CargoFe2o3ExecutableIdentity,
        ),
    ];
    for (value, field) in cases {
        assert_mismatch_without_mutation(state, BrokerFrameV3::PostExec(value), field);
    }
}

#[test]
fn capabilities_reject_bootstrap_replay_and_wrong_manifest() {
    let transcript = transcript(6, PipelineV1::CollectedTiledGemm, true);
    let state = state_before(transcript, 3);
    let binding_identity = transcript.binding.identity_sha256();
    let cases = [
        (
            CapabilitiesV3::new(
                transcript.process,
                binding_identity,
                digest(95),
                transcript.capability_identity,
                BrokerDescriptorManifestV3::CompilerCapabilities,
            )
            .unwrap(),
            BrokerTranscriptFieldV3::BootstrapIdentity,
        ),
        (
            CapabilitiesV3::new(
                transcript.process,
                binding_identity,
                transcript.bootstrap_identity,
                transcript.capability_identity,
                BrokerDescriptorManifestV3::Bootstrap,
            )
            .unwrap(),
            BrokerTranscriptFieldV3::CapabilitiesDescriptorManifest,
        ),
    ];
    for (value, field) in cases {
        assert_mismatch_without_mutation(state, BrokerFrameV3::Capabilities(value), field);
    }
}

fn compiler_observation_substitutions(
    transcript: Transcript,
    consume: bool,
) -> [(BrokerFrameV3, BrokerTranscriptFieldV3); 6] {
    let binding_identity = transcript.binding.identity_sha256();
    let make =
        |process, binding_identity, capability_identity, compiler_closure, runtime, backend| {
            if consume {
                BrokerFrameV3::Consume(
                    ConsumeV3::new(
                        process,
                        binding_identity,
                        capability_identity,
                        compiler_closure,
                        runtime,
                        backend,
                    )
                    .unwrap(),
                )
            } else {
                BrokerFrameV3::Prepare(
                    PrepareV3::new(
                        process,
                        binding_identity,
                        capability_identity,
                        compiler_closure,
                        runtime,
                        backend,
                    )
                    .unwrap(),
                )
            }
        };
    let closure = transcript.binding.compiler_closure_identity();
    let runtime = transcript.binding.runtime_object_identity();
    let backend = transcript.binding.codegen_backend_identity();
    [
        (
            make(
                ProcessIdentityV3::new(
                    transcript.process.pid(),
                    transcript.process.start_time_ticks() + 1,
                )
                .unwrap(),
                binding_identity,
                transcript.capability_identity,
                closure,
                runtime,
                backend,
            ),
            BrokerTranscriptFieldV3::ProcessIdentity,
        ),
        (
            make(
                transcript.process,
                digest(96),
                transcript.capability_identity,
                closure,
                runtime,
                backend,
            ),
            BrokerTranscriptFieldV3::CapabilityBindingIdentity,
        ),
        (
            make(
                transcript.process,
                binding_identity,
                digest(97),
                closure,
                runtime,
                backend,
            ),
            BrokerTranscriptFieldV3::CapabilityIdentity,
        ),
        (
            make(
                transcript.process,
                binding_identity,
                transcript.capability_identity,
                digest(98),
                runtime,
                backend,
            ),
            BrokerTranscriptFieldV3::CompilerClosureIdentity,
        ),
        (
            make(
                transcript.process,
                binding_identity,
                transcript.capability_identity,
                closure,
                digest(99),
                backend,
            ),
            BrokerTranscriptFieldV3::RuntimeObjectIdentity,
        ),
        (
            make(
                transcript.process,
                binding_identity,
                transcript.capability_identity,
                closure,
                runtime,
                digest(100),
            ),
            BrokerTranscriptFieldV3::CodegenBackendIdentity,
        ),
    ]
}

#[test]
fn prepare_and_consume_reject_every_bound_identity_substitution() {
    let transcript = transcript(7, PipelineV1::CollectedRowSoftmax, false);
    for (frame_index, consume) in [(4, false), (5, true)] {
        let state = state_before(transcript, frame_index);
        for (frame, field) in compiler_observation_substitutions(transcript, consume) {
            assert_mismatch_without_mutation(state, frame, field);
        }
    }
}

#[test]
fn cross_transcript_frames_cannot_be_replayed_into_an_active_chain() {
    let first = transcript(8, PipelineV1::CollectedTiledGemm, true);
    let second = transcript(40, PipelineV1::CollectedTiledGemm, true);
    for index in 0..first.frames.len() {
        let state = state_before(first, index);
        let mut candidate = state;
        let error = candidate.advance(second.frames[index]).unwrap_err();
        assert!(matches!(
            error,
            BrokerStateErrorV3::TranscriptMismatch { .. }
        ));
        assert_eq!(candidate, state);
    }
}

#[test]
fn established_transition_fields_reject_all_decodable_single_bit_mutations() {
    let transcript = transcript(9, PipelineV1::CollectedTiledGemm, true);
    let mut mutations = 0_usize;
    for frame_index in [2_usize, 4, 5] {
        let state = state_before(transcript, frame_index);
        let canonical_frame = transcript.frames[frame_index];
        let canonical = canonical_frame.encode();
        for byte_index in 0..canonical.len() {
            for bit in 0..8 {
                let mut mutated = canonical.clone();
                mutated[byte_index] ^= 1 << bit;
                if let Ok(decoded) = decode_broker_frame_v3(&mutated) {
                    let mut candidate = state;
                    assert!(candidate.advance(decoded).is_err());
                    assert_eq!(candidate, state);
                }
                mutations += 1;
            }
        }
    }
    assert_eq!(mutations, 4_288);
}
