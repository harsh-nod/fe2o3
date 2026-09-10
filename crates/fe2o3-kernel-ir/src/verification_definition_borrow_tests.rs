use super::*;
use crate::{Signature, ValueDef};

fn with_verifier(module: &Module, check: impl FnOnce(&mut FunctionVerifier<'_, '_>)) {
    let function = &module.functions[0];
    let functions = module
        .functions
        .iter()
        .map(|function| (&function.id, function))
        .collect();
    let mut diagnostics = Vec::new();
    let mut verifier = FunctionVerifier::new(
        module,
        function,
        &functions,
        None,
        &mut diagnostics,
        analyze_control_flow(function).ok(),
    );
    verifier.verify();
    check(&mut verifier);
}

#[test]
fn definition_types_borrow_all_three_immutable_owners() {
    let element = Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadOnly);
    let slice = Type::slice(element.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let pointer = Type::pointer(element, AddressSpace::Global, AccessMode::ReadOnly);
    let mut entry = BasicBlock::new(BlockId(91));
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(3),
        arguments: vec![ValueId(u32::MAX)],
    });
    let mut continuation = BasicBlock::new(BlockId(3));
    continuation.parameters = vec![ValueDef::new(ValueId(4000), slice.clone())];
    continuation.operations = vec![Operation::effect_free(
        ValueDef::new(ValueId(8), pointer.clone()),
        OperationKind::SliceData {
            slice: ValueId(4000),
        },
    )];
    continuation.terminator = Some(Terminator::Return {
        values: vec![ValueId(8)],
    });
    let mut module = Module::new("borrowed_definition_types");
    module.functions.push(Function::definition(
        "nested_types",
        Signature::new(vec![slice], vec![pointer]),
        vec![ValueId(u32::MAX)],
        vec![entry, continuation],
    ));
    verify_module(&module).unwrap();

    with_verifier(&module, |verifier| {
        assert!(verifier.diagnostics.is_empty());
        let function = &module.functions[0];
        let block = &function.body.as_ref().unwrap().blocks[1];
        for (value, owner) in [
            (ValueId(u32::MAX), &function.signature.parameters[0]),
            (ValueId(4000), &block.parameters[0].ty),
            (ValueId(8), &block.operations[0].results[0].ty),
        ] {
            assert!(std::ptr::eq(verifier.ty(value).unwrap(), owner));
        }
        assert!(verifier.ty(ValueId(0)).is_none());
        assert_eq!(verifier.definitions.len(), 3);
    });
}

#[test]
fn duplicate_definition_keeps_last_type_site_and_diagnostic_order() {
    let mut block = BasicBlock::new(BlockId(9));
    block.parameters = vec![ValueDef::new(ValueId(7), Type::INDEX)];
    block.operations = vec![Operation::effect_free(
        ValueDef::new(ValueId(7), Type::BOOL),
        OperationKind::Constant(Constant::Bool(true)),
    )];
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(7)],
    });
    let mut module = Module::new("duplicate_definition_types");
    module.functions.push(Function::definition(
        "last_definition",
        Signature::new(vec![Type::F32], vec![Type::BOOL]),
        vec![ValueId(7)],
        vec![block],
    ));
    let base = DiagnosticLocation::function(&module, &module.functions[0]).at_block(BlockId(9));
    let expected = [None, Some(0)].map(|operation| Diagnostic {
        location: DiagnosticLocation {
            operation,
            ..base.clone()
        },
        code: DiagnosticCode::DuplicateValue,
        message: "SSA value %7 is defined more than once".to_owned(),
    });
    assert_eq!(verify_module(&module).unwrap_err().diagnostics(), &expected);

    with_verifier(&module, |verifier| {
        assert_eq!(verifier.diagnostics.as_slice(), &expected);
        assert_eq!(verifier.definitions.len(), 1);
        assert!(matches!(
            verifier.definitions[&ValueId(7)].site,
            DefSite::Operation(BlockId(9), 0)
        ));
        assert!(std::ptr::eq(
            verifier.ty(ValueId(7)).unwrap(),
            &module.functions[0].body.as_ref().unwrap().blocks[0].operations[0].results[0].ty
        ));
        verifier.verify_use(ValueId(7), BlockId(9), Some(0), base.clone());
        assert_eq!(verifier.diagnostics.len(), 3);
        assert_eq!(
            verifier.diagnostics[2].code,
            DiagnosticCode::NonDominatingUse
        );
    });
}

#[test]
fn malformed_parameter_rosters_only_define_the_matched_prefix() {
    for (signature_count, parameter_count) in [(1, 2), (2, 1)] {
        let ty = Type::slice(Type::F32, AddressSpace::Global, AccessMode::ReadOnly);
        let mut block = BasicBlock::new(BlockId(4));
        block.terminator = Some(Terminator::Return { values: vec![] });
        let mut module = Module::new("malformed_parameter_types");
        module.functions.push(Function::definition(
            "parameter_prefix",
            Signature::new(vec![ty; signature_count], vec![]),
            [ValueId(1000), ValueId(u32::MAX)][..parameter_count].to_vec(),
            vec![block],
        ));
        let errors = verify_module(&module).unwrap_err();
        assert_eq!(errors.diagnostics().len(), 1);
        assert_eq!(
            errors.diagnostics()[0].code,
            DiagnosticCode::SignatureMismatch
        );
        with_verifier(&module, |verifier| {
            assert!(verifier.diagnostics.is_empty());
            assert_eq!(verifier.definitions.len(), 1);
            assert!(std::ptr::eq(
                verifier.ty(ValueId(1000)).unwrap(),
                &module.functions[0].signature.parameters[0]
            ));
            assert!(verifier.ty(ValueId(u32::MAX)).is_none());
            verifier.verify_use(
                ValueId(u32::MAX),
                BlockId(4),
                None,
                DiagnosticLocation::function(&module, &module.functions[0]).at_block(BlockId(4)),
            );
            assert_eq!(verifier.diagnostics.len(), 1);
            assert_eq!(verifier.diagnostics[0].code, DiagnosticCode::UndefinedValue);
        });
    }
}
