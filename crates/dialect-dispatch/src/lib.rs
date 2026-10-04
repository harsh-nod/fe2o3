//! Bounded `dispatch.*` graph and selection intent.
//!
//! These records describe dependencies, workspace lifetimes, and variant
//! selection intent. They do not load, submit, execute, or authorize runtime
//! work. This crate is representation-only.

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
pub const DIALECT_NAME: &str = "dispatch";

/// Hard maximum represented graph capacity.
pub const HARD_MAX_GRAPH_NODES: u16 = 256;

pliron::dict_key!(
    DISPATCH_REGISTRATION_KEY,
    "fe2o3_dialect_dispatch_explicit_registration"
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
            Self::MarkerCollision => formatter.write_str("dispatch registration marker collision"),
            Self::CorruptMarker => formatter.write_str("dispatch registration marker is corrupt"),
        }
    }
}

impl Error for RegistrationError {}

/// Fixed-width graph, node, variant, event, or workspace identity.
#[pliron_attr(name = "dispatch.id")]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct DispatchIdAttr([u64; 4]);

impl DispatchIdAttr {
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

impl Verify for DispatchIdAttr {
    fn verify(&self, _context: &Context) -> Result<()> {
        if self.is_zero() {
            return verify_err_noloc!("dispatch.id cannot be the reserved all-zero identity");
        }
        Ok(())
    }
}

impl Printable for DispatchIdAttr {
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

impl Parsable for DispatchIdAttr {
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

/// Finite graph capacity classes.
#[pliron_attr(name = "dispatch.graph_capacity", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum GraphCapacityAttr {
    Nodes16,
    Nodes64,
    Nodes256,
}

impl GraphCapacityAttr {
    pub const fn max_nodes(self) -> u16 {
        match self {
            Self::Nodes16 => 16,
            Self::Nodes64 => 64,
            Self::Nodes256 => HARD_MAX_GRAPH_NODES,
        }
    }
}

/// Orthogonal execution-shape intent, without execution authority.
#[pliron_attr(name = "dispatch.mode", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DispatchModeAttr {
    UnfusedFinite,
    FiniteFusion,
    PersistentService,
}

/// Meaning of one directed dependency edge.
#[pliron_attr(name = "dispatch.dependency_kind", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DependencyKindAttr {
    Data,
    Completion,
    Visibility,
}

/// Finite workspace allocation classes.
#[pliron_attr(name = "dispatch.workspace_class", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum WorkspaceClassAttr {
    Bytes4096,
    Bytes65536,
    Bytes1048576,
    Bytes67108864,
}

impl WorkspaceClassAttr {
    pub const fn max_bytes(self) -> u64 {
        match self {
            Self::Bytes4096 => 4_096,
            Self::Bytes65536 => 65_536,
            Self::Bytes1048576 => 1_048_576,
            Self::Bytes67108864 => 67_108_864,
        }
    }
}

/// Lifetime owner for one workspace intent.
#[pliron_attr(name = "dispatch.workspace_lifetime", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum WorkspaceLifetimeAttr {
    Graph,
    Phase,
    Node,
}

/// Variant-selection intent. Admission must occur elsewhere.
#[pliron_attr(name = "dispatch.selection_policy", format, verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SelectionPolicyAttr {
    Exact,
    DeterministicRanked,
    SafeFallback,
}

/// Marker type for graph references.
#[pliron_type(
    name = "dispatch.graph_ref",
    format,
    generate_get = true,
    verifier = "succ"
)]
#[derive(Debug, Eq, Hash, PartialEq)]
pub struct GraphRefType;

/// Marker type for dependency events.
#[pliron_type(
    name = "dispatch.event_ref",
    format,
    generate_get = true,
    verifier = "succ"
)]
#[derive(Debug, Eq, Hash, PartialEq)]
pub struct EventRefType;

/// Marker type for workspace references.
#[pliron_type(
    name = "dispatch.workspace_ref",
    format,
    generate_get = true,
    verifier = "succ"
)]
#[derive(Debug, Eq, Hash, PartialEq)]
pub struct WorkspaceRefType;

/// Interface shared by dispatch-intent records.
#[op_interface]
pub trait DispatchIntentOpInterface {
    fn verify(_op: &dyn Op, _context: &Context) -> Result<()>
    where
        Self: Sized,
    {
        Ok(())
    }

