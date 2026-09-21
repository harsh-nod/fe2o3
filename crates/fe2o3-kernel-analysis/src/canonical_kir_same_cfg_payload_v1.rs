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
    } = a;
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
