include!("../mir_kir_cfg_refinement_v2.rs");

verus! {
proof fn substituted_callee_machine_is_not_refinement()
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
            edge_value: 9,
            join_parameter: 9,
            helper_return_operand: 9,
            call_result: 9,
            selected_arm: SelectedArmV2::Nonzero,
        }),
{
    reveal_with_fuel(run_mir_machine_v2, 8);
}
}
