//! Mandatory conditional transport in the existing nominal descriptor family.
//! V5 is inert. It is not a clean analysis, admitted artifact, or launch token.
use crate::conditional_invocation::{
    ConditionalInvocationContractV2, ConditionalInvocationWireErrorV1,
};
use crate::*;
use std::{error::Error, fmt};

/// Outer V5 accepts mandatory invocation V2 contracts only.
pub const DEVICE_DESCRIPTOR_VERSION_V5: u16 = 5;
/// Unchanged nominal-header digest slot.
pub const CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V5: usize = 16;
/// Length-prefixed complete V5 table content hash domain.
pub const DEVICE_DESCRIPTOR_TABLE_DOMAIN_V5: &[u8] = b"FE2O3/DEVICE-DESCRIPTOR-TABLE/V5\0";

/// Reuses nominal records, ABI layout and per-entry target requirements. Every
/// compiler-determined entry has exactly one contract in the same kernel order.
/// The common input is caller data, not an admitted V3 table or downgrade route.
pub struct DeviceDescriptorTableInputV5<'a> {
    /// Shared caller-authored nominal input, not a validated older table.
    pub nominal: DeviceDescriptorTableInputV3<'a>,
    /// Exactly one V2 contract per kernel in canonical kernel order.
    pub contracts: &'a [ConditionalInvocationContractV2<'a>],
}

/// Structural V5 framing or resource-callback refusal.
#[derive(Debug)]
pub enum DescriptorWireErrorV5<E> {
    /// Shared nominal framing/ABI validation failed.
    Nominal(DescriptorWireErrorV3<E>),
    /// Strict invocation V2 decoding or content validation failed.
    Contract(ConditionalInvocationWireErrorV1<E>),
    /// Mandatory outer binding or framing failed.
    Invalid(&'static str),
    /// The caller output has a different extent from complete preflight.
    OutputLength {
        /// Exact required extent.
        expected: usize,
        /// Supplied extent.
        actual: usize,
    },
}
impl<E> From<DescriptorWireErrorV3<E>> for DescriptorWireErrorV5<E> {
    fn from(e: DescriptorWireErrorV3<E>) -> Self {
        Self::Nominal(e)
    }
}
impl<E> From<ConditionalInvocationWireErrorV1<E>> for DescriptorWireErrorV5<E> {
    fn from(e: ConditionalInvocationWireErrorV1<E>) -> Self {
        Self::Contract(e)
    }
}
impl<E: fmt::Display> fmt::Display for DescriptorWireErrorV5<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nominal(e) => e.fmt(f),
            Self::Contract(e) => e.fmt(f),
            Self::Invalid(field) => write!(f, "invalid V5 descriptor {field}"),
            Self::OutputLength { expected, actual } => {
                write!(f, "V5 output length {actual}, expected {expected}")
            }
        }
    }
}
impl<E: Error + 'static> Error for DescriptorWireErrorV5<E> {}
pub(crate) type ResultV5<T, E> = Result<T, DescriptorWireErrorV5<E>>;

