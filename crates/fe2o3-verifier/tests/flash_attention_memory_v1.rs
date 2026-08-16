use fe2o3_verifier::{
    FLASH_ATTENTION_MEMORY_ACCESS_WIDTH_V1, FLASH_ATTENTION_MEMORY_GLOBAL_ADDRESS_SPACE_V1,
    FlashAttentionLogicalAccessV1, FlashAttentionMemoryBufferV1,
    FlashAttentionMemoryContractErrorV1, FlashAttentionMemoryEffectKindV1,
    FlashAttentionMemoryIdentitiesV1, FlashAttentionMemoryPhaseV1, FlashAttentionMemoryRegionsV1,
    FlashAttentionMemoryVerusExpectedEvidenceV1, check_flash_attention_memory_contract_v1,
    validate_flash_attention_logical_access_v1,
};

fn regions() -> FlashAttentionMemoryRegionsV1 {
    FlashAttentionMemoryRegionsV1 {
        query_base: 0x1000,
        key_base: 0x2000,
        value_base: 0x3000,
        output_base: 0x4000,
        query_bytes: 512,
        key_bytes: 512,
        value_bytes: 512,
        output_bytes: 512,
    }
}

fn causal_read() -> FlashAttentionLogicalAccessV1 {
    FlashAttentionLogicalAccessV1 {
        lane: 63,
        query_row: 7,
        key_row: Some(7),
        buffer: FlashAttentionMemoryBufferV1::Value,
        element_index: 127,
        address_space: FLASH_ATTENTION_MEMORY_GLOBAL_ADDRESS_SPACE_V1,
        byte_width: FLASH_ATTENTION_MEMORY_ACCESS_WIDTH_V1,
        phase: FlashAttentionMemoryPhaseV1::CausalRecurrence,
        kind: FlashAttentionMemoryEffectKindV1::Read,
    }
}

#[test]
fn exact_fixed_domain_is_bounded_ordered_and_single_writer() {
    let checked = check_flash_attention_memory_contract_v1(
        FlashAttentionMemoryIdentitiesV1::exact(),
        regions(),
    )
    .unwrap();
    assert!(checked.exhaustively_checks_fixed_source_index_bounds());
    assert!(checked.exhaustively_checks_fixed_source_output_disjointness());
    assert!(!checked.has_identity_bound_verus_receipt());
    assert!(!checked.proves_compiler_refinement());
    assert!(!checked.proves_isa_refinement());
    assert!(!checked.proves_logical_to_machine_address_refinement());
    assert!(!checked.proves_machine_memory_safety());
    assert!(!checked.proves_generalized_race_freedom());
    assert!(!checked.proves_gpu_execution());
}

#[test]
fn every_identity_axis_fails_closed() {
    for mutate in 0..6 {
        let mut identities = FlashAttentionMemoryIdentitiesV1::exact();
        match mutate {
            0 => identities.source[0] ^= 1,
            1 => identities.profile[0] ^= 1,
            2 => identities.kernel_ir[0] ^= 1,
            3 => identities.descriptor[0] ^= 1,
            4 => identities.launch[0] ^= 1,
            5 => identities.effects[0] ^= 1,
            _ => unreachable!(),
        }
        assert_eq!(
            check_flash_attention_memory_contract_v1(identities, regions()),
            Err(FlashAttentionMemoryContractErrorV1::Identity)
        );
    }
}

#[test]
fn extents_alignment_overflow_and_output_aliasing_fail_closed() {
    let mut candidate = regions();
    candidate.query_bytes = 508;
    assert_eq!(
        check_flash_attention_memory_contract_v1(
            FlashAttentionMemoryIdentitiesV1::exact(),
            candidate
        ),
        Err(FlashAttentionMemoryContractErrorV1::Extent)
    );
    let mut candidate = regions();
    candidate.output_base += 2;
    assert_eq!(
        check_flash_attention_memory_contract_v1(
            FlashAttentionMemoryIdentitiesV1::exact(),
            candidate
        ),
        Err(FlashAttentionMemoryContractErrorV1::Alignment)
    );
    let mut candidate = regions();
    candidate.query_base = u64::MAX - 511;
    assert_eq!(
        check_flash_attention_memory_contract_v1(
            FlashAttentionMemoryIdentitiesV1::exact(),
            candidate
        ),
        Err(FlashAttentionMemoryContractErrorV1::AddressOverflow)
    );
    let mut candidate = regions();
    candidate.output_base = candidate.value_base + 256;
    assert_eq!(
        check_flash_attention_memory_contract_v1(
            FlashAttentionMemoryIdentitiesV1::exact(),
            candidate
        ),
        Err(FlashAttentionMemoryContractErrorV1::OutputAliasesInput)
    );
}

