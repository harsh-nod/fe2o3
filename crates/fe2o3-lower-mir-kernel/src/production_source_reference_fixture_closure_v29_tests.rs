use super::*;

fn replace_body(
    original: &SemanticFunctionDeclV1,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let mut result = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source().clone(),
        original.abi().clone(),
        locals,
        original.entry(),
        blocks,
    )
    .unwrap();
    if let Some(entry) = original.kernel_entry() {
        result = result.with_kernel_entry(entry.clone());
    }
    result
}

fn fresh_temporary(locals: &[SemanticLocalDeclV1], ty: SemanticTypeIdV1) -> SemanticLocalDeclV1 {
    let mut ordinal = u32::try_from(locals.len()).unwrap();
    loop {
        let mut bytes = [253; 32];
        bytes[28..].copy_from_slice(&ordinal.to_le_bytes());
        let identity = SemanticLocalIdentityV1::from_sha256(bytes);
        if locals.iter().all(|local| local.identity() != identity) {
            return SemanticLocalDeclV1::new(
                identity,
                ty,
                SemanticLocalRoleV1::Temporary,
                source(),
            );
        }
        ordinal = ordinal.checked_add(1).unwrap();
    }
}

pub(super) fn add_address_observation(
    types: &mut Vec<SemanticTypeDeclV1>,
    functions: &mut [SemanticFunctionDeclV1],
) {
    let raw = SemanticTypeIdV1::from_index(u32::try_from(types.len()).unwrap());
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([251; 32]),
        SemanticLayoutIdentityV1::from_sha256([251; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                WORD,
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    ));
    let original = &functions[2];
    assert_eq!(original.blocks().len(), 1);
    let mut locals = original.locals().to_vec();
    let temporary = u32::try_from(locals.len()).unwrap();
    locals.push(fresh_temporary(&locals, raw));
    let body = &original.blocks()[0];
    let mut statements = body.statements().to_vec();
    let before_return = statements.len().checked_sub(1).unwrap();
    // Rust exposes provenance only from a raw pointer. The preceding cast is
    // itself an address observation of the tracked shared reference.
    statements.splice(
        before_return..before_return,
        [
            assign(
                place(temporary, raw),
                SemanticRvalueKindV1::Cast {
                    kind: SemanticCastKindV1::Pointer,
                    operand: SemanticOperandV1::Copy(place(2, REFERENCE)),
                },
            ),
            assign(
                place(3, WORD),
                SemanticRvalueKindV1::Cast {
                    kind: SemanticCastKindV1::PointerExposeProvenance,
                    operand: SemanticOperandV1::Copy(place(temporary, raw)),
                },
            ),
        ],
    );
    let block = SemanticBasicBlockV1::new(
        body.identity(),
        body.source().clone(),
        statements,
        body.terminator().clone(),
    )
    .unwrap();
    functions[2] = replace_body(original, locals, vec![block]);
}

pub(super) fn retain_fixture_closure(
    types: &[SemanticTypeDeclV1],
    functions: &mut Vec<SemanticFunctionDeclV1>,
) {
    let mut reached = vec![false; functions.len()];
    let mut pending = vec![ROOT.index() as usize];
    while let Some(index) = pending.pop() {
        if std::mem::replace(&mut reached[index], true) {
            continue;
        }
        for block in functions[index].blocks() {
            let callee = match block.terminator().kind() {
                SemanticTerminatorKindV1::Call(call) => Some(call.callee()),
                SemanticTerminatorKindV1::TailCall(call) => Some(call.callee()),
                _ => None,
            };
            if let Some(callee) = callee {
                let callee = callee.index() as usize;
                assert!(callee < functions.len(), "fixture has a non-defined callee");
                pending.push(callee);
            }
        }
    }
    let retained = reached.iter().rposition(|reached| *reached).unwrap() + 1;
    assert!(
        reached[..retained].iter().all(|reached| *reached),
        "fixture reachability is not a dense prefix; explicit ID remapping is required"
    );
    functions.truncate(retained);

    let mut used = vec![false; types.len()];
    let mut pending = functions
        .iter()
        .flat_map(|function| function.locals().iter().map(|local| local.ty()))
        .collect::<Vec<_>>();
    while let Some(ty) = pending.pop() {
        if std::mem::replace(&mut used[ty.index() as usize], true) {
            continue;
        }
        match types[ty.index() as usize].shape() {
            SemanticTypeShapeV1::Unit
            | SemanticTypeShapeV1::Scalar(_)
            | SemanticTypeShapeV1::ValidityScalar(_) => {}
            SemanticTypeShapeV1::Pointer(pointer) => pending.push(pointer.pointee()),
            SemanticTypeShapeV1::Tuple(fields)
            | SemanticTypeShapeV1::Aggregate(fields)
            | SemanticTypeShapeV1::Union(fields) => pending.extend_from_slice(fields.fields()),
            SemanticTypeShapeV1::Array { element, .. } | SemanticTypeShapeV1::Slice { element } => {
                pending.push(*element);
            }
            SemanticTypeShapeV1::Enum { discriminant, variants } => {
                pending.push(*discriminant);
                for variant in variants {
                    pending.extend_from_slice(variant.fields().fields());
                }
            }
            _ => panic!("fixture type closure needs an explicit shape rule"),
        }
    }
    let original = &functions[ROOT.index() as usize];
    let mut locals = original.locals().to_vec();
    // Preserve fixed test type IDs without retaining an unreachable helper.
    // These are ordinary unused MIR temporaries, with no executable effects.
    for (index, used) in used.into_iter().enumerate() {
        if !used {
            let ty = SemanticTypeIdV1::from_index(u32::try_from(index).unwrap());
            locals.push(fresh_temporary(&locals, ty));
        }
    }
    functions[ROOT.index() as usize] = replace_body(original, locals, original.blocks().to_vec());
}
