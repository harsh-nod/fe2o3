use super::*;
use syn::parse_quote;

#[test]
fn atomic_entry_custody_has_mutable_slice_abi_and_no_logical_bytes() {
    let input: ItemFn = parse_quote! {
        pub fn atomic(context: KernelContext<'_>, counters: Global<'_, u32, AtomicReadWrite<SystemScope>>) {}
    };
    let physical = physical_typed_kernel_input_v1(&input, &quote!(fe2o3_device)).unwrap();
    let options = parse_kernel_options(quote!(typed)).unwrap();
    let model = model_general_typed_signature_v1(&physical, &options, [0x45; 32]).unwrap();
    assert_eq!(physical.sig.inputs.len(), 1);
    assert_eq!(
        model.arguments,
        [GeneralTypedArgumentKindV1::MutableSlice(
            GeneralTypedScalarV1::U32
        )]
    );
    assert_eq!((model.abi.size(), model.abi.alignment()), (16, 8));
    let field = &model.abi.fields()[0];
    assert_eq!(
        (field.offset(), field.size(), field.alignment()),
        (0, 16, 8)
    );
    assert_eq!(
        field.kind(),
        AbiKind::Slice {
            element_size: 4,
            element_alignment: 4
        }
    );
    assert_eq!(field.access(), Access::ReadWrite);
    assert_eq!(field.mutability(), Mutability::Mutable);
    assert_eq!(field.address_space(), AddressSpace::Global);
    assert_eq!(field.ownership(), ArgumentOwnership::UniqueBorrow);
    assert_eq!(field.alias_class(), AliasClass::Exclusive);

    let evidence = RustLayoutEvidenceV1::new(
        RustTypeEvidenceV1::new(RustSourceTypeShapeV1::mutable_slice(
            RustScalarElementTypeV1::U32,
        )),
        RustcAbiClassV1::ScalarPair,
        PointerWidth::Bits64,
        16,
        8,
        vec![
            RustPhysicalComponentV1::new(
                0,
                8,
                8,
                RustPhysicalComponentKindV1::Pointer {
                    mutability: RustPointerMutabilityV1::Mut,
                    pointee: RustScalarElementTypeV1::U32,
                },
            )
            .unwrap(),
            RustPhysicalComponentV1::new(8, 8, 8, RustPhysicalComponentKindV1::Usize).unwrap(),
        ],
    )
    .unwrap();
    assert_eq!(field.type_identity(), evidence.type_identity());

    let shared: ItemFn = parse_quote!(
        pub fn atomic(counters: &[u32]) {}
    );
    let old = model_general_typed_signature_v1(&shared, &options, [0x45; 32]).unwrap();
    assert_eq!((old.abi.size(), old.abi.alignment()), (16, 8));
    assert_ne!(field.type_identity(), old.abi.fields()[0].type_identity());
    assert_ne!(
        model.generated_host_contract_identity,
        old.generated_host_contract_identity
    );
    let adapter = generated_worker_v3_adapter_v1(&physical, &model).to_string();
    assert!(adapter.contains("bind_mutable_argument"));
    let layout = generated_worker_v3_layout_v1(&model).to_string();
    assert!(layout.contains("mutable_slice_type_identity_v1"));
    assert!(!layout.contains("shared_slice_type_identity_v1"));
}

#[test]
fn atomic_entry_custody_preserves_other_role_contracts() {
    let input: ItemFn = parse_quote! {
        pub fn roles(
            context: KernelContext<'_>,
            read: Global<'_, u32, ReadOnly>,
            write: Global<'_, u32, DisjointWrite<Index1D>>,
            exclusive: Global<'_, u32, ExclusiveReadWrite>,
            atomic: Global<'_, u32, AtomicReadWrite<SystemScope>>,
        ) {}
    };
    let physical = physical_typed_kernel_input_v1(&input, &quote!(fe2o3_device)).unwrap();
    let model = model_general_typed_signature_v1(
        &physical,
        &parse_kernel_options(quote!(typed)).unwrap(),
        [0; 32],
    )
    .unwrap();
    assert_eq!((model.abi.size(), model.abi.alignment()), (64, 8));
    assert_eq!(
        model
            .abi
            .fields()
            .iter()
            .map(|f| (f.offset(), f.size()))
            .collect::<Vec<_>>(),
        [(0, 16), (16, 16), (32, 16), (48, 16)]
    );
    assert_eq!(
        model
            .abi
            .fields()
            .iter()
            .map(|f| f.access())
            .collect::<Vec<_>>(),
        [
            Access::ReadOnly,
            Access::WriteOnly,
            Access::ReadWrite,
            Access::ReadWrite
        ]
    );
    assert_eq!(
        model
            .abi
            .fields()
            .iter()
            .map(|f| f.ownership())
            .collect::<Vec<_>>(),
        [
            ArgumentOwnership::SharedBorrow,
            ArgumentOwnership::UniqueBorrow,
            ArgumentOwnership::UniqueBorrow,
            ArgumentOwnership::UniqueBorrow
        ]
    );
}
