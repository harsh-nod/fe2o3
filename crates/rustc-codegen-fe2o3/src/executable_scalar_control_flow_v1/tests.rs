use dialect_mir::{
    MirBasicBlock, MirBinaryOp, MirBlockId, MirConstant, MirConstantValue, MirEdge,
    MirExecutableModule, MirOperand, MirPlace, MirRvalue, MirStatement, MirStatementKind,
    MirTerminator, MirTerminatorKind, MirTypeId, ValidatedMirExecutableModule,
};
use fe2o3_kernel_ir::scalar_ops_v2::{IntBinary, IntMode, Operation as ScalarOperation, Predicate};
use fe2o3_kernel_ir::{FunctionRole, OperationKind, Terminator};

use super::*;

const NESTED_LOOP: &str = include_str!("../../../dialect-mir/tests/fixtures/nested-loop.mir.json");
const INTEGER_MATCH: &str =
    include_str!("../../../dialect-mir/tests/fixtures/integer-match.mir.json");

fn decode(source: &str) -> ValidatedMirExecutableModule {
    MirExecutableModule::from_canonical_text(source).expect("canonical executable MIR")
}

fn authority(source: &ValidatedMirExecutableModule) -> AuthenticatedScalarControlFlowExportV1 {
    AuthenticatedScalarControlFlowExportV1::for_test(
        &source.functions[0].identity,
        source.functions[0].identity.rsplit("::").next().unwrap(),
        CollectedFunctionRole::DeviceFfiExport,
    )
    .unwrap()
}

fn lower(
    source: &ValidatedMirExecutableModule,
) -> Result<ExecutableScalarControlFlowArtifactV1, ExecutableScalarControlFlowErrorV1> {
    lower_executable_scalar_control_flow_v1(source, &authority(source))
}

fn u32_constant(value: u128) -> MirOperand {
    MirOperand::Constant(MirConstant {
        ty: MirTypeId(1),
        value: MirConstantValue::Integer(value),
    })
}

fn bool_constant(value: bool) -> MirOperand {
    MirOperand::Constant(MirConstant {
        ty: MirTypeId(0),
        value: MirConstantValue::Bool(value),
    })
}

fn local(local: u32, ty: u32) -> MirPlace {
    MirPlace::local(dialect_mir::MirLocalId(local), MirTypeId(ty))
}

fn assign(place: MirPlace, value: MirOperand) -> MirStatement {
    MirStatement {
        kind: MirStatementKind::Assign {
            place,
            value: MirRvalue::Use(value),
        },
        span: None,
    }
}

fn goto(target: u32) -> MirTerminator {
    MirTerminator {
        kind: MirTerminatorKind::Goto(MirEdge::new(MirBlockId(target))),
        span: None,
    }
}

fn switch_bool(condition_local: u32, yes: u32, no: u32) -> MirTerminator {
    MirTerminator {
        kind: MirTerminatorKind::SwitchInt {
            discr: MirOperand::Copy(local(condition_local, 0)),
            targets: vec![(1, MirEdge::new(MirBlockId(yes)))],
            otherwise: MirEdge::new(MirBlockId(no)),
        },
        span: None,
    }
}

fn block(statements: Vec<MirStatement>, terminator: MirTerminator) -> MirBasicBlock {
    MirBasicBlock {
        parameters: Vec::new(),
        statements,
        terminator,
    }
}

fn return_block() -> MirBasicBlock {
    block(
        vec![assign(local(0, 1), u32_constant(0))],
        MirTerminator {
            kind: MirTerminatorKind::Return,
            span: None,
        },
    )
}

fn bounded_module(identity: &str) -> MirExecutableModule {
    let mut module = decode(NESTED_LOOP).into_unvalidated();
    module.functions[0].identity = identity.to_owned();
    let locals = &module.functions[0].body.locals;
    module.functions[0].body.locals = vec![locals[0].clone(), locals[2].clone(), locals[5].clone()];
    module.functions[0].body.entry = MirBlockId(0);
    module
}

