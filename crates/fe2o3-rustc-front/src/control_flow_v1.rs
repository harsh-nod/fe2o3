//! Canonical source control-flow sidecar.

use std::collections::VecDeque;
use std::fmt;

pub const CONTROL_FLOW_CONTRACT_MAGIC_V1: [u8; 8] = *b"FE2O3CF\0";
pub const CONTROL_FLOW_CONTRACT_VERSION_V1: u16 = 1;
pub const CONTROL_FLOW_REGISTRATION_PREFIX_V1: &str = "__fe2o3_control_flow_contract_v1_";
pub const CONTROL_FLOW_REGISTRATION_MAGIC_V1: u64 = u64::from_le_bytes(*b"FE2O3CFA");
pub const CONTROL_FLOW_REGISTRATION_VERSION_V1: u16 = 1;
pub const CONTROL_FLOW_REGISTRATION_KIND_V1: u16 = 1;
pub const MAX_CONTROL_FLOW_CONTRACT_BYTES_V1: usize = 1024 * 1024;
pub const MAX_CONTROL_FLOW_NODES_V1: usize = 4096;
pub const MAX_CONTROL_FLOW_EDGES_V1: usize = 16_384;
pub const MAX_INTEGER_SWITCH_CASES_V1: usize = 256;
pub const MAX_SOURCE_FILE_BYTES_V1: usize = 1024;

const HEADER_BYTES_V1: usize = 28;
const CFG_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3-CANONICAL-CFG-IDENTITY-V1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ControlFlowValidationErrorV1 {
    EmptyGraph,
    TooMany { field: &'static str, max: usize },
    Overflow { field: &'static str },
    InvalidSourceFile,
    InvalidSourceSpan,
    NonDenseNodeId { expected: u32, actual: u32 },
    InvalidEntryNode(u32),
    EntryKindRequired(u32),
    DuplicateEntryNode(u32),
    MissingExit,
    InvalidSuccessor { node: u32, successor: u32 },
    UnreachableNode(u32),
    ZeroLoopBound(u32),
    LoopHasNoBackedge(u32),
    InvalidLoopTransfer { node: u32, loop_header: u32 },
    BreakTargetMismatch { node: u32 },
    ContinueTargetMismatch { node: u32 },
    UnsupportedIntegerWidth(u16),
    IntegerCaseOutOfRange { node: u32, bits: u128 },
    DuplicateIntegerCase { node: u32, bits: u128 },
    IrreducibleControlFlow,
    EncodedContractTooLarge,
}

impl fmt::Display for ControlFlowValidationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyGraph => formatter.write_str("control-flow graph must not be empty"),
            Self::TooMany { field, max } => write!(formatter, "{field} exceeds {max}"),
            Self::Overflow { field } => write!(formatter, "{field} overflows its wire field"),
            Self::InvalidSourceFile => formatter.write_str("source file is empty or invalid"),
            Self::InvalidSourceSpan => formatter.write_str("source span is empty or reversed"),
            Self::NonDenseNodeId { expected, actual } => write!(
                formatter,
                "control-flow node IDs must be dense: expected {expected}, found {actual}"
            ),
            Self::InvalidEntryNode(node) => write!(formatter, "entry node {node} does not exist"),
            Self::EntryKindRequired(node) => {
                write!(formatter, "entry node {node} must have entry kind")
            }
            Self::DuplicateEntryNode(node) => {
                write!(formatter, "non-entry node {node} has entry kind")
            }
            Self::MissingExit => formatter.write_str("control-flow graph has no exit node"),
            Self::InvalidSuccessor { node, successor } => {
                write!(
                    formatter,
                    "node {node} references missing successor {successor}"
                )
            }
            Self::UnreachableNode(node) => write!(formatter, "node {node} is unreachable"),
            Self::ZeroLoopBound(node) => {
                write!(formatter, "loop node {node} has a zero iteration bound")
            }
            Self::LoopHasNoBackedge(node) => {
                write!(formatter, "loop node {node} has no structural backedge")
            }
            Self::InvalidLoopTransfer { node, loop_header } => write!(
                formatter,
                "node {node} references non-loop header {loop_header}"
            ),
            Self::BreakTargetMismatch { node } => {
                write!(formatter, "break node {node} does not target its loop exit")
            }
            Self::ContinueTargetMismatch { node } => {
                write!(
                    formatter,
                    "continue node {node} does not target its loop header"
                )
            }
            Self::UnsupportedIntegerWidth(width) => {
                write!(formatter, "integer switch width {width} is unsupported")
            }
            Self::IntegerCaseOutOfRange { node, bits } => write!(
                formatter,
                "integer switch node {node} case bits {bits:#034x} are outside its discriminant type"
            ),
            Self::DuplicateIntegerCase { node, bits } => {
                write!(
                    formatter,
                    "integer switch node {node} duplicates case bits {bits:#034x}"
                )
            }
            Self::IrreducibleControlFlow => {
                formatter.write_str("irreducible control flow is unsupported")
            }
            Self::EncodedContractTooLarge => {
                formatter.write_str("control-flow contract exceeds its byte limit")
            }
        }
    }
}

