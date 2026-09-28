use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

fn independent<C>() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 3 * size_of::<Result<T, E>>() + 2 * size_of::<Result<T, SourceError>>()
    }
    [
        h::<C>(),
        align_of::<C>(),
        h::<ConsumerCapture<'_, '_, '_, C>>(),
        h::<AssertUnwindSafe<ConsumerCapture<'_, '_, '_, C>>>(),
        h::<Builder>(),
        h::<Schedule<'_, '_, '_>>(),
        h::<Cursor>(),
        h::<Root>(),
        h::<Argument>(),
        h::<Component>(),
        h::<Option<Root>>(),
        h::<Option<Argument>>(),
        h::<Option<Component>>(),
        h::<&Root>(),
        h::<&Argument>(),
        h::<&Component>(),
        h::<Vec<Root>>(),
        h::<Vec<Argument>>(),
        h::<Vec<Component>>(),
        h::<(&mut Vec<Root>, Root)>(),
        h::<(&mut Vec<Argument>, Argument)>(),
        h::<(&mut Vec<Component>, Component)>(),
        h::<&[Root]>(),
        h::<&[Argument]>(),
        h::<&[Component]>(),
        h::<OwnerRef<'_>>(),
        h::<&[TypedDescriptorRootV1]>(),
        h::<&TypedDescriptorRootV1>(),
        h::<&TypedDescriptorArgumentV1>(),
        h::<ProductionAmdTargetProfileV1>(),
        h::<&mut Budget<'_>>(),
        h::<&Module>(),
        h::<&fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12>(),
        h::<&[SemanticTypeDeclV1]>(),
        h::<&[TypedDescriptorArgumentV1]>(),
        h::<&Vec<Type>>(),
        h::<&Vec<fe2o3_kernel_ir::Kernel>>(),
        h::<&Vec<Function>>(),
        h::<&fe2o3_kernel_ir::FunctionId>(),
        h::<&str>(),
        h::<&SemanticTypeLayoutV1>(),
        h::<&Function>(),
        h::<Option<&Function>>(),
        h::<&fe2o3_kernel_ir::Kernel>(),
        h::<&Type>(),
        h::<&ScalarType>(),
        h::<&Cursor>(),
        h::<&mut Cursor>(),
        h::<(&mut Cursor, u16, u16)>(),
        h::<&SemanticTypeDeclV1>(),
        h::<Option<&SemanticTypeDeclV1>>(),
        h::<&AdmittedInertSemanticMirV1>(),
        h::<&mut Plan<'_, '_, '_>>(),
        h::<&mut Builder>(),
        h::<fe2o3_lower_mir_kernel::ProductionSourceAbiArgumentV1>(),
        h::<fe2o3_lower_mir_kernel::ProductionSourceAbiComponentV1<'_>>(),
        h::<fe2o3_lower_mir_kernel::ProductionPhysicalArgumentV1<'_>>(),
        h::<std::ops::Range<usize>>(),
        h::<std::ops::Range<usize>>(),
        h::<std::slice::Iter<'_, TypedDescriptorRootV1>>(),
        h::<std::slice::Iter<'_, fe2o3_kernel_ir::Kernel>>(),
        h::<std::slice::Iter<'_, Type>>(),
        h::<std::slice::Iter<'_, Function>>(),
        h::<Option<&TypedDescriptorRootV1>>(),
        h::<Option<&fe2o3_kernel_ir::Kernel>>(),
        h::<Option<&Type>>(),
        h::<DeviceLayoutDescriptorV1>(),
        h::<Kind>(),
        h::<(Kind, u16, u16)>(),
        h::<((Kind, u16, u16), bool, u64, ValueId, SemanticTypeIdentityV1)>(),
        h::<(u32, u32, u32)>(),
        h::<(usize, usize, usize)>(),
        h::<Option<Resource>>(),
        h::<Option<u64>>(),
        h::<Option<u32>>(),
        h::<Option<usize>>(),
        h::<ScalarTypeV1>(),
        h::<ScalarType>(),
        h::<ValueId>(),
        h::<SemanticTypeIdentityV1>(),
        h::<SemanticLayoutIdentityV1>(),
        h::<SemanticLocalIdV1>(),
        h::<bool>(),
        h::<()>(),
        h::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>(),
        24 * h::<usize>(),
        4 * h::<u64>(),
        6 * h::<u32>(),
        3 * h::<u16>(),
        h::<Result<R<()>, Box<dyn std::any::Any + Send>>>(),
        h::<Result<R<Builder>, Box<dyn std::any::Any + Send>>>(),
        h::<Result<(), Box<dyn std::any::Any + Send>>>(),
        h::<Box<dyn std::any::Any + Send>>(),
        2 * h::<BuildFrame<'_, '_>>(),
        h::<AssertUnwindSafe<BuildFrame<'_, '_>>>(),
        h::<VisitFrame<'_, '_, '_>>(),
        h::<(
            OwnerRef<'_>,
            &[TypedDescriptorRootV1],
            ProductionAmdTargetProfileV1,
            &mut Schedule<'_, '_, '_>,
        )>(),
        h::<&mut Schedule<'_, '_, '_>>(),
        h::<(&mut Builder,)>(),
    ]
    .into_iter()
    .sum()
}