/// No public V3 table accessor or conversion: the mandatory contract cannot be
/// discarded by interpreting this table through the older transport API.
///
/// ```compile_fail
/// use fe2o3_kernel_descriptor::{DeviceDescriptorTableV3, DeviceDescriptorTableV5};
/// fn downgrade<'a>(table: DeviceDescriptorTableV5<'a>) -> DeviceDescriptorTableV3<'a> {
///     table.into()
/// }
/// ```
///
/// ```compile_fail,E0308
/// use fe2o3_kernel_descriptor::{DeviceDescriptorTableV4, DeviceDescriptorTableV5};
/// fn relabel<'a>(table: DeviceDescriptorTableV5<'a>) -> DeviceDescriptorTableV4<'a> { table }
/// ```
/// ```compile_fail
/// use fe2o3_kernel_descriptor::{DeviceDescriptorTableV5, decode_device_descriptor_table_v5};
/// fn escape(bytes: Vec<u8>) -> DeviceDescriptorTableV5<'static> {
///     decode_device_descriptor_table_v5(&bytes, &mut |_| Ok::<(), ()>(())).unwrap()
/// }
/// ```
pub struct DeviceDescriptorTableV5<'wire> {
    pub(crate) nominal: DeviceDescriptorTableV3<'wire>,
    pub(crate) contracts: [(usize, usize); MAX_KERNELS],
}
impl<'wire> DeviceDescriptorTableV5<'wire> {
    /// Borrow the exact complete V5 wire bytes.
    pub fn canonical_bytes(&self) -> &'wire [u8] {
        self.nominal.canonical_bytes()
    }
    /// Return the inert code-object digest field.
    pub fn canonical_code_object_digest(&self) -> CanonicalCodeObjectDigest {
        self.nominal.canonical_code_object_digest()
    }
    /// Return the unchanged AMD code-object version.
    pub fn code_object_version(&self) -> CodeObjectVersion {
        self.nominal.code_object_version()
    }
    /// Return the nominal device target.
    pub fn device_target(&self) -> DeviceTargetV1 {
        self.nominal.device_target()
    }
    /// Borrow the claimed compiler name.
    pub fn compiler_name(&self) -> &'wire str {
        self.nominal.compiler_name()
    }
    /// Borrow the claimed compiler release.
    pub fn compiler_release(&self) -> &'wire str {
        self.nominal.compiler_release()
    }
    /// Borrow the claimed compiler commit.
    pub fn compiler_commit(&self) -> &[u8; 20] {
        self.nominal.compiler_commit()
    }
    /// Borrow the claimed producer name.
    pub fn producer_name(&self) -> &'wire str {
        self.nominal.producer_name()
    }
    /// Borrow the claimed producer version.
    pub fn producer_version(&self) -> &'wire str {
        self.nominal.producer_version()
    }
    /// Return the complete ordered kernel count.
    /// Borrow the indexed kernel without discarding its V2 contract.
    pub fn kernel_count(&self) -> usize {
        self.nominal.kernel_count()
    }
    /// Return the source-type row count.
    pub fn type_count(&self) -> usize {
        self.nominal.type_count()
    }
    /// Return the device-layout row count.
    pub fn layout_count(&self) -> usize {
        self.nominal.layout_count()
    }
    /// Query one shared source-type row with the existing nominal debit.
    pub fn source_type<E>(
        &self,
        identity: RustTypeIdentity,
        c: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV5<SourceTypeRecordV3, E> {
        Ok(self.nominal.source_type(identity, c)?)
    }
    /// Query one shared device-layout row with the existing nominal debit.
    pub fn device_layout<E>(
        &self,
        identity: DeviceLayoutIdentity,
        c: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV5<DeviceLayoutRecordV1, E> {
        Ok(self.nominal.device_layout(identity, c)?)
    }
    /// Query the selected kernel's target requirements.
    pub fn requirement<E>(
        &self,
        index: usize,
        c: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV5<KernelTargetRequirementsV2, E> {
        Ok(self.nominal.requirement(index, c)?)
    }
    /// Hash the entire exact table under the V5 content domain.
    pub fn table_digest<E>(
        &self,
        c: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV5<DeviceDescriptorTableDigest, E> {
        crate::nominal_v3::pay(
            c,
            DEVICE_DESCRIPTOR_TABLE_DOMAIN_V5.len() + 8 + self.canonical_bytes().len(),
        )?;
        Ok(DeviceDescriptorTableDigest::from_bytes(
            crate::digest::domain_hash(DEVICE_DESCRIPTOR_TABLE_DOMAIN_V5, self.canonical_bytes()),
        ))
    }
    pub fn kernel<E>(
        &self,
        index: usize,
        c: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV5<KernelDescriptorRefV5<'_, 'wire>, E> {
        Ok(KernelDescriptorRefV5 {
            nominal: self.nominal.kernel(index, c)?,
            table: self,
            index,
        })
    }
    /// Find a kernel using the existing charged nominal search.
    pub fn find_kernel<E>(
        &self,
        id: KernelId,
        c: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV5<KernelDescriptorRefV5<'_, 'wire>, E> {
        for i in 0..self.kernel_count() {
            let kernel = self.kernel(i, c)?;
            crate::nominal_v3::pay(c, 32)?;
            if kernel.kernel_id() == id {
                return Ok(kernel);
            }
        }
        Err(DescriptorWireErrorV5::Invalid("kernel selector"))
    }
}

/// Borrowed V5 kernel; common row queries cannot erase its mandatory contract.
pub struct KernelDescriptorRefV5<'view, 'wire> {
    pub(crate) nominal: KernelDescriptorRefV3<'view, 'wire>,
    pub(crate) table: &'view DeviceDescriptorTableV5<'wire>,
    pub(crate) index: usize,
}
impl<'view, 'wire> KernelDescriptorRefV5<'view, 'wire> {
    /// Return the nominal kernel identity.
    pub fn kernel_id(&self) -> KernelId {
        self.nominal.kernel_id()
    }
    /// Borrow the logical kernel name.
    pub fn logical_name(&self) -> &'wire str {
        self.nominal.logical_name()
    }
    /// Borrow the physical entry name.
    pub fn entry_name(&self) -> &'wire str {
        self.nominal.entry_name()
    }
    /// Borrow the expected descriptor symbol.
    pub fn descriptor_symbol(&self) -> &'wire str {
        self.nominal.descriptor_symbol()
    }
    /// Return the inert source-evidence fields.
    pub fn source_evidence(&self) -> BuildEvidenceV1 {
        self.nominal.source_evidence()
    }
    /// Return the inert executable-evidence fields.
    pub fn executable_ir_evidence(&self) -> BuildEvidenceV1 {
        self.nominal.executable_ir_evidence()
    }
    /// Borrow the nominal launch constraints.
    pub fn launch(&self) -> &LaunchConstraintsV1 {
        self.nominal.launch()
    }
    /// Return the unchanged nominal physical ABI layout.
    pub fn abi_layout(&self) -> KernelAbiLayoutV1 {
        self.nominal.abi_layout()
    }
    /// Return the logical argument count.
    pub fn argument_count(&self) -> usize {
        self.nominal.argument_count()
    }
    /// Return the physical component count.
    pub fn component_count(&self) -> usize {
        self.nominal.component_count()
    }
    /// Return the capability count.
    pub fn capability_count(&self) -> usize {
        self.nominal.capability_count()
    }
    /// Common inert nominal rows, not a V3 kernel/table conversion.
    pub fn arguments(&self) -> ArgumentCursorV3<'view, 'wire> {
        self.nominal.arguments()
    }
    /// Borrow the common inert capability cursor.
    pub fn capabilities(&self) -> CapabilityCursorV3<'view, 'wire> {
        self.nominal.capabilities()
    }
    /// Strictly decode this kernel's exact V2 contract; no proof or source authentication.
    pub fn conditional_contract<E>(
        &self,
        c: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV5<ConditionalInvocationContractV2<'wire>, E> {
        let (start, length) = self.table.contracts[self.index];
        Ok(
            crate::conditional_invocation::decode_conditional_invocation_contract_v2(
                &self.table.canonical_bytes()[start..start + length],
                c,
            )?,
        )
    }
}

/// Complete retained V5 borrowed table extent.
pub const DESCRIPTOR_TABLE_VIEW_STORAGE_V5: usize = size_of::<DeviceDescriptorTableV5<'static>>();
/// Simultaneous nominal/V2 query scratch and typed row projections.
pub const DESCRIPTOR_QUERY_STORAGE_V5: usize = DESCRIPTOR_QUERY_STORAGE_V3
    + crate::conditional_invocation::CONDITIONAL_INVOCATION_CODEC_STORAGE_V2
    + size_of::<KernelDescriptorRefV5<'static, 'static>>()
    + size_of::<crate::wire_v4_join::ArgumentProjection>() * 2;
/// Complete V5 reader scratch, in addition to input backing and returned view.
pub const DESCRIPTOR_READER_SCRATCH_STORAGE_V5: usize = DESCRIPTOR_READER_SCRATCH_STORAGE_V3
    + DESCRIPTOR_QUERY_STORAGE_V5
    + size_of::<[(usize, usize); MAX_KERNELS]>()
    + size_of::<crate::wire_common::Reader<'static>>();
/// Complete V5 encoder scratch, in addition to input and output backing.
pub const DESCRIPTOR_ENCODER_SCRATCH_STORAGE_V5: usize = DESCRIPTOR_ENCODER_SCRATCH_STORAGE_V3
    + DESCRIPTOR_QUERY_STORAGE_V5
    + size_of::<DeviceDescriptorTableInputV5<'static>>();
