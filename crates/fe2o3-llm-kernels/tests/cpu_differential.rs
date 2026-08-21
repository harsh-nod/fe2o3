use fe2o3_llm_kernels::rope_kv::*;

const OWNER: KvOwnerIdentityV1 = KvOwnerIdentityV1([0x71; 16]);

fn generation(role: Qwen3ModelRoleV1) -> PageTableGenerationV1 {
    match role {
        Qwen3ModelRoleV1::Target8B => PageTableGenerationV1::Target(TargetPageTableGenerationV1 {
            pool_id: [0x18; 16],
            generation: 41,
        }),
        Qwen3ModelRoleV1::Draft06B => PageTableGenerationV1::Draft(DraftPageTableGenerationV1 {
            pool_id: [0x06; 16],
            generation: 73,
        }),
    }
}

fn table(
    role: Qwen3ModelRoleV1,
    context: ContextBucketV1,
    page: PageBucketV1,
    initialized: u32,
) -> Qwen3PageTableV1 {
    let page_tokens = u32::from(page.tokens());
    assert!(page_tokens <= context.tokens());
    assert_eq!(context.tokens() % page_tokens, 0);
    let count = context.tokens() / page_tokens;
    let physical_base = match role {
        Qwen3ModelRoleV1::Target8B => 4_000,
        Qwen3ModelRoleV1::Draft06B => 8_000,
    };
    let generation = generation(role);
    let entries = (0..count)
        .map(|logical_page| {
            let page_start = logical_page * page_tokens;
            PageTableEntryV1 {
                logical_page: logical_page as u16,
                physical_page: physical_base + (count - 1 - logical_page) * 3,
                physical_generation: generation.value(),
                initialized_tokens: initialized.saturating_sub(page_start).min(page_tokens) as u16,
                exclusive_owner: OWNER,
            }
        })
        .collect();
    Qwen3PageTableV1 {
        generation,
        context,
        page,
        entries,
    }
}

fn candidate(
    role: Qwen3ModelRoleV1,
    active_tokens: TokenBucketV1,
    context: ContextBucketV1,
    page: PageBucketV1,
) -> Qwen3RopeKvCandidateV1 {
    exact_qwen3_rope_kv_candidate_v1(role, SequenceBucketV1::S4, active_tokens, context, page)
}

#[test]
fn exact_target_and_draft_geometry_is_admitted() {
    assert_eq!(
        Qwen3ModelRoleV1::Target8B.geometry(),
        Qwen3RopeKvGeometryV1 {
            layers: 36,
            query_heads: 32,
            kv_heads: 8,
            head_dimension: 128,
            rotary_dimension: 128,
            gqa_group_size: 4,
        }
    );
    assert_eq!(
        Qwen3ModelRoleV1::Draft06B.geometry(),
        Qwen3RopeKvGeometryV1 {
            layers: 28,
            query_heads: 16,
            kv_heads: 8,
            head_dimension: 128,
            rotary_dimension: 128,
            gqa_group_size: 2,
        }
    );
}

#[test]
fn every_compatible_finite_bucket_candidate_is_canonical() {
    let roles = [Qwen3ModelRoleV1::Target8B, Qwen3ModelRoleV1::Draft06B];
    let sequences = [
        SequenceBucketV1::S1,
        SequenceBucketV1::S4,
        SequenceBucketV1::S16,
        SequenceBucketV1::S32,
    ];
    let tokens = [
        TokenBucketV1::T1,
        TokenBucketV1::T2,
        TokenBucketV1::T3,
        TokenBucketV1::T4,
        TokenBucketV1::T5,
        TokenBucketV1::T8,
        TokenBucketV1::T9,
        TokenBucketV1::T16,
        TokenBucketV1::T17,
        TokenBucketV1::T128,
        TokenBucketV1::T512,
        TokenBucketV1::T2048,
        TokenBucketV1::T8192,
    ];
    let contexts = [
        ContextBucketV1::C128,
        ContextBucketV1::C1024,
        ContextBucketV1::C4096,
        ContextBucketV1::C8192,
    ];
    let pages = [PageBucketV1::P16, PageBucketV1::P64, PageBucketV1::P256];

    let mut admitted = 0;
    for role in roles {
        for sequence in sequences {
            for token in tokens {
                for context in contexts {
                    for page in pages {
                        let candidate =
                            exact_qwen3_rope_kv_candidate_v1(role, sequence, token, context, page);
                        let compatible = token.tokens() <= context.tokens()
                            && context.tokens() >= u32::from(page.tokens())
                            && context.tokens() % u32::from(page.tokens()) == 0;
                        assert_eq!(
                            validate_qwen3_rope_kv_candidate_v1(&candidate).is_ok(),
                            compatible
                        );
                        admitted += usize::from(compatible);
                    }
                }
            }
        }
    }
    assert_eq!(admitted, 1_024);
}

