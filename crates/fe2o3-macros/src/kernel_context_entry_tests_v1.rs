use super::*;
use crate::{expand_kernel_with_imports, parse_kernel_options};
use quote::ToTokens;
use reserved_fe2o3_symbols::derive_crate_binding_id_v1;
use syn::{Item, TypeBareFn};

fn expand(input: ItemFn, typed: bool) -> syn::File {
    let options = parse_kernel_options(if typed { quote!(typed) } else { quote!() }).unwrap();
    let binding = typed.then(|| derive_crate_binding_id_v1("context-test", ["entry-v1"]));
    let host = typed.then(|| {
        quote!(
            use gpu_host as __fe2o3_kernel_host;
        )
    });
    syn::parse2(
        expand_kernel_with_imports(
            input,
            options,
            &quote!(
                use gpu_device as __fe2o3_kernel_device;
            ),
            Some(&quote!(gpu_device)),
            host.as_ref(),
            binding,
        )
        .unwrap(),
    )
    .unwrap()
}

fn functions(file: &syn::File) -> Vec<&ItemFn> {
    file.items
        .iter()
        .filter_map(|item| match item {
            Item::Fn(f) => Some(f),
            _ => None,
        })
        .collect()
}

#[test]
fn context_is_absent_from_both_physical_entry_and_registration_abis() {
    for typed in [false, true] {
        let file = expand(
            parse_quote! {
                #[inline(always)]
                pub fn transform(ctx: KernelContext<'_>, factor: u32, mut output: DisjointSlice<u32>) {
                    let _ = (ctx, factor, &mut output);
                }
            },
            typed,
        );
        let functions = functions(&file);
        assert_eq!(functions.len(), 2);
        let helper = functions
            .iter()
            .find(|f| f.sig.ident.to_string().starts_with("__fe2o3_kernel_body"))
            .unwrap();
        let root = functions
            .iter()
            .find(|f| f.sig.ident != helper.sig.ident)
            .unwrap();
        assert_eq!(helper.sig.inputs.len(), 3);
        assert_eq!(root.sig.inputs.len(), 2);
        assert!(
            helper
                .sig
                .inputs
                .to_token_stream()
                .to_string()
                .contains("KernelContext < '_ , __fe2o3_kernel_marker_transform >")
        );
        assert!(
            helper
                .sig
                .inputs
                .to_token_stream()
                .to_string()
                .contains("mut output")
        );
        assert!(
            !root
                .sig
                .inputs
                .to_token_stream()
                .to_string()
                .contains("mut output")
        );
        assert_eq!(
            helper
                .attrs
                .iter()
                .filter(|a| a.path().is_ident("inline"))
                .count(),
            1
        );
        assert!(
            helper
                .attrs
                .iter()
                .any(|a| a.to_token_stream().to_string().contains("inline (never)"))
        );
        let body = root.block.to_token_stream().to_string();
        assert_eq!(body.matches("__compiler_issue").count(), 1);
        assert!(body.contains("__fe2o3_context_argument_0 , __fe2o3_context_argument_1"));
        assert!(
            helper
                .block
                .to_token_stream()
                .to_string()
                .contains("& mut output")
        );
        for item in &file.items {
            let Item::Static(item) = item else { continue };
            let Type::Tuple(tuple) = item.ty.as_ref() else {
                continue;
            };
            let Type::BareFn(TypeBareFn { inputs, .. }) = tuple.elems.last().unwrap() else {
                continue;
            };
            assert_eq!(inputs.len(), 2);
            assert!(
                !inputs
                    .to_token_stream()
                    .to_string()
                    .contains("KernelContext")
            );
        }
        let text = file.to_token_stream().to_string();
        assert!(text.contains("fn check < '__fe2o3 >"));
        assert!(text.contains(
            "PhantomData < fn (KernelContext < '__fe2o3 >) -> KernelContext < '__fe2o3 > >"
        ));
        if typed {
            let module = file
                .items
                .iter()
                .find_map(|item| match item {
                    Item::Mod(m) if m.ident == "transform_gpu" => Some(m),
                    _ => None,
                })
                .unwrap()
                .to_token_stream()
                .to_string();
            assert!(!module.contains("KernelContext"));
            assert!(!module.contains("ctx"));
        }
    }
}

