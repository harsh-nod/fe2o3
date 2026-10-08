//! One numerical completion inside the existing cooperative interpreter.
use super::*;
use crate::MatrixInputRoleV1;
use crate::matrix_fp4_fp8_exact_v1 as domain;

pub(super) const ARRIVAL_WORK: usize = 40;
// 64*72 screening (32 A + 4 A padding + 32 B + 4 C), 256*128*10
// MAC units (separate A/B maps, unpack/codec pairs, product/scale/add),
// 256*4 result units, 64*5 completion, and 320 scratch initialization.
// Existing wave rendezvous plus membership scans add 66*N. All work is prepaid.
const RESOLUTION_FIXED_WORK: usize = 333_952;

#[derive(Clone, Copy)]
pub(super) struct Input {
    lhs: [u32; 8],
    rhs: [u32; 8],
    accumulator: [u32; 4],
}

struct Scratch {
    members: [usize; 64],
    results: [[u32; 4]; 64],
}

/// Logical payload, not an RSS/whole-host-stack claim. Reserve before execution.
pub(super) fn resident_bytes() -> Option<usize> {
    // Allow construction/move coexistence for the fixed scratch and result array.
    size_of::<Scratch>()
        .checked_mul(2)?
        .checked_add(size_of::<[RuntimeValue; 4]>().checked_mul(2)?)?
        .checked_add(size_of::<[i64; 16]>())?
        .checked_add(size_of::<Input>().checked_mul(2)?)?
        .checked_add(size_of::<[u32; 3]>())
}

pub(super) fn resolution_work(participants: usize) -> Option<usize> {
    participants
        .checked_mul(66)?
        .checked_add(RESOLUTION_FIXED_WORK)
}

pub(super) fn is_operation(operation: &OperationKind) -> bool {
    matches!(operation, OperationKind::Matrix(matrix) if domain::supported(matrix))
}

pub(super) fn prepare(
    engine: &mut Engine<'_, impl SimulationEventSinkV1>,
    frame: &RuntimeFrame<'_>,
    matrix: &MatrixOperation,
    site: CompactSite,
) -> Result<Input, SimulationExecutionErrorV1> {
    if !domain::supported(matrix) {
        return Err(engine.at(
            site,
            SimulationExecutionErrorKindV1::InternalInvariant("unadmitted matrix exact profile"),
        ));
    }
    let MatrixOperationKind::ScaledMultiplyAccumulate {
        lhs,
        rhs,
        accumulator,
        ..
    } = &matrix.kind
    else {
        return Err(engine.at(
            site,
            SimulationExecutionErrorKindV1::InternalInvariant("matrix exact kind"),
        ));
    };
    engine.charge_steps(&site, ARRIVAL_WORK)?;
    let mut input = Input {
        lhs: [0; 8],
        rhs: [0; 8],
        accumulator: [0; 4],
    };
    for (ids, output) in [(lhs, &mut input.lhs), (rhs, &mut input.rhs)] {
        for (id, slot) in ids.iter().zip(output) {
            let scalar = scalar_value(engine, &frame.values, *id, &site)?;
            if scalar.ty() != ScalarType::U32 {
                return Err(engine.at(
                    site,
                    SimulationExecutionErrorKindV1::RuntimeType {
                        value: Some(*id),
                        expected: "packed U32 mixed FP4/FP8 matrix input",
                    },
                ));
            }
            *slot = scalar.bits() as u32;
        }
    }
    for (id, slot) in accumulator.iter().zip(&mut input.accumulator) {
        let scalar = scalar_value(engine, &frame.values, *id, &site)?;
        if scalar.ty() != ScalarType::F32 {
            return Err(engine.at(
                site,
                SimulationExecutionErrorKindV1::RuntimeType {
                    value: Some(*id),
                    expected: "F32 matrix accumulator",
                },
            ));
        }
        *slot = scalar.bits() as u32;
    }
    Ok(input)
}

fn input<'m>(machine: &'m InvocationMachine<'_>) -> Option<&'m Input> {
    match machine.collective_input()? {
        CollectiveInput::MatrixFp4Fp8Exact(input) => Some(input),
        _ => None,
    }
}

fn f32_value(bits: u32, target: SimulationTargetV1) -> ScalarBitsV1 {
    // A u32 always fits the fixed F32 scalar width, independent of index width.
    ScalarBitsV1::new(ScalarType::F32, u128::from(bits), target)
        .expect("u32 is an exact F32 bit container")
}

