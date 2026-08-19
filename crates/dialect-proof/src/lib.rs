//! Solver-neutral, non-executable `proof.*` Pliron overlay.
//!
//! Every evidence reference names exactly one property and one status at one
//! covered boundary. No status, operation, or interface in this crate grants
//! proof promotion, publication, loading, launch, or runtime authority.

#![forbid(unsafe_code)]

use std::{error::Error, fmt};

use pliron::{
    attribute::Attribute,
    builtin::op_interfaces::{NOpdsInterface, NRegionsInterface, NResultsInterface},
    combine::{Parser, count_min_max, parser::char::hex_digit},
    common_traits::Verify,
    context::Context,
    derive::{op_interface, pliron_attr, pliron_op, pliron_type},
    dialect::{Dialect, DialectName},
    op::Op,
    operation::Operation,
    parsable::{Parsable, ParseResult, StateStream},
    printable::{self, Printable},
    result::Result,
    r#type::Type,
    verify_err, verify_err_noloc,
};

mod registration;

pub use registration::dialect_registration;

/// Pliron dialect name.
pub const DIALECT_NAME: &str = "proof";

/// Number of bits in every stable reference carried by this shell.
pub const IDENTITY_BITS: usize = 256;

pliron::dict_key!(
    PROOF_REGISTRATION_KEY,
    "fe2o3_dialect_proof_explicit_registration"
);

#[derive(Debug)]
struct RegistrationMarker;

/// Result of explicitly registering this dialect in a context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistrationOutcome {
    /// The complete dialect surface was explicitly registered.
    Registered,
    /// The same complete surface was already registered by this crate.
    AlreadyRegistered,
}

/// A fail-closed explicit registration error.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistrationError {
    /// Another typed value already claimed this crate's marker key.
    MarkerCollision,
    /// The marker map referenced absent auxiliary data.
    CorruptMarker,
}

impl fmt::Display for RegistrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MarkerCollision => formatter.write_str("proof registration marker collision"),
            Self::CorruptMarker => formatter.write_str("proof registration marker is corrupt"),
        }
    }
}

impl Error for RegistrationError {}

/// A fixed-width canonical reference. The all-zero value is reserved and rejected.
#[pliron_attr(name = "proof.id")]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ProofIdAttr([u64; 4]);

impl ProofIdAttr {
    pub const fn new(words: [u64; 4]) -> Self {
        Self(words)
    }

    pub const fn words(&self) -> [u64; 4] {
        self.0
    }

    pub const fn is_zero(&self) -> bool {
        self.0[0] == 0 && self.0[1] == 0 && self.0[2] == 0 && self.0[3] == 0
    }
}

impl Verify for ProofIdAttr {
    fn verify(&self, _context: &Context) -> Result<()> {
        if self.is_zero() {
            return verify_err_noloc!("proof.id cannot be the reserved all-zero identity");
        }
        Ok(())
    }
}

impl Printable for ProofIdAttr {
    fn fmt(
        &self,
        _context: &Context,
        _state: &printable::State,
        formatter: &mut core::fmt::Formatter<'_>,
    ) -> core::fmt::Result {
        write!(
            formatter,
            "{:016x}{:016x}{:016x}{:016x}",
            self.0[0], self.0[1], self.0[2], self.0[3]
        )
    }
}

impl Parsable for ProofIdAttr {
    type Arg = ();
    type Parsed = Self;

    fn parse<'a>(
        state_stream: &mut StateStream<'a>,
        _arg: Self::Arg,
    ) -> ParseResult<'a, Self::Parsed> {
        let word = || {
            count_min_max::<String, _, _>(16, 16, hex_digit())
                .and_then(|digits| u64::from_str_radix(&digits, 16))
        };
        word()
            .and(word())
            .and(word())
            .and(word())
            .map(|(((first, second), third), fourth)| Self([first, second, third, fourth]))
            .parse_stream(state_stream)
            .into()
    }
}

/// Independently tracked proof properties.
#[pliron_attr(name = "proof.property", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PropertyAttr {
    Bounds,
    Provenance,
    RegionDisjointness,
    Initialization,
    RaceFreedom,
    Convergence,
    FunctionalRefinement,
    NumericalBound,
    Determinism,
    DeadlockFreedom,
}

/// Status of one property only. It is never a global verification badge.
#[pliron_attr(name = "proof.evidence_status", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum EvidenceStatusAttr {
    Proved,
    Validated,
    Contracted,
    Checked,
    Unsupported,
}

impl EvidenceStatusAttr {
    /// Evidence status alone never grants authority.
    pub const fn grants_authority(self) -> bool {
        false
    }
}

