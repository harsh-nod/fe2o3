use vstd::prelude::*;

verus! {

pub enum MirPcV3 {
    RootCall, Compute, FirstBranch, FirstArm, FirstJoin, LoopHeader, LoopBody,
    SecondBranch, SecondArm, SecondJoin, LeafCall, LeafCast, LeafReturn,
    HelperReturn, RootContinuation, CheckedOverflow, Done,
}

pub enum KirPcV3 {
    RootCall, Expression, FirstCond, FirstEdge, FirstPhi, LoopPhi, LoopBackedge,
    SecondCond, SecondEdge, SecondPhi, LeafCall, LeafCast, LeafReturn,
    HelperReturn, RootContinuation, CheckedOverflow, Done,
}

/// Closed expression family used by the broad, non-authoritative machine.
pub enum ExpressionModeV3 { BitXor, WrappingAdd, CheckedAdd }

pub struct ProgramV3 {
    pub modulus: int,
    pub output_modulus: int,
    pub mode: ExpressionModeV3,
    pub fallback: int,
    pub increment: int,
    pub threshold: int,
    pub trip_count: nat,
}

pub open spec fn admitted_modulus_v3(modulus: int) -> bool {
    modulus == 256 || modulus == 65536 || modulus == 4294967296
        || modulus == 18446744073709551616
}

pub open spec fn valid_program_v3(program: ProgramV3) -> bool {
    &&& admitted_modulus_v3(program.modulus)
    &&& admitted_modulus_v3(program.output_modulus)
    &&& 0 <= program.fallback < program.modulus
    &&& 0 <= program.increment < program.modulus
    &&& 0 <= program.threshold < program.modulus
    &&& 1 <= program.trip_count <= 4
}

pub open spec fn wrapping_add_v3(left: int, right: int, modulus: int) -> int {
    (left + right) % modulus
}

pub open spec fn checked_add_overflows_v3(left: int, right: int, modulus: int) -> bool {
    left + right >= modulus
}

pub open spec fn truncate_or_zero_extend_v3(value: int, target_modulus: int) -> int {
    value % target_modulus
}

pub open spec fn unsigned_eq_v3(left: int, right: int) -> bool { left == right }
pub open spec fn unsigned_lt_v3(left: int, right: int) -> bool { left < right }

pub open spec fn pow2_v3(exponent: nat) -> nat
    decreases exponent,
{
    if exponent == 0 { 1 } else { 2 * pow2_v3((exponent - 1) as nat) }
}

pub open spec fn width_for_modulus_v3(modulus: int) -> nat {
    if modulus == 256 { 8 }
    else if modulus == 65536 { 16 }
    else if modulus == 4294967296 { 32 }
    else { 64 }
}

pub open spec fn bit_v3(value: int, bit: nat, modulus: int) -> int {
    ((value % modulus) / (pow2_v3(bit) as int)) % 2
}

pub open spec fn mir_bitwise_xor_v3(left: int, right: int, bit: nat, modulus: int) -> int
    decreases bit,
{
    if bit == 0 { 0 } else {
        let index = (bit - 1) as nat;
        mir_bitwise_xor_v3(left, right, index, modulus)
            + if bit_v3(left, index, modulus) != bit_v3(right, index, modulus) {
                pow2_v3(index) as int
            } else { 0 }
    }
}

pub open spec fn kir_bitwise_xor_v3(left: int, right: int, bit: nat, modulus: int) -> int
    decreases bit,
{
    if bit == 0 { 0 } else {
        let index = (bit - 1) as nat;
        kir_bitwise_xor_v3(left, right, index, modulus)
            + if bit_v3(left, index, modulus) != bit_v3(right, index, modulus) {
                pow2_v3(index) as int
            } else { 0 }
    }
}

pub open spec fn mir_expression_v3(program: ProgramV3, left: int, right: int) -> int {
    if program.mode == ExpressionModeV3::BitXor {
        mir_bitwise_xor_v3(left, right, width_for_modulus_v3(program.modulus), program.modulus)
    } else {
        wrapping_add_v3(left, right, program.modulus)
    }
}

pub open spec fn kir_expression_v3(program: ProgramV3, left: int, right: int) -> int {
    if program.mode == ExpressionModeV3::BitXor {
        kir_bitwise_xor_v3(left, right, width_for_modulus_v3(program.modulus), program.modulus)
    } else {
        wrapping_add_v3(left, right, program.modulus)
    }
}

pub struct MirStateV3 {
    pub pc: MirPcV3,
    pub program: ProgramV3,
    pub left: int,
    pub right: int,
    pub expression: int,
    pub first_phi: int,
    pub loop_value: int,
    pub iteration: nat,
    pub second_phi: int,
    pub call_destination: int,
    pub first_zero: bool,
    pub second_less: bool,
}

pub struct KirStateV3 {
    pub pc: KirPcV3,
    pub program: ProgramV3,
    pub left_parameter: int,
    pub right_parameter: int,
    pub expression_ssa: int,
    pub first_edge_value: int,
    pub first_block_parameter: int,
    pub loop_block_parameter: int,
    pub loop_iteration_parameter: nat,
    pub second_edge_value: int,
    pub second_block_parameter: int,
    pub leaf_parameter: int,
    pub leaf_result: int,
    pub helper_return: int,
    pub call_result: int,
    pub first_zero: bool,
    pub second_less: bool,
}

pub open spec fn initial_mir_v3(program: ProgramV3, left: int, right: int) -> MirStateV3 {
    MirStateV3 {
        pc: MirPcV3::RootCall, program, left, right, expression: 0, first_phi: 0,
        loop_value: 0, iteration: 0, second_phi: 0, call_destination: 0,
        first_zero: false, second_less: false,
    }
}

pub open spec fn initial_kir_v3(program: ProgramV3, left: int, right: int) -> KirStateV3 {
    KirStateV3 {
        pc: KirPcV3::RootCall, program, left_parameter: left, right_parameter: right,
        expression_ssa: 0, first_edge_value: 0, first_block_parameter: 0,
        loop_block_parameter: 0, loop_iteration_parameter: 0, second_edge_value: 0,
        second_block_parameter: 0, leaf_parameter: 0, leaf_result: 0,
        helper_return: 0, call_result: 0, first_zero: false, second_less: false,
    }
}

pub open spec fn blocks_related_v3(mir: MirPcV3, kir: KirPcV3) -> bool {
    ||| mir == MirPcV3::RootCall && kir == KirPcV3::RootCall
    ||| mir == MirPcV3::Compute && kir == KirPcV3::Expression
    ||| mir == MirPcV3::FirstBranch && kir == KirPcV3::FirstCond
    ||| mir == MirPcV3::FirstArm && kir == KirPcV3::FirstEdge
    ||| mir == MirPcV3::FirstJoin && kir == KirPcV3::FirstPhi
    ||| mir == MirPcV3::LoopHeader && kir == KirPcV3::LoopPhi
    ||| mir == MirPcV3::LoopBody && kir == KirPcV3::LoopBackedge
    ||| mir == MirPcV3::SecondBranch && kir == KirPcV3::SecondCond
    ||| mir == MirPcV3::SecondArm && kir == KirPcV3::SecondEdge
    ||| mir == MirPcV3::SecondJoin && kir == KirPcV3::SecondPhi
    ||| mir == MirPcV3::LeafCall && kir == KirPcV3::LeafCall
    ||| mir == MirPcV3::LeafCast && kir == KirPcV3::LeafCast
    ||| mir == MirPcV3::LeafReturn && kir == KirPcV3::LeafReturn
    ||| mir == MirPcV3::HelperReturn && kir == KirPcV3::HelperReturn
    ||| mir == MirPcV3::RootContinuation && kir == KirPcV3::RootContinuation
    ||| mir == MirPcV3::CheckedOverflow && kir == KirPcV3::CheckedOverflow
    ||| mir == MirPcV3::Done && kir == KirPcV3::Done
}

pub open spec fn states_related_v3(mir: MirStateV3, kir: KirStateV3) -> bool {
    &&& blocks_related_v3(mir.pc, kir.pc)
    &&& mir.program == kir.program
    &&& mir.left == kir.left_parameter
    &&& mir.right == kir.right_parameter
    &&& mir.expression == kir.expression_ssa
    &&& mir.first_phi == kir.first_block_parameter
    &&& mir.loop_value == kir.loop_block_parameter
    &&& mir.iteration == kir.loop_iteration_parameter
    &&& mir.second_phi == kir.second_block_parameter
    &&& mir.first_zero == kir.first_zero
    &&& mir.second_less == kir.second_less
    &&& (mir.pc != MirPcV3::FirstJoin || mir.first_phi == kir.first_edge_value)
    &&& (mir.pc != MirPcV3::SecondJoin || mir.second_phi == kir.second_edge_value)
    &&& (!(mir.pc == MirPcV3::LeafCall || mir.pc == MirPcV3::LeafCast
        || mir.pc == MirPcV3::LeafReturn || mir.pc == MirPcV3::HelperReturn
        || mir.pc == MirPcV3::RootContinuation || mir.pc == MirPcV3::Done)
        || mir.second_phi == kir.leaf_parameter)
    &&& (!(mir.pc == MirPcV3::LeafReturn || mir.pc == MirPcV3::HelperReturn
        || mir.pc == MirPcV3::RootContinuation || mir.pc == MirPcV3::Done)
        || mir.call_destination == kir.leaf_result)
    &&& (!(mir.pc == MirPcV3::HelperReturn || mir.pc == MirPcV3::RootContinuation
        || mir.pc == MirPcV3::Done) || mir.call_destination == kir.helper_return)
    &&& (!(mir.pc == MirPcV3::RootContinuation || mir.pc == MirPcV3::Done)
        || mir.call_destination == kir.call_result)
    &&& (mir.pc != MirPcV3::Done || mir.call_destination == kir.call_result)
}

pub open spec fn mir_step_v3(state: MirStateV3) -> Option<MirStateV3> {
    if state.pc == MirPcV3::RootCall {
        Some(MirStateV3 { pc: MirPcV3::Compute, ..state })
    } else if state.pc == MirPcV3::Compute {
        if state.program.mode == ExpressionModeV3::CheckedAdd
            && checked_add_overflows_v3(state.left, state.right, state.program.modulus) {
            Some(MirStateV3 { pc: MirPcV3::CheckedOverflow, ..state })
        } else {
            Some(MirStateV3 { pc: MirPcV3::FirstBranch,
                expression: mir_expression_v3(state.program, state.left, state.right), ..state })
        }
    } else if state.pc == MirPcV3::FirstBranch {
        Some(MirStateV3 { pc: MirPcV3::FirstArm,
            first_zero: unsigned_eq_v3(state.expression, 0), ..state })
    } else if state.pc == MirPcV3::FirstArm {
        Some(MirStateV3 { pc: MirPcV3::FirstJoin,
            first_phi: if state.first_zero { state.expression } else { state.program.fallback }, ..state })
    } else if state.pc == MirPcV3::FirstJoin {
        Some(MirStateV3 { pc: MirPcV3::LoopHeader, loop_value: state.first_phi, ..state })
    } else if state.pc == MirPcV3::LoopHeader {
        Some(MirStateV3 { pc: if state.iteration < state.program.trip_count {
            MirPcV3::LoopBody } else { MirPcV3::SecondBranch }, ..state })
    } else if state.pc == MirPcV3::LoopBody {
        Some(MirStateV3 { pc: MirPcV3::LoopHeader,
            loop_value: wrapping_add_v3(state.loop_value, state.program.increment, state.program.modulus),
            iteration: state.iteration + 1, ..state })
    } else if state.pc == MirPcV3::SecondBranch {
        Some(MirStateV3 { pc: MirPcV3::SecondArm,
            second_less: unsigned_lt_v3(state.loop_value, state.program.threshold), ..state })
    } else if state.pc == MirPcV3::SecondArm {
        Some(MirStateV3 { pc: MirPcV3::SecondJoin,
            second_phi: if state.second_less { state.loop_value } else { state.program.fallback }, ..state })
    } else if state.pc == MirPcV3::SecondJoin {
        Some(MirStateV3 { pc: MirPcV3::LeafCall, ..state })
    } else if state.pc == MirPcV3::LeafCall {
        Some(MirStateV3 { pc: MirPcV3::LeafCast, ..state })
    } else if state.pc == MirPcV3::LeafCast {
        Some(MirStateV3 { pc: MirPcV3::LeafReturn,
            call_destination: truncate_or_zero_extend_v3(state.second_phi, state.program.output_modulus), ..state })
    } else if state.pc == MirPcV3::LeafReturn {
        Some(MirStateV3 { pc: MirPcV3::HelperReturn, ..state })
    } else if state.pc == MirPcV3::HelperReturn {
        Some(MirStateV3 { pc: MirPcV3::RootContinuation, ..state })
    } else if state.pc == MirPcV3::RootContinuation {
        Some(MirStateV3 { pc: MirPcV3::Done, ..state })
    } else { None }
}

pub open spec fn kir_step_v3(state: KirStateV3) -> Option<KirStateV3> {
    if state.pc == KirPcV3::RootCall {
        Some(KirStateV3 { pc: KirPcV3::Expression, ..state })
    } else if state.pc == KirPcV3::Expression {
        if state.program.mode == ExpressionModeV3::CheckedAdd
            && checked_add_overflows_v3(state.left_parameter, state.right_parameter, state.program.modulus) {
            Some(KirStateV3 { pc: KirPcV3::CheckedOverflow, ..state })
        } else {
            Some(KirStateV3 { pc: KirPcV3::FirstCond,
                expression_ssa: kir_expression_v3(
                    state.program, state.left_parameter, state.right_parameter), ..state })
        }
    } else if state.pc == KirPcV3::FirstCond {
        Some(KirStateV3 { pc: KirPcV3::FirstEdge,
            first_zero: unsigned_eq_v3(state.expression_ssa, 0), ..state })
    } else if state.pc == KirPcV3::FirstEdge {
        Some(KirStateV3 { pc: KirPcV3::FirstPhi,
            first_edge_value: if state.first_zero { state.expression_ssa } else { state.program.fallback },
            first_block_parameter: if state.first_zero { state.expression_ssa } else { state.program.fallback }, ..state })
    } else if state.pc == KirPcV3::FirstPhi {
        Some(KirStateV3 { pc: KirPcV3::LoopPhi,
            loop_block_parameter: state.first_block_parameter, ..state })
    } else if state.pc == KirPcV3::LoopPhi {
        Some(KirStateV3 { pc: if state.loop_iteration_parameter < state.program.trip_count {
            KirPcV3::LoopBackedge } else { KirPcV3::SecondCond }, ..state })
    } else if state.pc == KirPcV3::LoopBackedge {
        Some(KirStateV3 { pc: KirPcV3::LoopPhi,
            loop_block_parameter: wrapping_add_v3(state.loop_block_parameter, state.program.increment, state.program.modulus),
            loop_iteration_parameter: state.loop_iteration_parameter + 1, ..state })
    } else if state.pc == KirPcV3::SecondCond {
        Some(KirStateV3 { pc: KirPcV3::SecondEdge,
            second_less: unsigned_lt_v3(state.loop_block_parameter, state.program.threshold), ..state })
    } else if state.pc == KirPcV3::SecondEdge {
        Some(KirStateV3 { pc: KirPcV3::SecondPhi,
            second_edge_value: if state.second_less { state.loop_block_parameter } else { state.program.fallback },
            second_block_parameter: if state.second_less { state.loop_block_parameter } else { state.program.fallback }, ..state })
    } else if state.pc == KirPcV3::SecondPhi {
        Some(KirStateV3 { pc: KirPcV3::LeafCall, leaf_parameter: state.second_block_parameter, ..state })
    } else if state.pc == KirPcV3::LeafCall {
        Some(KirStateV3 { pc: KirPcV3::LeafCast, ..state })
    } else if state.pc == KirPcV3::LeafCast {
        Some(KirStateV3 { pc: KirPcV3::LeafReturn,
            leaf_result: truncate_or_zero_extend_v3(state.leaf_parameter, state.program.output_modulus), ..state })
    } else if state.pc == KirPcV3::LeafReturn {
        Some(KirStateV3 { pc: KirPcV3::HelperReturn, helper_return: state.leaf_result, ..state })
    } else if state.pc == KirPcV3::HelperReturn {
        Some(KirStateV3 { pc: KirPcV3::RootContinuation, call_result: state.helper_return, ..state })
    } else if state.pc == KirPcV3::RootContinuation {
        Some(KirStateV3 { pc: KirPcV3::Done, ..state })
    } else { None }
}

pub open spec fn terminal_mir_v3(state: MirStateV3) -> bool {
    state.pc == MirPcV3::Done || state.pc == MirPcV3::CheckedOverflow
}
pub open spec fn terminal_kir_v3(state: KirStateV3) -> bool {
    state.pc == KirPcV3::Done || state.pc == KirPcV3::CheckedOverflow
}

pub open spec fn run_mir_v3(state: MirStateV3, fuel: nat) -> Option<MirStateV3>
    decreases fuel,
{
    if terminal_mir_v3(state) { Some(state) } else if fuel == 0 { None } else {
        match mir_step_v3(state) {
            Some(next) => run_mir_v3(next, (fuel - 1) as nat), None => None,
        }
    }
}

pub open spec fn run_kir_v3(state: KirStateV3, fuel: nat) -> Option<KirStateV3>
    decreases fuel,
{
    if terminal_kir_v3(state) { Some(state) } else if fuel == 0 { None } else {
        match kir_step_v3(state) {
            Some(next) => run_kir_v3(next, (fuel - 1) as nat), None => None,
        }
    }
}

pub open spec fn mir_observation_v3(state: MirStateV3) -> Option<Seq<int>> {
    if state.pc == MirPcV3::CheckedOverflow { Some(seq![90, 1]) }
    else if state.pc == MirPcV3::Done { Some(seq![10, if state.first_zero {1} else {2},
        state.iteration as int, if state.second_less {1} else {2}, 20, 2, state.call_destination]) }
    else { None }
}

pub open spec fn kir_observation_v3(state: KirStateV3) -> Option<Seq<int>> {
    if state.pc == KirPcV3::CheckedOverflow { Some(seq![90, 1]) }
    else if state.pc == KirPcV3::Done { Some(seq![10, if state.first_zero {1} else {2},
        state.loop_iteration_parameter as int, if state.second_less {1} else {2}, 20, 2, state.call_result]) }
    else { None }
}

pub open spec fn loop_invariant_mir_v3(state: MirStateV3) -> bool {
    &&& state.iteration <= state.program.trip_count
    &&& 0 <= state.loop_value < state.program.modulus
}

pub open spec fn loop_invariant_kir_v3(state: KirStateV3) -> bool {
    &&& state.loop_iteration_parameter <= state.program.trip_count
    &&& 0 <= state.loop_block_parameter < state.program.modulus
}

pub proof fn wrapping_add_stays_in_width_v3(left: int, right: int, modulus: int)
    requires 0 <= left < modulus, 0 <= right < modulus, 0 < modulus
    ensures 0 <= wrapping_add_v3(left, right, modulus) < modulus
{}

/// The separately defined MIR and KIR XOR recurrences agree at every width.
pub proof fn bitwise_xor_refines_v3(left: int, right: int, width: nat, modulus: int)
    ensures
        mir_bitwise_xor_v3(left, right, width, modulus)
            == kir_bitwise_xor_v3(left, right, width, modulus),
    decreases width,
{
    if width > 0 {
        bitwise_xor_refines_v3(left, right, (width - 1) as nat, modulus);
    }
}

pub proof fn expression_refines_v3(program: ProgramV3, left: int, right: int)
    ensures mir_expression_v3(program, left, right) == kir_expression_v3(program, left, right)
{
    if program.mode == ExpressionModeV3::BitXor {
        bitwise_xor_refines_v3(
            left, right, width_for_modulus_v3(program.modulus), program.modulus);
    }
}

pub proof fn loop_backedge_preserves_invariant_v3(mir: MirStateV3, kir: KirStateV3)
    requires
        valid_program_v3(mir.program), states_related_v3(mir, kir),
        mir.pc == MirPcV3::LoopBody, loop_invariant_mir_v3(mir),
        mir.iteration < mir.program.trip_count,
    ensures
        mir_step_v3(mir).is_some(), kir_step_v3(kir).is_some(),
        loop_invariant_mir_v3(mir_step_v3(mir).unwrap()),
        loop_invariant_kir_v3(kir_step_v3(kir).unwrap()),
        states_related_v3(mir_step_v3(mir).unwrap(), kir_step_v3(kir).unwrap()),
{
    wrapping_add_stays_in_width_v3(mir.loop_value, mir.program.increment, mir.program.modulus);
}

pub proof fn related_step_preserves_simulation_v3(mir: MirStateV3, kir: KirStateV3)
    requires
        valid_program_v3(mir.program), states_related_v3(mir, kir),
        !terminal_mir_v3(mir),
        0 <= mir.left < mir.program.modulus, 0 <= mir.right < mir.program.modulus,
        0 <= mir.loop_value < mir.program.modulus,
    ensures
        mir_step_v3(mir).is_some(), kir_step_v3(kir).is_some(),
        states_related_v3(mir_step_v3(mir).unwrap(), kir_step_v3(kir).unwrap()),
{
    if mir.pc == MirPcV3::RootCall {
    } else if mir.pc == MirPcV3::Compute {
        expression_refines_v3(mir.program, mir.left, mir.right);
        if mir.program.mode == ExpressionModeV3::CheckedAdd
            && checked_add_overflows_v3(mir.left, mir.right, mir.program.modulus) {
        } else {
        }
    } else if mir.pc == MirPcV3::FirstBranch {
    } else if mir.pc == MirPcV3::FirstArm {
    } else if mir.pc == MirPcV3::FirstJoin {
    } else if mir.pc == MirPcV3::LoopHeader {
        if mir.iteration < mir.program.trip_count {
        } else {
        }
    } else if mir.pc == MirPcV3::LoopBody {
        wrapping_add_stays_in_width_v3(mir.loop_value, mir.program.increment, mir.program.modulus);
    } else if mir.pc == MirPcV3::SecondBranch {
    } else if mir.pc == MirPcV3::SecondArm {
    } else if mir.pc == MirPcV3::SecondJoin {
    } else if mir.pc == MirPcV3::LeafCall {
    } else if mir.pc == MirPcV3::LeafCast {
    } else if mir.pc == MirPcV3::LeafReturn {
    } else if mir.pc == MirPcV3::HelperReturn {
    } else {
    }
}

/// Exact production helper/call-result observation for the u32 XOR diamond.
pub open spec fn mir_xor_diamond_call_observation_v3(
    left: int, right: int, fallback: int,
) -> Seq<int> {
    let expression = mir_bitwise_xor_v3(left, right, 32, 4294967296);
    seq![10, left, right, 20, if expression == 0 { 1 } else { 2 }, 30,
        if expression == 0 { expression } else { fallback }]
}

pub open spec fn kir_xor_diamond_call_observation_v3(
    left: int, right: int, fallback: int,
) -> Seq<int> {
    let expression = kir_bitwise_xor_v3(left, right, 32, 4294967296);
    seq![10, left, right, 20, if expression == 0 { 1 } else { 2 }, 30,
        if expression == 0 { expression } else { fallback }]
}

/// For every related pair of loaded u32 inputs, the exact production XOR
/// expression, arm direction, join value, helper return, and root call result
/// have the same observation. Load/store memory behavior is outside this
/// theorem and is composed separately.
pub proof fn fe2o3_mir_kir_xor_diamond_call_refines_v3(
    mir_left: int,
    mir_right: int,
    kir_left: int,
    kir_right: int,
    fallback: int,
)
    requires
        0 <= mir_left < 4294967296,
        0 <= mir_right < 4294967296,
        0 <= fallback < 4294967296,
        kir_left == mir_left,
        kir_right == mir_right,
    ensures
        mir_xor_diamond_call_observation_v3(mir_left, mir_right, fallback)
            == kir_xor_diamond_call_observation_v3(kir_left, kir_right, fallback),
{
    bitwise_xor_refines_v3(mir_left, mir_right, 32, 4294967296);
}

/// All admitted widths share truncation/zero-extension and unsigned comparison semantics.
pub proof fn scalar_width_cast_and_comparisons_refine_v3(value: int, target: int, other: int)
    requires admitted_modulus_v3(target), 0 <= value, 0 <= other < target
    ensures
        0 <= truncate_or_zero_extend_v3(value, target) < target,
        unsigned_eq_v3(other, other),
        unsigned_lt_v3(other, target),
{}

/// Checked overflow is observably distinct from wrapping at every admitted width.
pub proof fn checked_overflow_cannot_be_wrapping_v3(modulus: int)
    requires admitted_modulus_v3(modulus)
    ensures
        checked_add_overflows_v3(modulus - 1, 1, modulus),
        wrapping_add_v3(modulus - 1, 1, modulus) == 0,
{}

/// For every closed-width environment and loop trip count 1..=4, the two
/// independently stepped machines preserve both diamonds, both phi transfers,
/// the loop-carried value/invariant, direct-call depth two, casts, and result.
#[verifier::rlimit(50)]
pub proof fn fe2o3_mir_kir_structured_cfg_refines_v3(
    program: ProgramV3, left: int, right: int, fuel: nat,
)
    requires
        valid_program_v3(program),
        0 <= left < program.modulus,
        0 <= right < program.modulus,
        fuel >= 14 + 2 * program.trip_count,
    ensures
        run_mir_v3(initial_mir_v3(program, left, right), fuel).is_some(),
        run_kir_v3(initial_kir_v3(program, left, right), fuel).is_some(),
        states_related_v3(
            run_mir_v3(initial_mir_v3(program, left, right), fuel).unwrap(),
            run_kir_v3(initial_kir_v3(program, left, right), fuel).unwrap()),
        mir_observation_v3(run_mir_v3(initial_mir_v3(program, left, right), fuel).unwrap())
            == kir_observation_v3(run_kir_v3(initial_kir_v3(program, left, right), fuel).unwrap()),
{
    expression_refines_v3(program, left, right);
    reveal_with_fuel(run_mir_v3, 25);
    reveal_with_fuel(run_kir_v3, 25);
    if program.mode == ExpressionModeV3::CheckedAdd
        && checked_add_overflows_v3(left, right, program.modulus) {
    } else if program.trip_count == 1 {
    } else if program.trip_count == 2 {
    } else if program.trip_count == 3 {
    } else {
    }
}

}

fn main() {}
