#![forbid(unsafe_code)]

extern crate std;

use core::{cell::RefCell, ops::Index};
use std::{panic::AssertUnwindSafe, vec, vec::Vec};

#[derive(Clone, Copy)]
struct TestIndex(usize);

impl TestIndex {
    fn get(&self) -> usize {
        self.0
    }
}

mod cpu_thread {
    pub(super) fn index_1d(linear: usize) -> super::TestIndex {
        super::TestIndex(linear)
    }
}

struct TrackedInput<'a> {
    values: &'a [f32],
    reads: RefCell<Vec<usize>>,
}

impl<'a> TrackedInput<'a> {
    fn new(values: &'a [f32]) -> Self {
        Self {
            values,
            reads: RefCell::new(Vec::new()),
        }
    }
}

impl Index<usize> for TrackedInput<'_> {
    type Output = f32;

    fn index(&self, index: usize) -> &Self::Output {
        self.reads.borrow_mut().push(index);
        &self.values[index]
    }
}

struct TestOutput<'a> {
    values: &'a mut [f32],
    attempts: Vec<usize>,
}

impl<'a> TestOutput<'a> {
    fn new(values: &'a mut [f32]) -> Self {
        Self {
            values,
            attempts: Vec::new(),
        }
    }

    fn get_mut(&mut self, index: TestIndex) -> Option<&mut f32> {
        self.attempts.push(index.0);
        self.values.get_mut(index.0)
    }
}

fn invoke(linear: usize, a: &TrackedInput<'_>, b: &TrackedInput<'_>, output: &mut TestOutput<'_>) {
    vecadd_kernel_body!(cpu_thread, (linear), production_f32_add, a, b, output);
}

fn bits(values: &[f32]) -> Vec<u32> {
    values.iter().map(|value| value.to_bits()).collect()
}

fn assert_reads(a: &TrackedInput<'_>, b: &TrackedInput<'_>, expected: &[usize]) {
    assert_eq!(a.reads.borrow().as_slice(), expected);
    assert_eq!(b.reads.borrow().as_slice(), expected);
}

#[test]
fn shared_body_nonuniform_finite_inputs_and_rounding() {
    let a = TrackedInput::new(&[
        1.25,
        -3.5,
        8.0,
        -0.75,
        16_777_216.0,
        -16_777_216.0,
        0.125,
        -100.5,
    ]);
    let b = TrackedInput::new(&[2.5, 1.25, -8.0, -0.125, 1.0, -1.0, 0.25, 2.25]);
    let expected = [
        3.75,
        -2.25,
        0.0,
        -0.875,
        16_777_216.0,
        -16_777_216.0,
        0.375,
        -98.25,
    ];
    let mut values = [91.5; 8];
    let mut output = TestOutput::new(&mut values);
    for index in 0..expected.len() {
        invoke(index, &a, &b, &mut output);
    }
    assert_eq!(bits(output.values), bits(&expected));
    let active = (0..expected.len()).collect::<Vec<_>>();
    assert_reads(&a, &b, &active);
    assert_eq!(output.attempts, active);
}

#[test]
fn shared_body_floating_special_values_preserve_observed_host_semantics() {
    let a = TrackedInput::new(&[
        0.0,
        -0.0,
        -0.0,
        0.0,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::INFINITY,
        f32::NAN,
        1.0,
        f32::MAX,
        -f32::MAX,
    ]);
    let b = TrackedInput::new(&[
        0.0,
        -0.0,
        0.0,
        -0.0,
        2.0,
        -2.0,
        f32::NEG_INFINITY,
        1.0,
        f32::NAN,
        f32::MAX,
        -f32::MAX,
    ]);
    // None requires a computed NaN, without requiring its sign or payload.
    let expected = [
        Some(0x0000_0000),
        Some(0x8000_0000),
        Some(0x0000_0000),
        Some(0x0000_0000),
        Some(0x7f80_0000),
        Some(0xff80_0000),
        None,
        None,
        None,
        Some(0x7f80_0000),
        Some(0xff80_0000),
    ];
    let mut values = [91.5; 11];
    let mut output = TestOutput::new(&mut values);
    for index in 0..expected.len() {
        invoke(index, &a, &b, &mut output);
    }
    for (value, expected) in output.values.iter().zip(expected) {
        match expected {
            Some(expected) => assert_eq!(value.to_bits(), expected),
            None => assert!(value.is_nan()),
        }
    }
    let active = (0..expected.len()).collect::<Vec<_>>();
    assert_reads(&a, &b, &active);
    assert_eq!(output.attempts, active);
}

