//! Inert target-neutral carriers for canonical KIR V12 capability facts.
//!
//! These entities are part of the one live Pliron graph. They preserve exact
//! compiler input facts but grant no target, proof, artifact, or launch authority.

use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, AsyncCopyCompletionV1, AtomicKind, CollectiveCapabilityOperationV1,
    ExecutionCapabilityOpV1, ExecutionCapabilityRequirementV1,
    ExecutionCapabilityTypeV1 as KirExecutionCapabilityType, MAX_EXECUTION_CAPABILITY_OPERANDS_V1,
    MAX_EXECUTION_CAPABILITY_TYPE_BYTES_V1, MemoryOrdering, NumericalModeV1,
    ResourceCapabilityRequirementV1, ScalarType, SynchronizationScope, ValueId,
    decode_execution_capability_contract_v1, decode_execution_capability_type_v1,
    encode_execution_capability_contract_v1, encode_execution_capability_type_v1,
};
use pliron::{
    builtin::{
        ATTR_KEY_DEBUG_INFO,
        attributes::StringAttr,
        op_interfaces::{NOpdsInterface, NRegionsInterface, NResultsInterface, OneResultInterface},
    },
    common_traits::Verify,
    context::Context,
    derive::{pliron_attr, pliron_op, pliron_type},
    op::Op,
    operation::Operation,
    result::Result,
    r#type::{Type, TypedHandle},
    verify_err, verify_err_noloc,
};

const REQUIREMENT_CODEC_VERSION: u8 = 1;
const MAX_REQUIREMENT_BYTES: usize = 64;

/// Exact immutable V13 operation contract. Live SSA operands are deliberately
/// carried by the enclosing `gpu.execution_capability` operation instead.
#[pliron_attr(name = "kernel.execution_capability_contract", format = "$0")]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ExecutionCapabilityContractAttr(StringAttr);

impl ExecutionCapabilityContractAttr {
    pub fn new(contract: &ExecutionCapabilityOpV1) -> Option<Self> {
        Some(Self(StringAttr::new(encode_hex(
            &encode_execution_capability_contract_v1(contract)?,
        ))))
    }

    pub fn contract(&self, operand_count: usize) -> Option<ExecutionCapabilityOpV1> {
        if operand_count > MAX_EXECUTION_CAPABILITY_OPERANDS_V1 {
            return None;
        }
        let operands = (0..operand_count)
            .map(|ordinal| u32::try_from(ordinal).ok().map(ValueId))
            .collect::<Option<Vec<_>>>()?;
        let bytes = decode_hex(
            self.0.as_str(),
            fe2o3_kernel_ir::MAX_EXECUTION_CAPABILITY_CONTRACT_BYTES_V1,
        )?;
        let contract = decode_execution_capability_contract_v1(&bytes, operands)?;
        (encode_execution_capability_contract_v1(&contract).as_deref() == Some(bytes.as_slice()))
            .then_some(contract)
    }
}

impl Verify for ExecutionCapabilityContractAttr {
    fn verify(&self, _context: &Context) -> Result<()> {
        if self
            .contract(MAX_EXECUTION_CAPABILITY_OPERANDS_V1)
            .is_none()
        {
            return verify_err_noloc!(
                "kernel.execution_capability_contract is not canonical V1 data"
            );
        }
        Ok(())
    }
}

/// Logical zero-runtime-size execution authority retained in the live graph.
#[pliron_type(name = "kernel.execution_capability", format = "$0")]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ExecutionCapabilityType(StringAttr);

impl ExecutionCapabilityType {
    pub fn get(
        context: &Context,
        capability: &KirExecutionCapabilityType,
    ) -> Option<TypedHandle<Self>> {
        Some(Self::instantiate(
            Self(StringAttr::new(encode_hex(
                &encode_execution_capability_type_v1(capability)?,
            ))),
            context,
        ))
    }

    pub fn capability(&self) -> Option<KirExecutionCapabilityType> {
        let bytes = decode_hex(self.0.as_str(), MAX_EXECUTION_CAPABILITY_TYPE_BYTES_V1)?;
        let capability = decode_execution_capability_type_v1(&bytes)?;
        (encode_execution_capability_type_v1(&capability).as_deref() == Some(bytes.as_slice()))
            .then_some(capability)
    }
}

