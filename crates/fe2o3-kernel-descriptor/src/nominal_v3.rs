//! Inert nominal source records and allocation-free descriptor V3 views.

use std::fmt;

use crate::model::PhysicalAbiComponentV1;
use crate::*;

pub const DEVICE_DESCRIPTOR_VERSION_V3: u16 = 3;
pub const CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V3: usize = 16;
pub const RUST_TYPE_DOMAIN_V3: &[u8] = b"FE2O3/RUST-TYPE/V3\0";
pub const DEVICE_DESCRIPTOR_TABLE_DOMAIN_V3: &[u8] = b"FE2O3/DEVICE-DESCRIPTOR-TABLE/V3\0";

/// Nominal source kind, not producer authentication or execution authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceTypeDescriptorV3 {
    Scalar(ScalarTypeV1),
    SharedSlice(ScalarTypeV1),
    DisjointSlice(ScalarTypeV1),
    GlobalMutPointer(ScalarTypeV1),
    Usize,
    Isize,
}

impl SourceTypeDescriptorV3 {
    pub const fn physical_scalar(self) -> ScalarTypeV1 {
        match self {
            Self::Scalar(s)
            | Self::SharedSlice(s)
            | Self::DisjointSlice(s)
            | Self::GlobalMutPointer(s) => s,
            Self::Usize => ScalarTypeV1::U64,
            Self::Isize => ScalarTypeV1::I64,
        }
    }

    pub const fn canonical_payload(self) -> [u8; 4] {
        let (kind, scalar) = match self {
            Self::Scalar(s) => (1, crate::encode::scalar_tag(s)),
            Self::SharedSlice(s) => (2, crate::encode::scalar_tag(s)),
            Self::DisjointSlice(s) => (3, crate::encode::scalar_tag(s)),
            Self::GlobalMutPointer(s) => (4, crate::encode::scalar_tag(s)),
            Self::Usize => (5, 0),
            Self::Isize => (6, 0),
        };
        [kind, scalar, 0, 0]
    }

    pub(crate) const fn domain(self) -> &'static [u8] {
        match self {
            Self::Usize | Self::Isize => RUST_TYPE_DOMAIN_V3,
            _ => RUST_TYPE_DOMAIN_V1,
        }
    }

    pub(crate) fn expected_layout(self) -> DeviceLayoutDescriptorV1 {
        match self {
            Self::Scalar(s) => DeviceLayoutDescriptorV1::scalar(s),
            Self::SharedSlice(s) => DeviceLayoutDescriptorV1::shared_slice(s),
            Self::DisjointSlice(s) => DeviceLayoutDescriptorV1::disjoint_slice(s),
            Self::GlobalMutPointer(s) => DeviceLayoutDescriptorV1::global_mut_pointer(s),
            Self::Usize => DeviceLayoutDescriptorV1::scalar(ScalarTypeV1::U64),
            Self::Isize => DeviceLayoutDescriptorV1::scalar(ScalarTypeV1::I64),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceTypeRecordV3 {
    pub(crate) identity: RustTypeIdentity,
    pub(crate) descriptor: SourceTypeDescriptorV3,
}

impl SourceTypeRecordV3 {
    /// Computes the exact descriptor identity without constructing a V1 record.
    /// Caller prepays QUERY scratch, including this returned fixed row's header.
    pub fn new<E>(
        descriptor: SourceTypeDescriptorV3,
        charge: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, DescriptorWireErrorV3<E>> {
        pay(charge, descriptor.domain().len() + 8 + 4)?;
        Ok(Self {
            identity: RustTypeIdentity::from_bytes(crate::digest::domain_hash(
                descriptor.domain(),
                &descriptor.canonical_payload(),
            )),
            descriptor,
        })
    }

    pub const fn identity(self) -> RustTypeIdentity {
        self.identity
    }
    pub const fn descriptor(self) -> SourceTypeDescriptorV3 {
        self.descriptor
    }
}

/// Fixed-size physical data. This plain input is not a checked source view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalComponentV3 {
    pub kind: PhysicalAbiComponentKind,
    pub offset: u32,
    pub size: u16,
    pub alignment: u16,
    pub access: AccessMode,
    pub alias: AliasSemantics,
}

impl PhysicalComponentV3 {
    pub(crate) fn legacy(self) -> PhysicalAbiComponentV1 {
        PhysicalAbiComponentV1 {
            kind: self.kind,
            offset: self.offset,
            size: self.size,
            alignment: self.alignment,
            access: self.access,
            alias: self.alias,
        }
    }
}

/// Borrowed producer row; no hidden component Vec or name String is constructed.
pub struct LogicalArgumentInputV3<'a> {
    pub source_index: u16,
    pub name: &'a str,
    pub source_type: RustTypeIdentity,
    pub device_layout: DeviceLayoutIdentity,
    pub ownership: OwnershipSemantics,
    pub access: AccessMode,
    pub alias: AliasSemantics,
    pub components: &'a [PhysicalComponentV3],
}

/// Borrowed producer kernel. Complete admission occurs in the V3 encoder.
pub struct KernelDescriptorInputV3<'a> {
    pub kernel_id: KernelId,
    pub logical_name: &'a str,
    pub entry_name: &'a str,
    pub descriptor_symbol: &'a str,
    pub source_evidence: BuildEvidenceV1,
    pub executable_ir_evidence: BuildEvidenceV1,
    pub capabilities: &'a [CapabilityV1],
    pub abi_layout: KernelAbiLayoutV1,
    pub launch: &'a LaunchConstraintsV1,
    pub arguments: &'a [LogicalArgumentInputV3<'a>],
}

