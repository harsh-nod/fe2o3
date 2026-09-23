use fe2o3_device::amdgpu_ordered_program;
use fe2o3_device::ordered_program::__checked_ordered_program_words_v1 as flat;
use fe2o3_device::ordered_program::repeat_v1::__checked_ordered_repeat_v1 as repeat;
use fe2o3_device::ordered_program::select_v1::__checked_ordered_select_v1 as select;

const MOV_OUT_INPUT0: u16 = 8;
const ADD_OUT_OUT_INPUT1: u16 = 201;
const MOV_SCRATCH_INPUT0: u16 = 0;
const MOV_OUT_SCRATCH: u16 = 56;

#[test]
fn concrete_bool_constants_select_exact_count_and_packed_words() {
    const PICK_FIRST: bool = 3_u32 < 4;
    const FIRST: (u8, [u64; 4]) = select(PICK_FIRST, &[8, 201], &[8, 201, 201]);
    const SECOND: (u8, [u64; 4]) = select(4_u32 < 3, &[8, 201], &[8, 201, 201]);
    assert_eq!(FIRST, (2, [8 | (201 << 16), 0, 0, 0]));
    assert_eq!(SECOND, (3, [8 | (201 << 16) | (201 << 32), 0, 0, 0]));
    assert_eq!(FIRST, select(true, &[8, 201], &[8, 201, 201]));
    assert_eq!(SECOND, select(false, &[8, 201], &[8, 201, 201]));
}

#[test]
fn one_and_sixteen_step_arms_preserve_all_descriptors_and_zero_padding() {
    let one = [MOV_OUT_INPUT0];
    let sixteen = [
        0, 141, 323, 188, 321, 58, 64, 181, 60, 331, 73, 194, 56, 333, 16, 72,
    ];
    for (condition, expected) in [(true, one.as_slice()), (false, sixteen.as_slice())] {
        let (count, packed) = select(condition, &one, &sixteen);
        assert_eq!(usize::from(count), expected.len());
        assert_eq!(packed, flat(expected));
        for (index, word) in expected.iter().enumerate() {
            assert_eq!((packed[index / 4] >> (16 * (index % 4))) as u16, *word);
        }
        for index in expected.len()..16 {
            assert_eq!((packed[index / 4] >> (16 * (index % 4))) as u16, 0);
        }
    }
}

#[test]
fn selected_metadata_keeps_the_existing_five_const_eight_argument_terminal() {
    const FIRST: (u8, [u64; 4]) = select(true, &[8, 201], &[8, 201, 201]);
    const SECOND: (u8, [u64; 4]) = select(false, &[8, 201], &[8, 201, 201]);
    let _: fn(u32, u32, u32, u8, u8, u8, u8, u8) -> u32 =
        fe2o3_device::diagnostics::__amdgpu_ordered_program_e32_v1::<
            { FIRST.0 },
            { FIRST.1[0] },
            { FIRST.1[1] },
            { FIRST.1[2] },
            { FIRST.1[3] },
        >;
    let _: fn(u32, u32, u32, u8, u8, u8, u8, u8) -> u32 =
        fe2o3_device::diagnostics::__amdgpu_ordered_program_e32_v1::<
            { SECOND.0 },
            { SECOND.1[0] },
            { SECOND.1[1] },
            { SECOND.1[2] },
            { SECOND.1[3] },
        >;
}

#[test]
fn both_arms_are_checked_even_when_invalid_metadata_is_not_selected() {
    for bad in [
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
        for condition in [true, false] {
            assert!(
                std::panic::catch_unwind(|| select(condition, &bad, &[8])).is_err(),
                "first arm must be checked: {condition:?} {bad:?}"
            );
            assert!(
                std::panic::catch_unwind(|| select(condition, &[8], &bad)).is_err(),
                "second arm must be checked: {condition:?} {bad:?}"
            );
        }
    }
}

#[test]
fn initializedness_belongs_to_each_arm_and_never_leaks_across_alternatives() {
    let first = [MOV_SCRATCH_INPUT0, MOV_OUT_SCRATCH];
    let second = [MOV_OUT_INPUT0, ADD_OUT_OUT_INPUT1];
    assert_eq!(select(true, &first, &second), (2, flat(&first)));
    assert_eq!(select(false, &first, &second), (2, flat(&second)));
    for condition in [true, false] {
        assert!(
            std::panic::catch_unwind(|| select(condition, &first, &[MOV_OUT_SCRATCH])).is_err()
        );
        assert!(
            std::panic::catch_unwind(|| select(condition, &[MOV_OUT_SCRATCH], &first)).is_err()
        );
    }
}

#[test]
fn macro_operands_evaluate_once_in_order_for_literal_and_named_const_selection() {
    const SELECT_SECOND: bool = 4_u32 < 3;
    let order = std::cell::RefCell::new(Vec::new());
    let first = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        amdgpu_ordered_program! {
            gfx942_xnack_off_wave64;
            scratch(32); out(33);
            in(34) = { order.borrow_mut().push(0); u32::MAX };
            in(35) = { order.borrow_mut().push(1); 7 };
            in(36) = { order.borrow_mut().push(2); 11 };
            const_if(true) {
                mov(out, input0);
                add(out, out, input1);
            } else {
                mov(out, input0);
                add(out, out, input1);
                add(out, out, input1);
            }
        }
    }));
    assert!(
        first.is_err(),
        "the source marker must not silently execute on host"
    );
    assert_eq!(*order.borrow(), [0, 1, 2]);
    order.borrow_mut().clear();
    let second = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        amdgpu_ordered_program! {
            gfx942_xnack_off_wave64;
            scratch(32); out(33);
            in(34) = { order.borrow_mut().push(0); u32::MAX };
            in(35) = { order.borrow_mut().push(1); 7 };
            in(36) = { order.borrow_mut().push(2); 11 };
            const_if(SELECT_SECOND) {
                mov(out, input0);
                add(out, out, input1);
            } else {
                mov(out, input0);
                add(out, out, input1);
                add(out, out, input1);
            }
        }
    }));
    assert!(
        second.is_err(),
        "the source marker must not silently execute on host"
    );
    assert_eq!(*order.borrow(), [0, 1, 2]);
}

