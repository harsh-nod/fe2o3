//! Inert prefix classification only; full admission and source replay remain
//! mandatory. No statement here creates a completion or a capability value.
use super::*;

fn ty(n: u32) -> SemanticTypeIdV1 { SemanticTypeIdV1::from_index(n) }
fn lid(n: u32) -> SemanticLocalIdV1 { SemanticLocalIdV1::from_index(n) }
fn place(n: u32, t: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(lid(n), vec![], ty(t)).unwrap()
}
fn types() -> Vec<SemanticTypeDeclV1> {
    let pointer = |kind, mutability, pointee| SemanticTypeShapeV1::Pointer(
        SemanticPointerTypeV1::new_with_kind(ty(pointee), kind, mutability, 0, 64,
            SemanticPointerMetadataV1::None).unwrap());
    [
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed: false, bits: 32 }),
        pointer(SemanticPointerKindV1::Reference, SemanticMutabilityV1::Immutable, 0),
        pointer(SemanticPointerKindV1::Reference, SemanticMutabilityV1::Mutable, 0),
        pointer(SemanticPointerKindV1::Raw, SemanticMutabilityV1::Immutable, 0),
        SemanticTypeShapeV1::Unit,
        pointer(SemanticPointerKindV1::Reference, SemanticMutabilityV1::Immutable, 4),
    ].into_iter().enumerate().map(|(n, shape)| SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([n as u8 + 1; 32]),
        SemanticLayoutIdentityV1::from_sha256([n as u8 + 1; 32]),
        SemanticTypeLayoutV1::new(Some(8), 8).unwrap(), shape)).collect()
}
fn locals(t: u32) -> Vec<SemanticLocalDeclV1> {
    [SemanticLocalRoleV1::Return, SemanticLocalRoleV1::Temporary,
        SemanticLocalRoleV1::Argument(0), SemanticLocalRoleV1::Temporary,
        SemanticLocalRoleV1::Temporary].into_iter().enumerate().map(|(n, role)|
            SemanticLocalDeclV1::new(SemanticLocalIdentityV1::from_sha256([n as u8+1;32]),
                ty(t), role, SemanticSourceProvenanceV1::unavailable())).collect()
}
fn assign(dst: SemanticPlaceV1, src: SemanticOperandV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            dst, SemanticRvalueV1::new(src.ty(), SemanticRvalueKindV1::Use(src)))))
}
fn check(t: u32, prefix: SemanticStatementV1) -> Result<()> {
    let statements = [prefix, assign(place(0,t), SemanticOperandV1::Move(place(4,t)))];
    completion_prefix(&locals(t), &types(), &statements, 1, lid(0), lid(1), &mut 100)
}

#[test]
fn phase_completion_prefix_keeps_scalar_and_shared_scalar_reads() {
    for t in [0, 1] {
        check(t, assign(place(3,t), SemanticOperandV1::Copy(place(2,t)))).unwrap();
    }
}
#[test]
fn phase_completion_prefix_rejects_mutable_raw_and_capability_references() {
    for t in [2,3,4,5] {
        assert_eq!(check(t, assign(place(3,t), SemanticOperandV1::Copy(place(2,t)))),
            Err(SemanticMirErrorV1::InvalidFunctionAbi), "type {t}");
    }
}
#[test]
fn phase_completion_prefix_never_redefines_return_completion_or_argument() {
    for destination in [0,1,2] {
        assert_eq!(check(0, assign(place(destination,0), SemanticOperandV1::Copy(place(4,0)))),
            Err(SemanticMirErrorV1::InvalidFunctionAbi));
    }
}
#[test]
fn phase_completion_prefix_never_reads_return_or_completion() {
    for source in [0,1] {
        assert_eq!(check(0, assign(place(3,0), SemanticOperandV1::Copy(place(source,0)))),
            Err(SemanticMirErrorV1::InvalidFunctionAbi));
    }
}
#[test]
fn phase_completion_prefix_never_moves_or_synthesizes_a_value() {
    for operand in [SemanticOperandV1::Move(place(2,0)),
        SemanticOperandV1::Constant(SemanticConstantV1::new(ty(0),
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(1,4).unwrap())))] {
        assert_eq!(check(0, assign(place(3,0), operand)), Err(SemanticMirErrorV1::InvalidFunctionAbi));
    }
}
#[test]
fn phase_completion_prefix_pack_is_last_and_uses_existing_budget() {
    let statements = [assign(place(3,0), SemanticOperandV1::Copy(place(2,0))),
        assign(place(0,0), SemanticOperandV1::Move(place(3,0)))];
    for pack in [0,2,u32::MAX] {
        assert_eq!(completion_prefix(&locals(0), &types(), &statements, pack, lid(0),lid(1), &mut 100),
            Err(SemanticMirErrorV1::InvalidFunctionAbi));
    }
    assert_eq!(completion_prefix(&locals(0), &types(), &statements, 1, lid(0),lid(1), &mut 0),
        Err(SemanticMirErrorV1::LimitExceeded { resource: SemanticMirResourceV1::ValidationWork,
            max: 0, actual: 1 }));
}
#[test]
fn phase_completion_prefix_rejects_projection_and_type_substitution() {
    let projected = SemanticPlaceV1::new(lid(3),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0),ty(0)).unwrap()],ty(0)).unwrap();
    assert_eq!(check(0,assign(projected,SemanticOperandV1::Copy(place(2,0)))),
        Err(SemanticMirErrorV1::InvalidFunctionAbi));
    assert_eq!(check(0,assign(place(3,0),SemanticOperandV1::Copy(place(2,1)))),
        Err(SemanticMirErrorV1::InvalidFunctionAbi));
}
