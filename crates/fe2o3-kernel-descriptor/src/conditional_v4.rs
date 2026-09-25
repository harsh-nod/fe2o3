//! Mandatory conditional transport in the existing nominal descriptor family.
//! V4 is inert. It is not a clean analysis, admitted artifact, or launch token.
use crate::conditional_invocation::{
    ConditionalInvocationContractV1, ConditionalInvocationWireErrorV1,
};
use crate::*;
use std::{error::Error, fmt};

pub const DEVICE_DESCRIPTOR_VERSION_V4: u16 = 4;
pub const CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V4: usize = 16;
pub const DEVICE_DESCRIPTOR_TABLE_DOMAIN_V4: &[u8] = b"FE2O3/DEVICE-DESCRIPTOR-TABLE/V4\0";

/// Reuses nominal records, ABI layout and per-entry target requirements. Every
/// compiler-determined entry has exactly one contract in the same kernel order.
/// The common input is caller data, not an admitted V3 table or downgrade route.
pub struct DeviceDescriptorTableInputV4<'a> {
    pub nominal: DeviceDescriptorTableInputV3<'a>,
    pub contracts: &'a [ConditionalInvocationContractV1<'a>],
}

#[derive(Debug)]
pub enum DescriptorWireErrorV4<E> {
    Nominal(DescriptorWireErrorV3<E>),
    Contract(ConditionalInvocationWireErrorV1<E>),
    Invalid(&'static str),
    OutputLength { expected: usize, actual: usize },
}
impl<E> From<DescriptorWireErrorV3<E>> for DescriptorWireErrorV4<E> {
    fn from(e: DescriptorWireErrorV3<E>) -> Self {
        Self::Nominal(e)
    }
}
impl<E> From<ConditionalInvocationWireErrorV1<E>> for DescriptorWireErrorV4<E> {
    fn from(e: ConditionalInvocationWireErrorV1<E>) -> Self {
        Self::Contract(e)
    }
}
impl<E: fmt::Display> fmt::Display for DescriptorWireErrorV4<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nominal(e) => e.fmt(f),
            Self::Contract(e) => e.fmt(f),
            Self::Invalid(field) => write!(f, "invalid V4 descriptor {field}"),
            Self::OutputLength { expected, actual } => {
                write!(f, "V4 output length {actual}, expected {expected}")
            }
        }
    }
}
impl<E: Error + 'static> Error for DescriptorWireErrorV4<E> {}
pub(crate) type ResultV4<T, E> = Result<T, DescriptorWireErrorV4<E>>;

