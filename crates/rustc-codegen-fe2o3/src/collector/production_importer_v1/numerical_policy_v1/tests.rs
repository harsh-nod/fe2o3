use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

fn identity(tag: u8) -> SemanticTypeIdentityV1 {
    SemanticTypeIdentityV1::from_sha256([tag; 32])
}

fn zst(tag: u8) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        identity(tag),
        SemanticLayoutIdentityV1::from_sha256([tag; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(0),
            1,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
    )
}

fn context_reference(
    kind: SemanticPointerKindV1,
    mutability: SemanticMutabilityV1,
) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        identity(2),
        SemanticLayoutIdentityV1::from_sha256([2; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                SemanticTypeIdV1::from_index(0),
                kind,
                mutability,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    )
}

fn direct() -> SemanticAbiPassModeV1 {
    SemanticAbiPassModeV1::Direct(
        SemanticAbiValueAttributesV1::new(
            SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
            SemanticAbiExtensionV1::None,
            0,
            None,
        )
        .unwrap(),
    )
}

fn abi_with(
    arguments: Vec<SemanticTypeIdV1>,
    output: SemanticTypeIdV1,
    output_mode: SemanticAbiPassModeV1,
    ownership: Vec<SemanticSourceArgumentOwnershipV1>,
) -> SemanticFunctionAbiV1 {
    SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1::from_sha256([40; 32]),
        SemanticLayoutIdentityV1::from_sha256([41; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        u32::try_from(arguments.len()).unwrap(),
        arguments.clone(),
        output,
        arguments
            .into_iter()
            .map(|ty| SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(ty, direct())))
            .collect(),
        SemanticAbiValueV1::new(output, output_mode),
    )
    .unwrap()
    .with_source_argument_ownership(ownership)
    .unwrap()
}

struct Fixture {
    types: Vec<SemanticTypeDeclV1>,
    abi: SemanticFunctionAbiV1,
    source: NumericalPolicySourceV1,
    provenance: SemanticKernelCapabilityProvenanceV1,
    source_identity: SemanticFunctionIdentityV1,
}

impl Fixture {
    fn new() -> Self {
        let source_identity = SemanticFunctionIdentityV1::from_sha256([30; 32]);
        Self {
            types: vec![
                zst(1),
                context_reference(
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                ),
                zst(3),
                zst(4),
            ],
            abi: abi_with(
                vec![SemanticTypeIdV1::from_index(1)],
                SemanticTypeIdV1::from_index(2),
                SemanticAbiPassModeV1::Ignore,
                vec![SemanticSourceArgumentOwnershipV1::SharedBorrow],
            ),
            source: NumericalPolicySourceV1 {
                terminal: Some(TrustedDeviceItem::NumericalPolicyIssue),
                policy_marker: Some(TrustedDeviceItem::StrictIeeeNumericalPolicy),
                source_identity,
                context_reference: identity(2),
                context: identity(1),
                capability: identity(3),
                policy: identity(14),
                context_axes: [identity(11), identity(12), identity(13)],
                capability_axes: [identity(11), identity(12), identity(13)],
                instance_arguments: [identity(11), identity(12), identity(13), identity(14)],
            },
            provenance: SemanticKernelCapabilityProvenanceV1::new(
                SemanticFunctionIdV1::from_index(0),
                SemanticKernelBindingIdentityV1::from_sha256([20; 32]),
                SemanticKernelCapabilityFrontendUnitIdentityV1::from_sha256([21; 32]),
                identity(11),
                SemanticKernelCapabilityTargetBrandIdentityV1::from_sha256([22; 32]),
                SemanticKernelCapabilityLaunchBrandIdentityV1::from_sha256([23; 32]),
                SemanticKernelCapabilityIssuanceIdentityV1::from_sha256([24; 32]),
            )
            .unwrap(),
            source_identity,
        }
    }

    fn expand(
        &self,
    ) -> Result<SemanticCompilerIntrinsicOperationV1, ProductionSemanticImportErrorV1> {
        expand_numerical_policy_issue_v1(
            &self.abi,
            &self.types,
            &self.source,
            self.provenance,
            self.source_identity,
        )
    }

    fn rejects(&self, detail: &str) {
        let error = self
            .expand()
            .expect_err("substituted numerical-policy terminal must fail");
        assert!(error.to_string().contains(detail), "{error}");
    }
}

#[test]
fn strict_policy_expands_to_typed_semantic_authority_with_exact_custody() {
    let fixture = Fixture::new();
    let SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract } =
        fixture.expand().unwrap()
    else {
        panic!("policy issuance must retain an execution capability operation");
    };
    assert_eq!(
        contract.operation(),
        SemanticExecutionCapabilityOperationV1::NumericalPolicyIssue {
            context: SemanticTypeIdV1::from_index(1),
            capability: SemanticTypeIdV1::from_index(2),
            policy: identity(14),
        },
    );
    assert_eq!(
        contract.signature().arguments().collect::<Vec<_>>(),
        fixture.abi.source_input_types()
    );
    assert_eq!(
        contract.signature().output(),
        fixture.abi.source_output_type()
    );
    assert_eq!(contract.provenance(), fixture.provenance);
    assert_eq!(contract.source_identity(), fixture.source_identity);
    assert_eq!(contract.workgroup_brand(), None);
    assert_eq!(contract.epoch_before(), None);
    assert_eq!(contract.epoch_after(), None);
    assert_eq!(
        contract.obligations().bits(),
        SemanticExecutionSafetyObligationsV1::TARGET_SUPPORT
            | SemanticExecutionSafetyObligationsV1::NUMERICAL_POLICY,
    );
}

