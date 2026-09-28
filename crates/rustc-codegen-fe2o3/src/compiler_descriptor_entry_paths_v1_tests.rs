use super::*;
use Projection::{ArrayIndex as A, Field as F};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

pub(in super::super) fn inspect(view: &mut Schedule<'_, '_, '_>) -> R<()> {
    let root = view.root(0)?;
    let argument = view.argument(0)?;
    let array = argument.first_component != argument.end_component
        && view.component(argument.first_component)?.kind == Kind::Scalar(ScalarTypeV1::U64);
    let mut first = Vec::new();
    let mut whole = 0;
    view.visit_paths(|path| {
        assert!(!path.grants_authority());
        assert_eq!(path.root(), path.argument().root);
        if path.source_path().is_empty() {
            assert_eq!(path.source_type(), path.argument().source_type);
            assert_eq!(path.source_layout(), path.argument().source_layout);
            whole += 1;
        }
        match path.coverage() {
            Coverage::Zero => assert!(path.components().is_empty()),
            Coverage::Slots { first, end } => {
                assert!(first < end);
                assert_eq!(path.components().first().unwrap().slot, first);
                assert_eq!(path.components().last().unwrap().slot + 1, end);
            }
            Coverage::WithinSlot(slot) => {
                assert!(path.components().iter().all(|row| row.slot == slot))
            }
        }
        if path.root() == 0 && path.argument().ordinal == 0 {
            first.push((
                path.source_path().to_vec(),
                path.coverage(),
                path.source_type(),
            ));
        }
        Ok(())
    })?;
    assert_eq!(whole, 3);
    let expected = match root.physical_slots {
        2 => vec![vec![]],
        4 if array => vec![vec![A(0)], vec![A(1)], vec![]],
        4 => vec![vec![F(0)], vec![F(1)], vec![]],
        6 => vec![
            vec![F(0), F(0)],
            vec![F(0), F(1)],
            vec![F(0)],
            vec![F(1), A(0)],
            vec![F(1), A(1)],
            vec![F(1)],
            vec![],
        ],
        other => panic!("unexpected genuine packing source shape {other}"),
    };
    assert_eq!(
        first.iter().map(|row| row.0.clone()).collect::<Vec<_>>(),
        expected
    );
    if array {
        assert_eq!(first[0].2, first[1].2);
        assert_ne!(first[0].0, first[1].0);
        assert_ne!(first[0].1, first[1].1);
    }
    if root.physical_slots == 2 {
        assert_eq!(first[0].1, Coverage::Zero);
    }
    Ok(())
}

