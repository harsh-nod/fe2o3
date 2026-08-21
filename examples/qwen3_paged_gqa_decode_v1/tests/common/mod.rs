#![allow(dead_code)]

use fe2o3_qwen3_paged_gqa_decode_v1::*;

pub fn candidate(
    role: Qwen3AttentionRoleV1,
    bucket: B3PagedDecodeBucketV1,
) -> StructuralPagedGqaDecodeCandidateV1 {
    let profile = PagedGqaProfileDescriptorV1::canonical(role, bucket);
    admit_paged_gqa_decode_candidate_v1(PagedGqaCandidateDescriptorV1::canonical(profile))
        .expect("canonical candidate")
}

pub fn metadata(
    candidate: StructuralPagedGqaDecodeCandidateV1,
    committed_tokens: usize,
    fragmented: bool,
) -> PagedKvBatchMetadataV1 {
    let profile = candidate.profile().descriptor();
    let physical_pages = profile.sequences * M1_PAGES_PER_REQUEST_V1;
    let mut requests = Vec::with_capacity(profile.sequences);
    let mut entries = Vec::with_capacity(physical_pages);
    for request_index in 0..profile.sequences {
        let mut identity = [0_u8; 16];
        identity[0] = u8::try_from(request_index + 1).expect("bounded requests");
        identity[15] = match profile.role {
            Qwen3AttentionRoleV1::Target8B => 0x81,
            Qwen3AttentionRoleV1::Draft06B => 0x06,
        };
        let request_id = PagedKvRequestIdV1(identity);
        let generation = u64::try_from(request_index + 11).expect("bounded requests");
        let resident_tokens = committed_tokens + profile.active_tokens;
        requests.push(PagedKvRequestV1 {
            request_id,
            generation,
            committed_tokens,
            resident_tokens,
        });
        for logical_page in 0..M1_PAGES_PER_REQUEST_V1 {
            let flat = request_index * M1_PAGES_PER_REQUEST_V1 + logical_page;
            let physical_page = if fragmented {
                physical_pages - 1 - flat
            } else {
                flat
            };
            let page_start = logical_page * M1_KV_PAGE_TOKENS_V1;
            let initialized = resident_tokens
                .saturating_sub(page_start)
                .min(M1_KV_PAGE_TOKENS_V1);
            entries.push(PagedKvPageEntryV1 {
                logical_page: u16::try_from(logical_page).expect("512 pages"),
                physical_page: u32::try_from(physical_page).expect("bounded pages"),
                physical_generation: generation,
                request_id,
                initialized_tokens: u16::try_from(initialized).expect("P16"),
                initialized_mask: p16_initialized_mask_v1(initialized).expect("P16"),
            });
        }
    }
    PagedKvBatchMetadataV1 {
        role: profile.role,
        page_tokens: M1_KV_PAGE_TOKENS_V1,
        context_capacity_tokens: M1_CONTEXT_CAPACITY_TOKENS_V1,
        physical_pages,
        key_allocation: PagedKvAllocationIdV1([0x4b; 16]),
        value_allocation: PagedKvAllocationIdV1([0x56; 16]),
        requests,
        entries,
    }
}

pub fn bf16(value: f32) -> Bf16V1 {
    Bf16V1::from_f32_rne(value).expect("finite fixture")
}

pub fn physical_index(
    candidate: StructuralPagedGqaDecodeCandidateV1,
    metadata: &PagedKvBatchMetadataV1,
    request: usize,
    logical_token: usize,
    kv_head: usize,
    feature: usize,
) -> usize {
    let profile = candidate.profile().descriptor();
    let page = logical_token / M1_KV_PAGE_TOKENS_V1;
    let slot = logical_token % M1_KV_PAGE_TOKENS_V1;
    let entry = metadata.entries[request * M1_PAGES_PER_REQUEST_V1 + page];
    usize::try_from(entry.physical_page).expect("bounded physical page")
        * M1_KV_PAGE_TOKENS_V1
        * profile.geometry.kv_heads
        * profile.geometry.head_dimension
        + slot * profile.geometry.kv_heads * profile.geometry.head_dimension
        + kv_head * profile.geometry.head_dimension
        + feature
}

pub struct DataFixture {
    pub query: Vec<Bf16V1>,
    pub key: Vec<Bf16V1>,
    pub value: Vec<Bf16V1>,
    pub contiguous_key: Vec<Vec<Bf16V1>>,
    pub contiguous_value: Vec<Vec<Bf16V1>>,
}

pub fn data(
    candidate: StructuralPagedGqaDecodeCandidateV1,
    metadata: &PagedKvBatchMetadataV1,
) -> DataFixture {
    let profile = candidate.profile().descriptor();
    let query_elements = usize::try_from(candidate.resources().query_elements).expect("bounded");
    let kv_elements = usize::try_from(candidate.resources().kv_elements_each).expect("bounded");
    let mut query = vec![bf16(0.0); query_elements];
    let mut key = vec![bf16(0.0); kv_elements];
    let mut value = vec![bf16(0.0); kv_elements];
    let mut contiguous_key = Vec::with_capacity(profile.sequences);
    let mut contiguous_value = Vec::with_capacity(profile.sequences);

    for request in 0..profile.sequences {
        for local_query in 0..profile.active_tokens {
            for query_head in 0..profile.geometry.query_heads {
                for feature in 0..profile.geometry.head_dimension {
                    let index = (((request * profile.active_tokens + local_query)
                        * profile.geometry.query_heads
                        + query_head)
                        * profile.geometry.head_dimension)
                        + feature;
                    let signed = i32::try_from((request + local_query + query_head + feature) % 11)
                        .expect("small")
                        - 5;
                    query[index] = bf16(signed as f32 / 32.0);
                }
            }
        }
        let resident = metadata.requests[request].resident_tokens;
        let logical_elements =
            resident * profile.geometry.kv_heads * profile.geometry.head_dimension;
        let mut logical_key = vec![bf16(0.0); logical_elements];
        let mut logical_value = vec![bf16(0.0); logical_elements];
        for token in 0..resident {
            for kv_head in 0..profile.geometry.kv_heads {
                for feature in 0..profile.geometry.head_dimension {
                    let logical_index = (token * profile.geometry.kv_heads + kv_head)
                        * profile.geometry.head_dimension
                        + feature;
                    let key_signed =
                        i32::try_from((token * 3 + kv_head * 5 + feature) % 13).expect("small") - 6;
                    let value_signed =
                        i32::try_from((token * 7 + kv_head * 2 + feature) % 17).expect("small") - 8;
                    logical_key[logical_index] = bf16(key_signed as f32 / 16.0);
                    logical_value[logical_index] = bf16(value_signed as f32 / 8.0);
                    let physical =
                        physical_index(candidate, metadata, request, token, kv_head, feature);
                    key[physical] = logical_key[logical_index];
                    value[physical] = logical_value[logical_index];
                }
            }
        }
        contiguous_key.push(logical_key);
        contiguous_value.push(logical_value);
    }
    DataFixture {
        query,
        key,
        value,
        contiguous_key,
        contiguous_value,
    }
}
