#[derive(Clone, Copy, Debug)]
pub(crate) enum Mode {
    Success,
    ExactStorage,
    ShortStorage,
    WorkRefusal,
    ForeignEntry,
    RestoredFloor,
}

fn independent_headers() -> (usize, usize) {
    type Capture<'a, 'view, 'source, 'work> = (
        &'view Source<'source>,
        &'a Handoff<'view, 'source>,
        TargetProfile,
        &'a mut Budget<'work>,
        &'a std::cell::Cell<usize>,
        usize,
        usize,
    );
    type Outcome = Result<(String, usize), ClosedScalarTargetLlvmErrorV29>;
    (
        size_of::<ClosedScalarTargetLlvmV29<'_, '_, '_>>()
            + align_of::<ClosedScalarTargetLlvmV29<'_, '_, '_>>(),
        size_of::<Capture<'_, '_, '_, '_>>()
            + align_of::<Capture<'_, '_, '_, '_>>()
            + size_of::<AssertUnwindSafe<Capture<'_, '_, '_, '_>>>()
            + size_of::<Outcome>()
            + align_of::<Outcome>()
            + size_of::<std::thread::Result<Outcome>>()
            + size_of::<
                Result<
                    fe2o3_kernel_ir::FormalMemoryObligationAnalysis,
                    fe2o3_kernel_ir::CanonicalClosedScalarFormalErrorV18,
                >,
            >()
            + size_of::<Result<String, fe2o3_amdgcn_model::LoweringErrors>>()
            + size_of::<Result<(), ClosedScalarTargetLlvmErrorV29>>()
            + size_of::<fe2o3_kernel_ir::FormalMemoryObligations>()
            + size_of::<[u64; 3]>()
            + size_of::<std::cell::Cell<usize>>()
            + size_of::<Result<(), SourceError>>()
            + align_of::<Result<(), SourceError>>()
            + size_of::<fe2o3_kernel_ir::CanonicalClosedScalarFormalScopeV18<'_>>()
            + size_of::<
                Result<
                    fe2o3_kernel_ir::CanonicalClosedScalarFormalScopeV18<'_>,
                    fe2o3_kernel_ir::CanonicalClosedScalarFormalErrorV18,
                >,
            >(),
    )
}

#[test]
fn target_result_full_owned_frames_have_an_independent_header_oracle() {
    assert_eq!(headers().unwrap(), independent_headers());
}

/// Called only by the actual-rustc parent, once per fresh authentic transaction.
/// It is not an independent fake-owner or inspection-only acceptance endpoint.
pub(crate) fn genuine_case(
    source: &Source<'_>,
    handoff: &Handoff<'_, '_>,
    target: TargetProfile,
    budget: &mut Budget<'_>,
    mode: Mode,
) -> Result<(), ClosedScalarTargetLlvmErrorV29> {
    let floor = budget.storage();
    if matches!(mode, Mode::ForeignEntry) {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(500_000_000);
        let mut foreign = Budget::new(&mut work, budget.storage_limit());
        foreign.reserve_storage(floor)?;
        let before = (foreign.work(), foreign.storage(), foreign.peak_storage());
        let error = check_and_lower_target_llvm_v18(source, handoff, target, &mut foreign)
            .err()
            .unwrap();
        assert!(matches!(
            &error,
            Error::Source(SourceError::Resource(Resource::Accounting))
        ));
        assert_eq!(
            (foreign.work(), foreign.storage(), foreign.peak_storage()),
            before
        );
        assert_eq!(budget.storage(), floor);
        return Err(error);
    }
    if matches!(mode, Mode::WorkRefusal) {
        let limit = super::super::WORK_LIMIT;
        // Exactly the genuine source_ssa + check_original_source query prefix
        // fits. The next source.canonical debit fails after accepted headers.
        budget.charge_work(limit - budget.work() - 2)?;
        let error = check_and_lower_target_llvm_v18(source, handoff, target, budget)
            .err()
            .unwrap();
        assert!(
            matches!(&error, Error::Source(SourceError::Resource(Resource::Work(e))) if e.actual() == limit + 1 && e.limit() == limit)
        );
        assert_eq!(budget.work(), limit);
        assert_eq!(budget.storage(), floor);
        return Err(error);
    }
    let native = check_and_lower_target_llvm_v18(source, handoff, target, budget)?;
    let (wrapper, frames) = independent_headers();
    let retained = wrapper + frames + native.llvm_ir.capacity();
    assert_eq!(native.retained_storage(budget)?, retained);
    assert_eq!(budget.storage(), floor + retained);
    assert_eq!(native.target(budget)?, target);
    assert!(native.llvm_ir(budget)?.contains("kir-version:18"));
    assert!(!native.llvm_ir(budget)?.contains("kir-version:12"));
    if matches!(mode, Mode::RestoredFloor) {
        budget.release_storage(1)?;
        assert!(budget.storage() >= floor, "base handoff remains paid");
        assert!(matches!(
            native.llvm_ir(budget),
            Err(SourceError::Resource(Resource::Accounting))
        ));
        budget.reserve_storage(1)?;
        let before = budget.storage();
        let error = native.discard(budget).unwrap_err();
        assert!(matches!(error, SourceError::Resource(Resource::Accounting)));
        assert_eq!(
            budget.storage(),
            before,
            "linked denial is sticky after restoring the byte"
        );
        return Err(error.into());
    }
    native.discard(budget)?;
    assert_eq!(budget.storage(), floor);
    if matches!(mode, Mode::Success) {
        let native = check_and_lower_target_llvm_v18(source, handoff, target, budget)?;
        let paid = budget.storage();
        let retained = native.retained_storage(budget)?;
        drop(native);
        assert_eq!(
            budget.storage(),
            paid,
            "ordinary Drop refunds no canonical credit"
        );
        budget.release_storage(retained)?; // Test now owns the dead concrete receipt.
        assert_eq!(budget.storage(), floor);
        return Ok(());
    }
    assert!(matches!(mode, Mode::ExactStorage | Mode::ShortStorage));
    let short = usize::from(matches!(mode, Mode::ShortStorage));
    let limit = budget.storage_limit();
    let filler = limit - retained + short - floor;
    budget.reserve_storage(filler)?;
    let result = check_and_lower_target_llvm_v18(source, handoff, target, budget);
    if short == 0 {
        let native = result?;
        assert_eq!(budget.storage(), limit);
        assert_eq!(budget.peak_storage(), limit);
        assert_eq!(native.retained_storage(budget)?, retained);
        native.discard(budget)?;
        assert_eq!(budget.storage(), floor + filler);
        budget.release_storage(filler)?;
        assert_eq!(budget.storage(), floor);
        Ok(())
    } else {
        let error = result
            .err()
            .expect("one byte short of genuine actual text capacity must refuse");
        assert!(
            matches!(&error, Error::Source(SourceError::Resource(Resource::Storage(e))) if e.actual() == limit + 1 && e.limit() == limit)
        );
        assert_eq!(budget.failed_storage(), Some(limit + 1));
        assert_eq!(
            budget.storage(),
            floor + filler,
            "refund accepted header only, not refused text"
        );
        budget.release_storage(filler)?;
        assert_eq!(budget.storage(), floor);
        Err(error)
    }
}