#[test]
fn entry_packing_header_equation_and_first_reservation_are_exact() {
    type Capture = [u8; 37];
    let expected = independent::<Capture>();
    assert_eq!(headers::<Capture>().unwrap(), expected);
    for short in [false, true] {
        let mut work = Work::new(0);
        let floor = 23;
        let limit = floor + expected - usize::from(short);
        let mut budget = Budget::new(&mut work, limit);
        budget.reserve_storage(floor).unwrap();
        let result = budget.reserve_storage(expected);
        if short {
            let Resource::Storage(error) = result.unwrap_err() else {
                panic!("exact storage cause")
            };
            assert_eq!((error.actual(), error.limit()), (floor + expected, limit));
            assert_eq!((budget.storage(), budget.peak_storage()), (floor, floor));
        } else {
            result.unwrap();
            assert_eq!(
                (budget.storage(), budget.peak_storage()),
                (floor + expected, floor + expected)
            );
            budget.release_storage(expected).unwrap();
            assert_eq!(budget.storage(), floor);
        }
        assert_eq!(budget.work(), 0);
    }
}

#[test]
fn entry_packing_rows_use_actual_capacity_and_never_grow_on_append() {
    for count in [0, 1, 2, 8, 64] {
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let floor = 31;
        budget.reserve_storage(floor).unwrap();
        let mut rows = vector::<Root>(count, &mut budget).unwrap();
        let bytes = size_of::<Vec<Root>>() + rows.capacity() * size_of::<Root>();
        assert_eq!(budget.storage(), floor + bytes);
        let capacity = rows.capacity();
        let row = Root {
            first_argument: 0,
            end_argument: 0,
            first_component: 0,
            end_component: 0,
            physical_slots: 0,
            explicit_bytes: 0,
            segment_bytes: 256,
            alignment: 8,
        };
        for _ in 0..capacity {
            push(&mut rows, row).unwrap();
        }
        assert!(matches!(
            push(&mut rows, row),
            Err(E::Resource(Resource::Accounting))
        ));
        assert_eq!(rows.capacity(), capacity);
        assert_eq!(budget.storage(), floor + bytes);
        drop(rows);
        budget.release_storage(bytes).unwrap();
        assert_eq!(budget.storage(), floor);
    }
}

#[test]
fn entry_packing_hidden_tail_preserves_explicit_offsets_for_small_and_empty_rosters() {
    type Case<'a> = (&'a [(u16, u16)], &'a [u32], (u32, u32, u32));
    let cases: &[Case<'_>] = &[
        (&[], &[], (0, 256, 8)),
        (&[(1, 1)], &[0], (1, 264, 8)),
        (&[(2, 2)], &[0], (2, 264, 8)),
        (&[(4, 4)], &[0], (4, 264, 8)),
        (&[(4, 4), (4, 4), (4, 4)], &[0, 4, 8], (12, 272, 8)),
        (&[(4, 4), (4, 4)], &[0, 4], (8, 264, 8)),
        (&[(8, 8)], &[0], (8, 264, 8)),
        (&[(8, 8), (1, 1)], &[0, 8], (16, 272, 8)),
        (&[(1, 1), (8, 8), (4, 4)], &[0, 8, 16], (24, 280, 8)),
    ];
    for &(parts, offsets, expected) in cases {
        let mut cursor = Cursor::default();
        for (&(size, alignment), &offset) in parts.iter().zip(offsets) {
            assert_eq!(cursor.advance(size, alignment).unwrap(), offset);
        }
        assert_eq!(parts.len(), offsets.len());
        let before = (cursor.end, cursor.alignment);
        assert_eq!(cursor.finish().unwrap(), expected);
        assert_eq!(cursor.finish().unwrap(), expected);
        assert_eq!((cursor.end, cursor.alignment), before);
        let (explicit, segment, alignment) = expected;
        assert_eq!(segment - ((explicit + 7) & !7), 256);
        assert!(alignment >= 8);
    }
}

#[test]
fn entry_packing_hidden_tail_checks_both_padding_and_total_segment_arithmetic() {
    let limit = MAX_KERNARG_SEGMENT_BYTES;
    let exact = Cursor {
        end: limit - 256,
        alignment: 1,
    };
    assert_eq!(exact.finish().unwrap(), (limit - 256, limit, 8));
    for end in [limit - 255, limit, limit + 1] {
        assert!(matches!(
            Cursor { end, alignment: 1 }.finish(),
            Err(E::Mismatch("packing segment byte limit"))
        ));
    }
    for (end, alignment) in [(u32::MAX, 1), (u32::MAX - 255, 1), (u32::MAX, 8)] {
        assert!(matches!(
            Cursor { end, alignment }.finish(),
            Err(E::Resource(Resource::Arithmetic))
        ));
    }
}

#[test]
fn entry_packing_hidden_tail_fixed_work_is_independent_of_slot_count() {
    // This isolates the new logical arithmetic debit. The genuine-owner
    // qualification also retains its full constructor exact/one-short cuts.
    assert_eq!(ROOT_FINISH_WORK, 12);
    for end in [0, 1, 4, 12, 32, MAX_KERNARG_SEGMENT_BYTES - 256] {
        for short in [false, true] {
            let limit = 19 + 12 - usize::from(short);
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, 23);
            budget.charge_work(19).unwrap();
            budget.reserve_storage(23).unwrap();
            let result = (|| -> R<(u32, u32, u32)> {
                budget.charge_work(ROOT_FINISH_WORK)?;
                Cursor { end, alignment: 1 }.finish()
            })();
            if short {
                assert!(matches!(result, Err(E::Resource(Resource::Work(error)))
                    if error.actual() == 31 && error.limit() == 30));
                assert_eq!(budget.work(), 19);
                assert_eq!(budget.failed_work(), Some(31));
            } else {
                let (explicit, segment, alignment) = result.unwrap();
                assert_eq!((explicit, alignment), (end, 8));
                assert_eq!(segment, ((end + 7) & !7) + 256);
                assert_eq!(budget.work(), 31);
            }
            assert_eq!(budget.storage(), 23);
        }
    }
}
