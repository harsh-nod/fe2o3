//! Physical VCC uses the existing cooperative-wave rendezvous and real CFG sites.
use super::*;

pub(super) fn resolve<'a>(
    engine: &mut Engine<'a, impl SimulationEventSinkV1>,
    machines: &mut [InvocationMachine<'a>],
    arrival: CollectiveArrival<'a>,
    start: u64,
) -> Result<bool, SimulationExecutionErrorV1> {
    if !physical_entry_v20::is_collective(arrival.operation)
        && !physical_global_copy_v21::is_collective(arrival.operation)
    {
        return Ok(false);
    }
    if arrival.width != WaveWidth::Wave64 {
        return Err(engine.at(
            arrival.site,
            SimulationExecutionErrorKindV1::InternalInvariant("physical VCC Wave64"),
        ));
    }
    // Caller already joined all 64 resident members to this precise operation/site.
    // This roster is residency, not EXEC. The actual comparison checks full EXEC.
    let mut vcc = 0u64;
    for lane in 0..64 {
        let index = wave_member_index(machines, start, lane).ok_or_else(|| {
            engine.at(
                arrival.site,
                SimulationExecutionErrorKindV1::InternalInvariant("physical VCC member"),
            )
        })?;
        let Some(CollectiveInput::PhysicalEntryPredicate(predicate)) =
            machines[index].collective_input()
        else {
            return Err(engine.at(
                arrival.site,
                SimulationExecutionErrorKindV1::InternalInvariant("physical VCC predicate"),
            ));
        };
        if *predicate {
            vcc |= 1u64 << lane;
        }
    }
    let mask =
        ScalarBitsV1::new(ScalarType::U64, u128::from(vcc), engine.target).map_err(|_| {
            engine.at(
                arrival.site,
                SimulationExecutionErrorKindV1::InternalInvariant("physical VCC mask"),
            )
        })?;
    for lane in 0..64 {
        let index = wave_member_index(machines, start, lane).ok_or_else(|| {
            engine.at(
                arrival.site,
                SimulationExecutionErrorKindV1::InternalInvariant("physical VCC completion member"),
            )
        })?;
        engine.select_debug_invocation(Some(machines[index].invocation));
        machines[index].complete_collective(engine, &[RuntimeValue::Scalar(mask)])?;
    }
    Ok(true)
}