fn independent_loops(loop_count: usize) -> ValidatedMirExecutableModule {
    let mut module = bounded_module(&format!("tests::independent_loops_{loop_count}"));
    let mut blocks = Vec::with_capacity(2 * loop_count + 2);
    blocks.push(block(
        vec![assign(local(2, 0), bool_constant(true))],
        goto(1),
    ));
    let return_id = (2 * loop_count + 1) as u32;
    for index in 0..loop_count {
        let header = (1 + 2 * index) as u32;
        let body = header + 1;
        let next = if index + 1 == loop_count {
            return_id
        } else {
            body + 1
        };
        blocks.push(block(Vec::new(), switch_bool(2, body, next)));
        blocks.push(block(
            vec![assign(local(2, 0), bool_constant(false))],
            goto(header),
        ));
    }
    blocks.push(return_block());
    module.functions[0].body.blocks = blocks;
    module.validate().expect("independent natural loops")
}

fn nested_loops(depth: usize) -> ValidatedMirExecutableModule {
    let mut module = bounded_module(&format!("tests::nested_loops_{depth}"));
    let mut blocks = Vec::with_capacity(2 * depth + 2);
    blocks.push(block(
        vec![assign(local(2, 0), bool_constant(true))],
        goto(1),
    ));
    let return_id = (1 + 2 * depth) as u32;
    for index in 0..depth {
        let yes = if index + 1 < depth {
            (index + 2) as u32
        } else {
            (1 + depth + index) as u32
        };
        let no = if index == 0 {
            return_id
        } else {
            (depth + index) as u32
        };
        blocks.push(block(Vec::new(), switch_bool(2, yes, no)));
    }
    for index in 0..depth {
        blocks.push(block(Vec::new(), goto((1 + index) as u32)));
    }
    blocks.push(return_block());
    module.functions[0].body.blocks = blocks;
    module.validate().expect("nested natural loops")
}

fn operation_module(operation_count: usize) -> ValidatedMirExecutableModule {
    let mut module = bounded_module(&format!("tests::operations_{operation_count}"));
    let statements = (0..operation_count)
        .map(|value| assign(local(1, 1), u32_constant(value as u128)))
        .chain(std::iter::once(assign(
            local(0, 1),
            MirOperand::Copy(local(1, 1)),
        )))
        .collect();
    module.functions[0].body.locals.truncate(2);
    module.functions[0].body.blocks = vec![block(
        statements,
        MirTerminator {
            kind: MirTerminatorKind::Return,
            span: None,
        },
    )];
    module.validate().expect("bounded operation module")
}

