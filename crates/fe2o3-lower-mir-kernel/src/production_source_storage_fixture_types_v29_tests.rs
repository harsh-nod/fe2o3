// Fixture-local IDs are dense; source identities and all non-ID facts stay exact.
struct FixtureTypeClosureV29 {
    types: Vec<SemanticTypeDeclV1>,
    ids: Vec<Option<SemanticTypeIdV1>>,
}

fn fixture_type_children_v29(shape: &SemanticTypeShapeV1, mut visit: impl FnMut(SemanticTypeIdV1)) {
    match shape {
        SemanticTypeShapeV1::Pointer(pointer) => visit(pointer.pointee()),
        SemanticTypeShapeV1::Array { element, .. } | SemanticTypeShapeV1::Slice { element } => {
            visit(*element)
        }
        SemanticTypeShapeV1::Tuple(fields)
        | SemanticTypeShapeV1::Aggregate(fields)
        | SemanticTypeShapeV1::Union(fields) => fields.fields().iter().copied().for_each(visit),
        SemanticTypeShapeV1::Enum {
            discriminant,
            variants,
        } => {
            visit(*discriminant);
            for variant in variants {
                for &field in variant.fields().fields() {
                    visit(field);
                }
            }
        }
        SemanticTypeShapeV1::FunctionPointer {
            arguments,
            return_type,
            ..
        } => {
            for &argument in arguments.fields() {
                visit(argument);
            }
            visit(*return_type);
        }
        SemanticTypeShapeV1::Unit
        | SemanticTypeShapeV1::Never
        | SemanticTypeShapeV1::Scalar(_)
        | SemanticTypeShapeV1::ValidityScalar(_)
        | SemanticTypeShapeV1::Opaque => {}
    }
}

impl FixtureTypeClosureV29 {
    fn new(original: &[SemanticTypeDeclV1], roots: &[SemanticTypeIdV1]) -> Self {
        let mut reached = vec![false; original.len()];
        let mut pending = roots.to_vec();
        while let Some(ty) = pending.pop() {
            let index = ty.index() as usize;
            assert!(
                index < original.len(),
                "fixture type edge is outside its source roster"
            );
            if std::mem::replace(&mut reached[index], true) {
                continue;
            }
            fixture_type_children_v29(original[index].shape(), |child| pending.push(child));
        }
        let mut ids = vec![None; original.len()];
        let mut next = 0;
        for (index, retained) in reached.into_iter().enumerate() {
            if retained {
                ids[index] = Some(SemanticTypeIdV1::from_index(next));
                next = next.checked_add(1).unwrap();
            }
        }
        let id = |ty: SemanticTypeIdV1| ids[ty.index() as usize].expect("reachable fixture edge");
        let fields = |fields: &SemanticAggregateTypeV1| {
            SemanticAggregateTypeV1::new(fields.fields().iter().copied().map(id).collect()).unwrap()
        };
        let mut types = Vec::with_capacity(next as usize);
        for (ordinal, declaration) in original.iter().enumerate() {
            if ids[ordinal].is_none() {
                continue;
            }
            let shape = match declaration.shape() {
                SemanticTypeShapeV1::Pointer(pointer) => SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        id(pointer.pointee()),
                        pointer.kind(),
                        pointer.mutability(),
                        pointer.address_space(),
                        pointer.pointer_width_bits(),
                        pointer.metadata(),
                    )
                    .unwrap(),
                ),
                SemanticTypeShapeV1::Array { element, length } => SemanticTypeShapeV1::Array {
                    element: id(*element),
                    length: *length,
                },
                SemanticTypeShapeV1::Slice { element } => SemanticTypeShapeV1::Slice {
                    element: id(*element),
                },
                SemanticTypeShapeV1::Tuple(value) => SemanticTypeShapeV1::Tuple(fields(value)),
                SemanticTypeShapeV1::Aggregate(value) => {
                    SemanticTypeShapeV1::Aggregate(fields(value))
                }
                SemanticTypeShapeV1::Union(value) => SemanticTypeShapeV1::Union(fields(value)),
                SemanticTypeShapeV1::Enum {
                    discriminant,
                    variants,
                } => SemanticTypeShapeV1::enum_type(
                    id(*discriminant),
                    variants
                        .iter()
                        .map(|variant| {
                            SemanticEnumVariantV1::new_with_inhabitedness(
                                variant.discriminant(),
                                fields(variant.fields()),
                                variant.is_uninhabited(),
                            )
                        })
                        .collect(),
                )
                .unwrap(),
                SemanticTypeShapeV1::FunctionPointer {
                    safety,
                    extern_abi,
                    c_variadic,
                    arguments,
                    return_type,
                } => SemanticTypeShapeV1::FunctionPointer {
                    safety: *safety,
                    extern_abi: *extern_abi,
                    c_variadic: *c_variadic,
                    arguments: fields(arguments),
                    return_type: id(*return_type),
                },
                SemanticTypeShapeV1::Unit
                | SemanticTypeShapeV1::Never
                | SemanticTypeShapeV1::Scalar(_)
                | SemanticTypeShapeV1::ValidityScalar(_)
                | SemanticTypeShapeV1::Opaque => declaration.shape().clone(),
            };
            types.push(
                SemanticTypeDeclV1::new(
                    declaration.identity(),
                    declaration.layout_identity(),
                    declaration.layout().clone(),
                    shape,
                )
                .with_rust_type_kind(declaration.rust_type_kind())
                .with_rustc_abi_properties(declaration.abi_properties()),
            );
        }
        Self { types, ids }
    }

    fn id(&self, original: SemanticTypeIdV1) -> SemanticTypeIdV1 {
        self.ids[original.index() as usize].expect("fixture root belongs to the exact closure")
    }
}

