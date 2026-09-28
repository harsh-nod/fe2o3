use super::*;

#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct PreparationObservationV29 {
    pub(crate) kinds: Vec<String>,
    pub(crate) contexts: bool,
    pub(crate) materialized: bool,
}

thread_local! {
    static PREPARATION: std::cell::RefCell<Option<PreparationObservationV29>> = const { std::cell::RefCell::new(None) };
    static OBSERVE_PREPARATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub(crate) fn start_preparation_observation_v29() {
    OBSERVE_PREPARATION.with(|enabled| assert!(!enabled.replace(true)));
    PREPARATION.with_borrow_mut(|slot| *slot = None);
}

pub(crate) fn take_preparation_observation_v29() -> Option<PreparationObservationV29> {
    OBSERVE_PREPARATION.with(|enabled| enabled.set(false));
    PREPARATION.with_borrow_mut(Option::take)
}

pub(super) fn observe_prepared_source_v29(roots: &[AbiRoot<'_>], contexts: bool) {
    if !OBSERVE_PREPARATION.with(std::cell::Cell::get) {
        return;
    }
    use fe2o3_lower_mir_kernel::ProductionKernelArgumentAbiKindV18 as Kind;
    let kinds = roots
        .iter()
        .flat_map(|root| root.arguments.iter())
        .map(|argument| match &argument.kind {
            Kind::Descriptor { source, .. } => format!("{source:?}"),
            Kind::CompilerLaidOutByValue { .. } => "ByValue".to_owned(),
        })
        .collect();
    PREPARATION.with_borrow_mut(|slot| {
        *slot = Some(PreparationObservationV29 {
            kinds,
            contexts,
            materialized: false,
        })
    });
}

pub(super) fn observe_materialized_source_v29() {
    if !OBSERVE_PREPARATION.with(std::cell::Cell::get) {
        return;
    }
    PREPARATION.with_borrow_mut(|slot| {
        slot.as_mut()
            .expect("real prepared source precedes materialization")
            .materialized = true;
    });
}

#[test]
fn source_owned_context_preparation_capture_and_result_headers_are_independent() {
    type Capture<'a> = (
        fe2o3_pliron::ProductionSemanticSsaOwnerV1,
        fe2o3_lower_mir_kernel::ProductionSourceLaunchRosterV1,
        &'a [AbiRoot<'a>],
    );
    type Prepared = Result<
        fe2o3_lower_mir_kernel::ProductionPreparedSourceV18,
        ProductionSourceOwnedViewErrorV18,
    >;
    type Invoke<'a, 'work> = (
        Capture<'a>,
        &'a crate::collector::RetainedExecutionSourceV29<'a>,
        &'a mut Budget<'work>,
        &'a mut usize,
    );
    type Projected = Result<Prepared, fe2o3_lower_mir_kernel::ProductionContextRootErrorV29>;
    let expected = size_of::<Capture<'_>>()
        + align_of::<Capture<'_>>()
        + size_of::<Invoke<'_, '_>>()
        + align_of::<Invoke<'_, '_>>()
        + size_of::<AssertUnwindSafe<Invoke<'_, '_>>>()
        + size_of::<Prepared>()
        + align_of::<Prepared>()
        + size_of::<Projected>()
        + align_of::<Projected>()
        + size_of::<std::thread::Result<Projected>>()
        + size_of::<Option<crate::collector::RetainedExecutionSourceV29<'_>>>();
    assert_eq!(context_preparation_headers_v29().unwrap(), expected);
    for limit in [17 + expected - 1, 17 + expected] {
        let mut work = Work::new(expected);
        let mut budget = Budget::new(&mut work, limit);
        budget.reserve_storage(17).unwrap();
        let charged = pay_context_preparation_headers_v29(&mut budget);
        assert_eq!(charged.is_ok(), limit == 17 + expected);
        assert_eq!(budget.work(), expected);
        assert_eq!(
            budget.storage(),
            if limit == 17 + expected {
                17 + expected
            } else {
                17
            }
        );
        if limit < 17 + expected {
            assert!(
                matches!(charged, Err(Error::Resource(Resource::Storage(error))) if error.actual() == 17 + expected && error.limit() == limit)
            );
        }
    }
    let mut work = Work::new(expected - 1);
    let mut budget = Budget::new(&mut work, 17 + expected);
    budget.reserve_storage(17).unwrap();
    assert!(
        matches!(pay_context_preparation_headers_v29(&mut budget), Err(Error::Resource(Resource::Work(error))) if error.actual() == expected && error.limit() == expected - 1)
    );
    assert_eq!((budget.work(), budget.storage()), (0, 17));
}

