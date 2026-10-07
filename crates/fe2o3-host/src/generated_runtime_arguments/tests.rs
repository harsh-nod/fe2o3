use super::*;
use crate::generated_argument_plan::validate_argument_packing;
use crate::generated_kfd_arguments::GeneratedKfdReadWriteSlice;
use fe2o3_artifacts::{
    AbiField, AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership, Mutability,
    Name, PointerWidth,
};

pub(super) fn plan<T: GeneratedDeviceScalarV1>(
    accesses: &[Access],
    mapping: Option<RustDisjointIndexSpaceV1>,
) -> GeneratedArgumentPackingPlanV1 {
    let fields = accesses
        .iter()
        .enumerate()
        .map(|(index, access)| {
            let read_only = *access == Access::ReadOnly;
            let identity = if read_only {
                T::shared_slice_type_identity_v1(PointerWidth::Bits64)
            } else if let Some(mapping) = mapping {
                T::disjoint_slice_type_identity_for_index_space_v1(PointerWidth::Bits64, mapping)
            } else {
                T::disjoint_slice_type_identity_v1(PointerWidth::Bits64)
            };
            AbiField::new(
                Name::new(format!("arg_{index}")).unwrap(),
                (index * 16) as u64,
                16,
                8,
                AbiKind::Slice {
                    element_size: T::RUST_SCALAR_TYPE.size_bytes(),
                    element_alignment: T::RUST_SCALAR_TYPE.size_bytes() as u32,
                },
                if read_only {
                    Mutability::Immutable
                } else {
                    Mutability::Mutable
                },
                *access,
                AddressSpace::Global,
                identity,
                if read_only {
                    ArgumentOwnership::SharedBorrow
                } else {
                    ArgumentOwnership::UniqueBorrow
                },
                if read_only {
                    AliasClass::SharedReadOnly
                } else {
                    AliasClass::Exclusive
                },
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let bytes = (fields.len() * 16) as u64;
    let abi = AbiLayout::new(bytes, 8, PointerWidth::Bits64, fields.clone()).unwrap();
    let mappings = accesses
        .iter()
        .map(|access| {
            if *access == Access::ReadOnly {
                None
            } else {
                mapping
            }
        })
        .collect();
    let layout = CompilerGeneratedArgumentLayoutV1::new_with_disjoint_index_spaces_v1(
        bytes,
        8,
        PointerWidth::Bits64,
        fields,
        mappings,
    )
    .unwrap();
    validate_argument_packing(KernelId::from_bytes([0x67; 32]), &abi, &layout).unwrap()
}

fn budget(plan: &GeneratedArgumentPackingPlanV1) -> GeneratedRuntimeArgumentBudgetV1 {
    GeneratedRuntimeArgumentBudgetV1::new(
        plan,
        GeneratedRuntimeArgumentLimitsV1::new(4096, 4096, 8),
    )
    .unwrap()
}

fn output_fixture() -> (
    GeneratedRuntimePackedArgumentsV1,
    GeneratedRuntimeResultV1<u32>,
) {
    let plan = plan::<u32>(&[Access::ReadWrite], None);
    let mut budget = budget(&plan);
    let (output, observer) =
        GeneratedRuntimeReadWriteSlice::new(vec![3_u32, 7, 11, 19].into_boxed_slice());
    let binding = output.bind_argument(&plan, 0, &mut budget).unwrap();
    let packed =
        GeneratedRuntimeArgumentBindingV1::from_compiler_generated_parts(vec![], vec![binding])
            .pack(&plan, budget)
            .unwrap();
    (packed, observer)
}

fn words(values: &[u32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect::<Vec<_>>()
        .into_boxed_slice()
        .into_vec()
}

#[test]
fn owned_packing_reuses_exact_borrowed_abi_bytes_and_observations() {
    fn assert_owned<T: Send + 'static>() {}
    assert_owned::<GeneratedRuntimePackedArgumentsV1>();
    assert_owned::<GeneratedRuntimeOutputDecoderV1>();
    assert_owned::<GeneratedRuntimeResultV1<u32>>();
    let (owned, mut observer) = output_fixture();
    let mut values = [3_u32, 7, 11, 19];
    let plan = plan::<u32>(&[Access::ReadWrite], None);
    let binding = GeneratedKfdReadWriteSlice::new(&mut values)
        .bind_argument(&plan, 0)
        .unwrap();
    let borrowed =
        GeneratedKfdArgumentBinding::from_compiler_generated_parts(vec![], vec![binding])
            .pack(&plan)
            .unwrap();
    assert_eq!(owned.kernel_id(), borrowed.kernel_id());
    assert_eq!(owned.alignment(), borrowed.alignment());
    assert_eq!(owned.explicit_kernarg(), borrowed.explicit_kernarg());
    assert_eq!(owned.buffers(), borrowed.buffers());
    assert_eq!(owned.packing_observation(), borrowed.packing_observation());
    assert_eq!(
        owned.footprint(),
        GeneratedRuntimeArgumentFootprintV1 {
            kernarg_bytes: 16,
            input_bytes: 16,
            output_bytes: 16,
            payload_bytes: 64,
            result_bytes: 32,
            bindings: 1,
            output_observers: 1,
        }
    );
    assert!(observer.try_take().unwrap().is_none());
    drop(owned);
    assert!(matches!(
        observer.try_take(),
        Err(GeneratedRuntimeArgumentErrorV1::OutputUnavailable)
    ));
}

#[test]
fn borrowed_writeback_cannot_be_extracted_as_owned_storage() {
    let mut values = [3_u32];
    let plan = plan::<u32>(&[Access::ReadWrite], None);
    let binding = GeneratedKfdReadWriteSlice::new(&mut values)
        .bind_argument(&plan, 0)
        .unwrap();
    let borrowed =
        GeneratedKfdArgumentBinding::from_compiler_generated_parts(vec![], vec![binding])
            .pack(&plan)
            .unwrap();
    assert!(matches!(
        borrowed.into_owned_parts(),
        Err(GeneratedKfdArgumentError::RetainedWriteback)
    ));
    assert_eq!(values, [3]);
}

#[test]
fn byte_limits_reject_before_encoding_and_checked_charges_do_not_partially_debit() {
    let plan = plan::<u32>(&[Access::ReadWrite], None);
    for limits in [
        GeneratedRuntimeArgumentLimitsV1::new(63, 32, 1),
        GeneratedRuntimeArgumentLimitsV1::new(64, 31, 1),
    ] {
        let mut budget = GeneratedRuntimeArgumentBudgetV1::new(&plan, limits).unwrap();
        let before = budget.footprint();
        let (output, mut observer) =
            GeneratedRuntimeReadWriteSlice::new(vec![1_u32; 4].into_boxed_slice());
        assert!(matches!(
            output.bind_argument(&plan, 0, &mut budget),
            Err(GeneratedRuntimeArgumentErrorV1::PayloadLimit)
        ));
        assert_eq!(budget.footprint(), before);
        assert!(matches!(
            observer.try_take(),
            Err(GeneratedRuntimeArgumentErrorV1::OutputUnavailable)
        ));
    }
    assert!(matches!(
        GeneratedRuntimeArgumentBudgetV1::new(
            &plan,
            GeneratedRuntimeArgumentLimitsV1::new(32, 0, 0)
        ),
        Err(GeneratedRuntimeArgumentErrorV1::PayloadLimit)
    ));
    let mut budget = budget(&plan);
    let before = budget.footprint();
    assert!(matches!(
        budget.charge(usize::MAX, true),
        Err(GeneratedRuntimeArgumentErrorV1::ByteLength)
    ));
    assert_eq!(budget.footprint(), before);
}

#[test]
fn whole_invocation_preflight_rejects_without_binding_or_encoding_earlier_outputs() {
    let plan = plan::<u32>(&[Access::ReadWrite, Access::ReadWrite], None);
    let mut preflight = GeneratedRuntimeArgumentBudgetV1::new(
        &plan,
        GeneratedRuntimeArgumentLimitsV1::new(80, 32, 2),
    )
    .unwrap();
    let (first, mut first_observer) =
        GeneratedRuntimeReadWriteSlice::new(vec![3_u32].into_boxed_slice());
    let (second, mut second_observer) =
        GeneratedRuntimeReadWriteSlice::new(vec![7_u32; 4].into_boxed_slice());
    first.account_storage(&mut preflight).unwrap();
    let before = preflight.footprint();
    assert!(matches!(
        second.account_storage(&mut preflight),
        Err(GeneratedRuntimeArgumentErrorV1::PayloadLimit)
    ));
    assert_eq!(preflight.footprint(), before);
    assert!(matches!(
        *first.custody.legacy().state.lock().unwrap(),
        OutputState::Unbound
    ));
    assert!(matches!(
        *second.custody.legacy().state.lock().unwrap(),
        OutputState::Unbound
    ));
    assert_eq!(&*first.values, &[3]);
    assert!(first_observer.try_take().unwrap().is_none());
    assert!(second_observer.try_take().unwrap().is_none());
    drop((first, second));
    assert!(matches!(
        first_observer.try_take(),
        Err(GeneratedRuntimeArgumentErrorV1::OutputUnavailable)
    ));
    assert!(matches!(
        second_observer.try_take(),
        Err(GeneratedRuntimeArgumentErrorV1::OutputUnavailable)
    ));
}

#[test]
fn owned_type_access_and_index_mapping_substitution_reject() {
    let plan = plan::<u32>(&[Access::ReadWrite], None);
    let (wrong_type, _) = GeneratedRuntimeReadWriteSlice::new(vec![1_f32].into_boxed_slice());
    assert!(matches!(
        wrong_type.bind_argument(&plan, 0, &mut budget(&plan)),
        Err(GeneratedRuntimeArgumentErrorV1::Pack(_))
    ));
    let wrong_access = GeneratedRuntimeReadSlice::new(vec![1_u32].into_boxed_slice());
    assert!(matches!(
        wrong_access.bind_argument(&plan, 0, &mut budget(&plan)),
        Err(GeneratedRuntimeArgumentErrorV1::Pack(_))
    ));
    let (wrong_mapping, _) = GeneratedRuntimeReadWriteSlice::new(vec![1_u32].into_boxed_slice());
    assert!(matches!(
        wrong_mapping.bind_mapped_argument(
            &plan,
            0,
            RustDisjointIndexSpaceV1::ShiftedIndex1D { offset: 1 },
            &mut budget(&plan)
        ),
        Err(GeneratedRuntimeArgumentErrorV1::Pack(_))
    ));
}

#[test]
fn stale_private_output_custody_and_duplicate_binding_reject() {
    let plan = plan::<u32>(&[Access::ReadWrite, Access::ReadWrite], None);
    let mut budget = budget(&plan);
    let (first, mut observer) = GeneratedRuntimeReadWriteSlice::new(vec![1_u32].into_boxed_slice());
    let duplicate = GeneratedRuntimeReadWriteSlice {
        values: vec![2_u32].into_boxed_slice(),
        custody: OutputCustody::Legacy(LegacyOutputCustody {
            state: Arc::clone(&first.custody.legacy().state),
            scalar: u32::RUST_SCALAR_TYPE,
        }),
    };
    let first = first.bind_argument(&plan, 0, &mut budget).unwrap();
    assert!(matches!(
        duplicate.bind_argument(&plan, 1, &mut budget),
        Err(GeneratedRuntimeArgumentErrorV1::StaleOrAliasedOutput)
    ));
    assert!(matches!(
        observer.try_take(),
        Err(GeneratedRuntimeArgumentErrorV1::OutputUnavailable)
    ));
    drop(first);

    let (stale, _) = GeneratedRuntimeReadWriteSlice::new(vec![1_u32].into_boxed_slice());
    *stale.custody.legacy().state.lock().unwrap() = OutputState::Taken;
    assert!(matches!(
        stale.bind_argument(&plan, 0, &mut budget),
        Err(GeneratedRuntimeArgumentErrorV1::StaleOrAliasedOutput)
    ));
}

#[test]
fn dropped_observer_does_not_withdraw_owned_storage_or_decoder_custody() {
    let (packed, observer) = output_fixture();
    let weak = Arc::downgrade(&observer.state);
    drop(observer);
    assert!(weak.upgrade().is_some());
    std::thread::spawn(move || {
        assert_eq!(packed.buffers()[0].bytes(), words(&[3, 7, 11, 19]));
        let GeneratedRuntimePackedArgumentsV1 {
            packed, decoder, ..
        } = packed;
        drop(packed);
        decoder
            .decode_buffers(vec![(
                Gfx942RuntimeBufferAccessV1::ReadWrite,
                words(&[5, 9, 13, 21]),
            )])
            .unwrap();
    })
    .join()
    .unwrap();
    assert!(weak.upgrade().is_none());
}

#[test]
fn owned_result_outlives_input_and_rejects_wrong_type_and_repeated_take() {
    let (packed, mut observer) = output_fixture();
    let GeneratedRuntimePackedArgumentsV1 {
        packed, decoder, ..
    } = packed;
    drop(packed);
    decoder
        .decode_buffers(vec![(
            Gfx942RuntimeBufferAccessV1::ReadWrite,
            words(&[5, 7, 11, 19]),
        )])
        .unwrap();
    let mut forged_type = GeneratedRuntimeResultV1::<f32> {
        state: Arc::clone(&observer.state),
        scalar: PhantomData,
    };
    assert!(matches!(
        forged_type.try_take(),
        Err(GeneratedRuntimeArgumentErrorV1::BindingMismatch)
    ));
    assert_eq!(&*observer.try_take().unwrap().unwrap(), &[5, 7, 11, 19]);
    assert!(matches!(
        observer.try_take(),
        Err(GeneratedRuntimeArgumentErrorV1::OutputUnavailable)
    ));
}

#[test]
fn decoder_checks_every_shape_before_delivering_any_output() {
    let plan = plan::<u32>(&[Access::ReadWrite, Access::ReadWrite], None);
    let mut budget = budget(&plan);
    let (first, mut first_observer) =
        GeneratedRuntimeReadWriteSlice::new(vec![1_u32].into_boxed_slice());
    let (second, mut second_observer) =
        GeneratedRuntimeReadWriteSlice::new(vec![2_u32].into_boxed_slice());
    let bindings = vec![
        first.bind_argument(&plan, 0, &mut budget).unwrap(),
        second.bind_argument(&plan, 1, &mut budget).unwrap(),
    ];
    let packed = GeneratedRuntimeArgumentBindingV1::from_compiler_generated_parts(vec![], bindings)
        .pack(&plan, budget)
        .unwrap();
    assert!(matches!(
        packed.decoder.decode_buffers(vec![
            (Gfx942RuntimeBufferAccessV1::ReadWrite, words(&[10])),
            (Gfx942RuntimeBufferAccessV1::ReadOnly, words(&[20])),
        ]),
        Err(GeneratedRuntimeArgumentErrorV1::BindingMismatch)
    ));
    assert!(matches!(
        first_observer.try_take(),
        Err(GeneratedRuntimeArgumentErrorV1::OutputUnavailable)
    ));
    assert!(matches!(
        second_observer.try_take(),
        Err(GeneratedRuntimeArgumentErrorV1::OutputUnavailable)
    ));
}

#[test]
fn uncharged_binding_and_excess_result_capacity_reject() {
    let plan = plan::<u32>(&[Access::ReadWrite], None);
    let mut charged = budget(&plan);
    let (output, _) = GeneratedRuntimeReadWriteSlice::new(vec![1_u32].into_boxed_slice());
    let binding = output.bind_argument(&plan, 0, &mut charged).unwrap();
    assert!(matches!(
        GeneratedRuntimeArgumentBindingV1::from_compiler_generated_parts(vec![], vec![binding])
            .pack(&plan, budget(&plan)),
        Err(GeneratedRuntimeArgumentErrorV1::BindingMismatch)
    ));
    let (packed, mut observer) = output_fixture();
    let mut bytes = Vec::with_capacity(4096);
    bytes.extend_from_slice(&words(&[1, 2, 3, 4]));
    assert!(matches!(
        packed
            .decoder
            .decode_buffers(vec![(Gfx942RuntimeBufferAccessV1::ReadWrite, bytes)]),
        Err(GeneratedRuntimeArgumentErrorV1::BindingMismatch)
    ));
    assert!(matches!(
        observer.try_take(),
        Err(GeneratedRuntimeArgumentErrorV1::OutputUnavailable)
    ));
}

#[test]
fn empty_write_only_output_and_mapped_initialized_seed_are_preserved() {
    let mapping = RustDisjointIndexSpaceV1::ShiftedIndex1D { offset: 1 };
    let plan = plan::<u32>(&[Access::WriteOnly, Access::WriteOnly], Some(mapping));
    let mut budget = budget(&plan);
    let (empty, mut empty_observer) =
        GeneratedRuntimeWriteSlice::new(Vec::<u32>::new().into_boxed_slice());
    let (output, mut observer) =
        GeneratedRuntimeWriteSlice::new(vec![17_u32, 23].into_boxed_slice());
    let bindings = vec![
        empty
            .bind_mapped_argument(&plan, 0, mapping, &mut budget)
            .unwrap(),
        output
            .bind_mapped_argument(&plan, 1, mapping, &mut budget)
            .unwrap(),
    ];
    let packed = GeneratedRuntimeArgumentBindingV1::from_compiler_generated_parts(vec![], bindings)
        .pack(&plan, budget)
        .unwrap();
    assert_eq!(packed.buffers().len(), 1);
    assert_eq!(packed.buffers()[0].bytes(), words(&[17, 23]));
    assert_eq!(
        packed.buffers()[0].access(),
        Gfx942RuntimeBufferAccessV1::WriteOnly
    );
    packed
        .decoder
        .decode_buffers(vec![(
            Gfx942RuntimeBufferAccessV1::WriteOnly,
            words(&[17, 29]),
        )])
        .unwrap();
    assert!(empty_observer.try_take().unwrap().unwrap().is_empty());
    assert_eq!(&*observer.try_take().unwrap().unwrap(), &[17, 29]);
}