pub(in super::super) fn qualify(
    owner: OwnerRef<'_>,
    roots: &[TypedDescriptorRootV1],
    profile: ProductionAmdTargetProfileV1,
    floor: usize,
) {
    for mode in 0..7 {
        let work_limit = if mode == 5 { 1_000_000_000 } else { usize::MAX };
        let mut work = Work::new(work_limit);
        let limit = if mode == 4 {
            floor.checked_add(1_000_000_000).unwrap()
        } else {
            usize::MAX
        };
        let mut budget = Budget::new(&mut work, limit);
        budget.reserve_storage(floor).unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| {
            with_schedule(owner, roots, profile, &mut budget, |view| {
                let protected = view.budget.storage();
                match mode {
                    0 => {
                        assert!(matches!(
                            view.visit_paths(|_| Err(E::Mismatch("selected path consumer"))),
                            Err(E::Mismatch("selected path consumer"))
                        ));
                        assert_eq!(view.budget.storage(), protected);
                        inspect(view)?;
                    }
                    1 => {
                        assert!(
                            catch_unwind(AssertUnwindSafe(
                                || view.visit_paths(|_| panic!("selected path consumer panic"))
                            ))
                            .is_err()
                        );
                        assert_eq!(view.budget.storage(), protected);
                        inspect(view)?;
                    }
                    2 => {
                        let original = view.components;
                        let mut changed = original.to_vec();
                        assert!(changed.len() >= 3);
                        assert_ne!(changed[0].value, changed.last().unwrap().value);
                        changed[0].value = changed.last().unwrap().value;
                        let result = {
                            // Private test-only reconstructed view; never a public constructor.
                            let mut forged = Schedule {
                                owner: view.owner,
                                captured: view.captured,
                                profile: view.profile,
                                roots: view.roots,
                                arguments: view.arguments,
                                components: &changed,
                                slot: view.slot,
                                ledger: view.ledger,
                                required: view.required,
                                first: Cell::new(None),
                                budget: &mut *view.budget,
                            };
                            forged.visit_paths(|_| Ok(()))
                        };
                        assert!(matches!(
                            result,
                            Err(E::Mismatch("packing path exact physical leaf"))
                        ));
                        assert_eq!(view.budget.storage(), protected);
                        inspect(view)?;
                    }
                    3 => {
                        let first = view.charge_work(usize::MAX).unwrap_err();
                        let before = (view.budget.work(), view.budget.storage());
                        let mut reached = false;
                        let replay = view
                            .visit_paths(|_| {
                                reached = true;
                                Ok(())
                            })
                            .unwrap_err();
                        assert!(!reached);
                        assert_eq!(resource(&first), resource(&replay));
                        assert_eq!((view.budget.work(), view.budget.storage()), before);
                    }
                    4 => {
                        type Visit = for<'a> fn(Path<'a>) -> R<()>;
                        fn accept(_: Path<'_>) -> R<()> {
                            panic!("denied header invoked visitor")
                        }
                        let bytes = headers::<Visit>()?;
                        let padding = limit - protected - bytes + 1;
                        view.budget.reserve_storage(padding)?;
                        let work = view.budget.work();
                        let error = view.visit_paths(accept as Visit).unwrap_err();
                        assert!(
                            matches!(error, E::Resource(Resource::Storage(error)) if error.limit() == limit && error.actual() == limit + 1)
                        );
                        assert_eq!(view.budget.work(), work + 1);
                        assert_eq!(view.budget.failed_storage(), Some(limit + 1));
                        view.budget.release_storage(padding)?;
                        assert_eq!(view.budget.storage(), protected);
                        let before = (view.budget.work(), view.budget.storage());
                        let replay = view.visit_paths(accept as Visit).unwrap_err();
                        assert_eq!(resource(&error), resource(&replay));
                        assert_eq!((view.budget.work(), view.budget.storage()), before);
                    }
                    5 => {
                        let semantic = match view.owner {
                            OwnerRef::Direct(owner) => {
                                owner.source_semantic_kir().semantic().semantic()
                            }
                            OwnerRef::Erased(owner) => {
                                owner.original_source().semantic_ssa().source_semantic()
                            }
                        };
                        // Reach the original ArgumentView constructor with zero
                        // remaining work, not the outer schedule's query check.
                        let query_prefix = 1 + 16 + semantic.canonical_encoding().len();
                        let padding = work_limit - view.budget.work() - query_prefix - 1;
                        view.charge_work(padding)?;
                        let mut reached = false;
                        let first = view
                            .visit_paths(|_| {
                                reached = true;
                                Ok(())
                            })
                            .unwrap_err();
                        assert!(!reached);
                        assert!(
                            matches!(first, E::Resource(Resource::Work(error)) if error.limit() == work_limit && error.actual() > work_limit)
                        );
                        assert_eq!(view.budget.work(), work_limit);
                        assert_eq!(view.budget.storage(), protected);
                        let before = (view.budget.work(), view.budget.storage());
                        let replay = view
                            .visit_paths(|_| panic!("sticky source-plan work refusal"))
                            .unwrap_err();
                        assert_eq!(resource(&first), resource(&replay));
                        assert_eq!((view.budget.work(), view.budget.storage()), before);
                    }
                    6 => {
                        struct SelectedDrop;
                        impl Drop for SelectedDrop {
                            fn drop(&mut self) {
                                panic!("selected path capture drop")
                            }
                        }
                        let capture = SelectedDrop;
                        let stopped = catch_unwind(AssertUnwindSafe(|| {
                            view.visit_paths(move |_| {
                                let _keep = &capture;
                                Err(E::Resource(Resource::Arithmetic))
                            })
                        }));
                        let payload = stopped.unwrap_err();
                        assert_eq!(
                            payload.downcast_ref::<&str>(),
                            Some(&"selected path capture drop")
                        );
                        assert_eq!(view.budget.storage(), protected);
                        let before = (view.budget.work(), view.budget.storage());
                        assert!(matches!(
                            view.visit_paths(|_| panic!("lost prior resource")),
                            Err(E::Resource(Resource::Arithmetic))
                        ));
                        assert_eq!((view.budget.work(), view.budget.storage()), before);
                    }
                    _ => unreachable!(),
                }
                Ok(())
            })
        }));
        match mode {
            3 | 5 => assert!(matches!(result, Ok(Err(E::Resource(Resource::Work(_)))))),
            6 => assert!(matches!(result, Ok(Err(E::Resource(Resource::Arithmetic))))),
            4 => assert!(
                matches!(result, Ok(Err(E::Resource(Resource::Storage(error)))) if error.limit() == limit && error.actual() == limit + 1)
            ),
            _ => result.unwrap().unwrap(),
        }
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn entry_paths_independent_fixed_frames_have_exact_and_short_boundaries() {
    fn h<T>() -> usize {
        size_of::<T>() + 3 * size_of::<Result<T, E>>() + 2 * size_of::<Result<T, SourceError>>()
    }
    type V = for<'a> fn(Path<'a>) -> R<()>;
    let expected = [
        h::<V>(),
        align_of::<V>(),
        h::<Run<'_, '_, '_, V>>(),
        align_of::<Run<'_, '_, '_, V>>(),
        h::<AssertUnwindSafe<Run<'_, '_, '_, V>>>(),
        h::<RootCapture<'_, V>>(),
        h::<QueryCapture<'_, '_, '_, V>>(),
        align_of::<QueryCapture<'_, '_, '_, V>>(),
        h::<NodeCapture<'_, V>>(),
        h::<(&AdmittedInertSemanticMirV1, &Module)>(),
        h::<&Module>(),
        h::<&fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1>(),
        h::<&fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1>(),
        h::<&fe2o3_pliron::ProductionSemanticMirOwnerV1>(),
        h::<&fe2o3_pliron::ProductionSemanticSsaOwnerV1>(),
        h::<&fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12>(),
        h::<&AdmittedInertSemanticMirV1>(),
        h::<&[u8]>(),
        h::<&[SemanticFunctionIdV1]>(),
        h::<SemanticFunctionIdV1>(),
        h::<fe2o3_mir_model::semantic_mir_v1::SemanticKernelBodySelectionV1>(),
        h::<Option<fe2o3_mir_model::semantic_mir_v1::SemanticKernelBodySelectionV1>>(),
        h::<std::iter::Enumerate<std::slice::Iter<'_, Root>>>(),
        h::<(usize, &Root)>(),
        h::<Option<(usize, &Root)>>(),
        h::<(
            usize,
            &TypedDescriptorRootV1,
            &AdmittedInertSemanticMirV1,
            &mut Plan<'_, '_, '_>,
        )>(),
        h::<(
            usize,
            Root,
            &[Argument],
            &[Component],
            &AdmittedInertSemanticMirV1,
            &SourceNode<'_>,
        )>(),
        h::<Path<'_>>(),
        h::<Coverage>(),
        h::<Root>(),
        h::<Argument>(),
        h::<&Root>(),
        h::<&Argument>(),
        h::<&Component>(),
        h::<&SourceNode<'_>>(),
        h::<SourceNode<'_>>(),
        h::<SourceCoverage<'_>>(),
        h::<&[Root]>(),
        h::<&[Argument]>(),
        3 * h::<&[Component]>(),
        h::<Option<&Root>>(),
        h::<Option<&Argument>>(),
        h::<(&usize, &Root, &&Argument)>(),
        2 * h::<Option<&Component>>(),
        h::<Option<&[Component]>>(),
        h::<Option<&SemanticTypeDeclV1>>(),
        h::<&SemanticTypeDeclV1>(),
        h::<&[SemanticTypeDeclV1]>(),
        h::<SemanticTypeIdV1>(),
        h::<SemanticTypeIdentityV1>(),
        h::<SemanticLayoutIdentityV1>(),
        2 * h::<&[Projection]>(),
        h::<(SemanticLocalIdV1, &[Projection])>(),
        h::<Option<(SemanticLocalIdV1, &[Projection])>>(),
        h::<fe2o3_lower_mir_kernel::ProductionPhysicalArgumentV1<'_>>(),
        h::<
            Option<(
                fe2o3_lower_mir_kernel::ProductionPhysicalArgumentV1<'_>,
                bool,
            )>,
        >(),
        h::<(
            Coverage,
            usize,
            usize,
            Option<(
                fe2o3_lower_mir_kernel::ProductionPhysicalArgumentV1<'_>,
                bool,
            )>,
        )>(),
        h::<std::slice::Iter<'_, Component>>(),
        h::<(&usize, &Component)>(),
        h::<(
            &fe2o3_lower_mir_kernel::ProductionPhysicalArgumentV1<'_>,
            &bool,
            &&SemanticTypeDeclV1,
        )>(),
        h::<(&Root, &Option<E>)>(),
        h::<Option<E>>(),
        h::<&mut Option<E>>(),
        h::<&E>(),
        h::<&Resource>(),
        h::<Option<Resource>>(),
        h::<&Cell<Option<Resource>>>(),
        h::<&mut V>(),
        h::<&mut Plan<'_, '_, '_>>(),
        h::<(&mut Budget<'_>,)>(),
        h::<Option<usize>>(),
        h::<std::ops::Range<usize>>(),
        h::<Result<(), SourceError>>(),
        h::<&R<()>>(),
        h::<&Result<R<()>, Box<dyn std::any::Any + Send>>>(),
        h::<Result<R<()>, Box<dyn std::any::Any + Send>>>(),
        16 * h::<usize>(),
        4 * h::<bool>(),
    ]
    .into_iter()
    .sum::<usize>();
    assert_eq!(headers::<V>().unwrap(), expected);
    for short in [false, true] {
        let mut work = Work::new(0);
        let limit = 19 + expected - usize::from(short);
        let mut budget = Budget::new(&mut work, limit);
        budget.reserve_storage(19).unwrap();
        let result = budget.reserve_storage(headers::<V>().unwrap());
        if short {
            assert!(
                matches!(result, Err(Resource::Storage(error)) if error.limit() == limit && error.actual() == 19 + expected)
            );
            assert_eq!(budget.storage(), 19);
        } else {
            result.unwrap();
            budget.release_storage(expected).unwrap();
            assert_eq!(budget.storage(), 19);
        }
        assert_eq!(budget.work(), 0);
    }
}

#[test]
fn entry_paths_partition_bound_is_logarithmic_and_checked() {
    assert_eq!(node_work(0).unwrap(), 264);
    for count in [1usize, 2, 3, 4, 8, 16, 128, usize::MAX] {
        let expected = 256 + 8 * (usize::BITS as usize - count.leading_zeros() as usize + 1);
        assert_eq!(node_work(count).unwrap(), expected);
    }
}
