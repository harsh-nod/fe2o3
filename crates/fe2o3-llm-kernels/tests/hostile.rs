use fe2o3_llm_kernels::rope_kv::*;

const OWNER: KvOwnerIdentityV1 = KvOwnerIdentityV1([0xa5; 16]);

fn target_generation() -> PageTableGenerationV1 {
    PageTableGenerationV1::Target(TargetPageTableGenerationV1 {
        pool_id: [0x81; 16],
        generation: 19,
    })
}

fn draft_generation() -> PageTableGenerationV1 {
    PageTableGenerationV1::Draft(DraftPageTableGenerationV1 {
        pool_id: [0x28; 16],
        generation: 23,
    })
}

fn canonical_candidate() -> Qwen3RopeKvCandidateV1 {
    exact_qwen3_rope_kv_candidate_v1(
        Qwen3ModelRoleV1::Target8B,
        SequenceBucketV1::S4,
        TokenBucketV1::T2,
        ContextBucketV1::C128,
        PageBucketV1::P16,
    )
}

fn canonical_table(initialized: u32) -> Qwen3PageTableV1 {
    let generation = target_generation();
    Qwen3PageTableV1 {
        generation,
        context: ContextBucketV1::C128,
        page: PageBucketV1::P16,
        entries: (0..8)
            .map(|logical_page| PageTableEntryV1 {
                logical_page,
                physical_page: 900 + u32::from(7 - logical_page) * 2,
                physical_generation: generation.value(),
                initialized_tokens: initialized
                    .saturating_sub(u32::from(logical_page) * 16)
                    .min(16) as u16,
                exclusive_owner: OWNER,
            })
            .collect(),
    }
}

fn descriptor() -> Qwen3KvWriteDescriptorV1 {
    Qwen3KvWriteDescriptorV1 {
        candidate: canonical_candidate(),
        generation: target_generation(),
        owner: OWNER,
        sequence_index: 0,
        layer: 0,
        logical_start: 15,
    }
}

fn expectation(
    candidate: Qwen3RopeKvCandidateV1,
    generation: PageTableGenerationV1,
    owner: KvOwnerIdentityV1,
) -> Qwen3KvWriteExpectationV1 {
    Qwen3KvWriteExpectationV1 {
        candidate,
        generation,
        owner,
    }
}

fn assert_candidate_noncanonical(candidate: Qwen3RopeKvCandidateV1) {
    assert_eq!(
        validate_qwen3_rope_kv_candidate_v1(&candidate),
        Err(CandidateErrorV1::NonCanonical)
    );
}

#[test]
fn identity_processor_geometry_and_policy_drift_are_rejected() {
    let mut mutated = canonical_candidate();
    mutated.family_id[0] ^= 1;
    assert_candidate_noncanonical(mutated);

    let mut mutated = canonical_candidate();
    mutated.candidate_schema_id[31] ^= 1;
    assert_candidate_noncanonical(mutated);

    let mut mutated = canonical_candidate();
    mutated.schedule_id[15] ^= 1;
    assert_candidate_noncanonical(mutated);

    let mut mutated = canonical_candidate();
    mutated.processor = "gfx950";
    assert_candidate_noncanonical(mutated);

    let mut mutated = canonical_candidate();
    mutated.target_features = "+wavefrontsize32,+xnack";
    assert_candidate_noncanonical(mutated);

    let mut mutated = canonical_candidate();
    mutated.geometry.query_heads = 16;
    assert_candidate_noncanonical(mutated);

    let mut mutated = canonical_candidate();
    mutated.geometry.gqa_group_size = 2;
    assert_candidate_noncanonical(mutated);

    let mut mutated = canonical_candidate();
    mutated.frequency.theta = 10_000;
    assert_candidate_noncanonical(mutated);
}