#[test]
fn split_half_pair_is_total_involutive_and_non_self() {
    for dimension in 0..QWEN3_HEAD_DIMENSION_V1 {
        let pair = qwen3_rotary_pair_v1(dimension).unwrap();
        assert!(pair < QWEN3_HEAD_DIMENSION_V1);
        assert_ne!(pair, dimension);
        assert_eq!(qwen3_rotary_pair_v1(pair), Some(dimension));
    }
    assert_eq!(qwen3_rotary_pair_v1(QWEN3_HEAD_DIMENSION_V1), None);
}

#[test]
fn rope_pair_candidate_matches_dimension_reference_for_both_models() {
    for role in [Qwen3ModelRoleV1::Target8B, Qwen3ModelRoleV1::Draft06B] {
        let candidate = candidate(
            role,
            TokenBucketV1::T4,
            ContextBucketV1::C8192,
            PageBucketV1::P16,
        );
        let positions = [0, 1, 127, 8_191];
        let query_len = 4 * usize::from(candidate.geometry.query_heads) * 128;
        let key_len = 4 * usize::from(candidate.geometry.kv_heads) * 128;
        let query: Vec<_> = (0..query_len)
            .map(|index| ((index * 17 % 251) as f64 - 125.0) / 37.0)
            .collect();
        let key: Vec<_> = (0..key_len)
            .map(|index| ((index * 29 % 257) as f64 - 128.0) / 43.0)
            .collect();

        let reference = qwen3_rope_reference_v1(&candidate, &positions, &query, &key).unwrap();
        let pair = qwen3_rope_pair_candidate_v1(&candidate, &positions, &query, &key).unwrap();
        assert_eq!(reference, pair);
        assert_eq!(
            &reference.query[..usize::from(candidate.geometry.query_heads) * 128],
            &query[..usize::from(candidate.geometry.query_heads) * 128]
        );
        assert_eq!(
            &reference.key[..usize::from(candidate.geometry.kv_heads) * 128],
            &key[..usize::from(candidate.geometry.kv_heads) * 128]
        );
    }
}

