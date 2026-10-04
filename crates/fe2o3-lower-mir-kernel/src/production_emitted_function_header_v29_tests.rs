use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkBudgetV1 as Work, ExecutionRoleV15, FixedVectorTypeV12,
    FunctionRole, VectorLayoutV12,
};

type Header = (FunctionId, Signature, Vec<ValueId>);
const FLOOR: usize = 23;
const LIMIT: usize = 1_000_000;

fn inputs() -> Header {
    let mut name = String::with_capacity(256);
    name.push_str("retained_header");
    let types = vec![
        Type::Unit,
        Type::Scalar(ScalarType::U32),
        Type::Execution(ExecutionRoleV15::Context),
        Type::Execution(ExecutionRoleV15::MaskedTileU32 {
            lanes: 64,
            elements: 3,
        }),
        Type::vector(FixedVectorTypeV12::new(
            ScalarType::U16,
            8,
            VectorLayoutV12::Interleaved { factor: 2 },
        )),
        Type::pointer(
            Type::slice(
                Type::Scalar(ScalarType::U64),
                AddressSpace::Global,
                AccessMode::WriteOnly,
            ),
            AddressSpace::Private,
            AccessMode::ReadOnly,
        ),
    ];
    let values = (0..types.len())
        .map(|index| ValueId(index as u32 + 10))
        .collect();
    (
        FunctionId::new(name),
        Signature::new(
            types,
            vec![Type::F32, Type::Execution(ExecutionRoleV15::Workgroup)],
        ),
        values,
    )
}

