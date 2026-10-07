//! In-scope numeric refusal facts only; no owner queries or admission receipt.
use super::{Definition, Error, Type};
use fe2o3_kernel_ir::{
    BinaryOp, CanonicalKirDefinitionDescendantV1 as Descendant,
    CanonicalKirFunctionCoordinateV1 as Function, ScalarType,
};
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

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub(super) struct Facts {
    pub original: usize,
    pub coordinate: Definition,
    pub type_class: &'static str,
    pub scalar: Option<ScalarType>,
    pub descendant_count: Option<usize>,
    pub first_descendant: Option<Descendant>,
    pub target_function: Option<Function>,
    pub target_range: Option<(usize, usize)>,
    pub target_index: Option<usize>,
    pub binary: Option<BinaryOp>,
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

const LINE_BYTES: usize = 2048;

pub(super) fn headers() -> usize {
    // Root, recipe, transport and retained-locator frames can overlap.
    6 * size_of::<Facts>()
        + LINE_BYTES
        + 16 * size_of::<usize>()
        + 4 * size_of::<&()>()
        + 2 * size_of::<Phase>()
}

#[cfg(test)]
fn eligible(error: &Error) -> bool {
    matches!(error, Error::Statement(reason) if *reason != "generated source limit")
}

// Diagnostics neither query owners after failure nor replace the first error.
pub(super) fn annotate(error: Error, phase: Phase, facts: Facts) -> Error {
    #[cfg(test)]
    if eligible(&error) {
        use std::io::Write as _;
        let mut line = Line {
            bytes: [0; LINE_BYTES],
            len: 0,
        };
        if render(&mut line, phase, facts).is_ok() {
            let _ = std::io::stderr().lock().write_all(&line.bytes[..line.len]);
        }
    }
    #[cfg(not(test))]
    let _ = (phase, facts);
    error
}

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
        assert!(eligible(&Error::Statement("mismatch")));
        assert!(!eligible(&Error::Statement("generated source limit")));
        assert!(!eligible(&Error::Resource(Resource::Accounting)));
        let source = Error::Source(
            fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Resource(
                Resource::Accounting,
            ),
        );
        assert!(!eligible(&source));
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
    }
}
