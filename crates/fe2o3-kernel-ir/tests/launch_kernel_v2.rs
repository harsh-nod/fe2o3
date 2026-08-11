#[path = "../src/launch_kernel_v2.rs"]
mod launch_kernel_v2;

use launch_kernel_v2::*;

fn id(byte: u8) -> [u8; 32] {
    [byte; 32]
}

fn required_proofs(tuple_identity: KernelVariantTupleIdentityV2) -> Vec<LaunchProofObligationV2> {
    [
        LaunchProofKindV2::TargetAuthenticated,
        LaunchProofKindV2::ArtifactAuthenticated,
        LaunchProofKindV2::KernelIdentityAuthenticated,
        LaunchProofKindV2::SignatureLayoutAuthenticated,
        LaunchProofKindV2::PolicySelectionAuthenticated,
        LaunchProofKindV2::GeometryAndResourcesProved,
    ]
    .into_iter()
    .map(|kind| LaunchProofObligationV2::new(kind, tuple_identity))
    .collect()
}

fn signature() -> KernelSignatureV2 {
    KernelSignatureV2 {
        identity: KernelSignatureIdentityV2::from_bytes(id(3)),
        explicit_argument_bytes: 24,
        kernarg_segment_bytes: 32,
        kernarg_segment_alignment: 8,
        parameters: vec![
            AbiParameterV2 {
                source_index: 0,
                kind: AbiParameterKindV2::ByValue,
                semantic_type: SemanticTypeIdentityV2::from_bytes(id(10)),
                offset: 0,
                size: 4,
                alignment: 4,
            },
            AbiParameterV2 {
                source_index: 1,
                kind: AbiParameterKindV2::SharedGlobalPointer,
                semantic_type: SemanticTypeIdentityV2::from_bytes(id(11)),
                offset: 8,
                size: 8,
                alignment: 8,
            },
            AbiParameterV2 {
                source_index: 2,
                kind: AbiParameterKindV2::UniqueGlobalPointer,
                semantic_type: SemanticTypeIdentityV2::from_bytes(id(12)),
                offset: 16,
                size: 8,
                alignment: 8,
            },
        ],
    }
}

fn launch(block_x: u32, wavefront: WavefrontWidthV2) -> Gfx942LaunchContractV2 {
    let max_grid_x = u32::MAX / block_x;
    Gfx942LaunchContractV2 {
        rank: 1,
        block: BlockShapePolicyV2::Exact(DimensionsV2::new(block_x, 1, 1)),
        max_grid_blocks: DimensionsV2::new(max_grid_x, 1, 1),
        minimum_flat_workgroup_size: block_x,
        maximum_flat_workgroup_size: block_x,
        wavefront,
        require_full_waves: true,
        minimum_waves_per_execution_unit: 1,
        maximum_waves_per_execution_unit: 8,
        max_total_workitems: u64::from(max_grid_x) * u64::from(block_x),
        unsupported: UnsupportedLaunchFeaturesV2::NONE,
    }
}

fn variant(
    byte: u8,
    variant_name: &str,
    entry_name: &str,
    block_x: u32,
    wavefront: WavefrontWidthV2,
) -> KernelVariantV2 {
    KernelVariantV2 {
        kernel_identity: KernelIdentityV2::from_bytes(id(byte)),
        policy_identity: KernelPolicyIdentityV2::from_bytes(id(byte.wrapping_add(20))),
        artifact_identity: ArtifactIdentityV2::from_bytes(id(byte.wrapping_add(40))),
        tuple_identity: KernelVariantTupleIdentityV2::from_bytes([0; 32]),
        variant_name: variant_name.to_owned(),
        entry_name: entry_name.to_owned(),
        launch: launch(block_x, wavefront),
        resources: Gfx942ResourceLimitsV2 {
            static_lds_bytes: 1_024,
            maximum_dynamic_lds_bytes: 2_048,
            dynamic_lds_alignment: 16,
            private_segment_bytes: 128,
        },
        capabilities: vec![
            LaunchCapabilityV2::ExactWaveMode,
            LaunchCapabilityV2::StaticLds,
            LaunchCapabilityV2::DynamicLds,
            LaunchCapabilityV2::WorkgroupBarrier,
        ],
        proof_obligations: Vec::new(),
    }
}

fn bind_family(candidate: &mut LaunchKernelFamilyV2) {
    for index in 0..candidate.variants.len() {
        let tuple_identity = canonical_variant_tuple_identity_v2(
            &candidate.target,
            candidate.family_identity,
            &candidate.logical_name,
            &candidate.signature,
            &candidate.variants[index],
        );
        candidate.variants[index].tuple_identity = tuple_identity;
        candidate.variants[index].proof_obligations = required_proofs(tuple_identity);
    }
}

fn family() -> LaunchKernelFamilyV2 {
    let mut variants = vec![
        variant(
            4,
            "wg128-wave64",
            "saxpy_wg128_wave64",
            128,
            WavefrontWidthV2::Wave64,
        ),
        variant(
            5,
            "wg256-wave64",
            "saxpy_wg256_wave64",
            256,
            WavefrontWidthV2::Wave64,
        ),
    ];
    variants[1].artifact_identity = variants[0].artifact_identity;
    let mut candidate = LaunchKernelFamilyV2 {
        target: Gfx942TargetBindingV2::gfx942_xnack_minus(TargetIdentityV2::from_bytes(id(1))),
        family_identity: KernelFamilyIdentityV2::from_bytes(id(2)),
        logical_name: "saxpy".to_owned(),
        signature: signature(),
        variants,
    };
    bind_family(&mut candidate);
    candidate
}

fn limits() -> LaunchKernelLimitsV2 {
    LaunchKernelLimitsV2::default()
}

