//! Synthetic admitted carrier/Bind source, never frontend or kernel authority.
use fe2o3_mir_model::semantic_mir_v1::*;

#[path = "../../../matrix_borrows_v1/canonical_fixture.rs"]
mod canonical;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Mutation {
    None,
    DuplicateCapture,
    DeinitializeCapture,
    MoveSubcarrier,
}
pub(super) fn ty(i: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(i)
}
fn loc() -> SemanticSourceProvenanceV1 {
    SemanticSourceProvenanceV1::unavailable()
}
fn place(i: u32, t: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(i), vec![], ty(t)).unwrap()
}
fn mov(i: u32, t: u32) -> SemanticOperandV1 {
    SemanticOperandV1::Move(place(i, t))
}
fn zero(t: u32) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        ty(t),
        SemanticConstantValueV1::ZeroSized,
    ))
}
fn assign(i: u32, t: u32, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        loc(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(i, t),
            SemanticRvalueV1::new(ty(t), value),
        )),
    )
}
fn aggregate(fields: Vec<SemanticOperandV1>) -> SemanticRvalueKindV1 {
    SemanticRvalueKindV1::Aggregate(
        SemanticAggregateRvalueV1::new(SemanticAggregateKindV1::Aggregate, fields).unwrap(),
    )
}
fn field(i: u32, fields: &[(u32, u32)], result: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(i),
        fields
            .iter()
            .map(|&(f, t)| {
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(f), ty(t)).unwrap()
            })
            .collect(),
        ty(result),
    )
    .unwrap()
}
fn aggregate_type(index: u8, fields: &[u32], offsets: &[u64], size: u64) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([100 + index; 32]),
        SemanticLayoutIdentityV1::from_sha256([100 + index; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(size),
            8,
            SemanticAggregateLayoutV1::new(offsets.to_vec(), vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(
            SemanticAggregateTypeV1::new(fields.iter().copied().map(ty).collect()).unwrap(),
        ),
    )
}

pub(super) fn source(mutation: Mutation) -> AdmittedInertSemanticMirV1 {
    let base = canonical::source(canonical::Mutation::None);
    let mut types = base.types().to_vec();
    types.extend([
        aggregate_type(13, &[6, 4], &[0, 8], 8), // captured policy reference + marker
        aggregate_type(14, &[5, 6], &[0, 8], 16), // prior Matrix route must win
        aggregate_type(15, &[6, 6], &[0, 8], 16), // ambiguous policy-only carrier
        aggregate_type(16, &[5, 5, 6], &[0, 8, 16], 24), // ambiguous primary cannot be rescued
        aggregate_type(17, &[6, 6, 5], &[0, 8, 16], 24), // later primary survives policy ambiguity
        aggregate_type(18, &[13, 4], &[0, 8], 8), // nested policy carrier
    ]);
    let mut functions = base.functions().to_vec();
    let root = &functions[0];
    let mut locals = root.locals().to_vec();
    let added = [13, 13, 6, 14, 15, 16, 17, 18, 18];
    for &t in &added {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([70 + locals.len() as u8; 32]),
            ty(t),
            SemanticLocalRoleV1::Temporary,
            loc(),
        ));
    }
    let mut blocks = root.blocks().to_vec();
    let block = &blocks[3];
    let mut statements = block.statements().to_vec();
    let capture = assign(13, 13, aggregate(vec![mov(6, 6), zero(4)]));
    statements.push(capture.clone());
    if mutation == Mutation::DuplicateCapture {
        statements.push(capture)
    }
    statements.push(assign(20, 18, aggregate(vec![mov(13, 13), zero(4)])));
    statements.push(assign(21, 18, SemanticRvalueKindV1::Use(mov(20, 18))));
    if mutation == Mutation::MoveSubcarrier {
        statements.push(assign(
            14,
            13,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(field(21, &[(0, 13)], 13))),
        ));
    } else {
        statements.push(assign(
            14,
            13,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field(21, &[(0, 13)], 13))),
        ));
    }
    if mutation == Mutation::DeinitializeCapture {
        statements.push(SemanticStatementV1::new(
            loc(),
            SemanticStatementKindV1::Deinitialize(place(14, 13)),
        ));
    }
    statements.push(assign(
        15,
        6,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field(14, &[(0, 6)], 6))),
    ));
    let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
        panic!()
    };
    let mut args = call.arguments().to_vec();
    args[1] = SemanticOperandV1::Copy(place(15, 6));
    let call = SemanticDirectCallV1::new_callable(
        call.callee(),
        args,
        call.destination().cloned(),
        call.unwind(),
    )
    .unwrap();
    blocks[3] = SemanticBasicBlockV1::new(
        block.identity(),
        block.source(),
        statements,
        SemanticTerminatorV1::new(
            block.terminator().source(),
            SemanticTerminatorKindV1::Call(call),
        ),
    )
    .unwrap();
    functions[0] = SemanticFunctionDeclV1::new(
        root.identity(),
        root.role(),
        root.item_definition_identity(),
        root.monomorphization_identity(),
        root.generic_type_arguments_identity(),
        root.const_generic_arguments_identity(),
        root.source(),
        root.abi().clone(),
        locals,
        root.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(root.kernel_entry().unwrap().clone());
    InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([90; 32])),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        base.callables().to_vec(),
        base.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap()
}