#[test]
fn paged_coordinates_match_independent_physical_oracle_across_boundary() {
    for role in [Qwen3ModelRoleV1::Target8B, Qwen3ModelRoleV1::Draft06B] {
        let candidate = candidate(
            role,
            TokenBucketV1::T2,
            ContextBucketV1::C128,
            PageBucketV1::P16,
        );
        let table = table(role, ContextBucketV1::C128, PageBucketV1::P16, 15);
        let expected_generation = generation(role);
        let descriptor = Qwen3KvWriteDescriptorV1 {
            candidate,
            generation: expected_generation,
            owner: OWNER,
            sequence_index: 3,
            layer: candidate.geometry.layers - 1,
            logical_start: 15,
        };
        let expectation = Qwen3KvWriteExpectationV1 {
            candidate,
            generation: expected_generation,
            owner: OWNER,
        };
        validate_qwen3_kv_write_v1(&descriptor, &table, &expectation).unwrap();

        let extent = 2 * 8 * 128;
        let rotated_key: Vec<_> = (0..extent).map(|index| index as f64 / 19.0).collect();
        let value: Vec<_> = (0..extent).map(|index| -(index as f64) / 23.0).collect();
        let write_reference = qwen3_paged_kv_write_reference_v1(
            &descriptor,
            &table,
            &expectation,
            &rotated_key,
            &value,
        )
        .unwrap();
        assert_eq!(write_reference.elements.len(), extent);
        let mut written_offsets = std::collections::BTreeSet::new();
        for (index, element) in write_reference.elements.iter().enumerate() {
            assert_eq!(element.rotated_key, rotated_key[index]);
            assert_eq!(element.value, value[index]);
            assert!(written_offsets.insert(element.coordinate.pool_element_offset));
        }

        for local_token in 0..2 {
            for kv_head in 0..candidate.geometry.kv_heads {
                for component in 0..candidate.geometry.head_dimension {
                    let coordinate = qwen3_kv_write_coordinate_v1(
                        &descriptor,
                        &table,
                        &expectation,
                        local_token,
                        kv_head,
                        component,
                    )
                    .unwrap();
                    let logical_token = 15 + local_token;
                    let entry = &table.entries[(logical_token / 16) as usize];
                    let slot = logical_token % 16;
                    let independent_offset = ((u64::from(entry.physical_page) * 16
                        + u64::from(slot))
                        * u64::from(candidate.geometry.kv_heads)
                        + u64::from(kv_head))
                        * 128
                        + u64::from(component);
                    assert_eq!(coordinate.pool_element_offset, independent_offset);
                    assert_eq!(coordinate.logical_token, logical_token);
                    assert_eq!(coordinate.location.token_slot, slot as u16);
                }
            }
        }

        let projected = project_qwen3_kv_write_v1(&descriptor, &table, &expectation).unwrap();
        assert_eq!(projected.initialized_prefix_tokens().unwrap(), 17);
        assert_eq!(projected.entries[0].initialized_tokens, 16);
        assert_eq!(projected.entries[1].initialized_tokens, 1);
        for (before, after) in table.entries.iter().zip(&projected.entries) {
            assert_eq!(before.logical_page, after.logical_page);
            assert_eq!(before.physical_page, after.physical_page);
            assert_eq!(before.physical_generation, after.physical_generation);
            assert_eq!(before.exclusive_owner, after.exclusive_owner);
        }
    }
}

#[test]
fn every_page_bucket_maps_full_context_bijectively() {
    for (context, page) in [
        (ContextBucketV1::C128, PageBucketV1::P16),
        (ContextBucketV1::C1024, PageBucketV1::P64),
        (ContextBucketV1::C8192, PageBucketV1::P256),
    ] {
        let table = table(Qwen3ModelRoleV1::Target8B, context, page, context.tokens());
        let page_tokens = u32::from(page.tokens());
        let mut physical = std::collections::BTreeSet::new();
        for logical_token in 0..context.tokens() {
            let location = table
                .initialized_logical_to_physical(logical_token)
                .unwrap();
            assert_eq!(location.token_slot, (logical_token % page_tokens) as u16);
            assert!(physical.insert((location.physical_page, location.token_slot)));
        }
        assert_eq!(physical.len(), context.tokens() as usize);
    }
}

#[test]
fn target_and_draft_generation_namespaces_are_explicit_and_disjoint() {
    let target = match generation(Qwen3ModelRoleV1::Target8B) {
        PageTableGenerationV1::Target(value) => value,
        PageTableGenerationV1::Draft(_) => unreachable!(),
    };
    let draft = match generation(Qwen3ModelRoleV1::Draft06B) {
        PageTableGenerationV1::Draft(value) => value,
        PageTableGenerationV1::Target(_) => unreachable!(),
    };
    validate_qwen3_page_table_generations_v1(Qwen3PageTableGenerationsV1 { target, draft })
        .unwrap();
    assert_ne!(target.pool_id, draft.pool_id);
}
