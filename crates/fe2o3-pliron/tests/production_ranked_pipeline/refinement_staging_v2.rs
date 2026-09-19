use super::*;
use fe2o3_pliron::stage_ranked_kernel_with_policy_checked_refinement_v2 as stage;

struct Fixture {
    kernel: ProductionRankedKernelV1,
    imports: Vec<ImportedFunctionalRefinementProofV2>,
    policy: ProductionRefinementStagingPolicyV2,
    requests: [usize; 2],
}

fn fixture() -> Fixture {
    let (mut kernel, requests) = two_tensor_receipt_kernel();
    let obligations = requests.map(|operation| {
        let ProductionRankedOperationV1::RequestTensorRefinement { contract, subjects } =
            &kernel.blocks()[0].operations()[operation]
        else {
            unreachable!()
        };
        normalized_tensor_refinement_hash_for_kernel_v1(&kernel, 0, operation, contract, *subjects)
            .unwrap()
    });
    let mut imports = Vec::new();
    let mut policy = None;
    for (operation, obligation) in requests.into_iter().zip(obligations) {
        let (request, imported, selected_policy) = imported_reference(
            functional_binding(44, obligation),
            FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir,
        );
        kernel = kernel
            .bind_functional_refinement_request_v2(0, operation, request)
            .unwrap();
        imports.push(imported);
        policy = Some(selected_policy);
    }
    Fixture {
        kernel,
        imports,
        policy: policy.unwrap(),
        requests,
    }
}

fn rewrite(
    kernel: &ProductionRankedKernelV1,
    change: impl FnOnce(&mut [ProductionRankedOperationV1]),
) -> ProductionRankedKernelV1 {
    assert_eq!(kernel.blocks().len(), 1);
    let block = &kernel.blocks()[0];
    assert_eq!(block.index_argument_count(), 0);
    let mut operations = block.operations().to_vec();
    change(&mut operations);
    ProductionRankedKernelV1::new(
        kernel.function_name(),
        kernel.argument_count(),
        vec![ProductionRankedBlockV1::new(
            operations,
            block.terminator().clone(),
        )],
    )
    .unwrap()
}

fn replace_request(fixture: &mut Fixture, operation: usize, request: ProductionReferenceProofV2) {
    fixture.kernel = rewrite(&fixture.kernel, |operations| {
        let ProductionRankedOperationV1::RequireTensorRefinement { proof, .. } =
            &mut operations[operation]
        else {
            unreachable!()
        };
        *proof = request;
    });
}

