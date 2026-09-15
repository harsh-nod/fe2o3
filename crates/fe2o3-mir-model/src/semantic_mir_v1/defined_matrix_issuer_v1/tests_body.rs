struct Fixture {
    functions: Vec<SemanticFunctionDeclV1>,
    callables: Vec<SemanticCallableDeclV1>,
    declarations: Vec<SemanticTypeDeclV1>,
    types: SemanticKernelMatrixDeriveTypesV1,
    provenance: SemanticKernelCapabilityProvenanceV1,
    brand: SemanticTypeIdentityV1,
}

impl Fixture {
    fn observe(&self) -> Result<SemanticKernelMatrixDeriveV1, SemanticMirErrorV1> {
        SemanticKernelMatrixDeriveV1::for_defined_function(
            SemanticFunctionIdV1::from_index(0),
            &self.functions,
            &self.callables,
            &self.declarations,
            self.types,
            self.provenance,
            self.brand,
        )
    }
    fn bridge_with(&mut self, blocks: Vec<SemanticBasicBlockV1>) {
        self.functions[1] = function(
            31,
            abi(41, &[], self.types.matrix, false),
            &[
                (self.types.matrix, SemanticLocalRoleV1::Return),
                (self.types.unbranded_matrix, SemanticLocalRoleV1::Temporary),
            ],
            blocks,
        );
    }
}

