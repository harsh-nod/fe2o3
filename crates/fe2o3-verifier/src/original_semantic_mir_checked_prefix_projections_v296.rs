//! Demand-selected source projections, not a target or complete frame contract.
use super::super::super::component_demands::ComponentDomainV283;
use super::super::super::source_frame_plan::{
    FramePlan,
    prefix::{Prefix, ScalarDemand},
};
use super::*;

fn headers() -> usize {
    2 * size_of::<Checked>()
        + 3 * size_of::<Option<(usize, usize, usize)>>()
        + 3 * size_of::<Result<()>>()
        + 3 * size_of::<Range<usize>>()
        + 32 * size_of::<usize>()
        + 24 * size_of::<&()>()
        + 2 * size_of::<std::slice::Iter<'_, super::super::super::source_frame_plan::prefix::Local>>(
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_prefix_projection_headers_cover_query_borrows_and_emission() {
        assert_eq!(
            headers(),
            2 * size_of::<Checked>()
                + 3 * size_of::<Option<(usize, usize, usize)>>()
                + 3 * size_of::<Result<()>>()
                + 3 * size_of::<Range<usize>>()
                + 32 * size_of::<usize>()
                + 24 * size_of::<&()>()
                + 2 * size_of::<
                    std::slice::Iter<
                        '_,
                        super::super::super::super::source_frame_plan::prefix::Local,
                    >,
                >()
        );
    }

    #[test]
    fn checked_prefix_synthetic_numeric_aliases_keep_prestate_operand_reads() {
        // Numeric aliases test the generic law interface, not admission of an
        // ill-typed scalar/tuple alias into an actual source function.
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
        for (left, right) in [(3, 7), (5, 3), (3, 3)] {
            let alias = Checked {
                left: Value::Local {
                    local: left,
                    moved: false,
                },
                right: Value::Local {
                    local: right,
                    moved: false,
                },
                ..base
            };
            assert_eq!(alias.prefix_site_v296(), Some((3, left, right)));
        }
        assert_eq!(
            Checked {
                left: Value::Local {
                    local: 3,
                    moved: true
                },
                ..base
            }
            .prefix_site_v296(),
            None
        );
        let law = include_str!("original_semantic_mir_checked_projections_v296.vrs");
        let theorem = law
            .split_once("proof fn invocation_source_checked_add_local_projections_v296(")
            .unwrap()
            .1;
        let (contract, body) = theorem.split_once("\n{\n").unwrap();
        let requires = contract.split_once("    ensures").unwrap().0;
        assert!(
            requires.contains("source.machine.values[left_local] == MemoryValueV30::Scalar(left)")
        );
        assert!(
            requires
                .contains("source.machine.values[right_local] == MemoryValueV30::Scalar(right)")
        );
        assert!(!requires.contains("destination != left_local"));
        assert!(!requires.contains("destination != right_local"));
        assert!(body.contains("invocation_source_local_evaluates_v265(source, left_local"));
        assert!(body.contains("invocation_source_local_evaluates_v265(source, right_local"));
        assert!(!theorem.contains("assume("));
        assert!(!theorem.contains("admit("));
    }
}

impl Checked {
    pub(in super::super) fn prefix_site_v296(self) -> Option<(usize, usize, usize)> {
        match (
            self.destination,
            self.left,
            self.right,
            self.scalar,
            self.operation,
        ) {
            (
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
            ) => Some((destination, left, right)),
            _ => None,
        }
    }