#[test]
fn canonical_nested_loop_reaches_scalar_v2_kernel_ir_and_exact_gfx942_llvm() {
    let source = decode(NESTED_LOOP);
    let artifact = lower(&source).expect("bounded nested loop must lower");

    assert_eq!(artifact.summary.blocks, 9);
    assert_eq!(artifact.summary.loops, 2);
    assert_eq!(artifact.summary.maximum_loop_depth, 2);
    assert_eq!(artifact.scalar_operations.len(), 6);
    assert!(artifact.summary.kernel_ir_operations >= artifact.scalar_operations.len());
    assert_eq!(artifact.mem2reg.functions.len(), 1);
    assert!(artifact.mem2reg.promoted_local_count() >= 4);
    assert_eq!(artifact.canonical_function_identity, "fixture::nested_loop");
    assert_eq!(
        artifact.kernel_ir.functions[0].role,
        FunctionRole::DeviceFfiExport
    );
    assert_eq!(
        artifact.kernel_ir.functions[0].id.as_str(),
        artifact.emitted_symbol
    );

    let adds = artifact
        .scalar_operations
        .iter()
        .filter(|operation| {
            matches!(
                operation.operation.operation(),
                ScalarOperation::IntegerBinary {
                    op: IntBinary::Add,
                    mode: IntMode::Wrapping,
                    ..
                }
            )
        })
        .count();
    let comparisons = artifact
        .scalar_operations
        .iter()
        .filter(|operation| {
            matches!(
                operation.operation.operation(),
                ScalarOperation::IntegerCompare {
                    predicate: Predicate::Eq | Predicate::Lt,
                    ..
                }
            )
        })
        .count();
    assert_eq!((adds, comparisons), (3, 3));

    let body = artifact.kernel_ir.functions[0].body.as_ref().unwrap();
    for loop_header in [1, 3] {
        assert!(!body.blocks[loop_header].parameters.is_empty());
    }
    assert!(
        body.blocks.iter().any(|block| {
            matches!(block.terminator, Some(Terminator::ConditionalBranch { .. }))
        })
    );
    assert!(
        body.blocks
            .iter()
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(operation.kind, OperationKind::Binary { .. }))
    );

    assert!(
        artifact
            .gfx942_llvm
            .starts_with("target triple = \"amdgcn-amd-amdhsa\"\ntarget datalayout = \"e-p:64:64")
    );
    assert!(artifact.gfx942_llvm.contains(&format!(
        "define i32 @{}(i32 %arg0)",
        artifact.emitted_symbol
    )));
    assert!(artifact.gfx942_llvm.contains("\"target-cpu\"=\"gfx942\""));
    assert!(
        artifact
            .gfx942_llvm
            .contains("\"target-features\"=\"-wavefrontsize32,+wavefrontsize64,-xnack\"")
    );
    assert!(artifact.gfx942_llvm.contains(" = phi i32 "));

    let repeated = lower(&source).unwrap();
    assert_eq!(artifact.kernel_ir, repeated.kernel_ir);
    assert_eq!(artifact.scalar_operations, repeated.scalar_operations);
    assert_eq!(artifact.gfx942_llvm, repeated.gfx942_llvm);
}

#[test]
fn canonical_integer_match_reaches_typed_kernel_ir_switch() {
    let artifact = lower(&decode(INTEGER_MATCH)).expect("integer match must lower");
    let body = artifact.kernel_ir.functions[0].body.as_ref().unwrap();
    let Terminator::IntegerSwitch { cases, .. } = body.blocks[0].terminator.as_ref().unwrap()
    else {
        panic!("expected typed integer switch");
    };
    assert_eq!(cases.len(), 3);
    assert!(artifact.gfx942_llvm.contains("switch i32 %arg0"));
    assert!(artifact.gfx942_llvm.contains("i32 42, label %bb3"));
}

#[test]
fn raw_division_fails_before_kernel_ir_or_llvm_is_returned() {
    let mut source = decode(NESTED_LOOP).into_unvalidated();
    let statement = source.functions[0]
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.statements)
        .find(|statement| {
            matches!(
                statement.kind,
                MirStatementKind::Assign {
                    value: MirRvalue::BinaryOp {
                        op: MirBinaryOp::Add,
                        ..
                    },
                    ..
                }
            )
        })
        .unwrap();
    let MirStatementKind::Assign {
        value: MirRvalue::BinaryOp { op, .. },
        ..
    } = &mut statement.kind
    else {
        unreachable!();
    };
    *op = MirBinaryOp::Div;
    let source = source.validate().unwrap();

    let error = lower(&source).expect_err("raw division must remain fail closed");
    assert!(matches!(
        error,
        ExecutableScalarControlFlowErrorV1::Scalar {
            source: RustcScalarAdmissionErrorV2::UnsupportedMir(message),
            ..
        } if message.contains("exact assertion terminator")
    ));
}

#[test]
fn export_authority_rejects_helpers_and_forged_identities() {
    let source = decode(NESTED_LOOP);
    let helper = AuthenticatedScalarControlFlowExportV1::for_test(
        &source.functions[0].identity,
        "nested_loop",
        CollectedFunctionRole::InternalHelper,
    )
    .expect_err("an ordinary helper must not gain export authority");
    assert!(matches!(
        helper,
        ExecutableScalarControlFlowErrorV1::Authority { .. }
    ));

    let trusted = authority(&source);
    let mut forged = source.into_unvalidated();
    forged.functions[0].identity = "attacker::forged_export".to_owned();
    let forged = forged.validate().unwrap();
    let error = lower_executable_scalar_control_flow_v1(&forged, &trusted)
        .expect_err("a structurally valid forged identity must reject");
    assert!(matches!(
        error,
        ExecutableScalarControlFlowErrorV1::Authority { .. }
    ));
}