#[test]
fn canonical_family_round_trips_and_freezes_header() {
    let family = family();
    let bytes = encode_launch_kernel_family_v2(&family, &limits()).unwrap();
    assert_eq!(&bytes[..8], &LAUNCH_KERNEL_V2_MAGIC);
    assert_eq!(&bytes[8..10], &LAUNCH_KERNEL_V2_VERSION.to_le_bytes());
    assert_eq!(&bytes[10..12], &[0, 0]);
    assert_eq!(
        u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
        bytes.len() as u32
    );
    assert_eq!(&bytes[16..18], &2_u16.to_le_bytes());
    assert_eq!(&bytes[18..20], &3_u16.to_le_bytes());
    assert_eq!(&bytes[20..24], &[0; 4]);
    assert_eq!(&bytes[24..56], &id(1));
    assert_eq!(&bytes[56..64], &[1, 0, 6, 8, 1, 0, 0, 0]);
    assert_eq!(
        decode_launch_kernel_family_v2(&bytes, &limits()),
        Ok(family.clone())
    );
    assert_eq!(
        encode_launch_kernel_family_v2(
            &decode_launch_kernel_family_v2(&bytes, &limits()).unwrap(),
            &limits(),
        )
        .unwrap(),
        bytes
    );
    assert!(LAUNCH_KERNEL_V2_LIMITATIONS.contains("no export, lowering, bundle, runtime"));
}

#[test]
fn one_typed_signature_is_shared_by_distinct_family_policies() {
    let family = family();
    family.validate(&limits()).unwrap();
    assert_eq!(family.signature.identity.0, id(3));
    assert_eq!(family.variants.len(), 2);
    assert_ne!(
        family.variants[0].kernel_identity,
        family.variants[1].kernel_identity
    );
    assert_ne!(
        family.variants[0].policy_identity,
        family.variants[1].policy_identity
    );
    assert_eq!(
        family.variants[0].artifact_identity,
        family.variants[1].artifact_identity
    );
    assert_ne!(
        family.variants[0].tuple_identity,
        family.variants[1].tuple_identity
    );
    for variant in &family.variants {
        assert!(
            variant
                .proof_obligations
                .iter()
                .all(|proof| proof.variant_tuple_identity == variant.tuple_identity)
        );
    }
}

fn assert_tuple_substitution_rejected(mutation: impl FnOnce(&mut LaunchKernelFamilyV2)) {
    let mut candidate = family();
    mutation(&mut candidate);
    assert_eq!(
        candidate.validate(&limits()),
        Err(LaunchKernelValidationErrorV2::VariantTupleIdentityMismatch)
    );
}

#[test]
fn canonical_tuple_rejects_every_component_substitution() {
    assert_tuple_substitution_rejected(|candidate| {
        candidate.target.identity = TargetIdentityV2::from_bytes(id(90));
    });
    assert_tuple_substitution_rejected(|candidate| {
        candidate.family_identity = KernelFamilyIdentityV2::from_bytes(id(91));
    });
    assert_tuple_substitution_rejected(|candidate| {
        candidate.logical_name = "saxpy_v2".to_owned();
    });
    assert_tuple_substitution_rejected(|candidate| {
        candidate.signature.identity = KernelSignatureIdentityV2::from_bytes(id(92));
    });
    assert_tuple_substitution_rejected(|candidate| {
        candidate.signature.explicit_argument_bytes = 25;
    });
    assert_tuple_substitution_rejected(|candidate| {
        candidate.signature.parameters[0].semantic_type =
            SemanticTypeIdentityV2::from_bytes(id(93));
    });
    assert_tuple_substitution_rejected(|candidate| {
        candidate.variants[0].artifact_identity = ArtifactIdentityV2::from_bytes(id(94));
    });
    assert_tuple_substitution_rejected(|candidate| {
        candidate.variants[0].entry_name = "saxpy_replaced_entry".to_owned();
    });
    assert_tuple_substitution_rejected(|candidate| {
        candidate.variants[0].kernel_identity = KernelIdentityV2::from_bytes(id(95));
    });
    assert_tuple_substitution_rejected(|candidate| {
        candidate.variants[0].policy_identity = KernelPolicyIdentityV2::from_bytes(id(96));
    });
    assert_tuple_substitution_rejected(|candidate| {
        candidate.variants[0].variant_name = "wg129-wave64".to_owned();
    });
    assert_tuple_substitution_rejected(|candidate| {
        candidate.variants[0].launch.max_grid_blocks.x -= 1;
    });
    assert_tuple_substitution_rejected(|candidate| {
        candidate.variants[0]
            .launch
            .minimum_waves_per_execution_unit = 2;
    });
    assert_tuple_substitution_rejected(|candidate| {
        candidate.variants[0].launch.require_full_waves = false;
    });
    assert_tuple_substitution_rejected(|candidate| {
        candidate.variants[0].resources.private_segment_bytes += 1;
    });
    assert_tuple_substitution_rejected(|candidate| {
        candidate.variants[0]
            .capabilities
            .push(LaunchCapabilityV2::DeviceAtomics);
    });

    let mut mixed_cov = family();
    mixed_cov.target.code_object = CodeObjectVersionV2::V5;
    assert_eq!(
        mixed_cov.validate(&limits()),
        Err(LaunchKernelValidationErrorV2::UnsupportedTarget)
    );

    let mut swapped = family();
    let first_kernel = swapped.variants[0].kernel_identity;
    swapped.variants[0].kernel_identity = swapped.variants[1].kernel_identity;
    swapped.variants[1].kernel_identity = first_kernel;
    assert_eq!(
        swapped.validate(&limits()),
        Err(LaunchKernelValidationErrorV2::VariantTupleIdentityMismatch)
    );
}

