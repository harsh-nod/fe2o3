use super::super::{
    AMDGPU_DIAGNOSTICS_CAPABILITY_NAME, AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE,
    AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME, AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE,
    target_capability_is_supported_with_budget_v1,
};
use super::*;
use crate::{
    AddressSpace, CanonicalKernelIrVerificationResourceBudgetV1,
    CanonicalKernelIrVerificationResourceErrorV1, CanonicalKernelIrWorkBudgetV1,
    SynchronizationScope, WaveWidth,
};

fn representative_owned_keys() -> Vec<TargetCapability> {
    let mut keys = vec![
        TargetCapability::Float16,
        TargetCapability::BFloat16,
        TargetCapability::Float64,
        TargetCapability::Int64,
        TargetCapability::Subgroups,
        TargetCapability::WorkgroupMemory,
        TargetCapability::WorkgroupBarrier,
        TargetCapability::DynamicWorkgroupMemory,
        TargetCapability::WaveWidth(WaveWidth::Wave32),
        TargetCapability::WaveWidth(WaveWidth::Wave64),
    ];
    for size in [0, 1, 32, 64, u32::MAX] {
        keys.push(TargetCapability::SubgroupSize(size));
    }
    for width_bits in [16, 32] {
        for address_space in [
            AddressSpace::Private,
            AddressSpace::Workgroup,
            AddressSpace::Global,
            AddressSpace::Constant,
            AddressSpace::Generic,
        ] {
            for max_scope in [
                SynchronizationScope::Invocation,
                SynchronizationScope::Subgroup,
                SynchronizationScope::Workgroup,
                SynchronizationScope::Device,
                SynchronizationScope::System,
            ] {
                keys.push(TargetCapability::Atomic {
                    width_bits,
                    address_space,
                    max_scope,
                });
            }
        }
    }
    for namespace in ["", "\0", "n", "namespace", "\u{e9}"] {
        for name in [
            "",
            "0",
            "00",
            "0A",
            "0a",
            "ab01",
            "ff",
            "ff0",
            "\u{80}\u{10ffff}",
        ] {
            keys.push(TargetCapability::Extension {
                namespace: namespace.to_owned(),
                name: name.to_owned(),
            });
        }
    }
    keys
}

#[test]
fn borrowed_key_order_matches_owned_ord_for_every_representative_pair() {
    let owned = representative_owned_keys();
    let hex_cases: &[(&[u8], &str)] = &[
        (&[], ""),
        (&[0], "00"),
        (&[10], "0a"),
        (&[0xab, 1], "ab01"),
        (&[0xff], "ff"),
        (&[0xff, 0], "ff00"),
    ];
    let mut views = owned
        .iter()
        .map(TargetCapabilityRefV1::from_owned)
        .collect::<Vec<_>>();
    for namespace in ["", "namespace", "\u{e9}"] {
        for &(bytes, spelling) in hex_cases {
            let view = TargetCapabilityRefV1::extension_lower_hex(namespace, bytes);
            let expected = TargetCapability::Extension {
                namespace: namespace.to_owned(),
                name: spelling.to_owned(),
            };
            assert_eq!(view.into_owned(), expected);
            assert!(view.matches(&expected));
            views.push(view);
        }
    }

    for left in &views {
        let left_key: &dyn CapabilityBorrowedKeyV1 = left;
        let left_owned = left.into_owned();
        for right in &views {
            let right_key: &dyn CapabilityBorrowedKeyV1 = right;
            let right_owned = right.into_owned();
            let expected = left_owned.cmp(&right_owned);
            assert_eq!(left_key.cmp(right_key), expected, "{left:?}, {right:?}");
            assert_eq!(left_key.partial_cmp(right_key), Some(expected));
            assert_eq!(left_key.eq(right_key), expected.is_eq());
            assert_eq!(left.matches(&right_owned), expected.is_eq());
            let stored_key: &dyn CapabilityBorrowedKeyV1 = Borrow::borrow(&right_owned);
            assert_eq!(left_key.cmp(stored_key), expected);
            assert_eq!(stored_key.cmp(left_key), expected.reverse());
        }
    }

    // Borrow is also coherent when the tree is populated in a different order.
    for reverse in [false, true] {
        let mut stored = owned.clone();
        if reverse {
            stored.reverse();
        }
        let supported: BTreeSet<_> = stored.into_iter().collect();
        for view in &views {
            assert_eq!(
                contains_v1(*view, &supported),
                supported.contains(&view.into_owned())
            );
        }
        for absent in [
            TargetCapabilityRefV1::SubgroupSize(3),
            TargetCapabilityRefV1::extension("namespace", "0a0"),
            TargetCapabilityRefV1::extension_lower_hex("missing", &[0xab, 1]),
        ] {
            assert!(!contains_v1(absent, &supported));
        }
    }
}

