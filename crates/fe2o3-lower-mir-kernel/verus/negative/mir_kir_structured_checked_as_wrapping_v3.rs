include!("../mir_kir_structured_cfg_v3.rs");

verus! {
pub open spec fn hostile_program_v3() -> ProgramV3 {
    ProgramV3 { modulus: 256, output_modulus: 256, mode: ExpressionModeV3::CheckedAdd,
        fallback: 7, increment: 3, threshold: 40, trip_count: 3 }
}
proof fn hostile_mutation_v3()
    ensures
        mir_observation_v3(run_mir_v3(initial_mir_v3(hostile_program_v3(), 250, 10), 20).unwrap())
            == Some(seq![10, 2, 3, 1, 20, 2, 16]),
{
    reveal_with_fuel(run_mir_v3, 25);
}
}