#[test]
fn existing_flat_and_repeat_descriptors_are_unchanged() {
    assert_eq!(flat(&[133, 307, 413]), [1_773_841_612_933, 0, 0, 0]);
    for repetitions in [1, 2, 15] {
        let mut words = vec![MOV_OUT_INPUT0];
        words.extend(std::iter::repeat_n(ADD_OUT_OUT_INPUT1, repetitions));
        let expected = (words.len() as u8, flat(&words));
        assert_eq!(
            repeat(&[MOV_OUT_INPUT0], &[ADD_OUT_OUT_INPUT1], repetitions),
            expected
        );
        assert_eq!(select(true, &words, &[8]), expected);
        assert_eq!(select(false, &[8], &words), expected);
    }
}

#[test]
fn caller_bool_constants_do_not_collide_with_any_removed_macro_item_name() {
    const __CONDITION: bool = true;
    const __IF_TRUE: bool = false;
    const __IF_FALSE: bool = true;
    const __SELECTED: bool = false;
    let calls: [fn() -> u32; 4] = [
        || {
            amdgpu_ordered_program! {
                gfx942_xnack_off_wave64;
                scratch(32); out(33);
                in(34) = 19_u32;
                in(35) = 23_u32;
                in(36) = 42_u32;
                const_if(__CONDITION) {
                    mov(out, input0);
                    add(out, out, input1);
                } else {
                    mov(out, input0);
                    add(out, out, input1);
                    add(out, out, input1);
                }
            }
        },
        || {
            amdgpu_ordered_program! {
                gfx942_xnack_off_wave64;
                scratch(32); out(33);
                in(34) = 19_u32;
                in(35) = 23_u32;
                in(36) = 42_u32;
                const_if(__IF_TRUE) {
                    mov(out, input0);
                    add(out, out, input1);
                } else {
                    mov(out, input0);
                    add(out, out, input1);
                    add(out, out, input1);
                }
            }
        },
        || {
            amdgpu_ordered_program! {
                gfx942_xnack_off_wave64;
                scratch(32); out(33);
                in(34) = 19_u32;
                in(35) = 23_u32;
                in(36) = 42_u32;
                const_if(__IF_FALSE) {
                    mov(out, input0);
                    add(out, out, input1);
                } else {
                    mov(out, input0);
                    add(out, out, input1);
                    add(out, out, input1);
                }
            }
        },
        || {
            amdgpu_ordered_program! {
                gfx942_xnack_off_wave64;
                scratch(32); out(33);
                in(34) = 19_u32;
                in(35) = 23_u32;
                in(36) = 42_u32;
                const_if(__SELECTED) {
                    mov(out, input0);
                    add(out, out, input1);
                } else {
                    mov(out, input0);
                    add(out, out, input1);
                    add(out, out, input1);
                }
            }
        },
    ];
    for call in calls {
        assert!(
            std::panic::catch_unwind(call).is_err(),
            "each caller-named predicate must compile and reach the host marker"
        );
    }
}

#[test]
#[expect(
    non_snake_case,
    reason = "exercise exact names from the removed macro expansion"
)]
fn caller_runtime_operands_keep_all_four_removed_macro_item_names() {
    let __CONDITION = 19_u32;
    let __IF_TRUE = 23_u32;
    let __IF_FALSE = 42_u32;
    let __SELECTED = 71_u32;
    let order = std::cell::RefCell::new(Vec::new());
    let first = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        amdgpu_ordered_program! {
            gfx942_xnack_off_wave64;
            scratch(32); out(33);
            in(34) = { order.borrow_mut().push(__CONDITION); __CONDITION };
            in(35) = { order.borrow_mut().push(__IF_TRUE); __IF_TRUE };
            in(36) = { order.borrow_mut().push(__IF_FALSE); __IF_FALSE };
            const_if(true) {
                mov(out, input0);
                add(out, out, input1);
            } else {
                mov(out, input0);
                add(out, out, input1);
                add(out, out, input1);
            }
        }
    }));
    assert!(first.is_err(), "no silent host marker fallback");
    assert_eq!(*order.borrow(), [19, 23, 42]);
    order.borrow_mut().clear();
    let second = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        amdgpu_ordered_program! {
            gfx942_xnack_off_wave64;
            scratch(32); out(33);
            in(34) = { order.borrow_mut().push(__SELECTED); __SELECTED };
            in(35) = { order.borrow_mut().push(__IF_FALSE); __IF_FALSE };
            in(36) = { order.borrow_mut().push(__CONDITION); __CONDITION };
            const_if(false) {
                mov(out, input0);
                add(out, out, input1);
            } else {
                mov(out, input0);
                add(out, out, input1);
                add(out, out, input1);
            }
        }
    }));
    assert!(second.is_err(), "no silent host marker fallback");
    assert_eq!(*order.borrow(), [71, 42, 19]);
}