#[test]
fn lane_causal_extent_address_space_width_and_order_mutations_fail_closed() {
    let exact = causal_read();
    validate_flash_attention_logical_access_v1(exact).unwrap();
    for (candidate, error) in [
        (
            FlashAttentionLogicalAccessV1 { lane: 64, ..exact },
            FlashAttentionMemoryContractErrorV1::Lane,
        ),
        (
            FlashAttentionLogicalAccessV1 {
                query_row: 6,
                ..exact
            },
            FlashAttentionMemoryContractErrorV1::OutputOwnership,
        ),
        (
            FlashAttentionLogicalAccessV1 {
                key_row: Some(8),
                ..exact
            },
            FlashAttentionMemoryContractErrorV1::KeyOutsideCausalPrefix,
        ),
        (
            FlashAttentionLogicalAccessV1 {
                element_index: 128,
                ..exact
            },
            FlashAttentionMemoryContractErrorV1::Extent,
        ),
        (
            FlashAttentionLogicalAccessV1 {
                address_space: 3,
                ..exact
            },
            FlashAttentionMemoryContractErrorV1::AddressSpace,
        ),
        (
            FlashAttentionLogicalAccessV1 {
                byte_width: 8,
                ..exact
            },
            FlashAttentionMemoryContractErrorV1::AccessWidth,
        ),
        (
            FlashAttentionLogicalAccessV1 {
                phase: FlashAttentionMemoryPhaseV1::OwnedOutputCommit,
                ..exact
            },
            FlashAttentionMemoryContractErrorV1::EffectOrdering,
        ),
        (
            FlashAttentionLogicalAccessV1 {
                kind: FlashAttentionMemoryEffectKindV1::Write,
                ..exact
            },
            FlashAttentionMemoryContractErrorV1::EffectKind,
        ),
    ] {
        assert_eq!(
            validate_flash_attention_logical_access_v1(candidate),
            Err(error)
        );
    }
}

#[test]
fn output_mapping_mutations_fail_closed() {
    let exact = FlashAttentionLogicalAccessV1 {
        lane: 9,
        query_row: 1,
        key_row: None,
        buffer: FlashAttentionMemoryBufferV1::Output,
        element_index: 18,
        address_space: 1,
        byte_width: 4,
        phase: FlashAttentionMemoryPhaseV1::OwnedOutputCommit,
        kind: FlashAttentionMemoryEffectKindV1::Write,
    };
    validate_flash_attention_logical_access_v1(exact).unwrap();
    assert_eq!(
        validate_flash_attention_logical_access_v1(FlashAttentionLogicalAccessV1 {
            element_index: 20,
            ..exact
        }),
        Err(FlashAttentionMemoryContractErrorV1::OutputOwnership)
    );
}

#[test]
fn expected_verus_evidence_is_inert_and_does_not_upgrade_the_checker() {
    let checked = check_flash_attention_memory_contract_v1(
        FlashAttentionMemoryIdentitiesV1::exact(),
        regions(),
    )
    .unwrap();
    let expected = FlashAttentionMemoryVerusExpectedEvidenceV1::exact();

    assert_ne!(expected.proof_source, [0; 32]);
    assert_ne!(expected.published_machine_body, [0; 32]);
    assert_ne!(expected.analyzer_profile, [0; 32]);
    assert!(!checked.has_identity_bound_verus_receipt());
    assert!(!checked.proves_compiler_refinement());
    assert!(!checked.proves_isa_refinement());
    assert!(!checked.proves_logical_to_machine_address_refinement());
    assert!(!checked.proves_machine_memory_safety());
    assert!(!checked.proves_generalized_race_freedom());
    assert!(!checked.proves_gpu_execution());
}

#[test]
fn expected_verus_evidence_substitutions_do_not_equal_the_exact_descriptor() {
    let exact = FlashAttentionMemoryVerusExpectedEvidenceV1::exact();
    for mutate in 0..6 {
        let mut changed = exact;
        match mutate {
            0 => changed.proof_source[0] ^= 1,
            1 => changed.published_machine_body[0] ^= 1,
            2 => changed.analyzer_profile[0] ^= 1,
            3 => changed.verus_executable[0] ^= 1,
            4 => changed.verus_closure_manifest[0] ^= 1,
            5 => changed.transcript[0] ^= 1,
            _ => unreachable!(),
        }
        assert_ne!(changed, exact);
    }
}