impl Verify for ExecutionCapabilityType {
    fn verify(&self, _context: &Context) -> Result<()> {
        if self.capability().is_none() {
            return verify_err_noloc!("kernel.execution_capability is not canonical V1 data");
        }
        Ok(())
    }
}

/// One nonzero 256-bit canonical identity.
#[pliron_attr(name = "kernel.canonical_identity", format = "$0")]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CanonicalIdentityAttr(StringAttr);

impl CanonicalIdentityAttr {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        let mut text = String::with_capacity(64);
        for byte in bytes {
            use std::fmt::Write as _;
            write!(&mut text, "{byte:02x}").expect("writing to String cannot fail");
        }
        Self(StringAttr::new(text))
    }

    pub fn bytes(&self) -> Option<[u8; 32]> {
        decode_fixed_hex::<32>(self.0.as_str())
    }
}

impl Verify for CanonicalIdentityAttr {
    fn verify(&self, _context: &Context) -> Result<()> {
        match self.bytes() {
            Some(bytes) if bytes != [0; 32] => Ok(()),
            _ => verify_err_noloc!("kernel.canonical_identity must be nonzero canonical hex"),
        }
    }
}

/// Exact source coordinate of one canonical KIR operation.
#[pliron_attr(
    name = "kernel.source_coordinate",
    format = "`<` $function `,` $block `,` $operation `>`",
    verifier = "succ"
)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SourceCoordinateAttr {
    function: u32,
    block: u32,
    operation: u32,
}

impl SourceCoordinateAttr {
    pub const fn new(function: u32, block: u32, operation: u32) -> Self {
        Self {
            function,
            block,
            operation,
        }
    }

    pub const fn components(self) -> (u32, u32, u32) {
        (self.function, self.block, self.operation)
    }
}

/// Canonical zero-based position of one portable requirement.
#[pliron_attr(name = "kernel.requirement_ordinal", format = "$0", verifier = "succ")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RequirementOrdinalAttr(pub u32);

/// Complete portable execution requirement encoded in a closed, bounded form.
#[pliron_attr(name = "kernel.execution_requirement", format = "$0")]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ExecutionRequirementAttr(StringAttr);

impl ExecutionRequirementAttr {
    pub fn new(requirement: &ExecutionCapabilityRequirementV1) -> Self {
        let bytes = encode_requirement(requirement);
        let mut text = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            use std::fmt::Write as _;
            write!(&mut text, "{byte:02x}").expect("writing to String cannot fail");
        }
        Self(StringAttr::new(text))
    }

    pub fn requirement(&self) -> Option<ExecutionCapabilityRequirementV1> {
        let bytes = decode_hex(self.0.as_str(), MAX_REQUIREMENT_BYTES)?;
        decode_requirement(&bytes)
    }
}

impl Verify for ExecutionRequirementAttr {
    fn verify(&self, _context: &Context) -> Result<()> {
        if self.requirement().is_none() {
            return verify_err_noloc!("kernel.execution_requirement is not canonical V1 data");
        }
        Ok(())
    }
}

/// Logical context identity retained as a zero-runtime-size SSA type.
#[pliron_type(
    name = "kernel.kernel_context",
    format = "`<` $root `,` $kernel_marker `,` $target `,` $launch `>`"
)]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct KernelContextType {
    root: StringAttr,
    kernel_marker: CanonicalIdentityAttr,
    target: CanonicalIdentityAttr,
    launch: CanonicalIdentityAttr,
}

impl KernelContextType {
    pub fn get(
        context: &Context,
        root: impl Into<String>,
        kernel_marker: CanonicalIdentityAttr,
        target: CanonicalIdentityAttr,
        launch: CanonicalIdentityAttr,
    ) -> TypedHandle<Self> {
        Self::instantiate(
            Self {
                root: StringAttr::new(root.into()),
                kernel_marker,
                target,
                launch,
            },
            context,
        )
    }

    pub fn root(&self) -> &str {
        self.root.as_str()
    }

