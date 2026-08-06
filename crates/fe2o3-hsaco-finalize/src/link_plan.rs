use std::{collections::BTreeMap, fmt};

use fe2o3_hsaco::MAX_HSACO_BYTES;
use fe2o3_kernel_descriptor::DeviceTargetV1;
use sha2::{Digest, Sha256};

/// Maximum relocatable objects accepted by one link plan.
pub const MAX_LINK_INPUTS: usize = 128;
/// Maximum independently declared linker options.
pub const MAX_LINK_OPTIONS: usize = 64;
/// Maximum provenance nodes, including the inputs and output.
pub const MAX_LINK_PROVENANCE_NODES: usize = 1024;
/// Maximum parent edges summed across the provenance graph.
pub const MAX_LINK_PROVENANCE_EDGES: usize = 4096;
/// Maximum bytes in one canonical option name.
pub const MAX_LINK_OPTION_NAME_BYTES: usize = 64;
/// Maximum bytes in one canonical option value.
pub const MAX_LINK_OPTION_VALUE_BYTES: usize = 256;

const LINK_PLAN_DOMAIN_V1: &[u8] = b"FE2O3/AMDGPU-MULTI-INPUT-LINK-PLAN/V1\0";

/// A SHA-256 content identity paired with the exact byte length it names.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContentIdentityV1 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl ContentIdentityV1 {
    /// Calculates an identity without retaining the bytes.
    pub fn calculate(bytes: &[u8]) -> Self {
        Self {
            sha256: Sha256::digest(bytes).into(),
            byte_len: bytes.len() as u64,
        }
    }

    /// Constructs a declared identity. A plan validates its size and conflicts.
    pub const fn from_parts(sha256: [u8; 32], byte_len: u64) -> Self {
        Self { sha256, byte_len }
    }

    pub const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    /// Checks bytes against both the digest and length.
    pub fn matches(self, bytes: &[u8]) -> bool {
        self.byte_len == bytes.len() as u64
            && self.sha256 == <[u8; 32]>::from(Sha256::digest(bytes))
    }
}

/// One AMDGPU relocatable input to the final native link.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LinkInputV1 {
    identity: ContentIdentityV1,
    target: DeviceTargetV1,
}

impl LinkInputV1 {
    pub const fn new(identity: ContentIdentityV1, target: DeviceTargetV1) -> Self {
        Self { identity, target }
    }

    pub const fn identity(self) -> ContentIdentityV1 {
        self.identity
    }

    pub const fn target(self) -> DeviceTargetV1 {
        self.target
    }
}

/// The expected executable HSACO output of the native link.
///
/// Requiring a content identity makes a plan suitable for reproducible or
/// cached builds. First-time builds can create the plan after recording a
/// separately produced candidate, then independently verify that candidate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LinkOutputV1 {
    identity: ContentIdentityV1,
    target: DeviceTargetV1,
}

impl LinkOutputV1 {
    pub const fn new(identity: ContentIdentityV1, target: DeviceTargetV1) -> Self {
        Self { identity, target }
    }

    pub const fn identity(self) -> ContentIdentityV1 {
        self.identity
    }

    pub const fn target(self) -> DeviceTargetV1 {
        self.target
    }
}

/// One non-repeating, canonical linker option.
///
/// This is data, not a shell argument. An execution adapter must map supported
/// names to the direct LLVM/LLD worker API and reject unknown names.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct LinkOptionV1 {
    name: String,
    value: String,
}

impl LinkOptionV1 {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Result<Self, LinkPlanError> {
        let name = name.into();
        let value = value.into();
        validate_option_name(&name)?;
        validate_option_value(&value)?;
        Ok(Self { name, value })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

/// One node in a complete output-to-source provenance DAG.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProvenanceNodeV1 {
    identity: ContentIdentityV1,
    parents: Vec<ContentIdentityV1>,
}

impl ProvenanceNodeV1 {
    pub fn new(
        identity: ContentIdentityV1,
        parents: Vec<ContentIdentityV1>,
    ) -> Result<Self, LinkPlanError> {
        if parents.len() > MAX_LINK_PROVENANCE_EDGES {
            return Err(LinkPlanError::TooManyProvenanceEdges);
        }
        Ok(Self { identity, parents })
    }

    pub const fn identity(&self) -> ContentIdentityV1 {
        self.identity
    }