#[test]
fn proof_records_commit_the_exact_canonical_tuple() {
    let mut candidate = family();
    candidate.variants[0].proof_obligations[0].variant_tuple_identity =
        KernelVariantTupleIdentityV2::from_bytes(id(97));
    assert_eq!(
        candidate.validate(&limits()),
        Err(LaunchKernelValidationErrorV2::ProofTupleIdentityMismatch(
            LaunchProofKindV2::TargetAuthenticated
        ))
    );

    candidate = family();
    let substituted = KernelVariantTupleIdentityV2::from_bytes(id(98));
    candidate.variants[0].tuple_identity = substituted;
    for proof in &mut candidate.variants[0].proof_obligations {
        proof.variant_tuple_identity = substituted;
    }
    assert_eq!(
        candidate.validate(&limits()),
        Err(LaunchKernelValidationErrorV2::VariantTupleIdentityMismatch)
    );
}

#[test]
fn tuple_digest_uses_standard_sha256() {
    assert_eq!(
        sha256_test_vector_v2(b""),
        [
            0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f,
            0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b,
            0x78, 0x52, 0xb8, 0x55,
        ]
    );
    assert_eq!(
        sha256_test_vector_v2(b"abc"),
        [
            0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
            0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
            0xf2, 0x00, 0x15, 0xad,
        ]
    );
}

#[test]
fn exact_gfx942_target_binding_fails_closed() {
    assert_eq!(GFX942_REQUIRED_WAVEFRONT_WIDTH_V2, WavefrontWidthV2::Wave64);
    let mut candidate = family();
    candidate.target.xnack = XnackModeV2::Enabled;
    assert_eq!(
        candidate.validate(&limits()),
        Err(LaunchKernelValidationErrorV2::UnsupportedTarget)
    );
    candidate = family();
    candidate.target.code_object = CodeObjectVersionV2::V5;
    assert_eq!(
        candidate.validate(&limits()),
        Err(LaunchKernelValidationErrorV2::UnsupportedTarget)
    );
    candidate = family();
    candidate.target.pointer_width_bytes = 4;
    assert_eq!(
        candidate.validate(&limits()),
        Err(LaunchKernelValidationErrorV2::UnsupportedTarget)
    );
    candidate = family();
    candidate.target.endianness = EndiannessV2::Big;
    assert_eq!(
        candidate.validate(&limits()),
        Err(LaunchKernelValidationErrorV2::UnsupportedTarget)
    );
    candidate = family();
    candidate.variants[0].launch.wavefront = WavefrontWidthV2::Wave32;
    assert_eq!(
        candidate.validate(&limits()),
        Err(LaunchKernelValidationErrorV2::UnsupportedWavefrontWidth)
    );
}

#[test]
fn abi_parameter_bounds_alignment_overlap_and_pointer_width_are_checked() {
    let mut candidate = family();
    candidate.signature.parameters[1].source_index = 9;
    assert_eq!(
        candidate.validate(&limits()),
        Err(LaunchKernelValidationErrorV2::NonCanonicalParameterOrder)
    );
    candidate = family();
    candidate.signature.parameters[1].offset = 4;
    assert!(matches!(
        candidate.validate(&limits()),
        Err(LaunchKernelValidationErrorV2::MisalignedParameter { index: 1 })
    ));
    candidate = family();
    candidate.signature.parameters[2].offset = 8;
    assert!(matches!(
        candidate.validate(&limits()),
        Err(LaunchKernelValidationErrorV2::OverlappingParameters { index: 2 })
    ));
    candidate = family();
    candidate.signature.parameters[2].offset = 24;
    assert!(matches!(
        candidate.validate(&limits()),
        Err(LaunchKernelValidationErrorV2::ParameterOutOfBounds { index: 2 })
    ));
    candidate = family();
    candidate.signature.parameters[1].size = 4;
    assert!(matches!(
        candidate.validate(&limits()),
        Err(LaunchKernelValidationErrorV2::InvalidPointerParameter { index: 1 })
    ));
    candidate = family();
    candidate.signature.kernarg_segment_bytes = GFX942_MAX_KERNARG_SEGMENT_BYTES_V2 + 1;
    assert_eq!(
        candidate.validate(&limits()),
        Err(LaunchKernelValidationErrorV2::InvalidAbiBounds)
    );
}

#[test]
fn valid_gfx942_wave64_launches_produce_checked_facts() {
    let family = family();
    let wave64_128 = family
        .validate_launch(
            "wg128-wave64",
            LaunchRequestV2 {
                grid_blocks: DimensionsV2::new(65_535, 1, 1),
                block_threads: DimensionsV2::new(128, 1, 1),
                dynamic_lds_bytes: 512,
                dynamic_lds_alignment: 16,
            },
            &limits(),
        )
        .unwrap();
    assert_eq!(wave64_128.flat_workgroup_size, 128);
    assert_eq!(wave64_128.grid_block_count, 65_535);
    assert_eq!(
        wave64_128.global_workitems,
        DimensionsV2::new(8_388_480, 1, 1)
    );
    assert_eq!(wave64_128.total_workitems, 8_388_480);
    assert_eq!(wave64_128.waves_per_workgroup, 2);
    assert_eq!(wave64_128.total_lds_bytes, 1_536);

    let wave64 = family
        .validate_launch(
            "wg256-wave64",
            LaunchRequestV2 {
                grid_blocks: DimensionsV2::new(17, 1, 1),
                block_threads: DimensionsV2::new(256, 1, 1),
                dynamic_lds_bytes: 0,
                dynamic_lds_alignment: 1,
            },
            &limits(),
        )
        .unwrap();
    assert_eq!(wave64.waves_per_workgroup, 4);
}

