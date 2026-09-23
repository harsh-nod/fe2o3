use fe2o3_device::amdgpu_ordered_program;
use fe2o3_device::ordered_program::__checked_ordered_program_words_v1 as pack;

#[path = "ordered_program/repeat_v1.rs"]
mod repeat_v1;

#[path = "ordered_program/select_v1.rs"]
mod select_v1;

#[test]
fn marker_keeps_exact_eight_runtime_parameters_with_five_typed_consts() {
    let _: fn(u32, u32, u32, u8, u8, u8, u8, u8) -> u32 =
        fe2o3_device::diagnostics::__amdgpu_ordered_program_e32_v1::<1, 8, 0, 0, 0>;
}

#[test]
fn macro_evaluates_data_once_in_order_and_never_silently_runs_on_host() {
    let order = std::cell::RefCell::new(Vec::new());
    let observed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        amdgpu_ordered_program! {
            gfx942_xnack_off_wave64;
            scratch(32); out(33);
            in(34) = { order.borrow_mut().push(0); u32::MAX };
            in(35) = { order.borrow_mut().push(1); 7 };
            in(36) = { order.borrow_mut().push(2); 11 };
            xor(scratch, input0, input1);
            and(scratch, scratch, input2);
            xor(out, input1, scratch);
        }
    }));
    assert!(observed.is_err());
    assert_eq!(*order.borrow(), [0, 1, 2]);
}

#[test]
fn packing_is_exact_for_one_three_and_sixteen_authored_steps() {
    assert_eq!(pack(&[8]), [8, 0, 0, 0]);
    assert_eq!(pack(&[133, 307, 413]), [1_773_841_612_933, 0, 0, 0]);
    let words = [
        0, 141, 323, 188, 321, 58, 64, 181, 60, 331, 73, 194, 56, 333, 16, 72,
    ];
    let packed = pack(&words);
    for index in 0..16 {
        assert_eq!(
            (packed[index / 4] >> ((index % 4) * 16)) as u16,
            words[index]
        );
    }
    assert_eq!(words[14..], [16, 72]); // dead scratch write and self move retained
}

#[test]
fn const_packer_refuses_bad_count_bits_opcodes_roles_and_initialization() {
    for words in [
        vec![],
        vec![8; 17],
        vec![14],
        vec![15],
        vec![1032],
        vec![88],
        vec![120],
        vec![136],
        vec![56],
        vec![72],
        vec![0],
        vec![0, 200],
    ] {
        assert!(
            std::panic::catch_unwind(|| pack(&words)).is_err(),
            "{words:?}"
        );
    }
}

#[test]
fn reused_output_and_scratch_are_checked_before_each_write() {
    assert_eq!(pack(&[8, 72]), [8 | (72 << 16), 0, 0, 0]);
    assert_eq!(pack(&[0, 56]), [56 << 16, 0, 0, 0]);
    assert!(std::panic::catch_unwind(|| pack(&[56, 0])).is_err());
}
