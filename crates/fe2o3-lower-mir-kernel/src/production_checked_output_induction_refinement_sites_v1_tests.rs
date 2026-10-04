use super::*;
use fe2o3_kernel_ir::{
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirFunctionCoordinateV1 as Function,
    CanonicalKirOperationCoordinateV1 as Coordinate,
};
fn coordinate(operation: u32) -> Coordinate {
    Coordinate {
        block: Block {
            function: Function(0),
            block: 2,
        },
        operation,
    }
}

impl ProductionInductionRefinementOriginV1 {
    pub(crate) fn changed_source_for_induction_test(self) -> Self {
        Self {
            original_source: if self.original_source.is_some() {
                None
            } else {
                Some((
                    SemanticFunctionIdV1::from_index(u32::MAX),
                    SemanticBlockIdV1::from_index(0),
                    0,
                ))
            },
            ..self
        }
    }
    pub(crate) fn changed_overflow_for_induction_test(self) -> Self {
        Self {
            synthetic_overflow: if self.synthetic_overflow.is_some() {
                None
            } else {
                Some(Definition::Result {
                    operation: self.canonical.input(),
                    result: 1,
                })
            },
            ..self
        }
    }
}

#[test]
fn source_induction_refinement_synthetic_origin_never_invents_a_source_statement() {
    let input = coordinate(3);
    let row = Origin::CheckedAddSplit {
        input,
        sum_output: coordinate(3),
        false_output: coordinate(4),
        induction_row_ordinal: 0,
    };
    for site in [
        None,
        Some((
            SemanticFunctionIdV1::from_index(0),
            SemanticBlockIdV1::from_index(1),
            7,
        )),
    ] {
        let origin = source_origin(row, site);
        assert_eq!(origin.canonical_origin(), row);
        assert_eq!(origin.original_source_statement(), site);
        assert_eq!(
            origin.synthetic_overflow_definition(),
            Some(Definition::Result {
                operation: input,
                result: 1
            })
        );
        assert_eq!(origin.synthetic_false_source_statement(), None);
    }
    // These are inert record checks, not a detached source-positive admission.
    let unchanged = source_origin(
        Origin::Unchanged {
            input,
            output: coordinate(5),
        },
        None,
    );
    assert_eq!(unchanged.synthetic_overflow_definition(), None);
}
#[test]
fn source_induction_refinement_origin_equality_keeps_every_coordinate_and_source_field() {
    let row = Origin::CheckedAddSplit {
        input: coordinate(3),
        sum_output: coordinate(3),
        false_output: coordinate(4),
        induction_row_ordinal: 0,
    };
    let original = source_origin(
        row,
        Some((
            SemanticFunctionIdV1::from_index(0),
            SemanticBlockIdV1::from_index(1),
            7,
        )),
    );
    for index in 0..6 {
        let mut bad = original;
        match index {
            0 => bad.original_source = None,
            1 => {
                bad.synthetic_overflow = Some(Definition::Result {
                    operation: coordinate(3),
                    result: 0,
                })
            }
            2 => {
                if let Origin::CheckedAddSplit { input, .. } = &mut bad.canonical {
                    *input = coordinate(2)
                }
            }
            3 => {
                if let Origin::CheckedAddSplit { sum_output, .. } = &mut bad.canonical {
                    *sum_output = coordinate(4)
                }
            }
            4 => {
                if let Origin::CheckedAddSplit { false_output, .. } = &mut bad.canonical {
                    *false_output = coordinate(5)
                }
            }
            _ => {
                if let Origin::CheckedAddSplit {
                    induction_row_ordinal,
                    ..
                } = &mut bad.canonical
                {
                    *induction_row_ordinal = 1
                }
            }
        }
        assert_ne!(original, bad);
    }
}