#[test]
fn rank_shape_wave_grid_and_total_limits_fail_closed() {
    let mut candidate = family();
    candidate.variants[0].launch.rank = 0;
    assert_eq!(
        candidate.validate(&limits()),
        Err(LaunchKernelValidationErrorV2::InvalidRank)
    );

    candidate = family();
    candidate.variants[0].launch.block = BlockShapePolicyV2::Exact(DimensionsV2::new(33, 1, 1));
    candidate.variants[0].launch.minimum_flat_workgroup_size = 33;
    candidate.variants[0].launch.maximum_flat_workgroup_size = 33;
    assert_eq!(
        candidate.validate(&limits()),
        Err(LaunchKernelValidationErrorV2::NoAdmittedBlockShape)
    );

    let valid_family = family();
    assert_eq!(
        valid_family.validate_launch(
            "wg128-wave64",
            LaunchRequestV2 {
                grid_blocks: DimensionsV2::new(1, 2, 1),
                block_threads: DimensionsV2::new(128, 1, 1),
                dynamic_lds_bytes: 0,
                dynamic_lds_alignment: 1,
            },
            &limits(),
        ),
        Err(LaunchKernelValidationErrorV2::UnusedDimensionNotOne)
    );
    assert_eq!(
        valid_family.validate_launch(
            "wg128-wave64",
            LaunchRequestV2 {
                grid_blocks: DimensionsV2::new(u32::MAX, 1, 1),
                block_threads: DimensionsV2::new(128, 1, 1),
                dynamic_lds_bytes: 0,
                dynamic_lds_alignment: 1,
            },
            &limits(),
        ),
        Err(LaunchKernelValidationErrorV2::GridLimitExceeded)
    );

    candidate = family();
    candidate.variants[0].launch.max_total_workitems = 1;
    assert_eq!(
        candidate.validate(&limits()),
        Err(LaunchKernelValidationErrorV2::TotalWorkitemLimitExceeded)
    );
    candidate = family();
    candidate.variants[0].launch.max_grid_blocks = DimensionsV2::new(u32::MAX, 1, 1);
    assert_eq!(
        candidate.validate(&limits()),
        Err(LaunchKernelValidationErrorV2::ArithmeticOverflow)
    );
}

#[test]
fn block_policy_is_canonical_and_has_a_wave64_inhabitant() {
    let mut candidate = family();
    let launch = &mut candidate.variants[0].launch;
    launch.block = BlockShapePolicyV2::Bounded {
        minimum: DimensionsV2::new(128, 1, 1),
        maximum: DimensionsV2::new(128, 1, 1),
    };
    assert_eq!(
        candidate.validate(&limits()),
        Err(LaunchKernelValidationErrorV2::NonCanonicalBlockPolicy)
    );

    candidate = family();
    let launch = &mut candidate.variants[0].launch;
    launch.block = BlockShapePolicyV2::Bounded {
        minimum: DimensionsV2::new(33, 1, 1),
        maximum: DimensionsV2::new(33, 1, 1),
    };
    launch.minimum_flat_workgroup_size = 33;
    launch.maximum_flat_workgroup_size = 33;
    assert_eq!(
        candidate.validate(&limits()),
        Err(LaunchKernelValidationErrorV2::NonCanonicalBlockPolicy)
    );

    candidate = family();
    let launch = &mut candidate.variants[0].launch;
    launch.block = BlockShapePolicyV2::Bounded {
        minimum: DimensionsV2::new(33, 1, 1),
        maximum: DimensionsV2::new(63, 1, 1),
    };
    launch.minimum_flat_workgroup_size = 33;
    launch.maximum_flat_workgroup_size = 63;
    launch.max_grid_blocks = DimensionsV2::new(1, 1, 1);
    launch.max_total_workitems = 63;
    launch.require_full_waves = false;
    assert_eq!(
        candidate.validate(&limits()),
        Err(LaunchKernelValidationErrorV2::NoAdmittedBlockShape)
    );

    candidate = family();
    let launch = &mut candidate.variants[0].launch;
    launch.block = BlockShapePolicyV2::Bounded {
        minimum: DimensionsV2::new(33, 1, 1),
        maximum: DimensionsV2::new(64, 1, 1),
    };
    launch.minimum_flat_workgroup_size = 33;
    launch.maximum_flat_workgroup_size = 64;
    launch.max_grid_blocks = DimensionsV2::new(1, 1, 1);
    launch.max_total_workitems = 64;
    launch.require_full_waves = false;
    bind_family(&mut candidate);
    candidate.validate(&limits()).unwrap();
    assert!(
        candidate
            .validate_launch(
                "wg128-wave64",
                LaunchRequestV2 {
                    grid_blocks: DimensionsV2::new(1, 1, 1),
                    block_threads: DimensionsV2::new(64, 1, 1),
                    dynamic_lds_bytes: 0,
                    dynamic_lds_alignment: 1,
                },
                &limits(),
            )
            .is_ok()
    );
    assert_eq!(
        candidate.validate_launch(
            "wg128-wave64",
            LaunchRequestV2 {
                grid_blocks: DimensionsV2::new(1, 1, 1),
                block_threads: DimensionsV2::new(33, 1, 1),
                dynamic_lds_bytes: 0,
                dynamic_lds_alignment: 1,
            },
            &limits(),
        ),
        Ok(ValidatedLaunchFactsV2 {
            flat_workgroup_size: 33,
            grid_block_count: 1,
            global_workitems: DimensionsV2::new(33, 1, 1),
            total_workitems: 33,
            waves_per_workgroup: 1,
            total_lds_bytes: 1_024,
        })
    );
}