    pub const fn kernel_marker(&self) -> &CanonicalIdentityAttr {
        &self.kernel_marker
    }

    pub const fn target(&self) -> &CanonicalIdentityAttr {
        &self.target
    }

    pub const fn launch(&self) -> &CanonicalIdentityAttr {
        &self.launch
    }
}

impl Verify for KernelContextType {
    fn verify(&self, context: &Context) -> Result<()> {
        if self.root().is_empty() || self.root().len() > 256 {
            return verify_err_noloc!("kernel.kernel_context has an invalid root identity");
        }
        self.kernel_marker.verify(context)?;
        self.target.verify(context)?;
        self.launch.verify(context)
    }
}

/// Binds inert module declarations to one exact canonical graph epoch.
#[pliron_op(
    name = "kernel.graph_contract",
    format = "attr($kernel_graph_contract_epoch, $CanonicalIdentityAttr) ` ` attr($kernel_graph_contract_requirement_count, $RequirementOrdinalAttr)",
    interfaces = [
        NOpdsInterface<0>,
        NResultsInterface<0>,
        NRegionsInterface<0>
    ],
    attributes = (
        kernel_graph_contract_epoch: CanonicalIdentityAttr,
        kernel_graph_contract_requirement_count: RequirementOrdinalAttr
    )
)]
pub struct GraphContractOp;

impl GraphContractOp {
    pub fn new(
        context: &mut Context,
        epoch: CanonicalIdentityAttr,
        requirement_count: u32,
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
        op.set_attr_kernel_graph_contract_epoch(context, epoch);
        op.set_attr_kernel_graph_contract_requirement_count(
            context,
            RequirementOrdinalAttr(requirement_count),
        );
        op
    }

    pub fn epoch(&self, context: &Context) -> Option<[u8; 32]> {
        self.get_attr_kernel_graph_contract_epoch(context)?.bytes()
    }

    pub fn requirement_count(&self, context: &Context) -> Option<u32> {
        self.get_attr_kernel_graph_contract_requirement_count(context)
            .map(|count| count.0)
    }
}

impl Verify for GraphContractOp {
    fn verify(&self, context: &Context) -> Result<()> {
        verify_closed_shape(self, context, 2)?;
        if self.epoch(context).is_none() || self.requirement_count(context).is_none() {
            return verify_err!(self.loc(context), "kernel.graph_contract is incomplete");
        }
        Ok(())
    }
}

/// One ordered portable target requirement retained in the live graph.
#[pliron_op(
    name = "kernel.execution_requirement_decl",
    format = "attr($kernel_execution_requirement_decl_epoch, $CanonicalIdentityAttr) ` ` attr($kernel_execution_requirement_decl_ordinal, $RequirementOrdinalAttr) ` ` attr($kernel_execution_requirement_decl_requirement, $ExecutionRequirementAttr)",
    interfaces = [
        NOpdsInterface<0>,
        NResultsInterface<0>,
        NRegionsInterface<0>
    ],
    attributes = (
        kernel_execution_requirement_decl_epoch: CanonicalIdentityAttr,
        kernel_execution_requirement_decl_ordinal: RequirementOrdinalAttr,
        kernel_execution_requirement_decl_requirement: ExecutionRequirementAttr
    )
)]
pub struct ExecutionRequirementOp;

impl ExecutionRequirementOp {
    pub fn new(
        context: &mut Context,
        epoch: CanonicalIdentityAttr,
        ordinal: u32,
        requirement: &ExecutionCapabilityRequirementV1,
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
        op.set_attr_kernel_execution_requirement_decl_epoch(context, epoch);
        op.set_attr_kernel_execution_requirement_decl_ordinal(
            context,
            RequirementOrdinalAttr(ordinal),
        );
        op.set_attr_kernel_execution_requirement_decl_requirement(
            context,
            ExecutionRequirementAttr::new(requirement),
        );
        op
    }

    pub fn epoch(&self, context: &Context) -> Option<[u8; 32]> {
        self.get_attr_kernel_execution_requirement_decl_epoch(context)?
            .bytes()
    }

    pub fn ordinal(&self, context: &Context) -> Option<u32> {
        self.get_attr_kernel_execution_requirement_decl_ordinal(context)
            .map(|ordinal| ordinal.0)
    }

