use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

#[test]
fn expanded_transport_header_preserves_one_source_backing_and_one_module_header() {
    assert_eq!(
        header_addition().unwrap() + size_of::<Input>(),
        size_of::<Output>()
    );
    let module_storage = size_of::<Module>() + 123;
    assert_eq!(
        retained_addition(module_storage).unwrap(),
        header_addition().unwrap() + 123
    );
    assert!(matches!(
        retained_addition(size_of::<Module>() - 1),
        Err(E::Resource(Resource::Accounting))
    ));
    assert!(matches!(
        retained_addition(usize::MAX),
        Err(E::Resource(Resource::Arithmetic))
    ));
    assert_eq!(MAX_SOURCE_CAPACITY, 2 * MAX_DESCRIPTOR_TABLE_BYTES);
}

#[test]
fn expanded_transport_errors_preserve_source_module_and_catalog_types() {
    assert!(
        E::Resource(Resource::Accounting)
            .source()
            .unwrap()
            .is::<Resource>()
    );
    assert!(
        E::Expanded(descriptor::E::Panicked)
            .source()
            .unwrap()
            .is::<descriptor::E>()
    );
    assert!(
        E::Module(module::NominalModuleErrorV3::Resource(Resource::Arithmetic))
            .source()
            .unwrap()
            .is::<module::NominalModuleErrorV3>()
    );
    assert!(
        E::Catalog(SourcePipelineCatalogCallbackErrorV1::Callback(
            NativeError::Invalid("test"),
        ))
        .source()
        .unwrap()
        .is::<SourcePipelineCatalogCallbackErrorV1<NativeError>>()
    );
    assert!(E::Mismatch("test").source().is_none());
    assert!(E::Panicked.source().is_none());
}

#[test]
fn expanded_transport_scope_work_and_storage_order_is_independently_bounded() {
    let guard = descriptor::SCOPE + size_of::<R<()>>();
    for (work_limit, storage_limit, accepted, peak, kind) in [
        (20, 31 + guard + 7, 20, 31 + guard + 7, 0),
        (19, 31 + guard + 7, 17, 31 + guard, 1),
        (20, 31 + guard + 6, 20, 31 + guard, 2),
    ] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.charge_work(17).unwrap();
        budget.reserve_storage(31).unwrap();
        let result: R<()> = scoped(&mut budget, |budget| {
            budget.charge_work(3)?;
            budget.reserve_storage(7)?;
            Ok(())
        });
        match kind {
            0 => result.unwrap(),
            1 => assert!(matches!(result, Err(E::Resource(Resource::Work(_))))),
            2 => assert!(matches!(result, Err(E::Resource(Resource::Storage(_))))),
            _ => unreachable!(),
        }
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            (accepted, 31, peak)
        );
        assert_eq!(
            budget.failed_storage(),
            (kind == 2).then_some(31 + guard + 7)
        );
    }
}

#[test]
fn expanded_transport_scope_rejects_foreign_ledger_without_refunding_it() {
    let mut original = Work::new(4096);
    let mut foreign = Work::new(4096);
    let mut budget = Budget::new(&mut original, 4096);
    let mut other = Budget::new(&mut foreign, 4096);
    budget.reserve_storage(31).unwrap();
    other.reserve_storage(53).unwrap();
    let original_identity = budget.work_ledger_identity_v1();
    let foreign_identity = other.work_ledger_identity_v1();
    let result: R<()> = scoped(&mut budget, |b| {
        b.charge_work(3)?;
        std::mem::swap(b, &mut other);
        Ok(())
    });
    assert!(matches!(result, Err(E::Resource(Resource::Accounting))));
    assert!(budget.work_ledger_identity_v1() == foreign_identity);
    assert!(other.work_ledger_identity_v1() == original_identity);
    assert_eq!(budget.storage(), 53);
    assert_eq!(other.storage(), 31 + descriptor::SCOPE + size_of::<R<()>>());
    std::mem::swap(&mut budget, &mut other);
    budget
        .release_storage(descriptor::SCOPE + size_of::<R<()>>())
        .unwrap();
    assert_eq!(budget.storage(), 31);
    assert_eq!(other.storage(), 53);
}

impl Output {
    pub(crate) fn expanded_transport_hostiles_v3(&mut self, budget: &mut Budget<'_>) -> R<()> {
        let floor = budget.storage();
        self.module_storage += 1;
        let result = self.verify_equivalence(budget);
        self.module_storage -= 1;
        assert!(matches!(
            result,
            Err(E::Mismatch("exact cumulative expanded native receipt"))
        ));
        let old = self.module.replace_first_ascii_byte_for_test_v3(b'!');
        let result = self.verify_equivalence(budget);
        self.module.replace_first_ascii_byte_for_test_v3(old);
        assert!(matches!(
            result,
            Err(E::Catalog(SourcePipelineCatalogCallbackErrorV1::Callback(
                NativeError::Invalid(_)
            )))
        ));
        assert_eq!(budget.storage(), floor);
        self.verify_equivalence(budget)
    }
}
