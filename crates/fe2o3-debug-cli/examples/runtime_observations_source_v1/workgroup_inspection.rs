//! Native V5 inspection of exact source-linked canonical V10, with no simulation or repacking.
use super::common::*;
use fe2o3_kernel_ir::*;
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn kind(value: &OperationKind) -> Result<&'static str, String> {
    Ok(match value {
        OperationKind::Constant(_) => "constant",
        OperationKind::Intrinsic(_) => "intrinsic",
        OperationKind::MemoryIntrinsic(_) => "memory_intrinsic",
        OperationKind::Unary { .. } => "unary",
        OperationKind::Binary { .. } => "binary",
        OperationKind::Compare { .. } => "compare",
        OperationKind::Cast { .. } => "cast",
        OperationKind::Select { .. } => "select",
        OperationKind::Call { .. } => "call",
        OperationKind::SliceLength { .. } => "slice_length",
        OperationKind::SliceData { .. } => "slice_data",
        OperationKind::GetElementPointer { .. } => "get_element_pointer",
        OperationKind::Load { .. } => "load",
        OperationKind::GuardedLoad { .. } => "guarded_load",
        OperationKind::Store { .. } => "store",
        OperationKind::GuardedStore { .. } => "guarded_store",
        OperationKind::Barrier(_) => "barrier",
        OperationKind::Atomic(_) => "atomic",
        OperationKind::Fence(_) => "fence",
        OperationKind::WorkgroupBarrier(_) => "workgroup_barrier",
        OperationKind::WorkgroupMemory(_) => "workgroup_memory",
        OperationKind::Matrix(_) => "matrix",
        OperationKind::Gfx950LdsTranspose(_) => "gfx950_lds_transpose",
        OperationKind::Wave(_) => "wave",
        OperationKind::Alloca { .. } | OperationKind::InlineAssembly(_) => {
            return Err("ordinary WG census refuses private Alloca/inline assembly".into());
        }
        _ => return Err("operation outside bounded V10 WG inspection profile".into()),
    })
}
pub(super) fn inspect(bytes: Vec<u8>) -> Result<Value, String> {
    let digest = hash(&bytes);
    let bundle = VerifiedSimulationBundleV5::from_canonical_bytes(bytes).map_err(fail)?;
    bundle.revalidate().map_err(fail)?;
    demand(
        bundle.target() == "gfx942:xnack-" && bundle.kernel_count() == 1,
        "native V5 target/kernel",
    )?;
    let module = decode_module_v10(bundle.canonical_kir_v10()).map_err(fail)?;
    demand(
        module.kernels.len() == 1
            && module.kernels[0].id.as_str() == "workgroup_reduce_u32"
            && !module.functions.is_empty()
            && module.functions.len() <= 8,
        "native V5 function roster",
    )?;
    let map =
        DebugSourceMapDocumentV2::from_canonical_json_bytes(bundle.debug_map()).map_err(fail)?;
    demand(
        map.binding().bundle_subject_identity() == *bundle.subject_identity()
            && map.binding().canonical_kir().digest() == *bundle.canonical_kir_v10_digest()
            && map.binding().canonical_kir().canonical_bytes() == bundle.canonical_kir_v10_length(),
        "native V5 exact source-map/KIR/subject binding",
    )?;
    demand(
        !map.files().is_empty() && map.files().len() <= 32 && map.sites().len() <= 512,
        "native V5 source-map bounds",
    )?;
    let mut files = BTreeMap::new();
    let mut file_rows = Vec::new();
    for file in map.files() {
        demand(
            file.byte_len() <= 256 * 1024 && file.display_path().len() <= 4096,
            "native V5 file bounds",
        )?;
        demand(
            files.insert(file.identity(), file.byte_len()).is_none(),
            "native V5 unique source file",
        )?;
        file_rows.push(
            json!({"identity":hex(&file.identity()),"bytes":file.byte_len(),
            "display_path":file.display_path()}),
        );
    }
    let mut sites = BTreeMap::new();
    for site in map.sites() {
        let coordinate = site.site();
        let key = [
            coordinate.function_ordinal(),
            coordinate.block_ordinal(),
            coordinate.operation_ordinal(),
        ];
        demand(
            site.spans().len() <= 16 && sites.insert(key, site).is_none(),
            "native V5 unique source site",
        )?;
    }
    let mut operations = Vec::new();
    for (function_index, function) in module.functions.iter().enumerate() {
        demand(
            function.id.as_str().len() <= 4096,
            "native V5 function name bound",
        )?;
        if let Some(body) = &function.body {
            demand(body.blocks.len() <= 64, "native V5 block bound")?;
            for (block_index, block) in body.blocks.iter().enumerate() {
                for (operation_index, operation) in block.operations.iter().enumerate() {
                    demand(operations.len() < 512, "native V5 complete operation bound")?;
                    let key = [
                        function_index as u64,
                        block_index as u64,
                        operation_index as u64,
                    ];
                    let mut spans = Vec::new();
                    if let Some(site) = sites.remove(&key) {
                        for span in site.spans() {
                            let len = files
                                .get(&span.file_identity())
                                .ok_or("native V5 source file absent")?;
                            demand(
                                span.byte_start() <= span.byte_end() && span.byte_end() <= *len,
                                "native V5 span byte bounds",
                            )?;
                            spans.push(json!({"file_identity":hex(&span.file_identity()),
                                "byte_start":span.byte_start().to_string(),"byte_end":span.byte_end().to_string()}));
                        }
                    }
                    operations.push(json!({"coordinate":{"function":function_index,"block":block_index,"operation":operation_index},
                        "runtime_site":[function_index,block.id.0,operation_index],"function_name":function.id.as_str(),
                        "kind":kind(&operation.kind)?,"mnemonic":null,"inline_assembly_source":null,"source_spans":spans}));
                }
            }
        }
    }
    demand(
        !operations.is_empty() && sites.is_empty(),
        "native V5 complete source/operation join",
    )?;
    Ok(
        json!({"schema":"task-runtime-workgroup-native-v5-inspection-v1","status":"passed",
        "bundle_version":5,"canonical_kir_version":10,"target":bundle.target(),
        "bundle_sha256":digest,"bundle_identity":hex(bundle.identity().as_bytes()),
        "canonical_kir_sha256":hash(bundle.canonical_kir_v10()),"canonical_kir_digest":hex(bundle.canonical_kir_v10_digest()),
        "canonical_kir_bytes":bundle.canonical_kir_v10_length(),"source_map_identity":hex(&bundle.debug_map_identity()),
        "source_map_sha256":hash(bundle.debug_map()),"bundle_subject_identity":hex(bundle.subject_identity()),
        "operation_count":operations.len(),"operations":operations,"files":file_rows,
        "source_authenticated":false,"hardware_observed":false,"compiler_resume_authority":false,
        "authority":{"source_authenticated":false,"grants_production_resume":false},
        "scope":"native V5 source-linked inspection only; no conversion, simulation, compiler resume or hardware"}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_or_other_bundle_is_not_repacked_as_v5() {
        assert!(inspect(vec![0; 8]).is_err());
    }
}
