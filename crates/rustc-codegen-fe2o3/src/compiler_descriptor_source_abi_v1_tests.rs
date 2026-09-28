use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct Qualification {
    pub(crate) roots: usize,
    pub(crate) logical: usize,
    pub(crate) physical: usize,
    pub(crate) capture_mutants: usize,
    pub(crate) endpoint_mutants: usize,
    pub(crate) same_typed_endpoint_mutants: usize,
    pub(crate) resource_cuts: usize,
    pub(crate) bridge_refusals: usize,
    pub(crate) packing: entry_packing::tests::Qualification,
}

fn bridge_refusal_controls(owner: OwnerRef<'_>, floor: usize) -> usize {
    let semantic = match owner {
        OwnerRef::Direct(value) => value.source_semantic_kir().semantic().semantic(),
        OwnerRef::Erased(value) => value.original_source().semantic_ssa().source_semantic(),
    };
    let root = semantic.roots()[0];
    let selected = semantic.select_kernel_body_for_root_v1(root).unwrap();
    for resource_first in [false, true] {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(floor).unwrap();
        let result: R<()> = with_plan(owner, root, selected.body(), &mut budget, |plan| {
            assert!(plan.counts()?.0 > 0);
            if resource_first {
                assert!(matches!(
                    plan.charge_work(usize::MAX),
                    Err(SourceError::ArgumentCorrespondenceResource(Resource::Work(
                        _
                    )))
                ));
            }
            Err(E::Mismatch("selected callback error"))
        });
        if resource_first {
            assert!(matches!(
                result,
                Err(E::Source(SourceError::ArgumentCorrespondenceResource(
                    Resource::Work(_)
                )))
            ));
            assert!(budget.failed_work().is_some());
        } else {
            assert!(matches!(
                result,
                Err(E::Mismatch("selected callback error"))
            ));
            assert!(budget.failed_work().is_none());
        }
        assert_eq!(budget.storage(), floor);
    }
    2
}

fn measure(
    owner: OwnerRef<'_>,
    roots: &[TypedDescriptorRootV1],
    profile: ProductionAmdTargetProfileV1,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
) -> (R<SourceAbiSummaryV1>, usize, usize) {
    // Immutable owners admit fresh continuing meters, provided their genuine
    // live owned storage is reserved and the entire proof is replayed.
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(floor).unwrap();
    let result = check(owner, roots, profile, &mut budget);
    assert_eq!(budget.storage(), floor);
    assert!(budget.work() <= work_limit && budget.peak_storage() <= storage_limit);
    (result, budget.work(), budget.peak_storage())
}

fn endpoint_mutants(
    owner: OwnerRef<'_>,
    roots: &[TypedDescriptorRootV1],
    budget: &mut Budget<'_>,
) -> R<(usize, usize)> {
    let (semantic, endpoints) = match owner {
        OwnerRef::Direct(value) => {
            let source = value.source_semantic_kir();
            (
                source.semantic().semantic(),
                [
                    source.module(),
                    source.pre_ranked_executable().unwrap().module(),
                    value.bound().module(),
                    value.output().module(),
                ],
            )
        }
        OwnerRef::Erased(value) => (
            value.original_source().semantic_ssa().source_semantic(),
            [
                value.original_source().executable().module(),
                value.erased().module(),
                value.bound().module(),
                value.output().module(),
            ],
        ),
    };
    let root = semantic.roots()[0];
    let selected = semantic.select_kernel_body_for_root_v1(root).unwrap();
    let source = &semantic.functions()[selected.body().index() as usize];
    let mut count = 0;
    let mut same_typed = 0;
    for endpoint in 1..4 {
        for fault in 0..4 {
            // Diagnostic endpoint copies can only reach the private join test;
            // they are never promoted to checked optimizer owners.
            let mut changed = endpoints[endpoint].clone();
            let id = changed.kernels[0].entry.clone();
            let function = changed.functions.iter_mut().find(|f| f.id == id).unwrap();
            match fault {
                0 => {
                    let len = function.signature.parameters.len();
                    assert!(len >= 2);
                    let pair = (0..len)
                        .flat_map(|a| ((a + 1)..len).map(move |b| (a, b)))
                        .find(|&(a, b)| {
                            function.signature.parameters[a] == function.signature.parameters[b]
                        });
                    let (a, b) = pair.unwrap_or((0, len - 1));
                    same_typed += usize::from(pair.is_some());
                    function.body.as_mut().unwrap().parameters.swap(a, b);
                }
                1 => function.signature.parameters[0] = Type::Unit,
                2 => {
                    function.body.as_mut().unwrap().parameters.pop();
                }
                3 => function.role = FunctionRole::InternalHelper,
                _ => unreachable!(),
            }
            let mut substituted = endpoints;
            substituted[endpoint] = &changed;
            let result = with_plan(owner, root, selected.body(), budget, |plan| {
                check_root(plan, semantic, source, &roots[0], 0, substituted)
            });
            assert!(matches!(
                result,
                Err(E::Mismatch(
                    "exact original/optimized entry parameter transport"
                        | "complete physical entry signature coverage"
                ))
            ));
            count += 1;
        }
    }
    Ok((count, same_typed))
}

