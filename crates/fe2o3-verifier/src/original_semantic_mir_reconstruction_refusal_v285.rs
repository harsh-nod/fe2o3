//! In-scope numeric refusal facts only; no owner queries or admission receipt.
use super::{Definition, Error, Type};
pub(super) use crate::mixed_optimizer_refinement_v26::MixedOptimizerReconstructionRefusalV285 as Facts;
use fe2o3_kernel_ir::CanonicalKirDefinitionDescendantV1 as Descendant;
use std::mem::size_of;

#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub(super) enum Phase {
    TransportDescendants,
    TransportNonScalar,
    TransportNotBlockArgument,
    TransportIncoming,
    RetainedCoordinate,
    ReplacementLocator,
    RecipeDescendants,
    ErasedRecipe,
    TargetLookup,
    TargetOwner,
}

impl Phase {
    fn name(self) -> &'static str {
        match self {
            Self::TransportDescendants => "transport-descendants",
            Self::TransportNonScalar => "transport-non-scalar",
            Self::TransportNotBlockArgument => "transport-not-block-argument",
            Self::TransportIncoming => "transport-incoming",
            Self::RetainedCoordinate => "retained-coordinate",
            Self::ReplacementLocator => "replacement-locator",
            Self::RecipeDescendants => "recipe-descendants",
            Self::ErasedRecipe => "erased-recipe",
            Self::TargetLookup => "target-lookup",
            Self::TargetOwner => "target-owner",
        }
    }
}

impl Facts {
    pub(super) fn new(original: usize, coordinate: Definition, ty: &Type) -> Self {
        let (type_class, scalar) = match ty {
            Type::Scalar(scalar) => ("scalar", Some(*scalar)),
            Type::Pointer(_) => ("pointer", None),
            Type::Slice(_) => ("slice", None),
            Type::Unit => ("unit", None),
            _ => ("other", None),
        };
        Self {
            phase: "unreturned",
            original,
            coordinate,
            type_class,
            scalar,
            descendant_count: None,
            first_descendant: None,
            target_function: None,
            target_range: None,
            target_index: None,
            binary: None,
        }
    }

    pub(super) fn descendants(mut self, rows: &[Descendant]) -> Self {
        self.descendant_count = Some(rows.len());
        self.first_descendant = rows.first().copied();
        self
    }
}

pub(super) fn headers() -> usize {
    // Root, recipe, transport and retained-locator frames can overlap.
    6 * size_of::<Facts>()
        + 2 * size_of::<Error>()
        + 16 * size_of::<usize>()
        + 4 * size_of::<&()>()
        + 2 * size_of::<Phase>()
}

// Diagnostics neither query owners after failure nor replace the first error.
pub(super) fn annotate(error: Error, phase: Phase, mut facts: Facts) -> Error {
    match error {
        Error::Statement("generated source limit") => error,
        Error::Statement(reason) => {
            facts.phase = phase.name();
            Error::SourceReconstruction { facts, reason }
        }
        Error::SourceReconstruction {
            facts: mut first,
            reason,
        } => {
            // The outer authenticated root supplies missing expected-target scope,
            // never a replacement original coordinate, phase or located index.
            if first.target_function.is_none() && first.target_range.is_none() {
                first.target_function = facts.target_function;
                first.target_range = facts.target_range;
            }
            Error::SourceReconstruction {
                facts: first,
                reason,
            }
        }
        error => error,
    }
}

#[cfg(test)]
const LINE_BYTES: usize = 2048;

#[cfg(test)]
struct Line {
    bytes: [u8; LINE_BYTES],
    len: usize,
}

#[cfg(test)]
impl std::fmt::Write for Line {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        let end = self.len.checked_add(text.len()).ok_or(std::fmt::Error)?;
        let target = self.bytes.get_mut(self.len..end).ok_or(std::fmt::Error)?;
        target.copy_from_slice(text.as_bytes());
        self.len = end;
        Ok(())
    }
}

