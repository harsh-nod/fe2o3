use super::*;

pub(in crate::semantic_mir_v1) mod permutations;

pub(in crate::semantic_mir_v1) struct Fixture {
    pub functions: Vec<SemanticFunctionDeclV1>,
    pub callables: Vec<SemanticCallableDeclV1>,
    pub declarations: Vec<SemanticTypeDeclV1>,
    pub derive_types: SemanticKernelMathDeriveTypesV1,
    pub bind_types: SemanticPolicyMathBindTypesV1,
    pub provenance: SemanticKernelCapabilityProvenanceV1,
    pub policy: SemanticTypeIdentityV1,
    pub brand: SemanticTypeIdentityV1,
}

impl Fixture {
    pub fn derive(&self) -> Result<SemanticKernelMathDeriveV1, SemanticMirErrorV1> {
        SemanticKernelMathDeriveV1::for_defined_function(
            SemanticFunctionIdV1(0),
            &self.functions,
            &self.callables,
            &self.declarations,
            self.derive_types,
            self.provenance,
            self.brand,
        )
    }

    pub fn bind(&self) -> Result<SemanticPolicyMathBindV1, SemanticMirErrorV1> {
        SemanticPolicyMathBindV1::for_defined_function(
            SemanticFunctionIdV1(2),
            &self.functions,
            &self.callables,
            &self.declarations,
            self.bind_types,
            self.provenance,
            self.policy,
            self.brand,
        )
    }
}

fn attributes() -> SemanticAbiValueAttributesV1 {
    SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, true, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap()
}

fn abi(
    tag: u8,
    inputs: &[SemanticTypeIdV1],
    output: SemanticTypeIdV1,
    pair: bool,
) -> SemanticFunctionAbiV1 {
    SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1([tag; 32]),
        SemanticLayoutIdentityV1([tag; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        inputs.len() as u32,
        inputs.to_vec(),
        output,
        inputs
            .iter()
            .map(|ty| {
                SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                    *ty,
                    SemanticAbiPassModeV1::Direct(attributes()),
                ))
            })
            .collect(),
        SemanticAbiValueV1::new(
            output,
            if pair {
                SemanticAbiPassModeV1::Pair {
                    first: attributes(),
                    second: attributes(),
                }
            } else {
                SemanticAbiPassModeV1::Ignore
            },
        ),
    )
    .unwrap()
    .with_source_argument_ownership(vec![
        SemanticSourceArgumentOwnershipV1::SharedBorrow;
        inputs.len()
    ])
    .unwrap()
}

fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1(local), vec![], ty).unwrap()
}

fn block(
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    kind: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1([tag; 32]),
        source,
        statements,
        SemanticTerminatorV1::new(source, kind),
    )
    .unwrap()
}

fn call(callee: u32, output_local: u32, output: SemanticTypeIdV1) -> SemanticBasicBlockV1 {
    block(
        1,
        vec![],
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1(callee),
                vec![],
                Some(SemanticCallDestinationV1::new(
                    place(output_local, output),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1(1),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        ),
    )
}

fn function(
    tag: u8,
    abi: SemanticFunctionAbiV1,
    locals: &[(SemanticTypeIdV1, SemanticLocalRoleV1)],
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1([tag; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1([tag; 32]),
        SemanticMonomorphizationIdentityV1([tag; 32]),
        SemanticGenericTypeArgumentsIdentityV1([tag; 32]),
        SemanticConstGenericArgumentsIdentityV1([tag; 32]),
        source,
        abi,
        locals
            .iter()
            .enumerate()
            .map(|(index, (ty, role))| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1([index as u8 + 1; 32]),
                    *ty,
                    *role,
                    source,
                )
            })
            .collect(),
        SemanticBlockIdV1(0),
        blocks,
    )
    .unwrap()
}

fn aggregate(tag: u8, fields: &[SemanticTypeIdV1], pair: bool) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1([tag; 32]),
        SemanticLayoutIdentityV1([tag; 32]),
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(if pair { 16 } else { 0 }),
            if pair { 8 } else { 1 },
            if pair {
                SemanticBackendReprV1::ScalarPair {
                    first: pointer_scalar(),
                    second: pointer_scalar(),
                }
            } else {
                SemanticBackendReprV1::memory(true)
            },
            false,
            SemanticAggregateLayoutV1::new(
                if pair {
                    vec![0, 8, 16]
                } else {
                    vec![0; fields.len()]
                },
                vec![],
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(fields.to_vec()).unwrap()),
    )
}

