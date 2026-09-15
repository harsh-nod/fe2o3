//! Shared allocation receiver retains its source reference and owned SSA type.
use super::*;

pub(super) fn valid(
    reference: ExecutionTypeIdentityV1,
    workgroup: ExecutionTypeIdentityV1,
    lds: ExecutionTypeIdentityV1,
    element: ExecutionTypeIdentityV1,
    layout: ExecutionElementLayoutV1,
    elements: u64,
) -> bool {
    let identities = [reference, workgroup, lds, element];
    identities
        .iter()
        .enumerate()
        .all(|(index, identity)| identity.is_complete() && !identities[..index].contains(identity))
        && elements != 0
        && layout.is_complete()
        && layout.checked_footprint(elements).is_some()
}

pub(super) const fn obligations() -> u32 {
    ExecutionSafetyObligationsV1::TARGET_SUPPORT
        | ExecutionSafetyObligationsV1::DYNAMIC_WORKGROUP_IDENTITY
        | ExecutionSafetyObligationsV1::DISJOINT_LDS_ALLOCATION
        | ExecutionSafetyObligationsV1::LIFETIME_VALIDITY
        | ExecutionSafetyObligationsV1::ALIASING_VALIDITY
}

pub(super) fn encode(writer: &mut ContractWriter, operation: &ExecutionCapabilityOperationV1) {
    let ExecutionCapabilityOperationV1::LdsAllocateBorrowed {
        workgroup_reference,
        workgroup,
        lds,
        element,
        layout,
        elements,
    } = operation
    else {
        unreachable!("borrowed allocation encoder requires op30");
    };
    writer.u8(30);
    writer.identity(*workgroup_reference);
    writer.identity(*workgroup);
    writer.identity(*lds);
    writer.identity(*element);
    put_layout(writer, *layout);
    writer.u64(*elements);
}

pub(super) fn decode(reader: &mut ContractReader<'_>) -> Option<ExecutionCapabilityOperationV1> {
    Some(ExecutionCapabilityOperationV1::LdsAllocateBorrowed {
        workgroup_reference: reader.identity()?,
        workgroup: reader.identity()?,
        lds: reader.identity()?,
        element: reader.identity()?,
        layout: get_layout(reader)?,
        elements: reader.u64()?,
    })
}

#[cfg(test)]
mod tests;
