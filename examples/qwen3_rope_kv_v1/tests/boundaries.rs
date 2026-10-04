use fe2o3_qwen3_rope_kv_v1::rope_kv::*;

const OWNER: KvOwnerIdentityV1 = KvOwnerIdentityV1([0x63; 16]);
const ROLES: [Qwen3ModelRoleV1; 2] = [Qwen3ModelRoleV1::Target8B, Qwen3ModelRoleV1::Draft06B];
type Rotate = fn(
    &Qwen3RopeKvCandidateV1,
    &[u32],
    &[f64],
    &[f64],
) -> Result<Qwen3RopeOutputV1, RopeReferenceErrorV1>;
const ROTATE: [Rotate; 2] = [qwen3_rope_reference_v1, qwen3_rope_pair_candidate_v1];

fn generation(role: Qwen3ModelRoleV1) -> PageTableGenerationV1 {
    match role {
        Qwen3ModelRoleV1::Target8B => PageTableGenerationV1::Target(TargetPageTableGenerationV1 {
            pool_id: [1; 16],
            generation: 3,
        }),
        Qwen3ModelRoleV1::Draft06B => PageTableGenerationV1::Draft(DraftPageTableGenerationV1 {
            pool_id: [2; 16],
            generation: 5,
        }),
    }
}

fn table(
    role: Qwen3ModelRoleV1,
    context: ContextBucketV1,
    page: PageBucketV1,
    initialized: u32,
) -> Qwen3PageTableV1 {
    let generation = generation(role);
    let page_tokens = u32::from(page.tokens());
    let count = context.tokens() / page_tokens;
    Qwen3PageTableV1 {
        generation,
        context,
        page,
        entries: (0..count)
            .map(|i| PageTableEntryV1 {
                logical_page: i as u16,
                physical_page: 2 * (count - i),
                physical_generation: generation.value(),
                initialized_tokens: initialized.saturating_sub(i * page_tokens).min(page_tokens)
                    as u16,
                exclusive_owner: OWNER,
            })
            .collect(),
    }
}

fn candidate(
    role: Qwen3ModelRoleV1,
    tokens: TokenBucketV1,
    context: ContextBucketV1,
    page: PageBucketV1,
) -> Qwen3RopeKvCandidateV1 {
    exact_qwen3_rope_kv_candidate_v1(role, SequenceBucketV1::S1, tokens, context, page)
}

fn write(
    table: &Qwen3PageTableV1,
    candidate: Qwen3RopeKvCandidateV1,
) -> (Qwen3KvWriteDescriptorV1, Qwen3KvWriteExpectationV1) {
    (
        Qwen3KvWriteDescriptorV1 {
            candidate,
            generation: table.generation,
            owner: OWNER,
            sequence_index: 0,
            layer: candidate.geometry.layers - 1,
            logical_start: table.initialized_prefix_tokens().unwrap(),
        },
        Qwen3KvWriteExpectationV1 {
            candidate,
            generation: table.generation,
            owner: OWNER,
        },
    )
}

#[test]
fn incompatible_direct_tables_reject_all_public_paths_for_both_roles() {
    for role in ROLES {
        for nonempty in [false, true] {
            let mut invalid = table(role, ContextBucketV1::C128, PageBucketV1::P256, 0);
            if nonempty {
                invalid.entries.push(PageTableEntryV1 {
                    logical_page: 0,
                    physical_page: 0,
                    physical_generation: invalid.generation.value(),
                    initialized_tokens: 0,
                    exclusive_owner: OWNER,
                });
            }
            let error = PageTableErrorV1::PageDoesNotDivideContext;
            assert_eq!(
                invalid.validate_against(invalid.generation, OWNER),
                Err(error)
            );
            assert_eq!(invalid.initialized_prefix_tokens(), Err(error));
            assert_eq!(invalid.logical_to_physical(0), Err(error));
            assert_eq!(invalid.initialized_logical_to_physical(0), Err(error));
        }
    }
}

