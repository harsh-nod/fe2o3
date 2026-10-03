//! Initialize the checked argument basis; failed private scratch is discarded.

use super::{CheckedU32PrefixArgumentV1, Origin};

macro_rules! ordinary_exec {
    ($body:expr) => {
        $body
    };
}

include!("basis_body.rs");

pub(super) fn initialize_argument_basis(
    arguments: &[CheckedU32PrefixArgumentV1],
    source: &mut [Origin],
    kernel: &mut [Origin],
) -> bool {
    checked_u32_prefix_basis_body_v1!(
        ordinary_exec,
        arguments,
        source,
        kernel,
        clear,
        [],
        index,
        []
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::ValueId;

    fn binding(argument: usize, semantic_local: u32) -> CheckedU32PrefixArgumentV1 {
        CheckedU32PrefixArgumentV1 {
            argument,
            semantic_local,
            kernel_ir_value: ValueId(91 - argument as u32),
        }
    }

    #[test]
    fn initializes_paired_basis_and_clears_only_unmapped_source_slots() {
        let arguments = [binding(0, 2), binding(1, 0)];
        let mut source = [Origin::Constant(u32::MAX); 5];
        let mut kernel = [Origin::Argument(usize::MAX); 2];
        assert!(initialize_argument_basis(
            &arguments,
            &mut source,
            &mut kernel
        ));
        assert_eq!(
            source,
            [
                Origin::Argument(1),
                Origin::Uninitialized,
                Origin::Argument(0),
                Origin::Uninitialized,
                Origin::Uninitialized
            ]
        );
        assert_eq!(kernel, [Origin::Argument(0), Origin::Argument(1)]);
    }

    #[test]
    fn rejects_misordered_or_duplicate_argument_labels() {
        for arguments in [
            [binding(1, 2), binding(0, 0)],
            [binding(0, 2), binding(0, 0)],
        ] {
            assert!(!initialize_argument_basis(
                &arguments,
                &mut [Origin::Uninitialized; 3],
                &mut [Origin::Uninitialized; 2]
            ));
        }
    }

    #[test]
    fn rejects_duplicate_and_out_of_range_locals_without_panicking() {
        for arguments in [
            [binding(0, 0), binding(1, 0)],
            [binding(0, 2), binding(1, 3)],
            [binding(0, u32::MAX), binding(1, 0)],
        ] {
            assert!(!initialize_argument_basis(
                &arguments,
                &mut [Origin::Uninitialized; 3],
                &mut [Origin::Uninitialized; 2]
            ));
        }
    }

    #[test]
    fn rejects_both_kernel_length_mismatches() {
        let arguments = [binding(0, 0)];
        for length in [0, 2] {
            assert!(!initialize_argument_basis(
                &arguments,
                &mut [Origin::Uninitialized; 1],
                &mut vec![Origin::Uninitialized; length]
            ));
        }
    }

    #[test]
    fn empty_basis_erases_dirty_source_and_unmapped_reads_still_reject() {
        let mut source = [Origin::Argument(usize::MAX), Origin::Constant(0)];
        assert!(initialize_argument_basis(&[], &mut source, &mut []));
        assert_eq!(source, [Origin::Uninitialized; 2]);
        assert!(!super::super::fold::fold(
            &mut source,
            &[super::super::PrefixStep {
                destination: 1,
                input: super::super::PrefixInput::Cell(0),
            }]
        ));
        assert!(initialize_argument_basis(&[], &mut [], &mut []));
    }

    #[test]
    fn accepts_exactly_injective_local_maps_in_small_exhaustive_domain() {
        for a in 0..5 {
            for b in 0..5 {
                for c in 0..5 {
                    let arguments = [binding(0, a), binding(1, b), binding(2, c)];
                    let mut source = [Origin::Constant(99); 4];
                    let mut kernel = [Origin::Constant(42); 3];
                    let expected =
                        [a, b, c].iter().all(|local| *local < 4) && a != b && a != c && b != c;
                    assert_eq!(
                        initialize_argument_basis(&arguments, &mut source, &mut kernel),
                        expected
                    );
                    if expected {
                        for (index, local) in [a, b, c].into_iter().enumerate() {
                            assert_eq!(source[local as usize], Origin::Argument(index));
                            assert_eq!(kernel[index], Origin::Argument(index));
                        }
                        for (local, origin) in source.iter().enumerate() {
                            if ![a, b, c].contains(&(local as u32)) {
                                assert_eq!(*origin, Origin::Uninitialized);
                            }
                        }
                    }
                }
            }
        }
    }
}
