// The production executor includes these exact expressions. The optional Verus harness proves
// conditional bitvector properties of them, not decoding, ABI, hardware, or kernel refinement.
// Verify this file with the pinned Verus and --cfg verus_keep_ghost. No assume/external_body.

macro_rules! gfx942_exec_transition_v1 {
    (add_u32, $left:expr, $right:expr, $carry:expr) => {{
        let carry = if $carry { 1u64 } else { 0u64 };
        let sum = ($left as u64) + ($right as u64) + carry;
        (sum as u32, sum >= 0x1_0000_0000u64)
    }};
    (sub_u32, $left:expr, $right:expr, $borrow:expr) => {{
        let borrow = if $borrow { 1u64 } else { 0u64 };
        // The bias avoids host underflow, including MAX + borrow on the right.
        let difference = 0x1_0000_0000u64 + ($left as u64) - ($right as u64) - borrow;
        (difference as u32, ($left as u64) < ($right as u64) + borrow)
    }};
    (mov, $source:expr, $scc:expr) => {
        ($source, $scc, None::<u64>)
    };
    (and, $left:expr, $right:expr) => {
        ($left & $right, Some(($left & $right) != 0), None::<u64>)
    };
    (or, $left:expr, $right:expr) => {
        ($left | $right, Some(($left | $right) != 0), None::<u64>)
    };
    (xor, $left:expr, $right:expr) => {
        ($left ^ $right, Some(($left ^ $right) != 0), None::<u64>)
    };
    (andn2, $left:expr, $right:expr) => {
        ($left & !$right, Some(($left & !$right) != 0), None::<u64>)
    };
    (and_save, $source:expr, $exec:expr) => {
        ($exec, Some(($source & $exec) != 0), Some($source & $exec))
    };
    (andn2_save, $source:expr, $exec:expr) => {
        ($exec, Some(($source & !$exec) != 0), Some($source & !$exec))
    };
    (execz, $exec:expr, $target:expr, $next:expr) => {
        if $exec == 0 { $target } else { $next }
    };
    (execnz, $exec:expr, $target:expr, $next:expr) => {
        if $exec != 0 { $target } else { $next }
    };
    (branch_delta, $displacement:expr) => {
        $displacement * 4 + 4
    };
}

#[cfg(verus_keep_ghost)]
mod conditional_lemmas {
    use vstd::prelude::*;