pub(crate) fn qualify(
    owner: OwnerRef<'_>,
    roots: &[TypedDescriptorRootV1],
    profile: ProductionAmdTargetProfileV1,
    budget: &mut Budget<'_>,
) -> R<Qualification> {
    let floor = budget.storage();
    let packing = entry_packing::tests::qualify(owner, roots, profile, floor);
    let positive = check(owner, roots, profile, budget)?;
    assert_eq!(check(owner, roots, profile, budget)?, positive);
    check_if_aggregate(owner, roots, profile, budget)?;
    assert_eq!(budget.storage(), floor);
    assert_eq!(positive.roots, 1);
    assert_eq!(positive.logical, 3);
    assert_ne!(positive.logical, positive.physical);
    let mut capture_mutants = 0;
    for fault in 0..12 {
        let mut changed = roots.to_vec();
        let mut args = changed[0].arguments.as_slice().to_vec();
        match fault {
            0 => {
                let mut digest = *args[0].semantic_type_identity.as_bytes();
                digest[0] ^= 1;
                args[0].semantic_type_identity = SemanticTypeIdentityV1::from_sha256(digest);
            }
            1 => {
                let mut digest = *args[0].semantic_layout_identity.as_bytes();
                digest[0] ^= 1;
                args[0].semantic_layout_identity = SemanticLayoutIdentityV1::from_sha256(digest);
            }
            2 => args[0].source_size += 1,
            3 => args[0].source_alignment *= 2,
            4 => args[0].rustc_abi_class = RustcAbiClassV1::Uninhabited,
            5 => args[0].access = AccessMode::ReadOnly,
            6 => args[0].kind = DescriptorArgumentKindV1::Scalar(ScalarTypeV1::U64),
            7 => {
                args.pop();
            }
            8 => args.swap(0, 2),
            9 => changed[0].export_name.push('_'),
            10 => {
                let mut binding = changed[0].kernel_binding_bytes();
                binding[0] ^= 1;
                changed[0].kernel_binding = KernelBindingIdV1::from_bytes(binding);
            }
            11 => {}
            _ => unreachable!(),
        }
        changed[0].arguments = TypedArgumentListV1::new(args).unwrap();
        if fault == 11 {
            changed.push(changed[0].clone());
        }
        let error = check(owner, &changed, profile, budget).unwrap_err();
        assert!(
            matches!(error, E::Mismatch(_)),
            "capture fault {fault}: {error:?}"
        );
        let mut packing_entered = false;
        let error = entry_packing::with_schedule(owner, &changed, profile, budget, |_| {
            packing_entered = true;
            Ok(())
        })
        .unwrap_err();
        assert!(!packing_entered);
        assert!(
            matches!(error, E::Mismatch(_)),
            "packing capture fault {fault}: {error:?}"
        );
        assert_eq!(budget.storage(), floor);
        capture_mutants += 1;
    }
    let bridge_refusals = bridge_refusal_controls(owner, floor);
    let (endpoint_mutants, same_typed_endpoint_mutants) = endpoint_mutants(owner, roots, budget)?;
    assert_eq!(budget.storage(), floor);
    let (full, work, peak) = measure(owner, roots, profile, floor, usize::MAX, usize::MAX);
    assert_eq!(full?, positive);
    assert_eq!(
        measure(owner, roots, profile, floor, work, peak).0?,
        positive
    );
    assert!(
        measure(owner, roots, profile, floor, work - 1, usize::MAX)
            .0
            .is_err()
    );
    assert!(
        measure(owner, roots, profile, floor, usize::MAX, peak - 1)
            .0
            .is_err()
    );
    assert!(matches!(
        measure(
            owner,
            roots,
            profile,
            floor,
            usize::MAX,
            floor + headers()? - 1
        )
        .0,
        Err(E::Resource(Resource::Storage(_)))
    ));
    Ok(Qualification {
        roots: positive.roots,
        logical: positive.logical,
        physical: positive.physical,
        capture_mutants,
        endpoint_mutants,
        same_typed_endpoint_mutants,
        resource_cuts: 3,
        bridge_refusals,
        packing,
    })
}