#[test]
fn complete_canonical_identity_prevents_same_stem_collisions() {
    let mut left = decode(NESTED_LOOP).into_unvalidated();
    left.functions[0].identity = "left::same".to_owned();
    let left = left.validate().unwrap();
    let left_artifact = lower(&left).unwrap();

    let mut right = decode(NESTED_LOOP).into_unvalidated();
    right.functions[0].identity = "right::same".to_owned();
    let right = right.validate().unwrap();
    let right_artifact = lower(&right).unwrap();

    assert_ne!(left_artifact.emitted_symbol, right_artifact.emitted_symbol);
    assert_ne!(left_artifact.kernel_ir, right_artifact.kernel_ir);
    assert_ne!(left_artifact.gfx942_llvm, right_artifact.gfx942_llvm);
    assert!(
        left_artifact
            .emitted_symbol
            .starts_with("same__fe2o3_scf_v1_")
    );
    assert_eq!(
        left_artifact.emitted_symbol.len(),
        "same__fe2o3_scf_v1_".len() + 64
    );
}

#[test]
fn loop_count_boundary_is_preflighted_exactly() {
    let exact = lower(&independent_loops(16)).expect("16 loops are admitted");
    assert_eq!(exact.summary.loops, 16);
    assert_eq!(exact.summary.maximum_loop_depth, 1);

    let error = lower(&independent_loops(17)).expect_err("17 loops must reject");
    assert!(matches!(
        error,
        ExecutableScalarControlFlowErrorV1::ResourceLimit {
            resource: "natural loop count",
            limit: 16,
            actual: 17,
        }
    ));
}

#[test]
fn loop_depth_boundary_is_preflighted_exactly() {
    let exact = lower(&nested_loops(8)).expect("depth eight is admitted");
    assert_eq!(exact.summary.loops, 8);
    assert_eq!(exact.summary.maximum_loop_depth, 8);

    let error = lower(&nested_loops(9)).expect_err("depth nine must reject");
    assert!(matches!(
        error,
        ExecutableScalarControlFlowErrorV1::ResourceLimit {
            resource: "natural loop nesting depth",
            limit: 8,
            actual: 9,
        }
    ));
}

#[test]
fn operation_boundary_is_preflighted_exactly() {
    let exact = lower(&operation_module(4_096)).expect("4096 operations are admitted");
    assert_eq!(exact.summary.kernel_ir_operations, 4_096);

    let error = lower(&operation_module(4_097)).expect_err("4097 operations must reject");
    assert!(matches!(
        error,
        ExecutableScalarControlFlowErrorV1::ResourceLimit {
            resource: "Kernel IR operation count",
            limit: 4_096,
            actual: 4_097,
        }
    ));
}

#[test]
fn multiple_executable_functions_exceed_the_closed_v1_bound() {
    let source = decode(NESTED_LOOP);
    let trusted = authority(&source);
    let mut source = source.into_unvalidated();
    let mut second = source.functions[0].clone();
    second.identity = "fixture::nested_loop_second".to_owned();
    source.functions.push(second);
    source
        .functions
        .sort_by(|lhs, rhs| lhs.identity.cmp(&rhs.identity));
    let source = source.validate().unwrap();

    let error = lower_executable_scalar_control_flow_v1(&source, &trusted)
        .expect_err("V1 must reject a second function");
    assert!(matches!(
        error,
        ExecutableScalarControlFlowErrorV1::ResourceLimit {
            resource: "function count",
            limit: 1,
            actual: 2,
        }
    ));
}