fn copy(
    input: &Header,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Header, ProductionSemanticKirErrorV1> {
    copy_emitted_function_header_v29(
        &input.0,
        &input.1.parameters,
        &input.1.results,
        &input.2,
        budget,
    )
}

fn boxes(mut ty: &Type) -> usize {
    let mut count = 0;
    loop {
        ty = match ty {
            Type::Pointer(pointer) => &pointer.pointee,
            Type::Slice(slice) => &slice.element,
            _ => return count,
        };
        count += 1;
    }
}

fn payload(header: &Header) -> usize {
    header.0.retained_capacity_bytes()
        + (header.1.parameters.capacity() + header.1.results.capacity())
            * std::mem::size_of::<Type>()
        + header.2.capacity() * std::mem::size_of::<ValueId>()
        + header
            .1
            .parameters
            .iter()
            .chain(&header.1.results)
            .map(boxes)
            .sum::<usize>()
            * std::mem::size_of::<Type>()
}

fn measured(input: &Header) -> (usize, usize) {
    let mut work = Work::new(LIMIT);
    let bytes = {
        let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let header = copy(input, &mut budget).unwrap();
        let bytes = payload(&header);
        assert_eq!(budget.storage(), FLOOR + bytes);
        drop(header);
        budget.release_storage(bytes).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        bytes
    };
    (work.work(), bytes)
}

#[test]
fn header_copy_preserves_all_type_variants_and_observed_capacities() {
    let input = inputs();
    let mut work = Work::new(LIMIT);
    let expected_work = input.0.as_str().len()
        + 2
        + 9
        + input.2.len()
        + input
            .1
            .parameters
            .iter()
            .chain(&input.1.results)
            .map(|ty| boxes(ty) + 1)
            .sum::<usize>();
    {
        let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
        budget.reserve_storage(FLOOR).unwrap();
        let header = copy(&input, &mut budget).unwrap();
        assert_eq!(header, input);
        assert_ne!(header.0.as_str().as_ptr(), input.0.as_str().as_ptr());
        assert_ne!(header.1.parameters.as_ptr(), input.1.parameters.as_ptr());
        assert_ne!(header.1.results.as_ptr(), input.1.results.as_ptr());
        assert_ne!(header.2.as_ptr(), input.2.as_ptr());
        let Type::Pointer(original) = &input.1.parameters[5] else {
            panic!("input pointer")
        };
        let Type::Pointer(copied) = &header.1.parameters[5] else {
            panic!("copied pointer")
        };
        assert!(!std::ptr::eq(&*original.pointee, &*copied.pointee));
        let bytes = payload(&header);
        assert_eq!(budget.storage(), FLOOR + bytes);
        drop(header);
        budget.release_storage(bytes).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
    assert_eq!(work.work(), expected_work);
}

#[test]
fn metered_header_matches_legacy_values_for_both_function_roles() {
    let input = inputs();
    for role in [FunctionRole::KernelEntry, FunctionRole::InternalHelper] {
        let mut work = Work::new(LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
        let actual = copy(&input, &mut budget).unwrap();
        let expected = input.clone();
        let make = |(id, signature, values): Header| match role {
            FunctionRole::KernelEntry => Function::kernel_entry(id, signature, values, vec![]),
            FunctionRole::InternalHelper => {
                Function::internal_helper(id, signature, values, vec![])
            }
            _ => unreachable!(),
        };
        assert_eq!(make(actual), make(expected));
    }
}

#[test]
fn empty_header_and_scalar_types_do_not_invent_boxed_allocations() {
    let input = (FunctionId::new(""), Signature::new(vec![], vec![]), vec![]);
    assert_eq!(measured(&input), (11, 0));
    let mut work = Work::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, FLOOR);
    budget.reserve_storage(FLOOR).unwrap();
    for ty in [
        Type::Unit,
        Type::BOOL,
        Type::F64,
        Type::Execution(ExecutionRoleV15::Context),
    ] {
        assert_eq!(copy_emitted_header_type_v29(&ty, &mut budget).unwrap(), ty);
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn header_type_copy_has_no_cfg_depth_or_execution_role_restriction() {
    let mut ty = Type::Execution(ExecutionRoleV15::LaneFragmentU32 {
        lanes: 32,
        elements: 7,
    });
    for index in 0..513 {
        ty = if index % 2 == 0 {
            Type::pointer(ty, AddressSpace::Constant, AccessMode::ReadOnly)
        } else {
            Type::slice(ty, AddressSpace::Generic, AccessMode::ReadWrite)
        };
    }
    let mut work = Work::new(LIMIT);
    {
        let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
        let copied = copy_emitted_header_type_v29(&ty, &mut budget).unwrap();
        assert_eq!(copied, ty);
        assert_eq!(budget.storage(), 513 * std::mem::size_of::<Type>());
    }
    assert_eq!(work.work(), 514);
}

#[test]
fn copied_header_survives_source_drop_and_two_copies_keep_separate_charges() {
    let input = inputs();
    let original_bytes = payload(&input);
    let mut work = Work::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR + original_bytes).unwrap();
    let first = copy(&input, &mut budget).unwrap();
    let first_bytes = payload(&first);
    let second = copy(&input, &mut budget).unwrap();
    let second_bytes = payload(&second);
    assert_eq!(
        budget.storage(),
        FLOOR + original_bytes + first_bytes + second_bytes
    );
    drop(input);
    budget.release_storage(original_bytes).unwrap();
    assert_eq!(budget.storage(), FLOOR + first_bytes + second_bytes);
    assert_eq!(first, second);
    assert_ne!(first.1.parameters.as_ptr(), second.1.parameters.as_ptr());
    drop(first);
    budget.release_storage(first_bytes).unwrap();
    assert_eq!(second.0.as_str(), "retained_header");
    assert_eq!(budget.storage(), FLOOR + second_bytes);
    drop(second);
    budget.release_storage(second_bytes).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn header_copy_exact_work_and_storage_succeed_with_an_existing_sibling() {
    let input = inputs();
    let (one_work, one_storage) = measured(&input);
    let mut work = Work::new(2 * one_work);
    {
        let mut budget = ArgumentBudgetV1::new(&mut work, FLOOR + 2 * one_storage);
        budget.reserve_storage(FLOOR).unwrap();
        let sibling = copy(&input, &mut budget).unwrap();
        let floor = budget.storage();
        let header = scoped_module_attempt_v29(&mut budget, floor, |budget| {
            copy(&input, budget).map_err(ScopedModuleErrorV29::from)
        })
        .unwrap();
        assert_eq!(header, sibling);
        assert_eq!(
            budget.storage(),
            FLOOR + payload(&sibling) + payload(&header)
        );
    }
    assert_eq!(work.work(), 2 * one_work);
}

#[test]
fn every_header_work_denial_preserves_the_live_sibling_and_original_floor() {
    let input = inputs();
    let (one_work, one_storage) = measured(&input);
    for remaining in 0..one_work {
        let mut work = Work::new(one_work + remaining);
        let mut budget = ArgumentBudgetV1::new(&mut work, FLOOR + 2 * one_storage);
        budget.reserve_storage(FLOOR).unwrap();
        let sibling = copy(&input, &mut budget).unwrap();
        let pointer = sibling.1.parameters.as_ptr();
        let ledger = budget.work_ledger_identity_v1();
        let floor = budget.storage();
        let result = scoped_module_attempt_v29(&mut budget, floor, |budget| {
            copy(&input, budget).map_err(ScopedModuleErrorV29::from)
        });
        assert!(
            matches!(
                result,
                Err(ScopedModuleErrorV29::Source(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(_)
                    )
                ))
            ),
            "remaining work: {remaining}"
        );
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(sibling, input);
        assert_eq!(sibling.1.parameters.as_ptr(), pointer);
        drop(sibling);
        budget.release_storage(one_storage).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn every_header_storage_denial_preserves_the_live_sibling_and_original_floor() {
    let input = inputs();
    let (_, one_storage) = measured(&input);
    for remaining in 0..one_storage {
        let mut work = Work::new(LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, FLOOR + one_storage + remaining);
        budget.reserve_storage(FLOOR).unwrap();
        let sibling = copy(&input, &mut budget).unwrap();
        let floor = budget.storage();
        let result = scoped_module_attempt_v29(&mut budget, floor, |budget| {
            copy(&input, budget).map_err(ScopedModuleErrorV29::from)
        });
        assert!(
            matches!(
                result,
                Err(ScopedModuleErrorV29::Source(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Storage(_)
                    )
                ))
            ),
            "remaining storage: {remaining}"
        );
        assert_eq!(budget.storage(), floor);
        assert_eq!(sibling, input);
    }
}

#[test]
fn header_unwind_cleanup_keeps_sibling_credit_before_hostile_payload_drop() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    struct HostilePayload(Arc<AtomicBool>);
    impl Drop for HostilePayload {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
            panic!("payload destructor outside ledger cleanup");
        }
    }
    let input = inputs();
    let mut work = Work::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let sibling = copy(&input, &mut budget).unwrap();
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let dropped = Arc::new(AtomicBool::new(false));
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: Result<(), ScopedModuleErrorV29> =
            scoped_module_attempt_v29(&mut budget, floor, |budget| {
                let _partial = copy(&input, budget).map_err(ScopedModuleErrorV29::from)?;
                std::panic::panic_any(HostilePayload(Arc::clone(&dropped)));
            });
    }));
    assert!(caught.is_err());
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    assert_eq!(sibling, input);
    assert!(!dropped.load(Ordering::SeqCst));
    let payload = caught.expect_err("injected unwind");
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(payload))).is_err());
    assert!(dropped.load(Ordering::SeqCst));
    assert_eq!(budget.storage(), floor);
}
