use dialect_mir::pliron::{
    MirSemanticOperationKind, MirSemanticSourceSpan, MirSemanticSpanProvenance,
};
use fe2o3_lower_mir_kernel::{
    LoweringConfig, LoweringError, LoweringLimits, MirKernelLoweringConformanceFunctionV1,
    MirKernelLoweringConformanceInputV1, MirKernelLoweringConformanceV1, SourceOperationEvidence,
};

fn span() -> MirSemanticSourceSpan {
    MirSemanticSourceSpan::new([1, 2, 3, 4], 9, 3, 9, 17).unwrap()
}

fn provenance() -> MirSemanticSpanProvenance {
    MirSemanticSpanProvenance::new(span(), span()).unwrap()
}

fn config() -> LoweringConfig {
    LoweringConfig::new(LoweringLimits::new(1, 2, 4, 32, 2).unwrap(), 1).unwrap()
}

#[test]
fn exact_typed_rust_return_feeds_supported_lowering() {
    let input = MirKernelLoweringConformanceInputV1::new(
        "rust-return",
        vec![
            MirKernelLoweringConformanceFunctionV1::new("kernel", vec![]).with_semantic_return(
                0,
                [10, 20, 30, 40],
                provenance(),
            ),
        ],
    );
    let result = MirKernelLoweringConformanceV1
        .run(&input, config())
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
    let input = MirKernelLoweringConformanceInputV1::new(
        "rust-reject",
        vec![
            MirKernelLoweringConformanceFunctionV1::new("kernel", vec![]).with_semantic_statement(
                0,
                MirSemanticOperationKind::StatementAssign,
                [10, 20, 30, 40],
                provenance(),
            ),
        ],
    );
    assert_eq!(
        MirKernelLoweringConformanceV1.run(&input, config()),
        Err(LoweringError::UnsupportedRustSemanticOperation {
            function: 0,
            block: 0,
            ordinal: 0,
            kind: MirSemanticOperationKind::StatementAssign,
            provenance: provenance(),
        })
    );
}
