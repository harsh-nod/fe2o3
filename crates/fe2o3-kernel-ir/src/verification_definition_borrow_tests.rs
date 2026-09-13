use super::*;
use crate::{
    AccessMode, BasicBlock, CanonicalKernelIrVerificationResourceBudgetV1,
    CanonicalKernelIrWorkBudgetV1, Constant, ControlFlowLimits, Operation, OperationKind,
    Signature, Terminator, ValueDef, ValueId, VerificationDefinitionSiteV1,
    VerificationDiagnosticCollectorV1, VerificationFunctionPassV1, VerificationFunctionStateV1,
    VerificationModuleStateV1, analyze_control_flow_with_verification_budget_v1,
    run_verification_function_pass_v1,
};

pub(super) fn with_verifier(
    module: &Module,
    supported: Option<&BTreeSet<TargetCapability>>,
    extra_diagnostics: usize,
    check: impl FnOnce(&mut VerificationFunctionPassV1<'_, '_, '_>),
) -> Vec<Diagnostic> {
    let function = &module.functions[0];
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, usize::MAX);
    let module_state = VerificationModuleStateV1::build(module, &mut budget).unwrap();
    let mut count = VerificationDiagnosticCollectorV1::count();
    run_verification_function_pass_v1(
        module,
        function,
        &module_state,
        supported,
        &mut count,
        &mut budget,
    )
    .unwrap();
    let expected = count.counted().unwrap() + extra_diagnostics;
    count.abandon(&mut budget).unwrap();
    let mut diagnostics =
        VerificationDiagnosticCollectorV1::materialize(expected, &mut budget).unwrap();
    run_verification_function_pass_v1(
        module,
        function,
        &module_state,
        supported,
        &mut diagnostics,
        &mut budget,
    )
    .unwrap();

    let function_state = VerificationFunctionStateV1::build(function, &mut budget)
        .unwrap()
        .unwrap();
    let control_flow = analyze_control_flow_with_verification_budget_v1(
        function,
        ControlFlowLimits::DEFAULT,
        &mut budget,
    )
    .unwrap();
    {
        let mut verifier = VerificationFunctionPassV1 {
            module,
            function,
            module_state: &module_state,
            function_state: &function_state,
            supported_capabilities: supported,
            diagnostics: &mut diagnostics,
            budget: &mut budget,
            control_flow: Some(&control_flow),
            dynamic_workgroup_memory_declarations: 0,
            gfx950_lds_transpose_current_formats: 0,
        };
        check(&mut verifier);
    }
    function_state.release(&mut budget).unwrap();
    control_flow.release(&mut budget).unwrap();
    module_state.release(&mut budget).unwrap();
    let diagnostics = diagnostics.finish_materialized(&mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
    diagnostics
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

    let diagnostics = with_verifier(&module, None, 0, |verifier| {
        let function = &module.functions[0];
        let block = &function.body.as_ref().unwrap().blocks[1];
        for (value, owner) in [
            (ValueId(u32::MAX), &function.signature.parameters[0]),
            (ValueId(4000), &block.parameters[0].ty),
            (ValueId(8), &block.operations[0].results[0].ty),
        ] {
            assert!(std::ptr::eq(
                verifier.definition_type_v1(value).unwrap().unwrap(),
                owner,
            ));
        }
        assert!(verifier.definition_type_v1(ValueId(0)).unwrap().is_none());
        assert_eq!(verifier.function_state.definition_rows().len(), 3);
    });
    assert!(diagnostics.is_empty());
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

    let diagnostics = with_verifier(&module, None, 1, |verifier| {
        // The shared index retains all duplicate rows, but resolves the same
        // last definition as main's former one-entry map.
        let rows = verifier.function_state.definition_rows();
        assert_eq!(rows.len(), 3);
        assert!(rows.iter().all(|row| row.key == 7));
        let definition = verifier
            .function_state
            .definition(ValueId(7), verifier.budget)
            .unwrap()
            .unwrap();
        assert!(matches!(
            definition.site,
            VerificationDefinitionSiteV1::Operation(BlockId(9), 0)
        ));
        assert!(std::ptr::eq(
            definition.ty,
            &module.functions[0].body.as_ref().unwrap().blocks[0].operations[0].results[0].ty
        ));
        verifier
            .verify_use(ValueId(7), BlockId(9), Some(0), &base.borrowed_v1())
            .unwrap();
    });
    let mut expected_use = expected.to_vec();
    expected_use.push(Diagnostic {
        location: base,
        code: DiagnosticCode::NonDominatingUse,
        message: "definition of %7 does not dominate this use".to_owned(),
    });
    expected_use.sort();
    assert_eq!(diagnostics, expected_use);
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
        let diagnostics = with_verifier(&module, None, 1, |verifier| {
            assert_eq!(verifier.function_state.definition_rows().len(), 1);
            assert!(std::ptr::eq(
                verifier.definition_type_v1(ValueId(1000)).unwrap().unwrap(),
                &module.functions[0].signature.parameters[0]
            ));
            assert!(
                verifier
                    .definition_type_v1(ValueId(u32::MAX))
                    .unwrap()
                    .is_none()
            );
            verifier
                .verify_use(
                    ValueId(u32::MAX),
                    BlockId(4),
                    None,
                    &DiagnosticLocation::function(&module, &module.functions[0])
                        .at_block(BlockId(4))
                        .borrowed_v1(),
                )
                .unwrap();
        });
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::UndefinedValue);
    }
}
