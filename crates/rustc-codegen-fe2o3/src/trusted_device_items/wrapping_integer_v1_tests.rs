fn reviewed_wrapping_integer_contract(
    canonical_path: &str,
) -> super::ReviewedSafeCoreWrappingIntegerContractV1<'_> {
    super::ReviewedSafeCoreWrappingIntegerContractV1 {
        item_instance: true,
        core_identity: true,
        generic_arguments: 0,
        mir_available: true,
        canonical_path,
        safe_signature: true,
        rust_abi: true,
        variadic: false,
        input_count: 2,
        first_integer: Some("u32"),
        second_integer: Some("u32"),
        result_integer: Some("u32"),
    }
}

#[test]
fn safe_core_wrapping_integer_contract_admits_each_primitive_and_operation() {
    for integer in [
        "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize",
    ] {
        for method in ["wrapping_add", "wrapping_sub", "wrapping_mul"] {
            let path = format!("core::num::<impl {integer}>::{method}");
            let contract = super::ReviewedSafeCoreWrappingIntegerContractV1 {
                first_integer: Some(integer),
                second_integer: Some(integer),
                result_integer: Some(integer),
                ..reviewed_wrapping_integer_contract(&path)
            };
            assert!(
                super::authenticate_reviewed_safe_core_wrapping_integer_contract_v1(contract),
                "{path}"
            );
        }
    }
}

#[test]
fn safe_core_wrapping_integer_contract_rejects_incomplete_authority_and_signature() {
    use super::ReviewedSafeCoreWrappingIntegerContractV1 as Contract;
    let reviewed = reviewed_wrapping_integer_contract("core::num::<impl u32>::wrapping_add");
    for hostile in [
        Contract {
            item_instance: false,
            ..reviewed
        },
        Contract {
            core_identity: false,
            ..reviewed
        },
        Contract {
            generic_arguments: 1,
            ..reviewed
        },
        Contract {
            mir_available: false,
            ..reviewed
        },
        Contract {
            safe_signature: false,
            ..reviewed
        },
        Contract {
            rust_abi: false,
            ..reviewed
        },
        Contract {
            variadic: true,
            ..reviewed
        },
        Contract {
            input_count: 0,
            ..reviewed
        },
        Contract {
            input_count: 1,
            ..reviewed
        },
        Contract {
            input_count: 3,
            ..reviewed
        },
        Contract {
            first_integer: None,
            ..reviewed
        },
        Contract {
            second_integer: None,
            ..reviewed
        },
        Contract {
            result_integer: None,
            ..reviewed
        },
        Contract {
            first_integer: Some("i32"),
            ..reviewed
        },
        Contract {
            second_integer: Some("i32"),
            ..reviewed
        },
        Contract {
            result_integer: Some("i32"),
            ..reviewed
        },
    ] {
        assert!(
            !super::authenticate_reviewed_safe_core_wrapping_integer_contract_v1(hostile),
            "{hostile:?}"
        );
    }
}

#[test]
fn safe_core_wrapping_integer_contract_rejects_forged_paths_and_noninteger_types() {
    for path in [
        "",
        "wrapping_add",
        "core::num::<impl u32>::wrapping_add_extra",
        "core::num::<impl u32>::wrapping_add::helper",
        "lookalike::core::num::<impl u32>::wrapping_add",
        "fake_core::num::<impl u32>::wrapping_add",
        "core::other::<impl u32>::wrapping_add",
        "core::num::<impl i32>::wrapping_add",
        "core::num::<impl u64>::wrapping_add",
        "core::num::<impl u32>::checked_add",
        "core::num::<impl u32>::unchecked_add",
        "core::num::<impl u32>::wrapping_div",
        "core::num::<impl u32>::wrapping_add_signed",
    ] {
        assert!(
            !super::authenticate_reviewed_safe_core_wrapping_integer_contract_v1(
                reviewed_wrapping_integer_contract(path)
            ),
            "{path}"
        );
    }
    for noninteger in [
        "bool", "f32", "f64", "char", "u256", "Scalar", "&u32", "(u32,)",
    ] {
        let path = format!("core::num::<impl {noninteger}>::wrapping_add");
        let contract = super::ReviewedSafeCoreWrappingIntegerContractV1 {
            first_integer: Some(noninteger),
            second_integer: Some(noninteger),
            result_integer: Some(noninteger),
            ..reviewed_wrapping_integer_contract(&path)
        };
        assert!(
            !super::authenticate_reviewed_safe_core_wrapping_integer_contract_v1(contract),
            "{path}"
        );
    }
}
