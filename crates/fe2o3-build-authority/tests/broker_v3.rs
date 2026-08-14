use fe2o3_build_authority::{
    BOOTSTRAP_V3_PAYLOAD_LEN, BROKER_V3_BINDING_IDENTITY_DOMAIN, BROKER_V3_BINDING_WIRE_LEN,
    BROKER_V3_HEADER_LEN, BROKER_V3_MAGIC, BROKER_V3_VERSION, BootstrapV3, BrokerDescriptorKindV3,
    BrokerDescriptorManifestV3, BrokerFrameKindV3, BrokerFrameV3, BrokerIdentityFieldV3,
    BrokerProtocolErrorV3, BrokerTargetV3, CAPABILITIES_V3_PAYLOAD_LEN, CONSUME_V3_PAYLOAD_LEN,
    CapabilitiesV3, CapabilityBindingV3, ConsumeV3, DESCRIPTOR_MANIFEST_V3_WIRE_LEN,
    HELLO_V3_PAYLOAD_LEN, HelloV3, POST_EXEC_V3_PAYLOAD_LEN, PREPARE_V3_PAYLOAD_LEN,
    PROCESS_IDENTITY_V3_WIRE_LEN, PipelineV1, PostExecV3, PrepareV3, ProcessIdentityV3,
    PublicationRightsV1, decode_broker_frame_v3, decode_capability_binding_v3,
};

const HEADER: usize = BROKER_V3_HEADER_LEN;
const GOLDEN_BINDING_IDENTITY: [u8; 32] = [
    0x2a, 0x9c, 0xae, 0x79, 0x59, 0xe9, 0xef, 0xd2, 0x07, 0xde, 0x5f, 0x85, 0x9e, 0x10, 0x06, 0x88,
    0x47, 0x9f, 0x35, 0x94, 0x69, 0x84, 0x7e, 0xfe, 0xd8, 0x53, 0x33, 0x56, 0xd7, 0x14, 0xa5, 0x91,
];

fn digest(seed: u8) -> [u8; 32] {
    [seed; 32]
}

fn patterned_digest(seed: u8) -> [u8; 32] {
    let mut value = [0_u8; 32];
    for (index, byte) in value.iter_mut().enumerate() {
        *byte = seed.wrapping_mul(37).wrapping_add(index as u8 + 1);
    }
    value
}

fn binding(pipeline: PipelineV1, worker: Option<[u8; 32]>) -> CapabilityBindingV3 {
    CapabilityBindingV3::new(
        digest(1),
        digest(2),
        digest(3),
        pipeline,
        digest(4),
        digest(5),
        digest(6),
        digest(7),
        digest(8),
        digest(9),
        worker,
    )
    .unwrap()
}

fn process() -> ProcessIdentityV3 {
    ProcessIdentityV3::new(0x1122_3344, 0x0102_0304_0506_0708).unwrap()
}

fn frames(binding: CapabilityBindingV3) -> [BrokerFrameV3; 6] {
    let process = process();
    let binding_identity = binding.identity_sha256();
    let bootstrap_identity = digest(11);
    let capability_identity = digest(12);
    [
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
    ]
}

fn set_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn set_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

#[test]
fn wire_sizes_are_derived_from_fixed_fields() {
    assert_eq!(BROKER_V3_HEADER_LEN, 24);
    assert_eq!(PROCESS_IDENTITY_V3_WIRE_LEN, 16);
    assert_eq!(DESCRIPTOR_MANIFEST_V3_WIRE_LEN, 16);
    assert_eq!(BROKER_V3_BINDING_WIRE_LEN, 336);
    assert_eq!(HELLO_V3_PAYLOAD_LEN, 384);
    assert_eq!(BOOTSTRAP_V3_PAYLOAD_LEN, 96);
    assert_eq!(POST_EXEC_V3_PAYLOAD_LEN, 112);
    assert_eq!(CAPABILITIES_V3_PAYLOAD_LEN, 128);
    assert_eq!(PREPARE_V3_PAYLOAD_LEN, 176);
    assert_eq!(CONSUME_V3_PAYLOAD_LEN, 176);
}