#[test]
fn shared_body_output_extent_canaries_and_launch_tail() {
    const CANARY: f32 = -8192.5;
    for length in [1, 63, 64, 65] {
        let left = vec![1.25; length];
        let right = vec![2.5; length];
        let a = TrackedInput::new(&left);
        let b = TrackedInput::new(&right);
        let mut backing = vec![CANARY; length + 4];
        let launched = length.div_ceil(64) * 64 + 3;
        {
            let mut output = TestOutput::new(&mut backing[2..length + 2]);
            for index in 0..launched {
                invoke(index, &a, &b, &mut output);
            }
            assert!(
                output
                    .values
                    .iter()
                    .all(|value| value.to_bits() == 3.75_f32.to_bits())
            );
            assert_eq!(output.attempts, (0..launched).collect::<Vec<_>>());
        }
        assert_reads(&a, &b, &(0..length).collect::<Vec<_>>());
        assert_eq!(bits(&backing[..2]), vec![CANARY.to_bits(); 2]);
        assert_eq!(bits(&backing[length + 2..]), vec![CANARY.to_bits(); 2]);
    }
}

#[test]
fn shared_body_empty_and_extreme_inactive_indices_do_not_read_inputs() {
    let a = TrackedInput::new(&[]);
    let b = TrackedInput::new(&[]);
    let mut empty = [];
    let mut output = TestOutput::new(&mut empty);
    invoke(0, &a, &b, &mut output);
    invoke(usize::MAX, &a, &b, &mut output);
    assert_eq!(output.attempts, [0, usize::MAX]);
    assert_reads(&a, &b, &[]);

    let mut backing = [-77.5, 91.5, 92.5, -78.5];
    {
        let mut output = TestOutput::new(&mut backing[1..3]);
        invoke(2, &a, &b, &mut output);
        invoke(usize::MAX, &a, &b, &mut output);
        assert_eq!(output.attempts, [2, usize::MAX]);
    }
    assert_eq!(bits(&backing), bits(&[-77.5, 91.5, 92.5, -78.5]));
    assert_reads(&a, &b, &[]);
}

#[test]
fn shared_body_accepts_distinct_sufficient_input_extents() {
    let a = TrackedInput::new(&[1.25, -3.5, 800.0, 900.0]);
    let b = TrackedInput::new(&[2.5, 1.25, 1000.0]);
    let mut values = [91.5; 2];
    let mut output = TestOutput::new(&mut values);
    for index in 0..6 {
        invoke(index, &a, &b, &mut output);
    }
    assert_eq!(bits(output.values), bits(&[3.75, -2.25]));
    assert_reads(&a, &b, &[0, 1]);
    assert_eq!(output.attempts, [0, 1, 2, 3, 4, 5]);
}

#[test]
fn shared_body_active_short_inputs_panic_without_rolling_back_prior_writes() {
    for short_left in [true, false] {
        let full_left = [1.25, -3.5];
        let full_right = [2.5, 1.25];
        let a = TrackedInput::new(if short_left {
            &full_left[..1]
        } else {
            &full_left
        });
        let b = TrackedInput::new(if short_left {
            &full_right
        } else {
            &full_right[..1]
        });
        let mut values = [91.5; 2];
        let mut output = TestOutput::new(&mut values);
        invoke(0, &a, &b, &mut output);
        let failed = std::panic::catch_unwind(AssertUnwindSafe(|| {
            invoke(1, &a, &b, &mut output);
        }));
        assert!(failed.is_err());
        assert_eq!(bits(output.values), bits(&[3.75, 91.5]));
        assert_eq!(a.reads.borrow().as_slice(), &[0, 1]);
        assert_eq!(
            b.reads.borrow().as_slice(),
            if short_left { &[0][..] } else { &[0, 1][..] }
        );
        invoke(2, &a, &b, &mut output);
        invoke(usize::MAX, &a, &b, &mut output);
        assert_eq!(output.attempts, [0, 1, 2, usize::MAX]);
        assert_eq!(a.reads.borrow().as_slice(), &[0, 1]);
        assert_eq!(
            b.reads.borrow().as_slice(),
            if short_left { &[0][..] } else { &[0, 1][..] }
        );
        assert_eq!(bits(output.values), bits(&[3.75, 91.5]));
    }
}
