use vstd::prelude::*;

verus! {

pub open spec fn pow2_v2(exponent: nat) -> nat
    decreases exponent,
{
    if exponent == 0 { 1 } else { 2 * pow2_v2((exponent - 1) as nat) }
}

pub open spec fn norm_u32_v2(value: int) -> int { value % 4294967296 }

pub open spec fn bit_v2(value: int, bit: nat) -> int {
    (norm_u32_v2(value) / (pow2_v2(bit) as int)) % 2
}

pub open spec fn bitwise_v2(operator: nat, left: int, right: int, width: nat) -> int
    decreases width,
{
    if width == 0 { 0 } else {
        let bit = (width - 1) as nat;
        let left_set = bit_v2(left, bit) == 1;
        let right_set = bit_v2(right, bit) == 1;
        let set = if operator == 4 { left_set && right_set }
            else if operator == 5 { left_set || right_set }
            else { left_set != right_set };
        bitwise_v2(operator, left, right, bit)
            + if set { pow2_v2(bit) as int } else { 0 }
    }
}

pub open spec fn normalized_u32_eval_v2(operator: nat, left: int, right: int) -> int {
    if operator == 1 { norm_u32_v2(left + right) }
    else if operator == 2 { norm_u32_v2(left - right) }
    else if operator == 3 { norm_u32_v2(left * right) }
    else { bitwise_v2(operator, left, right, 32) }
}

/// Source syntax uses the deliberately distinct closed opcode range 11..=16.
pub open spec fn source_u32_eval_v2(source_operator: nat, left: int, right: int) -> int {
    normalized_u32_eval_v2((source_operator - 10) as nat, left, right)
}

/// Semantic MIR uses the closed opcode range 1..=6.
pub open spec fn mir_u32_eval_v2(mir_operator: nat, left: int, right: int) -> int {
    normalized_u32_eval_v2(mir_operator, left, right)
}

/// KIR uses the deliberately distinct closed opcode range 101..=106.
pub open spec fn kir_u32_eval_v2(kir_operator: nat, left: int, right: int) -> int {
    normalized_u32_eval_v2((kir_operator - 100) as nat, left, right)
}

pub open spec fn source_observation_v2(
    source_operator: nat,
    left: int,
    right: int,
    destination: int,
) -> Seq<int> {
    seq![left, right, destination, source_u32_eval_v2(source_operator, left, right)]
}

pub open spec fn mir_observation_v2(
    mir_operator: nat,
    left: int,
    right: int,
    destination: int,
) -> Seq<int> {
    seq![left, right, destination, mir_u32_eval_v2(mir_operator, left, right)]
}

pub open spec fn kir_observation_v2(
    kir_operator: nat,
    left: int,
    right: int,
    semantic_destination: int,
) -> Seq<int> {
    seq![left, right, semantic_destination, kir_u32_eval_v2(kir_operator, left, right)]
}

pub proof fn source_to_mir_parameter_step_v2(
    source_operator: nat,
    mir_operator: nat,
    source_left_binding: int,
    source_right_binding: int,
    source_destination_binding: int,
    mir_left_local: int,
    mir_right_local: int,
    mir_destination_local: int,
    source_left: int,
    source_right: int,
    mir_left: int,
    mir_right: int,
)
    requires
        11 <= source_operator <= 16,
        mir_operator == source_operator - 10,
        source_left_binding == mir_left_local,
        source_right_binding == mir_right_local,
        source_destination_binding == mir_destination_local,
        source_left == mir_left,
        source_right == mir_right,
    ensures
        source_u32_eval_v2(source_operator, source_left, source_right)
            == mir_u32_eval_v2(mir_operator, mir_left, mir_right),
        source_observation_v2(
            source_operator,
            source_left,
            source_right,
            source_destination_binding,
        ) == mir_observation_v2(
            mir_operator,
            mir_left,
            mir_right,
            mir_destination_local,
        ),
{
}

pub proof fn mir_to_kir_parameter_step_v2(
    mir_operator: nat,
    kir_operator: nat,
    mir_left_local: int,
    mir_right_local: int,
    mir_destination_local: int,
    kir_left_parameter_local: int,
    kir_right_parameter_local: int,
    kir_destination_semantic_local: int,
    mir_left: int,
    mir_right: int,
    kir_left: int,
    kir_right: int,
)
    requires
        1 <= mir_operator <= 6,
        kir_operator == mir_operator + 100,
        mir_left_local == kir_left_parameter_local,
        mir_right_local == kir_right_parameter_local,
        mir_destination_local == kir_destination_semantic_local,
        mir_left == kir_left,
        mir_right == kir_right,
    ensures
        mir_u32_eval_v2(mir_operator, mir_left, mir_right)
            == kir_u32_eval_v2(kir_operator, kir_left, kir_right),
        mir_observation_v2(mir_operator, mir_left, mir_right, mir_destination_local)
            == kir_observation_v2(
                kir_operator,
                kir_left,
                kir_right,
                kir_destination_semantic_local,
            ),
{
}

/// Exact boundary composition: distinct source, MIR, and KIR opcode spaces;
/// executable-checker guards for same-session owner/expression/body identities;
/// one common semantic module; positional parameter-local mapping; and ordered
/// operands and result for the effect-free binary operation.
pub proof fn fe2o3_source_mir_kir_u32_element_refines_v2(
    source_operator: nat,
    mir_operator: nat,
    kir_operator: nat,
    source_owner_identity: int,
    source_expression_identity: int,
    rustc_mir_body_identity: int,
    source_semantic_identity: int,
    mir_kir_semantic_identity: int,
    kir_identity: int,
    source_left_binding: int,
    source_right_binding: int,
    source_destination_binding: int,
    mir_left_local: int,
    mir_right_local: int,
    mir_destination_local: int,
    kir_left_parameter_local: int,
    kir_right_parameter_local: int,
    kir_destination_semantic_local: int,
    source_left: int,
    source_right: int,
    mir_left: int,
    mir_right: int,
    kir_left: int,
    kir_right: int,
)
    requires
        source_owner_identity != 0,
        source_expression_identity != 0,
        rustc_mir_body_identity != 0,
        kir_identity != 0,
        source_semantic_identity != 0,
        source_semantic_identity == mir_kir_semantic_identity,
        11 <= source_operator <= 16,
        mir_operator == source_operator - 10,
        kir_operator == mir_operator + 100,
        source_left_binding == mir_left_local,
        source_right_binding == mir_right_local,
        source_destination_binding == mir_destination_local,
        mir_left_local == kir_left_parameter_local,
        mir_right_local == kir_right_parameter_local,
        mir_destination_local == kir_destination_semantic_local,
        source_left == mir_left,
        source_right == mir_right,
        mir_left == kir_left,
        mir_right == kir_right,
    ensures
        source_u32_eval_v2(source_operator, source_left, source_right)
            == kir_u32_eval_v2(kir_operator, kir_left, kir_right),
        source_observation_v2(
            source_operator,
            source_left,
            source_right,
            source_destination_binding,
        ) == kir_observation_v2(
            kir_operator,
            kir_left,
            kir_right,
            kir_destination_semantic_local,
        ),
{
    source_to_mir_parameter_step_v2(
        source_operator,
        mir_operator,
        source_left_binding,
        source_right_binding,
        source_destination_binding,
        mir_left_local,
        mir_right_local,
        mir_destination_local,
        source_left,
        source_right,
        mir_left,
        mir_right,
    );
    mir_to_kir_parameter_step_v2(
        mir_operator,
        kir_operator,
        mir_left_local,
        mir_right_local,
        mir_destination_local,
        kir_left_parameter_local,
        kir_right_parameter_local,
        kir_destination_semantic_local,
        mir_left,
        mir_right,
        kir_left,
        kir_right,
    );
}

}

fn main() {}
