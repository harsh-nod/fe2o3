use super::{
    generated_worker_v3_adapter_v1, model_general_typed_signature_v1, parse_kernel_options,
};
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use syn::{File, ImplItem, ImplItemFn, Item, ItemFn, parse_quote, visit_mut::VisitMut};

fn generate(input: &ItemFn) -> File {
    let options = parse_kernel_options(quote!(typed)).unwrap();
    let model = model_general_typed_signature_v1(input, &options, [0x71; 32]).unwrap();
    syn::parse2(generated_worker_v3_adapter_v1(input, &model)).unwrap()
}

fn mixed_adapter() -> File {
    generate(&parse_quote! {
        pub fn mixed_owned(
            tag: u8,
            input: &[u16],
            scale: f64,
            output: WriteOnlyDisjointSlice<u32>,
            update: DisjointSlice<i32>,
            shifted: WriteOnlyDisjointSlice<u64, Shifted<Index1D, 7>>,
            count: u32,
            blocked: DisjointSlice<i16, Blocked<Index1D, 16, 4>>,
        ) {}
    })
}

fn syntax(file: File) -> String {
    file.into_token_stream().to_string()
}

fn expected_mixed_adapter() -> File {
    let shifted = quote! {
        __fe2o3_kernel_host::__generated::RustDisjointIndexSpaceV1::ShiftedIndex1D { offset: 7u64, }
    };
    let blocked = quote! {
        __fe2o3_kernel_host::__generated::RustDisjointIndexSpaceV1::blocked_index_1d(16u64, 4u64,)
            .expect("compiler-generated blocked index space is valid")
    };
    // Literal fixture ABI: no production layout or index-space generator supplies this oracle.
    let scalar = |name: &str, offset: u64, size: u64, ty: TokenStream, kind: TokenStream| {
        let alignment = u32::try_from(size).unwrap();
        quote! {
            __fe2o3_kernel_host::__generated::AbiField::new(
                __fe2o3_kernel_host::__generated::Name::new(#name)
                    .expect("generated Worker V3 argument name is valid"),
                #offset, #size, #alignment,
                __fe2o3_kernel_host::__generated::AbiKind::Scalar(
                    __fe2o3_kernel_host::__generated::ScalarType::#kind,
                ),
                __fe2o3_kernel_host::__generated::Mutability::Immutable,
                __fe2o3_kernel_host::__generated::Access::ByValue,
                __fe2o3_kernel_host::__generated::AddressSpace::Value,
                <#ty as __fe2o3_kernel_host::__generated::GeneratedDeviceScalarV1>::scalar_type_identity_v1(
                    __fe2o3_kernel_host::__generated::PointerWidth::Bits64,
                ),
                __fe2o3_kernel_host::__generated::ArgumentOwnership::ByValue,
                __fe2o3_kernel_host::__generated::AliasClass::Value,
            ).expect("generated Worker V3 ABI field is valid")
        }
    };
    let slice = |name: &str,
                 offset: u64,
                 element: u64,
                 access: TokenStream,
                 mutability: TokenStream,
                 ownership: TokenStream,
                 alias: TokenStream,
                 identity: TokenStream| {
        let element_alignment = u32::try_from(element).unwrap();
        quote! {
            __fe2o3_kernel_host::__generated::AbiField::new(
                __fe2o3_kernel_host::__generated::Name::new(#name)
                    .expect("generated Worker V3 argument name is valid"),
                #offset, 16u64, 8u32,
                __fe2o3_kernel_host::__generated::AbiKind::Slice {
                    element_size: #element, element_alignment: #element_alignment,
                },
                __fe2o3_kernel_host::__generated::Mutability::#mutability,
                __fe2o3_kernel_host::__generated::Access::#access,
                __fe2o3_kernel_host::__generated::AddressSpace::Global,
                #identity,
                __fe2o3_kernel_host::__generated::ArgumentOwnership::#ownership,
                __fe2o3_kernel_host::__generated::AliasClass::#alias,
            ).expect("generated Worker V3 ABI field is valid")
        }
    };
    let fields = [
        scalar("arg0", 0, 1, quote!(u8), quote!(U8)),
        slice(
            "arg1",
            8,
            2,
            quote!(ReadOnly),
            quote!(Immutable),
            quote!(SharedBorrow),
            quote!(SharedReadOnly),
            quote! {
                <u16 as __fe2o3_kernel_host::__generated::GeneratedDeviceScalarV1>::shared_slice_type_identity_v1(
                    __fe2o3_kernel_host::__generated::PointerWidth::Bits64,
                )
            },
        ),
        scalar("arg2", 24, 8, quote!(f64), quote!(F64)),
        slice(
            "arg3",
            32,
            4,
            quote!(WriteOnly),
            quote!(Mutable),
            quote!(UniqueBorrow),
            quote!(Exclusive),
            quote! {
                <u32 as __fe2o3_kernel_host::__generated::GeneratedDeviceScalarV1>::disjoint_slice_type_identity_v1(
                    __fe2o3_kernel_host::__generated::PointerWidth::Bits64,
                )
            },
        ),
        slice(
            "arg4",
            48,
            4,
            quote!(ReadWrite),
            quote!(Mutable),
            quote!(UniqueBorrow),
            quote!(Exclusive),
            quote! {
                <i32 as __fe2o3_kernel_host::__generated::GeneratedDeviceScalarV1>::disjoint_slice_type_identity_v1(
                    __fe2o3_kernel_host::__generated::PointerWidth::Bits64,
                )
            },
        ),
        slice(
            "arg5",
            64,
            8,
            quote!(WriteOnly),
            quote!(Mutable),
            quote!(UniqueBorrow),
            quote!(Exclusive),
            quote! {
                <u64 as __fe2o3_kernel_host::__generated::GeneratedDeviceScalarV1>::disjoint_slice_type_identity_for_index_space_v1(
                    __fe2o3_kernel_host::__generated::PointerWidth::Bits64, #shifted,
                )
            },
        ),
        scalar("arg6", 80, 4, quote!(u32), quote!(U32)),
        slice(
            "arg7",
            88,
            2,
            quote!(ReadWrite),
            quote!(Mutable),
            quote!(UniqueBorrow),
            quote!(Exclusive),
            quote! {
                <i16 as __fe2o3_kernel_host::__generated::GeneratedDeviceScalarV1>::disjoint_slice_type_identity_for_index_space_v1(
                    __fe2o3_kernel_host::__generated::PointerWidth::Bits64, #blocked,
                )
            },
        ),
    ];
    let layout = quote! {
        __fe2o3_kernel_host::__generated::CompilerGeneratedArgumentLayoutV1::new_with_disjoint_index_spaces_v1(
            104u64, 8u32, __fe2o3_kernel_host::__generated::PointerWidth::Bits64,
            [#(#fields),*].into_iter().collect(),
            [None, None, None, None, None, Some(#shifted), None, Some(#blocked)].into_iter().collect(),
        )
    };
    parse_quote! {
        /// Opaque owned data for this exact kernel signature. This is not launch authority.
        #[must_use = "owned generated arguments retain storage but do not launch a kernel"]
        #[allow(dead_code)]
        pub struct RuntimeArguments {
            tag: u8,
            input: __fe2o3_kernel_host::__generated::GeneratedRuntimeReadSlice<u16>,
            scale: f64,
            output: __fe2o3_kernel_host::__generated::GeneratedRuntimeWriteSlice<u32>,
            update: __fe2o3_kernel_host::__generated::GeneratedRuntimeReadWriteSlice<i32>,
            shifted: __fe2o3_kernel_host::__generated::GeneratedRuntimeWriteSlice<u64>,
            count: u32,
            blocked: __fe2o3_kernel_host::__generated::GeneratedRuntimeReadWriteSlice<i16>,
        }

        impl RuntimeArguments {
            #[allow(clippy::too_many_arguments)]
            pub fn new(
                tag: u8,
                input: __fe2o3_kernel_host::__generated::GeneratedRuntimeReadSlice<u16>,
                scale: f64,
                output: __fe2o3_kernel_host::__generated::GeneratedRuntimeWriteSlice<u32>,
                update: __fe2o3_kernel_host::__generated::GeneratedRuntimeReadWriteSlice<i32>,
                shifted: __fe2o3_kernel_host::__generated::GeneratedRuntimeWriteSlice<u64>,
                count: u32,
                blocked: __fe2o3_kernel_host::__generated::GeneratedRuntimeReadWriteSlice<i16>
            ) -> Self {
                Self { tag, input, scale, output, update, shifted, count, blocked }
            }
        }

        unsafe impl __fe2o3_kernel_host::__generated::CompilerGeneratedRuntimeArguments<Marker>
            for RuntimeArguments
        {
            fn generated_argument_layout() -> Result<
                __fe2o3_kernel_host::__generated::CompilerGeneratedArgumentLayoutV1,
                __fe2o3_kernel_host::__generated::GeneratedArgumentLayoutError,
            > {
                #layout
            }

            fn account_runtime_arguments(
                &self,
                budget: &mut __fe2o3_kernel_host::__generated::GeneratedRuntimeArgumentBudgetV1,
            ) -> Result<(), __fe2o3_kernel_host::__generated::GeneratedRuntimeArgumentErrorV1> {
                self.input.account_storage(budget)?;
                self.output.account_storage(budget)?;
                self.update.account_storage(budget)?;
                self.shifted.account_storage(budget)?;
                self.blocked.account_storage(budget)?;
                Ok(())
            }

            fn bind_runtime_arguments(
                self,
                plan: &__fe2o3_kernel_host::__generated::GeneratedArgumentPackingPlanV1,
                budget: &mut __fe2o3_kernel_host::__generated::GeneratedRuntimeArgumentBudgetV1,
            ) -> Result<
                __fe2o3_kernel_host::__generated::GeneratedRuntimeArgumentBindingV1,
                __fe2o3_kernel_host::__generated::GeneratedRuntimeArgumentErrorV1,
            > {
                let scalar_inputs = [
                    plan.scalar(0usize, self.tag).map_err(__fe2o3_kernel_host::__generated::GeneratedRuntimeArgumentErrorV1::Pack)?,
                    plan.scalar(2usize, self.scale).map_err(__fe2o3_kernel_host::__generated::GeneratedRuntimeArgumentErrorV1::Pack)?,
                    plan.scalar(6usize, self.count).map_err(__fe2o3_kernel_host::__generated::GeneratedRuntimeArgumentErrorV1::Pack)?
                ].into_iter().collect();
                let memory_arguments = [
                    self.input.bind_argument(plan, 1usize, budget)?,
                    self.output.bind_argument(plan, 3usize, budget)?,
                    self.update.bind_argument(plan, 4usize, budget)?,
                    self.shifted.bind_mapped_argument(plan, 5usize, #shifted, budget)?,
                    self.blocked.bind_mapped_argument(plan, 7usize, #blocked, budget)?
                ].into_iter().collect();
                Ok(__fe2o3_kernel_host::__generated::GeneratedRuntimeArgumentBindingV1::
                    from_compiler_generated_parts(scalar_inputs, memory_arguments))
            }
        }

        unsafe impl<'allocation>
            __fe2o3_kernel_host::__generated::CompilerGeneratedKfdArguments<'allocation, Marker,>
            for Arguments<'allocation,
                __fe2o3_kernel_host::__generated::GeneratedKfdReadSlice<'allocation, u16>,
                __fe2o3_kernel_host::__generated::GeneratedKfdWriteSlice<'allocation, u32>,
                __fe2o3_kernel_host::__generated::GeneratedKfdReadWriteSlice<'allocation, i32>,
                __fe2o3_kernel_host::__generated::GeneratedKfdWriteSlice<'allocation, u64>,
                __fe2o3_kernel_host::__generated::GeneratedKfdReadWriteSlice<'allocation, i16>
            >
        {
            fn generated_argument_layout() -> Result<
                __fe2o3_kernel_host::__generated::CompilerGeneratedArgumentLayoutV1,
                __fe2o3_kernel_host::__generated::GeneratedArgumentLayoutError,
            > {
                #layout
            }

            fn bind_kfd_arguments(
                self,
                plan: &__fe2o3_kernel_host::__generated::GeneratedArgumentPackingPlanV1,
            ) -> Result<
                __fe2o3_kernel_host::__generated::GeneratedKfdArgumentBinding<'allocation>,
                __fe2o3_kernel_host::__generated::GeneratedKfdArgumentError,
            > {
                let scalar_inputs = [
                    plan.scalar(0usize, self.tag).map_err(__fe2o3_kernel_host::__generated::GeneratedKfdArgumentError::Pack)?,
                    plan.scalar(2usize, self.scale).map_err(__fe2o3_kernel_host::__generated::GeneratedKfdArgumentError::Pack)?,
                    plan.scalar(6usize, self.count).map_err(__fe2o3_kernel_host::__generated::GeneratedKfdArgumentError::Pack)?
                ].into_iter().collect();
                let memory_arguments = [
                    self.input.bind_argument(plan, 1usize)?,
                    self.output.bind_argument(plan, 3usize)?,
                    self.update.bind_argument(plan, 4usize)?,
                    self.shifted.bind_mapped_argument(plan, 5usize, #shifted)?,
                    self.blocked.bind_mapped_argument(plan, 7usize, #blocked)?
                ].into_iter().collect();
                Ok(__fe2o3_kernel_host::__generated::GeneratedKfdArgumentBinding::
                    from_compiler_generated_parts(scalar_inputs, memory_arguments),)
            }
        }
    }
}

#[test]
fn mixed_owned_adapter_preserves_exact_layout_access_ordinals_and_budget() {
    assert_eq!(syntax(mixed_adapter()), syntax(expected_mixed_adapter()));
}

fn runtime_method_mut<'a>(file: &'a mut File, name: &str) -> &'a mut ImplItemFn {
    file.items
        .iter_mut()
        .find_map(|item| {
            let Item::Impl(item) = item else { return None };
            if item.self_ty.to_token_stream().to_string() != "RuntimeArguments"
                || item.trait_.is_none()
            {
                return None;
            }
            item.items.iter_mut().find_map(|item| match item {
                ImplItem::Fn(item) if item.sig.ident == name => Some(item),
                _ => None,
            })
        })
        .unwrap()
}

fn rewrite_first_call(
    function: &mut ImplItemFn,
    name: &str,
    rewrite: impl FnMut(&mut syn::ExprMethodCall),
) {
    struct Rewrite<'a, F> {
        name: &'a str,
        rewrite: F,
        changed: bool,
    }
    impl<F: FnMut(&mut syn::ExprMethodCall)> VisitMut for Rewrite<'_, F> {
        fn visit_expr_method_call_mut(&mut self, call: &mut syn::ExprMethodCall) {
            if !self.changed && call.method == self.name {
                (self.rewrite)(call);
                self.changed = true;
            } else {
                syn::visit_mut::visit_expr_method_call_mut(self, call);
            }
        }
    }
    let mut visitor = Rewrite {
        name,
        rewrite,
        changed: false,
    };
    visitor.visit_block_mut(&mut function.block);
    assert!(visitor.changed, "mutation did not reach {name}");
}

#[test]
fn owned_adapter_oracle_rejects_accounting_binding_and_unsafe_mutations() {
    let expected = syntax(expected_mixed_adapter());
    assert_eq!(syntax(mixed_adapter()), expected);
    for mutation in 0..9 {
        let mut changed = mixed_adapter();
        match mutation {
            0 => {
                runtime_method_mut(&mut changed, "account_runtime_arguments")
                    .block
                    .stmts
                    .remove(0);
            }
            1 => {
                let block =
                    &mut runtime_method_mut(&mut changed, "account_runtime_arguments").block;
                block.stmts.insert(0, block.stmts[0].clone());
            }
            2 => rewrite_first_call(
                runtime_method_mut(&mut changed, "bind_runtime_arguments"),
                "bind_argument",
                |call| {
                    call.args[1] = parse_quote!(0usize);
                },
            ),
            3 => rewrite_first_call(
                runtime_method_mut(&mut changed, "bind_runtime_arguments"),
                "bind_mapped_argument",
                |call| {
                    call.method = parse_quote!(bind_argument);
                    call.args = call
                        .args
                        .iter()
                        .enumerate()
                        .filter(|(index, _)| *index != 2)
                        .map(|(_, argument)| argument.clone())
                        .collect();
                },
            ),
            4 => rewrite_first_call(
                runtime_method_mut(&mut changed, "bind_runtime_arguments"),
                "bind_mapped_argument",
                |call| {
                    call.args[2] = parse_quote!(__fe2o3_kernel_host::__generated::RustDisjointIndexSpaceV1::blocked_index_1d(16u64, 4u64,).expect("compiler-generated blocked index space is valid"));
                },
            ),
            5 => runtime_method_mut(&mut changed, "bind_runtime_arguments")
                .block
                .stmts
                .insert(0, parse_quote!(unsafe {};)),
            6 => changed.items.push(parse_quote! {
                impl Clone for RuntimeArguments {
                    fn clone(&self) -> Self { unimplemented!() }
                }
            }),
            7 => rewrite_first_call(
                runtime_method_mut(&mut changed, "generated_argument_layout"),
                "collect",
                |call| {
                    *call.receiver = parse_quote!([].into_iter());
                },
            ),
            8 => {
                let Item::Struct(item) = &mut changed.items[0] else {
                    panic!("expected generated owned argument struct");
                };
                item.attrs
                    .push(parse_quote!(#[doc = include_str!("unexpected-input")]));
            }
            _ => unreachable!(),
        }
        assert_ne!(
            syntax(changed),
            expected,
            "mutation {mutation} escaped the oracle"
        );
    }
}

#[test]
fn one_unsupported_member_suppresses_the_entire_owned_and_borrowed_adapter() {
    let cases: [ItemFn; 4] = [
        parse_quote!(
            pub fn raw(tag: u8, input: &[u32], pointer: fe2o3_device::DeviceGlobalMutPtr<u32>) {}
        ),
        parse_quote!(
            pub fn array(input: &[u32], value: [u32; 4]) {}
        ),
        parse_quote!(
            pub fn tuple(input: &[u32], value: (u32, u64)) {}
        ),
        parse_quote!(
            pub fn record(input: &[u32], value: Parameters) {}
        ),
    ];
    for input in cases {
        assert!(
            generate(&input).items.is_empty(),
            "{} emitted a partial adapter",
            input.sig.ident
        );
    }
}

#[test]
fn zero_argument_owned_adapter_preserves_main_nullary_profile() {
    let input: ItemFn = parse_quote!(
        pub fn empty() {}
    );
    let generated = generate(&input);
    let arguments = generated
        .items
        .iter()
        .find_map(|item| match item {
            Item::Struct(item) if item.ident == "RuntimeArguments" => Some(item),
            _ => None,
        })
        .unwrap();
    assert!(arguments.fields.is_empty());
    assert!(arguments.generics.params.is_empty());
    let text = syntax(generated);
    assert!(text.contains("CompilerGeneratedRuntimeArguments < Marker >"));
    assert!(text.contains("pub fn new () -> Self"));
    assert!(!text.contains("account_storage"));
    assert!(!text.contains("plan . scalar"));
    assert!(!text.contains("bind_mapped_argument"));
}
