use super::*;
use crate::control_flow_v1::{
    analyze_kernel_control_flow_v1, lower_bounded_for_loops_v1, parse_control_flow_options_v1,
};
use quote::{ToTokens, quote};
use syn::{ItemFn, parse_quote};

fn unchanged(input: ItemFn) {
    let before = input.to_token_stream().to_string();
    assert_eq!(analyze_kernel_control_flow_v1(&input, None).unwrap(), None);
    let mut after = input;
    lower_bounded_for_loops_v1(&mut after, None).unwrap();
    assert_eq!(after.to_token_stream().to_string(), before);
}

#[test]
fn direct_constructor_matches_preserve_original_borrows_fields_and_assertions() {
    for mutable in [false, true] {
        let body = if mutable {
            quote! {
                let mut local = a ^ b;
                let option: Option<&mut u32> = if a & 1 == 0 { Some(&mut local) } else { None };
                let result = match option {
                    Some(value) => { *value = *value ^ b; *value },
                    None => a,
                };
                assert!(result == a);
            }
        } else {
            quote! {
                let local = a ^ b;
                let option: Option<&u32> = if a & 1 == 0 { Some(&local) } else { None };
                let result = match option {
                    Some(value) => *value ^ b,
                    None => a,
                };
                assert!(result == a);
            }
        };
        unchanged(parse_quote!(fn entry(a: u32, b: u32) { #body }));
    }
}

#[test]
fn registered_basic_and_typed_kernels_keep_the_original_match_and_assert_body() {
    for typed in [false, true] {
        let input: ItemFn = parse_quote! {
            pub fn entry(a: u32, b: u32) {
                let local = a ^ b;
                let option = if a & 1 == 0 { Some(&local) } else { None };
                let result = match option { Some(value) => *value ^ b, None => a };
                assert!(result == a);
            }
        };
        let expected = input.block.to_token_stream().to_string();
        let attributes = if typed { quote!(typed) } else { quote!() };
        let options = crate::parse_kernel_options(attributes).unwrap();
        let device = quote!(::fe2o3_device);
        let expansion = crate::expand_kernel_with_imports(
            input,
            options,
            &quote!(),
            Some(&device),
            None,
            typed.then_some(crate::CrateBindingIdV1::from_bytes([9; 32])),
        )
        .unwrap();
        let generated: syn::File = syn::parse2(expansion.clone()).unwrap();
        let bodies = generated.items.iter().filter_map(|item| match item {
            syn::Item::Fn(function) => Some(function.block.to_token_stream().to_string()),
            _ => None,
        });
        assert_eq!(bodies.filter(|body| body == &expected).count(), 1);
        let text = expansion.to_string();
        assert!(text.contains("__fe2o3_kernel_registration_entry"));
        assert!(!text.contains(crate::control_flow_v1::CONTROL_FLOW_REGISTRATION_PREFIX_V1));
    }
}

#[test]
fn constructor_paths_aliases_and_wildcards_are_not_integer_encoding_claims() {
    for input in [
        parse_quote! {
            fn entry(value: Result<(u32, u32), u32>) {
                match value { Ok((first, _)) => consume(first), Err(_) => {} }
            }
        },
        parse_quote! {
            fn entry(value: Choice<u32>) {
                match value {
                    Choice::Named { value: ref field, .. } => consume(field),
                    Choice::Pair(mut left, _, ..) => consume(left),
                    Choice::Empty | Choice::Other => {},
                }
            }
        },
        parse_quote! {
            fn entry(value: alias::Choice) {
                match value { alias::Choice::First => {}, _ => {} }
            }
        },
        parse_quote! {
            fn entry(value: Option<Option<u32>>) {
                match value { Some(None) => {}, _ => {} }
            }
        },
        parse_quote! {
            fn entry(value: Choice) {
                use Choice::{First as Selected, Second as Remaining};
                match value { Selected => {}, Remaining => {} }
            }
        },
        parse_quote! {
            fn entry(value: u32) {
                const Some: u32 = 17;
                const None: u32 = 93;
                match value { Some => {}, None => {}, _ => {} }
            }
        },
    ] {
        // Rustc resolves these names. Even Some/None constants do not create
        // nominal enum or pointer authority and no guessed sidecar is emitted.
        unchanged(input);
    }
}

#[test]
fn nominal_syntax_does_not_hide_unsupported_patterns_or_nested_control() {
    for input in [
        parse_quote!(
            fn entry(value: Option<u32>) {
                match value {
                    Some(3) => {}
                    _ => {}
                }
            }
        ),
        parse_quote!(
            fn entry(value: Option<u32>) {
                match value {
                    Some(x) if x > 0 => {}
                    _ => {}
                }
            }
        ),
        parse_quote!(
            fn entry(value: Option<Option<u32>>) {
                match value {
                    Some(Some(x)) => {}
                    _ => {}
                }
            }
        ),
        parse_quote!(
            fn entry(value: Option<u32>) {
                match value {
                    whole @ Some(_) => {}
                    _ => {}
                }
            }
        ),
        parse_quote!(
            fn entry(value: u32) {
                match value {
                    1 => {}
                    _ => {}
                }
            }
        ),
        parse_quote!(
            fn entry(value: u32) {
                match value {
                    1..=3 => {}
                    _ => {}
                }
            }
        ),
        parse_quote!(
            fn entry(value: &[u32]) {
                match value {
                    [head, ..] => {}
                    _ => {}
                }
            }
        ),
        parse_quote!(
            fn entry(value: Option<u32>) {
                match value {
                    Some(x) => loop {
                        consume(x);
                    },
                    None => {}
                }
            }
        ),
        parse_quote!(
            fn entry(value: Option<u32>) {
                match value {
                    Some(x) => match x {
                        0 => {}
                        _ => {}
                    },
                    None => {}
                }
            }
        ),
        parse_quote!(
            fn entry(value: Option<Option<u32>>) {
                match value {
                    Some(inner) => match inner {
                        Some(x) => loop {
                            consume(x);
                        },
                        None => {}
                    },
                    None => {}
                }
            }
        ),
    ] {
        assert!(
            analyze_kernel_control_flow_v1(&input, None)
                .unwrap_err()
                .to_string()
                .contains("requires control_flow")
        );
    }
}

#[test]
fn explicit_integer_declarations_keep_exact_existing_match_contract() {
    let declaration =
        parse_control_flow_options_v1(&parse_quote!(control_flow(integer_switches(u32)))).unwrap();
    let nominal: ItemFn = parse_quote! {
        fn entry(value: Option<u32>) { match value { Some(_) => {}, None => {} } }
    };
    assert!(
        analyze_kernel_control_flow_v1(&nominal, Some(&declaration))
            .unwrap_err()
            .to_string()
            .contains("not a fixed-width integer switch")
    );
    let integer: ItemFn = parse_quote! {
        fn entry(value: u32) { match value { 17 => {}, _ => {} } }
    };
    assert!(
        analyze_kernel_control_flow_v1(&integer, Some(&declaration))
            .unwrap()
            .is_some()
    );
}

#[test]
fn source_pattern_work_and_recursion_are_bounded_before_semantic_compilation() {
    let mut expression: ExprMatch = parse_quote!(match input {
        Name => (),
    });
    let arm = expression.arms[0].clone();
    expression.arms = vec![arm.clone(); crate::control_flow_v1::MAX_CASES_V1];
    assert!(accepts(&expression));
    expression.arms.push(arm);
    assert!(!accepts(&expression));
    let mut pattern: Pat = parse_quote!(Name);
    for _ in 0..MAX_PATTERN_DEPTH {
        pattern = parse_quote!((#pattern));
    }
    assert!(super::pattern(&pattern, true, 0, &mut usize::MAX));
    pattern = parse_quote!((#pattern));
    assert!(!super::pattern(&pattern, true, 0, &mut usize::MAX));
    let shape: Pat = parse_quote!(Pair(first, second));
    assert!(super::pattern(&shape, true, 0, &mut 3));
    assert!(!super::pattern(&shape, true, 0, &mut 2));
}
