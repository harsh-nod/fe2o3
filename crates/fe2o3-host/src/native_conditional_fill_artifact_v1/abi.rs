//! Closed native V5 source/descriptor ABI, without an older-family projection.
use fe2o3_kernel_descriptor::{
    AccessMode, AliasSemantics, BlockSizeV1, CapabilityV1, CodeObjectVersion,
    DESCRIPTOR_QUERY_STORAGE_V5, DESCRIPTOR_READER_SCRATCH_STORAGE_V5,
    DESCRIPTOR_TABLE_VIEW_STORAGE_V5, DescriptorWireErrorV3, DescriptorWireErrorV5,
    DeviceLayoutDescriptorV1, DeviceTargetV1, DimensionsV1, KernelAbiLayoutV1, OwnershipSemantics,
    PhysicalAbiComponentKind, PhysicalComponentV3, ScalarTypeV1, SourceTypeDescriptorV3,
    decode_device_descriptor_table_v5,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_verifier::{
    NativeConditionalFillProgramErrorV1,
    RecoveredCompilerConditionalNativeSemanticHandoffV5 as Owner,
    check_native_conditional_fill_program_v1,
};
use std::{fmt, mem::size_of};

#[derive(Debug)]
pub enum NativeConditionalFillAbiErrorV1 {
    Resource(Resource),
    Program(NativeConditionalFillProgramErrorV1),
    Descriptor(DescriptorWireErrorV5<Resource>),
    Row(DescriptorWireErrorV3<Resource>),
    Mismatch(&'static str),
}
type Error = NativeConditionalFillAbiErrorV1;
impl From<Resource> for Error {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "native closed-fill ABI: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Exact closed64 ABI borrowing the actual native source/F reconstruction.
/// Retains all V5 conditional contracts through its original owner. No V1
/// descriptor, ordinary receipt, machine theorem or runtime authority is minted.
///
/// ```compile_fail
/// use fe2o3_host::CheckedNativeConditionalFillAbiV1 as Checked;
/// fn escape(value: Checked<'_>) -> Checked<'static> { value }
/// ```
pub struct CheckedNativeConditionalFillAbiV1<'a> {
    owner: &'a Owner,
    kernel_id: [u8; 32],
    logical_name: &'a str,
    entry_name: &'a str,
    descriptor_symbol: &'a str,
    max_grid_x: u32,
}
impl CheckedNativeConditionalFillAbiV1<'_> {
    pub const fn owner(&self) -> &Owner {
        self.owner
    }
    pub const fn kernel_id(&self) -> &[u8; 32] {
        &self.kernel_id
    }
    pub const fn logical_name(&self) -> &str {
        self.logical_name
    }
    pub const fn entry_name(&self) -> &str {
        self.entry_name
    }
    pub const fn descriptor_symbol(&self) -> &str {
        self.descriptor_symbol
    }
    pub const fn max_grid_x(&self) -> u32 {
        self.max_grid_x
    }
    pub const fn grants_runtime_authority(&self) -> bool {
        false
    }

    /// Checks the exact generated Rust u32/Index1D layout before returning the
    /// existing inert numeric packing plan. This neither creates an older-schema
    /// descriptor nor drops any native semantic contract. Input owners stay prepaid;
    /// reserve the returned full plan charge before keeping it.
    pub fn prepare_argument_packing(
        &self,
        generated: &crate::CompilerGeneratedArgumentLayoutV1,
        budget: &mut Budget<'_>,
    ) -> Result<(crate::GeneratedArgumentPackingPlanV1, usize), Error> {
        crate::generated_argument_plan::native_conditional_fill_v1::prepare(self, generated, budget)
    }

    /// Native-only descriptive contract for the exact V5 source, final graph and
    /// descriptor. Runtime premises must still be established from actual packed
    /// invocation storage and independently authenticated execution authority.
    pub fn native_contract_identity(&self, budget: &mut Budget<'_>) -> Result<[u8; 32], Error> {
        use sha2::{Digest, Sha256};
        let owner = self.owner;
        let floor=owner.storage().retained_storage().checked_add(owner.handoff().backing_capacity())
            .and_then(|n|n.checked_add(fe2o3_compiler_ffi::INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DECODE_METADATA_STORAGE_V5))
            .and_then(|n|n.checked_add(size_of::<Self>())).ok_or(Resource::Arithmetic)?;
        let components = [
            owner.handoff().canonical_bytes(),
            owner.output().canonical().canonical_bytes(),
            owner.handoff().capsule().descriptor_bytes(),
        ];
        let work = components
            .iter()
            .try_fold(4096usize, |n, b| n.checked_add(b.len()))
            .ok_or(Resource::Arithmetic)?;
        budget.with_prepaid_scope(floor, 8, work, 4096, |_| {
            let mut hash = Sha256::new();
            hash.update(b"FE2O3/HOST/NATIVE-V5-CLOSED-FILL64-CONTRACT/V1\0");
            for bytes in components {
                hash.update((bytes.len() as u64).to_le_bytes());
                hash.update(bytes);
            }
            Ok(hash.finalize().into())
        })
    }
}

/// Original recovered owner/backing must be prepaid. Returns an unreserved
/// borrowed-view charge. No source graph, conditional report or descriptor is
/// reinterpreted through an older schema; local refusal restores only scratch.
pub fn check_native_conditional_fill_abi_v1<'a>(
    owner: &'a Owner,
    budget: &mut Budget<'_>,
) -> Result<(CheckedNativeConditionalFillAbiV1<'a>, usize), Error> {
    let (program, header) =
        check_native_conditional_fill_program_v1(owner, budget).map_err(Error::Program)?;
    let floor = budget.storage();
    let scratch = DESCRIPTOR_TABLE_VIEW_STORAGE_V5
        .checked_add(DESCRIPTOR_READER_SCRATCH_STORAGE_V5)
        .and_then(|n| {
            n.checked_add(DESCRIPTOR_QUERY_STORAGE_V5 * 2 + header.retained_storage() + 4096)
        })
        .ok_or(Resource::Arithmetic)?;
    budget.with_prepaid_scope(floor, 8, 4096, scratch, |budget| {
        let mut charge = |n| budget.charge_work(n);
        let table = decode_device_descriptor_table_v5(
            owner.handoff().capsule().descriptor_bytes(),
            &mut charge,
        )
        .map_err(Error::Descriptor)?;
        if table.kernel_count() != 1
            || table.code_object_version() != CodeObjectVersion::V6
            || table.device_target()
                != DeviceTargetV1::parse(fe2o3_amd_target::PRODUCTION_GFX942_DEVICE_TARGET_V1)
                    .expect("fixed target")
        {
            return Err(Error::Mismatch("singleton gfx942 CoV6 descriptor"));
        }
        let descriptor = table.kernel(0, &mut charge).map_err(Error::Descriptor)?;
        let source = owner.source().source().source();
        let semantic = source.semantic_ssa().source_semantic();
        let [root] = semantic.roots() else {
            return Err(Error::Mismatch("singleton semantic source"));
        };
        let entry = semantic.functions()[root.index() as usize]
            .kernel_entry()
            .ok_or(Error::Mismatch("source kernel entry"))?;
        let [launch] = source.source_launch().roots() else {
            return Err(Error::Mismatch("singleton source launch"));
        };
        let contract = entry.source_contract();
        let source_launch = contract
            .launch()
            .ok_or(Error::Mismatch("explicit source launch"))?;
        let declared = descriptor.launch();
        if entry.kernel_binding_identity().as_bytes() != descriptor.kernel_id().as_bytes()
            || entry.export_symbol().as_bytes() != descriptor.entry_name().as_bytes()
            || descriptor.entry_name() != program.function_symbol()
            || launch.selected_root() != *root
            || launch.kernel_binding() != *descriptor.kernel_id().as_bytes()
            || launch.source_rank() != 1
            || launch.source_launch().exact_workgroup() != Some([64, 1, 1])
            || launch.source_launch().max_grid()
                != [
                    declared.max_grid().x(),
                    declared.max_grid().y(),
                    declared.max_grid().z(),
                ]
            || source_launch.required().map(|v| v.as_array()) != Some([64, 1, 1])
            || source_launch
                .maximum()
                .is_some_and(|v| v.as_array() != [64, 1, 1])
            || source_launch.min_workgroups_per_compute_unit().is_some()
            || contract.resources().is_some()
            || contract.unsafe_assembly().is_some()
            || contract.reachable_assembly().is_some()
            || declared.rank() != 1
            || declared.block_size()
                != BlockSizeV1::Exact(DimensionsV1::new(64, 1, 1).expect("fixed block"))
            || declared.max_flat_workgroup_size() != 64
            || declared.max_grid().y() != 1
            || declared.max_grid().z() != 1
            || declared.static_shared_memory_bytes() != 0
            || declared.max_dynamic_shared_memory_bytes() != 0
        {
            return Err(Error::Mismatch("exact source/descriptor closed64 launch"));
        }
        if descriptor.abi_layout() != KernelAbiLayoutV1::new(16, 272, 8).expect("fixed ABI")
            || descriptor.argument_count() != 1
            || descriptor.component_count() != 2
            || descriptor.capability_count() != 1
        {
            return Err(Error::Mismatch("closed64 ABI extent"));
        }
        let mut capabilities = descriptor.capabilities();
        if capabilities.next(&mut charge).map_err(Error::Row)? != Some(CapabilityV1::AmdWave)
            || capabilities
                .next(&mut charge)
                .map_err(Error::Row)?
                .is_some()
        {
            return Err(Error::Mismatch("closed64 capability"));
        }
        let mut arguments = descriptor.arguments();
        let argument = arguments
            .next(&mut charge)
            .map_err(Error::Row)?
            .ok_or(Error::Mismatch("closed64 output"))?;
        if arguments.next(&mut charge).map_err(Error::Row)?.is_some()
            || argument.source_index() != 0
            || argument.name() != "arg0"
            || argument.ownership() != OwnershipSemantics::UniqueBorrow
            || argument.access() != AccessMode::WriteOnly
            || argument.alias() != AliasSemantics::Exclusive
            || argument.component_count() != 2
            || table
                .source_type(argument.source_type(), &mut charge)
                .map_err(Error::Descriptor)?
                .descriptor()
                != SourceTypeDescriptorV3::DisjointSlice(ScalarTypeV1::U32)
            || *table
                .device_layout(argument.device_layout(), &mut charge)
                .map_err(Error::Descriptor)?
                .descriptor()
                != DeviceLayoutDescriptorV1::disjoint_slice(ScalarTypeV1::U32)
        {
            return Err(Error::Mismatch("exclusive write-only disjoint u32 slice"));
        }
        let pointer = argument.component(0, &mut charge).map_err(Error::Row)?;
        let length = argument.component(1, &mut charge).map_err(Error::Row)?;
        if !closed_components(pointer, length) {
            return Err(Error::Mismatch("exact pointer/length physical ABI"));
        }
        // Mandatory V2 framing and source/formula binding were established by
        // original V5 recovery; retain that owner, never project a V3 table.
        let _contract = descriptor
            .conditional_contract(&mut charge)
            .map_err(Error::Descriptor)?;
        Ok((
            CheckedNativeConditionalFillAbiV1 {
                owner,
                kernel_id: *descriptor.kernel_id().as_bytes(),
                logical_name: descriptor.logical_name(),
                entry_name: descriptor.entry_name(),
                descriptor_symbol: descriptor.descriptor_symbol(),
                max_grid_x: declared.max_grid().x(),
            },
            size_of::<CheckedNativeConditionalFillAbiV1<'_>>(),
        ))
    })
}