fn pointer_scalar() -> SemanticBackendScalarV1 {
    SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
    )
}

fn reference(tag: u8, pointee: SemanticTypeIdV1) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1([tag; 32]),
        SemanticLayoutIdentityV1([tag; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(pointer_scalar()),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                pointee,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    )
}

pub(in crate::semantic_mir_v1) fn fixture() -> Fixture {
    let derive_types = SemanticKernelMathDeriveTypesV1::new([1, 0, 2, 3].map(SemanticTypeIdV1));
    let bind_types = SemanticPolicyMathBindTypesV1::new([4, 2, 6, 5, 7].map(SemanticTypeIdV1));
    let marker = SemanticTypeIdV1(8);
    let getter = function(
        30,
        abi(
            40,
            &[derive_types.context_reference],
            derive_types.math,
            false,
        ),
        &[
            (derive_types.math, SemanticLocalRoleV1::Return),
            (
                derive_types.context_reference,
                SemanticLocalRoleV1::Argument(0),
            ),
        ],
        vec![
            call(1, 0, derive_types.math),
            block(2, vec![], SemanticTerminatorKindV1::Return),
        ],
    );
    let bridge = function(
        31,
        abi(41, &[], derive_types.math, false),
        &[
            (derive_types.math, SemanticLocalRoleV1::Return),
            (derive_types.unbranded_math, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            call(3, 1, derive_types.unbranded_math),
            block(2, vec![], SemanticTerminatorKindV1::Return),
        ],
    );
    let binding = function(
        32,
        abi(
            42,
            &[bind_types.math_reference, bind_types.policy_reference],
            bind_types.bound,
            true,
        ),
        &[
            (bind_types.bound, SemanticLocalRoleV1::Return),
            (bind_types.math_reference, SemanticLocalRoleV1::Argument(0)),
            (
                bind_types.policy_reference,
                SemanticLocalRoleV1::Argument(1),
            ),
        ],
        vec![block(
            1,
            vec![SemanticStatementV1::new(
                SemanticSourceProvenanceV1::unavailable(),
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    place(0, bind_types.bound),
                    SemanticRvalueV1::new(
                        bind_types.bound,
                        SemanticRvalueKindV1::aggregate(
                            SemanticAggregateKindV1::Aggregate,
                            vec![
                                SemanticOperandV1::Copy(place(1, bind_types.math_reference)),
                                SemanticOperandV1::Copy(place(2, bind_types.policy_reference)),
                                SemanticOperandV1::Constant(SemanticConstantV1::new(
                                    marker,
                                    SemanticConstantValueV1::ZeroSized,
                                )),
                            ],
                        )
                        .unwrap(),
                    ),
                )),
            )],
            SemanticTerminatorKindV1::Return,
        )],
    );
    let functions = vec![getter, bridge, binding];
    let mut callables = (0..3)
        .map(|id| SemanticCallableDeclV1::defined(SemanticFunctionIdV1(id)))
        .collect::<Vec<_>>();
    callables.push(SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1([33; 32]),
            SemanticItemDefinitionIdentityV1([33; 32]),
            SemanticMonomorphizationIdentityV1([33; 32]),
            SemanticGenericTypeArgumentsIdentityV1([33; 32]),
            SemanticConstGenericArgumentsIdentityV1([33; 32]),
            SemanticSourceProvenanceV1::unavailable(),
            abi(43, &[], derive_types.unbranded_math, false),
        ),
        operation: SemanticCompilerIntrinsicOperationV1::MathContextCurrent {
            context: derive_types.unbranded_math,
        },
        operation_identity: SemanticCompilerIntrinsicIdentityV1([33; 32]),
    });
    Fixture {
        functions,
        callables,
        declarations: vec![
            aggregate(10, &[], false),
            reference(11, derive_types.context),
            aggregate(12, &[], false),
            aggregate(13, &[], false),
            reference(14, bind_types.math),
            aggregate(15, &[], false),
            reference(16, bind_types.capability),
            aggregate(
                17,
                &[
                    bind_types.math_reference,
                    bind_types.policy_reference,
                    marker,
                ],
                true,
            ),
            aggregate(18, &[], false),
        ],
        derive_types,
        bind_types,
        provenance: SemanticKernelCapabilityProvenanceV1::new(
            SemanticFunctionIdV1(3),
            SemanticKernelBindingIdentityV1([100; 32]),
            SemanticKernelCapabilityFrontendUnitIdentityV1([101; 32]),
            SemanticTypeIdentityV1([102; 32]),
            SemanticKernelCapabilityTargetBrandIdentityV1([103; 32]),
            SemanticKernelCapabilityLaunchBrandIdentityV1([104; 32]),
            SemanticKernelCapabilityIssuanceIdentityV1([105; 32]),
        )
        .unwrap(),
        policy: SemanticTypeIdentityV1([106; 32]),
        brand: SemanticTypeIdentityV1([107; 32]),
    }
}

