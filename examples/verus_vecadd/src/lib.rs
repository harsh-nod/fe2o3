#![no_std]
#![forbid(unsafe_code)]

use fe2o3_contracts::{IdentityWriteIndex, LaunchDomain1d, ThreadInDomain1d};

include!("vecadd_body.rs");
include!("elementwise_bodies.rs");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VecAddError {
    DomainLengthMismatch,
    ArithmeticOverflow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FillError {
    DomainLengthMismatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ElementwiseError {
    DomainLengthMismatch,
    GatherIndexOutOfBounds,
}

fn identity_source(thread: usize) -> usize {
    thread
}

fn exact_affine(value: i16, scale: i16, bias: i32) -> i64 {
    i64::from(value) * i64::from(scale) + i64::from(bias)
}

fn selected_source(indices: &[usize], thread: usize) -> usize {
    indices[thread]
}

/// Executes one identity-indexed copy write.
pub fn copy_thread(
    thread: usize,
    input: &[i64],
    output: &mut [i64],
) -> Result<(), ElementwiseError> {
    copy_kernel_body!(
        thread,
        identity_source,
        input,
        output,
        ElementwiseError::DomainLengthMismatch
    )
}

/// Copies all input elements through the shared per-thread body.
pub fn copy(input: &[i64], output: &mut [i64]) -> Result<(), ElementwiseError> {
    if input.len() != output.len() {
        return Err(ElementwiseError::DomainLengthMismatch);
    }
    for thread in 0..output.len() {
        copy_thread(thread, input, output)?;
    }
    Ok(())
}

/// Executes one exact affine map, widening before every arithmetic operation.
pub fn affine_map_thread(
    thread: usize,
    input: &[i16],
    output: &mut [i64],
    scale: i16,
    bias: i32,
) -> Result<(), ElementwiseError> {
    affine_map_kernel_body!(
        thread,
        exact_affine,
        input,
        output,
        scale,
        bias,
        ElementwiseError::DomainLengthMismatch
    )
}

/// Maps every input element to `value * scale + bias` exactly in `i64`.
pub fn affine_map(
    input: &[i16],
    output: &mut [i64],
    scale: i16,
    bias: i32,
) -> Result<(), ElementwiseError> {
    if input.len() != output.len() {
        return Err(ElementwiseError::DomainLengthMismatch);
    }
    for thread in 0..output.len() {
        affine_map_thread(thread, input, output, scale, bias)?;
    }
    Ok(())
}

/// Executes one bounds-checked gather write.
pub fn gather_thread(
    thread: usize,
    input: &[i64],
    indices: &[usize],
    output: &mut [i64],
) -> Result<(), ElementwiseError> {
    gather_kernel_body!(
        thread,
        selected_source,
        input,
        indices,
        output,
        ElementwiseError::DomainLengthMismatch,
        ElementwiseError::GatherIndexOutOfBounds
    )
}

/// Gathers every requested input element through the shared per-thread body.
pub fn gather(
    input: &[i64],
    indices: &[usize],
    output: &mut [i64],
) -> Result<(), ElementwiseError> {
    if indices.len() != output.len() {
        return Err(ElementwiseError::DomainLengthMismatch);
    }
    for thread in 0..output.len() {
        gather_thread(thread, input, indices, output)?;
    }
    Ok(())
}

/// Executes the work assigned to one logical thread.
pub fn vecadd_thread(
    domain: LaunchDomain1d,
    thread: ThreadInDomain1d,
    a: &[u32],
    b: &[u32],
    output: &mut [u32],
) -> Result<(), VecAddError> {
    vecadd_thread_body!(
        domain,
        thread,
        a,
        b,
        output,
        IdentityWriteIndex,
        VecAddError::DomainLengthMismatch,
        VecAddError::ArithmeticOverflow
    )
}

/// CPU reference driver over the same per-thread contract.
pub fn vecadd(a: &[u32], b: &[u32], output: &mut [u32]) -> Result<(), VecAddError> {
    if a.len() != b.len() || a.len() != output.len() {
        return Err(VecAddError::DomainLengthMismatch);
    }

    let domain = LaunchDomain1d::new(output.len());
    for linear in 0..domain.len() {
        let thread = domain
            .thread(linear)
            .ok_or(VecAddError::DomainLengthMismatch)?;
        vecadd_thread(domain, thread, a, b, output)?;
    }
    Ok(())
}

/// Executes one identity-indexed fill write for a logical thread.
pub fn fill_thread(
    domain: LaunchDomain1d,
    thread: ThreadInDomain1d,
    output: &mut [u32],
    value: u32,
) -> Result<(), FillError> {
    if thread.domain() != domain || output.len() != domain.len() {
        return Err(FillError::DomainLengthMismatch);
    }

    let write =
        IdentityWriteIndex::new(thread, output.len()).ok_or(FillError::DomainLengthMismatch)?;
    output[write.index().value()] = value;
    Ok(())
}

/// CPU reference driver over the per-thread fill contract.
pub fn fill(output: &mut [u32], value: u32) -> Result<(), FillError> {
    let domain = LaunchDomain1d::new(output.len());
    for linear in 0..domain.len() {
        let thread = domain
            .thread(linear)
            .ok_or(FillError::DomainLengthMismatch)?;
        fill_thread(domain, thread, output, value)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verus_harness_includes_the_real_gpu_source_fragment() {
        let harness = include_str!("../verus/vecadd.rs");
        let real_body = include_str!("../../vecadd/src/vecadd_body.rs");
        let real_kernel = include_str!("../../vecadd/src/main.rs");

        assert!(harness.contains("include!(\"../../vecadd/src/vecadd_body.rs\")"));
        assert!(real_body.contains("vecadd_kernel_body"));
        assert!(real_body.contains("*out = $add!($a[i], $b[i])"));
        assert!(real_kernel.contains("macro_rules! production_f32_add"));
        assert!(real_kernel.contains("$lhs + $rhs"));
    }

    #[test]
    fn elementwise_verus_harnesses_expand_the_shared_bodies() {
        let positive = include_str!("../verus/elementwise.rs");
        let bodies = include_str!("elementwise_bodies.rs");

        assert!(positive.contains("include!(\"../src/elementwise_bodies.rs\")"));
        for (declaration, invocation) in [
            ("macro_rules! copy_kernel_body", "copy_kernel_body!"),
            (
                "macro_rules! affine_map_kernel_body",
                "affine_map_kernel_body!",
            ),
            ("macro_rules! gather_kernel_body", "gather_kernel_body!"),
        ] {
            assert!(bodies.contains(declaration));
            assert!(positive.contains(invocation));
        }

        for fixture in [
            include_str!("../verus/negative/copy_wrong_source.rs"),
            include_str!("../verus/negative/affine_wrong_bias.rs"),
            include_str!("../verus/negative/gather_wrong_index.rs"),
        ] {
            assert!(fixture.contains("include!(\"../../src/elementwise_bodies.rs\")"));
        }
    }

    #[test]
    fn wave_and_lds_proofs_extend_the_branded_permission_model() {
        let positive = include_str!("../verus/wave_lds.rs");
        for marker in [
            "include!(\"vecadd.rs\")",
            "active_values_determine_reduction",
            "distinct_active_lanes_have_disjoint_scan_outputs",
            "owned_lds_write_is_in_bounds_and_framed",
            "distinct_threads_have_disjoint_lds_writes",
            "convergent_barrier_enables_shared_lds_read",
        ] {
            assert!(positive.contains(marker), "missing proof marker {marker}");
        }
        for shortcut in ["admit(", "assume(false", "#[verifier::external_body]"] {
            assert!(
                !positive.contains(shortcut),
                "wave/LDS proof contains forbidden shortcut {shortcut}"
            );
        }

        for (fixture, marker) in [
            (
                include_str!("../verus/negative/wave_inactive_lane_contributes.rs"),
                "mutated_inactive_lane_contributes",
            ),
            (
                include_str!("../verus/negative/lds_duplicate_writer.rs"),
                "mutated_duplicate_lds_writers_are_race_free",
            ),
            (
                include_str!("../verus/negative/lds_read_before_barrier.rs"),
                "mutated_read_before_barrier_is_legal",
            ),
            (
                include_str!("../verus/negative/lds_out_of_bounds_read.rs"),
                "mutated_unbounded_lds_read_is_in_bounds",
            ),
        ] {
            assert!(fixture.contains(marker), "missing mutation marker {marker}");
        }
    }

    #[test]
    fn copy_executes_identity_writes_and_rejects_shape_mismatches() {
        let mut output = [0; 4];
        assert_eq!(copy(&[7, -2, 11, 19], &mut output), Ok(()));
        assert_eq!(output, [7, -2, 11, 19]);

        assert_eq!(
            copy(&[1, 2], &mut output),
            Err(ElementwiseError::DomainLengthMismatch)
        );
        assert_eq!(
            copy_thread(4, &[1, 2, 3, 4], &mut output),
            Err(ElementwiseError::DomainLengthMismatch)
        );
    }

    #[test]
    fn affine_map_is_exact_for_extreme_inputs() {
        let input = [i16::MIN, -1, 0, i16::MAX];
        let mut output = [0; 4];
        assert_eq!(affine_map(&input, &mut output, i16::MIN, i32::MAX), Ok(()));
        assert_eq!(
            output,
            input.map(|value| { i64::from(value) * i64::from(i16::MIN) + i64::from(i32::MAX) })
        );

        assert_eq!(
            affine_map(&input[..3], &mut output, 2, -1),
            Err(ElementwiseError::DomainLengthMismatch)
        );
    }

    #[test]
    fn gather_checks_every_selected_input_index_before_writing() {
        let input = [10, 20, 30, 40];
        let mut output = [0; 3];
        assert_eq!(gather(&input, &[3, 0, 2], &mut output), Ok(()));
        assert_eq!(output, [40, 10, 30]);

        output = [9; 3];
        assert_eq!(
            gather(&input, &[3, 4, 2], &mut output),
            Err(ElementwiseError::GatherIndexOutOfBounds)
        );
        assert_eq!(output, [40, 9, 9]);

        output = [8; 3];
        assert_eq!(
            gather(&input, &[0, 1], &mut output),
            Err(ElementwiseError::DomainLengthMismatch)
        );
        assert_eq!(output, [8; 3]);
    }

    #[test]
    fn vecadd_executes_one_disjoint_write_per_thread() {
        let a = [1, 2, 3, 4];
        let b = [10, 20, 30, 40];
        let mut output = [0; 4];

        assert_eq!(vecadd(&a, &b, &mut output), Ok(()));
        assert_eq!(output, [11, 22, 33, 44]);
    }

    #[test]
    fn vecadd_rejects_a_domain_buffer_mismatch() {
        let domain = LaunchDomain1d::new(3);
        let thread = domain.thread(0).unwrap();
        let mut output = [0; 3];

        assert_eq!(
            vecadd_thread(domain, thread, &[1, 2], &[4, 5, 6], &mut output),
            Err(VecAddError::DomainLengthMismatch)
        );
        assert_eq!(output, [0; 3]);

        assert_eq!(
            vecadd_thread(domain, thread, &[1, 2, 3], &[4, 5], &mut output),
            Err(VecAddError::DomainLengthMismatch)
        );
        assert_eq!(output, [0; 3]);

        let wrong_domain_thread = LaunchDomain1d::new(2).thread(0).unwrap();

        assert_eq!(
            vecadd_thread(
                domain,
                wrong_domain_thread,
                &[1, 2, 3],
                &[4, 5, 6],
                &mut output,
            ),
            Err(VecAddError::DomainLengthMismatch)
        );
        assert_eq!(output, [0; 3]);
    }

    #[test]
    fn vecadd_reports_integer_overflow() {
        let mut output = [0];

        assert_eq!(vecadd(&[u32::MAX], &[0], &mut output), Ok(()));
        assert_eq!(output, [u32::MAX]);

        output[0] = 0;
        assert_eq!(
            vecadd(&[u32::MAX], &[1], &mut output),
            Err(VecAddError::ArithmeticOverflow)
        );
        assert_eq!(output, [0]);
    }

    #[test]
    fn fill_executes_one_identity_write_per_thread() {
        let mut output = [1, 2, 3, 4];

        assert_eq!(fill(&mut output, 17), Ok(()));
        assert_eq!(output, [17; 4]);
    }

    #[test]
    fn fill_accepts_an_empty_domain() {
        assert_eq!(fill(&mut [], 17), Ok(()));
    }

    #[test]
    fn fill_rejects_a_thread_from_another_domain() {
        let domain = LaunchDomain1d::new(2);
        let other_domain = LaunchDomain1d::new(3);
        let thread = other_domain.thread(0).unwrap();
        let mut output = [0; 2];

        assert_eq!(
            fill_thread(domain, thread, &mut output, 17),
            Err(FillError::DomainLengthMismatch)
        );
        assert_eq!(output, [0; 2]);
    }
}
