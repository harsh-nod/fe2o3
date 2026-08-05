//! Fill-only legalization for the opt-in production kernel-IR pipeline.
//!
//! Helper names are matched here only after `mir_import` classified their rustc `DefId` and
//! `translate_and_verify` produced this in-memory module. This module must not be used to grant
//! the same authority to decoded or caller-constructed kernel IR.

use crate::CODEGEN_PIPELINE_ENV;
use crate::amdgpu_llvm::{EmitError, PreparedDeviceKernel};
use crate::trusted_device_items::TrustedDeviceItem;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, ComparePredicate, FunctionBody, IntrinsicOperation, KernelId, Module,
    Operation, OperationKind, Terminator, Type, ValueDef, ValueId, WorkgroupSize, verify_module,
};
use std::collections::{BTreeMap, BTreeSet};

const FILL_KERNEL: &str = "fill";
const FILL_WORKGROUP_X: u32 = 256;

pub(crate) fn prepare_fill_collection(
    mut module: Module,
    expected_kernel_names: &[String],
) -> Result<Vec<PreparedDeviceKernel>, EmitError> {
    verify_module(&module).map_err(|errors| {
        reject(format!(
            "received invalid verified kernel IR before fill legalization: {errors}"
        ))
    })?;

    let mut expected = expected_kernel_names.to_vec();
    expected.sort();
    if expected.as_slice() != [FILL_KERNEL] {
        return Err(reject(format!(
            "currently supports exactly the `{FILL_KERNEL}` kernel export; collected {expected:?}; unset {CODEGEN_PIPELINE_ENV} to use the default legacy-v1 pipeline"
        )));
    }

    let mut translated = module
        .kernels
        .iter()
        .map(|kernel| kernel.id.as_str().to_string())
        .collect::<Vec<_>>();
    translated.sort();
    if translated != expected {
        return Err(reject(format!(
            "translated kernel identities {translated:?} do not match collected kernel identities {expected:?}"
        )));
    }

    let kernel = module
        .kernels
        .iter_mut()
        .find(|kernel| kernel.id.as_str() == FILL_KERNEL)
        .expect("identity equality established the fill kernel");
    kernel.workgroup_size = Some(WorkgroupSize::new(FILL_WORKGROUP_X, 1, 1));
    let entry = kernel.entry.clone();

    let function = module
        .functions
        .iter_mut()
        .find(|function| function.id == entry)
        .expect("initial verification established the kernel entry");
    let expected_slice = writable_f32_slice();
    if function.signature.parameters != [expected_slice.clone()]
        || !function.signature.results.is_empty()
    {
        return Err(reject(format!(
            "`{FILL_KERNEL}` must have exact kernel IR signature ([writable global f32 slice]) -> (); found {:?} -> {:?}",
            function.signature.parameters, function.signature.results
        )));
    }
    let body = function.body.as_mut().expect("verified kernel entry body");
    legalize_fill_body(body, &function.signature.parameters)?;

    verify_module(&module).map_err(|errors| {
        reject(format!(
            "fill legalization produced invalid kernel IR and was not emitted: {errors}"
        ))
    })?;
    let llvm_ir = dialect_amdgcn::lower_kernel_to_llvm_ir(&module, &KernelId::new(FILL_KERNEL))
        .map_err(|errors| {
            reject(format!(
                "G1 AMDGPU lowering rejected `{FILL_KERNEL}`: {errors}"
            ))
        })?;

    Ok(vec![PreparedDeviceKernel {
        name: FILL_KERNEL.to_string(),
        llvm_ir,
    }])
}