/// Last semantic boundary covered by one evidence record.
#[pliron_attr(name = "proof.covered_boundary", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CoveredBoundaryAttr {
    Source,
    Mir,
    StructuredKernel,
    Schedule,
    TargetNeutralGpu,
}

/// Marker type for obligation references in proof-only schemas.
#[pliron_type(
    name = "proof.obligation_ref",
    format,
    generate_get = true,
    verifier = "succ"
)]
#[derive(Debug, Eq, Hash, PartialEq)]
pub struct ObligationRefType;

/// Marker type for evidence references in proof-only schemas.
#[pliron_type(
    name = "proof.evidence_ref",
    format,
    generate_get = true,
    verifier = "succ"
)]
#[derive(Debug, Eq, Hash, PartialEq)]
pub struct EvidenceRefType;

/// Interface shared by inert proof overlay operations.
#[op_interface]
pub trait ProofOverlayOpInterface {
    fn verify(_op: &dyn Op, _context: &Context) -> Result<()>
    where
        Self: Sized,
    {
        Ok(())
    }

    fn is_executable(&self) -> bool {
        false
    }

    fn grants_authority(&self) -> bool {
        false
    }
}

/// Declares one solver-neutral property obligation.
#[pliron_op(
    name = "proof.obligation",
    format = "attr($proof_obligation_obligation_id, $ProofIdAttr) ` ` attr($proof_obligation_subject_id, $ProofIdAttr) ` ` attr($proof_obligation_model_id, $ProofIdAttr) ` ` attr($proof_obligation_property, $PropertyAttr)",
    interfaces = [
        ProofOverlayOpInterface,
        NOpdsInterface<0>,
        NResultsInterface<0>,
        NRegionsInterface<0>
    ],
    attributes = (
        proof_obligation_obligation_id: ProofIdAttr,
        proof_obligation_subject_id: ProofIdAttr,
        proof_obligation_model_id: ProofIdAttr,
        proof_obligation_property: PropertyAttr
    )
)]
pub struct ObligationOp;

impl ObligationOp {
    pub fn new(
        context: &mut Context,
        obligation_id: ProofIdAttr,
        subject_id: ProofIdAttr,
        model_id: ProofIdAttr,
        property: PropertyAttr,
    ) -> Self {
        let operation = Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![],
            vec![],
            vec![],
            0,
        );
        let op = Self::from_operation(operation);
        op.set_attr_proof_obligation_obligation_id(context, obligation_id);
        op.set_attr_proof_obligation_subject_id(context, subject_id);
        op.set_attr_proof_obligation_model_id(context, model_id);
        op.set_attr_proof_obligation_property(context, property);
        op
    }
}

impl Verify for ObligationOp {
    fn verify(&self, context: &Context) -> Result<()> {
        verify_closed_shape(self, context, 4)?;
        required_attr(
            self,
            context,
            self.get_attr_proof_obligation_obligation_id(context),
            "obligation_id",
        )?;
        required_attr(
            self,
            context,
            self.get_attr_proof_obligation_subject_id(context),
            "subject_id",
        )?;
        required_attr(
            self,
            context,
            self.get_attr_proof_obligation_model_id(context),
            "model_id",
        )?;
        required_attr(
            self,
            context,
            self.get_attr_proof_obligation_property(context),
            "property",
        )?;
        Ok(())
    }
}

/// References evidence for exactly one obligation/property/status tuple.
#[pliron_op(
    name = "proof.evidence_ref",
    format = "attr($proof_evidence_ref_evidence_id, $ProofIdAttr) ` ` attr($proof_evidence_ref_obligation_id, $ProofIdAttr) ` ` attr($proof_evidence_ref_property, $PropertyAttr) ` ` attr($proof_evidence_ref_status, $EvidenceStatusAttr) ` ` attr($proof_evidence_ref_covered_boundary, $CoveredBoundaryAttr)",
    interfaces = [
        ProofOverlayOpInterface,
        NOpdsInterface<0>,
        NResultsInterface<0>,
        NRegionsInterface<0>
    ],
    attributes = (
        proof_evidence_ref_evidence_id: ProofIdAttr,
        proof_evidence_ref_obligation_id: ProofIdAttr,
        proof_evidence_ref_property: PropertyAttr,
        proof_evidence_ref_status: EvidenceStatusAttr,
        proof_evidence_ref_covered_boundary: CoveredBoundaryAttr
    )
)]
pub struct EvidenceRefOp;

