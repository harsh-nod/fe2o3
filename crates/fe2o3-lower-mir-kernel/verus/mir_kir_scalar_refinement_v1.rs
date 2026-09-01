use vstd::prelude::*;

verus! {

pub open spec fn pow2_v1(exponent: nat) -> nat
    decreases exponent,
{
    if exponent == 0 { 1 } else { 2 * pow2_v1((exponent - 1) as nat) }
}

pub open spec fn norm_u32_v1(value: int) -> int {
    value % 4294967296
}

pub open spec fn bit_v1(value: int, bit: nat) -> int {
    (norm_u32_v1(value) / (pow2_v1(bit) as int)) % 2
}

pub open spec fn bitwise_v1(operator: nat, left: int, right: int, width: nat) -> int
    decreases width,
{
    if width == 0 {
        0
    } else {
        let bit = (width - 1) as nat;
        let left_set = bit_v1(left, bit) == 1;
        let right_set = bit_v1(right, bit) == 1;
        let result_set =
            if operator == 4 { left_set && right_set }
            else if operator == 5 { left_set || right_set }
            else { left_set != right_set };
        bitwise_v1(operator, left, right, bit)
            + if result_set { pow2_v1(bit) as int } else { 0 }
    }
}

pub open spec fn mir_u32_eval_v1(operator: nat, left: int, right: int) -> int {
    if operator == 1 { norm_u32_v1(left + right) }
    else if operator == 2 { norm_u32_v1(left - right) }
    else if operator == 3 { norm_u32_v1(left * right) }
    else { bitwise_v1(operator, left, right, 32) }
}

pub open spec fn kir_u32_eval_v1(operator: nat, left: int, right: int) -> int {
    if operator == 1 { norm_u32_v1(left + right) }
    else if operator == 2 { norm_u32_v1(left - right) }
    else if operator == 3 { norm_u32_v1(left * right) }
    else { bitwise_v1(operator, left, right, 32) }
}

pub open spec fn mir_effects_v1(
    operator: nat,
    left: int,
    right: int,
    destination: int,
) -> Seq<int> {
    seq![left, right, destination, mir_u32_eval_v1(operator, left, right)]
}

pub open spec fn kir_effects_v1(
    operator: nat,
    left: int,
    right: int,
    destination: int,
) -> Seq<int> {
    seq![left, right, destination, kir_u32_eval_v1(operator, left, right)]
}

/// For the closed operator set, equal selected-element inputs and destination
/// refine to the same wrapping output and ordered abstract read/write trace.
pub proof fn fe2o3_mir_kir_u32_element_refines_v1(
    mir_operator: nat,
    kir_operator: nat,
    mir_left: int,
    mir_right: int,
    mir_destination: int,
    kir_left: int,
    kir_right: int,
    kir_destination: int,
)
    requires
        1 <= mir_operator <= 6,
        kir_operator == mir_operator,
        kir_left == mir_left,
        kir_right == mir_right,
        kir_destination == mir_destination,
    ensures
        mir_u32_eval_v1(mir_operator, mir_left, mir_right)
            == kir_u32_eval_v1(kir_operator, kir_left, kir_right),
        mir_effects_v1(mir_operator, mir_left, mir_right, mir_destination)
            == kir_effects_v1(kir_operator, kir_left, kir_right, kir_destination),
{
}

}

fn main() {}