    fn is_executable(&self) -> bool {
        false
    }

    fn grants_runtime_authority(&self) -> bool {
        false
    }
}

/// Declares a bounded graph intent.
#[pliron_op(
    name = "dispatch.graph_intent",
    format = "attr($dispatch_graph_intent_graph_id, $DispatchIdAttr) ` ` attr($dispatch_graph_intent_capacity, $GraphCapacityAttr) ` ` attr($dispatch_graph_intent_mode, $DispatchModeAttr)",
    interfaces = [
        DispatchIntentOpInterface,
        NOpdsInterface<0>,
        NResultsInterface<0>,
        NRegionsInterface<0>
    ],
    attributes = (
        dispatch_graph_intent_graph_id: DispatchIdAttr,
        dispatch_graph_intent_capacity: GraphCapacityAttr,
        dispatch_graph_intent_mode: DispatchModeAttr
    )
)]
pub struct GraphIntentOp;

impl GraphIntentOp {
    pub fn new(
        context: &mut Context,
        graph_id: DispatchIdAttr,
        capacity: GraphCapacityAttr,
        mode: DispatchModeAttr,
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
        op.set_attr_dispatch_graph_intent_graph_id(context, graph_id);
        op.set_attr_dispatch_graph_intent_capacity(context, capacity);
        op.set_attr_dispatch_graph_intent_mode(context, mode);
        op
    }
}

impl Verify for GraphIntentOp {
    fn verify(&self, context: &Context) -> Result<()> {
        verify_closed_shape(self, context, 3)?;
        required_attr(
            self,
            context,
            self.get_attr_dispatch_graph_intent_graph_id(context),
            "graph_id",
        )?;
        let capacity = required_attr(
            self,
            context,
            self.get_attr_dispatch_graph_intent_capacity(context),
            "capacity",
        )?;
        required_attr(
            self,
            context,
            self.get_attr_dispatch_graph_intent_mode(context),
            "mode",
        )?;
        if capacity.max_nodes() > HARD_MAX_GRAPH_NODES {
            return verify_err!(
                self.loc(context),
                "dispatch graph exceeds its hard node cap"
            );
        }
        Ok(())
    }
}

/// Declares one non-reflexive dependency intent.
#[pliron_op(
    name = "dispatch.dependency_intent",
    format = "attr($dispatch_dependency_intent_graph_id, $DispatchIdAttr) ` ` attr($dispatch_dependency_intent_predecessor_id, $DispatchIdAttr) ` ` attr($dispatch_dependency_intent_successor_id, $DispatchIdAttr) ` ` attr($dispatch_dependency_intent_kind, $DependencyKindAttr)",
    interfaces = [
        DispatchIntentOpInterface,
        NOpdsInterface<0>,
        NResultsInterface<0>,
        NRegionsInterface<0>
    ],
    attributes = (
        dispatch_dependency_intent_graph_id: DispatchIdAttr,
        dispatch_dependency_intent_predecessor_id: DispatchIdAttr,
        dispatch_dependency_intent_successor_id: DispatchIdAttr,
        dispatch_dependency_intent_kind: DependencyKindAttr
    )
)]
pub struct DependencyIntentOp;

impl DependencyIntentOp {
    pub fn new(
        context: &mut Context,
        graph_id: DispatchIdAttr,
        predecessor_id: DispatchIdAttr,
        successor_id: DispatchIdAttr,
        kind: DependencyKindAttr,
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
        op.set_attr_dispatch_dependency_intent_graph_id(context, graph_id);
        op.set_attr_dispatch_dependency_intent_predecessor_id(context, predecessor_id);
        op.set_attr_dispatch_dependency_intent_successor_id(context, successor_id);
        op.set_attr_dispatch_dependency_intent_kind(context, kind);
        op
    }
}

impl Verify for DependencyIntentOp {
    fn verify(&self, context: &Context) -> Result<()> {
        verify_closed_shape(self, context, 4)?;
        required_attr(
            self,
            context,
            self.get_attr_dispatch_dependency_intent_graph_id(context),
            "graph_id",
        )?;
        let predecessor = required_attr(
            self,
            context,
            self.get_attr_dispatch_dependency_intent_predecessor_id(context),
            "predecessor_id",
        )?;
        let successor = required_attr(
            self,
            context,
            self.get_attr_dispatch_dependency_intent_successor_id(context),
            "successor_id",
        )?;
        required_attr(
            self,
            context,
            self.get_attr_dispatch_dependency_intent_kind(context),
            "kind",
        )?;
        if predecessor == successor {
            return verify_err!(
                self.loc(context),
                "dispatch dependency intent cannot contain a reflexive edge"
            );
        }
        Ok(())
    }
}

