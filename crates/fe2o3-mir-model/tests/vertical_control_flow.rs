use std::fs;
use std::path::PathBuf;

use fe2o3_mir_model::{
    MirAddressSpace, MirBasicBlock, MirBinaryOp, MirBlockId, MirBody, MirBodyForm, MirConstant,
    MirConstantValue, MirEdge, MirExecutableModule, MirExecutableTarget, MirExecutableVersion,
    MirFunction, MirLayout, MirLocalDecl, MirLocalId, MirLocalKind, MirOperand, MirPlace,
    MirRvalue, MirScalarType, MirSemanticType, MirStatement, MirStatementKind, MirTerminator,
    MirTerminatorKind, MirTypeId, MirTypeKind, analyze_mir_control_flow, promote_module_to_ssa,
};

const BRANCHING_FILL: &str = include_str!("fixtures/branching-fill.mir.json");
const NESTED_LOOP: &str = include_str!("fixtures/nested-loop.mir.json");
const INTEGER_MATCH: &str = include_str!("fixtures/integer-match.mir.json");
const CROSS_BLOCK_SSA: &str = include_str!("fixtures/cross-block-ssa.mir.json");

#[derive(Clone, Copy)]
struct Types {
    bool_ty: MirTypeId,
    u32_ty: MirTypeId,
}

fn types() -> (Vec<MirSemanticType>, Types) {
    let bool_ty = MirSemanticType {
        layout: MirLayout::sized(1, 1),
        kind: MirTypeKind::Scalar(MirScalarType::Bool),
    };
    let u32_ty = MirSemanticType {
        layout: MirLayout::sized(4, 4),
        kind: MirTypeKind::Scalar(MirScalarType::Int {
            signed: false,
            bits: 32,
        }),
    };
    let mut table = vec![bool_ty.clone(), u32_ty.clone()];
    table.sort_by_key(|ty| ty.canonical_text().unwrap());
    let ids = Types {
        bool_ty: MirTypeId(table.iter().position(|ty| ty == &bool_ty).unwrap() as u32),
        u32_ty: MirTypeId(table.iter().position(|ty| ty == &u32_ty).unwrap() as u32),
    };
    (table, ids)
}

fn local(ty: MirTypeId, kind: MirLocalKind, mutable: bool, name: &str) -> MirLocalDecl {
    MirLocalDecl {
        ty,
        kind,
        mutable,
        storage_address_space: MirAddressSpace::DEFAULT,
        name: Some(name.into()),
        span: None,
    }
}

fn place(local: u32, ty: MirTypeId) -> MirPlace {
    MirPlace::local(MirLocalId(local), ty)
}

fn copy(local: u32, ty: MirTypeId) -> MirOperand {
    MirOperand::Copy(place(local, ty))
}

fn constant(value: u128, ty: MirTypeId) -> MirOperand {
    MirOperand::Constant(MirConstant {
        ty,
        value: MirConstantValue::Integer(value),
    })
}

fn assign(local: u32, ty: MirTypeId, value: MirRvalue) -> MirStatement {
    MirStatement {
        kind: MirStatementKind::Assign {
            place: place(local, ty),
            value,
        },
        span: None,
    }
}

fn binary(
    destination: u32,
    destination_ty: MirTypeId,
    operation: MirBinaryOp,
    lhs: MirOperand,
    rhs: MirOperand,
) -> MirStatement {
    assign(
        destination,
        destination_ty,
        MirRvalue::BinaryOp {
            op: operation,
            lhs,
            rhs,
        },
    )
}

fn edge(target: u32) -> MirEdge {
    MirEdge::new(MirBlockId(target))
}

fn terminator(kind: MirTerminatorKind) -> MirTerminator {
    MirTerminator { kind, span: None }
}

fn block(statements: Vec<MirStatement>, kind: MirTerminatorKind) -> MirBasicBlock {
    MirBasicBlock {
        parameters: vec![],
        statements,
        terminator: terminator(kind),
    }
}

fn switch(discr: MirOperand, targets: &[(u128, u32)], otherwise: u32) -> MirTerminatorKind {
    MirTerminatorKind::SwitchInt {
        discr,
        targets: targets
            .iter()
            .map(|(value, target)| (*value, edge(*target)))
            .collect(),
        otherwise: edge(otherwise),
    }
}

