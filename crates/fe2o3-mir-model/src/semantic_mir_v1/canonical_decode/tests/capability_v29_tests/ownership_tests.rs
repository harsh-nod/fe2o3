use super::*;

fn flags(
    request: &InertSemanticMirRequestV1,
    work: u64,
) -> Result<(Vec<bool>, u64), SemanticMirErrorV1> {
    let mut context = ValidationContextV1 {
        request,
        limits: SemanticMirLimitsV1::default()
            .with_limit(SemanticMirResourceV1::ValidationWork, work)
            .unwrap(),
        totals: ValidationTotalsV1::default(),
        work: 0,
        owned_execution_roles: Vec::new(),
    };
    let flags = crate::semantic_mir_v1::capability_v29::owned_role_types(&mut context)?;
    Ok((flags, context.work))
}

fn add_wrapper(
    request: &mut InertSemanticMirRequestV1,
    field: SemanticTypeIdV1,
) -> SemanticTypeIdV1 {
    let mut types = request.types.to_vec();
    let id = SemanticTypeIdV1(types.len() as u32);
    types.push(aggregate(&types, &[field], false));
    request.types = types.into_boxed_slice();
    let mut locals = request.functions[0].locals.to_vec();
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1(identity(130)),
        id,
        SemanticLocalRoleV1::Temporary,
        SemanticSourceProvenanceV1::unavailable(),
    ));
    request.functions[0].locals = locals.into_boxed_slice();
    id
}

fn place(request: &InertSemanticMirRequestV1, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1(
            request.functions[0]
                .locals
                .iter()
                .position(|local| local.ty == ty)
                .unwrap() as u32,
        ),
        vec![],
        ty,
    )
    .unwrap()
}

fn assign(
    request: &mut InertSemanticMirRequestV1,
    ty: SemanticTypeIdV1,
    kind: SemanticRvalueKindV1,
) {
    request.functions[0].blocks[0].statements = vec![SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(request, ty),
            SemanticRvalueV1::new(ty, kind),
        )),
    )]
    .into_boxed_slice();
}

#[test]
fn capability_owned_containment_distinguishes_borrowed_captures() {
    let maximum = SemanticMirLimitsV1::default().limit(SemanticMirResourceV1::ValidationWork);
    let mut request = request(2, 2);
    let wrapper = add_wrapper(&mut request, CONTEXT_REF);
    let (owned, work) = flags(&request, maximum).unwrap();
    assert_eq!(
        owned
            .iter()
            .enumerate()
            .filter_map(|(index, owned)| owned.then_some(index))
            .collect::<Vec<_>>(),
        vec![
            CONTEXT.0 as usize,
            WORKGROUP.0 as usize,
            TILE.0 as usize,
            FRAGMENT.0 as usize
        ]
    );
    assert!(!owned[wrapper.0 as usize]);
    assert_eq!(flags(&request, work).unwrap().1, work);
    assert!(matches!(
        flags(&request, work - 1),
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::ValidationWork,
            ..
        })
    ));
}

#[test]
fn capability_nested_constants_cannot_fabricate_roles_but_moves_can_transport_them() {
    let limits = SemanticMirLimitsV1::default();
    for field in [CONTEXT, TILE] {
        let mut request = request(2, 2);
        let wrapper = add_wrapper(&mut request, field);
        for ty in [field, wrapper] {
            let mut changed = request.clone();
            let size = changed.types[ty.0 as usize].layout.size_bytes.unwrap();
            let value = if size == 0 {
                SemanticConstantValueV1::ZeroSized
            } else {
                SemanticConstantValueV1::Bytes(
                    SemanticConstantBytesV1::new(vec![0; size as usize]).unwrap(),
                )
            };
            assign(
                &mut changed,
                ty,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                    ty, value,
                ))),
            );
            assert!(matches!(
                changed.admit_exact_v29(limits),
                Err(SemanticMirErrorV1::InvalidTypeOperation {
                    operation: SemanticTypeOperationV1::Constant,
                    ..
                })
            ));
        }
        let operand = SemanticOperandV1::Move(place(&request, field));
        assign(
            &mut request,
            wrapper,
            SemanticRvalueKindV1::aggregate(SemanticAggregateKindV1::Aggregate, vec![operand])
                .unwrap(),
        );
        request.admit_exact_v29(limits).unwrap();
    }
    let mut request = request(2, 2);
    let wrapper = add_wrapper(&mut request, WORKGROUP_REF);
    let operand = SemanticOperandV1::Copy(place(&request, WORKGROUP_REF));
    assign(
        &mut request,
        wrapper,
        SemanticRvalueKindV1::aggregate(SemanticAggregateKindV1::Aggregate, vec![operand]).unwrap(),
    );
    request.admit_exact_v29(limits).unwrap();
}

