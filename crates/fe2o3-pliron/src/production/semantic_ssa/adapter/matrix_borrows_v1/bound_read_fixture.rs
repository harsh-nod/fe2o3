#![allow(dead_code)]
use super::canonical_fixture;
use fe2o3_mir_model::semantic_mir_v1::*;

include!("canonical_fixture_helpers.rs");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Mutation {
    None,
    SecondReference,
    DuplicateBorrow,
    ExtraCapture,
    ProjectedCopy,
    AliasEscape,
    DeadOwner,
    Deinitialize,
    PolicyField,
    MoveField,
    NestedRead,
    ProjectedDestination,
    UnknownWrapper,
    NarrowField,
}

fn field(receiver: u32, owned: u32, index: u32, result: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        local_id(receiver),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty(owned)).unwrap(),
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(index), ty(result)).unwrap(),
        ],
        ty(result),
    )
    .unwrap()
}

// Keep the checked Bind and getter bodies. The admitted source closure may
// not retain an unreachable Narrow definition, so Bind-only cases remove it.
pub(super) fn source(mutation: Mutation) -> AdmittedInertSemanticMirV1 {
    use SemanticLocalRoleV1::Temporary as Tmp;
    let old_mutation = match mutation {
        Mutation::DuplicateBorrow => canonical_fixture::Mutation::DuplicateBorrow,
        Mutation::ExtraCapture => canonical_fixture::Mutation::ExtraCapture,
        Mutation::ProjectedCopy => canonical_fixture::Mutation::ProjectedCopy,
        Mutation::DeadOwner => canonical_fixture::Mutation::DeadOwner,
        Mutation::Deinitialize => canonical_fixture::Mutation::Uninitialized,
        _ => canonical_fixture::Mutation::None,
    };
    let seed = canonical_fixture::source(old_mutation);
    let old = &seed.functions()[0];
    let mut locals = old
        .locals()
        .iter()
        .map(|l| (l.ty().index(), l.role()))
        .collect::<Vec<_>>();
    assert_eq!(locals.len(), 13);
    locals.extend([(5, Tmp), (8, Tmp), (9, Tmp), (6, Tmp), (7, Tmp)]);
    let mut types = seed.types().to_vec();
    let mut blocks = old.blocks().to_vec();
    let mut statements = blocks[4].statements().to_vec();
    match mutation {
        Mutation::SecondReference => {
            types.push(reference(13, 7));
            locals.push((13, Tmp));
            statements.push(borrow(18, 13, 7, 7));
        }
        Mutation::AliasEscape => {
            statements.push(assign(14, 8, SemanticRvalueKindV1::Use(copy(8, 8))));
            statements.push(assign(15, 9, aggregate(vec![copy(14, 8), zero(4)])));
        }
        Mutation::PolicyField => statements.push(assign(
            16,
            6,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field(8, 7, 1, 6))),
        )),
        Mutation::MoveField => statements.push(assign(
            13,
            5,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(field(8, 7, 0, 5))),
        )),
        Mutation::NestedRead => {
            let mut projections = field(8, 7, 0, 5).projections().to_vec();
            projections.push(
                SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, ty(2)).unwrap(),
            );
            statements.push(assign(
                2,
                2,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                    SemanticPlaceV1::new(local_id(8), projections, ty(2)).unwrap(),
                )),
            ));
        }
        Mutation::ProjectedDestination => statements.push(SemanticStatementV1::new(
            loc(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                field(8, 7, 0, 5),
                SemanticRvalueV1::new(ty(5), SemanticRvalueKindV1::Use(copy(5, 5))),
            )),
        )),
        Mutation::UnknownWrapper => {
            types.push(aggregate_type(13, &[5, 6, 4, 4], &[0, 8, 16, 16], 16, true));
            types.push(
                reference(14, 13).with_rustc_abi_properties(
                    SemanticTypeAbiPropertiesV1::new(false, false)
                        .with_rustc_layout_is_noundef(true)
                        .with_scalar_pointee_info(Some(pointee(16, 8)), None),
                ),
            );
            locals.extend([(13, Tmp), (14, Tmp)]);
            statements.push(assign(
                18,
                13,
                aggregate(vec![copy(5, 5), copy(6, 6), zero(4), zero(4)]),
            ));
            statements.push(borrow(19, 14, 18, 13));
            statements.push(assign(
                13,
                5,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field(19, 13, 0, 5))),
            ));
        }
        Mutation::NarrowField => {
            types.push(
                reference(13, 9).with_rustc_abi_properties(
                    SemanticTypeAbiPropertiesV1::new(false, false)
                        .with_rustc_layout_is_noundef(true)
                        .with_scalar_pointee_info(Some(pointee(8, 8)), None),
                ),
            );
            locals.push((13, Tmp));
            blocks[5] = block(
                5,
                vec![
                    borrow(18, 13, 9, 9),
                    assign(
                        14,
                        8,
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field(18, 9, 0, 8))),
                    ),
                ],
                SemanticTerminatorKindV1::Return,
            );
        }
        _ => (),
    }
    if mutation != Mutation::NarrowField {
        blocks[0] = block(0, blocks[0].statements().to_vec(), call(5, vec![], 1, 1, 1));
        blocks[1] = block(
            1,
            blocks[1].statements().to_vec(),
            call(3, vec![copy(10, 10), copy(11, 11)], 2, 2, 2),
        );
        blocks[2] = block(
            2,
            blocks[2].statements().to_vec(),
            call(4, vec![copy(12, 12)], 3, 3, 3),
        );
        blocks[4] = block(4, statements, call(2, vec![copy(8, 8)], 13, 5, 5));
    }
    let root = function(30, true, &[], 0, &locals, blocks)
        .with_kernel_entry(old.kernel_entry().unwrap().clone());
    let mut functions = seed.functions().to_vec();
    functions[0] = root;
    let mut callables = seed.callables().to_vec();
    if mutation != Mutation::NarrowField {
        functions.remove(2);
        callables.remove(2);
        callables[2] = SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(2));
    }
    InertSemanticMirRequestV1::new_with_callables(
        seed.target().clone(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        seed.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap()
}