#[test]
fn effect_race_resource_and_authority_drift_are_rejected() {
    let mut mutated = canonical_candidate();
    mutated.effects.swap(0, 1);
    assert_candidate_noncanonical(mutated);

    let mut mutated = canonical_candidate();
    mutated.effects[6].requires_exclusive_owner = false;
    assert_candidate_noncanonical(mutated);

    let mut mutated = canonical_candidate();
    mutated.race.physical_pages_unique = false;
    assert_candidate_noncanonical(mutated);

    let mut mutated = canonical_candidate();
    mutated.race.atomics = 1;
    assert_candidate_noncanonical(mutated);

    let mut mutated = canonical_candidate();
    mutated.resources.max_context_tokens = u32::MAX;
    assert_candidate_noncanonical(mutated);

    let mut mutated = canonical_candidate();
    mutated.resources.wave_width = 32;
    assert_candidate_noncanonical(mutated);

    let mut mutated = canonical_candidate();
    mutated.authority.artifact_authority = true;
    assert_candidate_noncanonical(mutated);

    let mut mutated = canonical_candidate();
    mutated.authority.load_authority = true;
    assert_candidate_noncanonical(mutated);

    let mut mutated = canonical_candidate();
    mutated.authority.launch_authority = true;
    assert_candidate_noncanonical(mutated);

    let mut mutated = canonical_candidate();
    mutated.authority.kv_system_refinement = true;
    assert_candidate_noncanonical(mutated);
}

#[test]
fn incompatible_bucket_cross_products_fail_closed() {
    let tokens_too_large = exact_qwen3_rope_kv_candidate_v1(
        Qwen3ModelRoleV1::Draft06B,
        SequenceBucketV1::S1,
        TokenBucketV1::T8192,
        ContextBucketV1::C128,
        PageBucketV1::P16,
    );
    assert_eq!(
        validate_qwen3_rope_kv_candidate_v1(&tokens_too_large),
        Err(CandidateErrorV1::TokensExceedContext)
    );
    let page_too_large = exact_qwen3_rope_kv_candidate_v1(
        Qwen3ModelRoleV1::Draft06B,
        SequenceBucketV1::S1,
        TokenBucketV1::T1,
        ContextBucketV1::C128,
        PageBucketV1::P256,
    );
    assert_eq!(
        validate_qwen3_rope_kv_candidate_v1(&page_too_large),
        Err(CandidateErrorV1::PageDoesNotDivideContext)
    );
}

#[test]
fn page_table_shape_alias_freshness_and_prefix_drift_are_rejected() {
    let mut mutated = canonical_table(15);
    mutated.entries.pop();
    assert_eq!(
        mutated.validate_against(target_generation(), OWNER),
        Err(PageTableErrorV1::EntryCount)
    );

    let mut mutated = canonical_table(15);
    mutated.entries[1].logical_page = 2;
    assert_eq!(
        mutated.validate_against(target_generation(), OWNER),
        Err(PageTableErrorV1::LogicalPageOrder)
    );

    let mut mutated = canonical_table(15);
    mutated.entries[1].physical_page = mutated.entries[0].physical_page;
    assert_eq!(
        mutated.validate_against(target_generation(), OWNER),
        Err(PageTableErrorV1::DuplicatePhysicalPage)
    );

    let mut mutated = canonical_table(15);
    mutated.entries[0].physical_page = M1_MAX_PHYSICAL_PAGES_V1;
    assert_eq!(
        mutated.validate_against(target_generation(), OWNER),
        Err(PageTableErrorV1::PhysicalPageOutOfBounds)
    );

    let mut mutated = canonical_table(15);
    mutated.entries[0].physical_generation += 1;
    assert_eq!(
        mutated.validate_against(target_generation(), OWNER),
        Err(PageTableErrorV1::StalePhysicalGeneration)
    );

    let mut mutated = canonical_table(15);
    mutated.entries[0].initialized_tokens = 17;
    assert_eq!(
        mutated.validate_against(target_generation(), OWNER),
        Err(PageTableErrorV1::InitializedOutOfBounds)
    );

    let mut mutated = canonical_table(15);
    mutated.entries[1].initialized_tokens = 1;
    assert_eq!(
        mutated.validate_against(target_generation(), OWNER),
        Err(PageTableErrorV1::NonPrefixInitialization)
    );
}

