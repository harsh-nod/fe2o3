include!("../mir_kir_cfg_refinement_v2.rs");

verus! {
proof fn reversed_branch_machine_is_not_refinement()
    ensures
        mir_observation_v2(
            run_mir_machine_v2(
                initial_mir_state_v2(MirEnvironmentV2 { argument: 1, fallback: 17 }),
                6,
            ).unwrap(),
        ) == kir_observation_v2(KirMachineStateV2 {
            pc: KirPcV2::Done,
            root_parameter: 1,
            fallback_constant: 17,
            helper_parameter: 1,
            edge_value: 1,
            join_parameter: 1,
            helper_return_operand: 1,
            call_result: 1,
            selected_arm: SelectedArmV2::Zero,
        }),
{
    reveal_with_fuel(run_mir_machine_v2, 8);
}
}
