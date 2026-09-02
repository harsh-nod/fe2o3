include!("../mir_kir_structured_cfg_v3.rs");

verus! {
pub open spec fn hostile_program_v3() -> ProgramV3 {
    ProgramV3 { modulus: 256, output_modulus: 65536, mode: ExpressionModeV3::WrappingAdd,
        fallback: 7, increment: 3, threshold: 40, trip_count: 3 }
}
proof fn hostile_mutation_v3()
    ensures
        mir_observation_v3(run_mir_v3(initial_mir_v3(hostile_program_v3(), 1, 1), 20).unwrap())
            == kir_observation_v3(KirStateV3 { pc: KirPcV3::Done,
                first_zero: true, second_less: true, loop_iteration_parameter: 3,
                call_result: 16, ..initial_kir_v3(hostile_program_v3(), 1, 1) }),
{
    reveal_with_fuel(run_mir_v3, 25);
}
}