fn legalize_fill_body(body: &mut FunctionBody, parameters: &[Type]) -> Result<(), EmitError> {
    if body.parameters.len() != parameters.len() {
        return Err(reject(
            "fill entry parameter identities do not match its signature",
        ));
    }

    let value_types = collect_value_types(body, parameters);
    let mut next_value = value_types.keys().next_back().map_or(Ok(0), |value| {
        value
            .0
            .checked_add(1)
            .ok_or_else(|| reject("fill kernel exhausted kernel IR value identities"))
    })?;
    let mut option_conditions = BTreeSet::new();
    let mut thread_calls = 0usize;
    let mut get_mut_calls = 0usize;
    let mut thread_index = None;
    let mut get_mut_index = None;

    for block in &mut body.blocks {
        let mut legalized = Vec::with_capacity(block.operations.len() + 4);
        for operation in std::mem::take(&mut block.operations) {
            let OperationKind::Call { callee, arguments } = &operation.kind else {
                legalized.push(operation);
                continue;
            };

            if callee.as_str() == TrustedDeviceItem::ThreadIndex1d.canonical_path() {
                require_call_shape(
                    "thread::index_1d",
                    &operation,
                    arguments,
                    &[],
                    &[Type::INDEX],
                    &value_types,
                )?;
                thread_calls += 1;
                thread_index = Some(operation.results[0].id);
                legalized.push(Operation::effect_free(
                    operation.results[0].clone(),
                    OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
                ));
                continue;
            }

            if callee.as_str() == TrustedDeviceItem::DisjointSliceGetMut.canonical_path() {
                let pointer = writable_f32_pointer();
                require_call_shape(
                    "DisjointSlice::get_mut",
                    &operation,
                    arguments,
                    &[writable_f32_slice(), Type::INDEX],
                    &[Type::INDEX, pointer.clone()],
                    &value_types,
                )?;
                get_mut_calls += 1;
                get_mut_index = Some(arguments[1]);

                let length = fresh_value(&mut next_value, Type::INDEX)?;
                legalized.push(Operation::effect_free(
                    length.clone(),
                    OperationKind::SliceLength {
                        slice: arguments[0],
                    },
                ));

                let condition = ValueDef::new(operation.results[0].id, Type::BOOL);
                option_conditions.insert(condition.id);
                legalized.push(Operation::effect_free(
                    condition,
                    OperationKind::Compare {
                        predicate: ComparePredicate::LessThan,
                        lhs: arguments[1],
                        rhs: length.id,
                    },
                ));

                let data = fresh_value(&mut next_value, pointer)?;
                legalized.push(Operation::effect_free(
                    data.clone(),
                    OperationKind::SliceData {
                        slice: arguments[0],
                    },
                ));
                legalized.push(Operation::effect_free(
                    operation.results[1].clone(),
                    OperationKind::GetElementPointer {
                        base: data.id,
                        offset: arguments[1],
                    },
                ));
                continue;
            }

            return Err(reject(format!(
                "fill legalization does not support call `{callee}`; no legacy fallback was attempted"
            )));
        }
        block.operations = legalized;
    }

    if thread_calls != 1 || get_mut_calls != 1 {
        return Err(reject(format!(
            "fill legalization requires exactly one trusted thread::index_1d call and one trusted DisjointSlice::get_mut call; found {thread_calls} and {get_mut_calls}"
        )));
    }
    if thread_index != get_mut_index {
        return Err(reject(format!(
            "fill DisjointSlice::get_mut must use the exact trusted global thread index; found thread result {thread_index:?} and get_mut index {get_mut_index:?}"
        )));
    }

    let unreachable_blocks = body
        .blocks
        .iter()
        .filter(|block| {
            block.parameters.is_empty()
                && block.operations.is_empty()
                && matches!(block.terminator, Some(Terminator::Unreachable))
        })
        .map(|block| block.id)
        .collect::<BTreeSet<_>>();
    let mut option_switches = 0usize;
    for block in &mut body.blocks {
        let Some(Terminator::Switch {
            selector,
            cases,
            default_target,
            default_arguments,
        }) = block.terminator.as_ref()
        else {
            continue;
        };
        if !option_conditions.contains(selector) {
            return Err(reject(format!(
                "fill contains unsupported non-Option switch in {}",
                block.id
            )));
        }
        if cases.len() != 2
            || cases.iter().any(|case| !case.arguments.is_empty())
            || !default_arguments.is_empty()
            || !unreachable_blocks.contains(default_target)
        {
            return Err(reject(format!(
                "fill Option switch in {} must have cases 0 and 1 with an unreachable default and no block arguments",
                block.id
            )));
        }
        let false_target = cases
            .iter()
            .find(|case| case.value == 0)
            .map(|case| case.target);
        let true_target = cases
            .iter()
            .find(|case| case.value == 1)
            .map(|case| case.target);
        let (Some(false_target), Some(true_target)) = (false_target, true_target) else {
            return Err(reject(format!(
                "fill Option switch in {} must contain exactly discriminants 0 and 1",
                block.id
            )));
        };
        block.terminator = Some(Terminator::ConditionalBranch {
            condition: *selector,
            then_target: true_target,
            then_arguments: Vec::new(),
            else_target: false_target,
            else_arguments: Vec::new(),
        });
        option_switches += 1;
    }
    if option_switches != 1 {
        return Err(reject(format!(
            "fill legalization requires exactly one Option switch; found {option_switches}"
        )));
    }
    Ok(())
}