fn closed_components(pointer: PhysicalComponentV3, length: PhysicalComponentV3) -> bool {
    pointer
        == PhysicalComponentV3 {
            kind: PhysicalAbiComponentKind::GlobalPointer,
            offset: 0,
            size: 8,
            alignment: 8,
            access: AccessMode::WriteOnly,
            alias: AliasSemantics::Exclusive,
        }
        && length
            == PhysicalComponentV3 {
                kind: PhysicalAbiComponentKind::SliceLengthU64,
                offset: 8,
                size: 8,
                alignment: 8,
                access: AccessMode::ByValue,
                alias: AliasSemantics::Value,
            }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closed_native_components_reject_every_physical_substitution() {
        let pointer = PhysicalComponentV3 {
            kind: PhysicalAbiComponentKind::GlobalPointer,
            offset: 0,
            size: 8,
            alignment: 8,
            access: AccessMode::WriteOnly,
            alias: AliasSemantics::Exclusive,
        };
        let length = PhysicalComponentV3 {
            kind: PhysicalAbiComponentKind::SliceLengthU64,
            offset: 8,
            size: 8,
            alignment: 8,
            access: AccessMode::ByValue,
            alias: AliasSemantics::Value,
        };
        assert!(closed_components(pointer, length));
        for field in 0..6 {
            let mutate = |mut row: PhysicalComponentV3| {
                match field {
                    0 => row.kind = PhysicalAbiComponentKind::ScalarByValue(ScalarTypeV1::U64),
                    1 => row.offset += 4,
                    2 => row.size = 4,
                    3 => row.alignment = 4,
                    4 => row.access = AccessMode::ReadWrite,
                    _ => row.alias = AliasSemantics::SharedReadOnly,
                }
                row
            };
            assert!(!closed_components(mutate(pointer), length));
            assert!(!closed_components(pointer, mutate(length)));
        }
    }
}
