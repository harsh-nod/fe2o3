use super::*;

fn reviewed(method: &str, width: u8) -> ContractV1<'_> {
    ContractV1 {
        item: true,
        core: true,
        mir: true,
        inherent_method: true,
        generic_count: 0,
        method,
        safe: true,
        rust_abi: true,
        variadic: false,
        exact_self_and_result: true,
        u32_count: true,
        argument_count: 2,
        width: Some(width),
    }
}

#[test]
fn fixed_width_wrapping_methods_have_total_full_u32_masks() {
    for width in [8, 16, 32, 64] {
        for (method, direction) in [
            ("wrapping_shl", DirectionV1::Left),
            ("wrapping_shr", DirectionV1::Right),
        ] {
            assert_eq!(
                contract_v1(reviewed(method, width)),
                Some((direction, width))
            );
            for count in [
                0,
                1,
                7,
                8,
                15,
                16,
                31,
                32,
                63,
                64,
                255,
                256,
                0x8000_0000,
                0x8000_0021,
                u32::MAX,
            ] {
                let masked = count & u32::from(width - 1);
                assert!(masked < u32::from(width));
                assert_eq!(masked, count % u32::from(width));
            }
        }
    }
}

#[test]
fn each_provider_signature_and_method_boundary_is_required() {
    for method in ["wrapping_shl", "wrapping_shr"] {
        let good = reviewed(method, 32);
        for bad in [
            ContractV1 {
                item: false,
                ..good
            },
            ContractV1 {
                core: false,
                ..good
            },
            ContractV1 { mir: false, ..good },
            ContractV1 {
                inherent_method: false,
                ..good
            },
            ContractV1 {
                generic_count: 1,
                ..good
            },
            ContractV1 {
                safe: false,
                ..good
            },
            ContractV1 {
                rust_abi: false,
                ..good
            },
            ContractV1 {
                variadic: true,
                ..good
            },
            ContractV1 {
                exact_self_and_result: false,
                ..good
            },
            ContractV1 {
                u32_count: false,
                ..good
            },
            ContractV1 {
                argument_count: 1,
                ..good
            },
            ContractV1 {
                argument_count: 3,
                ..good
            },
            ContractV1 {
                width: None,
                ..good
            },
            ContractV1 {
                width: Some(128),
                ..good
            },
        ] {
            assert_eq!(contract_v1(bad), None, "{bad:?}");
        }
    }
}

#[test]
fn unchecked_panic_overflow_and_lookalike_names_are_not_summaries() {
    for method in [
        "",
        "unchecked_shl",
        "unchecked_shr",
        "unchecked_shl_exact",
        "checked_shr",
        "overflowing_shl",
        "unbounded_shr",
        "precondition_check",
        "panic",
        "wrapping_shl_extra",
        "wrapping_shr::helper",
        "rotate_left",
        "wrapping_add",
    ] {
        assert_eq!(contract_v1(reviewed(method, 64)), None, "{method}");
    }
}

#[test]
fn existing_intrinsic_tags_and_cardinality_stay_unchanged() {
    use fe2o3_mir_model::semantic_mir_v1::SemanticSaturatingIntegerOpV1;
    for (operation, tag) in [
        (ProductionRustcIntrinsicOperationV1::FabsF32, 1),
        (
            ProductionRustcIntrinsicOperationV1::SaturatingInteger(
                SemanticSaturatingIntegerOpV1::Add,
            ),
            2,
        ),
        (
            ProductionRustcIntrinsicOperationV1::SaturatingInteger(
                SemanticSaturatingIntegerOpV1::Subtract,
            ),
            3,
        ),
    ] {
        let call = NormalizedCallV1::Rustc(operation);
        assert_eq!(call.operation_tag(), tag);
        assert_eq!(call.statement_count(), 1);
    }
}
