use super::*;

const LIMIT: usize = 20_000_000;

fn with_binding(
    consume: impl FnOnce(
        &SourceReferencePlanV29<'_, '_>,
        &SemanticSourceReferenceBindingV29,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let owner = owner(Case::Shared);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        ROOT,
        &mut budget,
        |instances, budget| {
            let floor = budget.storage();
            let result = with_source_reference_plan_v29(instances, budget, |plan, budget| {
                let references = SourceReferenceEmissionV29::new(plan, budget)?;
                let binding =
                    capture_binding(&references, capture_instance(plan, 0), ValueId(711), budget);
                let SemanticValueBindingV1::Aggregate(fields) = binding else {
                    panic!("capture")
                };
                let SemanticValueBindingV1::SourceReference(binding) = &fields[0] else {
                    panic!("reference")
                };
                consume(plan, binding, budget)
            });
            assert!(budget.storage() >= floor);
            budget.release_storage(budget.storage() - floor).unwrap();
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(result)
        },
    )
    .unwrap()
}

#[derive(Clone, Copy)]
enum Fault {
    None,
    Panic,
    Floor,
    Slot,
    Ledger(fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1),
}

struct Meter<'a, 'work> {
    original: &'a mut ArgumentBudgetV1<'work>,
    fault: Fault,
    at: usize,
    calls: usize,
    required: Option<usize>,
    peak: usize,
    fired: bool,
}

impl<'a, 'work> Meter<'a, 'work> {
    fn new(original: &'a mut ArgumentBudgetV1<'work>, fault: Fault, at: usize) -> Self {
        let peak = original.storage();
        Self {
            original,
            fault,
            at,
            calls: 0,
            required: None,
            peak,
            fired: false,
        }
    }
}

impl SemanticEmissionBudgetV1 for Meter<'_, '_> {
    fn work_ledger_identity_v1(&self) -> fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 {
        if self.fired
            && let Fault::Ledger(ledger) = self.fault
        {
            return ledger;
        }
        self.original.work_ledger_identity_v1()
    }
    fn charge_work(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        self.original.charge_work(amount)?;
        self.calls += 1;
        if self.calls == self.at {
            self.fired = true;
            match self.fault {
                Fault::Panic => panic!("validation scratch panic"),
                Fault::Floor => {
                    let retained = self.required.unwrap() - 1;
                    self.original
                        .release_storage(self.original.storage() - retained)?;
                }
                Fault::None | Fault::Slot | Fault::Ledger(_) => {}
            }
        }
        Ok(())
    }
    fn reserve_storage(&mut self, bytes: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        self.original.reserve_storage(bytes)?;
        self.required.get_or_insert(self.original.storage());
        self.peak = self.peak.max(self.original.storage());
        Ok(())
    }
    fn release_storage(&mut self, bytes: usize) -> Result<(), ProductionSemanticKirErrorV1> {
        Ok(self.original.release_storage(bytes)?)
    }
    fn storage(&self) -> usize {
        self.original.storage()
    }
    fn prepared_input_slot_v1(&self) -> Option<usize> {
        let original = SemanticEmissionBudgetV1::prepared_input_slot_v1(self.original);
        if self.fired && matches!(self.fault, Fault::Slot) {
            original.map(|slot| slot ^ 1)
        } else {
            original
        }
    }
    fn permits_prepared_input_refund_v1(
        &self,
        source: Option<&SourceReferencePlanV29<'_, '_>>,
        slot: usize,
        ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
        required: usize,
        bytes: usize,
    ) -> bool {
        self.prepared_input_slot_v1() == Some(slot)
            && self.work_ledger_identity_v1() == ledger
            && SemanticEmissionBudgetV1::permits_prepared_input_refund_v1(
                self.original,
                source,
                slot,
                ledger,
                required,
                bytes,
            )
    }
    fn source_reference_owner_v29(
        &mut self,
        plan: &SourceReferencePlanV29<'_, '_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        SemanticEmissionBudgetV1::source_reference_owner_v29(self.original, plan)
    }
    fn source_reference_representation_v29(
        &mut self,
        plan: &SourceReferencePlanV29<'_, '_>,
        loan: usize,
    ) -> Result<SourceReferenceRepresentationV29, ProductionSemanticKirErrorV1> {
        SemanticEmissionBudgetV1::source_reference_representation_v29(self.original, plan, loan)
    }
    fn source_reference_scalar_cell_v29(
        &mut self,
        plan: &SourceReferencePlanV29<'_, '_>,
        loan: usize,
    ) -> Result<Option<(usize, SourceReferenceScalarCellV29)>, ProductionSemanticKirErrorV1> {
        SemanticEmissionBudgetV1::source_reference_scalar_cell_v29(self.original, plan, loan)
    }
}