fn collect_value_types(body: &FunctionBody, parameters: &[Type]) -> BTreeMap<ValueId, Type> {
    let mut types = body
        .parameters
        .iter()
        .copied()
        .zip(parameters.iter().cloned())
        .collect::<BTreeMap<_, _>>();
    for block in &body.blocks {
        for value in block.parameters.iter().chain(
            block
                .operations
                .iter()
                .flat_map(|operation| &operation.results),
        ) {
            types.insert(value.id, value.ty.clone());
        }
    }
    types
}

fn require_call_shape(
    name: &str,
    operation: &Operation,
    arguments: &[ValueId],
    expected_arguments: &[Type],
    expected_results: &[Type],
    value_types: &BTreeMap<ValueId, Type>,
) -> Result<(), EmitError> {
    let argument_types = arguments
        .iter()
        .map(|argument| value_types.get(argument).cloned())
        .collect::<Option<Vec<_>>>();
    let result_types = operation
        .results
        .iter()
        .map(|result| result.ty.clone())
        .collect::<Vec<_>>();
    if argument_types.as_deref() != Some(expected_arguments) || result_types != expected_results {
        return Err(reject(format!(
            "trusted {name} call has unsupported kernel IR signature {:?} -> {result_types:?}; expected {expected_arguments:?} -> {expected_results:?}",
            argument_types.unwrap_or_default()
        )));
    }
    Ok(())
}

fn fresh_value(next: &mut u32, ty: Type) -> Result<ValueDef, EmitError> {
    let value = ValueDef::new(ValueId(*next), ty);
    *next = next
        .checked_add(1)
        .ok_or_else(|| reject("fill kernel exhausted kernel IR value identities"))?;
    Ok(value)
}

fn writable_f32_slice() -> Type {
    Type::slice(Type::F32, AddressSpace::Global, AccessMode::ReadWrite)
}

fn writable_f32_pointer() -> Type {
    Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadWrite)
}