#[test]
fn all_compatible_direct_context_page_pairs_keep_exact_boundaries() {
    for role in ROLES {
        let mut admitted = 0;
        for context in [
            ContextBucketV1::C128,
            ContextBucketV1::C1024,
            ContextBucketV1::C4096,
            ContextBucketV1::C8192,
        ] {
            for page in [PageBucketV1::P16, PageBucketV1::P64, PageBucketV1::P256] {
                let table = table(role, context, page, context.tokens());
                if context == ContextBucketV1::C128 && page == PageBucketV1::P256 {
                    assert_eq!(
                        table.validate_against(table.generation, OWNER),
                        Err(PageTableErrorV1::PageDoesNotDivideContext)
                    );
                    continue;
                }
                admitted += 1;
                table.validate_against(table.generation, OWNER).unwrap();
                assert_eq!(table.initialized_prefix_tokens(), Ok(context.tokens()));
                for token in [0, u32::from(page.tokens()) - 1, context.tokens() - 1] {
                    let expected = KvPhysicalLocationV1 {
                        physical_page: table.entries[(token / u32::from(page.tokens())) as usize]
                            .physical_page,
                        token_slot: (token % u32::from(page.tokens())) as u16,
                        physical_generation: table.generation.value(),
                    };
                    assert_eq!(table.logical_to_physical(token), Ok(expected));
                    assert_eq!(table.initialized_logical_to_physical(token), Ok(expected));
                }
                assert_eq!(
                    table.logical_to_physical(context.tokens()),
                    Err(PageTableErrorV1::LogicalTokenOutOfBounds)
                );
                assert_eq!(
                    table.initialized_logical_to_physical(context.tokens()),
                    Err(PageTableErrorV1::UninitializedRead)
                );
            }
        }
        assert_eq!(admitted, 11);
    }
}