    pub fn parents(&self) -> &[ContentIdentityV1] {
        &self.parents
    }
}

/// Stable digest of the canonical plan description, not of linked output bytes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LinkPlanIdentityV1([u8; 32]);

impl LinkPlanIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// A bounded deterministic description of a multi-object AMDGPU native link.
///
/// All collections are required to be canonical on construction. The
/// `canonicalized` constructor is provided for callers that start with sets.
/// This value is descriptive and grants no compiler, linker, load, or launch
/// authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultiInputLinkPlanV1 {
    target: DeviceTargetV1,
    inputs: Vec<LinkInputV1>,
    options: Vec<LinkOptionV1>,
    output: LinkOutputV1,
    provenance: Vec<ProvenanceNodeV1>,
    identity: LinkPlanIdentityV1,
}

impl MultiInputLinkPlanV1 {
    pub fn new(
        target: DeviceTargetV1,
        inputs: Vec<LinkInputV1>,
        options: Vec<LinkOptionV1>,
        output: LinkOutputV1,
        provenance: Vec<ProvenanceNodeV1>,
    ) -> Result<Self, LinkPlanError> {
        validate_parts(target, &inputs, &options, output, &provenance)?;
        let canonical = encode_canonical(target, &inputs, &options, output, &provenance);
        let identity = LinkPlanIdentityV1(Sha256::digest(canonical).into());
        Ok(Self {
            target,
            inputs,
            options,
            output,
            provenance,
            identity,
        })
    }

    /// Sorts set-like fields into their V1 order before full validation.
    pub fn canonicalized(
        target: DeviceTargetV1,
        mut inputs: Vec<LinkInputV1>,
        mut options: Vec<LinkOptionV1>,
        output: LinkOutputV1,
        mut provenance: Vec<ProvenanceNodeV1>,
    ) -> Result<Self, LinkPlanError> {
        inputs.sort_by_key(|input| input.identity);
        options.sort();
        for node in &mut provenance {
            node.parents.sort_unstable();
        }
        provenance.sort_by_key(|node| node.identity);
        Self::new(target, inputs, options, output, provenance)
    }

    pub const fn target(&self) -> DeviceTargetV1 {
        self.target
    }

    pub fn inputs(&self) -> &[LinkInputV1] {
        &self.inputs
    }

    pub fn options(&self) -> &[LinkOptionV1] {
        &self.options
    }

    pub const fn output(&self) -> LinkOutputV1 {
        self.output
    }

    pub fn provenance(&self) -> &[ProvenanceNodeV1] {
        &self.provenance
    }

    pub const fn identity(&self) -> LinkPlanIdentityV1 {
        self.identity
    }

    pub fn canonical_bytes(&self) -> Vec<u8> {
        encode_canonical(
            self.target,
            &self.inputs,
            &self.options,
            self.output,
            &self.provenance,
        )
    }

