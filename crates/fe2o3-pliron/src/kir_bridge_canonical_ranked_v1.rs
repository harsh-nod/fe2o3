//! Private read-only projection of the authoritative neutral graph.
use super::*;
use crate::production_analysis::CanonicalRankedPolicyFailureV1 as Failure;
use pliron::builtin::{attributes::IdentifierAttr, op_interfaces::ATTR_KEY_SYM_NAME};

type Budget<'w> = CanonicalKernelIrVerificationResourceBudgetV1<'w>;
type Resource = CanonicalKernelIrVerificationResourceErrorV1;

pub(crate) struct NativeCanonicalRankedProjectionV1<'g> {
    graph: KirPlironGraphV12<'g>,
    witness: NativeBridgeWitnessV1,
    epoch: u64,
}

fn unsupported(function: usize, block: Option<usize>, operation: Option<usize>) -> Failure {
    Failure::UnsupportedGraph {
        function,
        block,
        operation,
    }
}

fn scalar(ty: &Type) -> bool {
    matches!(ty, Type::Unit | Type::Scalar(_))
}

fn profile(
    input: &VerifiedCanonicalKernelIrModuleV12,
    budget: &mut Budget<'_>,
) -> Result<(), Failure> {
    for (fi, function) in input.module().functions.iter().enumerate() {
        budget.charge_work(1)?;
        for ty in function
            .signature
            .parameters
            .iter()
            .chain(&function.signature.results)
        {
            budget.charge_work(1)?;
            if !scalar(ty) {
                return Err(unsupported(fi, None, None));
            }
        }
        let body = function
            .body
            .as_ref()
            .ok_or_else(|| unsupported(fi, None, None))?;
        for (bi, block) in body.blocks.iter().enumerate() {
            budget.charge_work(1)?;
            for value in &block.parameters {
                budget.charge_work(1)?;
                if !scalar(&value.ty) {
                    return Err(unsupported(fi, Some(bi), None));
                }
            }
            for (oi, operation) in block.operations.iter().enumerate() {
                budget.charge_work(1)?;
                if !matches!(
                    operation.kind,
                    OperationKind::Constant(_)
                        | OperationKind::Unary { .. }
                        | OperationKind::Binary { .. }
                        | OperationKind::Compare { .. }
                        | OperationKind::Cast { .. }
                        | OperationKind::Select { .. }
                ) {
                    return Err(unsupported(fi, Some(bi), Some(oi)));
                }
                for result in &operation.results {
                    budget.charge_work(1)?;
                    if !scalar(&result.ty) {
                        return Err(unsupported(fi, Some(bi), Some(oi)));
                    }
                }
            }
            if !matches!(
                block.terminator,
                Some(
                    Terminator::Branch { .. }
                        | Terminator::ConditionalBranch { .. }
                        | Terminator::Return { .. }
                )
            ) {
                return Err(unsupported(fi, Some(bi), None));
            }
        }
    }
    Ok(())
}

fn epoch(context: &Context) -> Result<u64, Failure> {
    context
        .ir_mutation_attempt_epoch()
        .map(|e| e.value())
        .map_err(|_| Failure::Mutation)
}

fn schema(
    context: &Context,
    pointer: Ptr<Operation>,
    expected: &[&str],
    budget: &mut Budget<'_>,
) -> Result<(), Failure> {
    let raw = pointer.deref(context);
    budget.charge_work(1)?;
    if raw.attributes.0.len() != expected.len() {
        return Err(Failure::NativeSchema);
    }
    for key in raw.attributes.0.keys() {
        let name: &str = key.as_ref();
        budget.charge_work(checked_bridge_add_v12(name.len(), expected.len())?)?;
        if !expected.contains(&name) {
            return Err(Failure::NativeSchema);
        }
    }
    Ok(())
}

fn symbol<'a>(raw: &'a Operation) -> Result<&'a str, Failure> {
    raw.attributes
        .get::<IdentifierAttr>(&ATTR_KEY_SYM_NAME)
        .map(|name| name.as_ref().as_ref())
        .ok_or(Failure::NativeSchema)
}

