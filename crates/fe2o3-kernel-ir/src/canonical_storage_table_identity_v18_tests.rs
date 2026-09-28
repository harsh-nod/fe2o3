use super::*;
use crate::CanonicalKernelIrWorkBudgetV1 as Work;

fn independent_headers() -> usize {
    let [writer, _, writer_result] = crate::wire::storage_v18_tests::header_layout_premises();
    2 * writer
        + 2 * writer_result
        + 2 * size_of::<Result<usize, KernelIrEncodeError>>()
        + 2 * size_of::<Result<Vec<u8>, KernelIrEncodeError>>()
        + 2 * size_of::<Vec<u8>>()
        + size_of::<Scope<'_, '_>>()
        + 2 * size_of::<Result<Scope<'_, '_>, ResourceError>>()
        + 4 * size_of::<
            Result<CanonicalStorageTableIdentityV18, CanonicalKernelIrReplayAdmissionErrorV18>,
        >()
        + 2 * size_of::<Result<(), ResourceError>>()
        + 2 * size_of::<Result<usize, ResourceError>>()
        + 2 * size_of::<CanonicalStorageTableIdentityV18>()
        + 2 * size_of::<Sha256>()
        + 2 * 32
}
fn literal(scalar: bool) -> Vec<u8> {
    let mut bytes = if scalar { vec![1, 0, 0, 0] } else { vec![0; 4] };
    if scalar {
        bytes.extend_from_slice(&8_u64.to_le_bytes());
        bytes.extend_from_slice(&8_u32.to_le_bytes());
        bytes.extend_from_slice(&[1, 9]);
    }
    bytes
}
fn owner(scalar: bool) -> VerifiedCanonicalKernelIrModuleV18 {
    let module = if scalar {
        super::super::tests::small_module()
    } else {
        Module::new("m")
    };
    super::super::tests::admit(&module)
}
fn schedule(scalar: bool) -> (usize, usize, usize, usize) {
    let (wire, count, emit) = if scalar { (18, 6, 19) } else { (4, 1, 4) };
    let hash = 4 + 34 + 2 + 8 + wire;
    (wire, count, emit, hash)
}

#[test]
fn canonical_table_identity_uses_shared_row_bytes_and_independent_hash_domain() {
    assert_eq!(DOMAIN, b"FE2O3/CANONICAL-STORAGE-TABLE/V18\0");
    assert_eq!(DOMAIN.len(), 34);
    assert_eq!(headers().unwrap(), independent_headers());
    for scalar in [false, true] {
        let owner = owner(scalar);
        let bytes = literal(scalar);
        let (wire, count, emit, hash) = schedule(scalar);
        let mut work = Work::new(1 + count + emit + hash);
        let mut budget = Budget::new(&mut work, 13 + independent_headers() + wire);
        budget.reserve_storage(13).unwrap();
        let key = owner
            .storage_table_identity_with_budget_v18(&mut budget)
            .unwrap();
        let mut expected = Sha256::new();
        expected.update(34_u32.to_le_bytes());
        expected.update(b"FE2O3/CANONICAL-STORAGE-TABLE/V18\0");
        expected.update(1_u16.to_le_bytes());
        expected.update((wire as u64).to_le_bytes());
        expected.update(bytes);
        let digest: [u8; 32] = expected.finalize().into();
        assert_eq!(key.digest(), &digest);
        assert_eq!(key.encoded_length(), wire as u64);
        assert_eq!(budget.work(), 1 + count + emit + hash);
        assert_eq!(budget.storage(), 13);
        assert_eq!(budget.peak_storage(), 13 + independent_headers() + wire);
    }
}

#[test]
fn canonical_table_identity_survives_ordinary_module_changes_but_not_layout_changes() {
    let source = super::super::tests::fixture(crate::AddressSpace::Private);
    let before = super::super::tests::admit(&source);
    let mut changed = source.clone();
    changed.id = crate::ModuleId::new("changed");
    changed.functions[0].body.as_mut().unwrap().blocks[0].operations[1].kind =
        crate::OperationKind::Constant(crate::Constant::U64(99));
    let after = super::super::tests::admit(&changed);
    assert_ne!(before.identity(), after.identity());
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let first = before
        .storage_table_identity_with_budget_v18(&mut budget)
        .unwrap();
    let second = after
        .storage_table_identity_with_budget_v18(&mut budget)
        .unwrap();
    assert_eq!(first, second);
    assert!(!std::ptr::eq(before.module(), after.module()));
    let mut other = super::super::tests::small_module();
    let same_count = super::super::tests::admit(&other)
        .storage_table_identity_with_budget_v18(&mut budget)
        .unwrap();
    other.storage_layouts[0].kind = crate::StorageLayoutKindV1::Scalar(crate::ScalarType::I64);
    let other = super::super::tests::admit(&other);
    let different_scalar = other
        .storage_table_identity_with_budget_v18(&mut budget)
        .unwrap();
    assert_ne!(first, different_scalar);
    assert_ne!(same_count, different_scalar);
    assert_eq!(budget.storage(), 0);
}

