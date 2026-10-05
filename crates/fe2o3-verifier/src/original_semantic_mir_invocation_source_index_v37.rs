//! Original named launch queries, independent of their canonical lowering.

use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAxisV1 as Axis, SemanticCompilerIntrinsicOperationV1 as Intrinsic,
    SemanticDirectCallV1 as Call,
};

#[path = "original_semantic_mir_source_witness_calls_v38.rs"]
mod witnesses;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Query {
    Local,
    Group,
    WorkgroupSize,
    WorkgroupCount,
    Witness(witnesses::WitnessCall),
}

#[derive(Clone, Copy, Debug)]
pub(super) struct IndexCall {
    query: Query,
    axis: usize,
    destination: usize,
    continuation: usize,
}

fn classify(operation: Intrinsic) -> Result<(Query, usize)> {
    let (query, axis) = match operation {
        Intrinsic::ThreadIndex(axis) => (Query::Local, axis),
        Intrinsic::WorkgroupIndex(axis) => (Query::Group, axis),
        Intrinsic::WorkgroupDimension(axis) => (Query::WorkgroupSize, axis),
        Intrinsic::GridDimension(axis) => (Query::WorkgroupCount, axis),
        _ => return Err(unsupported()),
    };
    Ok((
        query,
        match axis {
            Axis::X => 0,
            Axis::Y => 1,
            Axis::Z => 2,
        },
    ))
}

impl IndexCall {
    pub(super) fn derive(
        slots: &SourceSlots<'_, '_>,
        plan: &InvocationPlan<'_, '_>,
        root: usize,
        instance: usize,
        block: usize,
        call: &Call,
        callable: &Callable,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        out.budget.reserve_storage(headers())?;
        out.budget.reserve_storage(witnesses::headers())?;
        out.budget.charge_work(12)?;
        let Callable::CompilerIntrinsic {
            binding, operation, ..
        } = callable
        else {
            return Err(mismatch());
        };
        let source = slots.correspondence(out)?.source(out.budget)?;
        let semantic = source.source_semantic(out.budget)?;
        let row = plan.instance(root, instance, out)?;
        let function = semantic
            .functions()
            .get(row.function.index() as usize)
            .ok_or_else(mismatch)?;
        let destination = call.destination().ok_or_else(unsupported)?;
        let local = destination.place().local().index() as usize;
        let continuation = destination.edge().target().index() as usize;
        if call.arguments().len() != binding.abi().source_input_types().len()
            || !call.variadic_argument_abis().is_empty()
            || call.unwind() != Unwind::Unreachable
            || destination.edge().role() != EdgeRole::CallReturn
            || !destination.place().projections().is_empty()
            || binding.abi().return_type() != destination.place().ty()
            || function.locals().get(local).map(|local| local.ty())
                != Some(destination.place().ty())
            || continuation >= row.blocks.len()
            || slots
                .legacy_descriptor_by_source(root, instance, local as u32, out)?
                .is_some()
        {
            return Err(unsupported());
        }
        for (argument, ty) in call
            .arguments()
            .iter()
            .zip(binding.abi().source_input_types())
        {
            out.budget.charge_work(1)?;
            if argument.ty() != *ty {
                return Err(mismatch());
            }
        }
        let (query, axis) = if let Some(witness) = witnesses::WitnessCall::derive(
            slots, plan, root, instance, block, call, *operation, out,
        )? {
            (Query::Witness(witness), 0)
        } else {
            if !call.arguments().is_empty()
                || ScalarV30::from_source(semantic.types(), destination.place().ty())?
                    != (ScalarV30::Integer {
                        signed: false,
                        width: 32,
                    })
            {
                return Err(unsupported());
            }
            classify(*operation)?
        };
        Ok(Self {
            query,
            axis,
            destination: row
                .locals
                .start
                .checked_add(local)
                .ok_or(Resource::Arithmetic)?,
            continuation: row
                .blocks
                .start
                .checked_add(continuation)
                .ok_or(Resource::Arithmetic)?,
        })
    }

