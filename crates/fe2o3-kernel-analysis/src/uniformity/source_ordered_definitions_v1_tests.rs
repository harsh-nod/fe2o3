use super::*;
use fe2o3_kernel_ir::{Signature, ValueDef};

fn constant(value: u32, payload: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(value), Type::Scalar(ScalarType::U32)),
        OperationKind::Constant(Constant::U32(payload)),
    )
}

fn check_index(operations: Vec<Operation>, expected: &[(ValueId, u32)], malformed: bool) {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = operations;
    block.terminator = Some(Terminator::Return { values: vec![] });
    let function = Function::internal_helper(
        "source_ordered_definitions",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    );
    let body = function.body.as_ref().unwrap();
    let calls = BTreeSet::new();
    let analyzer = Analyzer::new(&function, body, &[], &calls, &calls, None, None);
    let actual: Vec<_> = analyzer
        .value_definitions
        .iter()
        .map(|(value, operation)| {
            let OperationKind::Constant(Constant::U32(payload)) = &operation.kind else {
                panic!("fixture definition must remain a constant");
            };
            let source = body.blocks[0]
                .operations
                .iter()
                .rev()
                .find(|source| source.results[0].id == *value)
                .unwrap();
            assert!(std::ptr::eq(*operation, source));
            (*value, *payload)
        })
        .collect();
    assert_eq!(actual.as_slice(), expected);
    assert_eq!(analyzer.report.diagnostics().is_empty(), !malformed);
}

#[test]
fn source_ordered_definition_index_accepts_empty_input() {
    check_index(vec![], &[], false);
}

#[test]
fn source_ordered_definition_index_keeps_sorted_keys_and_last_borrow() {
    // Duplicates deliberately remain malformed; this pins diagnostic-path behavior.
    for operations in [
        vec![constant(90, 1), constant(7, 2), constant(90, 3)],
        vec![constant(7, 2), constant(90, 1), constant(90, 3)],
    ] {
        check_index(operations, &[(ValueId(7), 2), (ValueId(90), 3)], true);
    }
}
