//! Independent safe CPU references for the systems tutorial kernels.

use crate::{
    ALL_EXPERTS, CANDIDATES, DISPATCH_CAPACITY, DRAFT_STEPS, EXPERTS, GRADIENT_SHARDS, HIDDEN,
    MUON_DIM, MUON_ELEMENTS, MUON_ITERATIONS, MUON_LEARNING_RATE, NGRAM, OUTPUT, QUERIES,
    STATE_WIDTH, TABLE_SIZE, TOKENS, TOP_K,
};

/// Complete CPU routing observation.
#[derive(Clone, Debug, PartialEq)]
pub struct MoeRoutingReference {
    /// Token-major top-2 expert IDs.
    pub top_experts: Vec<u32>,
    /// Token-major normalized route weights.
    pub top_weights: Vec<f32>,
    /// Number of routes assigned to each expert.
    pub expert_counts: Vec<u32>,
    /// Expert-major compact route IDs with `-1` padding.
    pub dispatch: Vec<i32>,
}

/// Speculative transaction result.
#[derive(Clone, Debug, PartialEq)]
pub struct SpeculativeReference {
    /// Accepted prefix length per candidate.
    pub accepted: Vec<u32>,
    /// One for a committed candidate, zero for rollback.
    pub committed: Vec<u32>,
    /// Candidate-major state after commit or rollback.
    pub state: Vec<f32>,
}

/// Muon result and pre-normalization norm.
#[derive(Clone, Debug, PartialEq)]
pub struct MuonReference {
    /// Polar/Newton-Schulz update matrix.
    pub update: Vec<f32>,
    /// Frobenius norm of the fixed-order shard sum.
    pub norm: f32,
}

/// Decodes an OCP E2M1 nibble.
pub fn decode_fp4(bits: u8) -> f32 {
    let values = [0.0_f32, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 6.0];
    let value = values[(bits & 7) as usize];
    if bits & 8 == 0 { value } else { -value }
}

/// Decodes OCP E4M3, including its positive and negative NaN encodings.
pub fn decode_fp8(bits: u8) -> f32 {
    let sign = if bits & 0x80 == 0 { 1.0 } else { -1.0 };
    let exponent = (bits >> 3) & 15;
    let mantissa = (bits & 7) as f32;
    if exponent == 15 && mantissa == 7.0 {
        return f32::NAN;
    }
    let value = if exponent == 0 {
        mantissa * 2.0_f32.powi(-9)
    } else {
        (1.0 + mantissa / 8.0) * 2.0_f32.powi(exponent as i32 - 7)
    };
    sign * value
}

/// Computes stable top-2 routing and dispatch independently from the kernel.
pub fn moe_routing_reference(activations: &[u8], router_weights: &[f32]) -> MoeRoutingReference {
    assert_eq!(activations.len(), TOKENS * HIDDEN);
    assert_eq!(router_weights.len(), EXPERTS * HIDDEN);
    let mut result = MoeRoutingReference {
        top_experts: vec![0; TOKENS * TOP_K],
        top_weights: vec![0.0; TOKENS * TOP_K],
        expert_counts: vec![0; EXPERTS],
        dispatch: vec![-1; EXPERTS * DISPATCH_CAPACITY],
    };
    for token in 0..TOKENS {
        let mut logits = [0.0_f32; EXPERTS];
        for expert in 0..EXPERTS {
            logits[expert] = (0..HIDDEN)
                .map(|depth| {
                    decode_fp4(activations[token * HIDDEN + depth])
                        * router_weights[expert * HIDDEN + depth]
                })
                .sum();
        }
        let mut order = [0_usize, 1, 2, 3];
        order.sort_by(|left, right| {
            logits[*right]
                .total_cmp(&logits[*left])
                .then(left.cmp(right))
        });
        let maximum = logits[order[0]].max(logits[order[1]]);
        let weights = [
            (logits[order[0]] - maximum).exp(),
            (logits[order[1]] - maximum).exp(),
        ];
        let denominator = weights[0] + weights[1];
        for choice in 0..TOP_K {
            let expert = order[choice];
            let route = token * TOP_K + choice;
            result.top_experts[route] = expert as u32;
            result.top_weights[route] = weights[choice] / denominator;
            let slot = result.expert_counts[expert] as usize;
            result.expert_counts[expert] += 1;
            result.dispatch[expert * DISPATCH_CAPACITY + slot] = route as i32;
        }
    }
    result
}

/// Computes one two-expert logical rank, with an optional shared expert.
pub fn moe_rank_reference(
    activations: &[u8],
    expert_weights: &[u8],
    routing: &MoeRoutingReference,
    first_expert: usize,
    include_shared: bool,
) -> Vec<f32> {
    assert_eq!(expert_weights.len(), ALL_EXPERTS * HIDDEN * OUTPUT);
    let mut output = vec![0.0; TOKENS * OUTPUT];
    for token in 0..TOKENS {
        for column in 0..OUTPUT {
            for choice in 0..TOP_K {
                let route = token * TOP_K + choice;
                let expert = routing.top_experts[route] as usize;
                if (first_expert..first_expert + 2).contains(&expert) {
                    let dot: f32 = (0..HIDDEN)
                        .map(|depth| {
                            decode_fp4(activations[token * HIDDEN + depth])
                                * decode_fp8(
                                    expert_weights[(expert * HIDDEN + depth) * OUTPUT + column],
                                )
                        })
                        .sum();
                    output[token * OUTPUT + column] +=
                        routing.top_weights[route] * dot / (1.0 + (-dot).exp());
                }
            }
            if include_shared {
                let dot: f32 = (0..HIDDEN)
                    .map(|depth| {
                        decode_fp4(activations[token * HIDDEN + depth])
                            * decode_fp8(
                                expert_weights
                                    [((ALL_EXPERTS - 1) * HIDDEN + depth) * OUTPUT + column],
                            )
                    })
                    .sum();
                output[token * OUTPUT + column] += 0.25 * dot / (1.0 + (-dot).exp());
            }
        }
    }
    output
}