    verus! {
    pub open spec fn add_u32(left: u32, right: u32, carry: bool) -> (u32, bool) {
        gfx942_exec_transition_v1!(add_u32, left, right, carry)
    }
    pub open spec fn sub_u32(left: u32, right: u32, borrow: bool) -> (u32, bool) {
        gfx942_exec_transition_v1!(sub_u32, left, right, borrow)
    }

    pub proof fn addition_word_and_carry_reconstruct_sum(left: u32, right: u32, carry: bool)
        ensures
            (add_u32(left, right, carry).0 as u64
                | ((if add_u32(left, right, carry).1 { 1u64 } else { 0u64 }) << 32))
                == ((left as u64 + right as u64
                    + (if carry { 1u64 } else { 0u64 })) as u64),
    {
        let incoming: u64 = if carry { 1 } else { 0 };
        assert(incoming <= 1);
        assert(
            (((left as u64 + right as u64 + incoming) as u32) as u64
                | ((if left as u64 + right as u64 + incoming >= 0x1_0000_0000u64 {
                    1u64
                } else { 0u64 }) << 32))
                == ((left as u64 + right as u64 + incoming) as u64)
        ) by(bit_vector) requires incoming <= 1;
    }

    pub proof fn subtraction_word_and_no_borrow_reconstruct_biased_difference(
        left: u32, right: u32, borrow: bool,
    )
        ensures
            sub_u32(left, right, borrow).1
                == ((left as u64) < right as u64 + (if borrow { 1u64 } else { 0u64 })),
            (sub_u32(left, right, borrow).0 as u64
                | ((if sub_u32(left, right, borrow).1 { 0u64 } else { 1u64 }) << 32))
                == ((0x1_0000_0000u64 + left as u64 - right as u64
                    - (if borrow { 1u64 } else { 0u64 })) as u64),
    {
        let incoming: u64 = if borrow { 1 } else { 0 };
        assert(incoming <= 1);
        assert(
            (((0x1_0000_0000u64 + left as u64 - right as u64 - incoming) as u32) as u64
                | ((if (left as u64) < right as u64 + incoming { 0u64 } else { 1u64 }) << 32))
                == ((0x1_0000_0000u64 + left as u64 - right as u64 - incoming) as u64)
        ) by(bit_vector) requires incoming <= 1;
    }

    pub open spec fn and_save(source: u64, exec: u64) -> (u64, Option<bool>, Option<u64>) {
        gfx942_exec_transition_v1!(and_save, source, exec)
    }
    pub open spec fn andn2_save(source: u64, exec: u64) -> (u64, Option<bool>, Option<u64>) {
        gfx942_exec_transition_v1!(andn2_save, source, exec)
    }
    pub open spec fn mask_and(left: u64, right: u64) -> (u64, Option<bool>, Option<u64>) {
        gfx942_exec_transition_v1!(and, left, right)
    }
    pub open spec fn mask_or(left: u64, right: u64) -> (u64, Option<bool>, Option<u64>) {
        gfx942_exec_transition_v1!(or, left, right)
    }
    pub open spec fn mask_xor(left: u64, right: u64) -> (u64, Option<bool>, Option<u64>) {
        gfx942_exec_transition_v1!(xor, left, right)
    }
    pub open spec fn mask_andn2(left: u64, right: u64) -> (u64, Option<bool>, Option<u64>) {
        gfx942_exec_transition_v1!(andn2, left, right)
    }

    pub proof fn saveexec_preserves_old_mask_and_restricts(source: u64, exec: u64)
        ensures
            and_save(source, exec).0 == exec,
            and_save(source, exec).1 == Some((source & exec) != 0),
            and_save(source, exec).2 == Some(source & exec),
            ((source & exec) & !exec) == 0,
            ((source & exec) & !source) == 0,
    {
        assert(((source & exec) & !exec) == 0) by(bit_vector);
        assert(((source & exec) & !source) == 0) by(bit_vector);
    }

    pub proof fn andn2_saveexec_complements_old_exec(source: u64, exec: u64)
        ensures
            andn2_save(source, exec).0 == exec,
            andn2_save(source, exec).1 == Some((source & !exec) != 0),
            andn2_save(source, exec).2 == Some(source & !exec),
            ((source & !exec) & exec) == 0,
    {
        assert(((source & !exec) & exec) == 0) by(bit_vector);
    }

    pub proof fn scalar_bitwise_scc(left: u64, right: u64)
        ensures
            mask_and(left, right).1 == Some(mask_and(left, right).0 != 0),
            mask_or(left, right).1 == Some(mask_or(left, right).0 != 0),
            mask_xor(left, right).1 == Some(mask_xor(left, right).0 != 0),
            mask_andn2(left, right).1 == Some(mask_andn2(left, right).0 != 0),
    {}

    pub proof fn move_preserves_even_undefined_scc(source: u64, scc: Option<bool>)
        ensures gfx942_exec_transition_v1!(mov, source, scc) == (source, scc, None::<u64>),
    {}

    pub proof fn saved_mask_restores_after_and(source: u64, exec: u64)
        ensures mask_or(source & exec, and_save(source, exec).0).0 == exec,
    {
        assert(((source & exec) | exec) == exec) by(bit_vector);
    }

    pub proof fn branch_senses_are_complementary(exec: u64, target: u64, next: u64)
        ensures
            gfx942_exec_transition_v1!(execz, exec, target, next)
                == gfx942_exec_transition_v1!(execnz, exec, next, target),
            exec == 0 ==> gfx942_exec_transition_v1!(execz, exec, target, next) == target,
            exec != 0 ==> gfx942_exec_transition_v1!(execnz, exec, target, next) == target,
    {}
    }
}

#[cfg(verus_keep_ghost)]
fn main() {}