fn module(
    identity: &str,
    types: Vec<MirSemanticType>,
    locals: Vec<MirLocalDecl>,
    blocks: Vec<MirBasicBlock>,
) -> MirExecutableModule {
    MirExecutableModule {
        version: MirExecutableVersion::V1,
        target: MirExecutableTarget::gfx942(),
        types,
        callables: vec![],
        functions: vec![MirFunction {
            identity: identity.into(),
            body: MirBody {
                form: MirBodyForm::Places,
                locals,
                blocks,
                entry: MirBlockId(0),
            },
            span: None,
        }],
    }
}

fn branching_fill() -> MirExecutableModule {
    let (types, ids) = types();
    module(
        "fixture::branching_fill",
        types,
        vec![
            local(ids.u32_ty, MirLocalKind::Return, true, "return"),
            local(ids.u32_ty, MirLocalKind::Argument, false, "index"),
            local(ids.u32_ty, MirLocalKind::Temporary, true, "value"),
            local(ids.bool_ty, MirLocalKind::Temporary, true, "in_bounds"),
        ],
        vec![
            block(
                vec![
                    assign(2, ids.u32_ty, MirRvalue::Use(constant(0, ids.u32_ty))),
                    binary(
                        3,
                        ids.bool_ty,
                        MirBinaryOp::Lt,
                        copy(1, ids.u32_ty),
                        constant(10, ids.u32_ty),
                    ),
                ],
                switch(copy(3, ids.bool_ty), &[(1, 1)], 2),
            ),
            block(
                vec![assign(
                    2,
                    ids.u32_ty,
                    MirRvalue::Use(constant(7, ids.u32_ty)),
                )],
                MirTerminatorKind::Goto(edge(3)),
            ),
            block(
                vec![assign(
                    2,
                    ids.u32_ty,
                    MirRvalue::Use(constant(0, ids.u32_ty)),
                )],
                MirTerminatorKind::Goto(edge(3)),
            ),
            block(
                vec![assign(0, ids.u32_ty, MirRvalue::Use(copy(2, ids.u32_ty)))],
                MirTerminatorKind::Return,
            ),
        ],
    )
}

fn integer_match() -> MirExecutableModule {
    let (types, ids) = types();
    module(
        "fixture::integer_match",
        types,
        vec![
            local(ids.u32_ty, MirLocalKind::Return, true, "return"),
            local(ids.u32_ty, MirLocalKind::Argument, false, "tag"),
            local(ids.u32_ty, MirLocalKind::Temporary, true, "result"),
        ],
        vec![
            block(
                vec![assign(
                    2,
                    ids.u32_ty,
                    MirRvalue::Use(constant(0, ids.u32_ty)),
                )],
                switch(copy(1, ids.u32_ty), &[(0, 1), (7, 2), (42, 3)], 4),
            ),
            block(
                vec![assign(
                    2,
                    ids.u32_ty,
                    MirRvalue::Use(constant(10, ids.u32_ty)),
                )],
                MirTerminatorKind::Goto(edge(5)),
            ),
            block(
                vec![assign(
                    2,
                    ids.u32_ty,
                    MirRvalue::Use(constant(20, ids.u32_ty)),
                )],
                MirTerminatorKind::Goto(edge(5)),
            ),
            block(
                vec![assign(
                    2,
                    ids.u32_ty,
                    MirRvalue::Use(constant(30, ids.u32_ty)),
                )],
                MirTerminatorKind::Goto(edge(5)),
            ),
            block(
                vec![assign(
                    2,
                    ids.u32_ty,
                    MirRvalue::Use(constant(99, ids.u32_ty)),
                )],
                MirTerminatorKind::Goto(edge(5)),
            ),
            block(
                vec![assign(0, ids.u32_ty, MirRvalue::Use(copy(2, ids.u32_ty)))],
                MirTerminatorKind::Return,
            ),
        ],
    )
}

