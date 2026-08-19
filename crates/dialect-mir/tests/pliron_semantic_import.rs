#![cfg(feature = "pliron")]

use dialect_mir::{
    MirTypeId,
    pliron::{
        MirDialectBuildError, MirDialectLimits, MirModuleOp, MirSemanticOperationKind,
        MirSemanticSourceSpan, MirSemanticSpanProvenance, MirSnapshotOperation,
        register_mir_dialect,
    },
};
use pliron::{context::Context, op::Op, operation::verify_operation};

fn context() -> Context {
    let mut context = Context::new();
    register_mir_dialect(&mut context);
    context
}

fn span(seed: u64) -> MirSemanticSourceSpan {
    MirSemanticSourceSpan::new([seed, seed + 1, seed + 2, seed + 3], 11, 7, 11, 19)
        .expect("valid span")
}

fn provenance(seed: u64) -> MirSemanticSpanProvenance {
    MirSemanticSpanProvenance::new(span(seed), span(seed + 100)).expect("valid provenance")
}

#[test]
fn exact_semantic_order_and_cfg_survive_owner_bound_snapshot() {
    let mut context = context();
    let module = MirModuleOp::try_new(&mut context, "rust-import", MirDialectLimits::default())
        .expect("module");
    let function = module
        .append_function(&mut context, "kernel", &[MirTypeId(0)])
        .expect("function");
    let entry = function.entry_block(&context).expect("entry handle");
    let block1 = function.append_block(&mut context).expect("block 1");
    let block2 = function.append_block(&mut context).expect("block 2");
    entry
        .append_semantic_statement(
            &mut context,
            0,
            MirSemanticOperationKind::StatementStorageLive,
            [1, 2, 3, 4],
            provenance(10),
        )
        .expect("first statement");
    entry
        .append_semantic_statement(
            &mut context,
            1,
            MirSemanticOperationKind::StatementAssign,
            [5, 6, 7, 8],
            provenance(20),
        )
        .expect("second statement");
    entry
        .replace_with_semantic_terminator(
            &mut context,
            2,
            MirSemanticOperationKind::TerminatorSwitchInt,
            [9, 10, 11, 12],
            provenance(30),
            &[block2.clone(), block1.clone(), block2],
        )
        .expect("terminator");

    verify_operation(module.get_operation(), &context).expect("verified import");
    let snapshot = module
        .body(&context)
        .expect("body handle")
        .semantic_functions(&context)
        .expect("semantic snapshot");
    let operations = snapshot[0].blocks()[0].operations();
    assert_eq!(operations.len(), 4);
    let MirSnapshotOperation::SemanticStatement(first) = &operations[1] else {
        panic!("expected first semantic statement");
    };
    assert_eq!(first.ordinal(), 0);
    assert_eq!(first.kind(), MirSemanticOperationKind::StatementStorageLive);
    assert_eq!(first.identity(), [1, 2, 3, 4]);
    assert_eq!(first.expansion_span(), span(10));
    assert_eq!(first.call_site_span(), span(110));
    let MirSnapshotOperation::SemanticStatement(second) = &operations[2] else {
        panic!("expected second semantic statement");
    };
    assert_eq!(second.ordinal(), 1);
    assert_eq!(second.kind(), MirSemanticOperationKind::StatementAssign);
    let MirSnapshotOperation::SemanticTerminator(terminator) = &operations[3] else {
        panic!("expected semantic terminator");
    };
    assert_eq!(terminator.ordinal(), 2);
    assert_eq!(
        terminator.kind(),
        MirSemanticOperationKind::TerminatorSwitchInt
    );
    assert_eq!(terminator.successors(), &[2, 1, 2]);
}

#[test]
fn semantic_builders_reject_cross_owner_and_field_substitutions() {
    let mut owner = context();
    let module = MirModuleOp::try_new(&mut owner, "owner", MirDialectLimits::default()).unwrap();
    let function = module.append_function(&mut owner, "kernel", &[]).unwrap();
    let entry = function.entry_block(&owner).unwrap();
    let mut foreign = context();

    assert!(matches!(
        entry.append_semantic_statement(
            &mut foreign,
            0,
            MirSemanticOperationKind::StatementNop,
            [1, 0, 0, 0],
            provenance(1),
        ),
        Err(MirDialectBuildError::MalformedOperation(
            "invalid block handle"
        ))
    ));
    assert!(matches!(
        entry.append_semantic_statement(
            &mut owner,
            0,
            MirSemanticOperationKind::StatementNop,
            [0; 4],
            provenance(1),
        ),
        Err(MirDialectBuildError::InvalidSemanticIdentity)
    ));
    assert!(matches!(
        entry.append_semantic_statement(
            &mut owner,
            0,
            MirSemanticOperationKind::TerminatorReturn,
            [1, 0, 0, 0],
            provenance(1),
        ),
        Err(MirDialectBuildError::InvalidSemanticKind(_))
    ));
    assert!(MirSemanticSourceSpan::new([1, 0, 0, 0], 4, 2, 3, 2).is_err());
}

#[test]
fn verification_rejects_noncanonical_operation_order() {
    let mut context = context();
    let module = MirModuleOp::try_new(&mut context, "order", MirDialectLimits::default()).unwrap();
    let function = module.append_function(&mut context, "kernel", &[]).unwrap();
    let entry = function.entry_block(&context).unwrap();
    entry
        .append_semantic_statement(
            &mut context,
            1,
            MirSemanticOperationKind::StatementNop,
            [1, 0, 0, 0],
            provenance(1),
        )
        .unwrap();
    assert!(verify_operation(module.get_operation(), &context).is_err());
}
