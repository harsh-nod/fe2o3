//! Bounded inert intent for one named source-owned local-order composition.
//!
//! Decoding or comparing this record never authenticates source, imports a
//! compiler owner, reuses evidence, or grants build/launch authority. The live
//! compiler must freshly capture source, retain its actual fixed Policy6 prefix,
//! replay I -> L, check actual constraints, and derive fresh analysis/artifacts.
//! Exactness names canonical KIR order, never final LLVM/machine scheduling.
use fe2o3_kernel_ir::VerifiedCanonicalKernelIrIdentityV12;
use fe2o3_kernel_opt::U32LocalOrderPreferenceV1;
use serde::{Deserialize, Serialize};
use std::io::{self, Write};

pub(crate) const BYTE_CAP: usize = 8192;
#[cfg(test)]
pub(crate) const SCHEMA: &str = "fe2o3-source-local-order-recipe-v1";
pub(crate) const COMPOSITION: &str = "source-local-order-policy6-v1";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InstanceBinding {
    pub(crate) function: [u8; 32],
    pub(crate) item: [u8; 32],
    pub(crate) monomorphization: [u8; 32],
    pub(crate) generic_types: [u8; 32],
    pub(crate) const_arguments: [u8; 32],
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Order {
    SourceOrder,
    ReverseReady,
}
impl Order {
    pub(crate) const fn preference(self) -> U32LocalOrderPreferenceV1 {
        match self {
            Self::SourceOrder => U32LocalOrderPreferenceV1::SourceOrder,
            Self::ReverseReady => U32LocalOrderPreferenceV1::ReverseReady,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Relation {
    XorBeforeOr,
    OrBeforeXor,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Strength {
    Exact,
    Advisory,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Constraint {
    pub(crate) relation: Relation,
    pub(crate) strength: Strength,
}

/// An inert comparison predicate, NOT a deserialized verified KIR identity.
/// Its digest and length must both match a freshly obtained typed identity.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProgramIdentity {
    digest: [u8; 32],
    canonical_length: u64,
}
impl ProgramIdentity {
    pub(crate) const fn from_verified(identity: &VerifiedCanonicalKernelIrIdentityV12) -> Self {
        Self {
            digest: *identity.digest(),
            canonical_length: identity.canonical_length(),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum SourceBinding {
    ExactRevision {
        expected_source_sha256: [u8; 32],
        expected_original: ProgramIdentity,
        expected_prefix: ProgramIdentity,
    },
    RebindCurrent {},
}

/// Historical annotations only. Even ExactRevision uses its separately named
/// refusal predicates, not these annotations, to compare the current program.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Origin {
    pub(crate) source: [u8; 32],
    pub(crate) semantic: [u8; 32],
    pub(crate) bound: [u8; 32],
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum Schema {
    #[serde(rename = "fe2o3-source-local-order-recipe-v1")]
    V1,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum Composition {
    #[serde(rename = "source-local-order-policy6-v1")]
    V1,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum Target {
    #[serde(rename = "gfx942_xnack_off_wave64")]
    Gfx942XnackOffWave64,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum Shape {
    #[serde(rename = "u32_xor_or_and_four_distinct_formal_parameters")]
    U32XorOrAndFourDistinctFormalParameters,
}

/// Closed immutable intent, deliberately not a compilation or analysis owner.
/// No user-selected pass list, receipt, source span, executable or output graph.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Recipe {
    schema: Schema,
    composition: Composition,
    target: Target,
    required: [u32; 3],
    maximum: [u32; 3],
    parameter_ordinals: [u32; 4],
    shape: Shape,
    binding: InstanceBinding,
    preference: Order,
    constraint: Constraint,
    source_binding: SourceBinding,
    origin: Origin,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Error {
    ByteLimit,
    Allocation,
    MalformedOrUnsupported,
    ProfileMismatch,
    InstanceChanged,
    SourceRevisionChanged,
    OriginalProgramChanged,
    PrefixProgramChanged,
    ExactConstraintNotHonored {
        requested: Relation,
        actual: Relation,
    },
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ByteLimit => f.write_str("local-order recipe byte limit"),
            Self::Allocation => f.write_str("local-order recipe bounded allocation failed"),
            Self::MalformedOrUnsupported => {
                f.write_str("local-order recipe malformed or unsupported")
            }
            Self::ProfileMismatch => f.write_str("local-order recipe exact profile mismatch"),
            Self::InstanceChanged => {
                f.write_str("local-order recipe item/instance binding changed")
            }
            Self::SourceRevisionChanged => {
                f.write_str("local-order recipe source revision changed")
            }
            Self::OriginalProgramChanged => {
                f.write_str("local-order recipe original program changed")
            }
            Self::PrefixProgramChanged => {
                f.write_str("local-order recipe fixed-prefix program changed")
            }
            Self::ExactConstraintNotHonored { .. } => {
                f.write_str("local-order recipe exact canonical constraint not honored")
            }
        }
    }
}
impl std::error::Error for Error {}

/// Pure comparison output. The compiler must derive actual order from the
/// retained checked I/L owners; accepting an arbitrary Relation is not authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum ConstraintOutcome {
    Honored {
        relation: Relation,
    },
    NotHonored {
        requested: Relation,
        actual: Relation,
    },
}

impl Recipe {
    /// Constructs inert intent only; current binding and execution remain fresh.
    pub(crate) const fn new(
        binding: InstanceBinding,
        origin: Origin,
        preference: Order,
        constraint: Constraint,
        source_binding: SourceBinding,
    ) -> Self {
        Self {
            schema: Schema::V1,
            composition: Composition::V1,
            target: Target::Gfx942XnackOffWave64,
            required: [64, 1, 1],
            maximum: [64, 1, 1],
            parameter_ordinals: [1, 2, 3, 4],
            shape: Shape::U32XorOrAndFourDistinctFormalParameters,
            binding,
            preference,
            constraint,
            source_binding,
            origin,
        }
    }

    fn validate(&self) -> Result<(), Error> {
        if self.required != [64, 1, 1]
            || self.maximum != [64, 1, 1]
            || self.parameter_ordinals != [1, 2, 3, 4]
        {
            return Err(Error::ProfileMismatch);
        }
        Ok(())
    }

    pub(crate) fn decode(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.is_empty() || bytes.len() > BYTE_CAP {
            return Err(Error::ByteLimit);
        }
        // No retained decoded String/Vec/map exists. Fixed arrays and closed variants
        // bound allocation/nesting; serde visitors also reject duplicate fields.
        let recipe: Self =
            serde_json::from_slice(bytes).map_err(|_| Error::MalformedOrUnsupported)?;
        recipe.validate()?;
        Ok(recipe)
    }

    pub(crate) fn encode(&self) -> Result<Vec<u8>, Error> {
        self.validate()?;
        let mut writer = BoundedWriter { bytes: Vec::new() };
        writer
            .bytes
            .try_reserve_exact(BYTE_CAP)
            .map_err(|_| Error::Allocation)?;
        serde_json::to_writer(&mut writer, self).map_err(|_| Error::ByteLimit)?;
        Ok(writer.bytes)
    }

    /// Early refusal predicates only. This cannot authenticate target, source,
    /// launch, numerical/resource applicability, a source anchor, or an owner.
    pub(crate) fn bind(
        &self,
        current: InstanceBinding,
        current_source_sha256: [u8; 32],
    ) -> Result<Order, Error> {
        self.validate()?;
        if self.binding != current {
            return Err(Error::InstanceChanged);
        }
        if let SourceBinding::ExactRevision {
            expected_source_sha256,
            ..
        } = self.source_binding
            && expected_source_sha256 != current_source_sha256
        {
            return Err(Error::SourceRevisionChanged);
        }
        Ok(self.preference)
    }

    /// Compare both full typed identities from the actual current N and fixed I.
    /// No verified identity or owner can be reconstructed from this recipe.
    pub(crate) fn check_current_program(
        &self,
        original: &VerifiedCanonicalKernelIrIdentityV12,
        prefix: &VerifiedCanonicalKernelIrIdentityV12,
    ) -> Result<(), Error> {
        self.check_program_predicates(
            ProgramIdentity::from_verified(original),
            ProgramIdentity::from_verified(prefix),
        )
    }

    fn check_program_predicates(
        &self,
        original: ProgramIdentity,
        prefix: ProgramIdentity,
    ) -> Result<(), Error> {
        self.validate()?;
        if let SourceBinding::ExactRevision {
            expected_original,
            expected_prefix,
            ..
        } = self.source_binding
        {
            if original != expected_original {
                return Err(Error::OriginalProgramChanged);
            }
            if prefix != expected_prefix {
                return Err(Error::PrefixProgramChanged);
            }
        }
        Ok(())
    }

    /// Check a fact derived by the compiler from its retained, replayed I/L.
    /// Advisory mismatch changes only the explicit report, never the computation
    /// or selected schedule. Exact mismatch exposes no successful recipe result.
    pub(crate) fn evaluate_constraint(&self, actual: Relation) -> Result<ConstraintOutcome, Error> {
        self.validate()?;
        let requested = self.constraint.relation;
        if requested == actual {
            return Ok(ConstraintOutcome::Honored { relation: actual });
        }
        match self.constraint.strength {
            Strength::Exact => Err(Error::ExactConstraintNotHonored { requested, actual }),
            Strength::Advisory => Ok(ConstraintOutcome::NotHonored { requested, actual }),
        }
    }

    #[cfg(test)]
    pub(crate) const fn binding(&self) -> InstanceBinding {
        self.binding
    }
    pub(crate) const fn preference(&self) -> Order {
        self.preference
    }
    pub(crate) const fn constraint(&self) -> Constraint {
        self.constraint
    }
    pub(crate) const fn source_binding(&self) -> SourceBinding {
        self.source_binding
    }
    #[cfg(test)]
    pub(crate) const fn origin(&self) -> Origin {
        self.origin
    }
    #[cfg(test)]
    pub(crate) const fn grants_authority(&self) -> bool {
        false
    }
}

struct BoundedWriter {
    bytes: Vec<u8>,
}
impl Write for BoundedWriter {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        let length = self
            .bytes
            .len()
            .checked_add(input.len())
            .ok_or_else(|| io::Error::other("local-order recipe byte limit"))?;
        if length > BYTE_CAP || length > self.bytes.capacity() {
            return Err(io::Error::other("local-order recipe byte limit"));
        }
        self.bytes.extend_from_slice(input);
        Ok(input.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
#[path = "source_local_order_recipe_v1_tests.rs"]
mod tests;
