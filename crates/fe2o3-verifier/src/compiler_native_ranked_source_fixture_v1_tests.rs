//! CPU consistency coverage using public test keys, never admitted execution.
use super::*;
use crate::ProductionMirPlironVerusExecutionClaimsV1 as Claims;
use ed25519_dalek::{Signer as _, SigningKey};
use fe2o3_compiler_lineage::{
    InertNativeNeutralSubjectV1, MultiRootProofRosterInputsV3, MultiRootProofRosterRootInputV3,
    encode_native_neutral_module_v1,
};
use fe2o3_functional_proof::{
    FunctionalRefinementBindingV2, FunctionalRefinementResultV2, FunctionalRefinementSubjectsV2,
    SafeReferenceKindV2, UnsignedFunctionalRefinementReceiptV2,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkBudgetV1, InertCanonicalKernelIrContractCatalogV1 as Catalog,
};
use fe2o3_lower_mir_kernel::{
    ProductionPreRankedKirOwnerV1, ProductionRankedAccessSourceV1, ProductionSemanticKirLimitsV1,
    ProductionSourceLaunchInputV1, ProductionSourceLaunchRosterV1,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use fe2o3_pliron::{
    ProductionEffectRefinementContractV2, ProductionGpuWriteSiteV2, ProductionNumericalContractV2,
    ProductionRankedBlockV1, ProductionRankedKernelV1, ProductionRankedTerminatorV1,
    ProductionRankedValueIdV1, ProductionRankedValueV1, ProductionReferenceOutputSiteV2,
    ProductionReferenceProofV2, ProductionSemanticExpressionV2, ProductionSemanticMirLimitsV1,
    ProductionSemanticMirOwnerV1, ProductionSemanticScalarTypeV2, ProductionSemanticSsaLimitsV1,
    ProductionSemanticSsaOwnerV1, normalized_effect_refinement_hash_for_kernel_v2,
};

const WORK: usize = 100_000_000;
const STORAGE: usize = 512 * 1024 * 1024;
const NAME: &str = "native_single_store";
const TEXT: &str = "typed single-store recipe; diagnostic text is not a proof\n";
const BINDING: [u8; 32] = [33; 32];

fn d(byte: u8) -> DigestV1 {
    DigestV1::from_untrusted_bytes([byte; 32])
}

fn source() -> ProductionPreRankedKirOwnerV1 {
    let unit = SemanticTypeIdV1::from_index(0);
    let u32_ty = SemanticTypeIdV1::from_index(1);
    let pointer = SemanticTypeIdV1::from_index(2);
    let carrier = SemanticTypeIdV1::from_index(3);
    let pointer_backend = SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(1, 8, 8),
        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
    ));
    let properties = SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
        Some(SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap()),
        None,
    );
    let types = vec![
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([1; 32]),
            SemanticLayoutIdentityV1::from_sha256([1; 32]),
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                0,
                1,
                SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                1,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Unit,
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([2; 32]),
            SemanticLayoutIdentityV1::from_sha256([2; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(4),
                4,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, 32, 4),
                    SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            }),
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([3; 32]),
            SemanticLayoutIdentityV1::from_sha256([3; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(Some(8), 8, pointer_backend, false)
                .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    u32_ty,
                    SemanticPointerKindV1::Raw,
                    SemanticMutabilityV1::Mutable,
                    1,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        )
        .with_rustc_abi_properties(properties),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([4; 32]),
            SemanticLayoutIdentityV1::from_sha256([4; 32]),
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(8),
                8,
                pointer_backend,
                false,
                SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![pointer]).unwrap()),
        )
        .with_rustc_abi_properties(properties),
    ];
    let layout = SemanticLayoutIdentityV1::from_sha256([250; 32]);
    let attrs = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([20; 32]),
        layout,
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            carrier,
            SemanticAbiPassModeV1::Direct(attrs),
        ))],
        SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ExclusiveOwner])
    .unwrap();
    let provenance = SemanticSourceProvenanceV1::unavailable();
    let pointer_assignment = SemanticStatementV1::new(
        provenance,
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(2), vec![], pointer).unwrap(),
            SemanticRvalueV1::new(
                pointer,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                    SemanticPlaceV1::new(
                        SemanticLocalIdV1::from_index(1),
                        vec![
                            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), pointer)
                                .unwrap(),
                        ],
                        pointer,
                    )
                    .unwrap(),
                )),
            ),
        )),
    );
    let store = SemanticStatementV1::new(
        provenance,
        SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(2),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, u32_ty)
                        .unwrap(),
                ],
                u32_ty,
            )
            .unwrap(),
            SemanticOperandV1::Constant(SemanticConstantV1::new(
                u32_ty,
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(7, 4).unwrap()),
            )),
            SemanticVolatilityV1::NonVolatile,
            None,
        )),
    );
    let dimensions = SemanticWorkgroupDimensionsV1::new([1, 1, 1]).unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([21; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([22; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([23; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([24; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([25; 32]),
        provenance,
        abi,
        vec![
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([26; 32]),
                unit,
                SemanticLocalRoleV1::Return,
                provenance,
            ),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([27; 32]),
                carrier,
                SemanticLocalRoleV1::Argument(0),
                provenance,
            ),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([29; 32]),
                pointer,
                SemanticLocalRoleV1::Temporary,
                provenance,
            ),
        ],
        SemanticBlockIdV1::from_index(0),
        vec![
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([28; 32]),
                provenance,
                vec![pointer_assignment, store],
                SemanticTerminatorV1::new(provenance, SemanticTerminatorKindV1::Return),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(NAME.as_bytes().to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(BINDING),
        SemanticKernelSourceContractV1::new(
            Some(
                SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None)
                    .unwrap(),
            ),
            None,
            None,
        )
        .unwrap(),
    ));
    let semantic = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(layout),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let inputs = [ProductionSourceLaunchRootInputV1::new(
        NAME,
        BINDING,
        ProductionSourceLaunchInputV1::new(1, Some([1, 1, 1]), [1, 1, 1]),
    )];
    let launch = ProductionSourceLaunchRosterV1::try_new(&semantic, &inputs).unwrap();
    let semantic =
        ProductionSemanticMirOwnerV1::try_new(semantic, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(semantic, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap()
}

fn sign(
    binding: FunctionalRefinementBindingV2,
    boundary: FunctionalRefinementBoundaryV2,
    toolchain: VerusToolchainIdentityV2,
) -> (
    ImportedFunctionalRefinementProofV2,
    InertFunctionalRefinementReceiptSignatureV2,
) {
    let key = SigningKey::from_bytes(&[71; 32]);
    let policy = FunctionalRefinementImportPolicyV2::new(
        key.verifying_key().to_bytes(),
        toolchain,
        boundary,
    )
    .unwrap();
    let unsigned = UnsignedFunctionalRefinementReceiptV2::from_verified_execution_join(
        policy.signer_identity(),
        binding,
        toolchain,
        d(77),
        FunctionalRefinementResultV2::Proved,
        boundary,
    )
    .unwrap();
    let signature = key.sign(unsigned.signing_bytes()).to_bytes();
    let wire = unsigned.attach_signature(signature);
    let imported = FunctionalRefinementReceiptImporterV2::new(policy, 1)
        .unwrap()
        .import(FunctionalRefinementImportExpectationV2::new(binding), &wire)
        .unwrap();
    (
        imported,
        InertFunctionalRefinementReceiptSignatureV2::from_untrusted_parts(
            wire,
            key.verifying_key().to_bytes(),
        ),
    )
}

fn ranked(
    source: &ProductionPreRankedKirOwnerV1,
    toolchain: VerusToolchainIdentityV2,
) -> (
    ProductionRankedKernelLoweringInputV1,
    InertFunctionalRefinementReceiptSignatureV2,
    NativeCompilerStagingCommitmentV1,
) {
    let layout = source.source_launch().roots()[0].layout();
    let local = |n| ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(n));
    let exact = ProductionNumericalContractV2::ExactBitVectorOperatorCongruence;
    let subjects = FunctionalRefinementSubjectsV2::new(
        SafeReferenceKindV2::Mir,
        d(61),
        DigestV1::ZERO,
        d(62),
        d(63),
        DigestV1::from_untrusted_bytes(
            *source
                .semantic_ssa()
                .source_semantic()
                .semantic_sha256()
                .as_bytes(),
        ),
    )
    .unwrap();
    let contract = ProductionEffectRefinementContractV2::new(
        73,
        ProductionGpuWriteSiteV2::new(0, 8),
        ProductionReferenceOutputSiteV2::new(0, 0, 0),
        local(0),
        vec![local(1)],
        vec![local(5)],
        vec![local(5)],
        local(4),
        local(4),
        local(4),
        local(4),
        local(2),
        local(3),
    )
    .unwrap();
    let scalar = |result, bits, ty| ProductionRankedOperationV1::SemanticExpression {
        result: ProductionRankedValueIdV1::new(result),
        expression: ProductionSemanticExpressionV2::Constant { scalar: ty, bits },
        numerical_contract: exact,
    };
    let u32_ty = ProductionSemanticScalarTypeV2::Integer {
        signed: false,
        bits: 32,
    };
    let kernel = ProductionRankedKernelV1::new(
        NAME,
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::ExecutionLayout {
                    grid_identity: layout.grid_identity(),
                    global_extents: layout.global_extents(),
                    workgroup_extents: layout.workgroup_extents(),
                    subgroup_size: layout.subgroup_size(),
                    full_physical_workgroups: layout.full_physical_workgroups(),
                },
                ProductionRankedOperationV1::View {
                    result: ProductionRankedValueIdV1::new(0),
                    element_width: 32,
                    writable: true,
                    shape: vec![1],
                    dynamic_extents: vec![],
                    allocation_origin: 1,
                    noalias_class: 1,
                },
                ProductionRankedOperationV1::IndexConstant {
                    result: ProductionRankedValueIdV1::new(1),
                    value: 0,
                },
                scalar(2, 7, u32_ty),
                scalar(3, 7, u32_ty),
                scalar(4, 1, ProductionSemanticScalarTypeV2::Bool),
                scalar(
                    5,
                    0,
                    ProductionSemanticScalarTypeV2::Integer {
                        signed: false,
                        bits: 64,
                    },
                ),
                ProductionRankedOperationV1::OwnershipContract {
                    view: local(0),
                    coverage: dialect_kernel::OwnershipCoverageAttr::TotalView,
                    partition: dialect_kernel::OwnershipPartitionAttr::ExactSets,
                },
                ProductionRankedOperationV1::ValueAccess {
                    kind: dialect_kernel::AccessKindAttr::Write,
                    view: local(0),
                    indices: vec![local(1)],
                    value: local(2),
                },
                ProductionRankedOperationV1::RequestEffectRefinement { contract, subjects },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    let ProductionRankedOperationV1::RequestEffectRefinement { contract, .. } =
        &kernel.blocks()[0].operations()[9]
    else {
        unreachable!()
    };
    let obligation =
        normalized_effect_refinement_hash_for_kernel_v2(&kernel, 0, 9, contract, subjects).unwrap();
    let binding = FunctionalRefinementBindingV2::from_subjects(subjects, obligation).unwrap();
    let (imported, signature) = sign(
        binding,
        FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
        toolchain,
    );
    let staging = commitment(&imported);
    let kernel = kernel
        .bind_functional_refinement_request_v2(
            0,
            9,
            ProductionReferenceProofV2::request_exact(imported.receipt_identity(), binding),
        )
        .unwrap();
    let policy =
        ProductionRefinementStagingPolicyV2::new(vec![imported.signer_identity()], toolchain)
            .unwrap();
    let lowering = compile_ranked_kernel_with_policy_checked_refinement_staging_v2(
        ProductionConstructionV1::ranked_kernel("full_native_fixture", kernel).unwrap(),
        ProductionSessionLimitsV1::default(),
        vec![imported],
        policy,
    )
    .unwrap();
    (lowering, signature, staging)
}

struct Fixture {
    semantic: Vec<u8>,
    native: Vec<u8>,
    native_graph: Vec<u8>,
    middle: Roster,
    correspondence: Roster,
    verus: Roster,
    kernel: ProductionRankedKernelV1,
    signature: InertFunctionalRefinementReceiptSignatureV2,
    staging: NativeCompilerStagingCommitmentV1,
}

fn fixture() -> Fixture {
    fixture_from_source(&source())
}

fn fixture_from_source(source: &ProductionPreRankedKirOwnerV1) -> Fixture {
    let semantic = source.semantic_ssa().source_semantic();
    let toolchain = VerusToolchainIdentityV2::new(d(72), d(73), d(74), d(75), d(76)).unwrap();
    let (ranked, signature, staging) = ranked(source, toolchain);
    let evidence =
        ProductionMiddleEndEvidenceV5::try_new(source.semantic_ssa().source_owner(), &ranked, TEXT)
            .unwrap();
    let prepared =
        crate::mir_pliron_per_compilation_verus_v1::rederive_mir_pliron_aggregate_preparation_v1(
            &ranked, &evidence,
        )
        .unwrap();
    let (aggregate, aggregate_signature) = sign(
        prepared.binding,
        FunctionalRefinementBoundaryV2::SafeReferenceMirToLivePliron,
        toolchain,
    );
    let claims = Claims::new(
        prepared.contract_identity,
        prepared.parallel_contract_identity,
        prepared.pliron_evidence_identity,
        prepared.composition_template_identity,
        prepared.generated_source_identity,
        prepared.binding,
        aggregate.signer_identity(),
        toolchain,
        aggregate.execution_identity(),
        aggregate.receipt_identity().digest(),
        prepared.retained_count,
    )
    .unwrap();
    let signed = Signed::new(
        claims,
        *aggregate_signature.verifying_key(),
        *aggregate_signature.wire(),
    )
    .unwrap();
    let root = semantic.roots()[0];
    let function = &semantic.functions()[root.index() as usize];
    let induction = Induction::from_report(
        &analyze_semantic_u32_induction_no_overflow_v1(
            semantic,
            semantic
                .select_kernel_body_for_root_v1(root)
                .unwrap()
                .body(),
        )
        .unwrap(),
    )
    .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget
        .reserve_storage(source.retained_analysis_storage_v1())
        .unwrap();
    let (catalog, storage) = Catalog::from_rows_with_budget(
        *semantic.semantic_sha256().as_bytes(),
        &[],
        &[],
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let graph = source.executable().canonical();
    let subject = InertNativeNeutralSubjectV1::new(
        *graph.identity().digest(),
        graph.canonical_bytes().len() as u64,
        *catalog.digest(),
        catalog.canonical_bytes().len() as u64,
    )
    .unwrap();
    let native = encode_native_neutral_module_v1(
        &subject,
        graph.canonical_bytes(),
        catalog.canonical_bytes(),
    )
    .unwrap();
    let mut identity = Sha256::new();
    identity.update(b"FE2O3/PRODUCTION-RANKED-KERNEL-ROSTER-IDENTITY/V1\0");
    identity.update(1_u64.to_le_bytes());
    for field in [
        &BINDING[..],
        NAME.as_bytes(),
        NAME.as_bytes(),
        &root.index().to_le_bytes(),
        &function.identity().as_bytes()[..],
        &[1],
        evidence.identity().sha256(),
        &evidence.identity().byte_len().to_le_bytes(),
        induction.semantic_mir_sha256(),
        &induction.function().to_le_bytes(),
        induction.function_identity(),
        &u64::from(induction.checked_additions_examined()).to_le_bytes(),
        &(induction.certificates().len() as u64).to_le_bytes(),
        &induction.work_units().to_le_bytes(),
    ] {
        identity.update((field.len() as u64).to_le_bytes());
        identity.update(field);
    }
    let identity = identity.finalize().into();
    let frame = |kind, payload| {
        Roster::new(MultiRootProofRosterInputsV3 {
            kind,
            semantic_mir_sha256: *semantic.semantic_sha256().as_bytes(),
            native_neutral_subject: subject,
            roster_identity: identity,
            canonical_kernel_order: &[0],
            roots: &[MultiRootProofRosterRootInputV3 {
                semantic_root: root.index(),
                semantic_root_identity: *function.identity().as_bytes(),
                kernel_binding: BINDING,
                source_rank: 1,
                workgroup: [1, 1, 1],
                logical_name: NAME,
                export_symbol: NAME,
                kernel_id: NAME,
                payload,
            }],
        })
        .unwrap()
    };
    Fixture {
        semantic: semantic.canonical_encoding().to_vec(),
        native,
        native_graph: graph.canonical_bytes().to_vec(),
        middle: frame(Kind::MiddleEnd, evidence.as_inert().canonical_bytes()),
        correspondence: frame(Kind::Correspondence, induction.canonical_bytes()),
        verus: frame(Kind::VerusExecution, signed.canonical_bytes()),
        kernel: ranked.kernel().clone(),
        signature,
        staging,
    }
}

impl Fixture {
    fn validate(
        &self,
        kernel: &ProductionRankedKernelV1,
        access_sources: &[ProductionRankedAccessSourceV1],
        budget: &mut Budget<'_>,
    ) -> Result<
        (
            ValidatedNativeCompilerRankedSourceProofV1,
            NativeCompilerRankedSourceProofStorageV1,
        ),
        E,
    > {
        self.validate_with_text(kernel, access_sources, TEXT, budget)
    }

    fn validate_with_text(
        &self,
        kernel: &ProductionRankedKernelV1,
        access_sources: &[ProductionRankedAccessSourceV1],
        diagnostic: &str,
        budget: &mut Budget<'_>,
    ) -> Result<
        (
            ValidatedNativeCompilerRankedSourceProofV1,
            NativeCompilerRankedSourceProofStorageV1,
        ),
        E,
    > {
        let launch = [ProductionSourceLaunchRootInputV1::new(
            NAME,
            BINDING,
            ProductionSourceLaunchInputV1::new(1, Some([1, 1, 1]), [1, 1, 1]),
        )];
        let commitments = [self.staging];
        let staging = [NativeCompilerRootStagingV1 {
            semantic_root: 0,
            commitments: &commitments,
        }];
        let signatures = [self.signature];
        let roots = [NativeCompilerRankedRootV1 {
            candidate: NativeRankedSourceCandidateV1::from_untrusted_parts(
                0,
                1,
                kernel,
                access_sources,
                &[],
                diagnostic,
            ),
            effect_receipts: &signatures,
        }];
        validate_native_compiler_ranked_source_proof_v1(
            NativeCompilerRankedSourceProofInputsV1 {
                source: NativeCompilerSourceProofInputsV1 {
                    semantic_mir: &self.semantic,
                    native_module: &self.native,
                    middle_end_roster: self.middle.canonical_bytes(),
                    correspondence_roster: self.correspondence.canonical_bytes(),
                    verus_roster: self.verus.canonical_bytes(),
                    launch_inputs: &launch,
                    staging_roots: &staging,
                },
                ranked_roots: &roots,
            },
            budget,
        )
    }
}

fn replace_and_resign_aggregate_claim(fixture: &mut Fixture, changed: usize) {
    use crate::mir_pliron_per_compilation_verus_v1::aggregate_obligation_from_commitments_v1;
    let original = Signed::decode(fixture.verus.root(0).unwrap().payload()).unwrap();
    let old = original.claims();
    let mut identities = [
        old.contract_identity(),
        old.parallel_contract_identity(),
        old.pliron_evidence_identity(),
        old.composition_template_identity(),
        old.generated_source_identity(),
    ];
    identities[changed] = d(199);
    let subjects = old.binding().subjects();
    let obligation = aggregate_obligation_from_commitments_v1(
        [
            identities[0],
            identities[1],
            identities[2],
            identities[3],
            identities[4],
            subjects.safe_reference_identity(),
            subjects.safe_reference_source_hash(),
            subjects.safe_reference_mir_hash(),
            subjects.kernel_subject_identity(),
            subjects.kernel_mir_hash(),
        ],
        [fixture.staging.digests()].into_iter(),
    );
    let binding = FunctionalRefinementBindingV2::from_subjects(subjects, obligation).unwrap();
    let (imported, signature) = sign(
        binding,
        FunctionalRefinementBoundaryV2::SafeReferenceMirToLivePliron,
        old.toolchain(),
    );
    let claims = Claims::new(
        identities[0],
        identities[1],
        identities[2],
        identities[3],
        identities[4],
        binding,
        imported.signer_identity(),
        imported.toolchain(),
        imported.execution_identity(),
        imported.receipt_identity().digest(),
        old.retained_policy_checked_staging(),
    )
    .unwrap();
    let signed = Signed::new(claims, *signature.verifying_key(), *signature.wire()).unwrap();
    assert_ne!(signed.signed_receipt(), original.signed_receipt());
    let row = fixture.verus.root(0).unwrap();
    fixture.verus = Roster::new(MultiRootProofRosterInputsV3 {
        kind: Kind::VerusExecution,
        semantic_mir_sha256: fixture.verus.semantic_mir_sha256(),
        native_neutral_subject: *fixture.verus.native_neutral_subject(),
        roster_identity: fixture.verus.roster_identity(),
        canonical_kernel_order: fixture.verus.canonical_kernel_order(),
        roots: &[MultiRootProofRosterRootInputV3 {
            semantic_root: row.semantic_root(),
            semantic_root_identity: row.semantic_root_identity(),
            kernel_binding: row.kernel_binding(),
            source_rank: row.source_rank(),
            workgroup: row.workgroup(),
            logical_name: row.logical_name(),
            export_symbol: row.export_symbol(),
            kernel_id: row.kernel_id(),
            payload: signed.canonical_bytes(),
        }],
    })
    .unwrap();
}

fn check_commitment_only_packet(fixture: &Fixture, budget: &mut Budget<'_>) -> Result<(), E> {
    let launch = [ProductionSourceLaunchRootInputV1::new(
        NAME,
        BINDING,
        ProductionSourceLaunchInputV1::new(1, Some([1, 1, 1]), [1, 1, 1]),
    )];
    let commitments = [fixture.staging];
    let staging = [NativeCompilerRootStagingV1 {
        semantic_root: 0,
        commitments: &commitments,
    }];
    validate_native_compiler_source_proof_v1(
        NativeCompilerSourceProofInputsV1 {
            semantic_mir: &fixture.semantic,
            native_module: &fixture.native,
            middle_end_roster: fixture.middle.canonical_bytes(),
            correspondence_roster: fixture.correspondence.canonical_bytes(),
            verus_roster: fixture.verus.canonical_bytes(),
            launch_inputs: &launch,
            staging_roots: &staging,
        },
        budget,
    )
    .map(drop)
}

#[test]
fn fresh_aggregate_rederivation_rejects_self_consistently_resigned_false_claims() {
    for changed in [0, 1, 3, 4] {
        let mut fixture = fixture();
        replace_and_resign_aggregate_claim(&mut fixture, changed);
        let access = [ProductionRankedAccessSourceV1::new(0, Some(1), 0, 0, 8)];
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(37).unwrap();
        // Signature/commitment consistency is deliberately weaker than deriving
        // the actual contract, template and generated source from the typed graph.
        check_commitment_only_packet(&fixture, &mut budget).unwrap();
        assert_eq!(budget.storage(), 37);
        assert!(matches!(
            fixture.validate(&fixture.kernel, &access, &mut budget),
            Err(E::Mismatch("fresh source/ranked aggregate subjects"))
        ));
        assert_eq!(budget.storage(), 37);
    }
}

#[test]
fn changed_or_oversized_diagnostic_text_is_rejected_before_ranked_recompilation() {
    let fixture = fixture();
    let access = [ProductionRankedAccessSourceV1::new(0, Some(1), 0, 0, 8)];
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(37).unwrap();
    check_commitment_only_packet(&fixture, &mut budget).unwrap();
    let base_work = budget.work();
    let changed = format!("X{}", &TEXT[1..]);
    let oversized = "x".repeat(TEXT.len() + 1024 * 1024);
    for diagnostic in [&changed, &oversized] {
        // Typed entry8, rootcount1, vectors3+3, root/rank3 and length1.
        // Equal-length inputs additionally pay the exact byte comparison.
        let exact = base_work
            + 19
            + if diagnostic.len() == TEXT.len() {
                TEXT.len()
            } else {
                0
            };
        let mut work = CanonicalKernelIrWorkBudgetV1::new(exact);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(37).unwrap();
        assert!(matches!(
            fixture.validate_with_text(&fixture.kernel, &access, diagnostic, &mut budget,),
            Err(E::Mismatch("exact typed ranked diagnostic text"))
        ));
        assert_eq!((budget.storage(), budget.work()), (37, exact));
    }
}

#[test]
fn full_signed_typed_native_source_packet_retains_fresh_correspondence_owner() {
    let fixture = fixture();
    let access = [ProductionRankedAccessSourceV1::new(0, Some(1), 0, 0, 8)];
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(37).unwrap();
    let (checked, storage) = fixture
        .validate(&fixture.kernel, &access, &mut budget)
        .unwrap();
    assert_eq!(budget.storage(), 37);
    budget.reserve_storage(storage.retained_storage()).unwrap();
    assert!(checked.replays_ranked_source_relation());
    assert_eq!(checked.root_count(), 1);
    checked.source().source().verify_equivalence().unwrap();
    assert_eq!(
        checked
            .source()
            .source()
            .pre_ranked_executable()
            .unwrap()
            .canonical()
            .canonical_bytes(),
        &fixture.native_graph
    );
    assert!(!checked.proves_whole_operational_or_indexed_address_equivalence());
    assert!(!checked.authenticates_compiler_or_launch_origin());
    assert!(!checked.grants_artifact_or_launch_authority());
    drop(checked);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 37);
}

#[test]
fn original_full_signature_rejects_changed_typed_graph_and_source_maps() {
    let fixture = fixture();
    let correct = [ProductionRankedAccessSourceV1::new(0, Some(1), 0, 0, 8)];
    let wrong_statement = [ProductionRankedAccessSourceV1::new(0, Some(0), 0, 0, 8)];
    let wrong_operation = [ProductionRankedAccessSourceV1::new(0, Some(1), 0, 0, 7)];
    for access in [&[][..], &wrong_statement[..], &wrong_operation[..]] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(37).unwrap();
        assert!(
            fixture
                .validate(&fixture.kernel, access, &mut budget)
                .is_err()
        );
        assert_eq!(budget.storage(), 37);
    }
    let mut operations = fixture.kernel.blocks()[0].operations().to_vec();
    let ProductionRankedOperationV1::SemanticExpression { expression, .. } = &mut operations[3]
    else {
        unreachable!()
    };
    let ProductionSemanticExpressionV2::Constant { bits, .. } = expression else {
        unreachable!()
    };
    *bits = 8;
    let changed = ProductionRankedKernelV1::new(
        NAME,
        0,
        vec![ProductionRankedBlockV1::new(
            operations,
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(37).unwrap();
    assert!(fixture.validate(&changed, &correct, &mut budget).is_err());
    assert_eq!(budget.storage(), 37);
}

#[path = "compiler_native_unit_local_erased_source_proof_v1_tests.rs"]
mod unit_local_erased_fixture;