fn observe(
    plan: &SourceReferencePlanV29<'_, '_>,
    binding: &SemanticSourceReferenceBindingV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(usize, usize, usize), ProductionSemanticKirErrorV1> {
    let floor = budget.storage();
    let work = budget.work();
    let mut meter = Meter::new(budget, Fault::None, usize::MAX);
    source_reference_validate_binding_v29(plan, binding, &mut meter)?;
    assert_eq!(meter.storage(), floor);
    Ok((
        meter.original.work() - work,
        meter.peak - floor,
        meter.calls,
    ))
}

#[test]
fn reference_validation_repeats_without_retaining_query_storage() {
    with_binding(|plan, binding, budget| {
        let floor = budget.storage();
        let first = observe(plan, binding, budget)?;
        assert!(first.1 > source_reference_validation_headers_v29()?);
        for _ in 0..32 {
            assert_eq!(observe(plan, binding, budget)?, first);
        }
        assert_eq!(budget.storage(), floor);
        assert_eq!(
            binding.values,
            [ValueDef::new(ValueId(711), Type::Scalar(ScalarType::U64))]
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn reference_validation_owner_refusal_precedes_new_headers() {
    for fault in 0..4 {
        with_binding(|plan, binding, budget| {
            let mut wrong = binding.clone();
            match fault {
                0 => wrong.owner ^= 1,
                1 => wrong.source[0] ^= 1,
                2 => {
                    wrong.root = plan
                        .instances
                        .id_at(plan.instances.instances().len() - 1)
                        .unwrap()
                }
                _ => wrong.source_type = UNIT,
            }
            let floor = budget.storage();
            let work = budget.work();
            let result = source_reference_validate_binding_v29(plan, &wrong, budget);
            assert!(
                matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::Unsupported {
                        detail: "source reference binding belongs to another owner",
                        ..
                    })
                ),
                "{result:?}"
            );
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.work() - work, 18);
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn reference_validation_wrong_payload_releases_only_query_credit() {
    for fault in 0..3 {
        with_binding(|plan, binding, budget| {
            let mut wrong = binding.clone();
            match fault {
                0 => wrong.values.clear(),
                1 => wrong.values[0].ty = Type::Scalar(ScalarType::U32),
                _ => wrong.values.push(wrong.values[0].clone()),
            }
            let floor = budget.storage();
            let result = source_reference_validate_binding_v29(plan, &wrong, budget);
            assert!(
                matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::Unsupported {
                        detail: "source reference physical payload differs",
                        ..
                    })
                ),
                "{result:?}"
            );
            assert_eq!(budget.storage(), floor);
            source_reference_validate_binding_v29(plan, binding, budget)
        })
        .unwrap();
    }
}

#[test]
fn reference_validation_exact_and_short_query_limits_keep_first_resource() {
    for storage in [false, true] {
        for short in [false, true] {
            let result = with_binding(|plan, binding, budget| {
                let (work, peak, _) = observe(plan, binding, budget)?;
                let floor = budget.storage();
                let allowed = if storage { peak } else { work } - usize::from(short);
                let padding = LIMIT - allowed - if storage { floor } else { budget.work() };
                if storage {
                    budget.reserve_storage(padding)?;
                } else {
                    budget.charge_work(padding)?;
                }
                let required = budget.storage();
                let result = source_reference_validate_binding_v29(plan, binding, budget);
                assert_eq!(budget.storage(), required);
                if short {
                    let error = result.unwrap_err();
                    assert!(
                        match &error {
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Storage(error),
                            ) => storage && error.limit() == LIMIT && error.actual() == LIMIT + 1,
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Work(error),
                            ) => !storage && error.limit() == LIMIT && error.actual() == LIMIT + 1,
                            _ => false,
                        },
                        "{error:?}"
                    );
                    let before = (budget.work(), budget.storage());
                    let replay =
                        source_reference_validate_binding_v29(plan, binding, budget).unwrap_err();
                    assert_eq!(format!("{replay:?}"), format!("{error:?}"));
                    assert_eq!((budget.work(), budget.storage()), before);
                } else {
                    result?;
                }
                if storage {
                    budget.release_storage(padding)?;
                }
                assert_eq!(budget.storage(), floor);
                Ok(())
            });
            if short {
                assert!(matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Storage(_) | ArgumentResourceV1::Work(_)
                        )
                    )
                ));
            } else {
                result.unwrap();
            }
        }
    }
}

#[test]
fn reference_validation_unwind_and_changed_custody_keep_exact_cleanup() {
    for mode in 0..4 {
        let result = with_binding(|plan, binding, budget| {
            let (_, _, calls) = observe(plan, binding, budget)?;
            let floor = budget.storage();
            let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let foreign = ArgumentBudgetV1::new(&mut foreign_work, LIMIT);
            let fault = match mode {
                0 => Fault::Panic,
                1 => Fault::Floor,
                2 => Fault::Slot,
                _ => Fault::Ledger(foreign.work_ledger_identity_v1()),
            };
            let mut meter = Meter::new(budget, fault, calls);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                source_reference_validate_binding_v29(plan, binding, &mut meter)
            }));
            assert!(meter.fired);
            if mode == 0 {
                let panic = result.unwrap_err();
                assert_eq!(
                    panic.downcast_ref::<&str>(),
                    Some(&"validation scratch panic")
                );
                assert_eq!(meter.storage(), floor);
            } else {
                assert!(matches!(
                    result,
                    Ok(Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Accounting
                        )
                    ))
                ));
                assert!(
                    meter.storage() > floor,
                    "lost custody must deny the entire query refund"
                );
                if mode == 1 {
                    assert_eq!(meter.storage(), meter.required.unwrap() - 1);
                }
                let before = (meter.original.work(), meter.storage());
                assert!(source_reference_validate_binding_v29(plan, binding, &mut meter).is_err());
                assert_eq!((meter.original.work(), meter.storage()), before);
            }
            Ok(())
        });
        if mode == 0 {
            result.unwrap();
        } else {
            assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Accounting
                    )
                )
            ));
        }
    }
}

