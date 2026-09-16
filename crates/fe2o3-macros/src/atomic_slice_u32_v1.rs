// Syntactic registration only; rustc independently authenticates the core DefId.
fn is_atomic_u32_spelling_v1(ty: &Type) -> bool {
    let Type::Path(path) = transparent_type_v1(ty) else {
        return false;
    };
    if path.qself.is_some()
        || path
            .path
            .segments
            .iter()
            .any(|segment| !matches!(segment.arguments, PathArguments::None))
    {
        return false;
    }
    let segments = path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    matches!(segments.as_slice(), [name] if name == "AtomicU32")
        || matches!(segments.as_slice(), [root, sync, atomic, name]
            if matches!(root.as_str(), "core" | "std") && sync == "sync" && atomic == "atomic" && name == "AtomicU32")
}

fn general_typed_atomic_slice_u32_identity_v1() -> TypeIdentity {
    let components = vec![
        RustPhysicalComponentV1::new(
            0,
            8,
            8,
            RustPhysicalComponentKindV1::Pointer {
                mutability: RustPointerMutabilityV1::Const,
                pointee: RustScalarElementTypeV1::U32,
            },
        )
        .expect("the atomic-slice pointer component is valid"),
        RustPhysicalComponentV1::new(8, 8, 8, RustPhysicalComponentKindV1::Usize)
            .expect("the atomic-slice length component is valid"),
    ];
    RustLayoutEvidenceV1::new(
        RustTypeEvidenceV1::new(RustSourceTypeShapeV1::SharedAtomicSliceU32),
        RustcAbiClassV1::ScalarPair,
        PointerWidth::Bits64,
        16,
        8,
        components,
    )
    .expect("the atomic-slice physical layout is valid")
    .type_identity()
}

fn generated_atomic_slice_u32_field_v1(field: &AbiField) -> proc_macro2::TokenStream {
    let name = field.name().as_str();
    let offset = field.offset();
    quote! {
        __fe2o3_kernel_host::__generated::AbiField::new(
            __fe2o3_kernel_host::__generated::Name::new(#name)
                .expect("generated atomic argument name is valid"),
            #offset, 16, 8,
            __fe2o3_kernel_host::__generated::AbiKind::Slice {
                element_size: 4, element_alignment: 4,
            },
            __fe2o3_kernel_host::__generated::Mutability::Immutable,
            __fe2o3_kernel_host::__generated::Access::ReadWrite,
            __fe2o3_kernel_host::__generated::AddressSpace::Global,
            __fe2o3_kernel_host::__generated::GeneratedKfdAtomicSliceU32::type_identity_v1(
                __fe2o3_kernel_host::__generated::PointerWidth::Bits64,
            ),
            __fe2o3_kernel_host::__generated::ArgumentOwnership::SharedBorrow,
            __fe2o3_kernel_host::__generated::AliasClass::SharedAtomic,
        ).expect("generated atomic-slice ABI field is valid")
    }
}

#[cfg(test)]
mod atomic_slice_u32_tests {
    use super::*;

    #[test]
    fn atomic_slice_registration_is_distinct_and_shared_writable() {
        for spelling in [
            "&[AtomicU32]",
            "&[core::sync::atomic::AtomicU32]",
            "&[::core::sync::atomic::AtomicU32]",
        ] {
            let ty: Type = syn::parse_str(spelling).unwrap();
            assert_eq!(
                parse_general_typed_argument_type_v1(&ty),
                Ok(GeneralTypedArgumentKindV1::SharedAtomicSliceU32)
            );
        }
        let field = general_typed_abi_field_v1(
            "channels".into(),
            0,
            GeneralTypedArgumentKindV1::SharedAtomicSliceU32,
        )
        .unwrap();
        assert_eq!(field.access(), Access::ReadWrite);
        assert_eq!(field.mutability(), Mutability::Immutable);
        assert_eq!(field.ownership(), ArgumentOwnership::SharedBorrow);
        assert_eq!(field.alias_class(), AliasClass::SharedAtomic);
        assert_ne!(
            field.type_identity(),
            general_typed_slice_type_identity_v1(GeneralTypedScalarV1::U32, false)
        );
        let generated = generated_atomic_slice_u32_field_v1(&field).to_string();
        assert!(generated.contains("GeneratedKfdAtomicSliceU32"));
        assert!(!generated.contains("GeneratedDeviceScalar"));
        assert!(!generated.contains("ReadOnly"));
    }

    #[test]
    fn atomic_slice_registration_rejects_other_widths_and_mutable_reference() {
        for spelling in [
            "&[AtomicU64]",
            "&[AtomicI32]",
            "&mut [AtomicU32]",
            "&[AtomicU32<u32>]",
        ] {
            let ty: Type = syn::parse_str(spelling).unwrap();
            assert!(parse_general_typed_argument_type_v1(&ty).is_err());
        }
        // Spelling grants only registration: the rustc source classifier must
        // still reject a local type shadowing this name.
        assert!(!is_atomic_u32_spelling_v1(
            &syn::parse_str::<Type>("fake::AtomicU32").unwrap()
        ));
    }
}