    pub fn verify_output_bytes(&self, bytes: &[u8]) -> Result<(), LinkPlanError> {
        if self.output.identity.matches(bytes) {
            Ok(())
        } else {
            Err(LinkPlanError::OutputIdentityMismatch)
        }
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum LinkPlanError {
    NoInputs,
    TooManyInputs,
    TooManyOptions,
    TooManyProvenanceNodes,
    TooManyProvenanceEdges,
    EmptyContent,
    ContentTooLarge,
    NonCanonicalOrder(&'static str),
    DuplicateInput(ContentIdentityV1),
    ConflictingContentLength([u8; 32]),
    InputTargetMismatch(ContentIdentityV1),
    OutputTargetMismatch,
    OutputAliasesInput,
    EmptyOptionName,
    OptionNameTooLong,
    InvalidOptionName,
    OptionValueTooLong,
    InvalidOptionValue,
    DuplicateOption(String),
    ConflictingOption(String),
    DuplicateProvenanceNode(ContentIdentityV1),
    MissingProvenanceNode(ContentIdentityV1),
    UnknownProvenanceParent(ContentIdentityV1),
    DuplicateProvenanceParent(ContentIdentityV1),
    OutputParentsMismatch,
    ProvenanceCycle(ContentIdentityV1),
    OrphanProvenanceNode(ContentIdentityV1),
    OutputIdentityMismatch,
}

impl fmt::Display for LinkPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoInputs => formatter.write_str("a link plan requires at least one input"),
            Self::TooManyInputs => formatter.write_str("link input count exceeds the V1 bound"),
            Self::TooManyOptions => formatter.write_str("link option count exceeds the V1 bound"),
            Self::TooManyProvenanceNodes => {
                formatter.write_str("provenance node count exceeds the V1 bound")
            }
            Self::TooManyProvenanceEdges => {
                formatter.write_str("provenance edge count exceeds the V1 bound")
            }
            Self::EmptyContent => {
                formatter.write_str("content identities must not name empty data")
            }
            Self::ContentTooLarge => {
                formatter.write_str("content identity exceeds the HSACO size bound")
            }
            Self::NonCanonicalOrder(field) => {
                write!(formatter, "{field} are not in canonical order")
            }
            Self::DuplicateInput(identity) => write!(formatter, "duplicate link input {identity}"),
            Self::ConflictingContentLength(digest) => write!(
                formatter,
                "one SHA-256 digest has conflicting lengths: {}",
                HexDigest(digest)
            ),
            Self::InputTargetMismatch(identity) => write!(
                formatter,
                "link input {identity} declares a different target"
            ),
            Self::OutputTargetMismatch => {
                formatter.write_str("link output declares a different target")
            }
            Self::OutputAliasesInput => {
                formatter.write_str("link output identity aliases an input identity")
            }
            Self::EmptyOptionName => formatter.write_str("link option name must not be empty"),
            Self::OptionNameTooLong => formatter.write_str("link option name exceeds the V1 bound"),
            Self::InvalidOptionName => {
                formatter.write_str("link option name is not canonical ASCII")
            }
            Self::OptionValueTooLong => {
                formatter.write_str("link option value exceeds the V1 bound")
            }
            Self::InvalidOptionValue => {
                formatter.write_str("link option value contains noncanonical text")
            }
            Self::DuplicateOption(name) => write!(formatter, "duplicate link option {name}"),
            Self::ConflictingOption(name) => {
                write!(formatter, "conflicting values for link option {name}")
            }
            Self::DuplicateProvenanceNode(identity) => {
                write!(formatter, "duplicate provenance node {identity}")
            }
            Self::MissingProvenanceNode(identity) => {
                write!(formatter, "missing provenance node {identity}")
            }
            Self::UnknownProvenanceParent(identity) => {
                write!(formatter, "unknown provenance parent {identity}")
            }
            Self::DuplicateProvenanceParent(identity) => {
                write!(formatter, "duplicate provenance parent {identity}")
            }
            Self::OutputParentsMismatch => {
                formatter.write_str("output provenance parents do not exactly match link inputs")
            }
            Self::ProvenanceCycle(identity) => {
                write!(formatter, "provenance cycle reaches {identity}")
            }
            Self::OrphanProvenanceNode(identity) => {
                write!(formatter, "orphan provenance node {identity}")
            }
            Self::OutputIdentityMismatch => {
                formatter.write_str("linked output bytes do not match the expected identity")
            }
        }
    }
}

impl std::error::Error for LinkPlanError {}

impl fmt::Display for ContentIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "sha256:{}:{}",
            HexDigest(&self.sha256),
            self.byte_len
        )
    }
}

struct HexDigest<'a>(&'a [u8; 32]);

impl fmt::Display for HexDigest<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

fn validate_parts(
    target: DeviceTargetV1,
    inputs: &[LinkInputV1],
    options: &[LinkOptionV1],
    output: LinkOutputV1,
    provenance: &[ProvenanceNodeV1],
) -> Result<(), LinkPlanError> {
    if inputs.is_empty() {
        return Err(LinkPlanError::NoInputs);
    }
    if inputs.len() > MAX_LINK_INPUTS {
        return Err(LinkPlanError::TooManyInputs);
    }
    if options.len() > MAX_LINK_OPTIONS {
        return Err(LinkPlanError::TooManyOptions);
    }
    if provenance.len() > MAX_LINK_PROVENANCE_NODES {
        return Err(LinkPlanError::TooManyProvenanceNodes);
    }

    validate_canonical_inputs(target, inputs)?;
    validate_canonical_options(options)?;
    validate_content_identity(output.identity)?;
    if output.target != target {
        return Err(LinkPlanError::OutputTargetMismatch);
    }
    if inputs.iter().any(|input| input.identity == output.identity) {
        return Err(LinkPlanError::OutputAliasesInput);
    }
    validate_provenance(inputs, output, provenance)
}

