use vstd::prelude::*;

verus! {

pub enum MirPcV2 {
    CallerCall,
    HelperSwitch,
    ZeroArm,
    NonzeroArm,
    HelperJoin,
    HelperReturn,
    CallerContinuation,
    Done,
}

pub enum KirPcV2 {
    RootCall,
    HelperEntry,
    ZeroBlock,
    NonzeroBlock,
    JoinBlock,
    HelperReturn,
    RootContinuation,
    Done,
}

pub enum SelectedArmV2 {
    Pending,
    Zero,
    Nonzero,
}

/// Values supplied to the admitted semantic-MIR caller.
pub struct MirEnvironmentV2 {
    pub argument: int,
    pub fallback: int,
}

/// Values supplied to independently executed canonical KIR.
pub struct KirEnvironmentV2 {
    pub root_parameter: int,
    pub fallback_constant: int,
}

/// Explicit semantic-MIR state. The helper return local and caller call
/// destination are separate cells.
pub struct MirMachineStateV2 {
    pub pc: MirPcV2,
    pub root_argument: int,
    pub fallback: int,
    pub helper_argument: int,
    pub helper_return: int,
    pub call_destination: int,
    pub selected_arm: SelectedArmV2,
}

/// Explicit canonical-KIR state. Edge value, join block parameter, helper
/// return operand, and root call result are separate SSA valuation entries.
pub struct KirMachineStateV2 {
    pub pc: KirPcV2,
    pub root_parameter: int,
    pub fallback_constant: int,
    pub helper_parameter: int,
    pub edge_value: int,
    pub join_parameter: int,
    pub helper_return_operand: int,
    pub call_result: int,
    pub selected_arm: SelectedArmV2,
}

pub open spec fn initial_environments_related_v2(
    mir: MirEnvironmentV2,
    kir: KirEnvironmentV2,
) -> bool {
    &&& 0 <= mir.argument < 4294967296
    &&& 0 <= mir.fallback < 4294967296
    &&& kir.root_parameter == mir.argument
    &&& kir.fallback_constant == mir.fallback
}

pub open spec fn initial_mir_state_v2(env: MirEnvironmentV2) -> MirMachineStateV2 {
    MirMachineStateV2 {
        pc: MirPcV2::CallerCall,
        root_argument: env.argument,
        fallback: env.fallback,
        helper_argument: 0,
        helper_return: 0,
        call_destination: 0,
        selected_arm: SelectedArmV2::Pending,
    }
}

pub open spec fn initial_kir_state_v2(env: KirEnvironmentV2) -> KirMachineStateV2 {
    KirMachineStateV2 {
        pc: KirPcV2::RootCall,
        root_parameter: env.root_parameter,
        fallback_constant: env.fallback_constant,
        helper_parameter: 0,
        edge_value: 0,
        join_parameter: 0,
        helper_return_operand: 0,
        call_result: 0,
        selected_arm: SelectedArmV2::Pending,
    }
}

/// Exact production block relation checked by the Rust evidence validator.
pub open spec fn blocks_related_v2(mir_pc: MirPcV2, kir_pc: KirPcV2) -> bool {
    ||| mir_pc == MirPcV2::CallerCall && kir_pc == KirPcV2::RootCall
    ||| mir_pc == MirPcV2::HelperSwitch && kir_pc == KirPcV2::HelperEntry
    ||| mir_pc == MirPcV2::ZeroArm && kir_pc == KirPcV2::ZeroBlock
    ||| mir_pc == MirPcV2::NonzeroArm && kir_pc == KirPcV2::NonzeroBlock
    ||| mir_pc == MirPcV2::HelperJoin && kir_pc == KirPcV2::JoinBlock
    ||| mir_pc == MirPcV2::HelperReturn && kir_pc == KirPcV2::HelperReturn
    ||| mir_pc == MirPcV2::CallerContinuation && kir_pc == KirPcV2::RootContinuation
    ||| mir_pc == MirPcV2::Done && kir_pc == KirPcV2::Done
}

pub open spec fn arm_has_been_selected_v2(pc: MirPcV2) -> bool {
    pc == MirPcV2::ZeroArm || pc == MirPcV2::NonzeroArm || pc == MirPcV2::HelperJoin
        || pc == MirPcV2::HelperReturn || pc == MirPcV2::CallerContinuation
        || pc == MirPcV2::Done
}

pub open spec fn arm_value_has_reached_join_v2(pc: MirPcV2) -> bool {
    pc == MirPcV2::HelperReturn || pc == MirPcV2::CallerContinuation || pc == MirPcV2::Done
}

pub open spec fn helper_has_returned_v2(pc: MirPcV2) -> bool {
    pc == MirPcV2::CallerContinuation || pc == MirPcV2::Done
}

pub open spec fn call_result_is_bound_v2(pc: MirPcV2) -> bool {
    pc == MirPcV2::Done
}

/// Simulation invariant covering semantic locals, KIR SSA valuation, arm-edge
/// to join-parameter transfer, and helper-return to caller-result transfer.
pub open spec fn machine_states_related_v2(
    mir: MirMachineStateV2,
    kir: KirMachineStateV2,
) -> bool {
    &&& blocks_related_v2(mir.pc, kir.pc)
    &&& mir.root_argument == kir.root_parameter
    &&& mir.fallback == kir.fallback_constant
    &&& (mir.pc == MirPcV2::CallerCall
        || mir.helper_argument == kir.helper_parameter)
    &&& mir.selected_arm == kir.selected_arm
    &&& (mir.pc != MirPcV2::HelperJoin
        || mir.helper_return == kir.edge_value)
    &&& (!arm_value_has_reached_join_v2(mir.pc)
        || mir.helper_return == kir.join_parameter)
    &&& (!helper_has_returned_v2(mir.pc)
        || kir.helper_return_operand == kir.join_parameter)
    &&& (!call_result_is_bound_v2(mir.pc)
        || mir.call_destination == mir.helper_return)
    &&& (!call_result_is_bound_v2(mir.pc)
        || kir.call_result == kir.helper_return_operand)
    &&& (!call_result_is_bound_v2(mir.pc)
        || mir.call_destination == kir.call_result)
}

/// One charged semantic-MIR macro transition.
pub open spec fn mir_macro_step_v2(state: MirMachineStateV2) -> Option<MirMachineStateV2> {
    if state.pc == MirPcV2::CallerCall {
        Some(MirMachineStateV2 {
            pc: MirPcV2::HelperSwitch,
            helper_argument: state.root_argument,
            ..state
        })
    } else if state.pc == MirPcV2::HelperSwitch {
        Some(MirMachineStateV2 {
            pc: if state.helper_argument == 0 { MirPcV2::ZeroArm } else { MirPcV2::NonzeroArm },
            ..state
        })
    } else if state.pc == MirPcV2::ZeroArm {
        Some(MirMachineStateV2 {
            pc: MirPcV2::HelperJoin,
            helper_return: state.helper_argument,
            selected_arm: SelectedArmV2::Zero,
            ..state
        })
    } else if state.pc == MirPcV2::NonzeroArm {
        Some(MirMachineStateV2 {
            pc: MirPcV2::HelperJoin,
            helper_return: state.fallback,
            selected_arm: SelectedArmV2::Nonzero,
            ..state
        })
    } else if state.pc == MirPcV2::HelperJoin {
        Some(MirMachineStateV2 { pc: MirPcV2::HelperReturn, ..state })
    } else if state.pc == MirPcV2::HelperReturn {
        Some(MirMachineStateV2 {
            pc: MirPcV2::CallerContinuation,
            ..state
        })
    } else if state.pc == MirPcV2::CallerContinuation {
        Some(MirMachineStateV2 {
            pc: MirPcV2::Done,
            call_destination: state.helper_return,
            ..state
        })
    } else {
        None
    }
}

/// One independently defined canonical-KIR macro transition.
pub open spec fn kir_macro_step_v2(state: KirMachineStateV2) -> Option<KirMachineStateV2> {
    if state.pc == KirPcV2::RootCall {
        Some(KirMachineStateV2 {
            pc: KirPcV2::HelperEntry,
            helper_parameter: state.root_parameter,
            ..state
        })
    } else if state.pc == KirPcV2::HelperEntry {
        Some(KirMachineStateV2 {
            pc: if state.helper_parameter == 0 { KirPcV2::ZeroBlock } else { KirPcV2::NonzeroBlock },
            ..state
        })
    } else if state.pc == KirPcV2::ZeroBlock {
        Some(KirMachineStateV2 {
            pc: KirPcV2::JoinBlock,
            edge_value: state.helper_parameter,
            selected_arm: SelectedArmV2::Zero,
            ..state
        })
    } else if state.pc == KirPcV2::NonzeroBlock {
        Some(KirMachineStateV2 {
            pc: KirPcV2::JoinBlock,
            edge_value: state.fallback_constant,
            selected_arm: SelectedArmV2::Nonzero,
            ..state
        })
    } else if state.pc == KirPcV2::JoinBlock {
        Some(KirMachineStateV2 {
            pc: KirPcV2::HelperReturn,
            join_parameter: state.edge_value,
            ..state
        })
    } else if state.pc == KirPcV2::HelperReturn {
        Some(KirMachineStateV2 {
            pc: KirPcV2::RootContinuation,
            helper_return_operand: state.join_parameter,
            ..state
        })
    } else if state.pc == KirPcV2::RootContinuation {
        Some(KirMachineStateV2 {
            pc: KirPcV2::Done,
            call_result: state.helper_return_operand,
            ..state
        })
    } else {
        None
    }
}

pub open spec fn run_mir_machine_v2(
    state: MirMachineStateV2,
    fuel: nat,
) -> Option<MirMachineStateV2>
    decreases fuel,
{
    if state.pc == MirPcV2::Done {
        Some(state)
    } else if fuel == 0 {
        None
    } else {
        match mir_macro_step_v2(state) {
            Some(next) => run_mir_machine_v2(next, (fuel - 1) as nat),
            None => None,
        }
    }
}

pub open spec fn run_kir_machine_v2(
    state: KirMachineStateV2,
    fuel: nat,
) -> Option<KirMachineStateV2>
    decreases fuel,
{
    if state.pc == KirPcV2::Done {
        Some(state)
    } else if fuel == 0 {
        None
    } else {
        match kir_macro_step_v2(state) {
            Some(next) => run_kir_machine_v2(next, (fuel - 1) as nat),
            None => None,
        }
    }
}

pub open spec fn mir_observation_v2(state: MirMachineStateV2) -> Option<Seq<int>> {
    if state.pc == MirPcV2::Done {
        Some(seq![10, if state.selected_arm == SelectedArmV2::Zero { 1 } else { 2 }, 30, state.call_destination])
    } else {
        None
    }
}

pub open spec fn kir_observation_v2(state: KirMachineStateV2) -> Option<Seq<int>> {
    if state.pc == KirPcV2::Done {
        Some(seq![10, if state.selected_arm == SelectedArmV2::Zero { 1 } else { 2 }, 30, state.call_result])
    } else {
        None
    }
}

pub open spec fn completed_runs_related_v2(
    mir: Option<MirMachineStateV2>,
    kir: Option<KirMachineStateV2>,
) -> bool {
    match (mir, kir) {
        (Some(mir_final), Some(kir_final)) =>
            machine_states_related_v2(mir_final, kir_final)
                && mir_observation_v2(mir_final) == kir_observation_v2(kir_final),
        (None, None) => true,
        _ => false,
    }
}

/// The block/local/SSA relation is an inductive simulation invariant for each
/// paired charged macro transition, not merely an end-state equality.
pub proof fn related_macro_step_preserves_simulation_v2(
    mir: MirMachineStateV2,
    kir: KirMachineStateV2,
)
    requires
        machine_states_related_v2(mir, kir),
        mir.pc != MirPcV2::Done,
    ensures
        mir_macro_step_v2(mir).is_some(),
        kir_macro_step_v2(kir).is_some(),
        machine_states_related_v2(
            mir_macro_step_v2(mir).unwrap(),
            kir_macro_step_v2(kir).unwrap(),
        ),
{
    if mir.pc == MirPcV2::CallerCall {
    } else if mir.pc == MirPcV2::HelperSwitch {
        if mir.helper_argument == 0 {
        } else {
        }
    } else if mir.pc == MirPcV2::ZeroArm {
    } else if mir.pc == MirPcV2::NonzeroArm {
    } else if mir.pc == MirPcV2::HelperJoin {
    } else if mir.pc == MirPcV2::HelperReturn {
    } else {
    }
}

/// For every related initial u32 environment, the independently stepped
/// semantic-MIR and canonical-KIR call slices reach the caller-continuation
/// observation boundary after six charged macro transitions with related state
/// and the same helper/call-result observation. The root continuation is not
/// executed by this theorem.
pub proof fn fe2o3_mir_kir_u32_diamond_call_refines_v2(
    mir_env: MirEnvironmentV2,
    kir_env: KirEnvironmentV2,
    fuel: nat,
)
    requires
        initial_environments_related_v2(mir_env, kir_env),
        fuel >= 6,
    ensures
        run_mir_machine_v2(initial_mir_state_v2(mir_env), fuel).is_some(),
        run_kir_machine_v2(initial_kir_state_v2(kir_env), fuel).is_some(),
        run_mir_machine_v2(initial_mir_state_v2(mir_env), fuel).unwrap().pc
            == MirPcV2::Done,
        run_kir_machine_v2(initial_kir_state_v2(kir_env), fuel).unwrap().pc
            == KirPcV2::Done,
        mir_observation_v2(
            run_mir_machine_v2(initial_mir_state_v2(mir_env), fuel).unwrap(),
        ).is_some(),
        kir_observation_v2(
            run_kir_machine_v2(initial_kir_state_v2(kir_env), fuel).unwrap(),
        ).is_some(),
        mir_observation_v2(
            run_mir_machine_v2(initial_mir_state_v2(mir_env), fuel).unwrap(),
        ) == kir_observation_v2(
            run_kir_machine_v2(initial_kir_state_v2(kir_env), fuel).unwrap(),
        ),
        completed_runs_related_v2(
            run_mir_machine_v2(initial_mir_state_v2(mir_env), fuel),
            run_kir_machine_v2(initial_kir_state_v2(kir_env), fuel),
        ),
{
    reveal_with_fuel(run_mir_machine_v2, 8);
    reveal_with_fuel(run_kir_machine_v2, 8);
    if mir_env.argument == 0 {
    } else {
    }
}

pub proof fn insufficient_fuel_fails_closed_v2(
    mir_env: MirEnvironmentV2,
    kir_env: KirEnvironmentV2,
    fuel: nat,
)
    requires
        initial_environments_related_v2(mir_env, kir_env),
        fuel < 6,
    ensures
        run_mir_machine_v2(initial_mir_state_v2(mir_env), fuel).is_none(),
        run_kir_machine_v2(initial_kir_state_v2(kir_env), fuel).is_none(),
{
    reveal_with_fuel(run_mir_machine_v2, 8);
    reveal_with_fuel(run_kir_machine_v2, 8);
    if mir_env.argument == 0 {
    } else {
    }
}

pub proof fn both_arms_are_observable_v2(fallback: int)
    requires
        0 < fallback < 4294967296,
    ensures
        mir_observation_v2(
            run_mir_machine_v2(
                initial_mir_state_v2(MirEnvironmentV2 { argument: 0, fallback }),
                6,
            ).unwrap(),
        ) != mir_observation_v2(
            run_mir_machine_v2(
                initial_mir_state_v2(MirEnvironmentV2 { argument: 1, fallback }),
                6,
            ).unwrap(),
        ),
{
    reveal_with_fuel(run_mir_machine_v2, 8);
}

}

fn main() {}