#[cfg(test)]
fn render(out: &mut Line, phase: Phase, facts: Facts) -> std::fmt::Result {
    use std::fmt::Write as _;
    writeln!(
        out,
        "FRAME_RECONSTRUCTION_V285 phase={phase:?} facts={facts:?}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mixed_optimizer_refinement_v26::Resource;
    use fe2o3_kernel_ir::{BinaryOp, CanonicalKirFunctionCoordinateV1 as Function, ScalarType};

    #[test]
    fn reconstruction_refusal_metadata_is_bounded_and_preserves_prior_errors() {
        // Synthetic coordinates test only the diagnostic record, not admission.
        let coordinate = Definition::FunctionArgument {
            function: Function(usize::MAX as u32),
            argument: u32::MAX,
        };
        let mut facts = Facts::new(usize::MAX, coordinate, &Type::Scalar(ScalarType::U32));
        facts = facts.descendants(&[Descendant {
            output: coordinate,
            kind: fe2o3_kernel_ir::CanonicalKirDefinitionDescendantKindV1::Substituted,
        }]);
        facts.target_function = Some(Function(u32::MAX));
        facts.target_range = Some((usize::MAX, usize::MAX));
        facts.target_index = Some(usize::MAX);
        facts.binary = Some(BinaryOp::Add);
        let mut line = Line {
            bytes: [0; LINE_BYTES],
            len: 0,
        };
        render(&mut line, Phase::TargetOwner, facts).unwrap();
        let text = std::str::from_utf8(&line.bytes[..line.len]).unwrap();
        assert!(text.contains("phase=TargetOwner"));
        assert!(text.contains("descendant_count: Some(1)"));
        assert!(text.contains("Substituted") && text.contains("binary: Some(Add)"));
        assert!(line.len < LINE_BYTES);
        let source = Error::Source(
            fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Resource(
                Resource::Accounting,
            ),
        );
        assert!(matches!(
            annotate(
                Error::Resource(Resource::Accounting),
                Phase::TargetOwner,
                facts
            ),
            Error::Resource(Resource::Accounting)
        ));
        assert!(matches!(
            annotate(
                Error::Statement("generated source limit"),
                Phase::TargetOwner,
                facts
            ),
            Error::Statement("generated source limit")
        ));
        assert!(matches!(
            annotate(source, Phase::TargetOwner, facts),
            Error::Source(
                fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Resource(
                    Resource::Accounting
                )
            )
        ));
        assert!(matches!(
            annotate(
                Error::GeneratedSourceLimit {
                    section: "original section",
                    emitted_bytes: 17,
                    limit_bytes: 23,
                },
                Phase::TargetOwner,
                facts,
            ),
            Error::GeneratedSourceLimit {
                section: "original section",
                emitted_bytes: 17,
                limit_bytes: 23,
            }
        ));
        for phase in [
            Phase::TransportDescendants,
            Phase::TransportNonScalar,
            Phase::TransportNotBlockArgument,
            Phase::TransportIncoming,
            Phase::RetainedCoordinate,
            Phase::ReplacementLocator,
            Phase::RecipeDescendants,
            Phase::ErasedRecipe,
            Phase::TargetLookup,
            Phase::TargetOwner,
        ] {
            let mut line = Line {
                bytes: [0; LINE_BYTES],
                len: 0,
            };
            render(&mut line, phase, facts).unwrap();
            assert!(line.len < LINE_BYTES);
        }
        let mut short = Line {
            bytes: [0; LINE_BYTES],
            len: LINE_BYTES,
        };
        assert!(render(&mut short, Phase::TargetOwner, facts).is_err());
        assert_eq!(short.len, LINE_BYTES);
        let mut inner = Facts::new(11, coordinate, &Type::Scalar(ScalarType::U32));
        inner.descendant_count = Some(0);
        let first = annotate(
            Error::Statement("original refusal"),
            Phase::ErasedRecipe,
            inner,
        );
        let joined = annotate(first, Phase::TargetOwner, facts);
        let value = fe2o3_mir_model::SsaValueV1::BlockArgument {
            block: fe2o3_mir_model::SsaBlockIdV1::new(3),
            variable: fe2o3_mir_model::SsaVariableIdV1::new(7),
        };
        let wrapped = joined.at_frame_binding_v284(
            [0, 1, 2, 7],
            value,
            Some(0),
            "product",
            "original-target-reconstruction",
        );
        let mut returned_line = Line {
            bytes: [0; LINE_BYTES],
            len: 0,
        };
        use std::fmt::Write as _;
        write!(&mut returned_line, "{wrapped}").unwrap();
        let returned_text = std::str::from_utf8(&returned_line.bytes[..returned_line.len]).unwrap();
        assert!(returned_text.contains("SourceFrameBinding"));
        assert!(returned_text.contains("reconstruction: Some("));
        assert!(returned_text.contains("erased-recipe"));
        assert!(returned_line.len < LINE_BYTES);
        // Later reconstruction and source wrappers cannot replace either context.
        let wrapped = annotate(wrapped, Phase::TargetOwner, facts).at_frame_binding_v284(
            [9; 4],
            value,
            None,
            "later domain",
            "later phase",
        );
        assert!(matches!(wrapped, Error::SourceFrameBinding {
            source: [0, 1, 2, 7], value: actual, component: Some(0),
            domain: "product", phase: "original-target-reconstruction",
            reason: "original refusal", reconstruction: Some(first),
        } if actual == value && first.original == 11 && first.phase == "erased-recipe"
            && first.descendant_count == Some(0) && first.target_function == facts.target_function
            && first.target_range == facts.target_range && first.target_index.is_none()));
    }
}
