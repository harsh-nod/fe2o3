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
        let wrapped = CompilerDescriptorError::SourceAbi(error);
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
    let selector = 2 * h::<Parameters<'_, '_>>()
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