#[test]
fn missing_stale_and_cross_role_generation_or_owner_are_rejected() {
    let mut mutated = canonical_table(15);
    mutated.generation = PageTableGenerationV1::Target(TargetPageTableGenerationV1 {
        pool_id: [0; 16],
        generation: 0,
    });
    assert_eq!(
        mutated.validate_against(mutated.generation, OWNER),
        Err(PageTableErrorV1::MissingGeneration)
    );

    assert_eq!(
        canonical_table(15).validate_against(draft_generation(), OWNER),
        Err(PageTableErrorV1::StaleGeneration)
    );
    assert_eq!(
        canonical_table(15).validate_against(target_generation(), KvOwnerIdentityV1([0; 16])),
        Err(PageTableErrorV1::MissingOwner)
    );

    let mut mutated = canonical_table(15);
    mutated.entries[0].exclusive_owner = KvOwnerIdentityV1([0x44; 16]);
    assert_eq!(
        mutated.validate_against(target_generation(), OWNER),
        Err(PageTableErrorV1::StaleOwner)
    );
}

#[test]
fn target_and_draft_pools_must_be_present_and_disjoint() {
    let target = TargetPageTableGenerationV1 {
        pool_id: [0x11; 16],
        generation: 1,
    };
    let draft = DraftPageTableGenerationV1 {
        pool_id: [0x22; 16],
        generation: 1,
    };
    assert!(
        validate_qwen3_page_table_generations_v1(Qwen3PageTableGenerationsV1 { target, draft })
            .is_ok()
    );
    assert_eq!(
        validate_qwen3_page_table_generations_v1(Qwen3PageTableGenerationsV1 {
            target,
            draft: DraftPageTableGenerationV1 {
                pool_id: target.pool_id,
                generation: 1,
            },
        }),
        Err(GenerationPairErrorV1::AliasedPoolIdentity)
    );
    assert_eq!(
        validate_qwen3_page_table_generations_v1(Qwen3PageTableGenerationsV1 {
            target,
            draft: DraftPageTableGenerationV1 {
                pool_id: [0; 16],
                generation: 0,
            },
        }),
        Err(GenerationPairErrorV1::Missing)
    );
}

#[test]
fn descriptor_identity_role_bounds_and_append_drift_are_rejected() {
    let table = canonical_table(15);
    let expected = canonical_candidate();

    let mut mutated = descriptor();
    mutated.generation = draft_generation();
    assert_eq!(
        validate_qwen3_kv_write_v1(
            &mutated,
            &table,
            &expectation(expected, draft_generation(), OWNER),
        ),
        Err(KvWriteErrorV1::RoleGenerationMismatch)
    );

    let mut mutated = descriptor();
    mutated.owner = KvOwnerIdentityV1([0x33; 16]);
    assert_eq!(
        validate_qwen3_kv_write_v1(
            &mutated,
            &table,
            &expectation(expected, target_generation(), OWNER),
        ),
        Err(KvWriteErrorV1::OwnerMismatch)
    );

    let mut mutated = descriptor();
    mutated.sequence_index = 4;
    assert_eq!(
        validate_qwen3_kv_write_v1(
            &mutated,
            &table,
            &expectation(expected, target_generation(), OWNER),
        ),
        Err(KvWriteErrorV1::SequenceOutOfBounds)
    );

    let mut mutated = descriptor();
    mutated.layer = 36;
    assert_eq!(
        validate_qwen3_kv_write_v1(
            &mutated,
            &table,
            &expectation(expected, target_generation(), OWNER),
        ),
        Err(KvWriteErrorV1::LayerOutOfBounds)
    );

    let mut mutated = descriptor();
    mutated.logical_start = 14;
    assert_eq!(
        validate_qwen3_kv_write_v1(
            &mutated,
            &table,
            &expectation(expected, target_generation(), OWNER),
        ),
        Err(KvWriteErrorV1::NonAppendWrite)
    );

    let wrong_expected = exact_qwen3_rope_kv_candidate_v1(
        Qwen3ModelRoleV1::Target8B,
        SequenceBucketV1::S4,
        TokenBucketV1::T1,
        ContextBucketV1::C128,
        PageBucketV1::P16,
    );
    assert_eq!(
        validate_qwen3_kv_write_v1(
            &descriptor(),
            &table,
            &expectation(wrong_expected, target_generation(), OWNER),
        ),
        Err(KvWriteErrorV1::Candidate(CandidateErrorV1::NonCanonical))
    );
}