/// Does not allocate, validate, or confer source/native authority by construction.
pub struct DeviceDescriptorTableInputV3<'a> {
    pub canonical_code_object_digest: CanonicalCodeObjectDigest,
    pub code_object_version: CodeObjectVersion,
    pub compiler: &'a CompilerIdentityV1,
    pub producer: &'a ProducerIdentityV1,
    pub device_target: DeviceTargetV1,
    pub type_records: &'a [SourceTypeRecordV3],
    pub layout_records: &'a [DeviceLayoutRecordV1],
    pub kernels: &'a [KernelDescriptorInputV3<'a>],
    pub requirements: &'a [KernelTargetRequirementsV2],
}

#[derive(Debug)]
pub enum DescriptorWireErrorV3<E> {
    Decode(DecodeError),
    Work(E),
    OutputLength {
        expected: usize,
        actual: usize,
    },
    Index {
        field: &'static str,
        index: usize,
        count: usize,
    },
}

impl<E> From<DecodeError> for DescriptorWireErrorV3<E> {
    fn from(error: DecodeError) -> Self {
        Self::Decode(error)
    }
}
impl<E> From<ValidationError> for DescriptorWireErrorV3<E> {
    fn from(error: ValidationError) -> Self {
        Self::Decode(error.into())
    }
}
impl<E: fmt::Display> fmt::Display for DescriptorWireErrorV3<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(e) => write!(f, "invalid V3 descriptor: {e}"),
            Self::Work(e) => write!(f, "V3 descriptor work refused: {e}"),
            Self::OutputLength { expected, actual } => {
                write!(f, "V3 output length {actual}, expected {expected}")
            }
            Self::Index {
                field,
                index,
                count,
            } => write!(f, "V3 {field} index {index}, count {count}"),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for DescriptorWireErrorV3<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Decode(e) => Some(e),
            Self::Work(e) => Some(e),
            _ => None,
        }
    }
}

pub(crate) fn pay<E>(
    charge: &mut impl FnMut(usize) -> Result<(), E>,
    amount: usize,
) -> Result<(), DescriptorWireErrorV3<E>> {
    charge(amount).map_err(DescriptorWireErrorV3::Work)
}

