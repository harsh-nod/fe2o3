//! Private structured queries, not a nominal-table downgrade or semantic cast.
use super::{E, R};
use fe2o3_kernel_descriptor::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};

pub(crate) trait TableQuery<'wire> {
    const QUERY_STORAGE: usize;
    const SUFFIX: &'static [u8];
    type Kernel<'view>: KernelQuery<'view, 'wire>
    where
        Self: 'view;
    fn canonical_bytes(&self) -> &'wire [u8];
    fn kernel_count(&self) -> usize;
    fn device_target(&self) -> DeviceTargetV1;
    fn code_object_version(&self) -> CodeObjectVersion;
    fn canonical_code_object_digest(&self) -> CanonicalCodeObjectDigest;
    fn kernel(&self, ordinal: usize, budget: &mut Budget<'_>) -> R<Self::Kernel<'_>>;
    fn source_type(&self, id: RustTypeIdentity, budget: &mut Budget<'_>) -> R<SourceTypeRecordV3>;
    fn device_layout(
        &self,
        id: DeviceLayoutIdentity,
        budget: &mut Budget<'_>,
    ) -> R<DeviceLayoutRecordV1>;
    fn requirement(&self, ordinal: usize, budget: &mut Budget<'_>)
    -> R<KernelTargetRequirementsV2>;
}

pub(crate) trait KernelQuery<'view, 'wire> {
    fn entry_name(&self) -> &'wire str;
    fn descriptor_symbol(&self) -> &'wire str;
    fn kernel_id(&self) -> KernelId;
    fn launch(&self) -> &LaunchConstraintsV1;
    fn abi_layout(&self) -> KernelAbiLayoutV1;
    fn argument_count(&self) -> usize;
    fn component_count(&self) -> usize;
    fn capability_count(&self) -> usize;
    fn arguments(&self) -> ArgumentCursorV3<'view, 'wire>;
    fn capabilities(&self) -> CapabilityCursorV3<'view, 'wire>;
}

// Only common-field queries are shared. V5's public methods return Nominal
// errors for these queries; any future non-nominal refusal remains fail-closed.
fn v5_query_error(error: DescriptorWireErrorV5<Resource>) -> E {
    match error {
        DescriptorWireErrorV5::Nominal(error) => E::Descriptor(error),
        _ => E::Invalid("conditional descriptor common-field query"),
    }
}

macro_rules! table_queries {
    ($table:ident, $kernel:ident, $scratch:ident, $suffix:expr, $error:expr) => {
        impl<'wire> TableQuery<'wire> for $table<'wire> {
            const QUERY_STORAGE: usize = $scratch;
            const SUFFIX: &'static [u8] = $suffix;
            type Kernel<'view>
                = $kernel<'view, 'wire>
            where
                Self: 'view;
            fn canonical_bytes(&self) -> &'wire [u8] {
                self.canonical_bytes()
            }
            fn kernel_count(&self) -> usize {
                self.kernel_count()
            }
            fn device_target(&self) -> DeviceTargetV1 {
                self.device_target()
            }
            fn code_object_version(&self) -> CodeObjectVersion {
                self.code_object_version()
            }
            fn canonical_code_object_digest(&self) -> CanonicalCodeObjectDigest {
                self.canonical_code_object_digest()
            }
            fn kernel(&self, ordinal: usize, budget: &mut Budget<'_>) -> R<Self::Kernel<'_>> {
                self.kernel(ordinal, &mut |w| budget.charge_work(w))
                    .map_err($error)
            }
            fn source_type(
                &self,
                id: RustTypeIdentity,
                budget: &mut Budget<'_>,
            ) -> R<SourceTypeRecordV3> {
                self.source_type(id, &mut |w| budget.charge_work(w))
                    .map_err($error)
            }
            fn device_layout(
                &self,
                id: DeviceLayoutIdentity,
                budget: &mut Budget<'_>,
            ) -> R<DeviceLayoutRecordV1> {
                self.device_layout(id, &mut |w| budget.charge_work(w))
                    .map_err($error)
            }
            fn requirement(
                &self,
                ordinal: usize,
                budget: &mut Budget<'_>,
            ) -> R<KernelTargetRequirementsV2> {
                self.requirement(ordinal, &mut |w| budget.charge_work(w))
                    .map_err($error)
            }
        }
        impl<'view, 'wire> KernelQuery<'view, 'wire> for $kernel<'view, 'wire> {
            fn entry_name(&self) -> &'wire str {
                self.entry_name()
            }
            fn descriptor_symbol(&self) -> &'wire str {
                self.descriptor_symbol()
            }
            fn kernel_id(&self) -> KernelId {
                self.kernel_id()
            }
            fn launch(&self) -> &LaunchConstraintsV1 {
                self.launch()
            }
            fn abi_layout(&self) -> KernelAbiLayoutV1 {
                self.abi_layout()
            }
            fn argument_count(&self) -> usize {
                self.argument_count()
            }
            fn component_count(&self) -> usize {
                self.component_count()
            }
            fn capability_count(&self) -> usize {
                self.capability_count()
            }
            fn arguments(&self) -> ArgumentCursorV3<'view, 'wire> {
                self.arguments()
            }
            fn capabilities(&self) -> CapabilityCursorV3<'view, 'wire> {
                self.capabilities()
            }
        }
    };
}

table_queries!(
    DeviceDescriptorTableV3,
    KernelDescriptorRefV3,
    DESCRIPTOR_QUERY_STORAGE_V3,
    super::PREFIX,
    E::Descriptor
);
table_queries!(
    DeviceDescriptorTableV5,
    KernelDescriptorRefV5,
    DESCRIPTOR_QUERY_STORAGE_V5,
    b"\nmodule asm \".section .fe2o3.kd.v5,\\22\\22,@progbits\"\nmodule asm \".balign 8\"\n",
    v5_query_error
);