impl std::error::Error for ControlFlowValidationErrorV1 {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ControlFlowDecodeErrorV1 {
    TooLarge,
    Truncated,
    InvalidMagic,
    UnknownVersion(u16),
    UnsupportedFlags(u16),
    InvalidLength(u32),
    TrailingBytes,
    NonzeroReserved(&'static str),
    CountOutOfRange {
        field: &'static str,
        count: u64,
        max: usize,
    },
    InvalidUtf8,
    UnknownTag {
        kind: &'static str,
        tag: u16,
    },
    NonCanonical,
    Validation(ControlFlowValidationErrorV1),
}

impl fmt::Display for ControlFlowDecodeErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => formatter.write_str("control-flow contract exceeds its byte limit"),
            Self::Truncated => formatter.write_str("control-flow contract is truncated"),
            Self::InvalidMagic => formatter.write_str("control-flow contract magic is invalid"),
            Self::UnknownVersion(version) => {
                write!(
                    formatter,
                    "unsupported control-flow contract version {version}"
                )
            }
            Self::UnsupportedFlags(flags) => {
                write!(formatter, "unsupported control-flow flags {flags:#x}")
            }
            Self::InvalidLength(length) => {
                write!(formatter, "invalid control-flow contract length {length}")
            }
            Self::TrailingBytes => formatter.write_str("control-flow contract has trailing bytes"),
            Self::NonzeroReserved(field) => write!(formatter, "{field} reserved field is nonzero"),
            Self::CountOutOfRange { field, count, max } => {
                write!(formatter, "{field} count {count} exceeds {max}")
            }
            Self::InvalidUtf8 => formatter.write_str("source file is not valid UTF-8"),
            Self::UnknownTag { kind, tag } => write!(formatter, "unknown {kind} tag {tag}"),
            Self::NonCanonical => formatter.write_str("control-flow contract is not canonical"),
            Self::Validation(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ControlFlowDecodeErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Validation(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ControlFlowValidationErrorV1> for ControlFlowDecodeErrorV1 {
    fn from(value: ControlFlowValidationErrorV1) -> Self {
        Self::Validation(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct ControlFlowNodeIdV1(u32);

impl ControlFlowNodeIdV1 {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct FrontendSourceSpanV1 {
    file: String,
    start_line: u32,
    start_column: u32,
    end_line: u32,
    end_column: u32,
}

impl FrontendSourceSpanV1 {
    pub fn new(
        file: impl Into<String>,
        start_line: u32,
        start_column: u32,
        end_line: u32,
        end_column: u32,
    ) -> Result<Self, ControlFlowValidationErrorV1> {
        let file = file.into();
        if file.is_empty() || file.len() > MAX_SOURCE_FILE_BYTES_V1 || file.as_bytes().contains(&0)
        {
            return Err(ControlFlowValidationErrorV1::InvalidSourceFile);
        }
        if start_line == 0
            || start_column == 0
            || end_line == 0
            || end_column == 0
            || (end_line, end_column) < (start_line, start_column)
        {
            return Err(ControlFlowValidationErrorV1::InvalidSourceSpan);
        }
        Ok(Self {
            file,
            start_line,
            start_column,
            end_line,
            end_column,
        })
    }

    pub fn file(&self) -> &str {
        &self.file
    }

    pub const fn start(&self) -> (u32, u32) {
        (self.start_line, self.start_column)
    }

    pub const fn end(&self) -> (u32, u32) {
        (self.end_line, self.end_column)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FrontendIntegerSwitchTypeV1 {
    width: u16,
    signed: bool,
}

impl FrontendIntegerSwitchTypeV1 {
    pub fn new(width: u16, signed: bool) -> Result<Self, ControlFlowValidationErrorV1> {
        if !matches!(width, 8 | 16 | 32 | 64 | 128) {
            return Err(ControlFlowValidationErrorV1::UnsupportedIntegerWidth(width));
        }
        Ok(Self { width, signed })
    }

    pub const fn width(self) -> u16 {
        self.width
    }

    pub const fn is_signed(self) -> bool {
        self.signed
    }

    fn accepts_bits(self, bits: u128) -> bool {
        if self.signed {
            if self.width == 128 {
                return true;
            }
            let mask = (1_u128 << self.width) - 1;
            let truncated = bits & mask;
            let sign_bit = 1_u128 << (self.width - 1);
            let canonical = if truncated & sign_bit == 0 {
                truncated
            } else {
                truncated | !mask
            };
            bits == canonical
        } else {
            self.width == 128 || bits < (1_u128 << self.width)
        }
    }

    fn compare_bits(self, left: u128, right: u128) -> std::cmp::Ordering {
        if self.signed {
            (left as i128).cmp(&(right as i128))
        } else {
            left.cmp(&right)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FrontendIntegerSwitchCaseV1 {
    bits: u128,
    target: ControlFlowNodeIdV1,
}

impl FrontendIntegerSwitchCaseV1 {
    pub const fn from_bits(bits: u128, target: ControlFlowNodeIdV1) -> Self {
        Self { bits, target }
    }

    pub const fn from_signed(value: i128, target: ControlFlowNodeIdV1) -> Self {
        Self::from_bits(value as u128, target)
    }

    pub const fn from_unsigned(value: u128, target: ControlFlowNodeIdV1) -> Self {
        Self::from_bits(value, target)
    }

    pub const fn bits(self) -> u128 {
        self.bits
    }

    pub const fn target(self) -> ControlFlowNodeIdV1 {
        self.target
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ControlFlowNodeKindV1 {
    Entry {
        target: ControlFlowNodeIdV1,
    },
    Block {
        target: ControlFlowNodeIdV1,
    },
    Exit,
    Branch {
        then_target: ControlFlowNodeIdV1,
        else_target: ControlFlowNodeIdV1,
    },
    Loop {
        max_iterations: u32,
        body: ControlFlowNodeIdV1,
        exit: ControlFlowNodeIdV1,
    },
    Break {
        loop_header: ControlFlowNodeIdV1,
        target: ControlFlowNodeIdV1,
    },
    Continue {
        loop_header: ControlFlowNodeIdV1,
        target: ControlFlowNodeIdV1,
    },
    IntegerSwitch {
        ty: FrontendIntegerSwitchTypeV1,
        cases: Vec<FrontendIntegerSwitchCaseV1>,
        default: ControlFlowNodeIdV1,
    },
}

impl ControlFlowNodeKindV1 {
    pub fn integer_switch(
        ty: FrontendIntegerSwitchTypeV1,
        mut cases: Vec<FrontendIntegerSwitchCaseV1>,
        default: ControlFlowNodeIdV1,
    ) -> Result<Self, ControlFlowValidationErrorV1> {
        if cases.len() > MAX_INTEGER_SWITCH_CASES_V1 {
            return Err(ControlFlowValidationErrorV1::TooMany {
                field: "integer switch cases",
                max: MAX_INTEGER_SWITCH_CASES_V1,
            });
        }
        cases.sort_unstable_by(|left, right| ty.compare_bits(left.bits, right.bits));
        Ok(Self::IntegerSwitch { ty, cases, default })
    }

    fn successors(&self) -> Vec<ControlFlowNodeIdV1> {
        match self {
            Self::Entry { target } | Self::Block { target } => vec![*target],
            Self::Exit => Vec::new(),
            Self::Branch {
                then_target,
                else_target,
            } => vec![*then_target, *else_target],
            Self::Loop { body, exit, .. } => vec![*body, *exit],
            Self::Break { target, .. } | Self::Continue { target, .. } => vec![*target],
            Self::IntegerSwitch { cases, default, .. } => {
                let mut successors = cases.iter().map(|case| case.target).collect::<Vec<_>>();
                successors.push(*default);
                successors
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlFlowNodeV1 {
    id: ControlFlowNodeIdV1,
    span: FrontendSourceSpanV1,
    kind: ControlFlowNodeKindV1,
}

impl ControlFlowNodeV1 {
    pub const fn new(
        id: ControlFlowNodeIdV1,
        span: FrontendSourceSpanV1,
        kind: ControlFlowNodeKindV1,
    ) -> Self {
        Self { id, span, kind }
    }

    pub const fn id(&self) -> ControlFlowNodeIdV1 {
        self.id
    }

    pub const fn span(&self) -> &FrontendSourceSpanV1 {
        &self.span
    }

    pub const fn kind(&self) -> &ControlFlowNodeKindV1 {
        &self.kind
    }
}

/// Exact canonical bytes for the source-span-independent CFG projection.
///
/// This is a collision-free structural identity, not a cryptographic digest.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CanonicalCfgIdentityV1(Vec<u8>);

impl CanonicalCfgIdentityV1 {
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlFlowContractV1 {
    entry: ControlFlowNodeIdV1,
    nodes: Vec<ControlFlowNodeV1>,
}

impl ControlFlowContractV1 {
    pub fn new(
        entry: ControlFlowNodeIdV1,
        mut nodes: Vec<ControlFlowNodeV1>,
    ) -> Result<Self, ControlFlowValidationErrorV1> {
        if nodes.is_empty() {
            return Err(ControlFlowValidationErrorV1::EmptyGraph);
        }
        if nodes.len() > MAX_CONTROL_FLOW_NODES_V1 {
            return Err(ControlFlowValidationErrorV1::TooMany {
                field: "control-flow nodes",
                max: MAX_CONTROL_FLOW_NODES_V1,
            });
        }
        nodes.sort_unstable_by_key(ControlFlowNodeV1::id);
        for (expected, node) in nodes.iter().enumerate() {
            let expected =
                u32::try_from(expected).map_err(|_| ControlFlowValidationErrorV1::Overflow {
                    field: "control-flow node ID",
                })?;
            if node.id().get() != expected {
                return Err(ControlFlowValidationErrorV1::NonDenseNodeId {
                    expected,
                    actual: node.id().get(),
                });
            }
        }
        let contract = Self { entry, nodes };
        contract.validate()?;
        if encode_control_flow_contract_v1(&contract)?.len() > MAX_CONTROL_FLOW_CONTRACT_BYTES_V1 {
            return Err(ControlFlowValidationErrorV1::EncodedContractTooLarge);
        }
        Ok(contract)
    }

    pub const fn entry(&self) -> ControlFlowNodeIdV1 {
        self.entry
    }

    pub fn nodes(&self) -> &[ControlFlowNodeV1] {
        &self.nodes
    }

    pub fn cfg_identity(&self) -> CanonicalCfgIdentityV1 {
        let mut writer = Writer::new();
        writer.bytes(CFG_IDENTITY_DOMAIN_V1);
        writer.u32(self.entry.get());
        writer.u32(u32::try_from(self.nodes.len()).expect("validated node count fits u32"));
        for node in &self.nodes {
            writer.u32(node.id().get());
            encode_kind(&mut writer, node.kind()).expect("validated node payload fits wire fields");
        }
        CanonicalCfgIdentityV1(writer.finish())
    }

    fn validate(&self) -> Result<(), ControlFlowValidationErrorV1> {
        let node_count = self.nodes.len();
        let entry_index = usize::try_from(self.entry.get())
            .map_err(|_| ControlFlowValidationErrorV1::InvalidEntryNode(self.entry.get()))?;
        if entry_index >= node_count {
            return Err(ControlFlowValidationErrorV1::InvalidEntryNode(
                self.entry.get(),
            ));
        }
        if !matches!(
            self.nodes[entry_index].kind(),
            ControlFlowNodeKindV1::Entry { .. }
        ) {
            return Err(ControlFlowValidationErrorV1::EntryKindRequired(
                self.entry.get(),
            ));
        }

        let mut edges = Vec::with_capacity(node_count);
        let mut edge_count = 0_usize;
        let mut has_exit = false;
        for node in &self.nodes {
            if node.id() != self.entry && matches!(node.kind(), ControlFlowNodeKindV1::Entry { .. })
            {
                return Err(ControlFlowValidationErrorV1::DuplicateEntryNode(
                    node.id().get(),
                ));
            }
            has_exit |= matches!(node.kind(), ControlFlowNodeKindV1::Exit);
            validate_node_payload(node, &self.nodes)?;
            let successors = node.kind().successors();
            edge_count = edge_count.checked_add(successors.len()).ok_or(
                ControlFlowValidationErrorV1::Overflow {
                    field: "control-flow edge count",
                },
            )?;
            if edge_count > MAX_CONTROL_FLOW_EDGES_V1 {
                return Err(ControlFlowValidationErrorV1::TooMany {
                    field: "control-flow edges",
                    max: MAX_CONTROL_FLOW_EDGES_V1,
                });
            }
            for successor in &successors {
                if usize::try_from(successor.get()).map_or(true, |index| index >= node_count) {
                    return Err(ControlFlowValidationErrorV1::InvalidSuccessor {
                        node: node.id().get(),
                        successor: successor.get(),
                    });
                }
            }
            edges.push(successors);
        }
        if !has_exit {
            return Err(ControlFlowValidationErrorV1::MissingExit);
        }

        let reachable = reachable_nodes(entry_index, &edges);
        if let Some(index) = reachable.iter().position(|reachable| !reachable) {
            return Err(ControlFlowValidationErrorV1::UnreachableNode(
                u32::try_from(index).expect("validated node count fits u32"),
            ));
        }
        validate_reducible(self, &edges, entry_index)
    }
}

fn validate_node_payload(
    node: &ControlFlowNodeV1,
    nodes: &[ControlFlowNodeV1],
) -> Result<(), ControlFlowValidationErrorV1> {
    match node.kind() {
        ControlFlowNodeKindV1::Loop { max_iterations, .. } if *max_iterations == 0 => {
            Err(ControlFlowValidationErrorV1::ZeroLoopBound(node.id().get()))
        }
        ControlFlowNodeKindV1::Break {
            loop_header,
            target,
        } => {
            let loop_node = nodes.get(loop_header.get() as usize).ok_or(
                ControlFlowValidationErrorV1::InvalidLoopTransfer {
                    node: node.id().get(),
                    loop_header: loop_header.get(),
                },
            )?;
            let ControlFlowNodeKindV1::Loop { exit, .. } = loop_node.kind() else {
                return Err(ControlFlowValidationErrorV1::InvalidLoopTransfer {
                    node: node.id().get(),
                    loop_header: loop_header.get(),
                });
            };
            if target != exit {
                return Err(ControlFlowValidationErrorV1::BreakTargetMismatch {
                    node: node.id().get(),
                });
            }
            Ok(())
        }
        ControlFlowNodeKindV1::Continue {
            loop_header,
            target,
        } => {
            if !matches!(
                nodes
                    .get(loop_header.get() as usize)
                    .map(ControlFlowNodeV1::kind),
                Some(ControlFlowNodeKindV1::Loop { .. })
            ) {
                return Err(ControlFlowValidationErrorV1::InvalidLoopTransfer {
                    node: node.id().get(),
                    loop_header: loop_header.get(),
                });
            }
            if target != loop_header {
                return Err(ControlFlowValidationErrorV1::ContinueTargetMismatch {
                    node: node.id().get(),
                });
            }
            Ok(())
        }
        ControlFlowNodeKindV1::IntegerSwitch { ty, cases, .. } => {
            if cases.len() > MAX_INTEGER_SWITCH_CASES_V1 {
                return Err(ControlFlowValidationErrorV1::TooMany {
                    field: "integer switch cases",
                    max: MAX_INTEGER_SWITCH_CASES_V1,
                });
            }
            for case in cases {
                if !ty.accepts_bits(case.bits()) {
                    return Err(ControlFlowValidationErrorV1::IntegerCaseOutOfRange {
                        node: node.id().get(),
                        bits: case.bits(),
                    });
                }
            }
            if let Some(pair) = cases
                .windows(2)
                .find(|pair| ty.compare_bits(pair[0].bits(), pair[1].bits()).is_ge())
            {
                return Err(ControlFlowValidationErrorV1::DuplicateIntegerCase {
                    node: node.id().get(),
                    bits: pair[1].bits(),
                });
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn reachable_nodes(entry: usize, edges: &[Vec<ControlFlowNodeIdV1>]) -> Vec<bool> {
    let mut reachable = vec![false; edges.len()];
    let mut queue = VecDeque::from([entry]);
    while let Some(node) = queue.pop_front() {
        if reachable[node] {
            continue;
        }
        reachable[node] = true;
        queue.extend(edges[node].iter().map(|successor| successor.get() as usize));
    }
    reachable
}

fn validate_reducible(
    contract: &ControlFlowContractV1,
    edges: &[Vec<ControlFlowNodeIdV1>],
    entry: usize,
) -> Result<(), ControlFlowValidationErrorV1> {
    let count = edges.len();
    let mut predecessors = vec![Vec::new(); count];
    for (source, successors) in edges.iter().enumerate() {
        for successor in successors {
            predecessors[successor.get() as usize].push(source);
        }
    }

    let mut dominates = vec![vec![true; count]; count];
    dominates[entry].fill(false);
    dominates[entry][entry] = true;
    loop {
        let mut changed = false;
        for node in 0..count {
            if node == entry {
                continue;
            }
            let mut next = vec![true; count];
            for predecessor in &predecessors[node] {
                for (value, predecessor_value) in next.iter_mut().zip(&dominates[*predecessor]) {
                    *value &= *predecessor_value;
                }
            }
            next[node] = true;
            if next != dominates[node] {
                dominates[node] = next;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    let mut loop_has_backedge = vec![false; count];
    let mut acyclic_indegree = vec![0_usize; count];
    for (source, successors) in edges.iter().enumerate() {
        for successor in successors {
            let target = successor.get() as usize;
            if dominates[source][target] {
                if !matches!(
                    contract.nodes[target].kind(),
                    ControlFlowNodeKindV1::Loop { .. }
                ) {
                    return Err(ControlFlowValidationErrorV1::IrreducibleControlFlow);
                }
                loop_has_backedge[target] = true;
            } else {
                acyclic_indegree[target] += 1;
            }
        }
    }
    for (index, node) in contract.nodes.iter().enumerate() {
        if matches!(node.kind(), ControlFlowNodeKindV1::Loop { .. }) && !loop_has_backedge[index] {
            return Err(ControlFlowValidationErrorV1::LoopHasNoBackedge(
                node.id().get(),
            ));
        }
    }

    let mut queue = acyclic_indegree
        .iter()
        .enumerate()
        .filter_map(|(index, degree)| (*degree == 0).then_some(index))
        .collect::<VecDeque<_>>();
    let mut visited = 0_usize;
    while let Some(source) = queue.pop_front() {
        visited += 1;
        for successor in &edges[source] {
            let target = successor.get() as usize;
            if dominates[source][target] {
                continue;
            }
            acyclic_indegree[target] -= 1;
            if acyclic_indegree[target] == 0 {
                queue.push_back(target);
            }
        }
    }
    if visited != count {
        return Err(ControlFlowValidationErrorV1::IrreducibleControlFlow);
    }
    Ok(())
}

pub fn encode_control_flow_contract_v1(
    contract: &ControlFlowContractV1,
) -> Result<Vec<u8>, ControlFlowValidationErrorV1> {
    let mut writer = Writer::new();
    writer.bytes(&CONTROL_FLOW_CONTRACT_MAGIC_V1);
    writer.u16(CONTROL_FLOW_CONTRACT_VERSION_V1);
    writer.u16(0);
    writer.u32(0);
    writer.u32(u32::try_from(contract.nodes.len()).map_err(|_| {
        ControlFlowValidationErrorV1::Overflow {
            field: "control-flow node count",
        }
    })?);
    writer.u32(contract.entry.get());
    writer.u32(0);
    for node in &contract.nodes {
        writer.u32(node.id().get());
        encode_span(&mut writer, node.span())?;
        encode_kind(&mut writer, node.kind())?;
    }
    let mut bytes = writer.finish();
    if bytes.len() > MAX_CONTROL_FLOW_CONTRACT_BYTES_V1 {
        return Err(ControlFlowValidationErrorV1::EncodedContractTooLarge);
    }
    let length =
        u32::try_from(bytes.len()).map_err(|_| ControlFlowValidationErrorV1::Overflow {
            field: "control-flow contract length",
        })?;
    bytes[12..16].copy_from_slice(&length.to_le_bytes());
    Ok(bytes)
}

fn encode_span(
    writer: &mut Writer,
    span: &FrontendSourceSpanV1,
) -> Result<(), ControlFlowValidationErrorV1> {
    writer.u16(u16::try_from(span.file.len()).map_err(|_| {
        ControlFlowValidationErrorV1::Overflow {
            field: "source file length",
        }
    })?);
    writer.u16(0);
    writer.bytes(span.file.as_bytes());
    writer.u32(span.start_line);
    writer.u32(span.start_column);
    writer.u32(span.end_line);
    writer.u32(span.end_column);
    Ok(())
}

fn encode_kind(
    writer: &mut Writer,
    kind: &ControlFlowNodeKindV1,
) -> Result<(), ControlFlowValidationErrorV1> {
    match kind {
        ControlFlowNodeKindV1::Entry { target } => {
            writer.u16(1);
            writer.u16(0);
            writer.u32(target.get());
        }
        ControlFlowNodeKindV1::Block { target } => {
            writer.u16(2);
            writer.u16(0);
            writer.u32(target.get());
        }
        ControlFlowNodeKindV1::Exit => {
            writer.u16(3);
            writer.u16(0);
        }
        ControlFlowNodeKindV1::Branch {
            then_target,
            else_target,
        } => {
            writer.u16(4);
            writer.u16(0);
            writer.u32(then_target.get());
            writer.u32(else_target.get());
        }
        ControlFlowNodeKindV1::Loop {
            max_iterations,
            body,
            exit,
        } => {
            writer.u16(5);
            writer.u16(0);
            writer.u32(*max_iterations);
            writer.u32(body.get());
            writer.u32(exit.get());
        }
        ControlFlowNodeKindV1::Break {
            loop_header,
            target,
        } => {
            writer.u16(6);
            writer.u16(0);
            writer.u32(loop_header.get());
            writer.u32(target.get());
        }
        ControlFlowNodeKindV1::Continue {
            loop_header,
            target,
        } => {
            writer.u16(7);
            writer.u16(0);
            writer.u32(loop_header.get());
            writer.u32(target.get());
        }
        ControlFlowNodeKindV1::IntegerSwitch { ty, cases, default } => {
            writer.u16(8);
            writer.u16(0);
            writer.u16(ty.width());
            writer.u8(u8::from(ty.is_signed()));
            writer.u8(0);
            writer.u16(u16::try_from(cases.len()).map_err(|_| {
                ControlFlowValidationErrorV1::Overflow {
                    field: "integer switch case count",
                }
            })?);
            writer.u16(0);
            writer.u32(default.get());
            for case in cases {
                writer.u128(case.bits());
                writer.u32(case.target().get());
            }
        }
    }
    Ok(())
}

pub fn decode_control_flow_contract_v1(
    bytes: &[u8],
) -> Result<ControlFlowContractV1, ControlFlowDecodeErrorV1> {
    if bytes.len() > MAX_CONTROL_FLOW_CONTRACT_BYTES_V1 {
        return Err(ControlFlowDecodeErrorV1::TooLarge);
    }
    let mut reader = Reader::new(bytes);
    if reader.fixed::<8>()? != CONTROL_FLOW_CONTRACT_MAGIC_V1 {
        return Err(ControlFlowDecodeErrorV1::InvalidMagic);
    }
    let version = reader.u16()?;
    if version != CONTROL_FLOW_CONTRACT_VERSION_V1 {
        return Err(ControlFlowDecodeErrorV1::UnknownVersion(version));
    }
    let flags = reader.u16()?;
    if flags != 0 {
        return Err(ControlFlowDecodeErrorV1::UnsupportedFlags(flags));
    }
    let declared = reader.u32()?;
    if declared < HEADER_BYTES_V1 as u32 {
        return Err(ControlFlowDecodeErrorV1::InvalidLength(declared));
    }
    let declared =
        usize::try_from(declared).map_err(|_| ControlFlowDecodeErrorV1::InvalidLength(declared))?;
    if declared > bytes.len() {
        return Err(ControlFlowDecodeErrorV1::Truncated);
    }
    if declared < bytes.len() {
        return Err(ControlFlowDecodeErrorV1::TrailingBytes);
    }
    let node_count = reader.count_u32("control-flow nodes", MAX_CONTROL_FLOW_NODES_V1)?;
    let entry = ControlFlowNodeIdV1::new(reader.u32()?);
    reader.reserved_u32("control-flow header")?;
    let mut nodes = Vec::with_capacity(node_count);
    for _ in 0..node_count {
        let id = ControlFlowNodeIdV1::new(reader.u32()?);
        let span = decode_span(&mut reader)?;
        let kind = decode_kind(&mut reader)?;
        nodes.push(ControlFlowNodeV1::new(id, span, kind));
    }
    if !reader.is_finished() {
        return Err(ControlFlowDecodeErrorV1::TrailingBytes);
    }
    let contract = ControlFlowContractV1::new(entry, nodes)?;
    if encode_control_flow_contract_v1(&contract)? != bytes {
        return Err(ControlFlowDecodeErrorV1::NonCanonical);
    }
    Ok(contract)
}

fn decode_span(reader: &mut Reader<'_>) -> Result<FrontendSourceSpanV1, ControlFlowDecodeErrorV1> {
    let file_length = reader.count_u16("source file bytes", MAX_SOURCE_FILE_BYTES_V1)?;
    reader.reserved_u16("source span")?;
    let file = std::str::from_utf8(reader.take(file_length)?)
        .map_err(|_| ControlFlowDecodeErrorV1::InvalidUtf8)?;
    Ok(FrontendSourceSpanV1::new(
        file,
        reader.u32()?,
        reader.u32()?,
        reader.u32()?,
        reader.u32()?,
    )?)
}

fn decode_kind(reader: &mut Reader<'_>) -> Result<ControlFlowNodeKindV1, ControlFlowDecodeErrorV1> {
    let tag = reader.u16()?;
    reader.reserved_u16("control-flow node")?;
    Ok(match tag {
        1 => ControlFlowNodeKindV1::Entry {
            target: ControlFlowNodeIdV1::new(reader.u32()?),
        },
        2 => ControlFlowNodeKindV1::Block {
            target: ControlFlowNodeIdV1::new(reader.u32()?),
        },
        3 => ControlFlowNodeKindV1::Exit,
        4 => ControlFlowNodeKindV1::Branch {
            then_target: ControlFlowNodeIdV1::new(reader.u32()?),
            else_target: ControlFlowNodeIdV1::new(reader.u32()?),
        },
        5 => ControlFlowNodeKindV1::Loop {
            max_iterations: reader.u32()?,
            body: ControlFlowNodeIdV1::new(reader.u32()?),
            exit: ControlFlowNodeIdV1::new(reader.u32()?),
        },
        6 => ControlFlowNodeKindV1::Break {
            loop_header: ControlFlowNodeIdV1::new(reader.u32()?),
            target: ControlFlowNodeIdV1::new(reader.u32()?),
        },
        7 => ControlFlowNodeKindV1::Continue {
            loop_header: ControlFlowNodeIdV1::new(reader.u32()?),
            target: ControlFlowNodeIdV1::new(reader.u32()?),
        },
        8 => {
            let ty = FrontendIntegerSwitchTypeV1::new(reader.u16()?, reader.u8()? != 0)?;
            reader.reserved_u8("integer switch")?;
            let case_count =
                reader.count_u16("integer switch cases", MAX_INTEGER_SWITCH_CASES_V1)?;
            reader.reserved_u16("integer switch")?;
            let default = ControlFlowNodeIdV1::new(reader.u32()?);
            let mut cases = Vec::with_capacity(case_count);
            for _ in 0..case_count {
                cases.push(FrontendIntegerSwitchCaseV1::from_bits(
                    reader.u128()?,
                    ControlFlowNodeIdV1::new(reader.u32()?),
                ));
            }
            ControlFlowNodeKindV1::integer_switch(ty, cases, default)?
        }
        tag => {
            return Err(ControlFlowDecodeErrorV1::UnknownTag {
                kind: "control-flow node",
                tag,
            });
        }
    })
}

struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u128(&mut self, value: u128) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn bytes(&mut self, value: &[u8]) {
        self.bytes.extend_from_slice(value);
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], ControlFlowDecodeErrorV1> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(ControlFlowDecodeErrorV1::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(ControlFlowDecodeErrorV1::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], ControlFlowDecodeErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| ControlFlowDecodeErrorV1::Truncated)
    }

    fn u8(&mut self) -> Result<u8, ControlFlowDecodeErrorV1> {
        Ok(self.fixed::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, ControlFlowDecodeErrorV1> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, ControlFlowDecodeErrorV1> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn u128(&mut self) -> Result<u128, ControlFlowDecodeErrorV1> {
        Ok(u128::from_le_bytes(self.fixed()?))
    }

    fn reserved_u8(&mut self, field: &'static str) -> Result<(), ControlFlowDecodeErrorV1> {
        if self.u8()? != 0 {
            return Err(ControlFlowDecodeErrorV1::NonzeroReserved(field));
        }
        Ok(())
    }

    fn reserved_u16(&mut self, field: &'static str) -> Result<(), ControlFlowDecodeErrorV1> {
        if self.u16()? != 0 {
            return Err(ControlFlowDecodeErrorV1::NonzeroReserved(field));
        }
        Ok(())
    }

    fn reserved_u32(&mut self, field: &'static str) -> Result<(), ControlFlowDecodeErrorV1> {
        if self.u32()? != 0 {
            return Err(ControlFlowDecodeErrorV1::NonzeroReserved(field));
        }
        Ok(())
    }

    fn count_u16(
        &mut self,
        field: &'static str,
        max: usize,
    ) -> Result<usize, ControlFlowDecodeErrorV1> {
        let count = usize::from(self.u16()?);
        if count > max {
            return Err(ControlFlowDecodeErrorV1::CountOutOfRange {
                field,
                count: count as u64,
                max,
            });
        }
        Ok(count)
    }

    fn count_u32(
        &mut self,
        field: &'static str,
        max: usize,
    ) -> Result<usize, ControlFlowDecodeErrorV1> {
        let raw = self.u32()?;
        let count =
            usize::try_from(raw).map_err(|_| ControlFlowDecodeErrorV1::CountOutOfRange {
                field,
                count: u64::from(raw),
                max,
            })?;
        if count > max {
            return Err(ControlFlowDecodeErrorV1::CountOutOfRange {
                field,
                count: u64::from(raw),
                max,
            });
        }
        Ok(count)
    }

    fn is_finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}