/// Allocation-free physical record constructor; its V1 identity is unchanged.
/// Caller prepays QUERY scratch, including the returned fixed row's header.
pub fn device_layout_record_v3<E>(
    descriptor: DeviceLayoutDescriptorV1,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<DeviceLayoutRecordV1, DescriptorWireErrorV3<E>> {
    pay(charge, DEVICE_LAYOUT_DOMAIN_V1.len() + 8 + 12)?;
    let identity = DeviceLayoutIdentity::from_bytes(crate::digest::domain_hash(
        DEVICE_LAYOUT_DOMAIN_V1,
        &layout_payload(&descriptor),
    ));
    Ok(DeviceLayoutRecordV1 {
        identity,
        descriptor,
    })
}

pub(crate) fn layout_payload(d: &DeviceLayoutDescriptorV1) -> [u8; 12] {
    let size = d.size.to_le_bytes();
    let align = d.alignment.to_le_bytes();
    [
        crate::encode::descriptor_kind_tag(d.kind),
        crate::encode::scalar_tag(d.element),
        size[0],
        size[1],
        align[0],
        align[1],
        d.pointer_width,
        d.length_width,
        0,
        0,
        0,
        0,
    ]
}

/// Completely validated borrowed wire, not proof or artifact authority.
/// The inline MAX_KERNELS index is retained storage; this type is not Clone/Copy.
///
/// The caller retains paid VIEW storage and the complete borrowed wire backing.
/// Before `table_digest` or any fallible query, it prepays QUERY storage in
/// addition to all still-live views/cursors/results. QUERY covers one complete
/// kernel/capability/argument/component traversal; concurrent traversals need
/// separate extents. O(1) getters allocate nothing and perform no byte scan.
///
/// ```compile_fail
/// use fe2o3_kernel_descriptor::DeviceDescriptorTableV3;
/// fn duplicate(view: DeviceDescriptorTableV3<'_>) {
///     let first = view;
///     let second = view;
///     drop((first, second));
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_kernel_descriptor::*;
/// fn detached<'a>(bytes: &'a [u8]) -> ArgumentCursorV3<'a, 'a> {
///     let view = decode_device_descriptor_table_v3(bytes, &mut |_| Ok::<_, ()>(())).unwrap();
///     view.kernel(0, &mut |_| Ok::<_, ()>(())).unwrap().arguments()
/// }
/// ```
pub struct DeviceDescriptorTableV3<'wire> {
    pub(crate) bytes: &'wire [u8],
    pub(crate) digest: CanonicalCodeObjectDigest,
    pub(crate) cov: CodeObjectVersion,
    pub(crate) compiler_name: &'wire str,
    pub(crate) compiler_release: &'wire str,
    pub(crate) compiler_commit: [u8; 20],
    pub(crate) producer_name: &'wire str,
    pub(crate) producer_version: &'wire str,
    pub(crate) target: DeviceTargetV1,
    pub(crate) types_start: usize,
    pub(crate) layouts_start: usize,
    pub(crate) requirements_start: usize,
    pub(crate) type_count: usize,
    pub(crate) layout_count: usize,
    pub(crate) kernel_count: usize,
    pub(crate) kernel_offsets: [usize; MAX_KERNELS],
}