#[test]
fn binding_layout_and_cross_implementation_golden_identity_are_stable() {
    let binding = binding(PipelineV1::CollectedTiledGemm, Some(digest(10)));
    let encoded = binding.encode();

    assert_eq!(&encoded[0..32], &digest(1));
    assert_eq!(&encoded[32..64], &digest(2));
    assert_eq!(&encoded[64..96], &digest(3));
    assert_eq!(u16::from_le_bytes(encoded[96..98].try_into().unwrap()), 1);
    assert_eq!(u16::from_le_bytes(encoded[98..100].try_into().unwrap()), 2);
    assert_eq!(&encoded[100..104], &[0; 4]);
    for (index, seed) in (4_u8..=9).enumerate() {
        let offset = 104 + (index * 32);
        assert_eq!(&encoded[offset..offset + 32], &digest(seed));
    }
    assert_eq!(encoded[296], 1);
    assert_eq!(&encoded[297..304], &[0; 7]);
    assert_eq!(&encoded[304..336], &digest(10));
    assert_eq!(binding.identity_sha256(), GOLDEN_BINDING_IDENTITY);
    assert_eq!(decode_capability_binding_v3(&encoded), Ok(binding));
    assert_eq!(binding.target(), BrokerTargetV3::Gfx942XnackMinus);
    assert_eq!(binding.pipeline(), PipelineV1::CollectedTiledGemm);
    assert_eq!(binding.publication_rights(), PublicationRightsV1::NONE);
    assert_eq!(
        BROKER_V3_BINDING_IDENTITY_DOMAIN,
        b"FE2O3/PROTECTED-AUTHORITY-BROKER-V3-BINDING\0"
    );
}

#[test]
fn all_frames_have_exact_headers_sequences_and_roundtrip() {
    let expected = [
        (BrokerFrameKindV3::Hello, HELLO_V3_PAYLOAD_LEN, 0_u32),
        (BrokerFrameKindV3::Bootstrap, BOOTSTRAP_V3_PAYLOAD_LEN, 1),
        (BrokerFrameKindV3::PostExec, POST_EXEC_V3_PAYLOAD_LEN, 2),
        (
            BrokerFrameKindV3::Capabilities,
            CAPABILITIES_V3_PAYLOAD_LEN,
            3,
        ),
        (BrokerFrameKindV3::Prepare, PREPARE_V3_PAYLOAD_LEN, 4),
        (BrokerFrameKindV3::Consume, CONSUME_V3_PAYLOAD_LEN, 5),
    ];
    for pipeline in [
        PipelineV1::CollectedRowSoftmax,
        PipelineV1::CollectedTiledGemm,
    ] {
        for worker in [None, Some(patterned_digest(40))] {
            for (frame, (kind, payload_len, sequence)) in
                frames(binding(pipeline, worker)).into_iter().zip(expected)
            {
                let encoded = frame.encode();
                assert_eq!(frame.kind(), kind);
                assert_eq!(frame.encoded_len(), HEADER + payload_len);
                assert_eq!(&encoded[0..8], &BROKER_V3_MAGIC);
                assert_eq!(
                    u16::from_le_bytes(encoded[8..10].try_into().unwrap()),
                    BROKER_V3_VERSION
                );
                assert_eq!(
                    u16::from_le_bytes(encoded[10..12].try_into().unwrap()),
                    kind as u16
                );
                assert_eq!(
                    u32::from_le_bytes(encoded[12..16].try_into().unwrap()),
                    payload_len as u32
                );
                assert_eq!(
                    u32::from_le_bytes(encoded[16..20].try_into().unwrap()),
                    sequence
                );
                assert_eq!(&encoded[20..24], &[0; 4]);
                assert_eq!(decode_broker_frame_v3(&encoded), Ok(frame));
                assert_eq!(decode_broker_frame_v3(&encoded).unwrap().encode(), encoded);
            }
        }
    }
}

