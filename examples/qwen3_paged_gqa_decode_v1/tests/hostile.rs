mod common;

use common::{candidate, data, metadata};
use fe2o3_qwen3_paged_gqa_decode_v1::*;

fn target_decode() -> StructuralPagedGqaDecodeCandidateV1 {
    candidate(
        Qwen3AttentionRoleV1::Target8B,
        B3PagedDecodeBucketV1::DecodeS1C8192,
    )
}

#[test]
fn cross_request_page_alias_is_rejected_but_bijections_are_allowed() {
    let batch = candidate(
        Qwen3AttentionRoleV1::Target8B,
        B3PagedDecodeBucketV1::DecodeS8C8192,
    );
    for fragmented in [false, true] {
        let exact = metadata(batch, 0, fragmented);
        assert!(validate_paged_kv_metadata_v1(batch.profile(), &exact).is_ok());
        let mut alias = exact.clone();
        alias.entries[M1_PAGES_PER_REQUEST_V1].physical_page = alias.entries[0].physical_page;
        assert_eq!(
            validate_paged_kv_metadata_v1(batch.profile(), &alias),
            Err(PagedKvMetadataErrorV1::PhysicalPageAlias)
        );
        let mut swapped = exact.clone();
        swapped.entries[0].physical_page = exact.entries[M1_PAGES_PER_REQUEST_V1].physical_page;
        swapped.entries[M1_PAGES_PER_REQUEST_V1].physical_page = exact.entries[0].physical_page;
        assert!(validate_paged_kv_metadata_v1(batch.profile(), &swapped).is_ok());
    }
}

#[test]
fn resident_capacity_and_physical_extents_are_exact() {
    let batch = target_decode();
    let capacity = M1_CONTEXT_CAPACITY_TOKENS_V1;
    let active = batch.profile().descriptor().active_tokens;
    let exact = metadata(batch, capacity - active, true);
    assert!(validate_paged_kv_metadata_v1(batch.profile(), &exact).is_ok());
    for pages in [0, exact.physical_pages - 1, exact.physical_pages + 1] {
        let mut mutated = exact.clone();
        mutated.physical_pages = pages;
        assert_eq!(
            validate_paged_kv_metadata_v1(batch.profile(), &mutated),
            Err(PagedKvMetadataErrorV1::PhysicalPageCount)
        );
    }
    let mut mutated = exact.clone();
    mutated.context_capacity_tokens = capacity + 1;
    assert_eq!(
        validate_paged_kv_metadata_v1(batch.profile(), &mutated),
        Err(PagedKvMetadataErrorV1::BucketMismatch)
    );
    let mut mutated = exact.clone();
    mutated.page_tokens = M1_KV_PAGE_TOKENS_V1 + 1;
    assert_eq!(
        validate_paged_kv_metadata_v1(batch.profile(), &mutated),
        Err(PagedKvMetadataErrorV1::BucketMismatch)
    );
    let over = metadata(batch, capacity - active + 1, true);
    assert_eq!(
        validate_paged_kv_metadata_v1(batch.profile(), &over),
        Err(PagedKvMetadataErrorV1::ContextBounds)
    );
    let mut mutated = exact;
    mutated.requests[0].resident_tokens += 1;
    assert_eq!(
        validate_paged_kv_metadata_v1(batch.profile(), &mutated),
        Err(PagedKvMetadataErrorV1::ResidentBoundary)
    );
}

#[test]
fn stale_request_generation_page_index_and_order_reject() {
    let candidate = target_decode();
    let exact = metadata(candidate, 18, true);

    let mut mutated = exact.clone();
    mutated.entries[0].physical_generation += 1;
    assert_eq!(
        validate_paged_kv_metadata_v1(candidate.profile(), &mutated),
        Err(PagedKvMetadataErrorV1::StaleGeneration)
    );
    let mut mutated = exact.clone();
    mutated.entries[0].request_id = PagedKvRequestIdV1([0x77; 16]);
    assert_eq!(
        validate_paged_kv_metadata_v1(candidate.profile(), &mutated),
        Err(PagedKvMetadataErrorV1::StaleRequest)
    );
    let mut mutated = exact.clone();
    mutated.entries[0].physical_page = u32::try_from(mutated.physical_pages).unwrap();
    assert_eq!(
        validate_paged_kv_metadata_v1(candidate.profile(), &mutated),
        Err(PagedKvMetadataErrorV1::PhysicalPageOutOfBounds)
    );
    let mut mutated = exact.clone();
    mutated.entries[1].physical_page = mutated.entries[0].physical_page;
    assert_eq!(
        validate_paged_kv_metadata_v1(candidate.profile(), &mutated),
        Err(PagedKvMetadataErrorV1::PhysicalPageAlias)
    );
    let mut mutated = exact;
    mutated.entries[1].logical_page = 2;
    assert_eq!(
        validate_paged_kv_metadata_v1(candidate.profile(), &mutated),
        Err(PagedKvMetadataErrorV1::LogicalPageOrder)
    );
}

