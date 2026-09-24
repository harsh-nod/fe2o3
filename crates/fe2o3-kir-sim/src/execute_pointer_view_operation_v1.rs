//! Unchanged pointer/view semantics outlined from the ordinary dispatcher.
//! Debug-build callers reserve only the selected operation family's frame.
use super::*;

#[inline(never)]
pub(super) fn execute(
    engine: &mut Engine<'_, impl SimulationEventSinkV1>,
    values: &HashMap<ValueId, RuntimeValue>,
    operation: &Operation,
    site: CompactSite,
) -> Result<SmallResults<RuntimeValue>, SimulationExecutionErrorV1> {
    let one = |value| Ok(SmallResults::One(value));
    match &operation.kind {
        OperationKind::SliceLength { slice } => {
            let RuntimeValue::Slice(slice) = runtime_value(engine, values, *slice, &site)? else {
                return Err(engine.at(
                    site,
                    SimulationExecutionErrorKindV1::RuntimeType {
                        value: Some(*slice),
                        expected: "slice",
                    },
                ));
            };
            one(RuntimeValue::Scalar(
                ScalarBitsV1::index(slice.elements as u64, engine.target).map_err(|_| {
                    engine.at(site, SimulationExecutionErrorKindV1::IntegerOutOfRange)
                })?,
            ))
        }
        OperationKind::SliceData { slice } => {
            let RuntimeValue::Slice(slice) = runtime_value(engine, values, *slice, &site)? else {
                return Err(engine.at(
                    site,
                    SimulationExecutionErrorKindV1::RuntimeType {
                        value: Some(*slice),
                        expected: "slice",
                    },
                ));
            };
            let upper_bound = slice
                .byte_offset
                .checked_add(slice.byte_len)
                .ok_or_else(|| {
                    engine.at(
                        site,
                        SimulationExecutionErrorKindV1::InternalInvariant(
                            "preflighted slice view bounds",
                        ),
                    )
                })?;
            one(RuntimeValue::Pointer(PointerValue {
                allocation: slice.allocation,
                byte_offset: slice.byte_offset,
                element: slice.element,
                address_space: slice.address_space,
                access: slice.access,
                lower_bound: slice.byte_offset,
                upper_bound,
                abi_argument_ordinal: slice.abi_argument_ordinal,
            }))
        }
        OperationKind::GetElementPointer { base, offset } => {
            let RuntimeValue::Pointer(pointer) = runtime_value(engine, values, *base, &site)?
            else {
                return Err(engine.at(
                    site,
                    SimulationExecutionErrorKindV1::RuntimeType {
                        value: Some(*base),
                        expected: "pointer",
                    },
                ));
            };
            let offset = scalar_nonnegative_usize(
                scalar_value(engine, values, *offset, &site)?,
                engine.target,
            )
            .map_err(|kind| engine.at(site, kind))?;
            let element_bytes = engine.target.scalar_bytes(pointer.element).ok_or_else(|| {
                engine.at(
                    site,
                    SimulationExecutionErrorKindV1::InternalInvariant(
                        "preflighted pointer element",
                    ),
                )
            })?;
            let byte_delta = offset.checked_mul(element_bytes).ok_or_else(|| {
                engine.at(site, SimulationExecutionErrorKindV1::PointerOffsetOverflow)
            })?;
            let byte_offset = pointer.byte_offset.checked_add(byte_delta).ok_or_else(|| {
                engine.at(site, SimulationExecutionErrorKindV1::PointerOffsetOverflow)
            })?;
            one(RuntimeValue::Pointer(PointerValue {
                byte_offset,
                ..pointer.clone()
            }))
        }
        _ => Err(engine.at(
            site,
            SimulationExecutionErrorKindV1::InternalInvariant(
                "outlined pointer/view operation dispatch",
            ),
        )),
    }
}
