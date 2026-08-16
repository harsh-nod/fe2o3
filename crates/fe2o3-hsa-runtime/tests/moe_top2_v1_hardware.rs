//! Protected exact-profile MoE routing hardware gate and independent oracle.
//!
//! The ignored test intentionally cannot turn an artifact path or byte string
//! into authority. Until the production static wrapper can deliver the linear
//! finalization receipt in-process, it fails closed before HSA load.

const TOKENS: usize = 8;
const EXPERTS: usize = 4;
const TOP_K: usize = 2;
const CAPACITY: u32 = 4;
const ROUTES: usize = TOKENS * TOP_K;
const DROP: u32 = u32::MAX;
const CANARY_LEFT: u32 = 0xa11c_e001;
const CANARY_RIGHT: u32 = 0xa11c_e002;
const OUTPUT_POISON: u32 = 0xd15c_a4d0;

type BoxError = Box<dyn std::error::Error>;

#[derive(Clone, Debug, Eq, PartialEq)]
struct RoutingOutput {
    top2_experts: [u32; ROUTES],
    requested_counts: [u32; EXPERTS],
    admitted_counts: [u32; EXPERTS],
    expert_offsets: [u32; EXPERTS + 1],
    route_slots: [u32; ROUTES],
    permutation: [u32; ROUTES],
    inverse: [u32; ROUTES],
}

fn require(condition: bool, message: impl Into<String>) -> Result<(), BoxError> {
    if condition {
        Ok(())
    } else {
        Err(message.into().into())
    }
}

fn candidate_precedes(
    candidate_score: f32,
    candidate_expert: usize,
    incumbent_score: f32,
    incumbent_expert: usize,
) -> bool {
    candidate_score > incumbent_score
        || (candidate_score == incumbent_score && candidate_expert < incumbent_expert)
}

/// Independent strict-order CPU implementation of the frozen router.
fn routing_oracle(logits: &[f32; TOKENS * EXPERTS]) -> Result<RoutingOutput, BoxError> {
    require(
        logits.iter().all(|score| score.is_finite()),
        "non-finite logits",
    )?;
    let mut top2_experts = [0_u32; ROUTES];
    let mut requested_counts = [0_u32; EXPERTS];
    for token in 0..TOKENS {
        let mut best = usize::MAX;
        let mut second = usize::MAX;
        for expert in 0..EXPERTS {
            let score = logits[token * EXPERTS + expert];
            if best == usize::MAX
                || candidate_precedes(score, expert, logits[token * EXPERTS + best], best)
            {
                second = best;
                best = expert;
            } else if second == usize::MAX
                || candidate_precedes(score, expert, logits[token * EXPERTS + second], second)
            {
                second = expert;
            }
        }
        let route = token * TOP_K;
        top2_experts[route] = best as u32;
        top2_experts[route + 1] = second as u32;
        requested_counts[best] += 1;
        requested_counts[second] += 1;
    }

    let admitted_counts = requested_counts.map(|count| count.min(CAPACITY));
    let mut expert_offsets = [0_u32; EXPERTS + 1];
    for expert in 0..EXPERTS {
        expert_offsets[expert + 1] = expert_offsets[expert] + admitted_counts[expert];
    }

    let mut route_slots = [DROP; ROUTES];
    let mut permutation = [DROP; ROUTES];
    let mut inverse = [DROP; ROUTES];
    let mut seen = [0_u32; EXPERTS];
    for route in 0..ROUTES {
        let expert = top2_experts[route] as usize;
        let stable_rank = seen[expert];
        seen[expert] += 1;
        if stable_rank < CAPACITY {
            let slot = expert_offsets[expert] + stable_rank;
            route_slots[route] = slot;
            permutation[slot as usize] = route as u32;
            inverse[route] = slot;
        }
    }
    Ok(RoutingOutput {
        top2_experts,
        requested_counts,
        admitted_counts,
        expert_offsets,
        route_slots,
        permutation,
        inverse,
    })
}

fn validate_relations(output: &RoutingOutput) -> Result<(), BoxError> {
    require(output.expert_offsets[0] == 0, "offset origin")?;
    for expert in 0..EXPERTS {
        require(
            output.expert_offsets[expert + 1]
                == output.expert_offsets[expert] + output.admitted_counts[expert],
            "exclusive offsets",
        )?;
        require(output.admitted_counts[expert] <= CAPACITY, "capacity")?;
    }
    let admitted = output.expert_offsets[EXPERTS] as usize;
    for slot in 0..admitted {
        let route = output.permutation[slot];
        require(
            route != DROP && (route as usize) < ROUTES,
            "permutation domain",
        )?;
        require(
            output.inverse[route as usize] == slot as u32,
            "permutation inverse",
        )?;
        require(
            output.route_slots[route as usize] == slot as u32,
            "route slot inverse",
        )?;
    }
    require(
        output.permutation[admitted..]
            .iter()
            .all(|route| *route == DROP),
        "permutation sentinel tail",
    )?;
    for route in 0..ROUTES {
        require(
            output.route_slots[route] == output.inverse[route],
            "slot/inverse equality",
        )?;
    }
    Ok(())
}