#[test]
fn reference_validation_foreign_ledger_is_no_debit_before_headers() {
    let result = with_binding(|plan, binding, _| {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
        let mut foreign = ArgumentBudgetV1::new(&mut work, LIMIT);
        foreign.reserve_storage(23)?;
        let result = source_reference_validate_binding_v29(plan, binding, &mut foreign);
        assert!(matches!(
            result,
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
        assert_eq!(
            (foreign.work(), foreign.storage(), foreign.peak_storage()),
            (0, 23, 23)
        );
        Ok(())
    });
    assert!(matches!(
        result,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Accounting
            )
        )
    ));
}

#[test]
fn reference_validation_fixed_headers_have_an_independent_equation() {
    fn h<T>() -> usize {
        std::mem::size_of::<T>()
            + 2 * std::mem::size_of::<Result<T, ProductionSemanticKirErrorV1>>()
    }
    type Args<'a, 'p, 's> = (
        &'a SourceReferencePlanV29<'p, 's>,
        &'a SemanticSourceReferenceBindingV29,
        &'a mut dyn SemanticEmissionBudgetV1,
    );
    type Capture<'a, 'p, 's> = (
        &'a SourceReferencePlanV29<'p, 's>,
        &'a SemanticSourceReferenceBindingV29,
        &'a mut dyn SemanticEmissionBudgetV1,
        Option<&'a source_storage_v29::SourceStorageRootCustodyViewV29<'p, 's>>,
        &'a mut Option<source_storage_v29::SourceStorageRootGrowthV29<'a, 'p, 's>>,
    );
    let expected = 4 * h::<Args<'_, '_, '_>>()
        + h::<Capture<'_, '_, '_>>()
        + std::mem::align_of::<Capture<'_, '_, '_>>()
        + h::<std::panic::AssertUnwindSafe<Capture<'_, '_, '_>>>()
        + h::<&SourceReferencePlanV29<'_, '_>>()
        + h::<&SemanticSourceReferenceBindingV29>()
        + h::<&mut dyn SemanticEmissionBudgetV1>()
        + h::<&source_storage_v29::SourceStorageRootCustodyViewV29<'_, '_>>()
        + h::<Option<&source_storage_v29::SourceStorageRootCustodyViewV29<'_, '_>>>()
        + h::<&Option<&source_storage_v29::SourceStorageRootCustodyViewV29<'_, '_>>>()
        + h::<source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>()
        + h::<Option<source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>>()
        + h::<&mut Option<source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>>()
        + h::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>()
        + 4 * h::<usize>()
        + h::<Option<usize>>()
        + h::<bool>()
        + h::<Result<(), ProductionSemanticKirErrorV1>>()
        + h::<std::thread::Result<Result<(), ProductionSemanticKirErrorV1>>>()
        + h::<&std::thread::Result<Result<(), ProductionSemanticKirErrorV1>>>()
        + h::<Box<dyn std::any::Any + Send>>()
        + h::<&ProductionSemanticKirErrorV1>()
        + h::<Option<ProductionSemanticKirErrorV1>>()
        + h::<(
            Option<&SourceReferencePlanV29<'_, '_>>,
            fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
            usize,
            usize,
            usize,
            Option<source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>,
            Option<usize>,
            &mut dyn SemanticEmissionBudgetV1,
        )>()
        + h::<Option<&SourceReferencePlanV29<'_, '_>>>()
        + h::<&source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>()
        + h::<Option<&source_storage_v29::SourceStorageRootGrowthV29<'_, '_, '_>>>()
        + h::<usize>()
        + h::<bool>();
    assert_eq!(source_reference_validation_headers_v29().unwrap(), expected);
    for short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
        let limit = 23 + expected - usize::from(short);
        let mut budget = ArgumentBudgetV1::new(&mut work, limit);
        budget.reserve_storage(23).unwrap();
        let result = budget.reserve_storage(source_reference_validation_headers_v29().unwrap());
        if short {
            assert!(
                matches!(result, Err(ArgumentResourceV1::Storage(error)) if error.actual() == 23 + expected && error.limit() == limit)
            );
        } else {
            result.unwrap();
            budget.release_storage(expected).unwrap();
        }
        assert_eq!((budget.work(), budget.storage()), (0, 23));
    }
}