#[test]
fn repeated_fixed_and_extension_queries_keep_logarithmic_exact_work_without_storage() {
    const REPEATS: usize = 64;
    const WORK_PREFIX: usize = 7;
    const STORAGE_PREFIX: usize = 11;
    for (unrelated, comparisons) in [(128, 33), (1024, 44), (4096, 55)] {
        let mut supported = (0..unrelated)
            .map(|ordinal| TargetCapability::Extension {
                namespace: "noise".to_owned(),
                name: format!("{ordinal:08x}"),
            })
            .collect::<BTreeSet<_>>();
        supported.insert(TargetCapability::Extension {
            namespace: "target".to_owned(),
            name: "ab01".to_owned(),
        });
        supported.insert(TargetCapability::WaveWidth(WaveWidth::Wave64));
        assert_eq!(supported.len(), unrelated + 2);
        // Pinned 11-key nodes and minimum non-root occupancy5 give path-node
        // bounds3/4/5, independently of unrelated string lengths or placement.
        let hex_miss_classification = 2
            * (6_usize
                .max(AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE.len())
                .max(AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE.len())
                + 8_usize
                    .max(AMDGPU_DIAGNOSTICS_CAPABILITY_NAME.len())
                    .max(AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME.len())
                + 2);
        for (required, width, expected, classification) in [
            (
                TargetCapabilityRefV1::WaveWidth(WaveWidth::Wave64),
                4,
                true,
                0,
            ),
            (
                TargetCapabilityRefV1::WaveWidth(WaveWidth::Wave32),
                4,
                false,
                0,
            ),
            (
                TargetCapabilityRefV1::extension("target", "ab01"),
                6 + 4 + 3,
                true,
                0,
            ),
            (
                TargetCapabilityRefV1::extension_lower_hex("target", &[0xab, 1]),
                6 + 2 * 4 + 3,
                true,
                0,
            ),
            (
                TargetCapabilityRefV1::extension_lower_hex("target", &[0xab, 2]),
                6 + 2 * 4 + 3,
                false,
                hex_miss_classification,
            ),
        ] {
            let query_work = 1 + comparisons * width + classification;
            let exact = WORK_PREFIX + REPEATS * query_work;
            for limit in [exact, exact - 1] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
                work.charge_work(WORK_PREFIX).unwrap();
                let mut budget =
                    CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE_PREFIX);
                budget.reserve_storage(STORAGE_PREFIX).unwrap();
                for ordinal in 0..REPEATS {
                    let result = target_capability_is_supported_with_budget_v1(
                        required,
                        &supported,
                        &mut budget,
                    );
                    if limit < exact && ordinal == REPEATS - 1 {
                        assert!(matches!(
                            result,
                            Err(CanonicalKernelIrVerificationResourceErrorV1::Work(error))
                                if error.actual() == exact && error.limit() == exact - 1
                        ));
                        let last_charge = if classification == 0 {
                            comparisons * width
                        } else {
                            classification
                        };
                        assert_eq!(budget.work(), exact - last_charge);
                    } else {
                        assert_eq!(result, Ok(expected));
                    }
                    assert_eq!(budget.storage(), STORAGE_PREFIX);
                    assert_eq!(budget.peak_storage(), STORAGE_PREFIX);
                    assert_eq!(budget.failed_storage(), None);
                }
                if limit == exact {
                    assert_eq!(budget.work(), exact);
                }
                assert_eq!(
                    budget.work_budget_v1().failed_work(),
                    (limit < exact).then_some(exact)
                );
            }
        }
    }
}

