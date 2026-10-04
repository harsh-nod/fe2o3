use super::*;
use std::mem::size_of;

fn headers() -> usize {
    size_of::<std::vec::IntoIter<Type>>()
        + size_of::<&mut std::vec::IntoIter<Type>>()
        + size_of::<Option<Type>>()
        + size_of::<Type>()
        + size_of::<(Type, &mut dyn SemanticEmissionBudgetV1)>()
        + size_of::<Result<Type, ProductionSemanticKirErrorV1>>()
        + size_of::<&Type>()
        + size_of::<&Box<Type>>()
        + size_of::<&fe2o3_kernel_ir::PointerType>()
        + size_of::<&fe2o3_kernel_ir::SliceType>()
        + size_of::<usize>()
        + size_of::<(&mut usize, &mut dyn SemanticEmissionBudgetV1)>()
        + size_of::<Option<usize>>()
        + size_of::<Result<usize, ArgumentResourceV1>>()
        + size_of::<Result<(), ArgumentResourceV1>>()
        + size_of::<Result<(), ProductionSemanticKirErrorV1>>()
}

fn nested(mut ty: Type, boxes: usize) -> Type {
    for index in 0..boxes {
        ty = if index % 2 == 0 {
            Type::pointer(ty, AddressSpace::Private, AccessMode::ReadWrite)
        } else {
            Type::slice(ty, AddressSpace::Global, AccessMode::ReadOnly)
        };
    }
    ty
}

fn addresses(mut ty: &Type) -> Vec<usize> {
    let mut addresses = Vec::new();
    loop {
        let next = match ty {
            Type::Pointer(pointer) => &pointer.pointee,
            Type::Slice(slice) => &slice.element,
            _ => return addresses,
        };
        addresses.push(next.as_ref() as *const Type as usize);
        ty = next;
    }
}

fn assert_transport(error: &ProductionSemanticKirErrorV1) {
    assert!(
        matches!(
            error,
            ProductionSemanticKirErrorV1::Unsupported {
                function: 0,
                block: None,
                statement: None,
                detail: "execution CFG transport differs from its captured SSA state",
            }
        ),
        "{error:?}"
    );
}

#[test]
fn owned_reference_type_frames_have_independent_exact_and_short_storage_limits() {
    let expected = headers();
    assert_eq!(source_reference_owned_type_headers_v29().unwrap(), expected);
    for short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
        let mut budget = ArgumentBudgetV1::new(&mut work, expected - usize::from(short));
        let result = budget.reserve_storage(expected);
        if short {
            assert!(matches!(result, Err(ArgumentResourceV1::Storage(error))
                if error.actual() == expected && error.limit() == expected - 1));
            assert_eq!(budget.storage(), 0);
            assert_eq!(budget.failed_storage(), Some(expected));
        } else {
            result.unwrap();
            assert_eq!(budget.storage(), expected);
        }
        assert_eq!(budget.work(), 0);
    }
}

#[test]
fn owned_reference_type_transfer_preserves_closed_clone_grammar_and_every_box() {
    let terminals = [
        Type::Unit,
        Type::BOOL,
        Type::Scalar(ScalarType::U64),
        Type::F32,
        Type::Vector(fe2o3_kernel_ir::FixedVectorTypeV12::new(
            ScalarType::U32,
            4,
            fe2o3_kernel_ir::VectorLayoutV12::Contiguous,
        )),
        Type::StorageObject(fe2o3_kernel_ir::StorageLayoutIdV1(17)),
    ];
    for terminal in terminals {
        for boxes in [0, 1, 2, 7] {
            let input = nested(terminal.clone(), boxes);
            let before = addresses(&input);
            let paid = headers() + boxes * size_of::<Type>();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(boxes + 1);
            let mut budget = ArgumentBudgetV1::new(&mut work, paid);
            budget.reserve_storage(paid).unwrap();
            let moved = source_reference_move_type_v29(input, &mut budget).unwrap();
            assert_eq!(addresses(&moved), before);
            assert_eq!(budget.work(), boxes + 1);
            assert_eq!((budget.storage(), budget.peak_storage()), (paid, paid));
            assert_eq!(budget.failed_storage(), None);

            let mut clone_work = CanonicalKernelIrWorkBudgetV1::new(boxes + 1);
            let mut clone_budget = ArgumentBudgetV1::new(&mut clone_work, usize::MAX);
            let cloned = execution_cfg_clone_type_v29(&moved, &mut clone_budget).unwrap();
            assert_eq!(cloned, moved);
            assert_eq!(clone_budget.work(), budget.work());
            assert_eq!(clone_budget.storage(), boxes * size_of::<Type>());
            assert_eq!(addresses(&moved), before);
        }
    }
}