#[test]
fn defined_math_getter_binds_original_getter_bridge_and_current() {
    let f = fixture();
    let record = f.derive().unwrap();
    assert_eq!(record.function(), SemanticFunctionIdV1(0));
    assert_eq!(record.source_identity(), f.functions[0].identity());
    assert_eq!(record.abi_identity(), f.functions[0].abi().identity());
    assert_eq!(record.bridge().function(), SemanticFunctionIdV1(1));
    assert_eq!(record.bridge().source_identity(), f.functions[1].identity());
    assert_eq!(record.current_callable(), SemanticCallableIdV1(3));
    assert_eq!(
        record.current_source_identity(),
        f.callables[3].binding().unwrap().identity()
    );
    assert_eq!(record.types(), f.derive_types);
    assert_eq!(record.receiver_argument(), 0);
    assert_eq!(record.provenance(), f.provenance);
    assert_eq!(record.kernel_brand(), f.brand);
    validate_math_derive_attachment(&f.functions[0], record).unwrap();
}

#[test]
fn defined_math_bind_retains_both_reference_edges() {
    let f = fixture();
    let record = f.bind().unwrap();
    assert_eq!(record.function(), SemanticFunctionIdV1(2));
    assert_eq!(record.reference_arguments(), [0, 1]);
    assert_eq!(record.reference_fields(), [0, 1]);
    assert_eq!(record.types(), f.bind_types);
    assert_eq!(record.policy(), f.policy);
    assert_eq!(record.kernel_brand(), f.brand);
    validate_math_bind_attachment(&f.functions[2], record).unwrap();
}

#[test]
fn defined_math_rejects_substituted_or_erased_callees() {
    for callee in [0, 2, 3, u32::MAX] {
        let mut f = fixture();
        f.functions[0].blocks[0] = call(callee, 0, f.derive_types.math);
        assert!(f.derive().is_err(), "getter callee {callee}");
    }
    let mut f = fixture();
    f.functions[1].blocks[0] = block(1, vec![], SemanticTerminatorKindV1::Return);
    assert!(f.derive().is_err());
    let mut f = fixture();
    let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = &mut f.callables[3] else {
        panic!()
    };
    *operation = SemanticCompilerIntrinsicOperationV1::MathContextCurrent {
        context: f.derive_types.math,
    };
    assert!(
        f.derive().is_err(),
        "unbranded leaf cannot issue branded Math"
    );
}

#[test]
fn defined_math_rejects_moves_swapped_fields_and_wrong_marker() {
    for mutation in 0..4 {
        let mut f = fixture();
        let SemanticStatementKindV1::Assign(assignment) =
            &mut f.functions[2].blocks[0].statements[0].kind
        else {
            panic!()
        };
        let SemanticRvalueKindV1::Aggregate(aggregate) = &mut assignment.value.kind else {
            panic!()
        };
        match mutation {
            0 => aggregate.operands.swap(0, 1),
            1 => {
                aggregate.operands[0] =
                    SemanticOperandV1::Move(place(1, f.bind_types.math_reference))
            }
            2 => {
                aggregate.operands[2] = SemanticOperandV1::Constant(SemanticConstantV1::new(
                    f.bind_types.math,
                    SemanticConstantValueV1::ZeroSized,
                ))
            }
            3 => aggregate.kind = SemanticAggregateKindV1::Tuple,
            _ => unreachable!(),
        }
        assert!(f.bind().is_err(), "Bind mutation {mutation}");
    }
}

