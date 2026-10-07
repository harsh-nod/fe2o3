//! Source-step laws for the supported copied-local u32 addition case.
//! Selection uses the same retained event that emits the source dispatcher.
use super::*;

impl Checked {
    pub(in super::super) fn emit_local_add_proofs_v288(
        self,
        root: usize,
        instance: usize,
        block: usize,
        statement: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<bool> {
        let headers = 2 * size_of::<Self>()
            + 2 * size_of::<Value>()
            + size_of::<CheckedDestination>()
            + 12 * size_of::<usize>()
            + size_of::<u32>()
            + 2 * size_of::<Result<bool>>()
            + 6 * size_of::<&()>();
        out.budget.reserve_storage(headers)?;
        let result = (|| {
            out.budget.charge_work(4)?;
            let (
                CheckedDestination::Local(destination),
                Value::Local {
                    local: left,
                    moved: false,
                },
                Value::Local {
                    local: right,
                    moved: false,
                },
                ScalarV30::Integer {
                    signed: false,
                    width: 32,
                },
                SemanticCheckedBinaryOpV1::Add,
            ) = (
                self.destination,
                self.left,
                self.right,
                self.scalar,
                self.operation,
            )
            else {
                return Ok(false);
            };
            let ty = self.source_type.index();
            write!(out, "proof fn checked_add_actual_schema_{root}_{instance}_{block}_{statement}_v260()\n ensures invocation_source_aggregate_leaf_count_v42({ty}) == 2,\n invocation_source_aggregate_leaf_path_v42({ty}, 0) == seq![0int],\n invocation_source_aggregate_leaf_path_v42({ty}, 1) == seq![1int],\n invocation_source_aggregate_leaf_bits_v42({ty}, 0) == 32,\n invocation_source_aggregate_leaf_bits_v42({ty}, 1) == 1,\n invocation_source_byte_event_{root}_{instance}_v36({block}, {statement}) == Some(")
            .map_err(|_| out.error())?;
            self.emit(out)?;
            writeln!(out, "),\n{{ }}").map_err(|_| out.error())?;
            write!(out, r#"proof fn checked_add_actual_step_{root}_{instance}_{block}_{statement}_v260(
 source: InvocationSourceByteStateV36, left: int, right: int, little_endian: bool,
)
 requires source.machine.valid && invocation_source_byte_state_well_formed_v36(source),
 0 <= {destination} < source.machine.values.len(), !source.objects.contains_key({destination}),
 0 <= {left} < source.machine.values.len(), 0 <= {right} < source.machine.values.len(),
 source.machine.values[{left}] == MemoryValueV30::Scalar(left),
 source.machine.values[{right}] == MemoryValueV30::Scalar(right),
 0 <= left < 4294967296, 0 <= right < 4294967296,
 ensures ({{ let after = invocation_source_byte_step_v36(source,
 invocation_source_byte_event_{root}_{instance}_v36({block}, {statement}).unwrap(), {root}, {instance}, little_endian);
 after.machine.valid && after.machine.memory == source.machine.memory
 && after.machine.frames == source.machine.frames && after.machine.generations == source.machine.generations
 && after.logical.aggregates.contains_key({destination})
 && after.logical.aggregates[{destination}].leaves[seq![0int]] == MemoryValueV30::Scalar((left + right) % 4294967296)
 && after.logical.aggregates[{destination}].leaves[seq![1int]] == MemoryValueV30::Scalar(if left + right >= 4294967296 {{ 1int }} else {{ 0int }}) }}),
{{
 hide(invocation_source_byte_state_well_formed_v36);
 hide(invocation_source_aggregate_leaf_count_v42);
 hide(invocation_source_aggregate_leaf_path_v42);
 hide(invocation_source_aggregate_leaf_bits_v42);
 hide(invocation_source_byte_step_v36);
 hide(invocation_source_byte_event_{root}_{instance}_v36);
 assert(invocation_source_aggregate_leaf_count_v42({ty}) == 2
 && invocation_source_aggregate_leaf_path_v42({ty}, 0) == seq![0int]
 && invocation_source_aggregate_leaf_path_v42({ty}, 1) == seq![1int]
 && invocation_source_aggregate_leaf_bits_v42({ty}, 0) == 32
 && invocation_source_aggregate_leaf_bits_v42({ty}, 1) == 1
 && invocation_source_byte_event_{root}_{instance}_v36({block}, {statement}) == Some("#)
            .map_err(|_| out.error())?;
            self.emit(out)?;
            write!(
                out,
                r#")) by {{
 checked_add_actual_schema_{root}_{instance}_{block}_{statement}_v260();
 }}
 let checked_event = "#
            )
            .map_err(|_| out.error())?;
            self.emit(out)?;
            writeln!(out, r#";
 assert(invocation_source_byte_event_{root}_{instance}_v36({block}, {statement}).unwrap() == checked_event);
 assert(invocation_source_byte_step_v36(source,
 invocation_source_byte_event_{root}_{instance}_v36({block}, {statement}).unwrap(), {root}, {instance}, little_endian)
 == invocation_source_byte_step_v36(source, checked_event, {root}, {instance}, little_endian));
 invocation_source_checked_add_local_step_v266(source, {destination}, {ty},
 {left}, {right}, left, right, {root}, {instance}, little_endian);
}}
"#).map_err(|_| out.error())?;
            Ok(true)
        })();
        if result.is_ok() {
            out.budget.release_storage(headers)?;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_local_add_law_selection_keeps_operand_and_type_restrictions() {
        use crate::mixed_optimizer_refinement_v26::SOURCE_LIMIT;
        use fe2o3_kernel_ir::{
            CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
            CanonicalKernelIrWorkBudgetV1 as Work,
        };
        let base = Checked {
            destination: CheckedDestination::Local(3),
            source_type: TypeId::from_index(9),
            left: Value::Local {
                local: 5,
                moved: false,
            },
            right: Value::Local {
                local: 7,
                moved: false,
            },
            scalar: ScalarV30::Integer {
                signed: false,
                width: 32,
            },
            operation: SemanticCheckedBinaryOpV1::Add,
        };
        let access = Access {
            address: Address::Object {
                local: 3,
                offset: 0,
            },
            ty: TypeId::from_index(9),
            bytes: 8,
            alignment: 4,
        };
        for (event, selected) in [
            (base, true),
            (
                Checked {
                    scalar: ScalarV30::Integer {
                        signed: true,
                        width: 32,
                    },
                    ..base
                },
                false,
            ),
            (
                Checked {
                    scalar: ScalarV30::Integer {
                        signed: false,
                        width: 64,
                    },
                    ..base
                },
                false,
            ),
            (
                Checked {
                    operation: SemanticCheckedBinaryOpV1::Subtract,
                    ..base
                },
                false,
            ),
            (
                Checked {
                    operation: SemanticCheckedBinaryOpV1::Multiply,
                    ..base
                },
                false,
            ),
            (
                Checked {
                    left: Value::Local {
                        local: 5,
                        moved: true,
                    },
                    ..base
                },
                false,
            ),
            (
                Checked {
                    right: Value::Local {
                        local: 7,
                        moved: true,
                    },
                    ..base
                },
                false,
            ),
            (
                Checked {
                    left: Value::Constant(5),
                    ..base
                },
                false,
            ),
            (
                Checked {
                    right: Value::Constant(7),
                    ..base
                },
                false,
            ),
            (
                Checked {
                    left: Value::Read {
                        access,
                        moved: false,
                    },
                    ..base
                },
                false,
            ),
            (
                Checked {
                    right: Value::Component {
                        local: 7,
                        source_type: TypeId::from_index(9),
                        ordinal: 0,
                        moved: false,
                    },
                    ..base
                },
                false,
            ),
            (
                Checked {
                    destination: CheckedDestination::Object {
                        access,
                        offsets: [0, 4],
                    },
                    ..base
                },
                false,
            ),
        ] {
            let mut work = Work::new(1_000_000);
            let mut budget = Budget::new(&mut work, SOURCE_LIMIT + 1_000_000);
            budget.reserve_storage(SOURCE_LIMIT).unwrap();
            let mut out = Writer::new(&mut budget).unwrap();
            assert_eq!(
                event
                    .emit_local_add_proofs_v288(2, 4, 6, 8, &mut out)
                    .unwrap(),
                selected
            );
            if selected {
                for name in [
                    "checked_add_actual_schema_2_4_6_8_v260",
                    "checked_add_actual_step_2_4_6_8_v260",
                ] {
                    assert_eq!(out.text.matches(&format!("proof fn {name}(")).count(), 1);
                }
                assert!(
                    out.text
                        .contains("invocation_source_checked_add_local_step_v266(source, 3, 9,")
                );
                assert!(out.text.contains("5, 7, left, right, 2, 4, little_endian)"));
            } else {
                assert!(out.text.is_empty());
            }
        }
    }
}
