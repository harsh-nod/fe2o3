//! Source-step laws for the supported copied-local u32 addition case.
//! Selection uses the same retained event that emits the source dispatcher.
use super::*;

fn proof_headers() -> usize {
    2 * size_of::<Checked>()
        + 2 * size_of::<Value>()
        + size_of::<CheckedDestination>()
        + 28 * size_of::<usize>()
        + size_of::<u32>()
        + size_of::<Option<usize>>()
        + 2 * size_of::<Result<bool>>()
        + size_of::<Result<()>>()
        + 22 * size_of::<&()>()
}

impl Checked {
    pub(in super::super) fn emit_local_add_proofs_v288(
        self,
        root: usize,
        instance: usize,
        block: usize,
        statement: usize,
        pc: Option<usize>,
        out: &mut Writer<'_, '_>,
    ) -> Result<bool> {
        let headers = proof_headers();
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
            writeln!(out, r#"proof fn checked_add_actual_step_{root}_{instance}_{block}_{statement}_v260(
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
 checked_add_actual_schema_{root}_{instance}_{block}_{statement}_v260();
 invocation_source_checked_add_local_step_v266(source, {destination}, {ty},
 {left}, {right}, left, right, {root}, {instance}, little_endian);
}}
"#).map_err(|_| out.error())?;
            if let Some(pc) = pc {
                write!(out, r#"proof fn checked_add_actual_micro_step_{root}_{instance}_{block}_{statement}_v293(
 c: InvocationSourceMicroStateV36, left: int, right: int, little_endian: bool,
)
 requires invocation_source_active_{root}_{instance}_v36(c.source),
 c.source.machine.pc == {pc}, c.next_statement == {statement},
 c.next_statement == c.observations.len(),
 c.source.machine.valid && invocation_source_byte_state_well_formed_v36(c.source),
 0 <= {destination} < c.source.machine.values.len(), !c.source.objects.contains_key({destination}),
 0 <= {left} < c.source.machine.values.len(), 0 <= {right} < c.source.machine.values.len(),
 c.source.machine.values[{left}] == MemoryValueV30::Scalar(left),
 c.source.machine.values[{right}] == MemoryValueV30::Scalar(right),
 0 <= left < 4294967296, 0 <= right < 4294967296,
 ensures ({{ let n = invocation_source_micro_step_{root}_{instance}_v36(c, little_endian);
 let l = c.observations.len() as int;
 n.source.machine.valid && invocation_source_active_{root}_{instance}_v36(n.source)
 && invocation_source_byte_state_well_formed_v36(n.source)
 && n.source.machine.pc == c.source.machine.pc
 && n.source.machine.values.len() == c.source.machine.values.len()
 && n.source.slots == c.source.slots && n.source.objects == c.source.objects
 && n.next_statement == c.next_statement + 1
 && n.observations.len() == l + 1 && n.observations.take(l) == c.observations
 && n.observations[l].root == {root} && n.observations[l].instance == {instance}
 && n.observations[l].block == {block} && n.observations[l].statement == {statement}
 && n.observations[l].before == c.source && n.observations[l].after == n.source
 && n.observations[l].event == invocation_source_byte_event_{root}_{instance}_v36({block}, {statement})
 && n.source.machine.memory == c.source.machine.memory
 && n.source.machine.frames == c.source.machine.frames
 && n.source.machine.generations == c.source.machine.generations
 && n.source.logical.aggregates.contains_key({destination})
 && n.source.logical.aggregates[{destination}].leaves[seq![0int]] == MemoryValueV30::Scalar((left + right) % 4294967296)
 && n.source.logical.aggregates[{destination}].leaves[seq![1int]] == MemoryValueV30::Scalar(if left + right >= 4294967296 {{ 1int }} else {{ 0int }}) }}),
{{
 checked_add_actual_schema_{root}_{instance}_{block}_{statement}_v260();
 checked_add_actual_step_{root}_{instance}_{block}_{statement}_v260(c.source, left, right, little_endian);
 invocation_source_checked_add_local_well_formed_v294(c.source, {destination}, {ty},
     {left}, {right}, left, right, {root}, {instance}, little_endian);
 let event = invocation_source_byte_event_{root}_{instance}_v36({block}, {statement});
 assert(event.is_some());
 let after = invocation_source_byte_step_v36(c.source, event.unwrap(), {root}, {instance}, little_endian);
 assert(invocation_source_active_{root}_{instance}_v36(after));
 reveal(invocation_source_micro_step_{root}_{instance}_v36);
 let n = invocation_source_micro_step_{root}_{instance}_v36(c, little_endian);
 assert(n == invocation_source_micro_record_v36(c, after, {root}, {instance}, {block}, {statement}, event));
 invocation_source_micro_step_history_{root}_{instance}_v293(c, little_endian);
 reveal(invocation_source_micro_record_v36);
 assert(n.observations.take(c.observations.len() as int) =~= c.observations);
}}
proof fn checked_add_actual_prefix_{root}_{instance}_{block}_{statement}_v293(
 c: InvocationSourceMicroStateV36, fuel: nat, left: int, right: int, little_endian: bool,
)
 requires ({{ let p = invocation_source_micro_run_{root}_{instance}_v36(c, fuel, little_endian);
 invocation_source_active_{root}_{instance}_v36(p.source)
 && p.source.machine.pc == {pc} && p.next_statement == {statement}
 && p.next_statement == p.observations.len()
 && p.source.machine.valid && invocation_source_byte_state_well_formed_v36(p.source)
 && 0 <= {destination} < p.source.machine.values.len() && !p.source.objects.contains_key({destination})
 && 0 <= {left} < p.source.machine.values.len() && 0 <= {right} < p.source.machine.values.len()
 && p.source.machine.values[{left}] == MemoryValueV30::Scalar(left)
 && p.source.machine.values[{right}] == MemoryValueV30::Scalar(right)
 && 0 <= left < 4294967296 && 0 <= right < 4294967296 }}),
 ensures ({{ let p = invocation_source_micro_run_{root}_{instance}_v36(c, fuel, little_endian);
 let out = invocation_source_micro_run_{root}_{instance}_v36(c, fuel + 1, little_endian);
 let l = c.observations.len() as int;
 out.source.machine.valid && invocation_source_active_{root}_{instance}_v36(out.source)
 && invocation_source_byte_state_well_formed_v36(out.source)
 && out.source.machine.pc == p.source.machine.pc
 && out.source.machine.values.len() == p.source.machine.values.len()
 && out.source.slots == p.source.slots && out.source.objects == p.source.objects
 && out.observations.len() == l + fuel + 1
 && out.observations.take(l) == c.observations
 && out.next_statement == c.next_statement + fuel + 1
 && out.observations[l + fuel].root == {root} && out.observations[l + fuel].instance == {instance}
 && out.observations[l + fuel].block == {block} && out.observations[l + fuel].statement == {statement}
 && out.observations[l + fuel].before == p.source && out.observations[l + fuel].after == out.source
 && out.source.machine.memory == p.source.machine.memory
 && out.source.machine.frames == p.source.machine.frames
 && out.source.machine.generations == p.source.machine.generations
 && out.source.logical.aggregates.contains_key({destination})
 && out.source.logical.aggregates[{destination}].leaves[seq![0int]] == MemoryValueV30::Scalar((left + right) % 4294967296)
 && out.source.logical.aggregates[{destination}].leaves[seq![1int]] == MemoryValueV30::Scalar(if left + right >= 4294967296 {{ 1int }} else {{ 0int }}) }}),
{{
 let p = invocation_source_micro_run_{root}_{instance}_v36(c, fuel, little_endian);
 checked_add_actual_micro_step_{root}_{instance}_{block}_{statement}_v293(p, left, right, little_endian);
 invocation_source_micro_run_composes_{root}_{instance}_v292(c, fuel, 1, little_endian);
 reveal_with_fuel(invocation_source_micro_run_{root}_{instance}_v36, 2);
 assert(invocation_source_micro_run_{root}_{instance}_v36(p, 1, little_endian)
     == invocation_source_micro_step_{root}_{instance}_v36(p, little_endian));
 invocation_source_micro_run_history_{root}_{instance}_v293(c, fuel, little_endian);
 invocation_source_micro_run_history_{root}_{instance}_v293(c, fuel + 1, little_endian);
}}
"#).map_err(|_| out.error())?;
            }
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
    fn checked_micro_proof_headers_include_site_and_fallible_emission_frames() {
        assert_eq!(
            proof_headers(),
            2 * size_of::<Checked>()
                + 2 * size_of::<Value>()
                + size_of::<CheckedDestination>()
                + 28 * size_of::<usize>()
                + size_of::<u32>()
                + size_of::<Option<usize>>()
                + 2 * size_of::<Result<bool>>()
                + size_of::<Result<()>>()
                + 22 * size_of::<&()>()
        );
    }

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
                    .emit_local_add_proofs_v288(2, 4, 6, 8, Some(106), &mut out)
                    .unwrap(),
                selected
            );
            if selected {
                for name in [
                    "checked_add_actual_schema_2_4_6_8_v260",
                    "checked_add_actual_step_2_4_6_8_v260",
                    "checked_add_actual_micro_step_2_4_6_8_v293",
                    "checked_add_actual_prefix_2_4_6_8_v293",
                ] {
                    assert_eq!(out.text.matches(&format!("proof fn {name}(")).count(), 1);
                }
                assert!(
                    out.text
                        .contains("invocation_source_checked_add_local_step_v266(source, 3, 9,")
                );
                assert!(out.text.contains("5, 7, left, right, 2, 4, little_endian)"));
                let step = out
                    .text
                    .split_once("proof fn checked_add_actual_step_2_4_6_8_v260(")
                    .unwrap()
                    .1
                    .split_once("proof fn checked_add_actual_micro_step_")
                    .unwrap()
                    .0;
                let (contract, body) = step.split_once("\n{\n").unwrap();
                assert_eq!(
                    contract,
                    "\n source: InvocationSourceByteStateV36, left: int, right: int, little_endian: bool,\n)\n requires source.machine.valid && invocation_source_byte_state_well_formed_v36(source),\n 0 <= 3 < source.machine.values.len(), !source.objects.contains_key(3),\n 0 <= 5 < source.machine.values.len(), 0 <= 7 < source.machine.values.len(),\n source.machine.values[5] == MemoryValueV30::Scalar(left),\n source.machine.values[7] == MemoryValueV30::Scalar(right),\n 0 <= left < 4294967296, 0 <= right < 4294967296,\n ensures ({ let after = invocation_source_byte_step_v36(source,\n invocation_source_byte_event_2_4_v36(6, 8).unwrap(), 2, 4, little_endian);\n after.machine.valid && after.machine.memory == source.machine.memory\n && after.machine.frames == source.machine.frames && after.machine.generations == source.machine.generations\n && after.logical.aggregates.contains_key(3)\n && after.logical.aggregates[3].leaves[seq![0int]] == MemoryValueV30::Scalar((left + right) % 4294967296)\n && after.logical.aggregates[3].leaves[seq![1int]] == MemoryValueV30::Scalar(if left + right >= 4294967296 { 1int } else { 0int }) }),"
                );
                assert_eq!(
                    body,
                    " hide(invocation_source_byte_state_well_formed_v36);\n hide(invocation_source_aggregate_leaf_count_v42);\n hide(invocation_source_aggregate_leaf_path_v42);\n hide(invocation_source_aggregate_leaf_bits_v42);\n hide(invocation_source_byte_step_v36);\n hide(invocation_source_byte_event_2_4_v36);\n checked_add_actual_schema_2_4_6_8_v260();\n invocation_source_checked_add_local_step_v266(source, 3, 9,\n 5, 7, left, right, 2, 4, little_endian);\n}\n\n"
                );
                assert!(
                    out.text
                        .contains("c.source.machine.pc == 106, c.next_statement == 8,")
                );
                assert!(out.text.contains("n.observations[l].block == 6"));
                assert!(!out.text.contains("n.observations[l].block == 106"));
                let micro = out
                    .text
                    .split("proof fn checked_add_actual_micro_step_")
                    .nth(1)
                    .unwrap();
                for forbidden in [
                    "assume(",
                    "admit(",
                    "external_body",
                    "target",
                    "_map",
                    "fuel >=",
                ] {
                    assert!(!micro.contains(forbidden), "{forbidden}");
                }
                for required in [
                    "invocation_source_micro_step_history_2_4_v293(c, little_endian)",
                    "checked_add_actual_micro_step_2_4_6_8_v293(p, left, right, little_endian)",
                    "invocation_source_micro_run_composes_2_4_v292(c, fuel, 1, little_endian)",
                    "invocation_source_micro_run_history_2_4_v293(c, fuel + 1, little_endian)",
                ] {
                    assert!(micro.contains(required));
                }
            } else {
                assert!(out.text.is_empty());
            }
        }
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, SOURCE_LIMIT + 1_000_000);
        budget.reserve_storage(SOURCE_LIMIT).unwrap();
        let mut out = Writer::new(&mut budget).unwrap();
        assert!(
            base.emit_local_add_proofs_v288(2, 4, 6, 8, None, &mut out)
                .unwrap()
        );
        assert!(out.text.contains("proof fn checked_add_actual_step_"));
        assert!(!out.text.contains("proof fn checked_add_actual_micro_step_"));
        assert!(!out.text.contains("proof fn checked_add_actual_prefix_"));
    }
}
