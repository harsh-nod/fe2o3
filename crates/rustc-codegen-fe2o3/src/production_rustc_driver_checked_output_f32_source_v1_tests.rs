use super::*;

#[test]
#[ignore = "requires pinned nightly rust-src, AMD dependencies, F32 admission and ordinary-source compilation"]
fn ordinary_rust_f32_arithmetic_reaches_actual_o_native_and_simulator() {
    // Normal MIR remains covered separately from the retained-helper cases.
    // Retention is required by the observed O call/body census, not assumed
    // from an attribute or inferred from an equivalent no-call output.
    ordinary_rust_checked_output_cases(&[
        OrdinarySourceCase::F32Negate,
        OrdinarySourceCase::F32Divide,
        OrdinarySourceCase::RetainedF32Negate,
        OrdinarySourceCase::RetainedF32Divide,
    ]);
}