#[test]
fn source_abi_capture_error_chain_preserves_all_wrapped_resource_causes() {
    use fe2o3_lower_mir_kernel::{
        ProductionCheckedOutputAdmissionErrorPolicy3V1 as Policy3,
        ProductionCheckedOutputAdmissionErrorPolicy4V1 as Policy4,
    };
    for error in [
        E::Resource(Resource::Accounting),
        E::Source(SourceError::ArgumentCorrespondenceResource(
            Resource::Accounting,
        )),
        E::Checked(Policy4::Admission(Policy3::Resource(Resource::Accounting))),
        E::Target(dialect_amdgcn::ProductionTargetCoordinateErrorV1::Resource(
            Resource::Accounting,
        )),
    ] {
        let wrapped = CompilerDescriptorError::from_source_abi(error);
        let mut cause: &(dyn std::error::Error + 'static) = &wrapped;
        while let Some(next) = cause.source() {
            cause = next;
        }
        assert_eq!(
            cause.downcast_ref::<Resource>(),
            Some(&Resource::Accounting)
        );
    }
    assert!(std::error::Error::source(&E::Mismatch("capture mismatch")).is_none());
}

#[test]
fn source_abi_capture_outer_normalization_preserves_every_admission_variant() {
    use fe2o3_lower_mir_kernel::{
        ProductionCheckedOutputAdmissionErrorPolicy3V1 as P3,
        ProductionCheckedOutputAdmissionErrorPolicy4V1 as P4,
        ProductionCheckedOutputCensusContextV1 as Context,
        ProductionCheckedOutputRankedCensusKindV1 as Kind,
    };
    for error in [
        P3::Resource(Resource::Accounting),
        P3::Source(SourceError::CorrespondenceMismatch),
        P3::SourceOutput(fe2o3_lower_mir_kernel::ProductionSourceOutputErrorV1::InputCustody),
        P3::Coordinates(
            fe2o3_kernel_analysis::CanonicalKirCoordinatePreservationErrorV1::Mismatch(
                "exact source coordinate",
            ),
        ),
        P3::Formal(fe2o3_lower_mir_kernel::ProductionFormalMemoryErrorV1::ObligationMismatch),
        P3::PrivateAddressR2,
        P3::Unsupported {
            phase: "source",
            detail: "exact source detail",
        },
        P3::UnsupportedOperation {
            phase: "ranked",
            detail: "closed global effects and source scalar recipes",
            context: Context::Ranked {
                block: 11,
                operation: 23,
                kind: Kind::Barrier,
            },
        },
    ] {
        let expected_debug = format!("{error:?}");
        let expected_display = format!("checked Policy3 output admission failed: {error}");
        let normalized = CompilerDescriptorError::from_source_abi(E::Checked(P4::Admission(error)));
        let CompilerDescriptorError::CheckedOutputPolicy3(ref preserved) = normalized else {
            panic!("checked admission must reuse the exact Policy3 payload");
        };
        assert_eq!(format!("{preserved:?}"), expected_debug);
        assert_eq!(normalized.to_string(), expected_display);
        let cause = std::error::Error::source(&normalized).unwrap();
        assert!(std::ptr::eq(cause.downcast_ref::<P3>().unwrap(), preserved));
        assert!(cause.downcast_ref::<E>().is_none());
        assert!(cause.downcast_ref::<P4>().is_none());
    }
}

#[test]
fn source_abi_capture_outer_normalization_keeps_typed_optimization_causes() {
    use fe2o3_kernel_opt::CanonicalPolicy4OptimizationErrorV1 as Optimization;
    use fe2o3_lower_mir_kernel::ProductionCheckedOutputAdmissionErrorPolicy4V1 as P4;
    for error in [
        Optimization::Resource(Resource::Accounting),
        Optimization::InputHistory,
        Optimization::Panicked,
    ] {
        let expected_debug = format!("{error:?}");
        let expected_display = format!("source ABI checked optimization replay failed: {error}");
        // The preexisting optimization error is itself the terminal typed cause.
        assert!(std::error::Error::source(&error).is_none());
        let normalized =
            CompilerDescriptorError::from_source_abi(E::Checked(P4::Optimization(error)));
        let CompilerDescriptorError::SourceAbiCheckedOptimization(ref preserved) = normalized
        else {
            panic!("checked optimization must retain its exact typed payload");
        };
        assert_eq!(format!("{preserved:?}"), expected_debug);
        assert_eq!(normalized.to_string(), expected_display);
        let cause = std::error::Error::source(&normalized).unwrap();
        assert!(std::ptr::eq(
            cause.downcast_ref::<Optimization>().unwrap(),
            preserved
        ));
        assert!(cause.source().is_none());
        if let Optimization::Resource(error) = preserved {
            assert_eq!(*error, Resource::Accounting);
        }
    }
}

#[test]
fn source_abi_capture_nonchecked_diagnostics_keep_exact_display_and_leaf() {
    for error in [
        E::Resource(Resource::Accounting),
        E::Source(SourceError::ArgumentCorrespondenceResource(
            Resource::Accounting,
        )),
        E::Target(dialect_amdgcn::ProductionTargetCoordinateErrorV1::Resource(
            Resource::Accounting,
        )),
        E::Mismatch("exact captured source layout"),
    ] {
        let expected_debug = format!("{error:?}");
        let expected_display = format!("source ABI correspondence failed: {error}");
        let mismatch = matches!(error, E::Mismatch(_));
        let normalized = CompilerDescriptorError::from_source_abi(error);
        let CompilerDescriptorError::SourceAbi(ref detail) = normalized else {
            panic!("nonchecked failures retain source ABI diagnostic context");
        };
        assert_eq!(format!("{detail:?}"), expected_debug);
        assert_eq!(normalized.to_string(), expected_display);
        let mut cause = std::error::Error::source(&normalized).unwrap();
        assert!(std::ptr::eq(
            cause.downcast_ref::<SourceAbiDiagnosticV1>().unwrap(),
            detail
        ));
        while let Some(next) = cause.source() {
            cause = next;
        }
        if mismatch {
            assert!(cause.downcast_ref::<SourceAbiDiagnosticV1>().is_some());
        } else {
            assert_eq!(
                cause.downcast_ref::<Resource>(),
                Some(&Resource::Accounting)
            );
        }
    }
}

#[test]
fn source_abi_capture_errors_and_results_remain_compact_without_allocation() {
    assert!(size_of::<SourceAbiDiagnosticV1>() < size_of::<E>());
    assert!(size_of::<E>() < 128);
    assert!(size_of::<Result<(), E>>() < 128);
    assert!(size_of::<CompilerDescriptorError>() < 128);
    assert!(size_of::<Result<(), CompilerDescriptorError>>() < 128);
}

#[test]
fn source_abi_capture_selector_and_bridge_have_independent_frame_equations() {
    fn h<T>() -> usize {
        size_of::<T>()
            + 3 * size_of::<Result<T, SourceAbiErrorV1>>()
            + 2 * size_of::<Result<T, SourceError>>()
    }
    type Parameters<'a, 'work> = (
        OwnerRef<'a>,
        &'a [TypedDescriptorRootV1],
        ProductionAmdTargetProfileV1,
        &'a mut Budget<'work>,
        usize,
    );
    type Visitor<'a> = dyn for<'scope, 'source, 'work> FnMut(
            usize,
            &TypedDescriptorRootV1,
            &AdmittedInertSemanticMirV1,
            &mut Plan<'scope, 'source, 'work>,
        ) -> Result<(), SourceAbiErrorV1>
        + 'a;
    let selector = h::<Parameters<'_, '_>>()
        + h::<(Parameters<'_, '_>, Option<&mut Visitor<'_>>)>()
        + h::<std::slice::Iter<'_, TypedDescriptorRootV1>>()
        + h::<Option<&TypedDescriptorRootV1>>()
        + h::<&TypedDescriptorRootV1>()
        + h::<std::slice::Iter<'_, TypedDescriptorArgumentV1>>()
        + h::<Option<&TypedDescriptorArgumentV1>>()
        + h::<&TypedDescriptorArgumentV1>()
        + h::<&[TypedDescriptorArgumentV1]>()
        + h::<SourceAbiSummaryV1>()
        + h::<bool>()
        + h::<()>()
        + 8 * h::<usize>();
    assert_eq!(selector_headers().unwrap(), selector);
    type Payload = ([u64; 5], bool);
    type Output = (usize, usize);
    let bridge = h::<(
        OwnerRef<'_>,
        SemanticFunctionIdV1,
        SemanticFunctionIdV1,
        &mut Budget<'_>,
    )>() + h::<Payload>()
        + align_of::<Payload>()
        + h::<(Result<Output, SourceError>, Option<E>)>()
        + h::<Option<E>>()
        + h::<Output>()
        + h::<&mut Option<E>>()
        + size_of::<Payload>()
        + size_of::<&mut Option<E>>()
        + align_of::<Payload>()
        + align_of::<&mut Option<E>>()
        + 8 * h::<usize>()
        + h::<bool>();
    assert_eq!(bridge_headers::<Output, Payload>().unwrap(), bridge);
    for bytes in [selector, bridge] {
        for extra in [0, 1] {
            let mut work = Work::new(0);
            let mut budget = Budget::new(&mut work, 23 + bytes - extra);
            budget.reserve_storage(23).unwrap();
            let result = budget.reserve_storage(bytes);
            if extra == 0 {
                result.unwrap();
                budget.release_storage(bytes).unwrap();
            } else {
                assert!(matches!(result, Err(Resource::Storage(_))));
            }
            assert_eq!((budget.work(), budget.storage()), (0, 23));
        }
    }
}

#[test]
fn source_abi_capture_header_reservation_is_atomic_at_exact_boundary() {
    let required = headers().unwrap();
    for short in [false, true] {
        let mut work = Work::new(0);
        let mut budget = Budget::new(&mut work, required - usize::from(short));
        let result = budget.reserve_storage(required);
        if short {
            assert!(matches!(result, Err(Resource::Storage(_))));
            assert_eq!(budget.storage(), 0);
        } else {
            result.unwrap();
            assert_eq!(budget.storage(), required);
            budget.release_storage(required).unwrap();
        }
        assert_eq!(budget.work(), 0);
    }
}