#[test]
fn analytic_basis_rotations_check_frequency_and_sign_independently() {
    for role in ROLES {
        let candidate = candidate(
            role,
            TokenBucketV1::T1,
            ContextBucketV1::C8192,
            PageBucketV1::P256,
        );
        for rotate in ROTATE {
            for position in [1, 8191] {
                for pair in [0, 32] {
                    for upper_basis in [false, true] {
                        let dimension = pair + if upper_basis { 64 } else { 0 };
                        let mut query =
                            vec![0.0; usize::from(candidate.geometry.query_heads) * 128];
                        let mut key = vec![0.0; 8 * 128];
                        for head in query.chunks_exact_mut(128).chain(key.chunks_exact_mut(128)) {
                            head[dimension] = 1.0;
                        }
                        let output = rotate(&candidate, &[position], &query, &key).unwrap();
                        let angle = f64::from(position) / if pair == 0 { 1.0 } else { 1000.0 };
                        let (sine, cosine) = angle.sin_cos();
                        for head in output
                            .query
                            .chunks_exact(128)
                            .chain(output.key.chunks_exact(128))
                        {
                            for (index, &actual) in head.iter().enumerate() {
                                let expected = if index == pair {
                                    if upper_basis { -sine } else { cosine }
                                } else if index == pair + 64 {
                                    if upper_basis { cosine } else { sine }
                                } else {
                                    0.0
                                };
                                assert!(
                                    (actual - expected).abs() <= 32.0 * f64::EPSILON,
                                    "pair={pair} position={position} index={index}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn both_rope_algorithms_reject_late_nonfinite_inputs_and_finite_overflow() {
    for role in ROLES {
        let candidate = candidate(
            role,
            TokenBucketV1::T2,
            ContextBucketV1::C128,
            PageBucketV1::P16,
        );
        let query_len = 2 * usize::from(candidate.geometry.query_heads) * 128;
        for rotate in ROTATE {
            for in_query in [true, false] {
                for exceptional in [f64::NAN, -f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                    let mut query = vec![0.25; query_len];
                    let mut key = vec![0.5; 2 * 8 * 128];
                    let input = if in_query { &mut query } else { &mut key };
                    let last = input.len() - 1;
                    input[last] = exceptional;
                    assert_eq!(
                        rotate(&candidate, &[0, 1], &query, &key),
                        Err(RopeReferenceErrorV1::NonFiniteInput {
                            index: last + if in_query { 0 } else { query_len },
                        })
                    );
                }
                let mut query = vec![0.25; query_len];
                let mut key = vec![0.5; 2 * 8 * 128];
                let input = if in_query { &mut query } else { &mut key };
                let lower = input.len() - 128;
                input[lower] = f64::MAX;
                input[lower + 64] = -f64::MAX;
                assert_eq!(
                    rotate(&candidate, &[0, 1], &query, &key),
                    Err(RopeReferenceErrorV1::NonFiniteOutput { index: lower })
                );
            }
        }
    }
}

#[test]
fn highest_physical_coordinate_preserves_more_than_32_bits() {
    for role in ROLES {
        let mut table = table(role, ContextBucketV1::C1024, PageBucketV1::P256, 255);
        table.entries[0].physical_page = 65535;
        let candidate = candidate(role, TokenBucketV1::T1, table.context, table.page);
        let (descriptor, expected) = write(&table, candidate);
        let coordinate =
            qwen3_kv_write_coordinate_v1(&descriptor, &table, &expected, 0, 7, 127).unwrap();
        assert_eq!(coordinate.pool_element_offset, (1_u64 << 34) - 1);
        assert_eq!(coordinate.location.token_slot, 255);
        assert_eq!(coordinate.layer, role.geometry().layers - 1);
        table.entries[0].physical_page = 65536;
        assert_eq!(
            qwen3_kv_write_coordinate_v1(&descriptor, &table, &expected, 0, 7, 127),
            Err(KvWriteErrorV1::PageTable(
                PageTableErrorV1::PhysicalPageOutOfBounds
            ))
        );
    }
}

#[test]
fn rotated_keys_compose_with_paged_writes_and_exact_prefix_projection() {
    for role in ROLES {
        for page in [PageBucketV1::P16, PageBucketV1::P64, PageBucketV1::P256] {
            let page_tokens = u32::from(page.tokens());
            let table = table(role, ContextBucketV1::C1024, page, page_tokens - 1);
            let before = table.clone();
            let candidate = candidate(role, TokenBucketV1::T2, table.context, page);
            let (descriptor, expected) = write(&table, candidate);
            let query = vec![0.25; 2 * usize::from(candidate.geometry.query_heads) * 128];
            let key: Vec<_> = (0..2 * 8 * 128).map(|i| i as f64 / 8192.0).collect();
            let value: Vec<_> = (0..key.len()).map(|i| -(i as f64) / 4096.0).collect();
            let rotated =
                qwen3_rope_reference_v1(&candidate, &[page_tokens - 1, page_tokens], &query, &key)
                    .unwrap();
            let records = qwen3_paged_kv_write_reference_v1(
                &descriptor,
                &table,
                &expected,
                &rotated.key,
                &value,
            )
            .unwrap();
            let mut offsets = std::collections::BTreeSet::new();
            assert_eq!(records.elements.len(), key.len());
            for (i, record) in records.elements.iter().enumerate() {
                let token = page_tokens - 1 + (i / 1024) as u32;
                let head = (i / 128) % 8;
                let component = i % 128;
                let physical_page = table.entries[(token / page_tokens) as usize].physical_page;
                let slot = token % page_tokens;
                let offset =
                    ((u64::from(physical_page) * u64::from(page_tokens) + u64::from(slot)) * 8
                        + head as u64)
                        * 128
                        + component as u64;
                assert_eq!(record.rotated_key, rotated.key[i]);
                assert_eq!(record.value, value[i]);
                assert_eq!(record.coordinate.logical_token, token);
                assert_eq!(record.coordinate.kv_head, head as u16);
                assert_eq!(record.coordinate.component, component as u16);
                assert_eq!(record.coordinate.pool_element_offset, offset);
                assert!(offsets.insert(offset));
            }
            let projected = project_qwen3_kv_write_v1(&descriptor, &table, &expected).unwrap();
            assert_eq!(table, before);
            let mut exact = before;
            exact.entries[0].initialized_tokens = page.tokens();
            exact.entries[1].initialized_tokens = 1;
            assert_eq!(projected, exact);
            assert_eq!(projected.initialized_prefix_tokens(), Ok(page_tokens + 1));
            for token in [page_tokens - 1, page_tokens] {
                assert_eq!(
                    table.initialized_logical_to_physical(token),
                    Err(PageTableErrorV1::UninitializedRead)
                );
                assert_eq!(
                    projected.initialized_logical_to_physical(token),
                    table.logical_to_physical(token)
                );
            }
            assert_eq!(
                projected.initialized_logical_to_physical(page_tokens + 1),
                Err(PageTableErrorV1::UninitializedRead)
            );
        }
    }
}

#[test]
fn paged_write_rejects_late_nonfinite_values_in_each_input() {
    for role in ROLES {
        let table = table(role, ContextBucketV1::C128, PageBucketV1::P16, 15);
        let before = table.clone();
        let candidate = candidate(role, TokenBucketV1::T2, table.context, table.page);
        let (descriptor, expected) = write(&table, candidate);
        for in_key in [true, false] {
            for exceptional in [f64::NAN, -f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                let mut key = vec![0.25; 2048];
                let mut value = vec![-0.5; 2048];
                let input = if in_key { &mut key } else { &mut value };
                input[2047] = exceptional;
                assert_eq!(
                    qwen3_paged_kv_write_reference_v1(&descriptor, &table, &expected, &key, &value),
                    Err(KvWriteErrorV1::NonFiniteInput {
                        index: if in_key { 2047 } else { 4095 },
                    })
                );
                assert_eq!(table, before);
            }
        }
    }
}