    pub(super) fn emit(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        out.budget.charge_work(2)?;
        if let Query::Witness(witness) = self.query {
            write!(out, " let source = invocation_source_byte_pc_v36(").map_err(|_| out.error())?;
            witness.emit(self.destination, out)?;
            write!(out, ", {});\n InvocationSourceBlockResultV36 {{ source, before_control: cursor.source, observations: cursor.observations, operands: seq![], returned: None }}\n", self.continuation).map_err(|_| out.error())?;
            return Ok(());
        }
        let axis = self.axis;
        write!(out, " let source = match cursor.source.machine.frames.execution {{\n Some(execution) => {{ if !byte_execution_well_formed_v37(execution) || execution.rank <= {axis} {{ invocation_source_byte_refused_v36(cursor.source) }} else {{\n let value = ").map_err(|_| out.error())?;
        match self.query {
            Query::Local => write!(out, "execution.local[{axis}]"),
            Query::Group => write!(out, "execution.group[{axis}]"),
            Query::WorkgroupSize => write!(out, "execution.workgroup[{axis}]"),
            Query::WorkgroupCount => write!(out, "(execution.extent[{axis}] + execution.workgroup[{axis}] - 1) / execution.workgroup[{axis}]"),
            Query::Witness(_) => return Err(mismatch()),
        }.map_err(|_| out.error())?;
        write!(out, ";\n invocation_source_byte_pc_v36(invocation_source_byte_put_local_v36(cursor.source, {}, MemoryValueV30::Scalar(value % 4294967296int)), {})\n }} }}, None => invocation_source_byte_refused_v36(cursor.source) }};\n InvocationSourceBlockResultV36 {{ source, before_control: cursor.source, observations: cursor.observations, operands: seq![], returned: None }}\n", self.destination, self.continuation).map_err(|_| out.error())?;
        Ok(())
    }
}