fn fixture() -> Fixture {
    let types = SemanticKernelMatrixDeriveTypesV1::new(
        [1, 0, 2, 3, 4, 5].map(SemanticTypeIdV1::from_index),
    );
    let getter = function(
        30,
        abi(40, &[types.context_reference], types.matrix, false),
        &[
            (types.matrix, SemanticLocalRoleV1::Return),
            (types.context_reference, SemanticLocalRoleV1::Argument(0)),
        ],
        vec![
            call(1, 0, types.matrix),
            block(2, vec![], SemanticTerminatorKindV1::Return),
        ],
    );
    let bridge = function(
        31,
        abi(41, &[], types.matrix, false),
        &[
            (types.matrix, SemanticLocalRoleV1::Return),
            (types.unbranded_matrix, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            call(2, 1, types.unbranded_matrix),
            block(2, vec![], SemanticTerminatorKindV1::Return),
        ],
    );
    let current = SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([32; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([32; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([32; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([32; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([32; 32]),
            SemanticSourceProvenanceV1::unavailable(),
            abi(42, &[], types.unbranded_matrix, false),
        ),
        operation: SemanticCompilerIntrinsicOperationV1::MatrixContextCurrent {
            context: types.unbranded_matrix,
        },
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([32; 32]),
    };
    Fixture {
        functions: vec![getter, bridge],
        callables: vec![
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
            current,
        ],
        declarations: vec![
            aggregate(1, &[], false),
            reference(2, types.context),
            aggregate(
                3,
                &[
                    types.unbranded_matrix,
                    types.brand_marker,
                    types.thread_marker,
                ],
                false,
            ),
            aggregate(4, &[], false),
            aggregate(5, &[], false),
            aggregate(6, &[], false),
        ],
        types,
        provenance: SemanticKernelCapabilityProvenanceV1::new(
            SemanticFunctionIdV1::from_index(9),
            SemanticKernelBindingIdentityV1::from_sha256([80; 32]),
            SemanticKernelCapabilityFrontendUnitIdentityV1::from_sha256([81; 32]),
            SemanticTypeIdentityV1::from_sha256([82; 32]),
            SemanticKernelCapabilityTargetBrandIdentityV1::from_sha256([83; 32]),
            SemanticKernelCapabilityLaunchBrandIdentityV1::from_sha256([84; 32]),
            SemanticKernelCapabilityIssuanceIdentityV1::from_sha256([85; 32]),
        )
        .unwrap(),
        brand: SemanticTypeIdentityV1::from_sha256([86; 32]),
    }
}

#[test]
fn exact_observed_issuer_profile_retains_source_and_abi_commitments() {
    let f = fixture();
    let before = f.functions.clone();
    let record = f.observe().unwrap();
    assert_eq!(f.functions, before);
    assert_eq!(record.function().index(), 0);
    assert_eq!(record.bridge().function().index(), 1);
    assert_eq!(record.current_callable().index(), 2);
    assert_eq!(record.source_identity(), f.functions[0].identity());
    assert_eq!(
        record.bridge().abi_identity(),
        f.functions[1].abi().identity()
    );
    assert_eq!(
        record.current_abi_identity(),
        f.callables[2].binding().unwrap().abi().identity()
    );
    assert_eq!(record.types(), f.types);
    assert_eq!(record.provenance(), f.provenance);
    assert_eq!(record.kernel_brand(), f.brand);
    assert_ne!(record.body_identity(), record.bridge().body_identity());
    assert_eq!(f.observe().unwrap(), record);
}

#[test]
fn ignored_aggregate_without_current_issuer_is_not_a_recipe() {
    let mut f = fixture();
    f.bridge_with(vec![block(1, vec![], SemanticTerminatorKindV1::Return)]);
    assert_eq!(f.observe(), Err(SemanticMirErrorV1::InvalidFunctionAbi));
}

#[test]
fn wrong_current_family_or_payload_is_not_a_recipe() {
    for matrix in [false, true] {
        let mut f = fixture();
        let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = &mut f.callables[2]
        else {
            unreachable!()
        };
        *operation = if matrix {
            SemanticCompilerIntrinsicOperationV1::MatrixContextCurrent {
                context: f.types.matrix,
            }
        } else {
            SemanticCompilerIntrinsicOperationV1::MathContextCurrent {
                context: f.types.unbranded_matrix,
            }
        };
        assert_eq!(f.observe(), Err(SemanticMirErrorV1::InvalidFunctionAbi));
    }
}

#[test]
fn wrapper_field_order_and_brand_marker_are_exact() {
    for fields in [[3, 5, 4], [3, 4, 4], [3, 4, 0]] {
        let mut f = fixture();
        f.declarations[2] = aggregate(3, &fields.map(SemanticTypeIdV1::from_index), false);
        assert_eq!(f.observe(), Err(SemanticMirErrorV1::InvalidFunctionAbi));
    }
    let mut f = fixture();
    f.declarations[4] = aggregate(5, &[f.types.thread_marker], false);
    assert_eq!(f.observe(), Err(SemanticMirErrorV1::InvalidFunctionAbi));
}

#[test]
fn additional_statement_or_return_branch_cannot_be_hidden_by_ignore() {
    let mut f = fixture();
    f.bridge_with(vec![
        call(2, 1, f.types.unbranded_matrix),
        block(
            2,
            vec![SemanticStatementV1::new(
                SemanticSourceProvenanceV1::unavailable(),
                SemanticStatementKindV1::Nop,
            )],
            SemanticTerminatorKindV1::Return,
        ),
    ]);
    assert_eq!(f.observe(), Err(SemanticMirErrorV1::InvalidFunctionAbi));
    let mut f = fixture();
    f.bridge_with(vec![
        call(2, 1, f.types.unbranded_matrix),
        block(2, vec![], SemanticTerminatorKindV1::Return),
        block(3, vec![], SemanticTerminatorKindV1::Return),
    ]);
    assert_eq!(f.observe(), Err(SemanticMirErrorV1::InvalidFunctionAbi));
}

#[test]
fn unwind_or_call_result_retargeting_is_rejected() {
    for unwind in [
        SemanticUnwindActionV1::Continue,
        SemanticUnwindActionV1::Unreachable,
    ] {
        let mut f = fixture();
        let output = if unwind == SemanticUnwindActionV1::Unreachable {
            0
        } else {
            1
        };
        let entry = block(
            1,
            vec![],
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    SemanticCallableIdV1::from_index(2),
                    vec![],
                    Some(SemanticCallDestinationV1::new(
                        place(output, f.types.unbranded_matrix),
                        SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::CallReturn,
                            SemanticBlockIdV1::from_index(1),
                        ),
                    )),
                    unwind,
                )
                .unwrap(),
            ),
        );
        f.bridge_with(vec![
            entry,
            block(2, vec![], SemanticTerminatorKindV1::Return),
        ]);
        assert_eq!(f.observe(), Err(SemanticMirErrorV1::InvalidFunctionAbi));
    }
}

#[test]
fn source_abi_and_brand_substitution_change_record_identity() {
    let mut f = fixture();
    let record = f.observe().unwrap();
    f.functions[1] = function(
        31,
        abi(43, &[], f.types.matrix, false),
        &[
            (f.types.matrix, SemanticLocalRoleV1::Return),
            (f.types.unbranded_matrix, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            call(2, 1, f.types.unbranded_matrix),
            block(2, vec![], SemanticTerminatorKindV1::Return),
        ],
    );
    assert_ne!(f.observe().unwrap(), record);
    let mut f = fixture();
    f.brand = SemanticTypeIdentityV1::from_sha256([87; 32]);
    assert_ne!(f.observe().unwrap(), record);
}
