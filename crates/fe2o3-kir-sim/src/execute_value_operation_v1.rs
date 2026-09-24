//! Unchanged value/scalar semantics outlined from the ordinary dispatcher.
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
        OperationKind::Constant(constant) => one(RuntimeValue::Scalar(
            constant_scalar(constant, engine.target).map_err(|kind| engine.at(site, kind))?,
        )),
        OperationKind::Intrinsic(intrinsic) => {
            let invocation = engine.invocation.ok_or_else(|| {
                engine.at(
                    site,
                    SimulationExecutionErrorKindV1::InternalInvariant("intrinsic invocation"),
                )
            })?;
            let value = intrinsic_value(intrinsic.kind, invocation);
            one(RuntimeValue::Scalar(
                ScalarBitsV1::index(value, engine.target).map_err(|_| {
                    engine.at(site, SimulationExecutionErrorKindV1::IntegerOutOfRange)
                })?,
            ))
        }
        OperationKind::Unary { op, operand } => {
            let value = scalar_value(engine, values, *operand, &site)?;
            one(RuntimeValue::Scalar(
                execute_unary(*op, value, engine.target).map_err(|kind| engine.at(site, kind))?,
            ))
        }
        OperationKind::Binary { op, lhs, rhs } => {
            let lhs = scalar_value(engine, values, *lhs, &site)?;
            let rhs = scalar_value(engine, values, *rhs, &site)?;
            let scalars = execute_binary(*op, lhs, rhs, engine.target)
                .map_err(|kind| engine.at(site, kind))?;
            Ok(match scalars {
                SmallResults::None => SmallResults::None,
                SmallResults::One(value) => SmallResults::One(RuntimeValue::Scalar(value)),
                SmallResults::Two(first, second) => {
                    SmallResults::Two(RuntimeValue::Scalar(first), RuntimeValue::Scalar(second))
                }
            })
        }
        OperationKind::Compare {
            predicate,
            lhs,
            rhs,
        } => {
            let lhs = scalar_value(engine, values, *lhs, &site)?;
            let rhs = scalar_value(engine, values, *rhs, &site)?;
            one(RuntimeValue::Scalar(ScalarBitsV1::boolean(
                execute_compare(*predicate, lhs, rhs, engine.target)
                    .map_err(|kind| engine.at(site, kind))?,
            )))
        }
        OperationKind::Cast { kind, value, to } => {
            if *kind == CastKind::RestrictPointerAccess {
                let RuntimeValue::Pointer(mut pointer) =
                    runtime_value(engine, values, *value, &site)?.clone()
                else {
                    return Err(engine.at(
                        site,
                        SimulationExecutionErrorKindV1::InternalInvariant(
                            "preflighted pointer access restriction",
                        ),
                    ));
                };
                pointer.access = AccessMode::ReadOnly;
                return one(RuntimeValue::Pointer(pointer));
            }
            let value = scalar_value(engine, values, *value, &site)?;
            let Type::Scalar(to) = to else {
                return Err(engine.at(
                    site,
                    SimulationExecutionErrorKindV1::InternalInvariant("preflighted scalar cast"),
                ));
            };
            one(RuntimeValue::Scalar(
                execute_cast(*kind, value, *to, engine.target)
                    .map_err(|kind| engine.at(site, kind))?,
            ))
        }
        OperationKind::Select {
            condition,
            true_value,
            false_value,
        } => {
            let condition = scalar_value(engine, values, *condition, &site)?
                .as_bool()
                .ok_or_else(|| {
                    engine.at(
                        site,
                        SimulationExecutionErrorKindV1::RuntimeType {
                            value: Some(*condition),
                            expected: "boolean select condition",
                        },
                    )
                })?;
            one(runtime_value(
                engine,
                values,
                if condition { *true_value } else { *false_value },
                &site,
            )?
            .clone())
        }
        _ => Err(engine.at(
            site,
            SimulationExecutionErrorKindV1::InternalInvariant(
                "outlined value/scalar operation dispatch",
            ),
        )),
    }
}