#[test]
fn unsupported_cooperative_and_dynamic_launch_features_are_rejected() {
    for unsupported in [
        UnsupportedLaunchFeaturesV2 {
            cooperative_grid: true,
            ..UnsupportedLaunchFeaturesV2::NONE
        },
        UnsupportedLaunchFeaturesV2 {
            device_side_enqueue: true,
            ..UnsupportedLaunchFeaturesV2::NONE
        },
        UnsupportedLaunchFeaturesV2 {
            dynamic_parallelism: true,
            ..UnsupportedLaunchFeaturesV2::NONE
        },
    ] {
        let mut candidate = family();
        candidate.variants[0].launch.unsupported = unsupported;
        assert_eq!(
            candidate.validate(&limits()),
            Err(LaunchKernelValidationErrorV2::UnsupportedLaunchFeature)
        );
        assert!(matches!(
            encode_launch_kernel_family_v2(&candidate, &limits()),
            Err(LaunchKernelEncodeErrorV2::Model(
                LaunchKernelValidationErrorV2::UnsupportedLaunchFeature
            ))
        ));
    }
}

#[test]
fn resource_capability_and_proof_boundaries_are_enforced() {
    let mut candidate = family();
    candidate.variants[0].resources.static_lds_bytes = 65_000;
    candidate.variants[0].resources.maximum_dynamic_lds_bytes = 1_000;
    assert_eq!(
        candidate.validate(&limits()),
        Err(LaunchKernelValidationErrorV2::LdsLimitExceeded)
    );
    candidate = family();
    candidate.variants[0].resources.private_segment_bytes = GFX942_MAX_PRIVATE_SEGMENT_BYTES_V2 + 1;
    assert_eq!(
        candidate.validate(&limits()),
        Err(LaunchKernelValidationErrorV2::PrivateSegmentLimitExceeded)
    );
    candidate = family();
    candidate.variants[0].capabilities.remove(2);
    assert_eq!(
        candidate.validate(&limits()),
        Err(LaunchKernelValidationErrorV2::MissingCapability(
            LaunchCapabilityV2::DynamicLds
        ))
    );
    candidate = family();
    candidate.variants[0].proof_obligations.pop();
    assert_eq!(
        candidate.validate(&limits()),
        Err(LaunchKernelValidationErrorV2::MissingProofObligation(
            LaunchProofKindV2::GeometryAndResourcesProved
        ))
    );

    let family = family();
    assert_eq!(
        family.validate_launch(
            "wg128-wave64",
            LaunchRequestV2 {
                grid_blocks: DimensionsV2::new(1, 1, 1),
                block_threads: DimensionsV2::new(128, 1, 1),
                dynamic_lds_bytes: 2_049,
                dynamic_lds_alignment: 16,
            },
            &limits(),
        ),
        Err(LaunchKernelValidationErrorV2::DynamicLdsLimitExceeded)
    );
}

#[test]
fn family_order_and_all_identity_domains_are_substitution_sensitive() {
    let mut candidate = family();
    candidate.variants.swap(0, 1);
    assert_eq!(
        candidate.validate(&limits()),
        Err(LaunchKernelValidationErrorV2::NonCanonicalVariantOrder)
    );
    candidate = family();
    candidate.variants[1].kernel_identity = candidate.variants[0].kernel_identity;
    assert_eq!(
        candidate.validate(&limits()),
        Err(LaunchKernelValidationErrorV2::VariantTupleIdentityMismatch)
    );
    bind_family(&mut candidate);
    assert_eq!(
        candidate.validate(&limits()),
        Err(LaunchKernelValidationErrorV2::DuplicateKernelIdentity)
    );
    candidate = family();
    candidate.variants[1].policy_identity = candidate.variants[0].policy_identity;
    assert_eq!(
        candidate.validate(&limits()),
        Err(LaunchKernelValidationErrorV2::VariantTupleIdentityMismatch)
    );
    bind_family(&mut candidate);
    assert_eq!(
        candidate.validate(&limits()),
        Err(LaunchKernelValidationErrorV2::DuplicatePolicyIdentity)
    );
    candidate = family();
    candidate.variants[1].variant_name = candidate.variants[0].variant_name.clone();
    assert_eq!(
        candidate.validate(&limits()),
        Err(LaunchKernelValidationErrorV2::VariantTupleIdentityMismatch)
    );
    bind_family(&mut candidate);
    assert_eq!(
        candidate.validate(&limits()),
        Err(LaunchKernelValidationErrorV2::DuplicateVariantName)
    );
    candidate = family();
    candidate.variants[1].entry_name = candidate.variants[0].entry_name.clone();
    assert_eq!(
        candidate.validate(&limits()),
        Err(LaunchKernelValidationErrorV2::VariantTupleIdentityMismatch)
    );
    bind_family(&mut candidate);
    assert_eq!(
        candidate.validate(&limits()),
        Err(LaunchKernelValidationErrorV2::DuplicateEntryName)
    );
}