fn validate_canonical_inputs(
    target: DeviceTargetV1,
    inputs: &[LinkInputV1],
) -> Result<(), LinkPlanError> {
    let mut previous = None;
    let mut digest_lengths = BTreeMap::new();
    for input in inputs {
        validate_content_identity(input.identity)?;
        if input.target != target {
            return Err(LinkPlanError::InputTargetMismatch(input.identity));
        }
        if let Some(length) = digest_lengths.insert(input.identity.sha256, input.identity.byte_len)
            && length != input.identity.byte_len
        {
            return Err(LinkPlanError::ConflictingContentLength(
                input.identity.sha256,
            ));
        }
        if let Some(before) = previous {
            if before == input.identity {
                return Err(LinkPlanError::DuplicateInput(input.identity));
            }
            if before > input.identity {
                return Err(LinkPlanError::NonCanonicalOrder("link inputs"));
            }
        }
        previous = Some(input.identity);
    }
    Ok(())
}

fn validate_canonical_options(options: &[LinkOptionV1]) -> Result<(), LinkPlanError> {
    let mut previous: Option<&LinkOptionV1> = None;
    for option in options {
        validate_option_name(&option.name)?;
        validate_option_value(&option.value)?;
        if let Some(before) = previous {
            if before.name == option.name {
                return if before.value == option.value {
                    Err(LinkPlanError::DuplicateOption(option.name.clone()))
                } else {
                    Err(LinkPlanError::ConflictingOption(option.name.clone()))
                };
            }
            if before > option {
                return Err(LinkPlanError::NonCanonicalOrder("link options"));
            }
        }
        previous = Some(option);
    }
    Ok(())
}

fn validate_content_identity(identity: ContentIdentityV1) -> Result<(), LinkPlanError> {
    if identity.byte_len == 0 {
        return Err(LinkPlanError::EmptyContent);
    }
    if identity.byte_len > MAX_HSACO_BYTES as u64 {
        return Err(LinkPlanError::ContentTooLarge);
    }
    Ok(())
}

fn validate_provenance(
    inputs: &[LinkInputV1],
    output: LinkOutputV1,
    provenance: &[ProvenanceNodeV1],
) -> Result<(), LinkPlanError> {
    let mut previous = None;
    let mut digest_lengths = BTreeMap::new();
    let mut edge_count = 0usize;
    for node in provenance {
        validate_content_identity(node.identity)?;
        if let Some(length) = digest_lengths.insert(node.identity.sha256, node.identity.byte_len)
            && length != node.identity.byte_len
        {
            return Err(LinkPlanError::ConflictingContentLength(
                node.identity.sha256,
            ));
        }
        if let Some(before) = previous {
            if before == node.identity {
                return Err(LinkPlanError::DuplicateProvenanceNode(node.identity));
            }
            if before > node.identity {
                return Err(LinkPlanError::NonCanonicalOrder("provenance nodes"));
            }
        }
        previous = Some(node.identity);
        edge_count = edge_count
            .checked_add(node.parents.len())
            .ok_or(LinkPlanError::TooManyProvenanceEdges)?;
        if edge_count > MAX_LINK_PROVENANCE_EDGES {
            return Err(LinkPlanError::TooManyProvenanceEdges);
        }
        validate_canonical_parents(&node.parents)?;
        for parent in &node.parents {
            validate_content_identity(*parent)?;
            if let Some(length) = digest_lengths.insert(parent.sha256, parent.byte_len)
                && length != parent.byte_len
            {
                return Err(LinkPlanError::ConflictingContentLength(parent.sha256));
            }
        }
    }

    let nodes: BTreeMap<_, _> = provenance
        .iter()
        .map(|node| (node.identity, node))
        .collect();
    for input in inputs {
        if !nodes.contains_key(&input.identity) {
            return Err(LinkPlanError::MissingProvenanceNode(input.identity));
        }
    }
    let output_node = nodes
        .get(&output.identity)
        .ok_or(LinkPlanError::MissingProvenanceNode(output.identity))?;
    let expected_parents: Vec<_> = inputs.iter().map(|input| input.identity).collect();
    if output_node.parents != expected_parents {
        return Err(LinkPlanError::OutputParentsMismatch);
    }
    for node in provenance {
        for parent in &node.parents {
            if !nodes.contains_key(parent) {
                return Err(LinkPlanError::UnknownProvenanceParent(*parent));
            }
        }
    }

    let states = visit_provenance(output.identity, &nodes)?;
    for node in provenance {
        if states.get(&node.identity) != Some(&VisitState::Done) {
            return Err(LinkPlanError::OrphanProvenanceNode(node.identity));
        }
    }
    Ok(())
}

