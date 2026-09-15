use super::*;

fn extend_function(
    original: &SemanticFunctionDeclV1,
    ty: SemanticTypeIdV1,
) -> SemanticFunctionDeclV1 {
    let mut locals = original.locals().to_vec();
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([locals.len() as u8 + 1; 32]),
        ty,
        SemanticLocalRoleV1::Temporary,
        original.source(),
    ));
    SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        locals,
        original.entry(),
        original.blocks().to_vec(),
    )
    .unwrap()
}

fn add_fields(
    types: &mut Vec<SemanticTypeDeclV1>,
    function: &mut SemanticFunctionDeclV1,
    fields: Vec<SemanticTypeIdV1>,
) -> (u32, SemanticTypeIdV1) {
    let ty = SemanticTypeIdV1::from_index(types.len() as u32);
    let mut size = 0;
    let offsets = fields
        .iter()
        .map(|field| {
            let offset = size;
            size += types[field.index() as usize].layout().size_bytes().unwrap();
            offset
        })
        .collect();
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([types.len() as u8 + 1; 32]),
        SemanticLayoutIdentityV1::from_sha256([types.len() as u8 + 1; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(size),
            8,
            SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
        )
        .unwrap(),
        aggregate(fields),
    ));
    let local = function.locals().len() as u32;
    *function = extend_function(function, ty);
    (local, ty)
}

#[test]
fn product_one_deep_path_eight_succeeds_nine_rejects_whole_aggregate() {
    let (mut types, mut function, mut state) = fixture();
    let (mut local, mut ty) = (1, REF);
    let mut nested_types = vec![REF];
    // Seven fields on only one input, then one product field makes depth eight.
    for _ in 0..7 {
        let (next, next_ty) = add_fields(&mut types, &mut function, vec![ty]);
        let value = assignment(
            &types,
            &function,
            &state,
            &construct(next, next_ty, None, vec![copy(local, ty)]),
        )
        .unwrap();
        state.insert(next as usize, value);
        (local, ty) = (next, next_ty);
        nested_types.push(ty);
    }
    let (product_local, product_ty) = add_fields(&mut types, &mut function, vec![ty, REF]);
    let value = assignment(
        &types,
        &function,
        &state,
        &construct(
            product_local,
            product_ty,
            None,
            vec![copy(local, ty), copy(2, REF)],
        ),
    )
    .unwrap();
    assert!(matches!(
        value,
        ProjectedCapabilityValueV1::CapturedGlobalProduct(_)
    ));
    state.insert(product_local as usize, value);
    let deep_path = nested_types
        .iter()
        .rev()
        .map(|ty| (SemanticProjectionKindV1::Field(0), *ty))
        .collect::<Vec<_>>();
    assert_eq!(deep_path.len(), 8);
    assert_eq!(
        product::use_value(
            &types,
            &function,
            &state,
            &use_path(product_local, &deep_path),
            REF
        ),
        Some(state[&1].clone())
    );
    let shallow = use_path(product_local, &[(SemanticProjectionKindV1::Field(1), REF)]);
    assert_eq!(
        product::use_value(&types, &function, &state, &shallow, REF),
        Some(state[&2].clone())
    );

    let (outer, outer_ty) = add_fields(&mut types, &mut function, vec![product_ty]);
    let before = state.clone();
    assert!(
        assignment(
            &types,
            &function,
            &state,
            &construct(outer, outer_ty, None, vec![copy(product_local, product_ty)])
        )
        .is_none()
    );
    assert_eq!(state, before);
    assert!(!state.contains_key(&(outer as usize)));
    // The shallow path must not survive as a silently narrowed outer capture.
    assert!(
        product::use_value(
            &types,
            &function,
            &state,
            &use_path(
                outer,
                &[
                    (SemanticProjectionKindV1::Field(0), product_ty),
                    (SemanticProjectionKindV1::Field(1), REF)
                ]
            ),
            REF
        )
        .is_none()
    );
}

#[test]
fn product_one_deep_path_borrow_boundary_is_atomic() {
    let (mut types, mut function, mut state) = fixture();
    let (mut local, mut ty) = (1, REF);
    for _ in 0..6 {
        let (next, next_ty) = add_fields(&mut types, &mut function, vec![ty]);
        state.insert(
            next as usize,
            assignment(
                &types,
                &function,
                &state,
                &construct(next, next_ty, None, vec![copy(local, ty)]),
            )
            .unwrap(),
        );
        (local, ty) = (next, next_ty);
    }
    let (product_local, product_ty) = add_fields(&mut types, &mut function, vec![ty, REF]);
    state.insert(
        product_local as usize,
        assignment(
            &types,
            &function,
            &state,
            &construct(
                product_local,
                product_ty,
                None,
                vec![copy(local, ty), copy(2, REF)],
            ),
        )
        .unwrap(),
    );
    let borrowed_ty = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([230; 32]),
        SemanticLayoutIdentityV1::from_sha256([230; 32]),
        SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
        pointer(product_ty, false),
    ));
    let borrowed_local = function.locals().len() as u32;
    function = extend_function(&function, borrowed_ty);
    let borrowed = assignment(
        &types,
        &function,
        &state,
        &assign(
            borrowed_local,
            borrowed_ty,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: place(product_local, product_ty),
            },
        ),
    )
    .unwrap();
    assert!(matches!(
        borrowed,
        ProjectedCapabilityValueV1::CapturedGlobalProduct(_)
    ));
    state.insert(borrowed_local as usize, borrowed);
    let (outer, outer_ty) = add_fields(&mut types, &mut function, vec![borrowed_ty]);
    assert!(
        assignment(
            &types,
            &function,
            &state,
            &construct(
                outer,
                outer_ty,
                None,
                vec![copy(borrowed_local, borrowed_ty)]
            )
        )
        .is_none()
    );
    assert!(state.contains_key(&(borrowed_local as usize)));
    assert!(!state.contains_key(&(outer as usize)));
}

#[test]
fn product_sixteen_fields_succeed_seventeen_reject_all_handles() {
    let (mut types, mut function, state) = fixture();
    for count in [16, 17] {
        let mut fields = vec![SCALAR; count];
        fields[0] = REF;
        fields[count - 1] = REF;
        let (local, ty) = add_fields(&mut types, &mut function, fields);
        let mut operands = (0..count)
            .map(|_| {
                SemanticOperandV1::Constant(SemanticConstantV1::new(
                    SCALAR,
                    SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(9, 8).unwrap()),
                ))
            })
            .collect::<Vec<_>>();
        operands[0] = copy(1, REF);
        operands[count - 1] = copy(2, REF);
        let before = state.clone();
        let result = assignment(
            &types,
            &function,
            &state,
            &construct(local, ty, None, operands),
        );
        assert_eq!(state, before);
        if count == 16 {
            assert!(matches!(
                result,
                Some(ProjectedCapabilityValueV1::CapturedGlobalProduct(_))
            ));
            let mut retained = state.clone();
            retained.insert(local as usize, result.unwrap());
            for (field, input) in [(0, 1), (count as u32 - 1, 2)] {
                assert_eq!(
                    product::use_value(
                        &types,
                        &function,
                        &retained,
                        &use_path(local, &[(SemanticProjectionKindV1::Field(field), REF)]),
                        REF
                    ),
                    Some(state[&input].clone())
                );
            }
        } else {
            assert!(result.is_none());
        }
    }
}