#[test]
fn decoder_rejects_header_target_length_and_resource_mutations() {
    let canonical = encode_launch_kernel_family_v2(&family(), &limits()).unwrap();
    let mut mutated = canonical.clone();
    mutated[0] ^= 1;
    assert_eq!(
        decode_launch_kernel_family_v2(&mutated, &limits()),
        Err(LaunchKernelDecodeErrorV2::BadMagic)
    );
    mutated = canonical.clone();
    mutated[8..10].copy_from_slice(&4_u16.to_le_bytes());
    assert_eq!(
        decode_launch_kernel_family_v2(&mutated, &limits()),
        Err(LaunchKernelDecodeErrorV2::UnsupportedVersion(4))
    );
    mutated = canonical.clone();
    mutated[10] = 1;
    assert_eq!(
        decode_launch_kernel_family_v2(&mutated, &limits()),
        Err(LaunchKernelDecodeErrorV2::NonZeroReserved)
    );
    mutated = canonical.clone();
    let shorter = (canonical.len() as u32 - 1).to_le_bytes();
    mutated[12..16].copy_from_slice(&shorter);
    assert_eq!(
        decode_launch_kernel_family_v2(&mutated, &limits()),
        Err(LaunchKernelDecodeErrorV2::LengthMismatch)
    );
    mutated = canonical.clone();
    mutated[56] = 99;
    assert_eq!(
        decode_launch_kernel_family_v2(&mutated, &limits()),
        Err(LaunchKernelDecodeErrorV2::UnknownTag)
    );
    mutated = canonical.clone();
    mutated[57] = 1;
    assert_eq!(
        decode_launch_kernel_family_v2(&mutated, &limits()),
        Err(LaunchKernelDecodeErrorV2::Model(
            LaunchKernelValidationErrorV2::UnsupportedTarget
        ))
    );
    assert_eq!(
        decode_launch_kernel_family_v2(&canonical[..canonical.len() - 1], &limits()),
        Err(LaunchKernelDecodeErrorV2::LengthMismatch)
    );
    let tight = LaunchKernelLimitsV2 {
        max_encoded_bytes: canonical.len() - 1,
        ..limits()
    };
    assert!(matches!(
        decode_launch_kernel_family_v2(&canonical, &tight),
        Err(LaunchKernelDecodeErrorV2::ResourceLimit {
            resource: "encoded bytes",
            ..
        })
    ));
}

#[test]
fn source_is_safe_pure_rust_and_remains_inert() {
    let source = include_str!("../src/launch_kernel_v2.rs");
    assert!(!source.contains("unsafe {"));
    assert!(!source.contains("extern \"C\""));
    assert!(!source.contains("std::process"));
    assert!(!source.contains("std::fs"));
    let library = include_str!("../src/lib.rs");
    assert!(!library.contains("launch_kernel_v2"));
}

#[derive(Clone, Copy)]
struct DeterministicRng(u64);

impl DeterministicRng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0
    }

    fn below(&mut self, ceiling: u32) -> u32 {
        (self.next() % u64::from(ceiling)) as u32
    }
}

fn oracle_family() -> LaunchKernelFamilyV2 {
    let mut candidate = family();
    candidate.variants.truncate(1);
    let variant = &mut candidate.variants[0];
    variant.variant_name = "oracle".to_owned();
    variant.entry_name = "saxpy_oracle".to_owned();
    variant.launch = Gfx942LaunchContractV2 {
        rank: 3,
        block: BlockShapePolicyV2::Bounded {
            minimum: DimensionsV2::new(1, 1, 1),
            maximum: DimensionsV2::new(16, 8, 4),
        },
        max_grid_blocks: DimensionsV2::new(1_024, 128, 32),
        minimum_flat_workgroup_size: 1,
        maximum_flat_workgroup_size: 512,
        wavefront: WavefrontWidthV2::Wave64,
        require_full_waves: false,
        minimum_waves_per_execution_unit: 1,
        maximum_waves_per_execution_unit: 8,
        max_total_workitems: 2_147_483_648,
        unsupported: UnsupportedLaunchFeaturesV2::NONE,
    };
    variant.resources = Gfx942ResourceLimitsV2 {
        static_lds_bytes: 512,
        maximum_dynamic_lds_bytes: 4_096,
        dynamic_lds_alignment: 16,
        private_segment_bytes: 64,
    };
    bind_family(&mut candidate);
    candidate
}

fn independent_launch_oracle(
    variant: &KernelVariantV2,
    request: LaunchRequestV2,
) -> Option<ValidatedLaunchFactsV2> {
    let contract = variant.launch;
    if contract.wavefront != GFX942_REQUIRED_WAVEFRONT_WIDTH_V2 {
        return None;
    }
    let rank_ok = (1..=3).contains(&contract.rank)
        && !(contract.rank < 2 && (request.grid_blocks.y != 1 || request.block_threads.y != 1))
        && !(contract.rank < 3 && (request.grid_blocks.z != 1 || request.block_threads.z != 1));
    if !rank_ok
        || [
            request.grid_blocks.x,
            request.grid_blocks.y,
            request.grid_blocks.z,
            request.block_threads.x,
            request.block_threads.y,
            request.block_threads.z,
        ]
        .contains(&0)
    {
        return None;
    }
    if request.grid_blocks.x > contract.max_grid_blocks.x
        || request.grid_blocks.y > contract.max_grid_blocks.y
        || request.grid_blocks.z > contract.max_grid_blocks.z
    {
        return None;
    }
    let admitted_block = match contract.block {
        BlockShapePolicyV2::Exact(exact) => request.block_threads == exact,
        BlockShapePolicyV2::Bounded { minimum, maximum } => {
            request.block_threads.x >= minimum.x
                && request.block_threads.y >= minimum.y
                && request.block_threads.z >= minimum.z
                && request.block_threads.x <= maximum.x
                && request.block_threads.y <= maximum.y
                && request.block_threads.z <= maximum.z
        }
    };
    if !admitted_block {
        return None;
    }

    let flat = u128::from(request.block_threads.x)
        * u128::from(request.block_threads.y)
        * u128::from(request.block_threads.z);
    if flat < u128::from(contract.minimum_flat_workgroup_size)
        || flat > u128::from(contract.maximum_flat_workgroup_size)
        || flat > u128::from(GFX942_MAX_FLAT_WORKGROUP_SIZE_V2)
        || (contract.require_full_waves && flat % u128::from(contract.wavefront.lanes()) != 0)
    {
        return None;
    }
    if request.dynamic_lds_bytes > variant.resources.maximum_dynamic_lds_bytes {
        return None;
    }
    let alignment_ok = if request.dynamic_lds_bytes == 0 {
        request.dynamic_lds_alignment == 1
    } else {
        request.dynamic_lds_alignment.is_power_of_two()
            && request.dynamic_lds_alignment >= variant.resources.dynamic_lds_alignment
            && request.dynamic_lds_alignment <= 256
    };
    if !alignment_ok {
        return None;
    }
    let total_lds =
        u64::from(variant.resources.static_lds_bytes) + u64::from(request.dynamic_lds_bytes);
    if total_lds > u64::from(GFX942_MAX_LDS_BYTES_PER_WORKGROUP_V2) {
        return None;
    }

    let global_x = u128::from(request.grid_blocks.x) * u128::from(request.block_threads.x);
    let global_y = u128::from(request.grid_blocks.y) * u128::from(request.block_threads.y);
    let global_z = u128::from(request.grid_blocks.z) * u128::from(request.block_threads.z);
    let total = global_x * global_y * global_z;
    if global_x > u128::from(u32::MAX)
        || global_y > u128::from(u32::MAX)
        || global_z > u128::from(u32::MAX)
        || total > u128::from(contract.max_total_workitems)
        || total > u128::from(u64::MAX)
    {
        return None;
    }
    let lanes = u128::from(contract.wavefront.lanes());
    Some(ValidatedLaunchFactsV2 {
        flat_workgroup_size: flat as u32,
        grid_block_count: (u128::from(request.grid_blocks.x)
            * u128::from(request.grid_blocks.y)
            * u128::from(request.grid_blocks.z)) as u64,
        global_workitems: DimensionsV2::new(global_x as u32, global_y as u32, global_z as u32),
        total_workitems: total as u64,
        waves_per_workgroup: flat.div_ceil(lanes) as u32,
        total_lds_bytes: total_lds as u32,
    })
}