fn fixture_type_identity_v29(
    owner: &ProductionSemanticSsaOwnerV1,
    identity: SemanticTypeIdentityV1,
) -> SemanticTypeIdV1 {
    let mut found = owner
        .source_semantic()
        .types()
        .iter()
        .enumerate()
        .filter(|(_, declaration)| declaration.identity() == identity);
    let (ordinal, _) = found
        .next()
        .expect("original declaration identity remains in fixture");
    assert!(
        found.next().is_none(),
        "original declaration identity is unique"
    );
    SemanticTypeIdV1::from_index(u32::try_from(ordinal).unwrap())
}

fn fixture_type_v29(
    owner: &ProductionSemanticSsaOwnerV1,
    original: SemanticTypeIdV1,
) -> SemanticTypeIdV1 {
    fixture_type_identity_v29(owner, types()[original.index() as usize].identity())
}

#[test]
fn exact_fixture_type_closure_preserves_cycles_variants_and_all_non_id_facts() {
    let original = types();
    let closure = FixtureTypeClosureV29::new(&original, &[UNIT, RECURSIVE, NICHE, DIRECT]);
    assert!(closure.types.len() < original.len());
    assert!(closure.ids[DESCRIPTOR.index() as usize].is_none());
    for (old, mapped) in closure.ids.iter().enumerate() {
        let Some(mapped) = mapped else {
            continue;
        };
        let before = &original[old];
        let after = &closure.types[mapped.index() as usize];
        assert_eq!(after.identity(), before.identity());
        assert_eq!(after.layout_identity(), before.layout_identity());
        assert_eq!(after.layout(), before.layout());
        assert_eq!(after.abi_properties(), before.abi_properties());
        assert_eq!(after.rust_type_kind(), before.rust_type_kind());
        let mut expected = Vec::new();
        fixture_type_children_v29(before.shape(), |child| expected.push(closure.id(child)));
        let mut actual = Vec::new();
        fixture_type_children_v29(after.shape(), |child| actual.push(child));
        assert_eq!(actual, expected);
    }
    let SemanticTypeShapeV1::Pointer(link) =
        closure.types[closure.id(LINK).index() as usize].shape()
    else {
        panic!("recursive pointer edge");
    };
    assert_eq!(link.pointee(), closure.id(RECURSIVE));
    let SemanticTypeShapeV1::Enum {
        discriminant,
        variants,
    } = closure.types[closure.id(DIRECT).index() as usize].shape()
    else {
        panic!("original enum");
    };
    assert_eq!(*discriminant, closure.id(SIGNED));
    assert_eq!(
        variants
            .iter()
            .map(|variant| variant.discriminant())
            .collect::<Vec<_>>(),
        [255, 1]
    );
}

