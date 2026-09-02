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
    if source_operator == 11 { norm_u32_v2(left + right) }
    else if source_operator == 12 { norm_u32_v2(left - right) }
    else if source_operator == 13 { norm_u32_v2(left * right) }
    else { bitwise_v2((source_operator - 10) as nat, left, right, 32) }
}

/// Semantic MIR uses the closed opcode range 1..=6.
pub open spec fn mir_u32_eval_v2(mir_operator: nat, left: int, right: int) -> int {
    normalized_u32_eval_v2(mir_operator, left, right)
}

/// KIR uses the deliberately distinct closed opcode range 101..=106.
pub open spec fn kir_u32_eval_v2(kir_operator: nat, left: int, right: int) -> int {
    if kir_operator == 101 { norm_u32_v2(left + right) }
    else if kir_operator == 102 { norm_u32_v2(left - right) }
    else if kir_operator == 103 { norm_u32_v2(left * right) }
    else { bitwise_v2((kir_operator - 100) as nat, left, right, 32) }
}

/// Exact closed conversion performed by the source-to-semantic-MIR checker.
pub open spec fn source_mir_operator_relation_v2(source: nat, mir: nat) -> bool {
    (source == 11 && mir == 1)
        || (source == 12 && mir == 2)
        || (source == 13 && mir == 3)
        || (source == 14 && mir == 4)
        || (source == 15 && mir == 5)
        || (source == 16 && mir == 6)
}

/// Exact closed conversion performed by the semantic-MIR-to-KIR checker.
pub open spec fn mir_kir_operator_relation_v2(mir: nat, kir: nat) -> bool {
    (mir == 1 && kir == 101)
        || (mir == 2 && kir == 102)
        || (mir == 3 && kir == 103)
        || (mir == 4 && kir == 104)
        || (mir == 5 && kir == 105)
        || (mir == 6 && kir == 106)
}

/// Exact frontend binding-to-semantic-local relation and equal operand valuation.
pub open spec fn source_mir_environment_relation_v2(
    source_values: Map<int, int>,
    mir_values: Map<int, int>,
    source_to_mir: Map<int, int>,
    source_left: int,
    source_right: int,
    source_destination: int,
    mir_left: int,
    mir_right: int,
    mir_destination: int,
) -> bool {
    source_values.dom().contains(source_left)
        && source_values.dom().contains(source_right)
        && mir_values.dom().contains(mir_left)
        && mir_values.dom().contains(mir_right)
        && source_to_mir.dom().contains(source_left)
        && source_to_mir.dom().contains(source_right)
        && source_to_mir.dom().contains(source_destination)
        && source_to_mir[source_left] == mir_left
        && source_to_mir[source_right] == mir_right
        && source_to_mir[source_destination] == mir_destination
        && source_values[source_left] == mir_values[mir_left]
        && source_values[source_right] == mir_values[mir_right]
}

/// Exact semantic-local-to-KIR-SSA relation and equal parameter valuation.
pub open spec fn mir_kir_environment_relation_v2(
    mir_values: Map<int, int>,
    kir_values: Map<int, int>,
    mir_to_kir_ssa: Map<int, int>,
    mir_left: int,
    mir_right: int,
    mir_destination: int,
    kir_left_parameter: int,
    kir_right_parameter: int,
    kir_result: int,
) -> bool {
    mir_values.dom().contains(mir_left)
        && mir_values.dom().contains(mir_right)
        && kir_values.dom().contains(kir_left_parameter)
        && kir_values.dom().contains(kir_right_parameter)
        && mir_to_kir_ssa.dom().contains(mir_left)
        && mir_to_kir_ssa.dom().contains(mir_right)
        && mir_to_kir_ssa.dom().contains(mir_destination)
        && mir_to_kir_ssa[mir_left] == kir_left_parameter
        && mir_to_kir_ssa[mir_right] == kir_right_parameter
        && mir_to_kir_ssa[mir_destination] == kir_result
        && mir_values[mir_left] == kir_values[kir_left_parameter]
        && mir_values[mir_right] == kir_values[kir_right_parameter]
}

pub open spec fn source_step_v2(
    operator: nat,
    values: Map<int, int>,
    left: int,
    right: int,
    destination: int,
) -> Map<int, int> {
    values.insert(destination, source_u32_eval_v2(operator, values[left], values[right]))
}

pub open spec fn mir_step_v2(
    operator: nat,
    values: Map<int, int>,
    left: int,
    right: int,
    destination: int,
) -> Map<int, int> {
    values.insert(destination, mir_u32_eval_v2(operator, values[left], values[right]))
}

pub open spec fn kir_step_v2(
    operator: nat,
    values: Map<int, int>,
    left_parameter: int,
    right_parameter: int,
    result: int,
) -> Map<int, int> {
    values.insert(result, kir_u32_eval_v2(operator, values[left_parameter], values[right_parameter]))
}

/// The accepted operation is effect-free, so the observation contains only
/// the ordered operands and result rather than a fabricated memory-effect trace.
pub open spec fn source_observation_v2(
    operator: nat,
    values: Map<int, int>,
    left: int,
    right: int,
) -> Seq<int> {
    seq![values[left], values[right], source_u32_eval_v2(operator, values[left], values[right])]
}

pub open spec fn mir_observation_v2(
    operator: nat,
    values: Map<int, int>,
    left: int,
    right: int,
) -> Seq<int> {
    seq![values[left], values[right], mir_u32_eval_v2(operator, values[left], values[right])]
}

