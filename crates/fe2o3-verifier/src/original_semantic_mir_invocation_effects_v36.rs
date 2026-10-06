//! External observations use the same current-state source evaluators as the
//! statement and terminator dispatchers. Private initialization is related by
//! the mandatory heap predicate, never by erasing source validity checks.

pub(super) const INVOCATION_EFFECTS_V36: &str = concat!(
    include_str!("original_semantic_mir_native_provenance_v39.vrs"),
    include_str!("original_semantic_mir_native_initial_heaps_v77.vrs"),
    include_str!("original_semantic_mir_observed_effects_v39.vrs"),
    r#"
spec fn invocation_external_effect_v36(effect: MemoryOperationEffectV30) -> bool {
    match invocation_project_effect_v39(effect) {
        Some(_) => true,
        None => false,
    }
}

spec fn invocation_actual_effects_v36(observations: Seq<MemoryOperationObservationV30>)
    -> Seq<MemoryOperationEffectV30>
    decreases observations.len(),
{
    if observations.len() == 0 { seq![] } else {
        let head = observations[0];
        let effects = if !head.valid_before || !head.valid_after
            || !byte_observation_snapshots_valid_v39(head) {
            seq![MemoryOperationEffectV30::Refused]
        } else { match invocation_project_effect_v39(head.effect) {
            Some(effect) => seq![effect],
            None => seq![],
        } };
        effects + invocation_actual_effects_v36(observations.drop_first())
    }
}

spec fn invocation_source_read_effect_v36(
    source: InvocationSourceByteStateV36, value: InvocationSourceByteValueV36,
    bits: int, root: int, instance: int, little_endian: bool,
) -> Seq<MemoryOperationEffectV30> {
    match value {
        InvocationSourceByteValueV36::Read { access, .. } => {
            let evaluated = invocation_source_byte_evaluate_v36(source, value, bits, root, instance, little_endian);
            match invocation_source_byte_address_v36(source, access, root, instance) {
                Some(pointer) => if !evaluated.source.machine.valid {
                    seq![MemoryOperationEffectV30::Refused]
                } else if invocation_private_allocation_v36(pointer.allocation) { seq![] }
                else { seq![MemoryOperationEffectV30::Read {
                    address: MemoryValueV30::Pointer(pointer), width: access.width,
                    alignment: access.alignment, value: evaluated.value }] },
                None => seq![MemoryOperationEffectV30::Refused],
            }
        }
        InvocationSourceByteValueV36::Constant(_)
        | InvocationSourceByteValueV36::Local { .. }
        | InvocationSourceByteValueV36::Component { .. } => seq![],
    }
}

spec fn invocation_source_statement_effects_v36(
    observation: InvocationSourceStatementObservationV36, little_endian: bool,
) -> Seq<MemoryOperationEffectV30> {
    if !observation.before.machine.valid || !observation.after.machine.valid {
        seq![MemoryOperationEffectV30::Refused]
    } else {
        match observation.event {
            None => seq![MemoryOperationEffectV30::Refused],
            Some(InvocationSourceByteEventV36::ThreadWrite(write)) => {
                let result = invocation_source_thread_write_v88(observation.before, write,
                    observation.root, observation.instance, little_endian);
                if result.source == observation.after {
                    Seq::new(result.observations.len(), |i: int| result.observations[i].effect)
                } else { seq![MemoryOperationEffectV30::Refused] }
            }
            Some(InvocationSourceByteEventV36::ContextIssue(issue)) => {
                let result = invocation_source_context_issue_v161(observation.before, issue);
                if result.source == observation.after {
                    Seq::new(result.observations.len(), |i: int| result.observations[i].effect)
                } else { seq![MemoryOperationEffectV30::Refused] }
            }
            Some(InvocationSourceByteEventV36::ExecutionLoan(event)) => {
                // Loans alter logical authority, not externally visible bytes.
                // An unrelated claimed successor is still an explicit refusal.
                let after = invocation_source_execution_step_v168(observation.before, event);
                if after == observation.after { seq![] }
                else { seq![MemoryOperationEffectV30::Refused] }
            }
            Some(InvocationSourceByteEventV36::WorkgroupDerive(derive)) => {
                let result = invocation_source_workgroup_derive_v168(observation.before, derive);
                if result.source == observation.after {
                    Seq::new(result.observations.len(), |i: int| result.observations[i].effect)
                } else { seq![MemoryOperationEffectV30::Refused] }
            }
            Some(InvocationSourceByteEventV36::TileLoad(load)) => {
                let result = invocation_source_execution_tile_load_v168(observation.before, load,
                    observation.root, observation.instance, little_endian);
                if result.source == observation.after {
                    Seq::new(result.observations.len(), |i: int| result.observations[i].effect)
                } else { seq![MemoryOperationEffectV30::Refused] }
            }
            Some(InvocationSourceByteEventV36::TileTransport(transfer)) => {
                let result = invocation_source_execution_tile_transport_v170(observation.before, transfer);
                if result.source == observation.after {
                    Seq::new(result.observations.len(), |i: int| result.observations[i].effect)
                } else { seq![MemoryOperationEffectV30::Refused] }
            }
            Some(InvocationSourceByteEventV36::ScalarOperands(event)) => {
                let result = invocation_source_scalar_operands_v48(observation.before, event,
                    observation.root, observation.instance, little_endian);
                if result.source == observation.after {
                    Seq::new(result.observations.len(), |i: int| result.observations[i].effect)
                } else { seq![MemoryOperationEffectV30::Refused] }
            }
            Some(InvocationSourceByteEventV36::IntegerCast(cast)) => {
                let result = invocation_source_integer_cast_v43(observation.before, cast,
                    observation.root, observation.instance, little_endian);
                if result.source == observation.after {
                    Seq::new(result.observations.len(), |i: int| result.observations[i].effect)
                } else { seq![MemoryOperationEffectV30::Refused] }
            }
            Some(InvocationSourceByteEventV36::CheckedObject(event)) => {
                let result = invocation_source_checked_object_v44(observation.before, event,
                    observation.root, observation.instance, little_endian);
                if result.source == observation.after {
                    Seq::new(result.observations.len(), |i: int| result.observations[i].effect)
                } else { seq![MemoryOperationEffectV30::Refused] }
            }
            Some(InvocationSourceByteEventV36::EnumConstruct(constructed)) => {
                let result = invocation_source_enum_construct_v43(observation.before, constructed,
                    observation.root, observation.instance, little_endian);
                if result.source == observation.after {
                    Seq::new(result.observations.len(), |i: int| result.observations[i].effect)
                } else { seq![MemoryOperationEffectV30::Refused] }
            }
            Some(InvocationSourceByteEventV36::LogicalEnum(event)) => {
                let result = invocation_source_logical_enum_step_v47(observation.before, event,
                    observation.root, observation.instance, little_endian);
                if result.source == observation.after {
                    Seq::new(result.observations.len(), |i: int| result.observations[i].effect)
                } else { seq![MemoryOperationEffectV30::Refused] }
            }
            Some(InvocationSourceByteEventV36::Discriminant(read)) => {
                let result = invocation_source_discriminant_read_v41(observation.before, read,
                    observation.root, observation.instance, little_endian);
                seq![if result.source == observation.after { result.effect }
                    else { MemoryOperationEffectV30::Refused }]
            }
            Some(InvocationSourceByteEventV36::Transfer { destination, value, bits }) => {
                let read = invocation_source_read_effect_v36(observation.before, value, bits,
                    observation.root, observation.instance, little_endian);
                let evaluated = invocation_source_byte_evaluate_v36(observation.before, value, bits,
                    observation.root, observation.instance, little_endian);
                let write = match destination {
                    InvocationSourceByteDestinationV36::Memory(access) =>
                        match invocation_source_byte_address_v36(evaluated.source, access,
                            observation.root, observation.instance) {
                            Some(pointer) => if invocation_private_allocation_v36(pointer.allocation) { seq![] }
                                else { seq![MemoryOperationEffectV30::Write {
                                    address: MemoryValueV30::Pointer(pointer), width: access.width,
                                    alignment: access.alignment, value: evaluated.value }] },
                            None => seq![MemoryOperationEffectV30::Refused],
                        },
                    InvocationSourceByteDestinationV36::Local(_)
                    | InvocationSourceByteDestinationV36::Component(_) => seq![],
                };
                read + write
            }
            // Borrow formation checks initialized bytes but is not a Load.
            Some(InvocationSourceByteEventV36::Scalar)
            | Some(InvocationSourceByteEventV36::Checked { .. })
            | Some(InvocationSourceByteEventV36::AggregateTransfer { .. })
            | Some(InvocationSourceByteEventV36::AggregateDeinitialize(_))
            | Some(InvocationSourceByteEventV36::AggregateReset { .. })
            | Some(InvocationSourceByteEventV36::WitnessBorrow { .. })
            | Some(InvocationSourceByteEventV36::WitnessTransfer { .. })
            | Some(InvocationSourceByteEventV36::Descriptor(_))
            | Some(InvocationSourceByteEventV36::Pointer(_))
            | Some(InvocationSourceByteEventV36::Address { .. })
            | Some(InvocationSourceByteEventV36::Deinitialize(_))
            | Some(InvocationSourceByteEventV36::ObjectLive { .. })
            | Some(InvocationSourceByteEventV36::ObjectDead { .. })
            | Some(InvocationSourceByteEventV36::StorageLive { .. })
            | Some(InvocationSourceByteEventV36::StorageDead { .. }) => seq![],
        }
    }
}

spec fn invocation_source_statements_effects_v36(
    observations: Seq<InvocationSourceStatementObservationV36>, little_endian: bool,
) -> Seq<MemoryOperationEffectV30>
    decreases observations.len(),
{
    if observations.len() == 0 { seq![] } else {
        invocation_source_statement_effects_v36(observations[0], little_endian)
            + invocation_source_statements_effects_v36(observations.drop_first(), little_endian)
    }
}

spec fn invocation_source_operands_effects_v36(
    observations: Seq<InvocationSourceOperandObservationV36>, little_endian: bool,
) -> Seq<MemoryOperationEffectV30>
    decreases observations.len(),
{
    if observations.len() == 0 { seq![] } else {
        let head = observations[0];
        let effects = if !head.before.machine.valid || !head.after.machine.valid {
            seq![MemoryOperationEffectV30::Refused]
        } else { match head.operand {
            InvocationSourceOperandV36::Scalar { value, bits } =>
                invocation_source_read_effect_v36(head.before, value, bits,
                    head.root, head.instance, little_endian),
            InvocationSourceOperandV36::Pointer { .. }
            | InvocationSourceOperandV36::Slice { .. } => seq![],
            InvocationSourceOperandV36::Execution(_) |
            InvocationSourceOperandV36::Aggregate { .. } | InvocationSourceOperandV36::Enum { .. }
            | InvocationSourceOperandV36::Descriptor { .. } => {
                let evaluated = invocation_source_value_evaluate_v42(head.before, head.operand,
                    head.root, head.instance, little_endian);
                if evaluated.source == head.after && evaluated.value == head.value { seq![] }
                else { seq![MemoryOperationEffectV30::Refused] }
            },
        } };
        effects + invocation_source_operands_effects_v36(observations.drop_first(), little_endian)
    }
}

spec fn invocation_source_effects_v36(result: InvocationSourceBlockResultV36,
    little_endian: bool,
) -> Seq<MemoryOperationEffectV30> {
    invocation_source_statements_effects_v36(result.observations, little_endian)
        + invocation_source_operands_effects_v36(result.operands, little_endian)
        + Seq::new(invocation_source_trap_observations_v40(result).len(),
            |i: int| invocation_source_trap_observations_v40(result)[i].effect)
}
"#
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_effects_keep_operand_reads_after_ordered_statement_effects() {
        let text = INVOCATION_EFFECTS_V36;
        assert!(text.contains("invocation_source_statements_effects_v36(result.observations, little_endian)\n        + invocation_source_operands_effects_v36(result.operands, little_endian)"));
        assert!(text.contains("invocation_source_read_effect_v36(head.before, value, bits"));
        assert!(text.contains("read + write"));
    }

    #[test]
    fn source_effect_projection_reuses_independent_evaluation_and_refuses_bad_observations() {
        let text = INVOCATION_EFFECTS_V36;
        assert!(
            text.contains("invocation_source_byte_evaluate_v36(observation.before, value, bits")
        );
        assert!(text.contains("invocation_source_byte_address_v36(evaluated.source, access"));
        assert!(text.contains("None => seq![MemoryOperationEffectV30::Refused]"));
        assert!(
            text.contains("!observation.before.machine.valid || !observation.after.machine.valid")
        );
        assert!(text.contains("!head.valid_before || !head.valid_after"));
        assert!(!text.contains("byte_end_frame_v30"));
    }

    #[test]
    fn aggregate_and_descriptor_operand_effects_require_exact_evaluated_state_and_value() {
        let checked = "| InvocationSourceOperandV36::Descriptor { .. } => {\n                let evaluated = invocation_source_value_evaluate_v42(head.before, head.operand,\n                    head.root, head.instance, little_endian);\n                if evaluated.source == head.after && evaluated.value == head.value { seq![] }\n                else { seq![MemoryOperationEffectV30::Refused] }";
        assert_eq!(INVOCATION_EFFECTS_V36.matches(checked).count(), 2);
        let legacy = INVOCATION_EFFECTS_V36
            .split_once("spec fn invocation_source_operands_effects_v36(")
            .unwrap()
            .1;
        assert!(legacy.contains("!head.before.machine.valid || !head.after.machine.valid"));
        assert!(legacy.contains("InvocationSourceOperandV36::Aggregate { .. } | InvocationSourceOperandV36::Enum { .. }\n            | InvocationSourceOperandV36::Descriptor { .. } => {"));
        assert!(!legacy.contains("InvocationSourceOperandV36::Descriptor { .. } => seq![]"));
        assert!(!legacy.contains("InvocationSourceOperandV36::Aggregate { .. } => seq![]"));
        let aggregate = include_str!("original_semantic_mir_source_aggregate_values_v42.vrs");
        assert!(aggregate.contains("InvocationSourceOperandV36::Descriptor { local, recipe, moved } =>\n            invocation_source_descriptor_evaluate_v53(source, local, recipe, moved)"));
        assert!(aggregate.contains("InvocationSourceOperandV36::Aggregate { place, moved } =>\n            invocation_source_aggregate_place_evaluate_v42(source, place, moved)"));
        let snapshots = include_str!("original_semantic_mir_source_descriptor_snapshots_v53.vrs");
        for retained in [
            "if recipe.mutable && !moved",
            "invocation_source_descriptor_snapshot_v53(source, local, recipe)",
            "invocation_source_byte_put_local_v36(source, local, MemoryValueV30::Undefined)",
            "invocation_source_descriptor_snapshot_current_v53(consumed, snapshot, recipe)",
            "source.logical.versions[recipe.origin] == reference.loan.version",
            "source.machine.frames.active[i] == reference.loan.frame",
        ] {
            assert!(snapshots.contains(retained), "{retained}");
        }
    }
}

#[cfg(test)]
#[path = "original_semantic_mir_observed_effects_v39_tests.rs"]
mod observed_tests_v39;