#[test]
fn canonical_table_identity_exact_and_one_short_keep_real_prefixes_and_floors() {
    for scalar in [false, true] {
        let owner = owner(scalar);
        let (wire, count, emit, hash) = schedule(scalar);
        let exact = 1 + count + emit + hash;
        for (work_limit, storage_limit, work_prefix, denied_work, denied_storage) in [
            (
                exact - 1,
                independent_headers() + wire,
                1 + count + emit,
                Some(exact),
                None,
            ),
            (
                exact,
                independent_headers() + wire - 1,
                1 + count,
                None,
                Some(13 + independent_headers() + wire),
            ),
            (
                exact,
                independent_headers() - 1,
                1,
                None,
                Some(13 + independent_headers()),
            ),
            (0, independent_headers() + wire, 0, Some(1), None),
        ] {
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, 13 + storage_limit);
            budget.reserve_storage(13).unwrap();
            assert!(
                owner
                    .storage_table_identity_with_budget_v18(&mut budget)
                    .is_err()
            );
            assert_eq!(budget.work(), work_prefix);
            assert_eq!(budget.work_budget_v1().failed_work(), denied_work);
            assert_eq!(budget.failed_storage(), denied_storage);
            assert_eq!(budget.storage(), 13);
        }
    }
}

#[test]
fn canonical_table_identity_preserves_prior_denials_without_reusing_them_as_authority() {
    let owner = owner(true);
    let (wire, count, emit, hash) = schedule(true);
    let exact = 1 + count + emit + hash;
    let mut work = Work::new(5 + exact);
    work.charge_work(5).unwrap();
    assert!(work.charge_work(exact + 1).is_err());
    let mut budget = Budget::new(&mut work, 13 + independent_headers() + wire);
    budget.reserve_storage(13).unwrap();
    assert!(
        budget
            .reserve_storage(independent_headers() + wire + 1)
            .is_err()
    );
    owner
        .storage_table_identity_with_budget_v18(&mut budget)
        .unwrap();
    assert_eq!(budget.work(), 5 + exact);
    assert_eq!(budget.work_budget_v1().failed_work(), Some(6 + exact));
    assert_eq!(
        budget.failed_storage(),
        Some(14 + independent_headers() + wire)
    );
    assert_eq!(budget.storage(), 13);
}

#[test]
fn canonical_table_identity_covers_all_flat_row_kinds_and_survives_owner_readmission() {
    let mut source = Module::new("all-layouts");
    source.storage_layouts = crate::wire::storage_v18_tests::rows();
    for row in &mut source.storage_layouts {
        if let crate::StorageLayoutKindV1::Variants {
            encoding: crate::StorageVariantEncodingV1::Niche { .. },
            variants,
        } = &mut row.kind
        {
            for (index, variant) in variants.iter_mut().enumerate() {
                variant.discriminant = index as u128;
            }
        }
    }
    let before = super::super::tests::admit(&source);
    let mut work = Work::new(10_000_000);
    let mut budget = Budget::new(&mut work, 10_000_000);
    let (after, _) =
        VerifiedCanonicalKernelIrModuleV18::from_canonical_bytes_with_verification_budget_v18(
            before.canonical_bytes(),
            super::super::tests::LIMITS,
            &mut budget,
        )
        .unwrap();
    assert_eq!(
        before.module().storage_layouts,
        after.module().storage_layouts
    );
    assert_eq!(
        before
            .storage_table_identity_with_budget_v18(&mut budget)
            .unwrap(),
        after
            .storage_table_identity_with_budget_v18(&mut budget)
            .unwrap()
    );
    assert!(!std::ptr::eq(before.module(), after.module()));
    assert_eq!(budget.storage(), 0);
}
