use super::*;

fn valid_signature() -> SignatureContract {
    SignatureContract {
        safe: true,
        rust_abi: true,
        variadic: false,
        inputs: 2,
        shared_slice: true,
        usize_index: true,
        core_option: true,
        shared_element: true,
    }
}

#[test]
fn shared_usize_get_signature_requires_every_boundary() {
    let valid = valid_signature();
    assert!(authenticate_signature(valid));
    for invalid in [
        SignatureContract {
            safe: false,
            ..valid
        },
        SignatureContract {
            rust_abi: false,
            ..valid
        },
        SignatureContract {
            variadic: true,
            ..valid
        },
        SignatureContract { inputs: 1, ..valid },
        SignatureContract { inputs: 3, ..valid },
        SignatureContract {
            shared_slice: false,
            ..valid
        },
        SignatureContract {
            usize_index: false,
            ..valid
        },
        SignatureContract {
            core_option: false,
            ..valid
        },
        SignatureContract {
            shared_element: false,
            ..valid
        },
    ] {
        assert!(!authenticate_signature(invalid), "{invalid:?}");
    }
}