#[test]
fn forged_source_and_wrong_trusted_terminal_are_rejected() {
    for terminal in [None, Some(TrustedDeviceItem::KernelContextIssue)] {
        let mut fixture = Fixture::new();
        fixture.source.terminal = terminal;
        fixture.rejects("source or strict policy identity");
    }
    for source in [[0; 32], [31; 32]] {
        let mut fixture = Fixture::new();
        fixture.source_identity = SemanticFunctionIdentityV1::from_sha256(source);
        fixture.rejects("source or strict policy identity");
    }
}

#[test]
fn forged_policy_marker_and_substituted_instance_policy_are_rejected() {
    for marker in [None, Some(TrustedDeviceItem::NumericalPolicyCapability)] {
        let mut fixture = Fixture::new();
        fixture.source.policy_marker = marker;
        fixture.rejects("source or strict policy identity");
    }
    let mut fixture = Fixture::new();
    fixture.source.policy = identity(15);
    fixture.rejects("kernel, target, launch, or policy substitution");
    fixture.source.policy = identity(0);
    fixture.rejects("source or strict policy identity");
}

#[test]
fn policy_marker_cannot_alias_context_or_issued_capability() {
    for policy in [identity(1), identity(2), identity(3)] {
        let mut fixture = Fixture::new();
        fixture.source.policy = policy;
        fixture.source.instance_arguments[3] = policy;
        fixture.rejects("source or strict policy identity");
    }
}

#[test]
fn kernel_target_and_launch_substitution_are_independently_rejected() {
    for axis in 0..3 {
        let mut fixture = Fixture::new();
        fixture.source.capability_axes[axis] = identity(50);
        fixture.rejects("kernel, target, launch, or policy substitution");

        let mut fixture = Fixture::new();
        fixture.source.instance_arguments[axis] = identity(50);
        fixture.rejects("kernel, target, launch, or policy substitution");
    }
    let mut fixture = Fixture::new();
    fixture.source.context_axes[0] = identity(50);
    fixture.source.capability_axes[0] = identity(50);
    fixture.source.instance_arguments[0] = identity(50);
    fixture.rejects("kernel, target, launch, or policy substitution");
}

#[test]
fn equal_layout_wrong_nominal_capability_and_context_are_rejected() {
    for tag in [0, 4] {
        let mut fixture = Fixture::new();
        fixture.source.capability = identity(tag);
        fixture.rejects("exact context or capability type/layout");
        let mut fixture = Fixture::new();
        fixture.source.context = identity(tag);
        fixture.rejects("exact context or capability type/layout");
    }
    let mut fixture = Fixture::new();
    fixture.abi = abi_with(
        vec![SemanticTypeIdV1::from_index(1)],
        SemanticTypeIdV1::from_index(3),
        SemanticAbiPassModeV1::Ignore,
        vec![SemanticSourceArgumentOwnershipV1::SharedBorrow],
    );
    fixture.rejects("exact context or capability type/layout");
}

#[test]
fn wrong_arity_and_nonignored_policy_return_are_rejected() {
    for count in [0, 2] {
        let mut fixture = Fixture::new();
        fixture.abi = abi_with(
            vec![SemanticTypeIdV1::from_index(1); count],
            SemanticTypeIdV1::from_index(2),
            SemanticAbiPassModeV1::Ignore,
            vec![SemanticSourceArgumentOwnershipV1::SharedBorrow; count],
        );
        fixture.rejects("semantic source arity");
    }
    let mut fixture = Fixture::new();
    fixture.abi = abi_with(
        vec![SemanticTypeIdV1::from_index(1)],
        SemanticTypeIdV1::from_index(2),
        direct(),
        vec![SemanticSourceArgumentOwnershipV1::SharedBorrow],
    );
    fixture.rejects("semantic FnAbi");
}

#[test]
fn raw_mutable_and_by_value_context_forgery_are_rejected() {
    for (kind, mutability) in [
        (SemanticPointerKindV1::Raw, SemanticMutabilityV1::Immutable),
        (
            SemanticPointerKindV1::Reference,
            SemanticMutabilityV1::Mutable,
        ),
    ] {
        let mut fixture = Fixture::new();
        fixture.types[1] = context_reference(kind, mutability);
        fixture.rejects("shared reference shape");
    }
    let mut fixture = Fixture::new();
    fixture.abi = abi_with(
        vec![SemanticTypeIdV1::from_index(1)],
        SemanticTypeIdV1::from_index(2),
        SemanticAbiPassModeV1::Ignore,
        vec![SemanticSourceArgumentOwnershipV1::ByValue],
    );
    fixture.rejects("semantic FnAbi");
}

#[test]
fn missing_duplicate_and_nonaggregate_authority_types_are_rejected() {
    let mut fixture = Fixture::new();
    fixture.types.truncate(2);
    fixture.rejects("exact context or capability type/layout");

    let mut fixture = Fixture::new();
    fixture.types.push(zst(3));
    fixture.rejects("exact context or capability type/layout");

    let mut fixture = Fixture::new();
    fixture.types[2] = SemanticTypeDeclV1::new(
        identity(3),
        SemanticLayoutIdentityV1::from_sha256([3; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(0),
            1,
            SemanticBackendReprV1::memory(true),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Opaque,
    );
    fixture.rejects("exact context or capability type/layout");
}