#[test]
fn descriptor_manifests_are_exact_typed_counts_and_ordered_kinds() {
    let bootstrap = BrokerDescriptorManifestV3::Bootstrap;
    assert_eq!(bootstrap.descriptor_count(), 1);
    assert_eq!(
        bootstrap.descriptor_kind(0),
        Some(BrokerDescriptorKindV3::CargoFe2o3WrapperExecutable)
    );
    assert_eq!(bootstrap.descriptor_kind(1), None);

    let capabilities = BrokerDescriptorManifestV3::CompilerCapabilities;
    assert_eq!(capabilities.descriptor_count(), 5);
    assert_eq!(
        (0..5)
            .map(|index| capabilities.descriptor_kind(index).unwrap())
            .collect::<Vec<_>>(),
        vec![
            BrokerDescriptorKindV3::RustcExecutable,
            BrokerDescriptorKindV3::RustcRuntimeDirectory,
            BrokerDescriptorKindV3::CodegenBackend,
            BrokerDescriptorKindV3::ArtifactDirectory,
            BrokerDescriptorKindV3::CargoObservation,
        ]
    );
    assert_eq!(capabilities.descriptor_kind(5), None);
}

#[test]
fn optional_worker_encoding_is_canonical() {
    let absent = binding(PipelineV1::CollectedRowSoftmax, None);
    assert_eq!(absent.worker_v2_identity(), None);
    assert_eq!(&absent.encode()[296..336], &[0; 40]);
    assert_eq!(decode_capability_binding_v3(&absent.encode()), Ok(absent));

    let present = binding(PipelineV1::CollectedRowSoftmax, Some(patterned_digest(91)));
    assert_eq!(present.worker_v2_identity(), Some(patterned_digest(91)));
    assert_ne!(present.identity_sha256(), absent.identity_sha256());

    let mut invalid = absent.encode();
    invalid[304] = 1;
    assert_eq!(
        decode_capability_binding_v3(&invalid),
        Err(BrokerProtocolErrorV3::WorkerIdentityWithoutPresence)
    );

    let mut invalid = present.encode();
    invalid[296] = 2;
    assert_eq!(
        decode_capability_binding_v3(&invalid),
        Err(BrokerProtocolErrorV3::InvalidWorkerPresence { actual: 2 })
    );
}

#[test]
fn publication_authority_is_unrepresentable_and_rejected_on_wire() {
    let canonical = binding(PipelineV1::CollectedRowSoftmax, None);
    assert_eq!(canonical.publication_rights().bits(), 0);
    for rights in [1_u32, 2, u32::MAX] {
        let mut encoded = canonical.encode();
        set_u32(&mut encoded, 100, rights);
        assert_eq!(
            decode_capability_binding_v3(&encoded),
            Err(BrokerProtocolErrorV3::PublicationAuthorityForbidden { actual: rights })
        );
    }
}

#[test]
fn binding_rejects_unknown_reserved_zero_and_trailing_fields() {
    let canonical = binding(PipelineV1::CollectedTiledGemm, Some(digest(10))).encode();

    for length in [0, 1, 335] {
        assert_eq!(
            decode_capability_binding_v3(&canonical[..length]),
            Err(BrokerProtocolErrorV3::InvalidBindingLength { actual: length })
        );
    }
    let mut trailing = canonical.to_vec();
    trailing.push(0);
    assert_eq!(
        decode_capability_binding_v3(&trailing),
        Err(BrokerProtocolErrorV3::InvalidBindingLength { actual: 337 })
    );

    let mut invalid = canonical;
    set_u16(&mut invalid, 96, 2);
    assert_eq!(
        decode_capability_binding_v3(&invalid),
        Err(BrokerProtocolErrorV3::UnknownTarget { actual: 2 })
    );
    let mut invalid = canonical;
    set_u16(&mut invalid, 98, 3);
    assert_eq!(
        decode_capability_binding_v3(&invalid),
        Err(BrokerProtocolErrorV3::UnknownPipeline { actual: 3 })
    );
    for offset in 297..304 {
        let mut invalid = canonical;
        invalid[offset] = 1;
        assert_eq!(
            decode_capability_binding_v3(&invalid),
            Err(BrokerProtocolErrorV3::NonzeroBindingReserved)
        );
    }

    let identity_offsets = [0, 32, 64, 104, 136, 168, 200, 232, 264, 304];
    let identity_fields = [
        BrokerIdentityFieldV3::Policy,
        BrokerIdentityFieldV3::ProtectedAdmission,
        BrokerIdentityFieldV3::BuildSession,
        BrokerIdentityFieldV3::CargoEnvironment,
        BrokerIdentityFieldV3::TrampolineExecutable,
        BrokerIdentityFieldV3::CargoFe2o3Executable,
        BrokerIdentityFieldV3::CompilerClosure,
        BrokerIdentityFieldV3::RuntimeObject,
        BrokerIdentityFieldV3::CodegenBackend,
        BrokerIdentityFieldV3::WorkerV2,
    ];
    for (offset, field) in identity_offsets.into_iter().zip(identity_fields) {
        let mut invalid = canonical;
        invalid[offset..offset + 32].fill(0);
        assert_eq!(
            decode_capability_binding_v3(&invalid),
            Err(BrokerProtocolErrorV3::ZeroIdentity { field })
        );
    }
}