    pub fn requirement(&self, context: &Context) -> Option<ExecutionCapabilityRequirementV1> {
        self.get_attr_kernel_execution_requirement_decl_requirement(context)?
            .requirement()
    }
}

impl Verify for ExecutionRequirementOp {
    fn verify(&self, context: &Context) -> Result<()> {
        verify_closed_shape(self, context, 3)?;
        if self.epoch(context).is_none()
            || self.ordinal(context).is_none()
            || self.requirement(context).is_none()
        {
            return verify_err!(
                self.loc(context),
                "kernel.execution_requirement_decl is incomplete"
            );
        }
        Ok(())
    }
}

/// Authenticated creation of one logical kernel-context SSA value.
#[pliron_op(
    name = "kernel.kernel_context_issue",
    format = "attr($kernel_kernel_context_issue_epoch, $CanonicalIdentityAttr) ` ` attr($kernel_kernel_context_issue_coordinate, $SourceCoordinateAttr) ` ` attr($kernel_kernel_context_issue_operation_identity, $CanonicalIdentityAttr) ` ` attr($kernel_kernel_context_issue_frontend_unit, $CanonicalIdentityAttr) ` ` attr($kernel_kernel_context_issue_function, $CanonicalIdentityAttr) ` ` attr($kernel_kernel_context_issue_contract, $CanonicalIdentityAttr) ` ` attr($kernel_kernel_context_issue_issuance, $CanonicalIdentityAttr) ` : ` type($0)",
    interfaces = [
        NOpdsInterface<0>,
        OneResultInterface,
        NRegionsInterface<0>
    ],
    results = (context: KernelContextType),
    attributes = (
        kernel_kernel_context_issue_epoch: CanonicalIdentityAttr,
        kernel_kernel_context_issue_coordinate: SourceCoordinateAttr,
        kernel_kernel_context_issue_operation_identity: CanonicalIdentityAttr,
        kernel_kernel_context_issue_frontend_unit: CanonicalIdentityAttr,
        kernel_kernel_context_issue_function: CanonicalIdentityAttr,
        kernel_kernel_context_issue_contract: CanonicalIdentityAttr,
        kernel_kernel_context_issue_issuance: CanonicalIdentityAttr
    )
)]
pub struct KernelContextIssueOp;

impl KernelContextIssueOp {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        context: &mut Context,
        result_type: TypedHandle<KernelContextType>,
        epoch: CanonicalIdentityAttr,
        coordinate: SourceCoordinateAttr,
        operation_identity: CanonicalIdentityAttr,
        frontend_unit: CanonicalIdentityAttr,
        function: CanonicalIdentityAttr,
        contract: CanonicalIdentityAttr,
        issuance: CanonicalIdentityAttr,
    ) -> Self {
        let operation = Operation::new(
            context,
            Self::get_concrete_op_info(),
            vec![result_type.into()],
            vec![],
            vec![],
            0,
        );
        let op = Self::from_operation(operation);
        op.set_attr_kernel_kernel_context_issue_epoch(context, epoch);
        op.set_attr_kernel_kernel_context_issue_coordinate(context, coordinate);
        op.set_attr_kernel_kernel_context_issue_operation_identity(context, operation_identity);
        op.set_attr_kernel_kernel_context_issue_frontend_unit(context, frontend_unit);
        op.set_attr_kernel_kernel_context_issue_function(context, function);
        op.set_attr_kernel_kernel_context_issue_contract(context, contract);
        op.set_attr_kernel_kernel_context_issue_issuance(context, issuance);
        op
    }

    pub fn epoch(&self, context: &Context) -> Option<[u8; 32]> {
        self.get_attr_kernel_kernel_context_issue_epoch(context)?
            .bytes()
    }

    pub fn coordinate(&self, context: &Context) -> Option<SourceCoordinateAttr> {
        self.get_attr_kernel_kernel_context_issue_coordinate(context)
            .map(|coordinate| *coordinate)
    }

    pub fn operation_identity(&self, context: &Context) -> Option<[u8; 32]> {
        self.get_attr_kernel_kernel_context_issue_operation_identity(context)?
            .bytes()
    }

    pub fn source_identities(&self, context: &Context) -> Option<[[u8; 32]; 4]> {
        Some([
            self.get_attr_kernel_kernel_context_issue_frontend_unit(context)?
                .bytes()?,
            self.get_attr_kernel_kernel_context_issue_function(context)?
                .bytes()?,
            self.get_attr_kernel_kernel_context_issue_contract(context)?
                .bytes()?,
            self.get_attr_kernel_kernel_context_issue_issuance(context)?
                .bytes()?,
        ])
    }
}

