#[test]
fn reviewed_safe_core_wrapping_shifts_require_the_actual_u32_count_signature() {
    for integer in [
        "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize",
    ] {
        for method in ["wrapping_shl", "wrapping_shr"] {
            let path = format!("core::num::<impl {integer}>::{method}");
            let contract = super::ReviewedSafeCoreWrappingIntegerContractV1 {
                first_integer: Some(integer),
                second_integer: Some("u32"),
                result_integer: Some(integer),
                ..reviewed_wrapping_integer_contract(&path)
            };
            assert!(super::authenticate_reviewed_safe_core_wrapping_integer_contract_v1(contract));
            for count in [
                None,
                Some("i32"),
                Some("u8"),
                Some("u64"),
                Some("usize"),
                Some("bool"),
            ] {
                assert!(
                    !super::authenticate_reviewed_safe_core_wrapping_integer_contract_v1(
                        super::ReviewedSafeCoreWrappingIntegerContractV1 {
                            second_integer: count,
                            ..contract
                        }
                    )
                );
            }
        }
    }
}

#[test]
fn safe_wrapping_shift_names_do_not_replace_resolved_core_and_signature_authority() {
    use super::ReviewedSafeCoreWrappingIntegerContractV1 as Contract;
    for method in ["wrapping_shl", "wrapping_shr"] {
        let path = format!("core::num::<impl u8>::{method}");
        let reviewed = Contract {
            first_integer: Some("u8"),
            second_integer: Some("u32"),
            result_integer: Some("u8"),
            ..reviewed_wrapping_integer_contract(&path)
        };
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
                first_integer: Some("u32"),
                ..reviewed
            },
            Contract {
                result_integer: None,
                ..reviewed
            },
            Contract {
                result_integer: Some("u32"),
                ..reviewed
            },
        ] {
            assert!(
                !super::authenticate_reviewed_safe_core_wrapping_integer_contract_v1(hostile),
                "{hostile:?}"
            );
        }
    }
}

#[test]
fn unchecked_and_lookalike_shift_helpers_do_not_receive_source_safety_admission() {
    for path in [
        "",
        "wrapping_shl",
        "core::num::<impl u32>::wrapping_shl_extra",
        "core::num::<impl u32>::wrapping_shr::helper",
        "fake_core::num::<impl u32>::wrapping_shl",
        "lookalike::core::num::<impl u32>::wrapping_shr",
        "core::other::<impl u32>::wrapping_shl",
        "core::num::<impl u64>::wrapping_shr",
        "core::num::<impl u32>::checked_shl",
        "core::num::<impl u32>::unchecked_shl",
        "core::num::<impl u32>::unchecked_shr",
        "core::num::<impl u32>::unchecked_shl_exact",
        "core::num::<impl u32>::unbounded_shr",
        "core::num::<impl u32>::overflowing_shl",
        "core::num::<impl u32>::rotate_left",
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
        let path = format!("core::num::<impl {noninteger}>::wrapping_shl");
        assert!(
            !super::authenticate_reviewed_safe_core_wrapping_integer_contract_v1(
                super::ReviewedSafeCoreWrappingIntegerContractV1 {
                    first_integer: Some(noninteger),
                    second_integer: Some("u32"),
                    result_integer: Some(noninteger),
                    ..reviewed_wrapping_integer_contract(&path)
                }
            )
        );
    }
}

#[test]
fn wrapping_arithmetic_keeps_its_homogeneous_argument_contract() {
    for integer in [
        "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u64", "u128", "usize",
    ] {
        for method in ["wrapping_add", "wrapping_sub", "wrapping_mul"] {
            let path = format!("core::num::<impl {integer}>::{method}");
            let heterogeneous = super::ReviewedSafeCoreWrappingIntegerContractV1 {
                first_integer: Some(integer),
                second_integer: Some("u32"),
                result_integer: Some(integer),
                ..reviewed_wrapping_integer_contract(&path)
            };
            assert!(
                !super::authenticate_reviewed_safe_core_wrapping_integer_contract_v1(heterogeneous)
            );
            assert!(
                super::authenticate_reviewed_safe_core_wrapping_integer_contract_v1(
                    super::ReviewedSafeCoreWrappingIntegerContractV1 {
                        second_integer: Some(integer),
                        ..heterogeneous
                    }
                )
            );
        }
    }
}
