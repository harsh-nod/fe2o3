fn reauthenticate_capabilities_from_enum_payload_v1(
    binding: &mut SemanticValueBindingV1,
    local: SemanticLocalIdV1,
    variant: u32,
) -> Result<(), &'static str> {
    if semantic_binding_contains_execution_v29(binding) {
        return Err("execution bindings cannot be restored from enum payloads");
    }
    reauthenticate_ordinary_capabilities_from_enum_payload_v1(binding, local, variant)
}

fn reauthenticate_ordinary_capabilities_from_enum_payload_v1(
    binding: &mut SemanticValueBindingV1,
    local: SemanticLocalIdV1,
    variant: u32,
) -> Result<(), &'static str> {
    let availability = SemanticCapabilityAvailabilityV1::EnumPayload { local, variant };
    match binding {
        SemanticValueBindingV1::Aggregate(fields) => {
            for field in fields {
                reauthenticate_ordinary_capabilities_from_enum_payload_v1(field, local, variant)?;
            }
        }
        SemanticValueBindingV1::Enum {
            variant: Some(selected),
            payloads,
            ..
        } => {
            if let Some(fields) = payloads.get_mut(selected) {
                for field in fields {
                    reauthenticate_ordinary_capabilities_from_enum_payload_v1(
                        field, local, variant,
                    )?;
                }
            }
        }
        SemanticValueBindingV1::IndexWitness {
            availability: slot @ Some(_),
            ..
        } => *slot = Some(availability),
        SemanticValueBindingV1::GridLeader { availability: slot }
        | SemanticValueBindingV1::ComponentWitness {
            availability: slot, ..
        } => *slot = availability,
        SemanticValueBindingV1::Execution(_)
        | SemanticValueBindingV1::ExecutionBorrow(_)
        | SemanticValueBindingV1::MovedExecution => {
            return Err("execution bindings cannot be restored from enum payloads");
        }
        SemanticValueBindingV1::Unit
        | SemanticValueBindingV1::Unmaterialized
        | SemanticValueBindingV1::Enum { .. }
        | SemanticValueBindingV1::MathContext
        | SemanticValueBindingV1::CollectiveContext
        | SemanticValueBindingV1::WorkgroupLdsScope
        | SemanticValueBindingV1::DynamicLds { .. }
        | SemanticValueBindingV1::MatrixContext
        | SemanticValueBindingV1::WaveLane { .. }
        | SemanticValueBindingV1::MatrixFragment { .. }
        | SemanticValueBindingV1::AccumulatorFragment { .. }
        | SemanticValueBindingV1::Gfx950LdsTransposeTile { .. }
        | SemanticValueBindingV1::WorkgroupPipeline { .. }
        | SemanticValueBindingV1::Value { .. }
        | SemanticValueBindingV1::OptionPointer { .. }
        | SemanticValueBindingV1::IndexWitness {
            availability: None, ..
        }
        | SemanticValueBindingV1::OptionIndexWitness { .. }
        | SemanticValueBindingV1::OptionComponentWitness { .. }
        | SemanticValueBindingV1::OptionGridLeader { .. } => {}
    }
    Ok(())
}

fn semantic_binding_contains_execution_v29(binding: &SemanticValueBindingV1) -> bool {
    match binding {
        SemanticValueBindingV1::Execution(_)
        | SemanticValueBindingV1::ExecutionBorrow(_)
        | SemanticValueBindingV1::MovedExecution => true,
        SemanticValueBindingV1::Value {
            ty: Type::Execution(_),
            ..
        } => true,
        SemanticValueBindingV1::Aggregate(fields) => {
            fields.iter().any(semantic_binding_contains_execution_v29)
        }
        SemanticValueBindingV1::Enum { payloads, .. } => payloads
            .values()
            .flatten()
            .any(semantic_binding_contains_execution_v29),
        _ => false,
    }
}