impl Verify for KernelContextIssueOp {
    fn verify(&self, context: &Context) -> Result<()> {
        verify_closed_shape(self, context, 7)?;
        if self.epoch(context).is_none()
            || self.coordinate(context).is_none()
            || self.operation_identity(context).is_none()
            || self.source_identities(context).is_none()
        {
            return verify_err!(
                self.loc(context),
                "kernel.kernel_context_issue has incomplete provenance"
            );
        }
        Ok(())
    }
}

fn verify_closed_shape(op: &dyn Op, context: &Context, attributes: usize) -> Result<()> {
    let raw = op.get_operation().deref(context);
    let debug = raw.attributes.0.get(&*ATTR_KEY_DEBUG_INFO);
    let debug_valid = debug
        .map(|attribute| {
            let id = attribute.get_attr_id();
            id.dialect.as_ref() == "builtin" && AsRef::<str>::as_ref(&id.name) == "debug_info"
        })
        .unwrap_or(true);
    if raw.get_num_operands() != 0
        || raw.get_num_successors() != 0
        || raw.num_regions() != 0
        || raw.attributes.0.len() != attributes + usize::from(debug.is_some())
        || !debug_valid
    {
        return verify_err!(
            op.loc(context),
            "{} has malformed capability payload",
            op.get_opid()
        );
    }
    Ok(())
}

fn encode_requirement(requirement: &ExecutionCapabilityRequirementV1) -> Vec<u8> {
    let mut bytes = vec![REQUIREMENT_CODEC_VERSION];
    match requirement {
        ExecutionCapabilityRequirementV1::AddressSpace {
            address_space,
            access,
        } => {
            bytes.extend([
                1,
                encode_address_space(*address_space),
                encode_access(*access),
            ]);
        }
        ExecutionCapabilityRequirementV1::Atomic {
            value_type,
            operation,
            ordering,
            failure_ordering,
            scope,
            address_space,
        } => bytes.extend([
            2,
            encode_scalar(*value_type),
            encode_atomic(*operation),
            encode_order(*ordering),
            failure_ordering.map(encode_order).unwrap_or(0),
            encode_scope(*scope),
            encode_address_space(*address_space),
        ]),
        ExecutionCapabilityRequirementV1::Barrier {
            execution_scope,
            memory_scope,
            ordering,
            address_spaces,
        } => bytes.extend([
            3,
            encode_scope(*execution_scope),
            encode_scope(*memory_scope),
            encode_order(*ordering),
            address_spaces.iter().fold(0_u8, |mask, space| {
                mask | (1 << (encode_address_space(*space) - 1))
            }),
        ]),
        ExecutionCapabilityRequirementV1::Collective {
            execution_scope,
            operation,
            value_type,
            participants,
        } => {
            bytes.extend([
                4,
                encode_scope(*execution_scope),
                encode_collective(*operation),
                encode_scalar(*value_type),
            ]);
            bytes.extend(participants.to_le_bytes());
        }
        ExecutionCapabilityRequirementV1::Matrix {
            m,
            n,
            k,
            input_type,
            accumulator_type,
        } => {
            bytes.push(5);
            bytes.extend(m.to_le_bytes());
            bytes.extend(n.to_le_bytes());
            bytes.extend(k.to_le_bytes());
            bytes.extend([encode_scalar(*input_type), encode_scalar(*accumulator_type)]);
        }
        ExecutionCapabilityRequirementV1::AsyncCopy {
            source,
            destination,
            bytes: count,
            alignment,
            completion,
        } => {
            bytes.extend([
                6,
                encode_address_space(*source),
                encode_address_space(*destination),
            ]);
            bytes.extend(count.to_le_bytes());
            bytes.extend(alignment.to_le_bytes());
            match completion {
                AsyncCopyCompletionV1::ExplicitWaitGroups {
                    maximum_pending_groups,
                } => {
                    bytes.push(1);
                    bytes.extend(maximum_pending_groups.to_le_bytes());
                }
                AsyncCopyCompletionV1::WorkgroupBarrier => bytes.push(2),
            }
        }
        ExecutionCapabilityRequirementV1::Numerical { value_type, mode } => {
            bytes.extend([7, encode_scalar(*value_type), encode_numerical(*mode)]);
        }
        ExecutionCapabilityRequirementV1::Resource(resource) => {
            bytes.push(8);
            let (kind, value) = match resource {
                ResourceCapabilityRequirementV1::WorkgroupInvocationsAtMost(value) => {
                    (1, u64::from(*value))
                }
                ResourceCapabilityRequirementV1::StaticWorkgroupMemoryBytesAtMost(value) => {
                    (2, *value)
                }
                ResourceCapabilityRequirementV1::DynamicWorkgroupMemoryBytesAtMost(value) => {
                    (3, *value)
                }
                ResourceCapabilityRequirementV1::PrivateMemoryBytesPerInvocationAtMost(value) => {
                    (4, *value)
                }
            };
            bytes.push(kind);
            bytes.extend(value.to_le_bytes());
        }
    }
    bytes
}