#[test]
fn read_past_committed_is_limited_to_exact_active_suffix_and_final_page_tail() {
    let candidate = target_decode();
    let exact = metadata(candidate, 18, true);
    assert_eq!(exact.requests[0].resident_tokens, 19);
    assert_eq!(exact.entries[0].initialized_tokens, 16);
    assert_eq!(exact.entries[0].initialized_mask, u16::MAX);
    assert_eq!(exact.entries[1].initialized_tokens, 3);
    assert_eq!(exact.entries[1].initialized_mask, 0b111);
    assert!(validate_paged_kv_metadata_v1(candidate.profile(), &exact).is_ok());
    assert_eq!(
        paged_kv_physical_token_v1(candidate.profile(), &exact, 0, 18),
        Some((510, 2))
    );
    assert_eq!(
        paged_kv_physical_token_v1(candidate.profile(), &exact, 0, 19),
        None
    );

    let mut mutated = exact.clone();
    mutated.requests[0].resident_tokens += 1;
    assert_eq!(
        validate_paged_kv_metadata_v1(candidate.profile(), &mutated),
        Err(PagedKvMetadataErrorV1::ResidentBoundary)
    );
    let mut mutated = exact.clone();
    mutated.requests[0].committed_tokens -= 1;
    assert_eq!(
        validate_paged_kv_metadata_v1(candidate.profile(), &mutated),
        Err(PagedKvMetadataErrorV1::ResidentBoundary)
    );
    let mut mutated = exact.clone();
    mutated.entries[1].initialized_tokens = 4;
    assert_eq!(
        validate_paged_kv_metadata_v1(candidate.profile(), &mutated),
        Err(PagedKvMetadataErrorV1::InitializedPrefix)
    );
    let mut mutated = exact;
    mutated.entries[1].initialized_mask |= 1 << 3;
    assert_eq!(
        validate_paged_kv_metadata_v1(candidate.profile(), &mutated),
        Err(PagedKvMetadataErrorV1::InitializedMask)
    );
}

#[test]
fn role_allocation_request_and_cross_request_alias_mutations_reject() {
    let target = target_decode();
    let exact = metadata(target, 0, false);
    let mut mutated = exact.clone();
    mutated.role = Qwen3AttentionRoleV1::Draft06B;
    assert_eq!(
        validate_paged_kv_metadata_v1(target.profile(), &mutated),
        Err(PagedKvMetadataErrorV1::RoleMismatch)
    );
    let mut mutated = exact.clone();
    mutated.key_allocation = mutated.value_allocation;
    assert_eq!(
        validate_paged_kv_metadata_v1(target.profile(), &mutated),
        Err(PagedKvMetadataErrorV1::AllocationAlias)
    );
    let mut mutated = exact;
    mutated.requests[0].generation = 0;
    assert_eq!(
        validate_paged_kv_metadata_v1(target.profile(), &mutated),
        Err(PagedKvMetadataErrorV1::MissingGeneration)
    );

    let batch = candidate(
        Qwen3AttentionRoleV1::Target8B,
        B3PagedDecodeBucketV1::DecodeS8C8192,
    );
    let mut mutated = metadata(batch, 0, false);
    mutated.requests[1].request_id = mutated.requests[0].request_id;
    assert_eq!(
        validate_paged_kv_metadata_v1(batch.profile(), &mutated),
        Err(PagedKvMetadataErrorV1::RequestIdentity)
    );
}

#[test]
fn physical_slice_alias_and_wrong_extents_fail_before_publication() {
    let candidate = target_decode();
    let metadata = metadata(candidate, 0, false);
    let fixture = data(candidate, &metadata);
    let mut output = vec![Bf16V1::from_bits(0x3f80); fixture.query.len()];
    let before = output.clone();
    let error = qwen3_paged_gqa_decode_reference_v1(
        candidate,
        &metadata,
        PagedGqaInputV1 {
            query: &fixture.query,
            key: &fixture.key,
            value: &fixture.key,
        },
        &mut output,
    );
    assert_eq!(error, Err(PagedGqaReferenceErrorV1::KeyValuePhysicalAlias));
    assert_eq!(output, before);

    let error = qwen3_paged_gqa_decode_reference_v1(
        candidate,
        &metadata,
        PagedGqaInputV1 {
            query: &fixture.query[..fixture.query.len() - 1],
            key: &fixture.key,
            value: &fixture.value,
        },
        &mut output,
    );
    assert!(matches!(
        error,
        Err(PagedGqaReferenceErrorV1::WrongLength {
            tensor: PagedGqaTensorV1::Query,
            ..
        })
    ));
    assert_eq!(output, before);
}

#[test]
fn logically_read_nonfinite_rejects_but_uninitialized_tail_is_masked() {
    let candidate = target_decode();
    let metadata = metadata(candidate, 18, true);
    let mut fixture = data(candidate, &metadata);
    let mut output = vec![Bf16V1::from_bits(0x3f80); fixture.query.len()];
    let before = output.clone();
    let read_index = common::physical_index(candidate, &metadata, 0, 18, 0, 0);
    fixture.key[read_index] = Bf16V1::from_bits(0x7fc1);
    assert!(matches!(
        qwen3_paged_gqa_decode_reference_v1(
            candidate,
            &metadata,
            PagedGqaInputV1 {
                query: &fixture.query,
                key: &fixture.key,
                value: &fixture.value,
            },
            &mut output,
        ),
        Err(PagedGqaReferenceErrorV1::NonFiniteInput {
            tensor: PagedGqaTensorV1::Key,
            ..
        })
    ));
    assert_eq!(output, before);

    fixture.key[read_index] = Bf16V1::default();
    let masked_index = common::physical_index(candidate, &metadata, 0, 19, 0, 0);
    fixture.key[masked_index] = Bf16V1::from_bits(0x7fc1);
    assert!(
        qwen3_paged_gqa_decode_reference_v1(
            candidate,
            &metadata,
            PagedGqaInputV1 {
                query: &fixture.query,
                key: &fixture.key,
                value: &fixture.value,
            },
            &mut output,
        )
        .is_ok()
    );
}