/// Called only after the existing full-wave/site rendezvous and work precharge.
/// Every operand is validated and every output calculated before any binding.
pub(super) fn resolve<'a>(
    engine: &mut Engine<'a, impl SimulationEventSinkV1>,
    machines: &mut [InvocationMachine<'a>],
    arrival: CollectiveArrival<'a>,
    start: u64,
) -> Result<(), SimulationExecutionErrorV1> {
    let mut scratch = Scratch {
        members: [usize::MAX; 64],
        results: [[0; 4]; 64],
    };
    for (index, machine) in machines.iter().enumerate() {
        let local = local_linear(machine.invocation);
        if let Some(lane) = local.checked_sub(start).filter(|lane| *lane < 64) {
            let slot = &mut scratch.members[lane as usize];
            if *slot != usize::MAX {
                return Err(engine.at(
                    arrival.site,
                    SimulationExecutionErrorKindV1::InternalInvariant(
                        "matrix duplicate wave member",
                    ),
                ));
            }
            *slot = index;
        }
    }
    // Do not short-circuit the prepaid 4,608-component scan on a domain failure.
    let mut rejected = None;
    for (lane, index) in scratch.members.iter().copied().enumerate() {
        let machine = machines.get(index).ok_or_else(|| {
            engine.at(
                arrival.site,
                SimulationExecutionErrorKindV1::InternalInvariant("matrix missing wave member"),
            )
        })?;
        let values = input(machine).ok_or_else(|| {
            engine.at(
                arrival.site,
                SimulationExecutionErrorKindV1::InternalInvariant("matrix missing typed input"),
            )
        })?;
        for component in 0..32 {
            let nibble = (values.lhs[component / 8] >> (4 * (component % 8))) & 15;
            if domain::half_units(nibble).is_none() && rejected.is_none() {
                rejected = Some((lane, MatrixInputRoleV1::A, component));
            }
        }
        for word in 4..8 {
            if values.lhs[word] != 0 && rejected.is_none() {
                // Match the pure FP4 padding-component convention.
                rejected = Some((lane, MatrixInputRoleV1::A, 28 + word));
            }
        }
        for component in 0..32 {
            let byte = (values.rhs[component / 4] >> (8 * (component % 4))) & 255;
            if domain::quarter_units(byte).is_none() && rejected.is_none() {
                rejected = Some((lane, MatrixInputRoleV1::B, component));
            }
        }
        for (component, bits) in values.accumulator.iter().copied().enumerate() {
            if domain::accumulator_sixteenths(bits).is_none() && rejected.is_none() {
                rejected = Some((lane, MatrixInputRoleV1::Accumulator, component));
            }
        }
    }
    if let Some((lane, role, component)) = rejected {
        engine.select_debug_invocation(Some(machines[scratch.members[lane]].invocation));
        return Err(engine.at(
            arrival.site,
            SimulationExecutionErrorKindV1::UnsupportedMatrixInputDomain {
                role,
                lane: lane as u8,
                component: component as u8,
            },
        ));
    }
    for (lane, output) in scratch.results.iter_mut().enumerate() {
        let column = lane % 16;
        let accumulator =
            input(&machines[scratch.members[lane]]).expect("all typed wave inputs checked");
        for (component, result) in output.iter_mut().enumerate() {
            let row = 4 * (lane / 16) + component;
            let mut value = domain::accumulator_sixteenths(accumulator.accumulator[component])
                .expect("all accumulator domains checked");
            for k in 0..128 {
                // A is contiguous FP4 K32; B is split-K OCP FP8.
                let a_component = k % 32;
                let b_group = (k % 64) / 16;
                let b_component = (k % 16) + 16 * (k / 64);
                let a = input(&machines[scratch.members[row + 16 * (k / 32)]])
                    .expect("all typed A inputs checked")
                    .lhs[a_component / 8];
                let b = input(&machines[scratch.members[column + 16 * b_group]])
                    .expect("all typed B inputs checked")
                    .rhs[b_component / 4];
                let a = domain::half_units((a >> (4 * (a_component % 8))) & 15)
                    .expect("all FP4 domains checked");
                let b = domain::quarter_units((b >> (8 * (b_component % 4))) & 255)
                    .expect("all FP8 domains checked");
                value += 2 * a * b;
            }
            *result = domain::sixteenths_to_bits(value);
        }
    }
    for (lane, index) in scratch.members.iter().copied().enumerate() {
        let results =
            scratch.results[lane].map(|bits| RuntimeValue::Scalar(f32_value(bits, engine.target)));
        engine.select_debug_invocation(Some(machines[index].invocation));
        machines[index].complete_collective(engine, &results)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_work_formula_is_exact_and_overflow_checked() {
        assert_eq!(resolution_work(64), Some(338_176));
        assert_eq!(resolution_work(1024), Some(401_536));
        assert_eq!(resolution_work(64).unwrap() + 64 * ARRIVAL_WORK, 340_736);
        assert_eq!(resolution_work(usize::MAX), None);
    }

    #[test]
    fn compact_pending_matrix_state_preserves_shared_value_layouts() {
        assert_eq!(size_of::<Input>(), 80);
        assert_eq!(
            size_of::<Scratch>(),
            64 * size_of::<usize>() + 64 * 4 * size_of::<u32>()
        );
        assert!(size_of::<RuntimeValue>() <= 160);
        assert_eq!(size_of::<SimulationDebugValueV1>(), 176);
        assert_eq!(size_of::<SimulationDebugBindingV1>(), 192);
        // Legacy alternatives remain the largest inline payloads. No boxed wave.
        let legacy_ceiling = size_of::<(u8, PointerValue, [ScalarBitsV1; 4])>().max(size_of::<(
            u8,
            PointerValue,
            SliceValue,
            [u64; 6],
        )>());
        assert!(size_of::<CollectiveInput>() <= legacy_ceiling);
        assert!(
            resident_bytes().unwrap()
                >= 2 * size_of::<Scratch>() + 2 * size_of::<[RuntimeValue; 4]>()
        );
    }
}