fn decode_requirement(bytes: &[u8]) -> Option<ExecutionCapabilityRequirementV1> {
    let mut reader = Reader::new(bytes);
    if reader.u8()? != REQUIREMENT_CODEC_VERSION {
        return None;
    }
    let requirement = match reader.u8()? {
        1 => ExecutionCapabilityRequirementV1::AddressSpace {
            address_space: decode_address_space(reader.u8()?)?,
            access: decode_access(reader.u8()?)?,
        },
        2 => ExecutionCapabilityRequirementV1::Atomic {
            value_type: decode_scalar(reader.u8()?)?,
            operation: decode_atomic(reader.u8()?)?,
            ordering: decode_order(reader.u8()?)?,
            failure_ordering: decode_optional_order(reader.u8()?)?,
            scope: decode_scope(reader.u8()?)?,
            address_space: decode_address_space(reader.u8()?)?,
        },
        3 => {
            let execution_scope = decode_scope(reader.u8()?)?;
            let memory_scope = decode_scope(reader.u8()?)?;
            let ordering = decode_order(reader.u8()?)?;
            let mask = reader.u8()?;
            if mask == 0 || mask & !0x1f != 0 {
                return None;
            }
            let address_spaces = (1..=5)
                .filter(|tag| mask & (1 << (tag - 1)) != 0)
                .map(decode_address_space)
                .collect::<Option<_>>()?;
            ExecutionCapabilityRequirementV1::Barrier {
                execution_scope,
                memory_scope,
                ordering,
                address_spaces,
            }
        }
        4 => ExecutionCapabilityRequirementV1::Collective {
            execution_scope: decode_scope(reader.u8()?)?,
            operation: decode_collective(reader.u8()?)?,
            value_type: decode_scalar(reader.u8()?)?,
            participants: reader.u32()?,
        },
        5 => ExecutionCapabilityRequirementV1::Matrix {
            m: reader.u16()?,
            n: reader.u16()?,
            k: reader.u16()?,
            input_type: decode_scalar(reader.u8()?)?,
            accumulator_type: decode_scalar(reader.u8()?)?,
        },
        6 => {
            let source = decode_address_space(reader.u8()?)?;
            let destination = decode_address_space(reader.u8()?)?;
            let count = reader.u32()?;
            let alignment = reader.u16()?;
            let completion = match reader.u8()? {
                1 => AsyncCopyCompletionV1::ExplicitWaitGroups {
                    maximum_pending_groups: reader.u16()?,
                },
                2 => AsyncCopyCompletionV1::WorkgroupBarrier,
                _ => return None,
            };
            ExecutionCapabilityRequirementV1::AsyncCopy {
                source,
                destination,
                bytes: count,
                alignment,
                completion,
            }
        }
        7 => ExecutionCapabilityRequirementV1::Numerical {
            value_type: decode_scalar(reader.u8()?)?,
            mode: decode_numerical(reader.u8()?)?,
        },
        8 => {
            let kind = reader.u8()?;
            let value = reader.u64()?;
            let resource = match kind {
                1 => ResourceCapabilityRequirementV1::WorkgroupInvocationsAtMost(
                    value.try_into().ok()?,
                ),
                2 => ResourceCapabilityRequirementV1::StaticWorkgroupMemoryBytesAtMost(value),
                3 => ResourceCapabilityRequirementV1::DynamicWorkgroupMemoryBytesAtMost(value),
                4 => ResourceCapabilityRequirementV1::PrivateMemoryBytesPerInvocationAtMost(value),
                _ => return None,
            };
            ExecutionCapabilityRequirementV1::Resource(resource)
        }
        _ => return None,
    };
    reader.finished().then_some(requirement)
}