fn generated_symbol(name: &str, ordinal: usize) -> bool {
    let Some(digits) = name.strip_prefix("kir_fn_") else {
        return false;
    };
    !digits.is_empty()
        && (digits == "0" || !digits.starts_with('0'))
        && digits.bytes().all(|byte| byte.is_ascii_digit())
        && digits.parse::<usize>() == Ok(ordinal)
}

impl<'g> NativeCanonicalRankedProjectionV1<'g> {
    pub(crate) fn import(
        input: &'g VerifiedCanonicalKernelIrModuleV12,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Failure> {
        profile(input, budget)?;
        budget.reserve_storage(std::mem::size_of::<Self>())?;
        let (graph, witness) = import_native_neutral_v1(input, budget)?;
        let epoch = epoch(&graph.session.context)?;
        let mut result = Self {
            graph,
            witness,
            epoch,
        };
        result.check(budget)?;
        Ok(result)
    }

    pub(crate) fn owner(&self) -> &'g VerifiedCanonicalKernelIrModuleV12 {
        self.graph.source
    }

    pub(crate) fn check_epoch(&self) -> Result<(), Failure> {
        if epoch(&self.graph.session.context)? != self.epoch {
            return Err(Failure::Mutation);
        }
        Ok(())
    }

    fn check_schema(&self, budget: &mut Budget<'_>) -> Result<(), Failure> {
        self.graph.validate_custody_v12()?;
        let context = &self.graph.session.context;
        let root = self.graph.session.operations[&self.graph.root.identity];
        schema(context, root, &["sym_name"], budget)?;
        let raw = root.deref(context);
        if symbol(&raw)? != "kir_bridge_v12"
            || raw.num_regions() != 1
            || raw.get_num_operands() != 0
            || raw.get_num_results() != 0
            || raw.get_num_successors() != 0
        {
            return Err(Failure::NativeSchema);
        }
        let region = raw.get_region(0);
        let region = region.deref(context);
        let mut blocks = region.iter(context);
        let block = blocks.next().ok_or(Failure::NativeSchema)?;
        if blocks.next().is_some()
            || block.deref(context).get_num_arguments() != 0
            || !block.deref(context).attributes.0.is_empty()
        {
            return Err(Failure::NativeSchema);
        }
        let mut functions = 0;
        for function in block.deref(context).iter(context) {
            budget.charge_work(1)?;
            schema(context, function, &["sym_name", "func_type"], budget)?;
            let raw = function.deref(context);
            let name = symbol(&raw)?;
            budget.charge_work(name.len())?;
            if !generated_symbol(name, functions)
                || !Operation::is_op::<FuncOp>(function, context)
                || raw.num_regions() != 1
                || raw.get_num_operands() != 0
                || raw.get_num_results() != 0
                || raw.get_num_successors() != 0
            {
                return Err(Failure::NativeSchema);
            }
            for block in raw.get_region(0).deref(context).iter(context) {
                budget.charge_work(1)?;
                if !block.deref(context).attributes.0.is_empty() {
                    return Err(Failure::NativeSchema);
                }
                for operation in block.deref(context).iter(context) {
                    budget.charge_work(1)?;
                    let expected: &[&str] =
                        if Operation::is_op::<PlironConstantOp>(operation, context) {
                            &["gpu_constant_value"]
                        } else if Operation::is_op::<PlironUnaryOp>(operation, context) {
                            &["gpu_unary_kind"]
                        } else if Operation::is_op::<PlironBinaryOp>(operation, context) {
                            &["gpu_binary_kind"]
                        } else if Operation::is_op::<PlironCompareOp>(operation, context) {
                            &["gpu_compare_predicate"]
                        } else if Operation::is_op::<CastOp>(operation, context) {
                            &["gpu_cast_kind"]
                        } else if Operation::is_op::<CondBranchOp>(operation, context) {
                            &["operand_segment_sizes"]
                        } else if Operation::is_op::<PlironSelectOp>(operation, context)
                            || Operation::is_op::<BranchOp>(operation, context)
                            || Operation::is_op::<ReturnOp>(operation, context)
                        {
                            &[]
                        } else {
                            return Err(Failure::NativeSchema);
                        };
                    schema(context, operation, expected, budget)?;
                    if operation.deref(context).num_regions() != 0 {
                        return Err(Failure::NativeSchema);
                    }
                }
            }
            functions += 1;
        }
        if functions != self.owner().module().functions.len() {
            return Err(Failure::NativeSchema);
        }
        // Kernel order/roles are canonical metadata, not native symbol names.
        // Join each ordered root to its actual imported definition explicitly;
        // roundtrip extraction copies this metadata and cannot certify the join.
        for kernel in &self.owner().module().kernels {
            budget.charge_work(checked_bridge_add_v12(1, kernel.entry.as_str().len())?)?;
            let mut definition = None;
            for (ordinal, function) in self.owner().module().functions.iter().enumerate() {
                budget.charge_work(checked_bridge_add_v12(1, function.id.as_str().len())?)?;
                if function.id == kernel.entry {
                    if function.role != fe2o3_kernel_ir::FunctionRole::KernelEntry {
                        return Err(Failure::NativeSchema);
                    }
                    definition = Some(ordinal);
                    break;
                }
            }
            let ordinal = definition.ok_or(Failure::NativeSchema)?;
            budget.charge_work(ordinal.checked_add(1).ok_or(Resource::Arithmetic)?)?;
            let live = block
                .deref(context)
                .iter(context)
                .nth(ordinal)
                .ok_or(Failure::NativeSchema)?;
            let raw = live.deref(context);
            let name = symbol(&raw)?;
            budget.charge_work(name.len())?;
            if !generated_symbol(name, ordinal) {
                return Err(Failure::NativeSchema);
            }
        }
        Ok(())
    }

    pub(crate) fn check(&mut self, budget: &mut Budget<'_>) -> Result<(), Failure> {
        self.check_epoch()?;
        self.check_schema(budget)?;
        let floor = budget.storage();
        let extracted = self
            .graph
            .extract_admitted_inner_v1(budget, true, Some(&self.witness))?;
        let expected = self.owner().canonical().canonical_bytes();
        let actual = extracted.0.canonical().canonical_bytes();
        budget.charge_work(checked_bridge_add_v12(expected.len(), actual.len())?)?;
        let same = expected == actual;
        // The exact extracted owner/report are comparison operands, not another
        // editable verification subject. Their retained credits precede drop.
        drop(extracted);
        restore_bridge_floor_v12(budget, floor)?;
        if !same {
            return Err(Failure::ExactGraph);
        }
        self.check_epoch()
    }

    pub(crate) fn with_function<T>(
        &self,
        ordinal: usize,
        budget: &mut Budget<'_>,
        run: impl FnOnce(&Context, &FuncOp) -> T,
    ) -> Result<T, Failure> {
        self.check_epoch()?;
        budget.charge_work(ordinal.checked_add(1).ok_or(Resource::Arithmetic)?)?;
        let context = &self.graph.session.context;
        let root = self.graph.session.operations[&self.graph.root.identity];
        let region = root.deref(context).get_region(0);
        let block = region
            .deref(context)
            .iter(context)
            .next()
            .ok_or(Failure::NativeSchema)?;
        let pointer = block
            .deref(context)
            .iter(context)
            .nth(ordinal)
            .ok_or(Failure::NativeSchema)?;
        let function =
            Operation::get_op::<FuncOp>(pointer, context).ok_or(Failure::NativeSchema)?;
        Ok(run(context, &function))
    }

    #[cfg(test)]
    pub(crate) fn test_live<T>(&self, run: impl FnOnce(&Context, Ptr<Operation>) -> T) -> T {
        run(
            &self.graph.session.context,
            self.graph.session.operations[&self.graph.root.identity],
        )
    }

    #[cfg(test)]
    pub(crate) fn test_rebase_epoch(&mut self) {
        // Hostile-content tests deliberately test schema/extraction independently
        // from the separately tested whole-context mutation detector.
        self.epoch = epoch(&self.graph.session.context).unwrap();
    }
}