fn exercise_oracle_and_guards() -> Result<(), BoxError> {
    let all_ties = [1.0_f32; TOKENS * EXPERTS];
    let ties_before = all_ties;
    let ties = routing_oracle(&all_ties)?;
    require(all_ties == ties_before, "tie input changed")?;
    require(
        ties.top2_experts == [0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1],
        "lower-expert tie break",
    )?;
    require(ties.requested_counts == [8, 8, 0, 0], "tie requests")?;
    require(ties.admitted_counts == [4, 4, 0, 0], "tie capacity")?;
    require(ties.expert_offsets == [0, 4, 8, 8, 8], "tie offsets")?;
    require(
        ties.permutation
            == [
                0, 2, 4, 6, 1, 3, 5, 7, DROP, DROP, DROP, DROP, DROP, DROP, DROP, DROP,
            ],
        "stable capacity drops and sentinel tail",
    )?;
    require(
        ties.inverse
            == [
                0, 4, 1, 5, 2, 6, 3, 7, DROP, DROP, DROP, DROP, DROP, DROP, DROP, DROP,
            ],
        "stable inverse",
    )?;
    validate_relations(&ties)?;

    let mixed = std::array::from_fn(|index| {
        let token = index / EXPERTS;
        let expert = index % EXPERTS;
        if expert == token % EXPERTS {
            8.0
        } else {
            expert as f32 - 4.0
        }
    });
    let mixed_before = mixed;
    let mixed_output = routing_oracle(&mixed)?;
    require(mixed == mixed_before, "mixed input changed")?;
    validate_relations(&mixed_output)?;
    require(
        mixed_output.requested_counts.iter().sum::<u32>() == ROUTES as u32,
        "every route counted",
    )?;

    let admitted = mixed_output.expert_offsets[EXPERTS] as usize;
    let mut guarded = [OUTPUT_POISON; ROUTES + 2];
    guarded[0] = CANARY_LEFT;
    guarded[ROUTES + 1] = CANARY_RIGHT;
    guarded[1..=ROUTES].copy_from_slice(&mixed_output.permutation);
    require(guarded[0] == CANARY_LEFT, "left canary changed")?;
    require(guarded[ROUTES + 1] == CANARY_RIGHT, "right canary changed")?;
    require(
        guarded[1 + admitted..=ROUTES]
            .iter()
            .all(|value| *value == DROP),
        "guarded sentinel tail",
    )?;

    for exceptional in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut invalid = mixed;
        invalid[0] = exceptional;
        let guards = [CANARY_LEFT, OUTPUT_POISON, CANARY_RIGHT];
        require(
            routing_oracle(&invalid).is_err(),
            "exceptional logits accepted",
        )?;
        require(
            guards == [CANARY_LEFT, OUTPUT_POISON, CANARY_RIGHT],
            "exceptional path changed guarded output",
        )?;
    }
    Ok(())
}

fn exact_lower_hex_sha256(name: &str) -> Result<[u8; 32], BoxError> {
    let value = std::env::var(name).map_err(|_| format!("missing protected pin {name}"))?;
    require(
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        format!("{name} must be 64 lowercase hexadecimal digits"),
    )?;
    let mut digest = [0_u8; 32];
    for (index, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)?;
    }
    require(digest != [0; 32], format!("{name} must be nonzero"))?;
    Ok(digest)
}

#[test]
fn independent_moe_oracle_covers_ties_capacity_permutation_inverse_and_sentinels()
-> Result<(), BoxError> {
    exercise_oracle_and_guards()
}

#[test]
#[ignore = "requires the production static wrapper, exact measured pins, protected linear receipt injection, and MI300X"]
fn protected_gfx942_moe_top2_v1_hardware() -> Result<(), BoxError> {
    exercise_oracle_and_guards()?;
    let wrapper = exact_lower_hex_sha256("FE2O3_MOE_TOP2_V1_STATIC_WRAPPER_SHA256")?;
    let worker = exact_lower_hex_sha256("FE2O3_MOE_TOP2_V1_WORKER_SHA256")?;
    let llvm = exact_lower_hex_sha256("FE2O3_MOE_TOP2_V1_LLVM_SHA256")?;
    require(
        [wrapper, worker, llvm]
            .windows(2)
            .all(|pair| pair[0] != pair[1]),
        "protected measurements must be independently bound",
    )?;
    Err(
        "production static wrapper cannot yet inject the opaque linear MoE receipt; refusing artifact-path or raw-byte fallback before HSA load"
            .into(),
    )
}