fn validate_canonical_parents(parents: &[ContentIdentityV1]) -> Result<(), LinkPlanError> {
    for pair in parents.windows(2) {
        if pair[0] == pair[1] {
            return Err(LinkPlanError::DuplicateProvenanceParent(pair[0]));
        }
        if pair[0] > pair[1] {
            return Err(LinkPlanError::NonCanonicalOrder("provenance parents"));
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum VisitState {
    Visiting,
    Done,
}

fn visit_provenance(
    identity: ContentIdentityV1,
    nodes: &BTreeMap<ContentIdentityV1, &ProvenanceNodeV1>,
) -> Result<BTreeMap<ContentIdentityV1, VisitState>, LinkPlanError> {
    let mut states = BTreeMap::new();
    let mut stack = vec![(identity, 0usize)];
    states.insert(identity, VisitState::Visiting);

    while let Some((current, next_parent)) = stack.last_mut() {
        let node = nodes
            .get(current)
            .ok_or(LinkPlanError::UnknownProvenanceParent(*current))?;
        let Some(parent) = node.parents.get(*next_parent).copied() else {
            states.insert(*current, VisitState::Done);
            stack.pop();
            continue;
        };
        *next_parent += 1;
        match states.get(&parent) {
            Some(VisitState::Visiting) => return Err(LinkPlanError::ProvenanceCycle(parent)),
            Some(VisitState::Done) => {}
            None => {
                states.insert(parent, VisitState::Visiting);
                stack.push((parent, 0));
            }
        }
    }
    Ok(states)
}

fn validate_option_name(name: &str) -> Result<(), LinkPlanError> {
    if name.is_empty() {
        return Err(LinkPlanError::EmptyOptionName);
    }
    if name.len() > MAX_LINK_OPTION_NAME_BYTES {
        return Err(LinkPlanError::OptionNameTooLong);
    }
    if !name.bytes().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
    }) {
        return Err(LinkPlanError::InvalidOptionName);
    }
    Ok(())
}

fn validate_option_value(value: &str) -> Result<(), LinkPlanError> {
    if value.len() > MAX_LINK_OPTION_VALUE_BYTES {
        return Err(LinkPlanError::OptionValueTooLong);
    }
    if !value.is_ascii()
        || value
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return Err(LinkPlanError::InvalidOptionValue);
    }
    Ok(())
}

fn encode_canonical(
    target: DeviceTargetV1,
    inputs: &[LinkInputV1],
    options: &[LinkOptionV1],
    output: LinkOutputV1,
    provenance: &[ProvenanceNodeV1],
) -> Vec<u8> {
    let mut encoded = Vec::new();
    encoded.extend_from_slice(LINK_PLAN_DOMAIN_V1);
    push_text(&mut encoded, &target.to_string());
    push_u32(&mut encoded, inputs.len());
    for input in inputs {
        push_identity(&mut encoded, input.identity);
    }
    push_u32(&mut encoded, options.len());
    for option in options {
        push_text(&mut encoded, &option.name);
        push_text(&mut encoded, &option.value);
    }
    push_identity(&mut encoded, output.identity);
    push_u32(&mut encoded, provenance.len());
    for node in provenance {
        push_identity(&mut encoded, node.identity);
        push_u32(&mut encoded, node.parents.len());
        for parent in &node.parents {
            push_identity(&mut encoded, *parent);
        }
    }
    encoded
}

fn push_identity(encoded: &mut Vec<u8>, identity: ContentIdentityV1) {
    encoded.extend_from_slice(&identity.sha256);
    encoded.extend_from_slice(&identity.byte_len.to_le_bytes());
}

fn push_text(encoded: &mut Vec<u8>, text: &str) {
    push_u32(encoded, text.len());
    encoded.extend_from_slice(text.as_bytes());
}

fn push_u32(encoded: &mut Vec<u8>, value: usize) {
    encoded.extend_from_slice(&(value as u32).to_le_bytes());
}
