use dialect_mir::pliron::{
    MirDialectLimits, MirModuleOp, MirSemanticOperationKind, MirSemanticSourceSpan,
    MirSemanticSpanProvenance,
};
use fe2o3_lower_mir_kernel::{
    LoweringConfig, LoweringError, LoweringLimits, MirKernelLoweringPass, SourceOperationEvidence,
    register_pass,
};
use pliron::{context::Context, op::Op};

fn span() -> MirSemanticSourceSpan {
    MirSemanticSourceSpan::new([1, 2, 3, 4], 9, 3, 9, 17).unwrap()
}

fn provenance() -> MirSemanticSpanProvenance {
    MirSemanticSpanProvenance::new(span(), span()).unwrap()
}

fn pass() -> MirKernelLoweringPass {
    MirKernelLoweringPass::new(
        LoweringConfig::new(LoweringLimits::new(1, 2, 4, 32, 2).unwrap(), 1).unwrap(),
    )
}

#[test]
fn exact_typed_rust_return_feeds_supported_lowering() {
    let mut context = Context::new();
    register_pass(&mut context).unwrap();
    let module =
        MirModuleOp::try_new(&mut context, "rust-return", MirDialectLimits::default()).unwrap();
    let function = module.append_function(&mut context, "kernel", &[]).unwrap();
    function
        .entry_block(&context)
        .unwrap()
        .replace_with_semantic_terminator(
            &mut context,
            0,
            MirSemanticOperationKind::TerminatorReturn,
            [10, 20, 30, 40],
            provenance(),
            &[],
        )
        .unwrap();

    let mut pass = pass();
    let result = pass
        .run_checked(module.get_operation(), &mut context)
        .expect("typed return is supported");
    assert_eq!(
        result.record().source().functions()[0].blocks()[0].operations()[1],
        SourceOperationEvidence::SemanticReturn {
            identity: [10, 20, 30, 40],
            provenance: provenance(),
        }
    );
}

#[test]
fn unsupported_typed_rust_statement_rejects_without_fallback() {
    let mut context = Context::new();
    register_pass(&mut context).unwrap();
    let module =
        MirModuleOp::try_new(&mut context, "rust-reject", MirDialectLimits::default()).unwrap();
    let function = module.append_function(&mut context, "kernel", &[]).unwrap();
    function
        .entry_block(&context)
        .unwrap()
        .append_semantic_statement(
            &mut context,
            0,
            MirSemanticOperationKind::StatementAssign,
            [10, 20, 30, 40],
            provenance(),
        )
        .unwrap();

    let mut pass = pass();
    assert_eq!(
        pass.run_checked(module.get_operation(), &mut context),
        Err(LoweringError::UnsupportedRustSemanticOperation {
            function: 0,
            block: 0,
            ordinal: 0,
            kind: MirSemanticOperationKind::StatementAssign,
            provenance: provenance(),
        })
    );
    assert!(pass.take_result().is_none());
}