impl EvidenceRefOp {
    pub fn new(
        context: &mut Context,
        evidence_id: ProofIdAttr,
        obligation_id: ProofIdAttr,
        property: PropertyAttr,
        status: EvidenceStatusAttr,
        covered_boundary: CoveredBoundaryAttr,
    ) -> Self {
        let operation = Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![],
            vec![],
            vec![],
            0,
        );
        let op = Self::from_operation(operation);
        op.set_attr_proof_evidence_ref_evidence_id(context, evidence_id);
        op.set_attr_proof_evidence_ref_obligation_id(context, obligation_id);
        op.set_attr_proof_evidence_ref_property(context, property);
        op.set_attr_proof_evidence_ref_status(context, status);
        op.set_attr_proof_evidence_ref_covered_boundary(context, covered_boundary);
        op
    }

    pub fn status(&self, context: &Context) -> Option<EvidenceStatusAttr> {
        self.get_attr_proof_evidence_ref_status(context)
            .map(|status| *status)
    }
}

impl Verify for EvidenceRefOp {
    fn verify(&self, context: &Context) -> Result<()> {
        verify_closed_shape(self, context, 5)?;
        let evidence_id = required_attr(
            self,
            context,
            self.get_attr_proof_evidence_ref_evidence_id(context),
            "evidence_id",
        )?;
        let obligation_id = required_attr(
            self,
            context,
            self.get_attr_proof_evidence_ref_obligation_id(context),
            "obligation_id",
        )?;
        required_attr(
            self,
            context,
            self.get_attr_proof_evidence_ref_property(context),
            "property",
        )?;
        required_attr(
            self,
            context,
            self.get_attr_proof_evidence_ref_status(context),
            "status",
        )?;
        required_attr(
            self,
            context,
            self.get_attr_proof_evidence_ref_covered_boundary(context),
            "covered_boundary",
        )?;
        if evidence_id == obligation_id {
            return verify_err!(
                self.loc(context),
                "proof evidence and obligation identities must occupy distinct domains"
            );
        }
        Ok(())
    }
}

fn verification_error(op: &dyn Op, context: &Context, message: &str) -> pliron::result::Error {
    pliron::verify_error!(op.loc(context), "{message}")
}

fn required_attr<T: Clone>(
    op: &dyn Op,
    context: &Context,
    value: Option<std::cell::Ref<'_, T>>,
    name: &str,
) -> Result<T> {
    value
        .map(|value| (*value).clone())
        .ok_or_else(|| verification_error(op, context, &format!("missing typed {name} attribute")))
}

fn verify_closed_shape(op: &dyn Op, context: &Context, attributes: usize) -> Result<()> {
    let operation = op.get_operation();
    let operation = operation.deref(context);
    if operation.get_num_operands() != 0
        || operation.get_num_results() != 0
        || operation.get_num_successors() != 0
        || operation.num_regions() != 0
        || operation.attributes.0.len() != attributes
    {
        return verify_err!(
            op.loc(context),
            "{} has malformed or unbounded structural payload",
            op.get_opid()
        );
    }
    Ok(())
}

/// Explicitly registers every `proof.*` type, attribute, and operation.
pub fn register_dialect(
    context: &mut Context,
) -> std::result::Result<RegistrationOutcome, RegistrationError> {
    if let Some(index) = context.aux_data_map.get(&*PROOF_REGISTRATION_KEY).copied() {
        return match context.aux_data.get(index) {
            Some(marker) if marker.downcast_ref::<RegistrationMarker>().is_some() => {
                Ok(RegistrationOutcome::AlreadyRegistered)
            }
            Some(_) => Err(RegistrationError::MarkerCollision),
            None => Err(RegistrationError::CorruptMarker),
        };
    }

    let dialect_name = DialectName::try_new(DIALECT_NAME).expect("static proof dialect name");
    Dialect::register(context, &dialect_name);

    <ProofIdAttr as Attribute>::register::<ProofIdAttr>(context);
    <PropertyAttr as Attribute>::register::<PropertyAttr>(context);
    <EvidenceStatusAttr as Attribute>::register::<EvidenceStatusAttr>(context);
    <CoveredBoundaryAttr as Attribute>::register::<CoveredBoundaryAttr>(context);
    <ObligationRefType as Type>::register(context);
    <EvidenceRefType as Type>::register(context);
    <ObligationOp as Op>::register(context);
    <EvidenceRefOp as Op>::register(context);

    let marker = context.aux_data.insert(Box::new(RegistrationMarker));
    context
        .aux_data_map
        .insert(PROOF_REGISTRATION_KEY.clone(), marker);
    Ok(RegistrationOutcome::Registered)
}
