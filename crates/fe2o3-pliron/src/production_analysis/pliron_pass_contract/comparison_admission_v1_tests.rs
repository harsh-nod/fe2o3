struct CostedIdentityProviderV1 {
    inner: ScriptedIdentityProviderV1,
    comparisons: Rc<Cell<usize>>,
    capture_work_limit: Rc<Cell<usize>>,
    captures: usize,
    post_capture_epoch_reads: Rc<Cell<usize>>,
}

impl PlironStructuralIdentityProviderV1 for CostedIdentityProviderV1 {
    type Snapshot = Vec<u8>;

    fn mutation_epoch(&self) -> Result<u64, MutationEpochCaptureFailureV1> {
        if self.captures > 1 {
            self.post_capture_epoch_reads
                .set(self.post_capture_epoch_reads.get() + 1);
        }
        self.inner.mutation_epoch()
    }

    fn capture_with_resource_limits_v1(
        &mut self,
        limits: ProductionAnalysisResourceLimitsV1,
    ) -> Result<BoundedPlironIdentityCaptureV1<Self::Snapshot>, IdentityCaptureFailureV1> {
        let phase = ProductionAnalysisResourcePhaseV1::StructuralIdentity;
        self.captures += 1;
        self.capture_work_limit.set(limits.max_work());
        let work = ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, 10, 0, 0)
            .map_err(IdentityCaptureFailureV1::ResourceLimit)?;
        limits
            .require(phase, work)
            .map_err(IdentityCaptureFailureV1::ResourceLimit)?;
        let mut capture = self.inner.capture_with_resource_limits_v1(limits)?;
        let bound = ProductionAnalysisResourceUpperBoundV1::checked_phase(
            phase,
            10,
            capture.resource_upper_bound.retained_storage_upper_bound(),
            0,
        )
        .map_err(IdentityCaptureFailureV1::ResourceLimit)?;
        limits
            .require(phase, bound)
            .map_err(IdentityCaptureFailureV1::ResourceLimit)?;
        capture.resource_upper_bound = bound;
        Ok(capture)
    }

    fn label(&self, snapshot: &Self::Snapshot) -> PlironStructuralIdentityLabelV1 {
        self.inner.label(snapshot)
    }

    fn require_exact_identity(
        &self,
        expected: &Self::Snapshot,
        observed: &Self::Snapshot,
    ) -> Result<(), IdentityComparisonFailureV1> {
        self.comparisons.set(self.comparisons.get() + 1);
        self.inner.require_exact_identity(expected, observed)
    }

    fn retain_exact_identity(&self, snapshot: Self::Snapshot) -> Arc<[u8]> {
        self.inner.retain_exact_identity(snapshot)
    }
}

#[test]
fn comparison_admission_allows_capture_work_larger_than_retained_storage() {
    // Capture W10/R4 plus comparison W(4+4+1) gives W19/R5/P9.
    for (work, peak, succeeds) in [(19, 9, true), (18, 9, false), (19, 8, false)] {
        let comparisons = Rc::new(Cell::new(0));
        let capture_work_limit = Rc::new(Cell::new(0));
        let post_capture_epoch_reads = Rc::new(Cell::new(0));
        let provider = CostedIdentityProviderV1 {
            inner: provider(&[1, 1]),
            comparisons: Rc::clone(&comparisons),
            capture_work_limit: Rc::clone(&capture_work_limit),
            captures: 0,
            post_capture_epoch_reads: Rc::clone(&post_capture_epoch_reads),
        };
        let mut session = begin_production_pliron_pass_contract_session_v1(provider).unwrap();
        let result = session.run_contiguous_pass_with_resource_limits_v1(
            KernelCheckPassKindV1::TensorLayout,
            ProductionAnalysisResourceLimitsV1::new(work, peak),
            || Ok::<_, ()>(()),
        );
        if succeeds {
            result
                .expect("capture work need not equal retained storage")
                .unwrap();
            let bound = session.last_checkpoint_resource_upper_bound_v1().unwrap();
            assert_eq!(bound.work_upper_bound(), 19);
            assert_eq!(bound.retained_storage_upper_bound(), 5);
            assert_eq!(bound.peak_storage_upper_bound(), 9);
            assert_eq!(comparisons.get(), 1);
            assert_eq!(session.certificates.len(), 1);
            assert_eq!(post_capture_epoch_reads.get(), 1);
        } else {
            assert_eq!(
                result,
                Err(PlironPassPreservationErrorV1::ResourceLimit {
                    resource: if work < 19 {
                        "work upper bound"
                    } else {
                        "peak storage upper bound"
                    },
                })
            );
            assert_eq!(
                comparisons.get(),
                0,
                "unaffordable comparison must not execute"
            );
            assert!(session.certificates.is_empty());
            assert!(session.last_checkpoint_resource_upper_bound_v1().is_none());
            assert!(session.last_checkpoint().is_err());
            assert_eq!(post_capture_epoch_reads.get(), 0);
        }
        assert_eq!(capture_work_limit.get(), work - 5);
    }
}