#[test]
fn exhaustive_small_launch_domain_matches_independent_oracle() {
    let mut candidate = oracle_family();
    let variant = &mut candidate.variants[0];
    variant.launch.rank = 2;
    variant.launch.block = BlockShapePolicyV2::Bounded {
        minimum: DimensionsV2::new(1, 1, 1),
        maximum: DimensionsV2::new(8, 8, 1),
    };
    variant.launch.max_grid_blocks = DimensionsV2::new(4, 4, 1);
    variant.launch.maximum_flat_workgroup_size = 64;
    variant.launch.max_total_workitems = 1_024;
    variant.resources.maximum_dynamic_lds_bytes = 4;
    variant.resources.dynamic_lds_alignment = 2;
    bind_family(&mut candidate);
    candidate.validate(&limits()).unwrap();

    let mut cases = 0_u64;
    for grid_x in 0..=5 {
        for grid_y in 0..=5 {
            for block_x in 0..=5 {
                for block_y in 0..=5 {
                    for dynamic_lds_bytes in 0..=5 {
                        for dynamic_lds_alignment in [1, 2, 4] {
                            let request = LaunchRequestV2 {
                                grid_blocks: DimensionsV2::new(grid_x, grid_y, 1),
                                block_threads: DimensionsV2::new(block_x, block_y, 1),
                                dynamic_lds_bytes,
                                dynamic_lds_alignment,
                            };
                            let expected =
                                independent_launch_oracle(&candidate.variants[0], request);
                            let actual = candidate.validate_launch("oracle", request, &limits());
                            assert_eq!(actual.ok(), expected, "request={request:?}");
                            cases += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(cases, 23_328);
}

#[test]
fn exhaustive_small_wave_width_and_full_wave_boundaries() {
    let mut cases = 0_u64;
    for wavefront in [WavefrontWidthV2::Wave32, WavefrontWidthV2::Wave64] {
        for require_full_waves in [false, true] {
            for block_x in 1..=128 {
                let mut candidate = family();
                candidate.variants.truncate(1);
                let variant = &mut candidate.variants[0];
                variant.launch = Gfx942LaunchContractV2 {
                    rank: 1,
                    block: BlockShapePolicyV2::Exact(DimensionsV2::new(block_x, 1, 1)),
                    max_grid_blocks: DimensionsV2::new(1, 1, 1),
                    minimum_flat_workgroup_size: block_x,
                    maximum_flat_workgroup_size: block_x,
                    wavefront,
                    require_full_waves,
                    minimum_waves_per_execution_unit: 1,
                    maximum_waves_per_execution_unit: 8,
                    max_total_workitems: u64::from(block_x),
                    unsupported: UnsupportedLaunchFeaturesV2::NONE,
                };
                let expected =
                    wavefront == WavefrontWidthV2::Wave64 && block_x % wavefront.lanes() == 0;
                bind_family(&mut candidate);
                assert_eq!(candidate.validate(&limits()).is_ok(), expected);
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 512);
}

#[test]
fn exhaustive_block_policy_canonicalization_and_admissible_set() {
    let mut contract_cases = 0_u64;
    let mut request_cases = 0_u64;

    for require_full_waves in [false, true] {
        for block_x in 1..=96 {
            let mut candidate = family();
            candidate.variants.truncate(1);
            let launch = &mut candidate.variants[0].launch;
            launch.block = BlockShapePolicyV2::Exact(DimensionsV2::new(block_x, 1, 1));
            launch.max_grid_blocks = DimensionsV2::new(1, 1, 1);
            launch.minimum_flat_workgroup_size = block_x;
            launch.maximum_flat_workgroup_size = block_x;
            launch.require_full_waves = require_full_waves;
            launch.max_total_workitems = u64::from(block_x);
            bind_family(&mut candidate);
            let expected = block_x % 64 == 0;
            assert_eq!(candidate.validate(&limits()).is_ok(), expected);
            contract_cases += 1;
        }
    }

    for require_full_waves in [false, true] {
        for minimum in 1..=96 {
            for maximum in minimum..=96 {
                let mut candidate = family();
                candidate.variants.truncate(1);
                let launch = &mut candidate.variants[0].launch;
                launch.block = BlockShapePolicyV2::Bounded {
                    minimum: DimensionsV2::new(minimum, 1, 1),
                    maximum: DimensionsV2::new(maximum, 1, 1),
                };
                launch.max_grid_blocks = DimensionsV2::new(1, 1, 1);
                launch.minimum_flat_workgroup_size = minimum;
                launch.maximum_flat_workgroup_size = maximum;
                launch.require_full_waves = require_full_waves;
                launch.max_total_workitems = u64::from(maximum);
                bind_family(&mut candidate);

                let has_full_wave = (minimum..=maximum).any(|shape| shape % 64 == 0);
                let expected_valid = minimum != maximum && has_full_wave;
                assert_eq!(
                    candidate.validate(&limits()).is_ok(),
                    expected_valid,
                    "minimum={minimum} maximum={maximum} full={require_full_waves}"
                );
                contract_cases += 1;

                let exercise_requests =
                    minimum != maximum && (!require_full_waves || has_full_wave);
                if exercise_requests {
                    for shape in 1..=96 {
                        let expected_launch = expected_valid
                            && shape >= minimum
                            && shape <= maximum
                            && (!require_full_waves || shape % 64 == 0);
                        let actual = candidate.validate_launch(
                            "wg128-wave64",
                            LaunchRequestV2 {
                                grid_blocks: DimensionsV2::new(1, 1, 1),
                                block_threads: DimensionsV2::new(shape, 1, 1),
                                dynamic_lds_bytes: 0,
                                dynamic_lds_alignment: 1,
                            },
                            &limits(),
                        );
                        assert_eq!(
                            actual.is_ok(),
                            expected_launch,
                            "minimum={minimum} maximum={maximum} shape={shape} full={require_full_waves}"
                        );
                        request_cases += 1;
                    }
                }
            }
        }
    }
    assert_eq!(contract_cases, 9_504);
    assert_eq!(request_cases, 640_416);
}

#[test]
fn deterministic_randomized_launches_match_independent_oracle() {
    const CASES: u64 = 200_000;
    let candidate = oracle_family();
    candidate.validate(&limits()).unwrap();
    let alignments = [0, 1, 2, 3, 4, 8, 16, 32, 256, 512];
    let mut rng = DeterministicRng(0x8250_3038_0000_0001);
    let mut accepted = 0_u64;
    for case in 0..CASES {
        let request = LaunchRequestV2 {
            grid_blocks: DimensionsV2::new(rng.below(1_100), rng.below(140), rng.below(40)),
            block_threads: DimensionsV2::new(rng.below(20), rng.below(12), rng.below(7)),
            dynamic_lds_bytes: rng.below(5_000),
            dynamic_lds_alignment: alignments[(rng.next() as usize) % alignments.len()],
        };
        let expected = independent_launch_oracle(&candidate.variants[0], request);
        let actual = candidate.validate_launch("oracle", request, &limits());
        assert_eq!(
            actual.as_ref().ok(),
            expected.as_ref(),
            "case={case} request={request:?}"
        );
        accepted += u64::from(actual.is_ok());
    }
    assert_eq!(CASES, 200_000);
    assert!(accepted > 100);
    assert!(accepted < CASES);
}

#[test]
fn hostile_decoder_campaign_is_bounded_panic_free_and_canonical() {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    const CASES: u64 = 160_000;
    let limits = limits();
    let canonical = encode_launch_kernel_family_v2(&family(), &limits).unwrap();
    let mut rng = DeterministicRng(0x5330_3882_9420_0001);
    let mut accepted = 0_u64;

    for case in 0..CASES {
        let input = if case % 1_024 == 0 {
            canonical.clone()
        } else {
            match case % 4 {
                0 => {
                    let length = (rng.next() as usize) % (canonical.len() + 1);
                    canonical[..length].to_vec()
                }
                1 => {
                    let mut bytes = canonical.clone();
                    let mutations = 1 + (rng.next() % 4);
                    for _ in 0..mutations {
                        let offset = (rng.next() as usize) % bytes.len();
                        bytes[offset] ^= (rng.next() as u8).max(1);
                    }
                    bytes
                }
                2 => {
                    let length = (rng.next() as usize) % 513;
                    (0..length).map(|_| rng.next() as u8).collect()
                }
                _ => {
                    let mut bytes = canonical.clone();
                    let count = rng.next() as u16;
                    if rng.next() & 1 == 0 {
                        bytes[16..18].copy_from_slice(&count.to_le_bytes());
                    } else {
                        bytes[18..20].copy_from_slice(&count.to_le_bytes());
                    }
                    bytes
                }
            }
        };
        let decoded = catch_unwind(AssertUnwindSafe(|| {
            decode_launch_kernel_family_v2(&input, &limits)
        }))
        .unwrap_or_else(|_| panic!("decoder panicked for hostile case {case}"));
        if let Ok(value) = decoded {
            value.validate(&limits).unwrap();
            assert_eq!(
                encode_launch_kernel_family_v2(&value, &limits).unwrap(),
                input,
                "accepted non-canonical case {case}"
            );
            accepted += 1;
        }
    }
    assert_eq!(CASES, 160_000);
    assert!(accepted >= CASES / 1_024);

    let oversized = vec![0_u8; limits.max_encoded_bytes + 1];
    assert!(matches!(
        decode_launch_kernel_family_v2(&oversized, &limits),
        Err(LaunchKernelDecodeErrorV2::ResourceLimit {
            resource: "encoded bytes",
            ..
        })
    ));
}