#[test]
fn descriptor_context_overflow_and_coordinate_bounds_are_rejected() {
    let full_table = canonical_table(128);
    let mut overflow = descriptor();
    overflow.logical_start = 128;
    assert_eq!(
        validate_qwen3_kv_write_v1(
            &overflow,
            &full_table,
            &expectation(canonical_candidate(), target_generation(), OWNER),
        ),
        Err(KvWriteErrorV1::WriteExceedsContext)
    );

    let table = canonical_table(15);
    for (local_token, kv_head, component) in [(2, 0, 0), (0, 8, 0), (0, 0, 128)] {
        assert_eq!(
            qwen3_kv_write_coordinate_v1(
                &descriptor(),
                &table,
                &expectation(canonical_candidate(), target_generation(), OWNER),
                local_token,
                kv_head,
                component,
            ),
            Err(KvWriteErrorV1::CoordinateOutOfBounds)
        );
    }
}

#[test]
fn paged_write_value_extent_and_finiteness_drift_are_rejected() {
    let table = canonical_table(15);
    let expectation = expectation(canonical_candidate(), target_generation(), OWNER);
    let key = vec![0.25; 2 * 8 * 128];
    let mut value = vec![-0.5; 2 * 8 * 128];
    assert_eq!(
        qwen3_paged_kv_write_reference_v1(
            &descriptor(),
            &table,
            &expectation,
            &key[..key.len() - 1],
            &value,
        ),
        Err(KvWriteErrorV1::InputExtent)
    );
    value[7] = f64::INFINITY;
    assert_eq!(
        qwen3_paged_kv_write_reference_v1(&descriptor(), &table, &expectation, &key, &value,),
        Err(KvWriteErrorV1::NonFiniteInput {
            index: key.len() + 7,
        })
    );
}

#[test]
fn uninitialized_reads_fail_closed() {
    let table = canonical_table(15);
    assert!(table.initialized_logical_to_physical(14).is_ok());
    assert_eq!(
        table.initialized_logical_to_physical(15),
        Err(PageTableErrorV1::UninitializedRead)
    );
    assert_eq!(
        table.logical_to_physical(128),
        Err(PageTableErrorV1::LogicalTokenOutOfBounds)
    );
}

#[test]
fn rope_shape_position_and_finiteness_drift_are_rejected() {
    let candidate = canonical_candidate();
    let query_extent = 2 * 32 * 128;
    let key_extent = 2 * 8 * 128;
    let query = vec![0.25; query_extent];
    let key = vec![-0.5; key_extent];

    assert_eq!(
        qwen3_rope_reference_v1(&candidate, &[0], &query, &key),
        Err(RopeReferenceErrorV1::PositionCount)
    );
    assert_eq!(
        qwen3_rope_reference_v1(&candidate, &[0, 1], &query[..query.len() - 1], &key),
        Err(RopeReferenceErrorV1::QueryExtent)
    );
    assert_eq!(
        qwen3_rope_reference_v1(&candidate, &[0, 1], &query, &key[..key.len() - 1]),
        Err(RopeReferenceErrorV1::KeyExtent)
    );
    assert_eq!(
        qwen3_rope_reference_v1(&candidate, &[0, 8_192], &query, &key),
        Err(RopeReferenceErrorV1::PositionOutOfBounds { token: 1 })
    );
    let mut nonfinite = query.clone();
    nonfinite[17] = f64::NAN;
    assert_eq!(
        qwen3_rope_reference_v1(&candidate, &[0, 1], &nonfinite, &key),
        Err(RopeReferenceErrorV1::NonFiniteInput { index: 17 })
    );
}