#[test]
fn actual_schema_fixtures_have_only_their_original_reachable_type_identities() {
    let original = types();
    let check = |owner: ProductionSemanticSsaOwnerV1, expected: &[SemanticTypeIdV1]| {
        assert_eq!(
            owner
                .source_semantic()
                .types()
                .iter()
                .map(|row| row.identity())
                .collect::<Vec<_>>(),
            expected
                .iter()
                .map(|ty| original[ty.index() as usize].identity())
                .collect::<Vec<_>>()
        );
    };
    check(boundary_schema_owner_v29(WORD), &[UNIT, WORD, BYTE, PAIR]);
    check(boundary_schema_owner_v29(PAIR), &[UNIT, WORD, BYTE, PAIR]);
    check(
        boundary_schema_owner_v29(ARRAY),
        &[UNIT, WORD, BYTE, PAIR, ARRAY],
    );
    check(
        boundary_schema_owner_v29(DIRECT),
        &[UNIT, WORD, BYTE, PAIR, DIRECT, SIGNED],
    );
    check(
        backing_owner_v29(false),
        &[UNIT, WORD, REFERENCE, RECURSIVE],
    );
    check(backing_owner_v29(true), &[UNIT, WORD, RECURSIVE, LINK]);
    check(
        transient_descriptor_owner_v29(),
        &[UNIT, WORD, SLICE, DESCRIPTOR, TWO_DESCRIPTORS],
    );
    for descriptor in [false, true] {
        let (declarations, ty) = original_cycle_types_v29(2, descriptor);
        let expected = std::iter::once(declarations[UNIT.index() as usize].identity())
            .chain(
                declarations[ty.index() as usize..]
                    .iter()
                    .map(|row| row.identity()),
            )
            .collect::<Vec<_>>();
        let owner = original_raw_boundary_owner_v29(declarations, ty);
        assert_eq!(
            owner.source_semantic().types().len(),
            if descriptor { 7 } else { 5 }
        );
        assert_eq!(
            owner
                .source_semantic()
                .types()
                .iter()
                .map(|row| row.identity())
                .collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn fixture_remapping_retains_authenticated_nominal_kind_and_niche_metadata() {
    let fixture = selected_niche_fixture_v29(false, true);
    let original = fixture.owner.source_semantic().types();
    let closure = FixtureTypeClosureV29::new(original, &[fixture.enumeration]);
    let mut nominal = 0;
    for (ordinal, mapped) in closure.ids.iter().enumerate() {
        let Some(mapped) = mapped else {
            continue;
        };
        let before = &original[ordinal];
        let after = &closure.types[mapped.index() as usize];
        assert_eq!(after.identity(), before.identity());
        assert_eq!(after.layout_identity(), before.layout_identity());
        assert_eq!(after.layout(), before.layout());
        assert_eq!(after.abi_properties(), before.abi_properties());
        assert_eq!(after.rust_type_kind(), before.rust_type_kind());
        nominal += usize::from(matches!(
            before.rust_type_kind(),
            SemanticRustTypeKindV1::Execution(_)
        ));
    }
    assert_eq!(nominal, 1);
    let before = &original[fixture.enumeration.index() as usize];
    let after = &closure.types[closure.id(fixture.enumeration).index() as usize];
    assert_eq!(after.layout().variants(), before.layout().variants());
}
