use super::*;
use crate::{Signature, ValueDef, WaveWidth};

#[test]
fn caller_roster_is_borrowed_while_validating_requirements() {
    let supported = BTreeSet::from([
        TargetCapability::Float64,
        TargetCapability::Extension {
            namespace: "namespace".repeat(128),
            name: "feature".repeat(128),
        },
    ]);
    let mut module = Module::new("borrowed_target");
    module.required_capabilities = supported.clone();
    let mut verifier = ModuleVerifier {
        module: &module,
        diagnostics: Vec::new(),
        functions: BTreeMap::new(),
        supported_capabilities: Some(&supported),
    };
    verifier.verify();
    assert!(verifier.diagnostics.is_empty());
    assert!(std::ptr::eq(
        verifier.supported_capabilities.unwrap(),
        &supported
    ));
    verify_module_with_capabilities(&module, &supported).unwrap();
}

#[test]
fn malformed_target_diagnostics_precede_unsupported_requirements() {
    let mut module = Module::new("malformed_target");
    module.required_capabilities =
        BTreeSet::from([TargetCapability::Float64, TargetCapability::Int64]);
    let supported = BTreeSet::from([
        TargetCapability::SubgroupSize(3),
        TargetCapability::DynamicWorkgroupMemory,
        TargetCapability::Extension {
            namespace: String::new(),
            name: "x".to_owned(),
        },
        TargetCapability::WaveWidth(WaveWidth::Wave32),
        TargetCapability::WaveWidth(WaveWidth::Wave64),
    ]);
    let expected = [
        (
            DiagnosticCode::InvalidCapability,
            "dynamic workgroup memory requires the base workgroup-memory capability",
        ),
        (
            DiagnosticCode::InvalidCapability,
            "conflicting exact wave-width requirements: [Wave32, Wave64]",
        ),
        (
            DiagnosticCode::InvalidCapability,
            "wave width 32 conflicts with the declared subgroup size",
        ),
        (
            DiagnosticCode::InvalidCapability,
            "malformed target capability: SubgroupSize(3)",
        ),
        (
            DiagnosticCode::InvalidCapability,
            "malformed target capability: Extension { namespace: \"\", name: \"x\" }",
        ),
        (
            DiagnosticCode::UnsupportedCapability,
            "target does not support required capability Float64",
        ),
        (
            DiagnosticCode::UnsupportedCapability,
            "target does not support required capability Int64",
        ),
    ]
    .map(|(code, message)| Diagnostic {
        location: DiagnosticLocation::module(&module),
        code,
        message: message.to_owned(),
    });
    let mut verifier = ModuleVerifier {
        module: &module,
        diagnostics: Vec::new(),
        functions: BTreeMap::new(),
        supported_capabilities: Some(&supported),
    };
    verifier.verify();
    assert_eq!(verifier.diagnostics.as_slice(), &expected);
    assert!(std::ptr::eq(
        verifier.supported_capabilities.unwrap(),
        &supported
    ));

    // Public diagnostics sort by code and message within the same location.
    let sorted = [1, 0, 4, 3, 2, 5, 6].map(|index| expected[index].clone());
    assert_eq!(
        verify_module_with_capabilities(&module, &supported)
            .unwrap_err()
            .diagnostics(),
        &sorted
    );
}

#[test]
fn absent_and_empty_targets_keep_all_requirement_checks() {
    let mut block = BasicBlock::new(BlockId(9));
    block.operations.push(Operation::effect_free(
        ValueDef::new(
            ValueId(7),
            pointer_for(Type::F32, AddressSpace::Workgroup, AccessMode::ReadWrite),
        ),
        OperationKind::WorkgroupMemory(WorkgroupMemory {
            element: Type::F32,
            extent: WorkgroupMemoryExtent::Static(1),
            alignment: 4,
        }),
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut function = Function::definition(
        "requires_lds",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    );
    function
        .required_capabilities
        .insert(TargetCapability::WorkgroupMemory);
    let mut module = Module::new("target_requirements");
    module
        .required_capabilities
        .insert(TargetCapability::WorkgroupMemory);
    module.functions.push(function);
    verify_module(&module).unwrap();

    let expected = [
        DiagnosticLocation::module(&module),
        DiagnosticLocation::function(&module, &module.functions[0]),
        DiagnosticLocation::function(&module, &module.functions[0])
            .at_block(BlockId(9))
            .at_operation(0),
    ]
    .map(|location| Diagnostic {
        location,
        code: DiagnosticCode::UnsupportedCapability,
        message: "target does not support required capability WorkgroupMemory".to_owned(),
    });
    assert_eq!(
        verify_module_with_capabilities(&module, &BTreeSet::new())
            .unwrap_err()
            .diagnostics(),
        &expected
    );
    verify_module_with_capabilities(
        &module,
        &BTreeSet::from([TargetCapability::WorkgroupMemory]),
    )
    .unwrap();
}