#[test]
fn defined_math_rejects_abi_and_reference_substitution() {
    let mut f = fixture();
    f.functions[0].abi.source_argument_ownership[0] =
        SemanticSourceArgumentOwnershipV1::Unspecified;
    assert!(f.derive().is_err());
    let mut f = fixture();
    f.functions[2].abi.return_value =
        SemanticAbiValueV1::new(f.bind_types.bound, SemanticAbiPassModeV1::Ignore);
    assert!(f.bind().is_err());
    let mut f = fixture();
    let id = f.bind_types.policy_reference.index() as usize;
    f.declarations[id].shape = SemanticTypeShapeV1::Pointer(
        SemanticPointerTypeV1::new_with_kind(
            f.bind_types.capability,
            SemanticPointerKindV1::Raw,
            SemanticMutabilityV1::Immutable,
            0,
            64,
            SemanticPointerMetadataV1::None,
        )
        .unwrap(),
    );
    assert!(f.bind().is_err());
}

#[test]
fn defined_math_commitments_detect_same_shape_body_and_abi_changes() {
    let mut f = fixture();
    let derive = f.derive().unwrap();
    let bind = f.bind().unwrap();
    f.functions[0].locals[0].identity = SemanticLocalIdentityV1([200; 32]);
    assert!(validate_math_derive_attachment(&f.functions[0], derive).is_err());
    f.functions[2].abi.identity = SemanticAbiIdentityV1([201; 32]);
    assert!(validate_math_bind_attachment(&f.functions[2], bind).is_err());
    let mut f = fixture();
    f.functions[1].locals[0].identity = SemanticLocalIdentityV1([202; 32]);
    assert_ne!(
        derive,
        f.derive().unwrap(),
        "the bridge body is committed independently"
    );
}

#[test]
fn defined_math_rejects_nominal_alias_and_missing_identity() {
    let mut f = fixture();
    f.declarations[3].identity = f.declarations[2].identity;
    assert!(f.derive().is_err());
    let mut f = fixture();
    f.brand = f.policy;
    assert!(f.bind().is_err());
    let mut f = fixture();
    f.functions[0].identity = SemanticFunctionIdentityV1([0; 32]);
    assert!(f.derive().is_err());
    let f = fixture();
    let mut record = f.derive().unwrap();
    record.origin.body_identity = [0; 32];
    assert!(
        SemanticDefinedMathBodyV1::from_encoded_parts(
            record.function(),
            record.source_identity(),
            record.abi_identity(),
            *record.body_identity()
        )
        .is_err()
    );
}

#[test]
fn defined_math_body_hashing_is_bounded_and_deterministic() {
    let f = fixture();
    assert_eq!(f.derive(), f.derive());
    assert_eq!(f.bind(), f.bind());
    let mut budget = 1;
    assert!(matches!(
        SemanticKernelMathDeriveV1::observe(
            SemanticFunctionIdV1(0),
            &f.functions,
            &f.callables,
            &f.declarations,
            f.derive_types,
            f.provenance,
            f.brand,
            &mut budget
        ),
        Err(SemanticMirErrorV1::LimitExceeded {
            resource: SemanticMirResourceV1::CanonicalBytes,
            ..
        })
    ));
}