/// Declares one bounded workspace and its lifetime owner.
#[pliron_op(
    name = "dispatch.workspace_intent",
    format = "attr($dispatch_workspace_intent_graph_id, $DispatchIdAttr) ` ` attr($dispatch_workspace_intent_workspace_id, $DispatchIdAttr) ` ` attr($dispatch_workspace_intent_owner_id, $DispatchIdAttr) ` ` attr($dispatch_workspace_intent_class, $WorkspaceClassAttr) ` ` attr($dispatch_workspace_intent_lifetime, $WorkspaceLifetimeAttr)",
    interfaces = [
        DispatchIntentOpInterface,
        NOpdsInterface<0>,
        NResultsInterface<0>,
        NRegionsInterface<0>
    ],
    attributes = (
        dispatch_workspace_intent_graph_id: DispatchIdAttr,
        dispatch_workspace_intent_workspace_id: DispatchIdAttr,
        dispatch_workspace_intent_owner_id: DispatchIdAttr,
        dispatch_workspace_intent_class: WorkspaceClassAttr,
        dispatch_workspace_intent_lifetime: WorkspaceLifetimeAttr
    )
)]
pub struct WorkspaceIntentOp;

impl WorkspaceIntentOp {
    pub fn new(
        context: &mut Context,
        graph_id: DispatchIdAttr,
        workspace_id: DispatchIdAttr,
        owner_id: DispatchIdAttr,
        class: WorkspaceClassAttr,
        lifetime: WorkspaceLifetimeAttr,
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
        op.set_attr_dispatch_workspace_intent_graph_id(context, graph_id);
        op.set_attr_dispatch_workspace_intent_workspace_id(context, workspace_id);
        op.set_attr_dispatch_workspace_intent_owner_id(context, owner_id);
        op.set_attr_dispatch_workspace_intent_class(context, class);
        op.set_attr_dispatch_workspace_intent_lifetime(context, lifetime);
        op
    }
}

impl Verify for WorkspaceIntentOp {
    fn verify(&self, context: &Context) -> Result<()> {
        verify_closed_shape(self, context, 5)?;
        let graph_id = required_attr(
            self,
            context,
            self.get_attr_dispatch_workspace_intent_graph_id(context),
            "graph_id",
        )?;
        required_attr(
            self,
            context,
            self.get_attr_dispatch_workspace_intent_workspace_id(context),
            "workspace_id",
        )?;
        let owner_id = required_attr(
            self,
            context,
            self.get_attr_dispatch_workspace_intent_owner_id(context),
            "owner_id",
        )?;
        let class = required_attr(
            self,
            context,
            self.get_attr_dispatch_workspace_intent_class(context),
            "class",
        )?;
        let lifetime = required_attr(
            self,
            context,
            self.get_attr_dispatch_workspace_intent_lifetime(context),
            "lifetime",
        )?;
        if class.max_bytes() > 67_108_864 {
            return verify_err!(
                self.loc(context),
                "dispatch workspace exceeds its hard byte cap"
            );
        }
        if (lifetime == WorkspaceLifetimeAttr::Graph) != (owner_id == graph_id) {
            return verify_err!(
                self.loc(context),
                "graph-lifetime workspace ownership must name exactly its graph"
            );
        }
        Ok(())
    }
}

/// Declares selected and fallback variant intent without admitting either variant.
#[pliron_op(
    name = "dispatch.selection_intent",
    format = "attr($dispatch_selection_intent_graph_id, $DispatchIdAttr) ` ` attr($dispatch_selection_intent_selected_variant_id, $DispatchIdAttr) ` ` attr($dispatch_selection_intent_fallback_variant_id, $DispatchIdAttr) ` ` attr($dispatch_selection_intent_policy, $SelectionPolicyAttr)",
    interfaces = [
        DispatchIntentOpInterface,
        NOpdsInterface<0>,
        NResultsInterface<0>,
        NRegionsInterface<0>
    ],
    attributes = (
        dispatch_selection_intent_graph_id: DispatchIdAttr,
        dispatch_selection_intent_selected_variant_id: DispatchIdAttr,
        dispatch_selection_intent_fallback_variant_id: DispatchIdAttr,
        dispatch_selection_intent_policy: SelectionPolicyAttr
    )
)]
pub struct SelectionIntentOp;