#[test]
fn comparison_admission_checks_larger_output_cost_before_comparison() {
    // A different-size output must not be budgeted as if identity were proven.
    // Capture W10/R8 plus comparison W(4+8+1) gives W23/R9/P13.
    for (work, admits_comparison) in [(22, false), (23, true)] {
        let comparisons = Rc::new(Cell::new(0));
        let post_capture_epoch_reads = Rc::new(Cell::new(0));
        let mut inner = provider(&[]);
        inner.snapshots = [Ok(vec![1; 4]), Ok(vec![2; 8])].into_iter().collect();
        let provider = CostedIdentityProviderV1 {
            inner,
            comparisons: Rc::clone(&comparisons),
            capture_work_limit: Rc::new(Cell::new(0)),
            captures: 0,
            post_capture_epoch_reads: Rc::clone(&post_capture_epoch_reads),
        };
        let mut session = begin_production_pliron_pass_contract_session_v1(provider).unwrap();
        let result = session.run_contiguous_pass_with_resource_limits_v1(
            KernelCheckPassKindV1::TensorLayout,
            ProductionAnalysisResourceLimitsV1::new(work, 13),
            || Ok::<_, ()>(()),
        );
        if admits_comparison {
            assert!(
                matches!(
                    result,
                    Err(PlironPassPreservationErrorV1::StructuralIdentityChanged {
                        pass: KernelCheckPassKindV1::TensorLayout,
                        source_code: "FE2O3-PRESERVE-010",
                        ..
                    })
                ),
                "{result:?}"
            );
            assert_eq!(comparisons.get(), 1);
            assert_eq!(post_capture_epoch_reads.get(), 1);
        } else {
            assert_eq!(
                result,
                Err(PlironPassPreservationErrorV1::ResourceLimit {
                    resource: "work upper bound",
                })
            );
            assert_eq!(comparisons.get(), 0);
            assert_eq!(post_capture_epoch_reads.get(), 0);
        }
        assert!(session.certificates.is_empty());
        assert!(session.last_checkpoint_resource_upper_bound_v1().is_none());
        assert!(session.last_checkpoint().is_err());
        assert_eq!(session.next, 0);
    }
}

#[test]
fn comparison_admission_later_denial_preserves_only_the_prior_checkpoint() {
    let provider = CostedIdentityProviderV1 {
        inner: provider(&[1, 1, 1]),
        comparisons: Rc::new(Cell::new(0)),
        capture_work_limit: Rc::new(Cell::new(0)),
        captures: 0,
        post_capture_epoch_reads: Rc::new(Cell::new(0)),
    };
    let mut session = begin_production_pliron_pass_contract_session_v1(provider).unwrap();
    session
        .run_contiguous_pass_with_resource_limits_v1(
            KernelCheckPassKindV1::TensorLayout,
            ProductionAnalysisResourceLimitsV1::new(19, 9),
            || Ok::<_, ()>(()),
        )
        .unwrap()
        .unwrap();
    let before = session.last_checkpoint().unwrap();
    let previous_bound = session.last_checkpoint_resource_upper_bound_v1();
    let result = session.run_contiguous_pass_with_resource_limits_v1(
        KernelCheckPassKindV1::MemoryBounds,
        ProductionAnalysisResourceLimitsV1::new(18, 9),
        || Ok::<_, ()>(()),
    );
    assert_eq!(
        result,
        Err(PlironPassPreservationErrorV1::ResourceLimit {
            resource: "work upper bound",
        })
    );
    assert_eq!(session.next, 1);
    assert_eq!(session.certificates.len(), 1);
    assert!(session.pending.is_none());
    assert!(session.lineage.is_none());
    assert_eq!(session.lineage_identity, before.identity());
    assert_eq!(
        session.last_checkpoint_resource_upper_bound_v1(),
        previous_bound
    );
    let after = session.last_checkpoint().unwrap();
    assert_eq!(after.position(), before.position());
    assert_eq!(after.pass(), before.pass());
    assert_eq!(after.identity(), before.identity());
    assert_eq!(after.mutation_epoch(), before.mutation_epoch());
    assert!(Arc::ptr_eq(&after.custody, &before.custody));
    let called = Cell::new(false);
    let retry = session.run_contiguous_pass(KernelCheckPassKindV1::MemoryBounds, || {
        called.set(true);
        Ok::<_, ()>(())
    });
    assert!(matches!(
        retry,
        Err(PlironPassPreservationErrorV1::InvalidSessionState {
            detail: "the prior identity snapshot is absent",
        })
    ));
    assert!(!called.get());
    assert_eq!(session.next, 1);
    assert_eq!(session.certificates.len(), 1);
    assert_eq!(
        session.last_checkpoint().unwrap().identity(),
        before.identity()
    );
}