#[test]
fn defined_math_payloads_preserve_exact_fixed_fields() {
    let f = fixture();
    let derive = f.derive().unwrap();
    let bind = f.bind().unwrap();
    let mut writer = CanonicalWriterV1::new(512);
    derive.encode_payload(&mut writer).unwrap();
    let bytes = writer.finish();
    assert_eq!(bytes.len(), 512);
    assert_eq!(&bytes[..4], &0u32.to_le_bytes());
    assert_eq!(&bytes[4..36], derive.source_identity().as_bytes());
    assert_eq!(&bytes[36..68], derive.abi_identity().as_bytes());
    assert_eq!(&bytes[68..100], derive.body_identity());
    assert_eq!(&bytes[100..104], &1u32.to_le_bytes());
    assert_eq!(&bytes[200..204], &3u32.to_le_bytes());
    assert_eq!(&bytes[480..], f.brand.as_bytes());
    let mut writer = CanonicalWriterV1::new(380);
    bind.encode_payload(&mut writer).unwrap();
    let bytes = writer.finish();
    assert_eq!(bytes.len(), 380);
    assert_eq!(&bytes[316..348], f.policy.as_bytes());
    assert_eq!(&bytes[348..], f.brand.as_bytes());
    assert!(
        derive
            .encode_payload(&mut CanonicalWriterV1::new(511))
            .is_err()
    );
    assert!(
        bind.encode_payload(&mut CanonicalWriterV1::new(379))
            .is_err()
    );
}

fn rooted_fixture() -> Fixture {
    let mut f = fixture();
    let output = SemanticTypeIdV1(8);
    let mut root = function(
        34,
        abi(44, &[], output, false),
        &[(output, SemanticLocalRoleV1::Return)],
        vec![block(1, vec![], SemanticTerminatorKindV1::Return)],
    );
    root.role = SemanticFunctionRoleV1::KernelRoot;
    root = root.with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"defined_math_test".to_vec()).unwrap(),
        f.provenance.kernel_binding(),
        SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
    ));
    f.functions.push(root);
    f.callables
        .insert(3, SemanticCallableDeclV1::defined(SemanticFunctionIdV1(3)));
    f.functions[1].blocks[0] = call(4, 1, f.derive_types.unbranded_math);
    f
}

fn request(f: Fixture) -> InertSemanticMirRequestV1 {
    InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1([220; 32])),
        f.declarations,
        vec![],
        vec![],
        vec![],
        f.functions,
        f.callables,
        vec![SemanticFunctionIdV1(3)],
    )
    .unwrap()
}

fn context(
    request: &InertSemanticMirRequestV1,
    limits: SemanticMirLimitsV1,
) -> ValidationContextV1<'_> {
    ValidationContextV1 {
        request,
        limits,
        totals: ValidationTotalsV1::default(),
        work: 0,
    }
}

#[test]
fn defined_math_roster_validation_rechecks_root_and_bridge() {
    let f = rooted_fixture();
    let derive = f.derive().unwrap();
    let bind = f.bind().unwrap();
    let mut request = request(f);
    let limits = SemanticMirLimitsV1::default();
    validate_math_derive(
        &mut context(&request, limits),
        SemanticFunctionIdV1(0),
        derive,
    )
    .unwrap();
    validate_math_bind(
        &mut context(&request, limits),
        SemanticFunctionIdV1(2),
        bind,
    )
    .unwrap();
    request.functions[1].locals[0].identity = SemanticLocalIdentityV1([222; 32]);
    assert!(
        validate_math_derive(
            &mut context(&request, limits),
            SemanticFunctionIdV1(0),
            derive
        )
        .is_err()
    );
    request.functions[3].role = SemanticFunctionRoleV1::InternalHelper;
    assert!(
        validate_math_bind(
            &mut context(&request, limits),
            SemanticFunctionIdV1(2),
            bind
        )
        .is_err()
    );
    assert!(
        validate_math_derive(
            &mut context(&request, limits),
            SemanticFunctionIdV1(0),
            derive
        )
        .is_err()
    );
}

#[test]
fn defined_math_roster_validation_charges_hash_work() {
    let f = rooted_fixture();
    let derive = f.derive().unwrap();
    let bind = f.bind().unwrap();
    let request = request(f);
    let limits = SemanticMirLimitsV1::default()
        .with_limit(SemanticMirResourceV1::ValidationWork, 32)
        .unwrap();
    for error in [
        validate_math_derive(
            &mut context(&request, limits),
            SemanticFunctionIdV1(0),
            derive,
        )
        .unwrap_err(),
        validate_math_bind(
            &mut context(&request, limits),
            SemanticFunctionIdV1(2),
            bind,
        )
        .unwrap_err(),
    ] {
        assert!(matches!(
            error,
            SemanticMirErrorV1::LimitExceeded {
                resource: SemanticMirResourceV1::ValidationWork,
                ..
            }
        ));
    }
}