/// Applies transactional speculative acceptance.
pub fn speculative_reference(
    draft: &[i32],
    target: &[i32],
    scores: &[f32],
    thresholds: &[f32],
    base: &[f32],
    deltas: &[f32],
) -> SpeculativeReference {
    let mut result = SpeculativeReference {
        accepted: vec![0; CANDIDATES],
        committed: vec![0; CANDIDATES],
        state: vec![0.0; CANDIDATES * STATE_WIDTH],
    };
    for candidate in 0..CANDIDATES {
        let accepted = (0..DRAFT_STEPS)
            .take_while(|step| {
                draft[candidate * DRAFT_STEPS + *step] == target[*step]
                    && scores[candidate * DRAFT_STEPS + *step] >= thresholds[*step]
            })
            .count();
        result.accepted[candidate] = accepted as u32;
        result.committed[candidate] = u32::from(accepted == DRAFT_STEPS);
        for element in 0..STATE_WIDTH {
            let delta = if accepted == DRAFT_STEPS {
                (0..DRAFT_STEPS)
                    .map(|step| deltas[(candidate * DRAFT_STEPS + step) * STATE_WIDTH + element])
                    .sum()
            } else {
                0.0
            };
            result.state[candidate * STATE_WIDTH + element] = base[element] + delta;
        }
    }
    result
}

fn hash_gram(gram: &[i32]) -> u64 {
    gram.iter()
        .fold(1_469_598_103_934_665_603_u64, |hash, value| {
            (hash ^ (*value as u32 as u64)).wrapping_mul(1_099_511_628_211)
        })
}

/// Performs exact-key, priority-aware linear-probe N-gram lookup.
pub fn ngram_reference(
    queries: &[i32],
    hashes: &[u64],
    grams: &[i32],
    values: &[i32],
    priorities: &[i32],
) -> Vec<i32> {
    assert_eq!(queries.len(), QUERIES * NGRAM);
    (0..QUERIES)
        .map(|query| {
            let gram = &queries[query * NGRAM..(query + 1) * NGRAM];
            let hash = hash_gram(gram);
            let mut matches = (0..TABLE_SIZE)
                .filter_map(|probe| {
                    let slot = hash.wrapping_add(probe as u64) as usize & (TABLE_SIZE - 1);
                    (hashes[slot] == hash && &grams[slot * NGRAM..(slot + 1) * NGRAM] == gram)
                        .then_some(slot)
                })
                .collect::<Vec<_>>();
            matches.sort_by(|left, right| {
                priorities[*right]
                    .cmp(&priorities[*left])
                    .then(left.cmp(right))
            });
            matches.first().map_or(-1, |slot| values[*slot])
        })
        .collect()
}

/// Computes the fixed-order two-shard Muon update.
pub fn muon_reference(shards: &[f32]) -> MuonReference {
    assert_eq!(shards.len(), GRADIENT_SHARDS * MUON_ELEMENTS);
    let mut matrix = [0.0_f32; MUON_ELEMENTS];
    for element in 0..MUON_ELEMENTS {
        matrix[element] = (0..GRADIENT_SHARDS)
            .map(|shard| shards[shard * MUON_ELEMENTS + element])
            .sum();
    }
    let norm = matrix.iter().map(|value| value * value).sum::<f32>().sqrt();
    matrix.iter_mut().for_each(|value| *value /= norm + 1.0e-6);
    for _ in 0..MUON_ITERATIONS {
        let mut gram = [0.0_f32; MUON_ELEMENTS];
        let mut cubic = [0.0_f32; MUON_ELEMENTS];
        for row in 0..MUON_DIM {
            for column in 0..MUON_DIM {
                gram[row * MUON_DIM + column] = (0..MUON_DIM)
                    .map(|inner| matrix[row * MUON_DIM + inner] * matrix[column * MUON_DIM + inner])
                    .sum();
            }
        }
        for row in 0..MUON_DIM {
            for column in 0..MUON_DIM {
                cubic[row * MUON_DIM + column] = (0..MUON_DIM)
                    .map(|inner| gram[row * MUON_DIM + inner] * matrix[inner * MUON_DIM + column])
                    .sum();
            }
        }
        for element in 0..MUON_ELEMENTS {
            matrix[element] = 1.5 * matrix[element] - 0.5 * cubic[element];
        }
    }
    MuonReference {
        update: matrix
            .into_iter()
            .map(|value| -MUON_LEARNING_RATE * value)
            .collect(),
        norm,
    }
}