#[test]
fn diagnostic_alias_queries_are_bidirectional_logarithmic_and_allocation_free() {
    let modern = (
        AMDGPU_DIAGNOSTICS_CAPABILITY_NAMESPACE,
        AMDGPU_DIAGNOSTICS_CAPABILITY_NAME,
    );
    let legacy = (
        AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAMESPACE,
        AMDGPU_GFX942_DIAGNOSTICS_CAPABILITY_NAME,
    );
    for (required, alias) in [(modern, legacy), (legacy, modern)] {
        let mut supported = (0..256)
            .map(|ordinal| TargetCapability::Extension {
                namespace: "unrelated".to_owned(),
                name: format!("{ordinal:04x}"),
            })
            .collect::<BTreeSet<_>>();
        supported.insert(TargetCapability::Extension {
            namespace: alias.0.to_owned(),
            name: alias.1.to_owned(),
        });
        // A 257-key roster has the same three-node bound as any 71..430-key tree.
        const COMPARISONS: usize = 33;
        let exact_query = COMPARISONS * (required.0.len() + required.1.len() + 3);
        let classification =
            2 * (modern.0.len().max(legacy.0.len()) + modern.1.len().max(legacy.1.len()) + 2);
        let alias_query = COMPARISONS * (alias.0.len() + alias.1.len() + 3);
        let exact_work = 1 + exact_query + classification + 1 + alias_query;
        for limit in [exact_work, exact_work - 1] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
            let result = target_capability_is_supported_with_budget_v1(
                TargetCapabilityRefV1::extension(required.0, required.1),
                &supported,
                &mut budget,
            );
            if limit == exact_work {
                assert_eq!(result, Ok(true));
                assert_eq!(budget.work(), exact_work);
            } else {
                assert!(matches!(
                    result,
                    Err(CanonicalKernelIrVerificationResourceErrorV1::Work(error))
                        if error.actual() == exact_work && error.limit() == limit
                ));
                assert_eq!(budget.work(), 1 + exact_query + classification + 1);
            }
            assert_eq!((budget.storage(), budget.peak_storage()), (0, 0));
        }

        let absent_name = format!("{}-suffix", required.1);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
        assert_eq!(
            target_capability_is_supported_with_budget_v1(
                TargetCapabilityRefV1::extension(required.0, &absent_name),
                &supported,
                &mut budget,
            ),
            Ok(false)
        );
        assert_eq!((budget.storage(), budget.peak_storage()), (0, 0));
    }
}

#[test]
fn atomic_queries_preserve_scope_coverage_instead_of_key_equality() {
    let supported = BTreeSet::from([
        TargetCapability::Float64,
        TargetCapability::Atomic {
            width_bits: 32,
            address_space: AddressSpace::Private,
            max_scope: SynchronizationScope::Device,
        },
        TargetCapability::Atomic {
            width_bits: 32,
            address_space: AddressSpace::Global,
            max_scope: SynchronizationScope::Workgroup,
        },
        TargetCapability::Extension {
            namespace: "unrelated".to_owned(),
            name: "large".repeat(4096),
        },
    ]);
    for width_bits in [16, 32, 64] {
        for address_space in [
            AddressSpace::Private,
            AddressSpace::Global,
            AddressSpace::Workgroup,
        ] {
            for max_scope in [
                SynchronizationScope::Invocation,
                SynchronizationScope::Subgroup,
                SynchronizationScope::Workgroup,
                SynchronizationScope::Device,
                SynchronizationScope::System,
            ] {
                let expected_position = supported.iter().position(|candidate| {
                    matches!(
                        candidate,
                        TargetCapability::Atomic {
                            width_bits: candidate_width,
                            address_space: candidate_space,
                            max_scope: candidate_scope,
                        } if *candidate_width == width_bits
                            && *candidate_space == address_space
                            && candidate_scope.rank() >= max_scope.rank()
                    )
                });
                let visits = expected_position.map_or(supported.len(), |position| position + 1);
                let exact_work = 1 + 4 * visits;
                let required = TargetCapabilityRefV1::Atomic {
                    width_bits,
                    address_space,
                    max_scope,
                };
                for limit in [exact_work, exact_work - 1] {
                    let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
                    let mut budget =
                        CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
                    let result = target_capability_is_supported_with_budget_v1(
                        required,
                        &supported,
                        &mut budget,
                    );
                    if limit == exact_work {
                        assert_eq!(result, Ok(expected_position.is_some()));
                        assert_eq!(budget.work(), exact_work);
                    } else {
                        assert!(matches!(
                            result,
                            Err(CanonicalKernelIrVerificationResourceErrorV1::Work(error))
                                if error.actual() == exact_work && error.limit() == limit
                        ));
                        assert_eq!(budget.work(), exact_work - 4);
                    }
                    assert_eq!((budget.storage(), budget.peak_storage()), (0, 0));
                }
            }
        }
    }
}