pub(super) fn headers() -> usize {
    size_of::<IndexCall>()
        + size_of::<Result<IndexCall>>()
        + size_of::<(Query, usize)>()
        + size_of::<Result<(Query, usize)>>()
        + size_of::<Intrinsic>()
        + size_of::<Axis>()
        + 12 * size_of::<&()>()
        + 8 * size_of::<usize>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::*;

    const LIMIT: usize = 256 << 20;

    fn run(operation: Intrinsic, work: usize, storage: usize) -> (Result<()>, usize, usize, usize) {
        super::super::super::super::invocations::tests::run_callable_transform(
            work,
            storage,
            |_, functions, callables| {
                let helper = functions.last().unwrap();
                let word = SemanticTypeIdV1::from_index(0);
                let callable = SemanticCallableIdV1::from_index(callables.len() as u32);
                let abi = SemanticFunctionAbiV1::new(
                    SemanticAbiIdentityV1::from_sha256([231; 32]),
                    SemanticLayoutIdentityV1::from_sha256([232; 32]),
                    SemanticCanonAbiV1::Rust,
                    false,
                    false,
                    vec![],
                    helper.abi().arguments()[0].value().clone(),
                )
                .unwrap();
                callables.push(Callable::CompilerIntrinsic {
                    binding: SemanticNonBodyCallableBindingV1::new(
                        SemanticFunctionIdentityV1::from_sha256([233; 32]),
                        SemanticItemDefinitionIdentityV1::from_sha256([234; 32]),
                        SemanticMonomorphizationIdentityV1::from_sha256([235; 32]),
                        SemanticGenericTypeArgumentsIdentityV1::from_sha256([236; 32]),
                        SemanticConstGenericArgumentsIdentityV1::from_sha256([237; 32]),
                        helper.source(),
                        abi,
                    ),
                    operation,
                    operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([238; 32]),
                });
                for function in &mut functions[..2] {
                    let source = function.source();
                    let mut blocks = function.blocks().to_vec();
                    blocks[0] = SemanticBasicBlockV1::new(
                        blocks[0].identity(),
                        source,
                        blocks[0].statements().to_vec(),
                        SemanticTerminatorV1::new(
                            source,
                            SemanticTerminatorKindV1::Call(
                                SemanticDirectCallV1::new_callable(
                                    callable,
                                    vec![],
                                    Some(SemanticCallDestinationV1::new(
                                        SemanticPlaceV1::new(
                                            SemanticLocalIdV1::from_index(1),
                                            vec![],
                                            word,
                                        )
                                        .unwrap(),
                                        SemanticControlFlowEdgeV1::new(
                                            SemanticEdgeRoleV1::CallReturn,
                                            SemanticBlockIdV1::from_index(1),
                                        ),
                                    )),
                                    SemanticUnwindActionV1::Unreachable,
                                )
                                .unwrap(),
                            ),
                        ),
                    )
                    .unwrap();
                    *function = SemanticFunctionDeclV1::new(
                        function.identity(),
                        function.role(),
                        function.item_definition_identity(),
                        function.monomorphization_identity(),
                        function.generic_type_arguments_identity(),
                        function.const_generic_arguments_identity(),
                        source,
                        function.abi().clone(),
                        function.locals().to_vec(),
                        function.entry(),
                        blocks,
                    )
                    .unwrap()
                    .with_kernel_entry(function.kernel_entry().unwrap().clone());
                }
            },
            |plan, out| {
                super::super::tests::with_slots(plan, out, |slots, out| {
                    let mut program = SourceByteProgram::derive(plan, slots, out)?;
                    for root in 0..2 {
                        assert!(program.in_place_call(root, 0, 0, out)?);
                        assert!(!program.in_place_call(root, 0, 1, out)?);
                    }
                    let paired = super::super::super::paired::PairedInvocations::derive(
                        plan,
                        &program,
                        fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                        out,
                    )?;
                    program.emit(out)?;
                    paired.emit(out)?;
                    assert!(out.text.contains("cursor.source.machine.frames.execution"));
                    assert!(out.text.contains("value % 4294967296int"));
                    assert!(
                        out.text
                            .contains("None => invocation_source_byte_refused_v36(cursor.source)")
                    );
                    let query = match operation {
                        Intrinsic::ThreadIndex(_) => "let value = execution.local[0]",
                        Intrinsic::WorkgroupIndex(_) => "let value = execution.group[0]",
                        Intrinsic::WorkgroupDimension(_) => "let value = execution.workgroup[0]",
                        Intrinsic::GridDimension(_) => {
                            "let value = (execution.extent[0] + execution.workgroup[0] - 1) / execution.workgroup[0]"
                        }
                        _ => unreachable!(),
                    };
                    assert!(out.text.contains(query));
                    Ok(())
                })
            },
        )
    }

    #[test]
    fn original_mir_named_index_calls_enter_source_and_paired_pipeline_without_child_frames() {
        for operation in [
            Intrinsic::ThreadIndex(Axis::X),
            Intrinsic::WorkgroupIndex(Axis::X),
            Intrinsic::WorkgroupDimension(Axis::X),
            Intrinsic::GridDimension(Axis::X),
        ] {
            run(operation, LIMIT, LIMIT).0.unwrap();
        }
    }

    #[test]
    fn original_mir_named_index_calls_have_exact_and_one_short_pipeline_resources() {
        let operation = Intrinsic::ThreadIndex(Axis::X);
        let measured = run(operation, LIMIT, LIMIT);
        measured.0.unwrap();
        run(operation, measured.1, measured.3).0.unwrap();
        assert!(run(operation, measured.1 - 1, measured.3).0.is_err());
        assert!(run(operation, measured.1, measured.3 - 1).0.is_err());
    }

    #[test]
    fn original_mir_named_index_queries_preserve_each_source_axis_and_hierarchy() {
        for (axis, ordinal) in [(Axis::X, 0), (Axis::Y, 1), (Axis::Z, 2)] {
            for (operation, query) in [
                (Intrinsic::ThreadIndex(axis), Query::Local),
                (Intrinsic::WorkgroupIndex(axis), Query::Group),
                (Intrinsic::WorkgroupDimension(axis), Query::WorkgroupSize),
                (Intrinsic::GridDimension(axis), Query::WorkgroupCount),
            ] {
                assert_eq!(classify(operation).unwrap(), (query, ordinal));
            }
        }
        assert!(classify(Intrinsic::Trap).is_err());
        assert!(classify(Intrinsic::WorkgroupBarrier).is_err());
    }
}