impl SelectionIntentOp {
    pub fn new(
        context: &mut Context,
        graph_id: DispatchIdAttr,
        selected_variant_id: DispatchIdAttr,
        fallback_variant_id: DispatchIdAttr,
        policy: SelectionPolicyAttr,
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
        op.set_attr_dispatch_selection_intent_graph_id(context, graph_id);
        op.set_attr_dispatch_selection_intent_selected_variant_id(context, selected_variant_id);
        op.set_attr_dispatch_selection_intent_fallback_variant_id(context, fallback_variant_id);
        op.set_attr_dispatch_selection_intent_policy(context, policy);
        op
    }
}

impl Verify for SelectionIntentOp {
    fn verify(&self, context: &Context) -> Result<()> {
        verify_closed_shape(self, context, 4)?;
        required_attr(
            self,
            context,
            self.get_attr_dispatch_selection_intent_graph_id(context),
            "graph_id",
        )?;
        let selected = required_attr(
            self,
            context,
            self.get_attr_dispatch_selection_intent_selected_variant_id(context),
            "selected_variant_id",
        )?;
        let fallback = required_attr(
            self,
            context,
            self.get_attr_dispatch_selection_intent_fallback_variant_id(context),
            "fallback_variant_id",
        )?;
        let policy = required_attr(
            self,
            context,
            self.get_attr_dispatch_selection_intent_policy(context),
            "policy",
        )?;
        match policy {
            SelectionPolicyAttr::Exact if selected != fallback => {
                return verify_err!(
                    self.loc(context),
                    "exact dispatch selection must name one identical selected/fallback variant"
                );
            }
            SelectionPolicyAttr::DeterministicRanked | SelectionPolicyAttr::SafeFallback
                if selected == fallback =>
            {
                return verify_err!(
                    self.loc(context),
                    "ranked or fallback dispatch selection requires distinct variants"
                );
            }
            _ => {}
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

/// Explicitly registers every `dispatch.*` type, attribute, and operation.
pub fn register_dialect(
    context: &mut Context,
) -> std::result::Result<RegistrationOutcome, RegistrationError> {
    if let Some(index) = context
        .aux_data_map
        .get(&*DISPATCH_REGISTRATION_KEY)
        .copied()
    {
        return match context.aux_data.get(index) {
            Some(marker) if marker.downcast_ref::<RegistrationMarker>().is_some() => {
                Ok(RegistrationOutcome::AlreadyRegistered)
            }
            Some(_) => Err(RegistrationError::MarkerCollision),
            None => Err(RegistrationError::CorruptMarker),
        };
    }

    let dialect_name = DialectName::try_new(DIALECT_NAME).expect("static dispatch dialect name");
    Dialect::register(context, &dialect_name);

    <DispatchIdAttr as Attribute>::register::<DispatchIdAttr>(context);
    <GraphCapacityAttr as Attribute>::register::<GraphCapacityAttr>(context);
    <DispatchModeAttr as Attribute>::register::<DispatchModeAttr>(context);
    <DependencyKindAttr as Attribute>::register::<DependencyKindAttr>(context);
    <WorkspaceClassAttr as Attribute>::register::<WorkspaceClassAttr>(context);
    <WorkspaceLifetimeAttr as Attribute>::register::<WorkspaceLifetimeAttr>(context);
    <SelectionPolicyAttr as Attribute>::register::<SelectionPolicyAttr>(context);
    <GraphRefType as Type>::register(context);
    <EventRefType as Type>::register(context);
    <WorkspaceRefType as Type>::register(context);
    <GraphIntentOp as Op>::register(context);
    <DependencyIntentOp as Op>::register(context);
    <WorkspaceIntentOp as Op>::register(context);
    <SelectionIntentOp as Op>::register(context);

    let marker = context.aux_data.insert(Box::new(RegistrationMarker));
    context
        .aux_data_map
        .insert(DISPATCH_REGISTRATION_KEY.clone(), marker);
    Ok(RegistrationOutcome::Registered)
}