#[test]
fn headers_reject_unknown_flags_lengths_sequences_and_trailing_bytes() {
    let frame = frames(binding(PipelineV1::CollectedRowSoftmax, None))[2];
    let canonical = frame.encode();
    for length in [0, 1, 23] {
        assert_eq!(
            decode_broker_frame_v3(&canonical[..length]),
            Err(BrokerProtocolErrorV3::TruncatedHeader { actual: length })
        );
    }
    let mut invalid = canonical.clone();
    invalid[0] ^= 1;
    assert_eq!(
        decode_broker_frame_v3(&invalid),
        Err(BrokerProtocolErrorV3::InvalidMagic)
    );
    let mut invalid = canonical.clone();
    set_u16(&mut invalid, 8, 2);
    assert_eq!(
        decode_broker_frame_v3(&invalid),
        Err(BrokerProtocolErrorV3::UnsupportedVersion { actual: 2 })
    );
    let mut invalid = canonical.clone();
    set_u16(&mut invalid, 10, 7);
    assert_eq!(
        decode_broker_frame_v3(&invalid),
        Err(BrokerProtocolErrorV3::UnknownFrameType { actual: 7 })
    );
    let mut invalid = canonical.clone();
    set_u32(&mut invalid, 12, 111);
    assert_eq!(
        decode_broker_frame_v3(&invalid),
        Err(BrokerProtocolErrorV3::InvalidPayloadLength {
            kind: BrokerFrameKindV3::PostExec,
            expected: POST_EXEC_V3_PAYLOAD_LEN,
            actual: 111,
        })
    );
    let mut invalid = canonical.clone();
    set_u32(&mut invalid, 16, 3);
    assert_eq!(
        decode_broker_frame_v3(&invalid),
        Err(BrokerProtocolErrorV3::InvalidSequence {
            kind: BrokerFrameKindV3::PostExec,
            expected: 2,
            actual: 3,
        })
    );
    let mut invalid = canonical.clone();
    set_u32(&mut invalid, 20, 1);
    assert_eq!(
        decode_broker_frame_v3(&invalid),
        Err(BrokerProtocolErrorV3::UnsupportedFlags { actual: 1 })
    );
    let mut trailing = canonical;
    trailing.push(0);
    assert_eq!(
        decode_broker_frame_v3(&trailing),
        Err(BrokerProtocolErrorV3::InvalidEncodedLength {
            expected: HEADER + POST_EXEC_V3_PAYLOAD_LEN,
            actual: HEADER + POST_EXEC_V3_PAYLOAD_LEN + 1,
        })
    );
}