#[test]
fn capability_direct_aggregate_construction_is_not_issuance() {
    let mut request = request(2, 2);
    let marker = SemanticOperandV1::Constant(SemanticConstantV1::new(
        MARKER,
        SemanticConstantValueV1::ZeroSized,
    ));
    assign(
        &mut request,
        CONTEXT,
        SemanticRvalueKindV1::aggregate(SemanticAggregateKindV1::Aggregate, vec![marker; 5])
            .unwrap(),
    );
    assert!(matches!(
        request.admit_exact_v29(SemanticMirLimitsV1::default()),
        Err(SemanticMirErrorV1::InvalidTypeOperation {
            operation: SemanticTypeOperationV1::Aggregate,
            ..
        })
    ));
}

#[test]
fn capability_owned_containment_is_total_on_cycles_and_forward_dependencies() {
    // These hostile type graphs exercise only the bounded containment algorithm.
    let mut request = request(2, 2);
    let types = [vec![1], vec![0, 2], vec![], vec![4], vec![3], vec![1, 2]]
        .into_iter()
        .enumerate()
        .map(|(index, fields)| {
            let mut ty = request.types[MARKER.0 as usize].clone();
            ty.shape = SemanticTypeShapeV1::Aggregate(
                SemanticAggregateTypeV1::new(fields.into_iter().map(SemanticTypeIdV1).collect())
                    .unwrap(),
            );
            if index == 2 {
                ty.rust_type_kind = SemanticRustTypeKindV1::Execution(Role::KernelContext);
            }
            ty
        })
        .collect::<Vec<_>>();
    request.types = types.into_boxed_slice();
    let maximum = SemanticMirLimitsV1::default().limit(SemanticMirResourceV1::ValidationWork);
    let (owned, work) = flags(&request, maximum).unwrap();
    assert_eq!(owned, [true, true, true, false, false, true]);
    assert_eq!(flags(&request, work).unwrap().0, owned);
    assert!(matches!(
        flags(&request, work - 1),
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::ValidationWork,
            ..
        })
    ));
}

#[test]
fn capability_full_admission_has_an_exact_validation_work_boundary() {
    let request = request(2, 2);
    let limits = SemanticMirLimitsV1::default();
    let (mut lo, mut hi) = (0, limits.limit(SemanticMirResourceV1::ValidationWork));
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let limited = limits
            .with_limit(SemanticMirResourceV1::ValidationWork, mid)
            .unwrap();
        match request.clone().admit_exact_v29(limited) {
            Ok(_) => hi = mid,
            Err(SemanticMirErrorV1::LimitExceeded {
                resource: SemanticMirResourceV1::ValidationWork,
                ..
            }) => lo = mid + 1,
            other => panic!("wrong boundary: {other:?}"),
        }
    }
    assert!(
        request
            .clone()
            .admit_exact_v29(
                limits
                    .with_limit(SemanticMirResourceV1::ValidationWork, lo)
                    .unwrap()
            )
            .is_ok()
    );
    assert!(matches!(
        request.admit_exact_v29(
            limits
                .with_limit(SemanticMirResourceV1::ValidationWork, lo - 1)
                .unwrap()
        ),
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::ValidationWork,
            ..
        })
    ));
}

#[test]
fn capability_containment_counts_empty_variants_and_does_not_follow_signatures() {
    // Isolate containment, including shapes whose layout admission is a separate check.
    let mut request = request(2, 2);
    let mut types = request.types.to_vec();
    let start = types.len();
    for shape in [
        SemanticTypeShapeV1::Array {
            element: CONTEXT,
            length: 0,
        },
        SemanticTypeShapeV1::FunctionPointer {
            safety: SemanticFunctionSafetyV1::Safe,
            extern_abi: SemanticExternAbiV1::Rust,
            c_variadic: false,
            arguments: SemanticAggregateTypeV1::new(vec![WORKGROUP]).unwrap(),
            return_type: CONTEXT,
        },
        SemanticTypeShapeV1::Enum {
            discriminant: SCALAR,
            variants: (0..1000)
                .map(|index| {
                    SemanticEnumVariantV1::new(index, SemanticAggregateTypeV1::new(vec![]).unwrap())
                })
                .collect(),
        },
    ] {
        let mut ty = types[MARKER.0 as usize].clone();
        ty.shape = shape;
        types.push(ty);
    }
    request.types = types.into_boxed_slice();
    let maximum = SemanticMirLimitsV1::default().limit(SemanticMirResourceV1::ValidationWork);
    let (owned, work) = flags(&request, maximum).unwrap();
    assert_eq!(&owned[start..], [true, false, false]);
    assert_eq!(flags(&request, work).unwrap().1, work);
    assert!(matches!(
        flags(&request, work - 1000),
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::ValidationWork,
            ..
        })
    ));
}