macro_rules! closed_tags {
    ($encode:ident, $decode:ident, $ty:ty, { $($variant:path => $tag:literal),+ $(,)? }) => {
        const fn $encode(value: $ty) -> u8 {
            match value { $($variant => $tag),+ }
        }
        const fn $decode(tag: u8) -> Option<$ty> {
            match tag { $($tag => Some($variant),)+ _ => None }
        }
    };
}

closed_tags!(encode_address_space, decode_address_space, AddressSpace, {
    AddressSpace::Private => 1, AddressSpace::Workgroup => 2, AddressSpace::Global => 3,
    AddressSpace::Constant => 4, AddressSpace::Generic => 5,
});
closed_tags!(encode_access, decode_access, AccessMode, {
    AccessMode::ReadOnly => 1, AccessMode::WriteOnly => 2, AccessMode::ReadWrite => 3,
});
closed_tags!(encode_scalar, decode_scalar, ScalarType, {
    ScalarType::Bool => 1, ScalarType::I8 => 2, ScalarType::I16 => 3,
    ScalarType::I32 => 4, ScalarType::I64 => 5, ScalarType::I128 => 6,
    ScalarType::U8 => 7, ScalarType::U16 => 8, ScalarType::U32 => 9,
    ScalarType::U64 => 10, ScalarType::U128 => 11, ScalarType::Index => 12,
    ScalarType::F16 => 13, ScalarType::Bf16 => 14, ScalarType::F32 => 15,
    ScalarType::F64 => 16,
});
closed_tags!(encode_atomic, decode_atomic, AtomicKind, {
    AtomicKind::Load => 1, AtomicKind::Store => 2, AtomicKind::Exchange => 3,
    AtomicKind::CompareExchange => 4, AtomicKind::Add => 5, AtomicKind::Subtract => 6,
    AtomicKind::Min => 7, AtomicKind::Max => 8, AtomicKind::BitAnd => 9,
    AtomicKind::BitOr => 10, AtomicKind::BitXor => 11,
});
closed_tags!(encode_order, decode_order, MemoryOrdering, {
    MemoryOrdering::Relaxed => 1, MemoryOrdering::Acquire => 2,
    MemoryOrdering::Release => 3, MemoryOrdering::AcquireRelease => 4,
    MemoryOrdering::SequentiallyConsistent => 5,
});
closed_tags!(encode_scope, decode_scope, SynchronizationScope, {
    SynchronizationScope::Invocation => 1, SynchronizationScope::Subgroup => 2,
    SynchronizationScope::Workgroup => 3, SynchronizationScope::Device => 4,
    SynchronizationScope::System => 5,
});
closed_tags!(encode_collective, decode_collective, CollectiveCapabilityOperationV1, {
    CollectiveCapabilityOperationV1::Broadcast => 1,
    CollectiveCapabilityOperationV1::ReduceAdd => 2,
    CollectiveCapabilityOperationV1::ReduceMin => 3,
    CollectiveCapabilityOperationV1::ReduceMax => 4,
    CollectiveCapabilityOperationV1::InclusiveScanAdd => 5,
    CollectiveCapabilityOperationV1::ExclusiveScanAdd => 6,
    CollectiveCapabilityOperationV1::Any => 7,
    CollectiveCapabilityOperationV1::All => 8,
});
closed_tags!(encode_numerical, decode_numerical, NumericalModeV1, {
    NumericalModeV1::StrictIeee => 1, NumericalModeV1::AllowContraction => 2,
    NumericalModeV1::AllowApproximation => 3,
});

