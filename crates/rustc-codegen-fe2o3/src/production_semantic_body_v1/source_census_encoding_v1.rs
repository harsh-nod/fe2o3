//! Complete constructor-output digest schemas, not admitted source profiles.

use super::{ProductionSemanticBodyErrorV1 as Error, table};
use fe2o3_mir_model::semantic_mir_v1::{
    AdmittedInertSemanticMirV1, SemanticCallableDeclV1, SemanticCompilerIntrinsicOperationV1,
    SemanticMirWireVersionV1 as Version, SemanticRustTypeKindV1, SemanticTypeDeclV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Family {
    Ordinary,
    Execution,
    Integer,
    OrderedRegion,
    OrderedProgram,
    Inline,
    PointerSized,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProductionSourceCensusEncodingV1(Family);

impl ProductionSourceCensusEncodingV1 {
    pub(crate) fn select(
        types: &[SemanticTypeDeclV1],
        callables: &[SemanticCallableDeclV1],
        mut charge: impl FnMut(usize) -> Result<(), Error>,
    ) -> Result<Self, Error> {
        let mut families = 0;
        for ty in types {
            charge(1)?;
            families |= match ty.rust_type_kind() {
                SemanticRustTypeKindV1::Ordinary | SemanticRustTypeKindV1::Str => 0,
                SemanticRustTypeKindV1::Execution(_) => EXECUTION,
                SemanticRustTypeKindV1::Usize | SemanticRustTypeKindV1::Isize => POINTER_SIZED,
            };
        }
        for callable in callables {
            charge(1)?;
            if let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = callable {
                families |= match operation {
                    SemanticCompilerIntrinsicOperationV1::Execution(_) => EXECUTION,
                    SemanticCompilerIntrinsicOperationV1::SaturatingInteger(_)
                    | SemanticCompilerIntrinsicOperationV1::Gfx942Wave64ShuffleIndex { .. } => {
                        INTEGER
                    }
                    SemanticCompilerIntrinsicOperationV1::Gfx942OrderedRegion(_) => ORDERED_REGION,
                    SemanticCompilerIntrinsicOperationV1::Gfx942OrderedProgram(_) => {
                        ORDERED_PROGRAM
                    }
                    SemanticCompilerIntrinsicOperationV1::Gfx942InlineU32(_) => INLINE,
                    _ => 0,
                };
            }
        }
        charge(1)?;
        Self::from_families(families)
    }

    fn from_families(families: u8) -> Result<Self, Error> {
        let family = match families {
            0 => Family::Ordinary,
            EXECUTION => Family::Execution,
            INTEGER => Family::Integer,
            ORDERED_REGION => Family::OrderedRegion,
            ORDERED_PROGRAM => Family::OrderedProgram,
            INLINE => Family::Inline,
            POINTER_SIZED => Family::PointerSized,
            value if value == ORDERED_REGION | INLINE => Family::OrderedRegion,
            value if value == ORDERED_PROGRAM | INLINE => Family::OrderedProgram,
            _ => return Err(table("source census incompatible declaration schemas")),
        };
        Ok(Self(family))
    }

    pub(crate) const fn wire_version(self) -> Version {
        match self.0 {
            Family::Ordinary => Version::V28,
            Family::Execution => Version::V29,
            Family::Integer => Version::V33,
            Family::OrderedRegion => Version::V31,
            Family::OrderedProgram => Version::V32,
            Family::Inline => Version::V34,
            Family::PointerSized => Version::V35,
        }
    }

    pub(crate) const fn is_execution(self) -> bool {
        matches!(self.0, Family::Execution)
    }

    pub(crate) fn check_source_profile(
        self,
        semantic: &AdmittedInertSemanticMirV1,
        mut charge: impl FnMut(usize) -> Result<(), Error>,
    ) -> Result<(), Error> {
        charge(1)?;
        if self.accepts_profile(semantic.wire_version()) {
            Ok(())
        } else {
            Err(table("function commitment seal completeness"))
        }
    }

    pub(crate) const fn accepts_profile(self, source: Version) -> bool {
        match self.0 {
            Family::Ordinary => matches!(
                source,
                Version::V5
                    | Version::V6
                    | Version::V7
                    | Version::V8
                    | Version::V9
                    | Version::V10
                    | Version::V11
                    | Version::V12
                    | Version::V13
                    | Version::V14
                    | Version::V15
                    | Version::V28
            ),
            Family::Execution => matches!(source, Version::V29),
            Family::Integer => matches!(source, Version::V30 | Version::V33),
            Family::OrderedRegion => matches!(source, Version::V31),
            Family::OrderedProgram => matches!(source, Version::V32),
            Family::Inline => matches!(source, Version::V34),
            Family::PointerSized => matches!(source, Version::V35),
        }
    }

    #[cfg(test)]
    pub(crate) const fn execution_for_test() -> Self {
        Self(Family::Execution)
    }
}

const EXECUTION: u8 = 1;
const INTEGER: u8 = 2;
const ORDERED_REGION: u8 = 4;
const ORDERED_PROGRAM: u8 = 8;
const INLINE: u8 = 16;
const POINTER_SIZED: u8 = 32;

#[cfg(test)]
#[path = "source_census_encoding_v1_tests.rs"]
mod tests;
