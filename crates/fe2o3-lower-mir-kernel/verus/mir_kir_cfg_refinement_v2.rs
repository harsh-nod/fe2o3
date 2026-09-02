use vstd::prelude::*;

verus! {

pub open spec fn mir_result_v2(input: int, fallback: int) -> int {
    if input == 0 { input } else { fallback }
}

pub open spec fn kir_result_v2(
    input: int,
    kir_fallback: int,
    zero_case: int,
    zero_edge_value: int,
    default_edge_value: int,
) -> int {
    if input == zero_case { zero_edge_value } else { default_edge_value }
}

pub open spec fn consumes_steps_v2(fuel: nat, steps: nat) -> bool
    decreases steps,
{
    if steps == 0 {
        true
    } else {
        fuel > 0 && consumes_steps_v2((fuel - 1) as nat, (steps - 1) as nat)
    }
}

pub open spec fn mir_observation_v2(input: int, fallback: int, fuel: nat) -> Seq<int> {
    if !consumes_steps_v2(fuel, 6) {
        seq![-1]
    } else {
        let result = mir_result_v2(input, fallback);
        seq![0, if input == 0 { 1 } else { 2 }, 3, result]
    }
}

pub open spec fn kir_observation_v2(
    input: int,
    kir_fallback: int,
    fuel: nat,
    zero_case: int,
    zero_edge_value: int,
    default_edge_value: int,
    callee_is_helper: bool,
    return_is_join_parameter: bool,
) -> Seq<int> {
    if !consumes_steps_v2(fuel, 6) {
        seq![-1]
    } else if !callee_is_helper || !return_is_join_parameter {
        seq![-2]
    } else {
        let result = kir_result_v2(
            input,
            kir_fallback,
            zero_case,
            zero_edge_value,
            default_edge_value,
        );
        seq![0, if input == zero_case { 1 } else { 2 }, 3, result]
    }
}

/// Exact call, branch-direction, edge-value/phi, and return relations imply
/// equality of the fuel-bounded MIR and KIR observations.
pub proof fn fe2o3_mir_kir_u32_diamond_call_refines_v2(
    input: int,
    mir_fallback: int,
    kir_fallback: int,
    fuel: nat,
    zero_case: int,
    zero_edge_value: int,
    default_edge_value: int,
    callee_is_helper: bool,
    return_is_join_parameter: bool,
)
    requires
        0 <= input < 4294967296,
        0 <= mir_fallback < 4294967296,
        kir_fallback == mir_fallback,
        zero_case == 0,
        zero_edge_value == input,
        default_edge_value == kir_fallback,
        callee_is_helper,
        return_is_join_parameter,
    ensures
        mir_observation_v2(input, mir_fallback, fuel)
            == kir_observation_v2(
                input,
                kir_fallback,
                fuel,
                zero_case,
                zero_edge_value,
                default_edge_value,
                callee_is_helper,
                return_is_join_parameter,
            ),
{
}

pub proof fn insufficient_fuel_fails_closed_v2(input: int, fallback: int, fuel: nat)
    requires fuel < 6,
    ensures mir_observation_v2(input, fallback, fuel) == seq![-1],
{
    assert(!consumes_steps_v2(fuel, 6)) by (compute);
}

pub proof fn both_arms_are_observable_v2(fallback: int)
    requires fallback != 0,
    ensures
        mir_observation_v2(0, fallback, 6) != mir_observation_v2(1, fallback, 6),
{
    assert(consumes_steps_v2(6, 6)) by (compute);
}

}

fn main() {}
