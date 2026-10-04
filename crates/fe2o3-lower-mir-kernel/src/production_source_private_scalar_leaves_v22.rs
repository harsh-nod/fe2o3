// This private namespace names exact typed reads without asserting read-from.
// The enclosing private completion still checks every physical cell history.
fn source_scalar_read_capture_v22<'a>(
    anchors: &'a ScopedMemoryAnchorsV29,
    anchor: &'a ScopedMemoryAnchorV29,
    typed_private: bool,
) -> SourceOwnedResultV18<Option<(&'a ScopedMemoryReadV29, bool)>> {
    match &anchor.kind {
        ScopedMemoryAnchorKindV29::Access {
            payload: Some(ScopedMemoryPayloadV29::Load { read, .. }),
            ..
        } => Ok(Some((read, false))),
        ScopedMemoryAnchorKindV29::Object(index) if typed_private => {
            let object =
                anchors
                    .objects
                    .get(*index)
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "private scalar read object absent",
                    ))?;
            Ok(match &object.role {
                ScopedObjectRoleV29::ReadValue {
                    read: ScopedObjectReadOriginV29::Original(read),
                    ..
                } => Some((read, true)),
                _ => None,
            })
        }
        _ => Ok(None),
    }
}

fn source_scalar_read_kind_v22(kind: &OperationKind, typed_private: bool) -> bool {
    match (kind, typed_private) {
        (OperationKind::Storage(ScopedObjectOperationV29::ReadValue { access, .. }), true) => {
            access.address_space == AddressSpace::Private
                && !access.volatile
                && access.alignment != 0
        }
        (OperationKind::Load { access, .. } | OperationKind::GuardedLoad { access, .. }, false) => {
            !access.volatile
        }
        _ => false,
    }
}

fn source_scalar_read_value_v22(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    instance: usize,
    row: usize,
    anchor: &ScopedMemoryAnchorV29,
    operation: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    read: &ScopedMemoryReadV29,
    typed_private: bool,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<ValueId> {
    if !typed_private {
        let payload = relation
            .retained_scalar_payload_v18(
                root,
                operation,
                &SourcePhysicalAccessV18 {
                    instance,
                    row,
                    anchor,
                },
                budget,
            )?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "scalar leaf payload absent",
            ))?;
        if !matches!(payload.source, ScopedMemoryPayloadV29::Load { read: actual, .. } if actual == read)
        {
            return relation
                .source
                .missing("scalar leaf changed original read capture");
        }
        return Ok(payload.value);
    }
    let object = relation.retained_object_payload_at_v29(root, instance, row, operation, budget)?;
    budget.charge_work(9)?;
    let ScopedObjectRoleV29::ReadValue {
        source,
        read: ScopedObjectReadOriginV29::Original(actual),
    } = object.source.role
    else {
        return relation
            .source
            .missing("private scalar leaf is not an original typed read");
    };
    if actual != *read
        || source.root_type != read.ty
        || source.projected_type != read.ty
        || source.root_schema != source.projected_schema
        || source.path.count != 0
        || object.actual.role != object.source.role
        || !matches!(object.actual.operation, ScopedObjectOperationV29::ReadValue { access, .. }
            if access.address_space == AddressSpace::Private && !access.volatile && access.alignment != 0)
    {
        return relation
            .source
            .missing("private scalar leaf changed whole typed read");
    }
    object
        .actual
        .result
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "private scalar leaf lacks its actual result",
        ))
}
