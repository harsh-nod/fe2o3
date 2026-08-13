#![feature(rustc_private)]

use dialect_mir::{
    MirBinaryOp, MirExecutableModule, MirRvalue, MirStatementKind, ValidatedMirExecutableModule,
};
use fe2o3_kernel_ir::scalar_ops_v2::{IntBinary, IntMode, Operation as ScalarOperation, Predicate};
use fe2o3_kernel_ir::{OperationKind, Terminator};
use rustc_codegen_fe2o3::executable_scalar_control_flow_v1::{
    ExecutableScalarControlFlowErrorV1, lower_executable_scalar_control_flow_v1,
};
use rustc_codegen_fe2o3::scalar_mir_v2::RustcScalarAdmissionErrorV2;

const NESTED_LOOP: &str = include_str!("../../dialect-mir/tests/fixtures/nested-loop.mir.json");
const INTEGER_MATCH: &str = include_str!("../../dialect-mir/tests/fixtures/integer-match.mir.json");

fn decode(source: &str) -> ValidatedMirExecutableModule {
    MirExecutableModule::from_canonical_text(source).expect("canonical executable MIR")
}

#[test]
fn canonical_nested_loop_reaches_scalar_v2_kernel_ir_and_direct_gfx942_llvm() {
    let artifact = lower_executable_scalar_control_flow_v1(&decode(NESTED_LOOP))
        .expect("bounded nested loop must lower");

    assert_eq!(artifact.summary.blocks, 9);
    assert_eq!(artifact.summary.loops, 2);
    assert_eq!(artifact.summary.maximum_loop_depth, 2);
    assert_eq!(artifact.scalar_operations.len(), 6);
    assert!(artifact.summary.kernel_ir_operations >= artifact.scalar_operations.len());
    assert_eq!(artifact.mem2reg.functions.len(), 1);
    assert!(artifact.mem2reg.promoted_local_count() >= 4);

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

    let function = &artifact.kernel_ir.functions[0];
    let body = function.body.as_ref().expect("device function body");
    for loop_header in [1, 3] {
        assert!(
            !body.blocks[loop_header].parameters.is_empty(),
            "bb{loop_header} must carry loop state"
        );
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
            .any(|operation| { matches!(operation.kind, OperationKind::Binary { .. }) })
    );

    assert!(
        artifact
            .gfx942_llvm
            .starts_with("target triple = \"amdgcn-amd-amdhsa\"")
    );
    assert!(
        artifact
            .gfx942_llvm
            .contains("define i32 @nested_loop(i32 %arg0)")
    );
    assert!(artifact.gfx942_llvm.contains("\"target-cpu\"=\"gfx942\""));
    assert!(artifact.gfx942_llvm.contains(" = phi i32 "));
    assert!(artifact.gfx942_llvm.contains("edge_bb4_0_bb6"));

    let repeated = lower_executable_scalar_control_flow_v1(&decode(NESTED_LOOP)).unwrap();
    assert_eq!(artifact.kernel_ir, repeated.kernel_ir);
    assert_eq!(artifact.scalar_operations, repeated.scalar_operations);
    assert_eq!(artifact.gfx942_llvm, repeated.gfx942_llvm);
}

#[test]
fn canonical_integer_match_reaches_typed_kernel_ir_switch() {
    let artifact = lower_executable_scalar_control_flow_v1(&decode(INTEGER_MATCH))
        .expect("integer match must lower");
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
fn raw_division_fails_in_scalar_v2_before_kernel_ir_or_llvm_is_returned() {
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
        .expect("nested-loop add");
    let MirStatementKind::Assign {
        value: MirRvalue::BinaryOp { op, .. },
        ..
    } = &mut statement.kind
    else {
        unreachable!();
    };
    *op = MirBinaryOp::Div;
    let source = source
        .validate()
        .expect("mutated MIR remains structurally valid");

    let error = lower_executable_scalar_control_flow_v1(&source)
        .expect_err("raw division must remain fail closed");
    assert!(matches!(
        error,
        ExecutableScalarControlFlowErrorV1::Scalar {
            source: RustcScalarAdmissionErrorV2::UnsupportedMir(message),
            ..
        } if message.contains("exact assertion terminator")
    ));
}

#[test]
fn multiple_executable_functions_exceed_the_closed_v1_bound() {
    let mut source = decode(NESTED_LOOP).into_unvalidated();
    let mut second = source.functions[0].clone();
    second.identity = "fixture::nested_loop_second".to_owned();
    source.functions.push(second);
    source
        .functions
        .sort_by(|lhs, rhs| lhs.identity.cmp(&rhs.identity));
    let source = source
        .validate()
        .expect("two functions remain valid executable MIR");

    let error = lower_executable_scalar_control_flow_v1(&source)
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
