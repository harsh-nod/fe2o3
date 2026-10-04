//! Typed evaluation of the existing checked relocation DAG. These are
//! counterfactual pure expression queries, never executable predecessor states.
use super::*;
use crate::mixed_optimizer_refinement_v26::{Error as EmitError, Result as EmitResult, Writer};
use std::fmt::Write as _;

macro_rules! emit {
    ($out:expr, $($arg:tt)*) => {
        write!($out, $($arg)*).map_err(|_| $out.error())?
    };
}

fn emission_error(error: Error) -> EmitError {
    match error {
        Error::Resource(error) => error.into(),
        Error::Inventory(error) => error.into(),
        Error::Flow(error) => error.into(),
        Error::Mismatch(reason) => EmitError::Statement(reason),
    }
}

fn headers() -> usize {
    type Frame = (
        [usize; 12],
        [&'static (); 12],
        [Option<usize>; 2],
        [Range<usize>; 3],
        [EmitResult<()>; 2],
        std::slice::Iter<'static, usize>,
        std::slice::Iter<'static, Operand>,
        std::slice::Iter<'static, ResultBinding>,
        std::slice::Iter<'static, CutBinding>,
    );
    size_of::<Frame>() + align_of::<Frame>()
}

impl RelocationExpressionPlanV28<'_> {
    /// The namespace range is the one used when emitting the exact input
    /// ByteFunctions, with one function namespace per physical function ordinal.
    /// Both inventories and the complete relocation rows remain owner-checked.
    pub(in crate::mixed_optimizer_refinement_v26) fn emit_typed_expressions_v48(
        &self,
        pair: &Pair<'_>,
        input: &Inventory<'_>,
        output: &Inventory<'_>,
        namespace: usize,
        input_namespace_base: usize,
        out: &mut Writer<'_, '_>,
    ) -> EmitResult<()> {
        self.custody(out.budget).map_err(emission_error)?;
        if !std::ptr::eq(input.owner(), self.input) || !std::ptr::eq(output.owner(), self.output) {
            return Err(EmitError::Statement("typed relocation inventory owner"));
        }
        self.replay(pair, out.budget).map_err(emission_error)?;
        input_namespace_base
            .checked_add(input.functions().len())
            .ok_or(Resource::Arithmetic)?;
        out.budget.reserve_storage(headers())?;
        let result = self.emit_typed_inner_v48(input, output, namespace, input_namespace_base, out);
        let cleanup = out.budget.release_storage(headers());
        result?;
        cleanup?;
        self.custody(out.budget).map_err(emission_error)
    }

    fn emit_typed_inner_v48(
        &self,
        input: &Inventory<'_>,
        output: &Inventory<'_>,
        namespace: usize,
        input_namespace_base: usize,
        out: &mut Writer<'_, '_>,
    ) -> EmitResult<()> {
        let definitions = input.definitions().len();
        for &node_index in &self.order {
            out.budget.charge_work(12)?;
            let node = &self.nodes[node_index];
            let actual = &input.operations()[node.input];
            let block = block_index(input, actual.coordinate.block).map_err(emission_error)?;
            let input_namespace = input_namespace_base
                .checked_add(actual.coordinate.block.function.0 as usize)
                .ok_or(Resource::Arithmetic)?;
            emit!(
                out,
                "spec fn typed_relocated_expression_{namespace}_{node_index}_v48(base: MemoryStateV30, little_endian: bool) -> Option<Seq<MemoryValueV30>> {{\n if !base.valid || base.values.len() != {definitions} {{ None }} else {{\n"
            );
            for (ordinal, operand) in self.operands[node.operands.clone()].iter().enumerate() {
                out.budget.charge_work(3)?;
                if let Some((dependency, result)) = operand.expression {
                    emit!(
                        out,
                        " let operand{ordinal} = match typed_relocated_expression_{namespace}_{dependency}_v48(base, little_endian) {{ Some(values) => if {result} < values.len() {{ Some(values[{result}]) }} else {{ None }}, None => None }};\n"
                    );
                } else {
                    emit!(
                        out,
                        " let operand{ordinal} = Some(base.values[{}]);\n",
                        operand.input
                    );
                }
            }
            emit!(out, " if true");
            for ordinal in 0..node.operands.len() {
                out.budget.charge_work(1)?;
                emit!(out, " && operand{ordinal}.is_some()");
            }
            emit!(out, " {{\n let values = base.values");
            for (ordinal, operand) in self.operands[node.operands.clone()].iter().enumerate() {
                out.budget.charge_work(2)?;
                emit!(out, ".update({}, operand{ordinal}.unwrap())", operand.input);
            }
            emit!(
                out,
                ";\n let probe = MemoryStateV30 {{ values, pc: {block}, ..base }};\n let evaluated = byte_operation_{input_namespace}_{}_v30(probe, little_endian);\n if evaluated.state.valid && evaluated.state.pc == probe.pc && evaluated.state.values.len() == {definitions}\n && evaluated.state.memory == probe.memory && evaluated.state.generations == probe.generations && evaluated.state.frames == probe.frames\n && evaluated.observation.before == probe && evaluated.observation.after == evaluated.state\n && evaluated.observation.valid_before && evaluated.observation.valid_after && matches!(evaluated.observation.effect, MemoryOperationEffectV30::Pure) {{ Some(seq![",
                node.input
            );
            for result in &self.results[node.results.clone()] {
                out.budget.charge_work(1)?;
                emit!(out, "evaluated.state.values[{}],", result.input);
            }
            emit!(out, "]) }} else {{ None }}\n }} else {{ None }}\n }}\n}}\n");
        }

        // These selectors enumerate only the existing independently replayed
        // DAG and dominance cuts. They do not make unavailable input results
        // defined or assert that a counterfactual evaluation actually ran.
        emit!(
            out,
            "spec fn typed_relocation_is_moved_{namespace}_v48(definition: int) -> bool {{ false"
        );
        for result in &self.results {
            out.budget.charge_work(1)?;
            emit!(out, " || definition == {}", result.input);
        }
        emit!(
            out,
            " }}\nspec fn typed_relocation_result_{namespace}_v48(definition: int, base: MemoryStateV30, little_endian: bool) -> Option<MemoryValueV30> {{\n"
        );
        for result in &self.results {
            out.budget.charge_work(3)?;
            emit!(
                out,
                " if definition == {} {{ match typed_relocated_expression_{namespace}_{}_v48(base, little_endian) {{ Some(values) => if {} < values.len() {{ Some(values[{}]) }} else {{ None }}, None => None }} }} else\n",
                result.input,
                result.node,
                result.result,
                result.result
            );
        }
        emit!(
            out,
            " {{ None }}\n}}\nspec fn typed_relocation_cut_{namespace}_v48(block: int, before: MemoryStateV30, after: MemoryStateV30, little_endian: bool) -> bool {{\n before.values.len() == {definitions} && after.values.len() == {}",
            output.definitions().len()
        );
        for cut in &self.cuts {
            out.budget.charge_work(4)?;
            let result = self.results[cut.result];
            emit!(
                out,
                "\n && (block == {} ==> typed_relocation_result_{namespace}_v48({}, before, little_endian) == Some(after.values[{}]))",
                cut.block,
                result.input,
                result.output
            );
        }
        emit!(out, "\n}}\n");
        Ok(())
    }
}