fn reject(reason: impl Into<String>) -> EmitError {
    EmitError::Preflight {
        reason: format!(
            "{CODEGEN_PIPELINE_ENV}=kernel-ir-v1 production path rejected input: {}",
            reason.into()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::{
        BasicBlock, BlockId, Constant, Function, FunctionId, LaunchDomain, LaunchExtent,
        MemoryAccess, Signature, SwitchCase,
    };

    fn translated_fill() -> Module {
        let slice = writable_f32_slice();
        let pointer = writable_f32_pointer();

        let mut entry = BasicBlock::new(BlockId(0));
        entry.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(1), Type::INDEX),
            OperationKind::Call {
                callee: FunctionId::new(TrustedDeviceItem::ThreadIndex1d.canonical_path()),
                arguments: vec![],
            },
        ));
        entry.terminator = Some(Terminator::Branch {
            target: BlockId(1),
            arguments: vec![],
        });

        let mut get_mut = BasicBlock::new(BlockId(1));
        get_mut.operations.push(Operation::new(
            vec![
                ValueDef::new(ValueId(2), Type::INDEX),
                ValueDef::new(ValueId(3), pointer.clone()),
            ],
            OperationKind::Call {
                callee: FunctionId::new(TrustedDeviceItem::DisjointSliceGetMut.canonical_path()),
                arguments: vec![ValueId(0), ValueId(1)],
            },
        ));
        get_mut.terminator = Some(Terminator::Branch {
            target: BlockId(2),
            arguments: vec![],
        });

        let mut select = BasicBlock::new(BlockId(2));
        select.terminator = Some(Terminator::Switch {
            selector: ValueId(2),
            cases: vec![
                SwitchCase {
                    value: 0,
                    target: BlockId(4),
                    arguments: vec![],
                },
                SwitchCase {
                    value: 1,
                    target: BlockId(3),
                    arguments: vec![],
                },
            ],
            default_target: BlockId(5),
            default_arguments: vec![],
        });

        let mut store = BasicBlock::new(BlockId(3));
        store.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(4), Type::F32),
            OperationKind::Constant(Constant::F32Bits(42.5f32.to_bits())),
        ));
        store.operations.push(Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(3),
                value: ValueId(4),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ));
        store.terminator = Some(Terminator::Branch {
            target: BlockId(4),
            arguments: vec![],
        });

        let mut exit = BasicBlock::new(BlockId(4));
        exit.terminator = Some(Terminator::Return { values: vec![] });
        let mut unreachable = BasicBlock::new(BlockId(5));
        unreachable.terminator = Some(Terminator::Unreachable);

        let function = Function::definition(
            "fill_impl",
            Signature::new(vec![slice.clone()], vec![]),
            vec![ValueId(0)],
            vec![entry, get_mut, select, store, exit, unreachable],
        );
        let mut module = Module::new("tests::translated_fill");
        module.functions.push(function);
        module.functions.push(Function::declaration(
            TrustedDeviceItem::ThreadIndex1d.canonical_path(),
            Signature::new(vec![], vec![Type::INDEX]),
        ));
        module.functions.push(Function::declaration(
            TrustedDeviceItem::DisjointSliceGetMut.canonical_path(),
            Signature::new(vec![slice, Type::INDEX], vec![Type::INDEX, pointer]),
        ));
        module.kernels.push(fe2o3_kernel_ir::Kernel::new(
            FILL_KERNEL,
            "fill_impl",
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        ));
        module
    }

    #[test]
    fn verified_fill_uses_g1_deterministically() {
        let first = prepare_fill_collection(translated_fill(), &[FILL_KERNEL.to_string()])
            .expect("supported fill");
        let second = prepare_fill_collection(translated_fill(), &[FILL_KERNEL.to_string()])
            .expect("supported fill");

        assert_eq!(first.len(), 1);
        assert_eq!(first[0].name, FILL_KERNEL);
        assert_eq!(first[0].llvm_ir, second[0].llvm_ir);
        assert!(first[0].llvm_ir.contains("define amdgpu_kernel void @fill"));
        assert!(first[0].llvm_ir.contains("mul i64 %v1.group, 256"));
        assert!(first[0].llvm_ir.contains("!reqd_work_group_size !0"));
        assert!(!first[0].llvm_ir.contains("fe2o3_device"));
    }

    #[test]
    fn kernel_admission_is_exact_and_never_falls_back() {
        let error = prepare_fill_collection(translated_fill(), &["vecadd".to_string()])
            .expect_err("vecadd must remain on legacy-v1");

        let text = error.to_string();
        assert!(text.contains("supports exactly the `fill` kernel export"));
        assert!(text.contains("default legacy-v1 pipeline"));
    }

    #[test]
    fn unsupported_trusted_helper_is_rejected_before_g1() {
        let mut module = translated_fill();
        let function = &mut module.functions[0];
        let body = function.body.as_mut().expect("body");
        let OperationKind::Call { callee, .. } = &mut body.blocks[1].operations[0].kind else {
            panic!("get_mut call")
        };
        *callee = FunctionId::new(TrustedDeviceItem::DisjointSliceGetMutAt.canonical_path());
        let signature = module.functions[2].signature.clone();
        module.functions.push(Function::declaration(
            TrustedDeviceItem::DisjointSliceGetMutAt.canonical_path(),
            signature,
        ));

        let error = prepare_fill_collection(module, &[FILL_KERNEL.to_string()])
            .expect_err("get_mut_at is outside the production fill subset");
        assert!(error.to_string().contains("does not support call"));
        assert!(error.to_string().contains("no legacy fallback"));
    }

    #[test]
    fn get_mut_must_use_the_trusted_global_thread_index() {
        let mut module = translated_fill();
        let body = module.functions[0].body.as_mut().expect("body");
        body.blocks[1].operations.insert(
            0,
            Operation::effect_free(
                ValueDef::new(ValueId(10), Type::INDEX),
                OperationKind::Constant(Constant::Index(0)),
            ),
        );
        let OperationKind::Call { arguments, .. } = &mut body.blocks[1].operations[1].kind else {
            panic!("get_mut call")
        };
        arguments[1] = ValueId(10);

        let error = prepare_fill_collection(module, &[FILL_KERNEL.to_string()])
            .expect_err("a constant write index is outside the initial fill subset");
        assert!(
            error
                .to_string()
                .contains("must use the exact trusted global thread index")
        );
    }
}