fn decode_optional_order(tag: u8) -> Option<Option<MemoryOrdering>> {
    if tag == 0 {
        Some(None)
    } else {
        decode_order(tag).map(Some)
    }
}

fn decode_hex(text: &str, max_bytes: usize) -> Option<Vec<u8>> {
    if text.is_empty() || !text.len().is_multiple_of(2) || text.len() / 2 > max_bytes {
        return None;
    }
    text.as_bytes()
        .chunks_exact(2)
        .map(|pair| Some((hex_digit(pair[0])? << 4) | hex_digit(pair[1])?))
        .collect()
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut text, "{byte:02x}").expect("writing to String cannot fail");
    }
    text
}

fn decode_fixed_hex<const N: usize>(text: &str) -> Option<[u8; N]> {
    decode_hex(text, N)?.try_into().ok()
}

const fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }
    fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let end = self.cursor.checked_add(N)?;
        let value = self.bytes.get(self.cursor..end)?.try_into().ok()?;
        self.cursor = end;
        Some(value)
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take::<1>()?[0])
    }
    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take()?))
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take()?))
    }
    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take()?))
    }
    const fn finished(&self) -> bool {
        self.cursor == self.bytes.len()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn every_requirement_family_has_an_exact_closed_codec() {
        let requirements = [
            ExecutionCapabilityRequirementV1::AddressSpace {
                address_space: AddressSpace::Global,
                access: AccessMode::ReadOnly,
            },
            ExecutionCapabilityRequirementV1::Atomic {
                value_type: ScalarType::U32,
                operation: AtomicKind::CompareExchange,
                ordering: MemoryOrdering::AcquireRelease,
                failure_ordering: Some(MemoryOrdering::Acquire),
                scope: SynchronizationScope::Device,
                address_space: AddressSpace::Global,
            },
            ExecutionCapabilityRequirementV1::Barrier {
                execution_scope: SynchronizationScope::Workgroup,
                memory_scope: SynchronizationScope::Device,
                ordering: MemoryOrdering::AcquireRelease,
                address_spaces: BTreeSet::from([AddressSpace::Global, AddressSpace::Workgroup]),
            },
            ExecutionCapabilityRequirementV1::Collective {
                execution_scope: SynchronizationScope::Subgroup,
                operation: CollectiveCapabilityOperationV1::ReduceAdd,
                value_type: ScalarType::F32,
                participants: 32,
            },
            ExecutionCapabilityRequirementV1::Matrix {
                m: 16,
                n: 16,
                k: 16,
                input_type: ScalarType::F16,
                accumulator_type: ScalarType::F32,
            },
            ExecutionCapabilityRequirementV1::AsyncCopy {
                source: AddressSpace::Global,
                destination: AddressSpace::Workgroup,
                bytes: 256,
                alignment: 16,
                completion: AsyncCopyCompletionV1::ExplicitWaitGroups {
                    maximum_pending_groups: 4,
                },
            },
            ExecutionCapabilityRequirementV1::Numerical {
                value_type: ScalarType::F32,
                mode: NumericalModeV1::StrictIeee,
            },
            ExecutionCapabilityRequirementV1::Resource(
                ResourceCapabilityRequirementV1::DynamicWorkgroupMemoryBytesAtMost(4096),
            ),
        ];
        for requirement in requirements {
            let attr = ExecutionRequirementAttr::new(&requirement);
            assert_eq!(attr.requirement(), Some(requirement));
        }
    }

    #[test]
    fn requirement_codec_rejects_unknown_truncated_and_trailing_data() {
        for text in ["", "0", "01ff", "0101", "01010300"] {
            assert!(
                ExecutionRequirementAttr(StringAttr::new(text.into()))
                    .requirement()
                    .is_none()
            );
        }
    }
}
