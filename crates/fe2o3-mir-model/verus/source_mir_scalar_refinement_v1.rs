use vstd::prelude::*;

verus! {

pub open spec fn pow2_v1(exponent: nat) -> nat
    decreases exponent,
{
    if exponent == 0 { 1 } else { 2 * pow2_v1((exponent - 1) as nat) }
}

pub open spec fn norm_u32_v1(value: int) -> int { value % 4294967296 }

pub open spec fn bit_v1(value: int, bit: nat) -> int {
    (norm_u32_v1(value) / (pow2_v1(bit) as int)) % 2
}

pub open spec fn bitwise_v1(operator: nat, left: int, right: int, width: nat) -> int
    decreases width,
{
    if width == 0 { 0 } else {
        let bit = (width - 1) as nat;
        let left_set = bit_v1(left, bit) == 1;
        let right_set = bit_v1(right, bit) == 1;
        let set = if operator == 4 { left_set && right_set }
            else if operator == 5 { left_set || right_set }
            else { left_set != right_set };
        bitwise_v1(operator, left, right, bit)
            + if set { pow2_v1(bit) as int } else { 0 }
    }
}

pub open spec fn source_eval_v1(operator: nat, left: int, right: int) -> int {
    if operator == 1 { norm_u32_v1(left + right) }
    else if operator == 2 { norm_u32_v1(left - right) }
    else if operator == 3 { norm_u32_v1(left * right) }
    else { bitwise_v1(operator, left, right, 32) }
}

pub open spec fn mir_eval_v1(operator: nat, left: int, right: int) -> int {
    if operator == 1 { norm_u32_v1(left + right) }
    else if operator == 2 { norm_u32_v1(left - right) }
    else if operator == 3 { norm_u32_v1(left * right) }
    else { bitwise_v1(operator, left, right, 32) }
}

pub open spec fn source_effects_v1(operator: nat, left: int, right: int, destination: int) -> Seq<int> {
    seq![left, right, destination, source_eval_v1(operator, left, right)]
}

pub open spec fn mir_effects_v1(operator: nat, left: int, right: int, destination: int) -> Seq<int> {
    seq![left, right, destination, mir_eval_v1(operator, left, right)]
}

pub proof fn fe2o3_source_mir_u32_element_refines_v1(
    source_operator: nat,
    mir_operator: nat,
    source_type_bits: nat,
    mir_type_bits: nat,
    source_left_binding: int,
    source_right_binding: int,
    source_destination_binding: int,
    mir_left_binding: int,
    mir_right_binding: int,
    mir_destination_binding: int,
    source_left: int,
    source_right: int,
    mir_left: int,
    mir_right: int,
)
    requires
        1 <= source_operator <= 6,
        mir_operator == source_operator,
        source_type_bits == 32,
        mir_type_bits == source_type_bits,
        mir_left_binding == source_left_binding,
        mir_right_binding == source_right_binding,
        mir_destination_binding == source_destination_binding,
        mir_left == source_left,
        mir_right == source_right,
    ensures
        source_eval_v1(source_operator, source_left, source_right)
            == mir_eval_v1(mir_operator, mir_left, mir_right),
        source_effects_v1(source_operator, source_left, source_right, source_destination_binding)
            == mir_effects_v1(mir_operator, mir_left, mir_right, mir_destination_binding),
{
}

}

fn main() {}