#[test]
fn owned_reference_type_transfer_preserves_node_limit_and_execution_refusal_order() {
    for boxes in [255, 256] {
        let input = nested(Type::F32, boxes);
        let mut clone_work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut clone_budget = ArgumentBudgetV1::new(&mut clone_work, usize::MAX);
        let cloned = execution_cfg_clone_type_v29(&input, &mut clone_budget);
        let paid = headers() + boxes * size_of::<Type>();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, paid);
        budget.reserve_storage(paid).unwrap();
        let moved = source_reference_move_type_v29(input, &mut budget);
        assert_eq!(moved.is_ok(), boxes == 255);
        assert_eq!(budget.work(), boxes + 1);
        assert_eq!(clone_budget.work(), budget.work());
        match (moved, cloned) {
            (Ok(left), Ok(right)) => assert_eq!(left, right),
            (Err(left), Err(right)) => {
                assert_transport(&left);
                assert_transport(&right);
            }
            other => panic!("clone/move disagreement: {other:?}"),
        }
        assert_eq!(budget.storage(), paid);
    }
    for boxes in [0, 1, 7] {
        for short in [false, true] {
            let expected = boxes + 1;
            let input = nested(
                Type::Execution(fe2o3_kernel_ir::ExecutionRoleV15::Context),
                boxes,
            );
            let mut clone_work = CanonicalKernelIrWorkBudgetV1::new(expected - usize::from(short));
            let mut clone_budget = ArgumentBudgetV1::new(&mut clone_work, usize::MAX);
            let cloned = execution_cfg_clone_type_v29(&input, &mut clone_budget).unwrap_err();
            let paid = headers() + boxes * size_of::<Type>();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(expected - usize::from(short));
            let mut budget = ArgumentBudgetV1::new(&mut work, paid);
            budget.reserve_storage(paid).unwrap();
            let moved = source_reference_move_type_v29(input, &mut budget).unwrap_err();
            if short {
                for error in [&moved, &cloned] {
                    assert!(
                        matches!(error, ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(error)) if error.actual() == expected && error.limit() == expected - 1)
                    );
                }
            } else {
                assert_transport(&moved);
                assert_transport(&cloned);
            }
            assert_eq!(budget.work(), clone_budget.work());
            assert_eq!(budget.storage(), paid);
        }
    }
}

#[test]
fn owned_reference_type_transfer_work_cuts_and_iterator_drop_keep_original_credit() {
    for boxes in [0, 1, 7, 255] {
        for short in [false, true] {
            let expected = boxes + 1;
            let input = nested(Type::F32, boxes);
            let before = addresses(&input);
            let types = vec![input];
            let paid = headers()
                + size_of::<Vec<Type>>()
                + types.capacity() * size_of::<Type>()
                + boxes * size_of::<Type>();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(expected - usize::from(short));
            let mut budget = ArgumentBudgetV1::new(&mut work, paid);
            budget.reserve_storage(paid).unwrap();
            let result = {
                let mut iter = types.into_iter();
                let result = source_reference_move_type_v29(iter.next().unwrap(), &mut budget);
                assert!(iter.next().is_none());
                drop(iter);
                result
            };
            if short {
                assert!(
                    matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(error))) if error.actual() == expected && error.limit() == expected - 1)
                );
                assert_eq!(budget.failed_work(), Some(expected));
            } else {
                let moved = result.unwrap();
                assert_eq!(addresses(&moved), before);
                assert_eq!(moved, nested(Type::F32, boxes));
                assert_eq!(budget.failed_work(), None);
                drop(moved);
            }
            // No helper refund, including when the input/iterator is dropped.
            assert_eq!((budget.storage(), budget.peak_storage()), (paid, paid));
        }
    }
}