    pub(in super::super) fn emit_prefix_v296(
        self,
        frames: &FramePlan<'_, '_, '_, '_>,
        before: &Prefix<'_, '_, '_, '_, '_>,
        after: &Prefix<'_, '_, '_, '_, '_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        before.check(frames, out)?;
        after.check(frames, out)?;
        out.budget.reserve_storage(headers())?;
        out.budget.charge_work(10)?;
        let (destination, left, right) = self.prefix_site_v296().ok_or_else(mismatch)?;
        let frame = frames.frames.get(before.frame).ok_or_else(mismatch)?;
        if before.frame != after.frame
            || before.block != after.block
            || before.pc != after.pc
            || before.statement.checked_add(1) != Some(after.statement)
            || before.locals.len() != after.locals.len()
            || !frame.locals.contains(&destination)
            || !frame.locals.contains(&left)
            || !frame.locals.contains(&right)
        {
            return Err(mismatch());
        }
        let (root, instance, block, statement, pc, ty) = (
            frame.root,
            frame.instance,
            before.block,
            before.statement,
            before.pc,
            self.source_type.index(),
        );
        let destination_local = destination - frame.locals.start;
        let destination_row = after.locals.get(destination_local).ok_or_else(mismatch)?;
        if destination_row.domain != ComponentDomainV283::ScalarV42
            || destination_row.components.len() != 2
            || destination_row.ty != self.source_type
        {
            return Err(mismatch());
        }
        for row in &after.locals {
            out.budget.charge_work(2)?;
            let old = before.locals.get(row.local).ok_or_else(mismatch)?;
            if row.local != destination_local
                && matches!(row.scalar, ScalarDemand::Needed(_))
                && old.current != row.current
            {
                return Err(mismatch());
            }
        }
        write!(out, "// Partial promoted/component demand projection; no full-frame or currentness assertion.\nspec fn checked_prefix_demands_{root}_{instance}_{block}_{statement}_v296(before: InvocationSourceByteStateV36, after: InvocationSourceByteStateV36, left: int, right: int) -> bool {{ true")
            .map_err(|_| out.error())?;
        for row in &after.locals {
            out.budget.charge_work(3)?;
            let local = frame
                .locals
                .start
                .checked_add(row.local)
                .ok_or(Resource::Arithmetic)?;
            if local != destination && matches!(row.scalar, ScalarDemand::Needed(_)) {
                write!(
                    out,
                    "\n && after.machine.values[{local}] == before.machine.values[{local}]"
                )
                .map_err(|_| out.error())?;
            }
            // Component domains are independent of scalar promotion/availability.
            for leaf in 0..row.components.len() {
                out.budget.charge_work(2)?;
                if !after.component_required(row.local, leaf, out)? {
                    continue;
                }
                if local == destination {
                    let value = if leaf == 0 {
                        "(left + right) % 4294967296"
                    } else {
                        "if left + right >= 4294967296 { 1int } else { 0int }"
                    };
                    write!(out, "\n && after.logical.aggregates.contains_key({destination})\n && after.logical.aggregates[{destination}].leaves[seq![{leaf}int]] == MemoryValueV30::Scalar({value})")
                        .map_err(|_| out.error())?;
                } else {
                    // The selected projection preserves absence; an unchanged
                    // stored reference does not imply it remains current.
                    let domain = match row.domain {
                        ComponentDomainV283::ScalarV42 => "aggregate",
                        ComponentDomainV283::ProductV282 => "product",
                        ComponentDomainV283::None => return Err(mismatch()),
                    };
                    let source_type = row.ty.index();
                    write!(out, "\n && invocation_source_stored_{domain}_projection_v296(after, {local}, {source_type}, {leaf}) == invocation_source_stored_{domain}_projection_v296(before, {local}, {source_type}, {leaf})")
                        .map_err(|_| out.error())?;
                }
            }
        }
        write!(out, "\n}}\nproof fn checked_add_actual_demanded_step_{root}_{instance}_{block}_{statement}_v296(\n c: InvocationSourceMicroStateV36, left: int, right: int, little_endian: bool,\n)\n requires invocation_source_active_{root}_{instance}_v36(c.source),\n c.source.machine.pc == {pc}, c.next_statement == {statement}, c.next_statement == c.observations.len(),\n c.source.machine.valid && invocation_source_byte_state_well_formed_v36(c.source),\n 0 <= {destination} < c.source.machine.values.len(), !c.source.objects.contains_key({destination}),\n 0 <= {left} < c.source.machine.values.len(), 0 <= {right} < c.source.machine.values.len(),\n c.source.machine.values[{left}] == MemoryValueV30::Scalar(left),\n c.source.machine.values[{right}] == MemoryValueV30::Scalar(right),\n 0 <= left < 4294967296, 0 <= right < 4294967296,\n ensures ({{ let n = invocation_source_micro_step_{root}_{instance}_v36(c, little_endian);\n n.source.machine.valid && invocation_source_active_{root}_{instance}_v36(n.source)\n && invocation_source_byte_state_well_formed_v36(n.source)\n && n.source.machine.pc == c.source.machine.pc && n.source.machine.values.len() == c.source.machine.values.len()\n && n.source.machine.memory == c.source.machine.memory && n.source.machine.frames == c.source.machine.frames\n && n.source.machine.generations == c.source.machine.generations\n && n.source.slots == c.source.slots && n.source.objects == c.source.objects\n && checked_prefix_demands_{root}_{instance}_{block}_{statement}_v296(c.source, n.source, left, right) }}),\n{{\n checked_add_actual_micro_step_{root}_{instance}_{block}_{statement}_v293(c, left, right, little_endian);\n checked_add_actual_schema_{root}_{instance}_{block}_{statement}_v260();\n invocation_source_checked_add_local_projections_v296(c.source, {destination}, {ty}, {left}, {right}, left, right, {root}, {instance}, little_endian);\n reveal(invocation_source_micro_step_{root}_{instance}_v36);\n reveal(invocation_source_micro_record_v36);\n}}\n")
            .map_err(|_| out.error())?;
        write!(out, r#"proof fn checked_add_actual_demanded_prefix_{root}_{instance}_{block}_{statement}_v296(
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
 let n = invocation_source_micro_run_{root}_{instance}_v36(c, fuel + 1, little_endian);
 n.source.machine.valid && invocation_source_active_{root}_{instance}_v36(n.source)
 && invocation_source_byte_state_well_formed_v36(n.source)
 && n.source.machine.pc == p.source.machine.pc && n.source.machine.values.len() == p.source.machine.values.len()
 && n.source.machine.memory == p.source.machine.memory && n.source.machine.frames == p.source.machine.frames
 && n.source.machine.generations == p.source.machine.generations
 && n.source.slots == p.source.slots && n.source.objects == p.source.objects
 && checked_prefix_demands_{root}_{instance}_{block}_{statement}_v296(p.source, n.source, left, right) }}),
{{
 let p = invocation_source_micro_run_{root}_{instance}_v36(c, fuel, little_endian);
 checked_add_actual_demanded_step_{root}_{instance}_{block}_{statement}_v296(p, left, right, little_endian);
 invocation_source_micro_run_composes_{root}_{instance}_v292(c, fuel, 1, little_endian);
 reveal_with_fuel(invocation_source_micro_run_{root}_{instance}_v36, 2);
 assert(invocation_source_micro_run_{root}_{instance}_v36(p, 1, little_endian)
     == invocation_source_micro_step_{root}_{instance}_v36(p, little_endian));
}}
"#).map_err(|_| out.error())?;
        out.budget.release_storage(headers())?;
        Ok(())
    }
}