#[test]
fn consumed_staging_preserves_receipts_order_and_normal_pipeline_identity() {
    let mut results = Vec::new();
    for through_wrapper in [false, true] {
        let mut fixture = fixture();
        let expected = fixture
            .imports
            .iter()
            .map(|proof| {
                (
                    proof.receipt_identity(),
                    proof.binding(),
                    proof.signer_identity(),
                    proof.toolchain(),
                    proof.execution_identity(),
                    proof.boundary(),
                )
            })
            .collect::<Vec<_>>();
        fixture.imports.reverse();
        let recipe = fixture.kernel.clone();
        let input = if through_wrapper {
            compile_ranked_kernel_with_policy_checked_refinement_staging_v2(
                construction(fixture.kernel),
                ProductionSessionLimitsV1::default(),
                fixture.imports,
                fixture.policy,
            )
            .unwrap()
        } else {
            let staged = stage(
                construction(fixture.kernel),
                fixture.imports,
                fixture.policy,
            )
            .unwrap();
            compile_ranked_kernel_for_lowering_v1(staged, ProductionSessionLimitsV1::default())
                .unwrap()
        };
        assert_eq!(input.kernel(), &recipe);
        assert!(input.all_mandatory_reports_are_clean());
        assert!(!input.grants_compiler_refinement_authority());
        assert!(!input.grants_artifact_or_launch_authority());
        assert_eq!(
            input.production_pipeline_report().pass_order(),
            &PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2
        );
        let actual = input
            .retained_policy_checked_refinement_staging()
            .iter()
            .map(|proof| {
                assert!(proof.is_policy_checked_untrusted_staging());
                assert!(!proof.grants_source_to_isa_authority());
                assert!(!proof.grants_artifact_or_launch_authority());
                (
                    proof.receipt_identity(),
                    proof.binding(),
                    proof.signer_identity(),
                    proof.toolchain(),
                    proof.execution_identity(),
                    proof.boundary(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(actual, expected);
        results.push(input);
    }
    assert_eq!(results[0].kernel(), results[1].kernel());
    assert_eq!(
        results[0].exact_graph_identity(),
        results[1].exact_graph_identity()
    );
    assert_eq!(
        results[0].retained_policy_checked_refinement_staging(),
        results[1].retained_policy_checked_refinement_staging()
    );
}

#[derive(Clone, Copy, Debug)]
enum Failure {
    ConstructionKind,
    Unbound,
    DuplicateImport,
    DuplicateClaim,
    Missing,
    Unused,
    Binding,
    Boundary,
    Signer,
    Toolchain,
    Obligation,
}

#[test]
fn consuming_staging_and_compile_wrapper_preserve_exact_admission_errors() {
    use ProductionFunctionalRefinementAdmissionErrorV2 as Error;
    for case in [
        Failure::ConstructionKind,
        Failure::Unbound,
        Failure::DuplicateImport,
        Failure::DuplicateClaim,
        Failure::Missing,
        Failure::Unused,
        Failure::Binding,
        Failure::Boundary,
        Failure::Signer,
        Failure::Toolchain,
        Failure::Obligation,
    ] {
        for through_wrapper in [false, true] {
            let mut fixture = fixture();
            let identity = fixture.imports[0].receipt_identity();
            let binding = fixture.imports[0].binding();
            let boundary = fixture.imports[0].boundary();
            let operation = fixture.requests[0];
            let expected = match case {
                Failure::ConstructionKind => Error::WrongConstructionKind,
                Failure::Unbound => {
                    fixture.kernel = two_tensor_receipt_kernel().0;
                    Error::UnboundRequest
                }
                Failure::DuplicateImport => {
                    fixture
                        .imports
                        .push(imported_reference(binding, boundary).1);
                    Error::DuplicateImportedReceipt(identity)
                }
                Failure::DuplicateClaim => {
                    let second = fixture.requests[1];
                    replace_request(
                        &mut fixture,
                        second,
                        ProductionReferenceProofV2::request_exact(identity, binding),
                    );
                    Error::DuplicateReceiptClaim(identity)
                }
                Failure::Missing => {
                    fixture.imports.remove(0);
                    Error::MissingImportedReceipt(identity)
                }
                Failure::Unused => {
                    let (_, extra, _) =
                        imported_reference(functional_binding(45, proof_digest(77)), boundary);
                    let extra_identity = extra.receipt_identity();
                    fixture.imports.push(extra);
                    Error::UnusedImportedReceipt(extra_identity)
                }
                Failure::Binding => {
                    let (_, stale, _) = imported_reference(
                        functional_binding(45, binding.normalized_obligation_effect_ir_hash()),
                        boundary,
                    );
                    let stale_identity = stale.receipt_identity();
                    replace_request(
                        &mut fixture,
                        operation,
                        ProductionReferenceProofV2::request_exact(stale_identity, binding),
                    );
                    fixture.imports[0] = stale;
                    Error::BindingMismatch(stale_identity)
                }
                Failure::Boundary => {
                    let (request, wrong, _) = imported_reference(
                        binding,
                        FunctionalRefinementBoundaryV2::SafeReferenceSourceToKernelMir,
                    );
                    let wrong_identity = wrong.receipt_identity();
                    replace_request(&mut fixture, operation, request);
                    fixture.imports[0] = wrong;
                    Error::WrongBoundary(wrong_identity)
                }
                Failure::Signer => {
                    fixture.policy = ProductionRefinementStagingPolicyV2::new(
                        [proof_digest(92)],
                        fixture.policy.toolchain(),
                    )
                    .unwrap();
                    Error::WrongSigner(identity)
                }
                Failure::Toolchain => {
                    fixture.policy = ProductionRefinementStagingPolicyV2::new(
                        [fixture.imports[0].signer_identity()],
                        VerusToolchainIdentityV2::new(
                            proof_digest(41),
                            proof_digest(42),
                            proof_digest(43),
                            proof_digest(44),
                            proof_digest(45),
                        )
                        .unwrap(),
                    )
                    .unwrap();
                    Error::WrongToolchain(identity)
                }
                Failure::Obligation => {
                    fixture.kernel = rewrite(&fixture.kernel, |operations| {
                        let ProductionRankedOperationV1::ExecutionLayout { grid_identity, .. } =
                            &mut operations[0]
                        else {
                            unreachable!()
                        };
                        *grid_identity += 1;
                    });
                    Error::ObligationEffectDigestMismatch(identity)
                }
            };
            let construction = if matches!(case, Failure::ConstructionKind) {
                ProductionConstructionV1::builtin_module("not_ranked").unwrap()
            } else {
                construction(fixture.kernel)
            };
            let actual = if through_wrapper {
                match compile_ranked_kernel_with_policy_checked_refinement_staging_v2(
                    construction,
                    ProductionSessionLimitsV1::default(),
                    fixture.imports,
                    fixture.policy,
                ) {
                    Err(ProductionRankedCompileErrorV2::Proof(error)) => error,
                    other => panic!("{case:?} missed admission: {other:?}"),
                }
            } else {
                stage(construction, fixture.imports, fixture.policy).unwrap_err()
            };
            assert_eq!(actual, expected, "{case:?}, wrapper={through_wrapper}");
        }
    }
}

#[test]
fn successful_staging_cannot_skip_bounds_or_reorder_admission_before_them() {
    for kernel in [static_kernel(64, 64), dynamic_kernel(false)] {
        for through_wrapper in [false, true] {
            let fixture = fixture();
            let result = if through_wrapper {
                match compile_ranked_kernel_with_policy_checked_refinement_staging_v2(
                    construction(kernel.clone()),
                    ProductionSessionLimitsV1::default(),
                    Vec::new(),
                    fixture.policy,
                ) {
                    Err(ProductionRankedCompileErrorV2::Pipeline(error)) => Err(error),
                    other => panic!("expected normal pipeline rejection: {other:?}"),
                }
            } else {
                let staged = stage(construction(kernel.clone()), Vec::new(), fixture.policy)
                    .expect("receipt-free construction stages before bounds analysis");
                compile_ranked_kernel_for_lowering_v1(staged, ProductionSessionLimitsV1::default())
            };
            assert!(matches!(
                result,
                Err(ProductionRankedCompileErrorV1::Session(
                    ProductionSessionErrorV1::RankedBounds(_)
                ))
            ));
        }
        for through_wrapper in [false, true] {
            let fixture = fixture();
            let identity = fixture.imports[0].receipt_identity();
            let proof = fixture.imports.into_iter().next().unwrap();
            let actual = if through_wrapper {
                match compile_ranked_kernel_with_policy_checked_refinement_staging_v2(
                    construction(kernel.clone()),
                    ProductionSessionLimitsV1::default(),
                    vec![proof],
                    fixture.policy,
                ) {
                    Err(ProductionRankedCompileErrorV2::Proof(error)) => error,
                    other => panic!("admission must precede bounds: {other:?}"),
                }
            } else {
                stage(construction(kernel.clone()), vec![proof], fixture.policy).unwrap_err()
            };
            assert_eq!(
                actual,
                ProductionFunctionalRefinementAdmissionErrorV2::UnusedImportedReceipt(identity)
            );
        }
    }
}