#[test]
fn process_and_manifest_wire_adversaries_fail_closed() {
    let canonical_frames = frames(binding(PipelineV1::CollectedTiledGemm, None));
    let hello = canonical_frames[0].encode();
    let mut invalid = hello.clone();
    set_u32(&mut invalid, HEADER, 0);
    assert_eq!(
        decode_broker_frame_v3(&invalid),
        Err(BrokerProtocolErrorV3::ZeroProcessId)
    );
    let mut invalid = hello.clone();
    set_u32(&mut invalid, HEADER + 4, 1);
    assert_eq!(
        decode_broker_frame_v3(&invalid),
        Err(BrokerProtocolErrorV3::NonzeroProcessReserved)
    );
    let mut invalid = hello;
    invalid[HEADER + 8..HEADER + 16].fill(0);
    assert_eq!(
        decode_broker_frame_v3(&invalid),
        Err(BrokerProtocolErrorV3::ZeroProcessStartTime)
    );

    let bootstrap = canonical_frames[1].encode();
    let manifest = HEADER + 80;
    let mut invalid = bootstrap.clone();
    set_u16(&mut invalid, manifest, 3);
    assert_eq!(
        decode_broker_frame_v3(&invalid),
        Err(BrokerProtocolErrorV3::UnknownManifestType { actual: 3 })
    );
    let mut invalid = bootstrap.clone();
    set_u16(&mut invalid, manifest + 2, 2);
    assert_eq!(
        decode_broker_frame_v3(&invalid),
        Err(BrokerProtocolErrorV3::InvalidDescriptorCount {
            manifest: BrokerDescriptorManifestV3::Bootstrap,
            expected: 1,
            actual: 2,
        })
    );
    let mut invalid = bootstrap.clone();
    set_u16(&mut invalid, manifest + 4, 7);
    assert_eq!(
        decode_broker_frame_v3(&invalid),
        Err(BrokerProtocolErrorV3::UnknownDescriptorKind { actual: 7 })
    );
    let mut invalid = bootstrap.clone();
    set_u16(&mut invalid, manifest + 4, 2);
    assert_eq!(
        decode_broker_frame_v3(&invalid),
        Err(BrokerProtocolErrorV3::InvalidDescriptorKind {
            manifest: BrokerDescriptorManifestV3::Bootstrap,
            index: 0,
            expected: BrokerDescriptorKindV3::CargoFe2o3WrapperExecutable,
            actual: BrokerDescriptorKindV3::RustcExecutable,
        })
    );
    let mut invalid = bootstrap.clone();
    set_u16(&mut invalid, manifest + 6, 1);
    assert_eq!(
        decode_broker_frame_v3(&invalid),
        Err(BrokerProtocolErrorV3::NonzeroUnusedDescriptorSlot { index: 1 })
    );
    let mut invalid = bootstrap;
    set_u16(&mut invalid, manifest + 14, 1);
    assert_eq!(
        decode_broker_frame_v3(&invalid),
        Err(BrokerProtocolErrorV3::NonzeroManifestReserved)
    );
}

#[test]
fn deterministic_binding_and_frame_corpus_roundtrips_canonically() {
    for seed in 1_u8..=64 {
        let pipeline = if seed & 1 == 0 {
            PipelineV1::CollectedRowSoftmax
        } else {
            PipelineV1::CollectedTiledGemm
        };
        let worker = (seed % 3 == 0).then(|| patterned_digest(seed.wrapping_add(10)));
        let binding = CapabilityBindingV3::new(
            patterned_digest(seed),
            patterned_digest(seed.wrapping_add(1)),
            patterned_digest(seed.wrapping_add(2)),
            pipeline,
            patterned_digest(seed.wrapping_add(3)),
            patterned_digest(seed.wrapping_add(4)),
            patterned_digest(seed.wrapping_add(5)),
            patterned_digest(seed.wrapping_add(6)),
            patterned_digest(seed.wrapping_add(7)),
            patterned_digest(seed.wrapping_add(8)),
            worker,
        )
        .unwrap();
        assert_eq!(decode_capability_binding_v3(&binding.encode()), Ok(binding));
        for frame in frames(binding) {
            let encoded = frame.encode();
            let decoded = decode_broker_frame_v3(&encoded).unwrap();
            assert_eq!(decoded, frame);
            assert_eq!(decoded.encode(), encoded);
        }
    }
}

#[test]
fn every_single_bit_mutation_is_rejected_or_decodes_as_a_distinct_frame() {
    let canonical_frames = frames(binding(
        PipelineV1::CollectedTiledGemm,
        Some(patterned_digest(99)),
    ));
    let mut mutations = 0_usize;
    for canonical_frame in canonical_frames {
        let canonical = canonical_frame.encode();
        for byte_index in 0..canonical.len() {
            for bit in 0..8 {
                let mut mutated = canonical.clone();
                mutated[byte_index] ^= 1 << bit;
                if let Ok(decoded) = decode_broker_frame_v3(&mutated) {
                    assert_ne!(decoded, canonical_frame);
                    assert_eq!(decoded.encode(), mutated);
                }
                mutations += 1;
            }
        }
    }
    assert_eq!(mutations, 9_728);
}