fn nested_loop() -> MirExecutableModule {
    let (types, ids) = types();
    let increment = |local| {
        binary(
            local,
            ids.u32_ty,
            MirBinaryOp::Add,
            copy(local, ids.u32_ty),
            constant(1, ids.u32_ty),
        )
    };
    module(
        "fixture::nested_loop",
        types,
        vec![
            local(ids.u32_ty, MirLocalKind::Return, true, "return"),
            local(ids.u32_ty, MirLocalKind::Argument, false, "limit"),
            local(ids.u32_ty, MirLocalKind::Temporary, true, "outer"),
            local(ids.u32_ty, MirLocalKind::Temporary, true, "inner"),
            local(ids.u32_ty, MirLocalKind::Temporary, true, "sum"),
            local(ids.bool_ty, MirLocalKind::Temporary, true, "condition"),
        ],
        vec![
            block(
                vec![
                    assign(2, ids.u32_ty, MirRvalue::Use(constant(0, ids.u32_ty))),
                    assign(3, ids.u32_ty, MirRvalue::Use(constant(0, ids.u32_ty))),
                    assign(4, ids.u32_ty, MirRvalue::Use(constant(0, ids.u32_ty))),
                ],
                MirTerminatorKind::Goto(edge(1)),
            ),
            block(
                vec![binary(
                    5,
                    ids.bool_ty,
                    MirBinaryOp::Lt,
                    copy(2, ids.u32_ty),
                    copy(1, ids.u32_ty),
                )],
                switch(copy(5, ids.bool_ty), &[(1, 2)], 8),
            ),
            block(
                vec![assign(
                    3,
                    ids.u32_ty,
                    MirRvalue::Use(constant(0, ids.u32_ty)),
                )],
                MirTerminatorKind::Goto(edge(3)),
            ),
            block(
                vec![binary(
                    5,
                    ids.bool_ty,
                    MirBinaryOp::Lt,
                    copy(3, ids.u32_ty),
                    copy(1, ids.u32_ty),
                )],
                switch(copy(5, ids.bool_ty), &[(1, 4)], 7),
            ),
            block(
                vec![binary(
                    5,
                    ids.bool_ty,
                    MirBinaryOp::Eq,
                    copy(3, ids.u32_ty),
                    constant(2, ids.u32_ty),
                )],
                switch(copy(5, ids.bool_ty), &[(1, 6)], 5),
            ),
            block(
                vec![binary(
                    4,
                    ids.u32_ty,
                    MirBinaryOp::Add,
                    copy(4, ids.u32_ty),
                    copy(3, ids.u32_ty),
                )],
                MirTerminatorKind::Goto(edge(6)),
            ),
            block(vec![increment(3)], MirTerminatorKind::Goto(edge(3))),
            block(vec![increment(2)], MirTerminatorKind::Goto(edge(1))),
            block(
                vec![assign(0, ids.u32_ty, MirRvalue::Use(copy(4, ids.u32_ty)))],
                MirTerminatorKind::Return,
            ),
        ],
    )
}

fn canonical(module: MirExecutableModule) -> String {
    module.validate().unwrap().to_canonical_text().unwrap()
}

fn promoted_branching_fill() -> String {
    let validated = branching_fill().validate().unwrap();
    let (ssa, _) = promote_module_to_ssa(&validated).unwrap();
    ssa.to_canonical_text().unwrap()
}

#[test]
fn serialized_vertical_fixtures_are_canonical_and_roundtrip() {
    let fixtures = [
        (BRANCHING_FILL, canonical(branching_fill())),
        (NESTED_LOOP, canonical(nested_loop())),
        (INTEGER_MATCH, canonical(integer_match())),
        (CROSS_BLOCK_SSA, promoted_branching_fill()),
    ];
    for (serialized, expected) in fixtures {
        assert_eq!(serialized, expected);
        let decoded = MirExecutableModule::from_canonical_text(serialized).unwrap();
        assert_eq!(decoded.to_canonical_text().unwrap(), serialized);
        for function in &decoded.functions {
            analyze_mir_control_flow(&function.body).unwrap();
        }
    }
}

#[test]
fn branching_fixture_places_only_the_live_join_parameter() {
    let decoded = MirExecutableModule::from_canonical_text(CROSS_BLOCK_SSA).unwrap();
    let body = &decoded.functions[0].body;
    assert_eq!(body.blocks[0].parameters.len(), 1);
    assert!(body.blocks[1].parameters.is_empty());
    assert!(body.blocks[2].parameters.is_empty());
    assert_eq!(body.blocks[3].parameters.len(), 1);
    assert_eq!(body.blocks[3].parameters[0].origin, Some(MirLocalId(2)));
}

#[test]
#[ignore = "run with UPDATE_MIR_FIXTURES=1 to regenerate checked snapshots"]
fn regenerate_serialized_vertical_fixtures() {
    assert_eq!(
        std::env::var_os("UPDATE_MIR_FIXTURES").as_deref(),
        Some(std::ffi::OsStr::new("1"))
    );
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    fs::write(
        directory.join("branching-fill.mir.json"),
        canonical(branching_fill()),
    )
    .unwrap();
    fs::write(
        directory.join("nested-loop.mir.json"),
        canonical(nested_loop()),
    )
    .unwrap();
    fs::write(
        directory.join("integer-match.mir.json"),
        canonical(integer_match()),
    )
    .unwrap();
    fs::write(
        directory.join("cross-block-ssa.mir.json"),
        promoted_branching_fill(),
    )
    .unwrap();
}
