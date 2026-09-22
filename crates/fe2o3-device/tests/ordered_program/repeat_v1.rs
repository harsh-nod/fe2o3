use fe2o3_device::amdgpu_ordered_program;
use fe2o3_device::ordered_program::__checked_ordered_program_words_v1 as flat;
use fe2o3_device::ordered_program::repeat_v1::__checked_ordered_repeat_v1 as expand;

const MOV_OUT_INPUT0: u16 = 8;
const ADD_OUT_OUT_INPUT1: u16 = 201;
const MOV_SCRATCH_INPUT0: u16 = 0;
const ADD_SCRATCH_SCRATCH_INPUT1: u16 = 177;
const XOR_OUT_SCRATCH_INPUT2: u16 = 317;

#[test]
fn one_two_fifteen_repetitions_preserve_every_flat_descriptor() {
    for repetitions in [1, 2, 15] {
        let (count, packed) = expand(&[MOV_OUT_INPUT0], &[ADD_OUT_OUT_INPUT1], repetitions);
        assert_eq!(usize::from(count), repetitions + 1);
        let mut expected = vec![ADD_OUT_OUT_INPUT1; repetitions + 1];
        expected[0] = MOV_OUT_INPUT0;
        assert_eq!(packed, flat(&expected));
        for index in 0..usize::from(count) {
            assert_eq!(
                ((packed[index / 4] >> (16 * (index % 4))) & 0xffff) as u16,
                expected[index]
            );
        }
        for index in usize::from(count)..16 {
            assert_eq!((packed[index / 4] >> (16 * (index % 4))) & 0xffff, 0);
        }
    }
}

#[test]
fn expansion_is_a_const_expression_and_terminal_signature_is_unchanged() {
    const EXPANDED: (u8, [u64; 4]) = expand(&[MOV_OUT_INPUT0], &[ADD_OUT_OUT_INPUT1], 2);
    assert_eq!(EXPANDED, (3, [8 | (201 << 16) | (201 << 32), 0, 0, 0]));
    let _: fn(u32, u32, u32, u8, u8, u8, u8, u8) -> u32 =
        fe2o3_device::diagnostics::__amdgpu_ordered_program_e32_v1::<
            { EXPANDED.0 },
            { EXPANDED.1[0] },
            { EXPANDED.1[1] },
            { EXPANDED.1[2] },
            { EXPANDED.1[3] },
        >;
}

#[test]
fn initial_state_flows_through_all_repetitions_without_per_copy_reset() {
    let (count, packed) = expand(
        &[MOV_SCRATCH_INPUT0],
        &[ADD_SCRATCH_SCRATCH_INPUT1, XOR_OUT_SCRATCH_INPUT2],
        2,
    );
    assert_eq!(count, 5);
    assert_eq!(
        packed,
        flat(&[
            MOV_SCRATCH_INPUT0,
            ADD_SCRATCH_SCRATCH_INPUT1,
            XOR_OUT_SCRATCH_INPUT2,
            ADD_SCRATCH_SCRATCH_INPUT1,
            XOR_OUT_SCRATCH_INPUT2,
        ])
    );
    assert!(
        std::panic::catch_unwind(|| { expand(&[MOV_SCRATCH_INPUT0], &[ADD_OUT_OUT_INPUT1], 1) })
            .is_err()
    );
}

#[test]
fn release_active_limits_refuse_zero_sixteen_huge_empty_and_expansion_overflow() {
    for repetitions in [0, 16, 17, usize::MAX] {
        assert!(
            std::panic::catch_unwind(|| {
                expand(&[MOV_OUT_INPUT0], &[ADD_OUT_OUT_INPUT1], repetitions)
            })
            .is_err()
        );
    }
    // Both individual arrays satisfy their local length bound, but total17
    // rejects before expansion. This is not a synthetic usize-overflow branch.
    assert!(
        std::panic::catch_unwind(|| {
            expand(
                &[MOV_OUT_INPUT0],
                &[ADD_OUT_OUT_INPUT1, ADD_OUT_OUT_INPUT1],
                8,
            )
        })
        .is_err()
    );
    for (initial, repeated) in [
        (vec![], vec![ADD_OUT_OUT_INPUT1]),
        (vec![MOV_OUT_INPUT0], vec![]),
        (vec![MOV_OUT_INPUT0; 17], vec![ADD_OUT_OUT_INPUT1]),
        (vec![MOV_OUT_INPUT0], vec![ADD_OUT_OUT_INPUT1; 17]),
    ] {
        assert!(std::panic::catch_unwind(|| expand(&initial, &repeated, 1)).is_err());
    }
}

#[test]
fn existing_opcode_role_padding_and_exit_checks_are_reused_not_reimplemented() {
    for word in [14_u16, 15, 1032, 88, 120, 136] {
        assert!(std::panic::catch_unwind(|| expand(&[MOV_OUT_INPUT0], &[word], 1)).is_err());
    }
    assert!(
        std::panic::catch_unwind(|| expand(&[MOV_SCRATCH_INPUT0], &[MOV_SCRATCH_INPUT0], 1))
            .is_err()
    );
    assert!(
        std::panic::catch_unwind(|| expand(&[ADD_OUT_OUT_INPUT1], &[MOV_OUT_INPUT0], 1)).is_err()
    );
}

#[test]
fn macro_data_operands_run_once_in_order_and_marker_still_refuses_host_execution() {
    let order = std::cell::RefCell::new(Vec::new());
    let observed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        amdgpu_ordered_program! {
            gfx942_xnack_off_wave64;
            scratch(32); out(33);
            in(34) = { order.borrow_mut().push(0); u32::MAX };
            in(35) = { order.borrow_mut().push(1); 7 };
            in(36) = { order.borrow_mut().push(2); 11 };
            init { mov(out, input0); }
            repeat(15) { add(out, out, input1); }
        }
    }));
    assert!(observed.is_err(), "no silent host fallback");
    assert_eq!(*order.borrow(), [0, 1, 2]);
}

#[test]
fn old_flat_spelling_keeps_its_exact_packed_source_contract() {
    assert_eq!(flat(&[8]), [8, 0, 0, 0]);
    assert_eq!(flat(&[133, 307, 413]), [1_773_841_612_933, 0, 0, 0]);
    let old = std::panic::catch_unwind(|| {
        amdgpu_ordered_program! {
            gfx942_xnack_off_wave64;
            scratch(32); out(33); in(34) = 1; in(35) = 2; in(36) = 3;
            xor(scratch, input0, input1);
            and(scratch, scratch, input2);
            xor(out, input1, scratch);
        }
    });
    assert!(old.is_err());
}