#[test]
fn context_kernel_result_uses_one_helper_and_unit_physical_return() {
    let file = expand(
        parse_quote! {
            pub fn checked(ctx: KernelContext<'_>, mut output: DisjointSlice<u32>) -> KernelResult {
                let _ = (ctx, &mut output);
                Ok(())
            }
        },
        true,
    );
    let fs = functions(&file);
    assert_eq!(fs.len(), 2);
    let helper = fs
        .iter()
        .find(|f| f.sig.ident.to_string().starts_with("__fe2o3_kernel_body"))
        .unwrap();
    let root = fs.iter().find(|f| f.sig.ident != helper.sig.ident).unwrap();
    assert!(matches!(root.sig.output, ReturnType::Default));
    assert!(
        helper
            .sig
            .output
            .to_token_stream()
            .to_string()
            .contains("KernelResult")
    );
    assert!(
        root.block
            .to_token_stream()
            .to_string()
            .contains("let _ : gpu_device :: KernelResult")
    );
}

#[test]
fn source_context_syntax_and_forwarding_patterns_are_explicit() {
    for input in [
        "pub fn f(x: u32, ctx: KernelContext<'_>) {}",
        "pub fn f(ctx: KernelContext<'_>, other: KernelContext<'_>) {}",
        "pub fn f(ctx: KernelContext<'static>) {}",
        "pub fn f(ctx: KernelContext<'_, UserBrand>) {}",
        "pub fn f(ctx: &KernelContext<'_>) {}",
        "pub fn f(ctx: (KernelContext<'_>)) {}",
        "pub fn f(ctx: KernelContext<'_>, ref x: u32) {}",
        "pub fn f(ctx: KernelContext<'_>, x @ _: u32) {}",
        "pub fn f(ctx: KernelContext<'_>, #[allow(unused)] x: u32) {}",
        "#[track_caller] pub fn f(ctx: KernelContext<'_>) {}",
        "#[cfg(any())] pub fn f(ctx: KernelContext<'_>) {}",
        "pub extern \"C\" fn f(ctx: KernelContext<'_>) {}",
    ] {
        let mut input: ItemFn = syn::parse_str(input).unwrap();
        assert!(
            ContextEntryV1::take(&mut input, &parse_kernel_options(quote!()).unwrap()).is_err(),
            "{}",
            input.to_token_stream()
        );
    }
}

#[test]
fn context_aware_typed_profile_does_not_apply_legacy_vecadd_geometry() {
    let input = parse_quote! {
        pub fn f(ctx: KernelContext<'_>, a: &[f32], b: &[f32], output: DisjointSlice<f32>) {}
    };
    let options = parse_kernel_options(quote!(typed, launch(required = [64, 1, 1]))).unwrap();
    validate_typed_profile_v1(&input, &options).unwrap();
}

#[test]
fn opaque_context_alias_is_not_silently_rewritten() {
    let mut input: ItemFn = parse_quote! { pub fn f(ctx: Context, x: u32) {} };
    let original = input.to_token_stream().to_string();
    assert!(
        ContextEntryV1::take(&mut input, &parse_kernel_options(quote!()).unwrap())
            .unwrap()
            .is_none()
    );
    assert_eq!(input.to_token_stream().to_string(), original);
}

#[test]
fn user_names_and_lint_expectations_stay_with_the_logical_body() {
    let file = expand(
        parse_quote! {
            #[expect(unused_variables)]
            pub fn collision(ctx: KernelContext<'_>, __fe2o3_kernel_body_collision: u32) {
                let _ = ctx;
            }
        },
        false,
    );
    let fs = functions(&file);
    let root = fs
        .iter()
        .find(|f| f.sig.ident == "fe2o3_kernel_collision")
        .unwrap();
    let helper = fs
        .iter()
        .find(|f| f.sig.ident == "__fe2o3_kernel_body_collision")
        .unwrap();
    assert!(
        !root
            .sig
            .inputs
            .to_token_stream()
            .to_string()
            .contains("__fe2o3_kernel_body_collision")
    );
    assert!(
        helper
            .sig
            .inputs
            .to_token_stream()
            .to_string()
            .contains("__fe2o3_kernel_body_collision")
    );
    assert!(
        helper
            .attrs
            .iter()
            .any(|attr| attr.path().is_ident("expect"))
    );
    assert!(!root.attrs.iter().any(|attr| attr.path().is_ident("expect")));
}

#[test]
fn ordinary_expansion_has_no_context_registration_or_issuer() {
    for typed in [false, true] {
        let output = expand(
            parse_quote! { pub fn ordinary(x: u32) { let _ = x; } },
            typed,
        );
        let text = output.to_token_stream().to_string();
        assert!(!text.contains("__compiler_issue"));
        assert!(!text.contains("__fe2o3_kernel_context_contract"));
        assert_eq!(functions(&output).len(), 1);
    }
}
