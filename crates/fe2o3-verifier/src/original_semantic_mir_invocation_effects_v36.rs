//! External observations use the same current-state source evaluators as the
//! statement and terminator dispatchers. Private initialization is related by
//! the mandatory heap predicate, never by erasing source validity checks.

pub(super) const INVOCATION_EFFECTS_V36: &str = r#"
open spec fn invocation_external_effect_v36(effect: MemoryOperationEffectV30) -> bool {
    match effect {
        MemoryOperationEffectV30::Read { address, .. }
        | MemoryOperationEffectV30::Write { address, .. } => match address {
            MemoryValueV30::Pointer(pointer) => !invocation_private_allocation_v36(pointer.allocation),
            _ => false,
        },
        MemoryOperationEffectV30::Refused => true,
        _ => false,
    }
}

open spec fn invocation_actual_effects_v36(observations: Seq<MemoryOperationObservationV30>)
    -> Seq<MemoryOperationEffectV30>
    decreases observations.len(),
{
    if observations.len() == 0 { seq![] } else {
        let head = observations[0];
        let effects = if !head.valid_before || !head.valid_after {
            seq![MemoryOperationEffectV30::Refused]
        } else if invocation_external_effect_v36(head.effect) { seq![head.effect] } else { seq![] };
        effects + invocation_actual_effects_v36(observations.drop_first())
    }
}

open spec fn invocation_source_read_effect_v36(
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
        _ => seq![],
    }
}

open spec fn invocation_source_statement_effects_v36(
    observation: InvocationSourceStatementObservationV36, little_endian: bool,
) -> Seq<MemoryOperationEffectV30> {
    if !observation.before.machine.valid || !observation.after.machine.valid {
        seq![MemoryOperationEffectV30::Refused]
    } else {
        match observation.event {
            None => seq![MemoryOperationEffectV30::Refused],
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
                    _ => seq![],
                };
                read + write
            }
            // Borrow formation checks initialized bytes but is not a Load.
            Some(_) => seq![],
        }
    }
}

open spec fn invocation_source_statements_effects_v36(
    observations: Seq<InvocationSourceStatementObservationV36>, little_endian: bool,
) -> Seq<MemoryOperationEffectV30>
    decreases observations.len(),
{
    if observations.len() == 0 { seq![] } else {
        invocation_source_statement_effects_v36(observations[0], little_endian)
            + invocation_source_statements_effects_v36(observations.drop_first(), little_endian)
    }
}

open spec fn invocation_source_operands_effects_v36(
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
            _ => seq![],
        } };
        effects + invocation_source_operands_effects_v36(observations.drop_first(), little_endian)
    }
}

open spec fn invocation_source_effects_v36(result: InvocationSourceBlockResultV36,
    little_endian: bool,
) -> Seq<MemoryOperationEffectV30> {
    invocation_source_statements_effects_v36(result.observations, little_endian)
        + invocation_source_operands_effects_v36(result.operands, little_endian)
}
"#;

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
}
