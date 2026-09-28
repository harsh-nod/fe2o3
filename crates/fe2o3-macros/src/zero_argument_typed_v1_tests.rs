use super::*;
use syn::Item;

#[test]
fn zero_argument_typed_profile_has_empty_abi_and_bound_identity() {
    let input: ItemFn = parse_quote!(
        pub fn empty() {}
    );
    let options = parse_kernel_options(quote!(typed, launch(required = [64, 1, 1]))).unwrap();
    validate_typed_kernel_profile_v1(&input, &options).unwrap();
    let model = model_general_typed_signature_v1(&input, &options, [0x81; 32]).unwrap();
    assert!(model.arguments.is_empty());
    assert!(model.abi.fields().is_empty());
    assert_eq!((model.abi.size(), model.abi.alignment()), (0, 1));
    assert_eq!(model.abi.pointer_width(), PointerWidth::Bits64);
    let identity = model.generated_host_contract_identity;
    assert_eq!(
        identity,
        derive_generated_host_contract_identity_v1(
            MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
            [0x81; 32],
            "empty",
            "empty",
            &AbiLayout::new(0, 1, PointerWidth::Bits64, Vec::new()).unwrap(),
            &model.launch,
        )
    );
    for (signature, options, binding) in [
        (
            parse_quote!(
                pub fn other() {}
            ),
            options.clone(),
            [0x81; 32],
        ),
        (input.clone(), options.clone(), [0x82; 32]),
        (
            input.clone(),
            parse_kernel_options(quote!(typed)).unwrap(),
            [0x81; 32],
        ),
        (
            parse_quote!(
                pub fn empty(value: u32) {}
            ),
            options,
            [0x81; 32],
        ),
    ] {
        assert_ne!(
            identity,
            model_general_typed_signature_v1(&signature, &options, binding)
                .unwrap()
                .generated_host_contract_identity
        );
    }
}

#[test]
fn zero_argument_host_values_have_only_a_nullary_constructor() {
    let input: ItemFn = parse_quote!(
        pub fn empty() {}
    );
    let model = model_general_typed_signature_v1(
        &input,
        &parse_kernel_options(quote!(typed)).unwrap(),
        [0x81; 32],
    )
    .unwrap();
    let generated: syn::File = syn::parse2(generated_general_typed_arguments_v1(
        &input,
        &model.arguments,
    ))
    .unwrap();
    let arguments = generated
        .items
        .iter()
        .find_map(|item| match item {
            Item::Struct(item) if item.ident == "Arguments" => Some(item),
            _ => None,
        })
        .unwrap();
    assert!(arguments.generics.params.is_empty());
    assert!(arguments.fields.is_empty());
    let methods = generated
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Impl(item) => Some(&item.items),
            _ => None,
        })
        .flatten()
        .filter_map(|item| match item {
            syn::ImplItem::Fn(item) => Some(item),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].sig.ident, "new");
    assert!(methods[0].sig.inputs.is_empty());
    let adapter = generated_worker_v3_adapter_v1(&input, &model);
    syn::parse2::<syn::File>(adapter.clone()).unwrap();
    let adapter = adapter.to_string();
    assert!(adapter.contains("CompilerGeneratedKfdArguments"));
    assert!(!adapter.contains("plan . scalar"));
    assert!(!adapter.contains("bind_argument"));
    assert!(!adapter.contains("from_raw"));
}

#[test]
fn zero_argument_profile_preserves_function_and_launch_restrictions() {
    let options = parse_kernel_options(quote!(typed)).unwrap();
    for input in [
        parse_quote!(
            fn private() {}
        ),
        parse_quote!(
            pub unsafe fn unsafe_kernel() {}
        ),
        parse_quote!(
            pub async fn asynchronous() {}
        ),
        parse_quote!(
            pub const fn constant() {}
        ),
        parse_quote!(
            pub extern "C" fn foreign() {}
        ),
        parse_quote!(
            pub fn generic<T>() {}
        ),
        parse_quote!(
            pub fn nonunit() -> u32 {
                0
            }
        ),
    ] {
        assert!(validate_typed_kernel_profile_v1(&input, &options).is_err());
    }
    let input: ItemFn = parse_quote!(
        pub fn empty() {}
    );
    for options in [
        quote!(typed, launch(required = [257, 1, 1])),
        quote!(typed, launch(required = [64, 1, 1], max = [128, 1, 1])),
    ] {
        assert!(
            validate_typed_kernel_profile_v1(&input, &parse_kernel_options(options).unwrap())
                .is_err()
        );
    }
}
