// Inert unit fixture for physical projection only, not a source receipt.
use fe2o3_kernel_ir::*;

pub(super) fn fixture() -> (
    ReusableLdsConversionV1,
    ExecutionCapabilityOpV1,
    Type,
    ValueDef,
    Type,
) {
    let id = |n| ExecutionTypeIdentityV1::new([n; 32]);
    let conversion = ReusableLdsConversionV1 {
        input: id(1),
        output: id(2),
        element: id(3),
        layout: ExecutionElementLayoutV1 {
            byte_size: 4,
            byte_alignment: 4,
        },
        elements: 64,
        defined_function: [4; 32],
        defined_abi: [5; 32],
        defined_body: [6; 32],
        source_binding: [7; 32],
    };
    let provenance = ExecutionCapabilityProvenanceV1 {
        root: FunctionId::new("reusable"),
        kernel_binding: [8; 32],
        frontend_unit: [9; 32],
        kernel_marker: [10; 32],
        target_brand: [11; 32],
        launch_brand: [12; 32],
        issuance: [13; 32],
    };
    let input = ExecutionCapabilityTypeV1 {
        source_type: conversion.input,
        provenance: provenance.clone(),
        workgroup_brand: Some([14; 32]),
        epoch: Some([15; 32]),
        role: conversion.input_role(),
    };
    let result = ValueDef::new(
        ValueId(2),
        Type::ExecutionCapability(conversion.output_type(&input).unwrap()),
    );
    let operation = ExecutionCapabilityOperationV1::ReusableLdsConversion(conversion);
    let contract = ExecutionCapabilityOpV1 {
        signature: ExecutionCapabilitySignatureV1::new(&[conversion.input], conversion.output)
            .unwrap(),
        operands: vec![ValueId(1)],
        provenance,
        workgroup_brand: input.workgroup_brand,
        epoch_before: input.epoch,
        epoch_after: None,
        obligations: ExecutionSafetyObligationsV1::from_bits(required_execution_obligations_v1(
            &operation,
        )),
        source: ExecutionCapabilitySourceV1 {
            function: [16; 32],
            operation: conversion.defined_function,
            block: 0,
            occurrence: ExecutionCapabilitySourceOccurrenceV1::from_untrusted_parts(
                [17; 32], [18; 32], [19; 32], 0, 0,
            ),
        },
        operation,
    };
    (
        conversion,
        contract,
        Type::ExecutionCapability(input),
        result,
        Type::pointer(
            Type::Scalar(ScalarType::F32),
            AddressSpace::Workgroup,
            AccessMode::ReadWrite,
        ),
    )
}
