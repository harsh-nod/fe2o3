use fe2o3_device::amdgpu_ordered_region;

#[test]
fn marker_has_exact_non_generic_eight_argument_u32_signature() {
    let _: fn(u32, u32, u32, u8, u8, u8, u8, u8) -> u32 =
        fe2o3_device::diagnostics::__amdgpu_ordered_xor_add_e32_v1;
}

#[test]
fn macro_evaluates_each_data_expression_once_then_refuses_host_execution() {
    let order = std::cell::RefCell::new(Vec::new());
    let observed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        amdgpu_ordered_region! {
            gfx942_xnack_off_wave64;
            scratch(32);
            out(33);
            in(34) = { order.borrow_mut().push(0); u32::MAX };
            in(35) = { order.borrow_mut().push(1); 7 };
            in(36) = { order.borrow_mut().push(2); 11 };
            xor_add_u32_e32;
        }
    }));
    assert!(
        observed.is_err(),
        "the source marker has no silent CPU fallback"
    );
    assert_eq!(*order.borrow(), [0, 1, 2]);
}
