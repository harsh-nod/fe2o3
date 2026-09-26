//! Object copies use pointer-preserving LLVM memory intrinsics.

use super::storage_operations_v18::space_v18;
use super::*;
use fe2o3_kernel_ir::{
    MemoryAccess, StorageCopyOverlapV1 as Overlap, StorageOperationV1 as Storage,
};

pub(super) fn declarations(
    output: &mut dyn fmt::Write,
    context: &storage_native_v18::StorageEmissionContextV18<'_>,
    legacy: &BTreeSet<(KernelAddressSpace, KernelAddressSpace)>,
) -> Result<(), LoweringErrors> {
    // Five spaces and two operations are a closed finite declaration domain.
    let mut used = [[[false; 5]; 5]; 2];
    let spaces = [
        KernelAddressSpace::Generic,
        KernelAddressSpace::Global,
        KernelAddressSpace::Workgroup,
        KernelAddressSpace::Constant,
        KernelAddressSpace::Private,
    ];
    for function in &context.owner.module().functions {
        if let Some(body) = &function.body {
            for block in &body.blocks {
                for operation in &block.operations {
                    context.charge(1)?;
                    if let OperationKind::Storage(Storage::CopyObject {
                        source_access,
                        destination_access,
                        overlap,
                        ..
                    }) = operation.kind
                    {
                        let source = spaces
                            .iter()
                            .position(|s| *s == source_access.address_space)
                            .expect("closed spaces");
                        let destination = spaces
                            .iter()
                            .position(|s| *s == destination_access.address_space)
                            .expect("closed spaces");
                        used[usize::from(overlap == Overlap::MayOverlap)][destination][source] =
                            true;
                    }
                }
            }
        }
    }
    for (overlap, destinations) in used.iter().enumerate() {
        for (destination, sources) in destinations.iter().enumerate() {
            for (source, present) in sources.iter().enumerate() {
                context.charge(1)?;
                if !present {
                    continue;
                }
                if overlap == 0 && legacy.contains(&(spaces[destination], spaces[source])) {
                    continue;
                }
                let name = if overlap == 0 { "memcpy" } else { "memmove" };
                let destination = space_v18(spaces[destination]);
                let source = space_v18(spaces[source]);
                writeln!(output, "declare void @llvm.{name}.p{destination}.p{source}.i64(ptr addrspace({destination}) nocapture writeonly, ptr addrspace({source}) nocapture readonly, i64, i1 immarg)").unwrap();
            }
        }
    }
    Ok(())
}

impl FunctionLowerer<'_> {
    pub(super) fn emit_storage_copy(
        &self,
        output: &mut dyn fmt::Write,
        source: ValueId,
        destination: ValueId,
        source_access: MemoryAccess,
        destination_access: MemoryAccess,
        overlap: Overlap,
    ) -> Result<(), LoweringErrors> {
        let context = self.storage_context()?;
        let (_, source_row) = self.storage_address(source)?;
        let (_, destination_row) = self.storage_address(destination)?;
        if source_row.size != destination_row.size
            || source_access.volatile
            || destination_access.volatile
        {
            return Err(context.reject(
                "object copy requires matching extent and qualified nonvolatile endpoints",
            ));
        }
        let name = if overlap == Overlap::MayOverlap {
            "memmove"
        } else {
            "memcpy"
        };
        let src_space = space_v18(source_access.address_space);
        let dst_space = space_v18(destination_access.address_space);
        let source = self.value(source).0;
        let destination = self.value(destination).0;
        writeln!(output, "  call void @llvm.{name}.p{dst_space}.p{src_space}.i64(ptr addrspace({dst_space}) align {} {destination}, ptr addrspace({src_space}) align {} {source}, i64 {}, i1 false)", destination_access.alignment, source_access.alignment, source_row.size).unwrap();
        Ok(())
    }
}

#[cfg(test)]
#[path = "storage_copy_v18_tests.rs"]
mod tests;