impl<R> SourceOwnedCompilationContinuationV29<R> {
    pub(crate) fn assert_retained_bindings_for_test_v29(
        &self,
        expected: &fe2o3_pliron::ProductionSemanticSsaOwnerV1,
        expected_target: TargetProfile,
    ) {
        assert_eq!(&self.original_source, expected.source_semantic_sha256());
        assert_eq!(self.original_ssa, expected.identity());
        assert_eq!(
            self.bindings
                .rustc_preflight_plan
                .rustc_identity_inventory_sha256(),
            self.bindings.rustc_identity_inventory.sha256(),
        );
        let semantic = expected.source_semantic();
        assert_eq!(
            self.bindings.typed_descriptor_roots.len(),
            semantic.roots().len()
        );
        for (root, function) in self
            .bindings
            .typed_descriptor_roots
            .iter()
            .zip(semantic.roots())
        {
            let entry = semantic.functions()[function.index() as usize]
                .kernel_entry()
                .expect("retained original kernel root");
            assert_eq!(
                root.kernel_binding_bytes(),
                *entry.kernel_binding_identity().as_bytes()
            );
        }
        assert_eq!(self.bindings.rustc_target.profile(), expected_target);
        assert!(
            self.bindings
                .transaction
                .compiler_custody
                .is_extraction_only()
        );
        assert_eq!(
            self.bindings
                .transaction
                .compiler_custody
                .retained_protected_binding_count(),
            0
        );
    }
}

#[test]
fn source_owned_projection_storage_and_work_are_prepaid_at_exact_and_one_short_limits() {
    for limit in [56, 57] {
        let mut work = Work::new(8);
        let mut budget = Budget::new(&mut work, limit);
        budget.reserve_storage(17).unwrap();
        let result = paid_vec::<u64>(5, &mut budget);
        assert_eq!(budget.work(), 8);
        if limit == 57 {
            let rows = result.unwrap();
            assert_eq!(rows.capacity(), 5);
            assert_eq!(budget.storage(), 57);
            assert_eq!(budget.peak_storage(), 57);
        } else {
            assert!(
                matches!(result, Err(Error::Resource(Resource::Storage(error))) if error.actual() == 57 && error.limit() == 56)
            );
            assert_eq!(budget.storage(), 17);
            assert_eq!(budget.peak_storage(), 17);
            assert_eq!(budget.failed_storage(), Some(57));
        }
    }
    let mut work = Work::new(7);
    let mut budget = Budget::new(&mut work, 57);
    budget.reserve_storage(17).unwrap();
    assert!(
        matches!(paid_vec::<u64>(5, &mut budget), Err(Error::Resource(Resource::Work(error))) if error.actual() == 8 && error.limit() == 7)
    );
    assert_eq!((budget.work(), budget.storage()), (0, 17));
}

#[test]
fn source_owned_projection_overflow_refuses_before_work_or_storage_mutation() {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(17).unwrap();
    assert!(matches!(
        paid_vec::<u64>(usize::MAX, &mut budget),
        Err(Error::Resource(Resource::Arithmetic))
    ));
    assert!(matches!(
        paid_vec::<()>(usize::MAX, &mut budget),
        Err(Error::Resource(Resource::Arithmetic))
    ));
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (0, 17, 17)
    );
}

#[test]
fn source_owned_entry_header_pays_owned_aligned_capture_and_result_independently() {
    #[repr(align(256))]
    struct Large([u8; 16_384]);
    fn oracle<R, F>(_: &F) -> usize {
        type Invoke<'a, 's, 'w, F> = (
            F,
            &'a Source<'s>,
            &'a Handoff<'a, 's>,
            &'a [AbiRoot<'a>],
            TargetProfile,
            &'a mut Budget<'w>,
        );
        let expected = size_of::<F>()
            + align_of::<F>()
            + size_of::<AssertUnwindSafe<F>>()
            + size_of::<Invoke<'_, '_, '_, F>>()
            + align_of::<Invoke<'_, '_, '_, F>>()
            + size_of::<AssertUnwindSafe<Invoke<'_, '_, '_, F>>>()
            + size_of::<Result<R, Error>>()
            + align_of::<Result<R, Error>>()
            + size_of::<std::thread::Result<Result<R, Error>>>()
            + size_of::<AssertUnwindSafe<Result<R, Error>>>()
            + size_of::<SourceOwnedCompilationContinuationV29<R>>()
            + align_of::<SourceOwnedCompilationContinuationV29<R>>()
            + size_of::<Result<SourceOwnedCompilationContinuationV29<R>, Error>>()
            + align_of::<Result<SourceOwnedCompilationContinuationV29<R>, Error>>()
            + size_of::<PreparedSsaMaterializationV29>()
            + align_of::<PreparedSsaMaterializationV29>()
            + size_of::<Vec<Class>>()
            + size_of::<Vec<AbiRoot<'_>>>()
            + size_of::<ProductionKernelArgumentAbiInputV18<'_>>()
            + size_of::<ProductionExecutionSourceInputV29<'_>>()
            + size_of::<Work>()
            + size_of::<Budget<'_>>();
        assert_eq!(entry_headers::<R, F>().unwrap(), expected);
        expected
    }
    let empty = || ();
    let owned = Large([7; 16_384]);
    let large = move || owned;
    let small = oracle::<(), _>(&empty);
    let capture = oracle::<(), _>(&large);
    let result = oracle::<Large, _>(&empty);
    assert!(capture > small + 2 * size_of::<Large>());
    assert!(result > small + 2 * size_of::<Large>());
    assert_eq!(large().0[0], 7);
}
