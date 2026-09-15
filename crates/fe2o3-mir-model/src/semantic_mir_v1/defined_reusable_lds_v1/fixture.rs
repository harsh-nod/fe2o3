use super::*;

pub(super) struct Fixture {
    pub functions: Vec<SemanticFunctionDeclV1>,
    pub callables: Vec<SemanticCallableDeclV1>,
    pub declarations: Vec<SemanticTypeDeclV1>,
    pub types: SemanticReusableLdsTypesV1,
    pub source: SemanticReusableLdsSourceV1,
    pub provenance: SemanticKernelCapabilityProvenanceV1,
    pub brand: SemanticTypeIdentityV1,
    pub epoch: SemanticTypeIdentityV1,
}
impl Fixture {
    pub fn observe(&self) -> Result<SemanticReusableLdsConversionV1, SemanticMirErrorV1> {
        SemanticReusableLdsConversionV1::for_defined_function(
            SemanticFunctionIdV1::from_index(1),
            &self.functions,
            &self.callables,
            &self.declarations,
            self.types,
            self.source,
            self.provenance,
            self.brand,
            self.epoch,
            256,
        )
    }
    pub fn replace_caller(&mut self, blocks: Vec<SemanticBasicBlockV1>) {
        self.functions[0] = function(
            30,
            self.functions[0].abi().clone(),
            &[
                (self.types.output, SemanticLocalRoleV1::Return),
                (
                    SemanticTypeIdV1::from_index(8),
                    SemanticLocalRoleV1::Argument(0),
                ),
                (self.types.input, SemanticLocalRoleV1::Temporary),
            ],
            blocks,
        );
    }
    pub fn caller_with_argument(&mut self, argument: SemanticOperandV1) {
        let mut blocks = self.functions[0].blocks().to_vec();
        blocks[1] = call(2, 1, argument, 0, self.types.output, 2);
        self.replace_caller(blocks);
    }
}
pub(super) fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}
pub(super) fn block(tag: u8, kind: SemanticTerminatorKindV1) -> SemanticBasicBlockV1 {
    let s = SemanticSourceProvenanceV1::unavailable();
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([tag; 32]),
        s,
        vec![],
        SemanticTerminatorV1::new(s, kind),
    )
    .unwrap()
}
fn call(
    tag: u8,
    callee: u32,
    argument: SemanticOperandV1,
    destination: u32,
    output: SemanticTypeIdV1,
    next: u32,
) -> SemanticBasicBlockV1 {
    block(
        tag,
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(callee),
                vec![argument],
                Some(SemanticCallDestinationV1::new(
                    place(destination, output),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(next),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        ),
    )
}
pub(super) fn function(
    tag: u8,
    abi: SemanticFunctionAbiV1,
    locals: &[(SemanticTypeIdV1, SemanticLocalRoleV1)],
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let s = SemanticSourceProvenanceV1::unavailable();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([tag; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
        s,
        abi,
        locals
            .iter()
            .enumerate()
            .map(|(i, (ty, role))| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([i as u8 + 1; 32]),
                    *ty,
                    *role,
                    s,
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}
fn abi(tag: u8, input: SemanticTypeIdV1, output: SemanticTypeIdV1) -> SemanticFunctionAbiV1 {
    SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([tag; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        1,
        vec![input],
        output,
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            input,
            SemanticAbiPassModeV1::Ignore,
        ))],
        SemanticAbiValueV1::new(output, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
    .unwrap()
}
fn aggregate(tag: u8, fields: &[SemanticTypeIdV1]) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([tag; 32]),
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(0),
            1,
            SemanticBackendReprV1::memory(true),
            false,
            SemanticAggregateLayoutV1::new(vec![0; fields.len()], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(fields.to_vec()).unwrap()),
    )
}
pub(super) fn fixture() -> Fixture {
    let types =
        SemanticReusableLdsTypesV1::new([0, 1, 2, 3, 4, 5, 6, 7].map(SemanticTypeIdV1::from_index));
    let workgroup = SemanticTypeIdV1::from_index(8);
    let provenance = SemanticKernelCapabilityProvenanceV1::new(
        SemanticFunctionIdV1::from_index(0),
        SemanticKernelBindingIdentityV1::from_sha256([100; 32]),
        SemanticKernelCapabilityFrontendUnitIdentityV1::from_sha256([101; 32]),
        SemanticTypeIdentityV1::from_sha256([102; 32]),
        SemanticKernelCapabilityTargetBrandIdentityV1::from_sha256([103; 32]),
        SemanticKernelCapabilityLaunchBrandIdentityV1::from_sha256([104; 32]),
        SemanticKernelCapabilityIssuanceIdentityV1::from_sha256([105; 32]),
    )
    .unwrap();
    let brand = SemanticTypeIdentityV1::from_sha256([106; 32]);
    let epoch = SemanticTypeIdentityV1::from_sha256([107; 32]);
    let caller = function(
        30,
        abi(40, workgroup, types.output),
        &[
            (types.output, SemanticLocalRoleV1::Return),
            (workgroup, SemanticLocalRoleV1::Argument(0)),
            (types.input, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            call(
                1,
                2,
                SemanticOperandV1::Copy(place(1, workgroup)),
                2,
                types.input,
                1,
            ),
            call(
                2,
                1,
                SemanticOperandV1::Constant(SemanticConstantV1::new(
                    types.input,
                    SemanticConstantValueV1::ZeroSized,
                )),
                0,
                types.output,
                2,
            ),
            block(3, SemanticTerminatorKindV1::Return),
        ],
    );
    let converter = function(
        31,
        abi(41, types.input, types.output),
        &[
            (types.output, SemanticLocalRoleV1::Return),
            (types.input, SemanticLocalRoleV1::Argument(0)),
        ],
        vec![block(1, SemanticTerminatorKindV1::Return)],
    );
    let operation = SemanticExecutionCapabilityContractV1::new(
        SemanticExecutionCapabilityOperationV1::LdsAllocate {
            workgroup,
            lds: types.input,
            element: types.element,
            elements: 256,
        },
        SemanticExecutionCapabilitySignatureV1::new(&[workgroup], types.input).unwrap(),
        provenance,
        brand,
        epoch,
        None,
        SemanticFunctionIdentityV1::from_sha256([32; 32]),
    )
    .unwrap();
    let allocation = SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([32; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([32; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([32; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([32; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([32; 32]),
            SemanticSourceProvenanceV1::unavailable(),
            abi(42, workgroup, types.input),
        ),
        operation: SemanticCompilerIntrinsicOperationV1::ExecutionCapability {
            contract: operation,
        },
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([32; 32]),
    };
    let source = SemanticReusableLdsSourceV1 {
        caller: SemanticFunctionIdV1::from_index(0),
        caller_identity: caller.identity(),
        caller_abi: caller.abi().identity(),
        allocation_callable: SemanticCallableIdV1::from_index(2),
        allocation_identity: SemanticFunctionIdentityV1::from_sha256([32; 32]),
        allocation_abi: SemanticAbiIdentityV1::from_sha256([42; 32]),
        allocation_block: SemanticBlockIdV1::from_index(0),
        allocation_local: SemanticLocalIdV1::from_index(2),
        conversion_block: SemanticBlockIdV1::from_index(1),
        source_binding: [108; 32],
    };
    let element = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([3; 32]),
        SemanticLayoutIdentityV1::from_sha256([3; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 64, 8),
                SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 64,
        }),
    );
    let declarations = vec![
        aggregate(
            1,
            &[
                types.storage_marker,
                types.state_marker,
                types.workgroup_marker,
                types.epoch_marker,
                types.thread_marker,
            ],
        ),
        aggregate(
            2,
            &[
                types.storage_marker,
                types.workgroup_marker,
                types.thread_marker,
            ],
        ),
        element,
        aggregate(4, &[]),
        aggregate(5, &[]),
        aggregate(6, &[]),
        aggregate(7, &[]),
        aggregate(8, &[]),
        aggregate(9, &[]),
    ];
    Fixture {
        functions: vec![caller, converter],
        callables: vec![
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
            allocation,
        ],
        declarations,
        types,
        source,
        provenance,
        brand,
        epoch,
    }
}
