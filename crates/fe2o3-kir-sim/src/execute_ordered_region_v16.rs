//! One atomic CPU value result. No physical register file, EXEC simulation or
//! instruction microsteps are introduced by this observational operation.

use super::*;

#[inline(never)]
pub(super) fn execute(
    engine: &mut Engine<'_, impl SimulationEventSinkV1>,
    values: &HashMap<ValueId, RuntimeValue>,
    operation: &Operation,
    site: &CompactSite,
) -> Result<SmallResults<RuntimeValue>, SimulationExecutionErrorV1> {
    let invariant = |message| {
        engine.at(
            *site,
            SimulationExecutionErrorKindV1::InternalInvariant(message),
        )
    };
    let validated = fe2o3_kernel_ir::validate_gfx942_ordered_region_v1(operation, |value| {
        match values.get(&value) {
            Some(RuntimeValue::Scalar(scalar)) => Some(scalar.ty()),
            _ => None,
        }
    })
    .map_err(|_| invariant("ordered region contract changed after preflight"))?;
    let mut inputs = [0_u32; 3];
    for (index, value) in validated.inputs().iter().enumerate() {
        inputs[index] = u32::try_from(scalar_value(engine, values, *value, site)?.bits())
            .map_err(|_| invariant("ordered region U32 input bits"))?;
    }
    let bits = validated
        .profile()
        .evaluate_bits(&inputs)
        .map_err(|_| invariant("ordered region fixed input arity"))?;
    let scalar = ScalarBitsV1::new(ScalarType::U32, u128::from(bits), engine.target)
        .map_err(|_| invariant("ordered region U32 result bits"))?;
    Ok(SmallResults::One(RuntimeValue::Scalar(scalar)))
}