/// No public V3 table accessor or conversion: the mandatory contract cannot be
/// discarded by interpreting this table through the older transport API.
///
/// ```compile_fail
/// use fe2o3_kernel_descriptor::{DeviceDescriptorTableV3, DeviceDescriptorTableV4};
/// fn downgrade<'a>(table: DeviceDescriptorTableV4<'a>) -> DeviceDescriptorTableV3<'a> {
///     table.into()
/// }
/// ```
pub struct DeviceDescriptorTableV4<'wire> {
    pub(crate) nominal: DeviceDescriptorTableV3<'wire>,
    pub(crate) contracts: [(usize, usize); MAX_KERNELS],
}
impl<'wire> DeviceDescriptorTableV4<'wire> {
    pub fn canonical_bytes(&self) -> &'wire [u8] {
        self.nominal.canonical_bytes()
    }
    pub fn canonical_code_object_digest(&self) -> CanonicalCodeObjectDigest {
        self.nominal.canonical_code_object_digest()
    }
    pub fn code_object_version(&self) -> CodeObjectVersion {
        self.nominal.code_object_version()
    }
    pub fn device_target(&self) -> DeviceTargetV1 {
        self.nominal.device_target()
    }
    pub fn compiler_name(&self) -> &'wire str {
        self.nominal.compiler_name()
    }
    pub fn compiler_release(&self) -> &'wire str {
        self.nominal.compiler_release()
    }
    pub fn compiler_commit(&self) -> &[u8; 20] {
        self.nominal.compiler_commit()
    }
    pub fn producer_name(&self) -> &'wire str {
        self.nominal.producer_name()
    }
    pub fn producer_version(&self) -> &'wire str {
        self.nominal.producer_version()
    }
    pub fn kernel_count(&self) -> usize {
        self.nominal.kernel_count()
    }
    pub fn type_count(&self) -> usize {
        self.nominal.type_count()
    }
    pub fn layout_count(&self) -> usize {
        self.nominal.layout_count()
    }
    pub fn source_type<E>(
        &self,
        identity: RustTypeIdentity,
        c: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV4<SourceTypeRecordV3, E> {
        Ok(self.nominal.source_type(identity, c)?)
    }
    pub fn device_layout<E>(
        &self,
        identity: DeviceLayoutIdentity,
        c: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV4<DeviceLayoutRecordV1, E> {
        Ok(self.nominal.device_layout(identity, c)?)
    }
    pub fn requirement<E>(
        &self,
        index: usize,
        c: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV4<KernelTargetRequirementsV2, E> {
        Ok(self.nominal.requirement(index, c)?)
    }
    pub fn table_digest<E>(
        &self,
        c: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV4<DeviceDescriptorTableDigest, E> {
        crate::nominal_v3::pay(
            c,
            DEVICE_DESCRIPTOR_TABLE_DOMAIN_V4.len() + 8 + self.canonical_bytes().len(),
        )?;
        Ok(DeviceDescriptorTableDigest::from_bytes(
            crate::digest::domain_hash(DEVICE_DESCRIPTOR_TABLE_DOMAIN_V4, self.canonical_bytes()),
        ))
    }
    pub fn kernel<E>(
        &self,
        index: usize,
        c: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV4<KernelDescriptorRefV4<'_, 'wire>, E> {
        Ok(KernelDescriptorRefV4 {
            nominal: self.nominal.kernel(index, c)?,
            table: self,
            index,
        })
    }
    pub fn find_kernel<E>(
        &self,
        id: KernelId,
        c: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV4<KernelDescriptorRefV4<'_, 'wire>, E> {
        for i in 0..self.kernel_count() {
            let kernel = self.kernel(i, c)?;
            crate::nominal_v3::pay(c, 32)?;
            if kernel.kernel_id() == id {
                return Ok(kernel);
            }
        }
        Err(DescriptorWireErrorV4::Invalid("kernel selector"))
    }
}

pub struct KernelDescriptorRefV4<'view, 'wire> {
    pub(crate) nominal: KernelDescriptorRefV3<'view, 'wire>,
    pub(crate) table: &'view DeviceDescriptorTableV4<'wire>,
    pub(crate) index: usize,
}
impl<'view, 'wire> KernelDescriptorRefV4<'view, 'wire> {
    pub fn kernel_id(&self) -> KernelId {
        self.nominal.kernel_id()
    }
    pub fn logical_name(&self) -> &'wire str {
        self.nominal.logical_name()
    }
    pub fn entry_name(&self) -> &'wire str {
        self.nominal.entry_name()
    }
    pub fn descriptor_symbol(&self) -> &'wire str {
        self.nominal.descriptor_symbol()
    }
    pub fn source_evidence(&self) -> BuildEvidenceV1 {
        self.nominal.source_evidence()
    }
    pub fn executable_ir_evidence(&self) -> BuildEvidenceV1 {
        self.nominal.executable_ir_evidence()
    }
    pub fn launch(&self) -> &LaunchConstraintsV1 {
        self.nominal.launch()
    }
    pub fn abi_layout(&self) -> KernelAbiLayoutV1 {
        self.nominal.abi_layout()
    }
    pub fn argument_count(&self) -> usize {
        self.nominal.argument_count()
    }
    pub fn component_count(&self) -> usize {
        self.nominal.component_count()
    }
    pub fn capability_count(&self) -> usize {
        self.nominal.capability_count()
    }
    /// Common inert nominal rows, not a V3 kernel/table conversion.
    pub fn arguments(&self) -> ArgumentCursorV3<'view, 'wire> {
        self.nominal.arguments()
    }
    pub fn capabilities(&self) -> CapabilityCursorV3<'view, 'wire> {
        self.nominal.capabilities()
    }
    pub fn conditional_contract<E>(
        &self,
        c: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> ResultV4<ConditionalInvocationContractV1<'wire>, E> {
        let (start, length) = self.table.contracts[self.index];
        Ok(
            crate::conditional_invocation::decode_conditional_invocation_contract_v1(
                &self.table.canonical_bytes()[start..start + length],
                c,
            )?,
        )
    }
}

pub const DESCRIPTOR_TABLE_VIEW_STORAGE_V4: usize = size_of::<DeviceDescriptorTableV4<'static>>();
pub const DESCRIPTOR_QUERY_STORAGE_V4: usize = DESCRIPTOR_QUERY_STORAGE_V3
    + crate::conditional_invocation::CONDITIONAL_INVOCATION_CODEC_STORAGE_V1
    + size_of::<KernelDescriptorRefV4<'static, 'static>>()
    + size_of::<crate::wire_v4_join::ArgumentProjection>() * 2;
pub const DESCRIPTOR_READER_SCRATCH_STORAGE_V4: usize = DESCRIPTOR_READER_SCRATCH_STORAGE_V3
    + DESCRIPTOR_QUERY_STORAGE_V4
    + size_of::<[(usize, usize); MAX_KERNELS]>()
    + size_of::<crate::wire_common::Reader<'static>>();
pub const DESCRIPTOR_ENCODER_SCRATCH_STORAGE_V4: usize = DESCRIPTOR_ENCODER_SCRATCH_STORAGE_V3
    + DESCRIPTOR_QUERY_STORAGE_V4
    + size_of::<DeviceDescriptorTableInputV4<'static>>();
