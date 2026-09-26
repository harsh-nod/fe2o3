use super::super::storage_native_v18::tests::*;
use super::super::storage_resources_v18::{MeterV18, StorageMeterV18, headers_v18};
use super::tests::{multiple_module, multiple_roles, record_rows, root_module, with_roles};
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    VerifiedCanonicalKernelIrModuleV18 as Owner,
};
use std::mem::{align_of, size_of};

const FLOOR: usize = 81;

#[test]
fn root_roles_add_one_borrowed_context_pointer_and_no_second_storage_owner() {
    let alignment = align_of::<&Owner>()
        .max(align_of::<LoweringTarget>())
        .max(align_of::<&dyn StorageMeterV18>())
        .max(align_of::<Option<&RootRolesV29<'_>>>());
    let before_fields =
        size_of::<&Owner>() + size_of::<LoweringTarget>() + size_of::<&dyn StorageMeterV18>();
    let after_fields = before_fields + size_of::<Option<&RootRolesV29<'_>>>();
    let round = |bytes| (bytes + alignment - 1) & !(alignment - 1);
    assert_eq!(size_of::<Context<'_>>(), round(after_fields));
    assert_eq!(
        round(after_fields) - round(before_fields),
        size_of::<usize>()
    );
    assert_eq!(size_of::<RootRolesV29<'_>>(), 2 * size_of::<usize>());
    for short in [false, true] {
        let mut work = Work::new(1_000);
        let limit = FLOOR + headers_v18() - usize::from(short);
        let mut budget = Budget::new(&mut work, limit);
        budget.reserve_storage(FLOOR).unwrap();
        {
            let result = MeterV18::new(&mut budget);
            match result {
                Err(error) => {
                    assert!(short);
                    assert!(matches!(error, Resource::Storage(error)
                        if error.actual() == FLOOR + headers_v18() && error.limit() == limit));
                }
                Ok(meter) => {
                    assert!(!short);
                    drop(meter);
                }
            }
        }
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.work(), 0);
        assert_eq!(budget.peak_storage(), if short { FLOOR } else { limit });
    }
}

#[test]
fn complete_original_role_validation_has_independent_exact_work_and_sticky_one_short_refusal() {
    let module = root_module(record_rows(8), 1, Profile::Gfx942, false);
    with_owner(&module, |owner, _| {
        with_roles(
            owner,
            &[RootParameterRoleV29::InlineObject(Id(1))],
            |roles| {
                // Roster1 + row1 + actual-kernel1 + lookup(1 + 1+2*K) +
                // entry(1 + 1+2*F + 1+2*F) + Inline role1 + actual row1.
                let required = 10 + 2 * "entry".len() + 4 * "entry".len();
                for available in [required, required - 1] {
                    let mut work = Work::new(available);
                    let mut budget = Budget::new(&mut work, FLOOR + headers_v18());
                    budget.reserve_storage(FLOOR).unwrap();
                    let meter = MeterV18::new(&mut budget).unwrap();
                    let context = Context {
                        owner,
                        target: LoweringTarget::Gfx942XnackMinusV1,
                        meter: &meter,
                        root_roles: Some(roles),
                    };
                    let result = roles.validate(&context);
                    assert_eq!(result.is_ok(), available == required);
                    if available < required {
                        let first = meter.failure().unwrap();
                        assert!(
                            matches!(first, Resource::Work(error) if error.actual() == required && error.limit() == available)
                        );
                        assert!(context.charge(1).is_err());
                        assert_eq!(meter.failure(), Some(first));
                    }
                    drop(result);
                    drop(meter);
                    assert_eq!(budget.work(), available);
                    assert_eq!(budget.storage(), FLOOR);
                    assert_eq!(budget.peak_storage(), FLOOR + headers_v18());
                }
            },
        )
    });
}

#[test]
fn indexed_role_query_has_independent_exact_work_and_requires_the_actual_function() {
    let mut module = root_module(record_rows(8), 1, Profile::Gfx942, false);
    let mut helper = module.functions[0].clone();
    helper.id = FunctionId::from("helper");
    helper.role = FunctionRole::InternalHelper;
    module.functions.push(helper);
    with_owner(&module, |owner, _| {
        with_roles(
            owner,
            &[RootParameterRoleV29::InlineObject(Id(1))],
            |roles| {
                let kernel = &owner.module().kernels[0];
                let original = &owner.module().functions[0];
                let clone = original.clone();
                // Lookup(2+2*K), entry(3+4*F), original-function1, Inline2.
                let required = 8 + 2 * "entry".len() + 4 * "entry".len();
                for available in [required, required - 1] {
                    let mut work = Work::new(available);
                    let mut budget = Budget::new(&mut work, FLOOR + headers_v18());
                    budget.reserve_storage(FLOOR).unwrap();
                    let meter = MeterV18::new(&mut budget).unwrap();
                    let context = Context {
                        owner,
                        target: LoweringTarget::Gfx942XnackMinusV1,
                        meter: &meter,
                        root_roles: Some(roles),
                    };
                    let result = roles.parameter(&context, kernel, original, 0);
                    if available == required {
                        assert_eq!(result.unwrap(), RootParameterRoleV29::InlineObject(Id(1)));
                    } else {
                        assert!(result.is_err());
                        assert!(matches!(meter.failure(), Some(Resource::Work(error))
                if error.actual() == required && error.limit() == available));
                    }
                    drop(meter);
                    assert_eq!((budget.work(), budget.storage()), (available, FLOOR));
                }
                for actual in [&clone, &owner.module().functions[1]] {
                    let mut work = Work::new(required);
                    let mut budget = Budget::new(&mut work, FLOOR + headers_v18());
                    budget.reserve_storage(FLOOR).unwrap();
                    let meter = MeterV18::new(&mut budget).unwrap();
                    let context = Context {
                        owner,
                        target: LoweringTarget::Gfx942XnackMinusV1,
                        meter: &meter,
                        root_roles: Some(roles),
                    };
                    assert!(roles.parameter(&context, kernel, actual, 0).is_err());
                    assert_eq!(meter.failure(), None);
                    drop(meter);
                    assert_eq!(budget.work(), required - 2);
                    assert_eq!(budget.storage(), FLOOR);
                }
            },
        )
    });
}

#[test]
fn unordered_actual_owner_uses_bounded_binary_lookup_with_independently_counted_comparisons() {
    let module = multiple_module(Profile::Gfx942);
    with_owner(&module, |owner, _| {
        let parameters = [RootParameterRoleV29::InlineObject(Id(1))];
        let rows = multiple_roles(owner, &parameters);
        let roles = RootRolesV29 { roots: &rows };
        // Four one-byte ordered keys: adjacent comparisons3. Binary probes for
        // actual d,a,c,b are2,3,1,2. Entry names are seven bytes (a_entry etc.).
        let adjacent = 3 * (1 + 1 + 1);
        let lookup = 4 + (2 + 3 + 1 + 2) * (1 + 1 + 1);
        let entries = 4 * (1 + (1 + 7 + 7) + (1 + 7 + 7));
        let required = 1 + 4 + adjacent + 4 + lookup + entries + 4 * 2;
        for available in [required, required - 1] {
            let mut work = Work::new(available);
            let mut budget = Budget::new(&mut work, FLOOR + headers_v18());
            budget.reserve_storage(FLOOR).unwrap();
            let meter = MeterV18::new(&mut budget).unwrap();
            let context = Context {
                owner,
                target: LoweringTarget::Gfx942XnackMinusV1,
                meter: &meter,
                root_roles: Some(&roles),
            };
            assert_eq!(roles.validate(&context).is_ok(), available == required);
            if available < required {
                assert!(matches!(meter.failure(), Some(Resource::Work(error))
                if error.actual() == required && error.limit() == available));
            }
            drop(meter);
            assert_eq!((budget.work(), budget.storage()), (available, FLOOR));
        }
    });
}

#[test]
fn malformed_roster_prefix_and_scope_panic_refund_only_the_existing_meter_credit() {
    let module = root_module(record_rows(8), 1, Profile::Gfx942, false);
    with_owner(&module, |owner, _| {
        for panic_after in [false, true] {
            let mut work = Work::new(1_000);
            let mut budget = Budget::new(&mut work, FLOOR + headers_v18());
            budget.reserve_storage(FLOOR).unwrap();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let meter = MeterV18::new(&mut budget).unwrap();
                let roles = RootRolesV29 { roots: &[] };
                let context = Context {
                    owner,
                    target: LoweringTarget::Gfx942XnackMinusV1,
                    meter: &meter,
                    root_roles: Some(&roles),
                };
                assert!(roles.validate(&context).is_err());
                assert_eq!(meter.failure(), None);
                if panic_after {
                    std::panic::panic_any(0x52544e4154495645u64);
                }
            }));
            if panic_after {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<u64>(),
                    Some(&0x52544e4154495645u64)
                );
            } else {
                result.unwrap();
            }
            assert_eq!((budget.work(), budget.storage()), (1, FLOOR));
            assert_eq!(budget.peak_storage(), FLOOR + headers_v18());
        }
    });
}
