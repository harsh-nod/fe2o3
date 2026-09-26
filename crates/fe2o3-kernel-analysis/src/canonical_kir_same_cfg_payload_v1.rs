//! Exact unchanged module/function/CFG payload, excluding operation sequences.
pub(super) fn check(
    a: &fe2o3_kernel_ir::Module,
    b: &fe2o3_kernel_ir::Module,
) -> Result<(), &'static str> {
    let fe2o3_kernel_ir::Module {
        id,
        functions,
        kernels,
        required_capabilities,
        storage_layouts,
    } = a;
    // O(1) old-profile eligibility, separate from prepaid legacy equality.
    if !storage_layouts.is_empty() || !b.storage_layouts.is_empty() {
        return Err("legacy profile excludes storage layouts");
    }
    if id != &b.id
        || kernels != &b.kernels
        || required_capabilities != &b.required_capabilities
        || functions.len() != b.functions.len()
    {
        return Err("module payload");
    }
    for (a, b) in functions.iter().zip(&b.functions) {
        let fe2o3_kernel_ir::Function {
            id,
            signature,
            role,
            body,
            required_capabilities,
        } = a;
        if id != &b.id
            || signature != &b.signature
            || role != &b.role
            || required_capabilities != &b.required_capabilities
        {
            return Err("function payload");
        }
        match (body, &b.body) {
            (None, None) => {}
            (Some(a), Some(b)) => {
                let fe2o3_kernel_ir::FunctionBody { parameters, blocks } = a;
                if parameters != &b.parameters || blocks.len() != b.blocks.len() {
                    return Err("function body payload");
                }
                for (a, b) in blocks.iter().zip(&b.blocks) {
                    let fe2o3_kernel_ir::BasicBlock {
                        id,
                        parameters,
                        operations: _,
                        terminator,
                    } = a;
                    if id != &b.id || parameters != &b.parameters || terminator != &b.terminator {
                        return Err("exact CFG, parameters and edge occurrences");
                    }
                }
            }
            _ => return Err("declaration/body"),
        }
    }
    Ok(())
}

#[cfg(test)]
mod legacy_storage_schema_tests {
    use fe2o3_kernel_ir::{Module, ScalarType, StorageLayoutKindV1, StorageLayoutV1};

    fn eligible(input: &Module, output: &Module) -> bool {
        super::check(input, output).is_ok()
    }

    #[test]
    fn canonical_kir_same_cfg_payload_v1_refuses_nonempty_storage_tables() {
        let empty = Module::new("ordinary");
        assert!(eligible(&empty, &empty));
        let mut occupied = empty.clone();
        occupied.storage_layouts.push(StorageLayoutV1 {
            size: 1,
            alignment: 1,
            kind: StorageLayoutKindV1::Scalar(ScalarType::U8),
        });
        assert!(!eligible(&occupied, &empty));
        assert!(!eligible(&empty, &occupied));
        // Equal, structurally valid tables are still outside the old profile.
        assert!(!eligible(&occupied, &occupied));
        let mut other = occupied.clone();
        other.storage_layouts[0].kind = StorageLayoutKindV1::Scalar(ScalarType::I8);
        assert!(!eligible(&occupied, &other));
        let renamed = Module::new("different");
        assert!(!eligible(&empty, &renamed));
    }
}