pub open spec fn kir_observation_v2(
    operator: nat,
    values: Map<int, int>,
    left_parameter: int,
    right_parameter: int,
) -> Seq<int> {
    seq![
        values[left_parameter],
        values[right_parameter],
        kir_u32_eval_v2(operator, values[left_parameter], values[right_parameter]),
    ]
}

pub proof fn source_to_mir_environment_step_v2(
    source_operator: nat,
    mir_operator: nat,
    source_values: Map<int, int>,
    mir_values: Map<int, int>,
    source_to_mir: Map<int, int>,
    source_left: int,
    source_right: int,
    source_destination: int,
    mir_left: int,
    mir_right: int,
    mir_destination: int,
)
    requires
        source_mir_operator_relation_v2(source_operator, mir_operator),
        source_mir_environment_relation_v2(
            source_values,
            mir_values,
            source_to_mir,
            source_left,
            source_right,
            source_destination,
            mir_left,
            mir_right,
            mir_destination,
        ),
    ensures
        source_observation_v2(source_operator, source_values, source_left, source_right)
            == mir_observation_v2(mir_operator, mir_values, mir_left, mir_right),
        source_step_v2(
            source_operator,
            source_values,
            source_left,
            source_right,
            source_destination,
        )[source_destination] == mir_step_v2(
            mir_operator,
            mir_values,
            mir_left,
            mir_right,
            mir_destination,
        )[mir_destination],
{
}

pub proof fn mir_to_kir_environment_step_v2(
    mir_operator: nat,
    kir_operator: nat,
    mir_values: Map<int, int>,
    kir_values: Map<int, int>,
    mir_to_kir_ssa: Map<int, int>,
    mir_left: int,
    mir_right: int,
    mir_destination: int,
    kir_left_parameter: int,
    kir_right_parameter: int,
    kir_result: int,
)
    requires
        mir_kir_operator_relation_v2(mir_operator, kir_operator),
        mir_kir_environment_relation_v2(
            mir_values,
            kir_values,
            mir_to_kir_ssa,
            mir_left,
            mir_right,
            mir_destination,
            kir_left_parameter,
            kir_right_parameter,
            kir_result,
        ),
    ensures
        mir_observation_v2(mir_operator, mir_values, mir_left, mir_right)
            == kir_observation_v2(
                kir_operator,
                kir_values,
                kir_left_parameter,
                kir_right_parameter,
            ),
        mir_step_v2(mir_operator, mir_values, mir_left, mir_right, mir_destination)[mir_destination]
            == kir_step_v2(
                kir_operator,
                kir_values,
                kir_left_parameter,
                kir_right_parameter,
                kir_result,
            )[kir_result],
{
}

/// Exact bounded composition. Identity integers are executable-checker guards,
/// not cryptographic proof inside Verus. The theorem is universal over source,
/// MIR, and KIR environments related by the exact local and SSA maps retained
/// by the production checker.
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
    source_values: Map<int, int>,
    mir_values: Map<int, int>,
    kir_values: Map<int, int>,
    source_to_mir: Map<int, int>,
    mir_to_kir_ssa: Map<int, int>,
    source_left: int,
    source_right: int,
    source_destination: int,
    mir_left: int,
    mir_right: int,
    mir_destination: int,
    kir_left_parameter: int,
    kir_right_parameter: int,
    kir_result: int,
)
    requires
        source_owner_identity != 0,
        source_expression_identity != 0,
        rustc_mir_body_identity != 0,
        kir_identity != 0,
        source_semantic_identity != 0,
        source_semantic_identity == mir_kir_semantic_identity,
        source_mir_operator_relation_v2(source_operator, mir_operator),
        mir_kir_operator_relation_v2(mir_operator, kir_operator),
        source_left != source_destination,
        source_right != source_destination,
        mir_left != mir_destination,
        mir_right != mir_destination,
        kir_left_parameter != kir_result,
        kir_right_parameter != kir_result,
        source_mir_environment_relation_v2(
            source_values,
            mir_values,
            source_to_mir,
            source_left,
            source_right,
            source_destination,
            mir_left,
            mir_right,
            mir_destination,
        ),
        mir_kir_environment_relation_v2(
            mir_values,
            kir_values,
            mir_to_kir_ssa,
            mir_left,
            mir_right,
            mir_destination,
            kir_left_parameter,
            kir_right_parameter,
            kir_result,
        ),
    ensures
        source_observation_v2(source_operator, source_values, source_left, source_right)
            == kir_observation_v2(
                kir_operator,
                kir_values,
                kir_left_parameter,
                kir_right_parameter,
            ),
        source_step_v2(
            source_operator,
            source_values,
            source_left,
            source_right,
            source_destination,
        )[source_destination] == mir_step_v2(
            mir_operator,
            mir_values,
            mir_left,
            mir_right,
            mir_destination,
        )[mir_destination],
        mir_step_v2(mir_operator, mir_values, mir_left, mir_right, mir_destination)[mir_destination]
            == kir_step_v2(
                kir_operator,
                kir_values,
                kir_left_parameter,
                kir_right_parameter,
                kir_result,
            )[kir_result],
{
    source_to_mir_environment_step_v2(
        source_operator,
        mir_operator,
        source_values,
        mir_values,
        source_to_mir,
        source_left,
        source_right,
        source_destination,
        mir_left,
        mir_right,
        mir_destination,
    );
    mir_to_kir_environment_step_v2(
        mir_operator,
        kir_operator,
        mir_values,
        kir_values,
        mir_to_kir_ssa,
        mir_left,
        mir_right,
        mir_destination,
        kir_left_parameter,
        kir_right_parameter,
        kir_result,
    );
}

}

fn main() {}