impl<'wire> DeviceDescriptorTableV3<'wire> {
    pub const fn canonical_bytes(&self) -> &'wire [u8] {
        self.bytes
    }
    pub const fn canonical_code_object_digest(&self) -> CanonicalCodeObjectDigest {
        self.digest
    }
    pub const fn code_object_version(&self) -> CodeObjectVersion {
        self.cov
    }
    pub const fn device_target(&self) -> DeviceTargetV1 {
        self.target
    }
    pub const fn compiler_name(&self) -> &'wire str {
        self.compiler_name
    }
    pub const fn compiler_release(&self) -> &'wire str {
        self.compiler_release
    }
    pub const fn compiler_commit(&self) -> &[u8; 20] {
        &self.compiler_commit
    }
    pub const fn producer_name(&self) -> &'wire str {
        self.producer_name
    }
    pub const fn producer_version(&self) -> &'wire str {
        self.producer_version
    }
    pub const fn type_count(&self) -> usize {
        self.type_count
    }
    pub const fn layout_count(&self) -> usize {
        self.layout_count
    }
    pub const fn kernel_count(&self) -> usize {
        self.kernel_count
    }

    pub fn table_digest<E>(
        &self,
        charge: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<DeviceDescriptorTableDigest, DescriptorWireErrorV3<E>> {
        pay(
            charge,
            DEVICE_DESCRIPTOR_TABLE_DOMAIN_V3.len() + 8 + self.bytes.len(),
        )?;
        Ok(DeviceDescriptorTableDigest::from_bytes(
            crate::digest::domain_hash(DEVICE_DESCRIPTOR_TABLE_DOMAIN_V3, self.bytes),
        ))
    }

    pub fn source_type<E>(
        &self,
        identity: RustTypeIdentity,
        charge: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<SourceTypeRecordV3, DescriptorWireErrorV3<E>> {
        crate::wire_v3::lookup_source(self, identity, charge)
    }

    pub fn device_layout<E>(
        &self,
        identity: DeviceLayoutIdentity,
        charge: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<DeviceLayoutRecordV1, DescriptorWireErrorV3<E>> {
        crate::wire_v3::lookup_layout(self, identity, charge)
    }

    pub fn kernel<E>(
        &self,
        index: usize,
        charge: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<KernelDescriptorRefV3<'_, 'wire>, DescriptorWireErrorV3<E>> {
        crate::wire_v3::kernel(self, index, charge)
    }

    pub fn find_kernel<E>(
        &self,
        id: KernelId,
        charge: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<KernelDescriptorRefV3<'_, 'wire>, DescriptorWireErrorV3<E>> {
        crate::wire_v3::find_kernel(self, id, charge)
    }

    pub fn requirement<E>(
        &self,
        index: usize,
        charge: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<KernelTargetRequirementsV2, DescriptorWireErrorV3<E>> {
        crate::wire_v3::requirement(self, index, charge)
    }
}

pub struct KernelDescriptorRefV3<'view, 'wire> {
    pub(crate) table: &'view DeviceDescriptorTableV3<'wire>,
    pub(crate) id: KernelId,
    pub(crate) logical_name: &'wire str,
    pub(crate) entry_name: &'wire str,
    pub(crate) descriptor_symbol: &'wire str,
    pub(crate) source: BuildEvidenceV1,
    pub(crate) executable: BuildEvidenceV1,
    pub(crate) capabilities_start: usize,
    pub(crate) capability_count: usize,
    pub(crate) launch: LaunchConstraintsV1,
    pub(crate) abi: KernelAbiLayoutV1,
    pub(crate) arguments_start: usize,
    pub(crate) argument_count: usize,
    pub(crate) component_count: usize,
    pub(crate) end: usize,
}

impl<'view, 'wire> KernelDescriptorRefV3<'view, 'wire> {
    pub const fn kernel_id(&self) -> KernelId {
        self.id
    }
    pub const fn logical_name(&self) -> &'wire str {
        self.logical_name
    }
    pub const fn entry_name(&self) -> &'wire str {
        self.entry_name
    }
    pub const fn descriptor_symbol(&self) -> &'wire str {
        self.descriptor_symbol
    }
    pub const fn source_evidence(&self) -> BuildEvidenceV1 {
        self.source
    }
    pub const fn executable_ir_evidence(&self) -> BuildEvidenceV1 {
        self.executable
    }
    pub const fn launch(&self) -> &LaunchConstraintsV1 {
        &self.launch
    }
    pub const fn abi_layout(&self) -> KernelAbiLayoutV1 {
        self.abi
    }
    pub const fn argument_count(&self) -> usize {
        self.argument_count
    }
    pub const fn component_count(&self) -> usize {
        self.component_count
    }
    pub const fn capability_count(&self) -> usize {
        self.capability_count
    }
    pub fn capabilities(&self) -> CapabilityCursorV3<'view, 'wire> {
        CapabilityCursorV3 {
            table: self.table,
            position: self.capabilities_start,
            remaining: self.capability_count,
        }
    }
    pub fn arguments(&self) -> ArgumentCursorV3<'view, 'wire> {
        ArgumentCursorV3 {
            table: self.table,
            position: self.arguments_start,
            remaining: self.argument_count,
            end: self.end,
        }
    }
}

pub struct CapabilityCursorV3<'view, 'wire> {
    pub(crate) table: &'view DeviceDescriptorTableV3<'wire>,
    pub(crate) position: usize,
    pub(crate) remaining: usize,
}

impl CapabilityCursorV3<'_, '_> {
    pub fn next<E>(
        &mut self,
        charge: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Option<CapabilityV1>, DescriptorWireErrorV3<E>> {
        crate::wire_v3::next_capability(self, charge)
    }
}

pub struct ArgumentCursorV3<'view, 'wire> {
    pub(crate) table: &'view DeviceDescriptorTableV3<'wire>,
    pub(crate) position: usize,
    pub(crate) remaining: usize,
    pub(crate) end: usize,
}

impl<'view, 'wire> ArgumentCursorV3<'view, 'wire> {
    pub fn next<E>(
        &mut self,
        charge: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Option<LogicalArgumentRefV3<'view, 'wire>>, DescriptorWireErrorV3<E>> {
        crate::wire_v3::next_argument(self, charge)
    }
}

pub struct LogicalArgumentRefV3<'view, 'wire> {
    pub(crate) table: &'view DeviceDescriptorTableV3<'wire>,
    pub(crate) source_index: u16,
    pub(crate) name: &'wire str,
    pub(crate) source_type: RustTypeIdentity,
    pub(crate) device_layout: DeviceLayoutIdentity,
    pub(crate) ownership: OwnershipSemantics,
    pub(crate) access: AccessMode,
    pub(crate) alias: AliasSemantics,
    pub(crate) components_start: usize,
    pub(crate) component_count: usize,
}

impl<'view, 'wire> LogicalArgumentRefV3<'view, 'wire> {
    pub const fn source_index(&self) -> u16 {
        self.source_index
    }
    pub const fn name(&self) -> &'wire str {
        self.name
    }
    pub const fn source_type(&self) -> RustTypeIdentity {
        self.source_type
    }
    pub const fn device_layout(&self) -> DeviceLayoutIdentity {
        self.device_layout
    }
    pub const fn ownership(&self) -> OwnershipSemantics {
        self.ownership
    }
    pub const fn access(&self) -> AccessMode {
        self.access
    }
    pub const fn alias(&self) -> AliasSemantics {
        self.alias
    }
    pub const fn component_count(&self) -> usize {
        self.component_count
    }
    pub fn component<E>(
        &self,
        index: usize,
        charge: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<PhysicalComponentV3, DescriptorWireErrorV3<E>> {
        crate::wire_v3::component(self, index, charge)
    }
}

/// The complete retained view includes every slot of its fixed inline index.
pub const DESCRIPTOR_TABLE_VIEW_STORAGE_V3: usize = size_of::<DeviceDescriptorTableV3<'static>>();
/// Caller prepays this temporary extent, in addition to the returned view.
pub const DESCRIPTOR_READER_SCRATCH_STORAGE_V3: usize = crate::wire_v3::READER_SCRATCH_BYTES;
/// One simultaneous kernel/cursor/argument/component/record traversal.
/// Includes standalone record-construction and table-digest scratch. Retained
/// results remain paid until dropped; this is not authority or a hard RSS cap.
pub const DESCRIPTOR_QUERY_STORAGE_V3: usize = crate::wire_v3::QUERY_SCRATCH_BYTES;
/// Borrowed input header and simultaneous full preflight/encode scratch.
pub const DESCRIPTOR_ENCODER_SCRATCH_STORAGE_V3: usize = crate::wire_v3::ENCODER_SCRATCH_BYTES;

/// Validates all borrowed rows and returns the exact caller-buffer length.
/// Caller prepays ENCODER scratch; input backing remains caller-owned and live.
pub fn encoded_device_descriptor_table_v3_len<E>(
    input: &DeviceDescriptorTableInputV3<'_>,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<usize, DescriptorWireErrorV3<E>> {
    crate::wire_v3::preflight(input, charge).map(|(n, _)| n)
}

/// All fallible checks precede the first write; denial preserves the full output.
/// Caller prepays its output backing and ENCODER scratch before entry.
pub fn encode_device_descriptor_table_v3<E>(
    input: &DeviceDescriptorTableInputV3<'_>,
    output: &mut [u8],
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<(), DescriptorWireErrorV3<E>> {
    let (n, target) = crate::wire_v3::preflight(input, charge)?;
    if output.len() != n {
        return Err(DescriptorWireErrorV3::OutputLength {
            expected: n,
            actual: output.len(),
        });
    }
    // Every emitted byte and every visited bounded row is included before mutation.
    pay(charge, n * 2 + 1)?;
    let mut writer = Output {
        bytes: output,
        position: 0,
    };
    writer.bytes(&DEVICE_DESCRIPTOR_MAGIC);
    writer.u16(DEVICE_DESCRIPTOR_VERSION_V3);
    writer.u16(0);
    writer.u32(n as u32);
    writer.bytes(input.canonical_code_object_digest.as_bytes());
    writer.bytes(&[
        crate::encode::code_object_version_tag(input.code_object_version),
        8,
        1,
        0,
    ]);
    writer.text(input.compiler.name().as_str());
    writer.text(input.compiler.release().as_str());
    writer.bytes(input.compiler.commit());
    writer.text(input.producer.name().as_str());
    writer.text(input.producer.version().as_str());
    writer.u16(target.length as u16);
    writer.bytes(&target.bytes[..target.length]);
    writer.u16(input.type_records.len() as u16);
    writer.u16(input.layout_records.len() as u16);
    writer.u16(input.kernels.len() as u16);
    writer.u16(input.requirements.len() as u16);
    for row in input.type_records {
        writer.bytes(row.identity().as_bytes());
        writer.bytes(&row.descriptor().canonical_payload());
    }
    for row in input.layout_records {
        writer.bytes(row.identity().as_bytes());
        writer.bytes(&layout_payload(row.descriptor()));
    }
    for k in input.kernels {
        writer.kernel(k);
    }
    for req in input.requirements {
        writer.bytes(&crate::wire_v2::requirement_bytes(*req));
    }
    Ok(())
}

struct Output<'a> {
    bytes: &'a mut [u8],
    position: usize,
}
impl Output<'_> {
    fn bytes(&mut self, bytes: &[u8]) {
        let end = self.position + bytes.len();
        self.bytes[self.position..end].copy_from_slice(bytes);
        self.position = end;
    }
    fn u16(&mut self, n: u16) {
        self.bytes(&n.to_le_bytes());
    }
    fn u32(&mut self, n: u32) {
        self.bytes(&n.to_le_bytes());
    }
    fn text(&mut self, s: &str) {
        self.u16(s.len() as u16);
        self.bytes(s.as_bytes());
    }
    fn evidence(&mut self, tag: u8, e: BuildEvidenceV1) {
        self.bytes(&[tag, 1, 1, 0]);
        self.bytes(e.identity().as_bytes());
        self.bytes(e.digest().as_bytes());
    }
    fn kernel(&mut self, k: &KernelDescriptorInputV3<'_>) {
        self.bytes(k.kernel_id.as_bytes());
        self.text(k.logical_name);
        self.text(k.entry_name);
        self.text(k.descriptor_symbol);
        self.evidence(1, k.source_evidence);
        self.evidence(2, k.executable_ir_evidence);
        self.u16(k.capabilities.len() as u16);
        for cap in k.capabilities {
            self.u16(crate::encode::capability_tag(*cap));
        }
        let (tag, dims) = match k.launch.block_size() {
            BlockSizeV1::Any => (0, [0; 3]),
            BlockSizeV1::Exact(d) => (1, [d.x(), d.y(), d.z()]),
            BlockSizeV1::AtMost(d) => (2, [d.x(), d.y(), d.z()]),
        };
        self.bytes(&[k.launch.rank(), tag, 0, 0]);
        for n in dims {
            self.u32(n);
        }
        let grid = k.launch.max_grid();
        for n in [
            grid.x(),
            grid.y(),
            grid.z(),
            k.launch.max_flat_workgroup_size(),
            k.launch.static_shared_memory_bytes(),
            k.launch.max_dynamic_shared_memory_bytes(),
        ] {
            self.u32(n);
        }
        self.u16(k.arguments.len() as u16);
        self.u16(
            k.arguments
                .iter()
                .map(|a| a.components.len())
                .sum::<usize>() as u16,
        );
        self.u32(k.abi_layout.explicit_argument_size());
        self.u32(k.abi_layout.kernarg_segment_size());
        self.u32(k.abi_layout.kernarg_segment_alignment());
        for a in k.arguments {
            self.u16(a.source_index);
            self.u16(0);
            self.text(a.name);
            self.bytes(a.source_type.as_bytes());
            self.bytes(a.device_layout.as_bytes());
            self.bytes(&[
                crate::encode::ownership_tag(a.ownership),
                crate::encode::access_tag(a.access),
                crate::encode::alias_tag(a.alias),
                0,
            ]);
            self.u16(a.components.len() as u16);
            self.u16(0);
            for p in a.components {
                let (kind, scalar) = match p.kind {
                    PhysicalAbiComponentKind::ScalarByValue(s) => (1, crate::encode::scalar_tag(s)),
                    PhysicalAbiComponentKind::GlobalPointer => (2, 0),
                    PhysicalAbiComponentKind::SliceLengthU64 => {
                        (3, crate::encode::scalar_tag(ScalarTypeV1::U64))
                    }
                };
                self.bytes(&[
                    kind,
                    scalar,
                    crate::encode::access_tag(p.access),
                    crate::encode::alias_tag(p.alias),
                ]);
                self.u32(p.offset);
                self.u16(p.size);
                self.u16(p.alignment);
                self.u16(0);
                self.u16(0);
            }
        }
    }
}

pub(crate) const OUTPUT_HEADER_STORAGE: usize = size_of::<Output<'static>>();
