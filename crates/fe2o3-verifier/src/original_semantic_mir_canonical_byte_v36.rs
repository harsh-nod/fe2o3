//! A single actual canonical scalar operation over current tagged SSA values.
//! Parsing and arithmetic are shared with the existing original-KIR model.

use super::super::{
    Error, Inventory, Resource, Result, Writer, byte_memory_v30::ByteMemoryStateNamesV30,
};
use super::{ExpressionV30, NodeV30, ScalarV30, canonical, emit_graph_v30};
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
    CanonicalKirDefinitionCoordinateV1 as Definition, CanonicalKirUseCoordinateV1 as Use,
    FormalIndexWidth, Type,
};
use std::{cell::Cell, fmt::Write as _, mem::size_of};

const EMPTY: NodeV30 = NodeV30 {
    scalar: ScalarV30::Unit,
    expression: ExpressionV30::Constant(0),
};

// Complete semantic inputs to the scalar transition, excluding its separately
// checked model precondition and observation coordinate.
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct CanonicalByteScalarBodyV55 {
    definitions: usize,
    inputs: [(usize, ScalarV30); 3],
    arguments: usize,
    nodes: [NodeV30; 4],
    destination: usize,
}

/// This is an immutable arithmetic plan, not an owner or memory-admission token.
/// The containing byte function retains and checks its allocation resolver.
pub(crate) struct CanonicalByteScalarV30<'inventory, 'owner> {
    inventory: &'inventory Inventory<'owner>,
    operation: usize,
    inputs: [(usize, ScalarV30); 3],
    arguments: usize,
    nodes: [NodeV30; 4],
    destination: usize,
    required: usize,
    slot: usize,
    ledger: Ledger,
    failure: Cell<Option<Resource>>,
}

fn mismatch() -> Error {
    Error::Statement("canonical byte scalar occurrence differs")
}

impl<'inventory, 'owner> CanonicalByteScalarV30<'inventory, 'owner> {
    pub(crate) fn derive(
        inventory: &'inventory Inventory<'owner>,
        operation: usize,
        width: FormalIndexWidth,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        out.budget.reserve_storage(headers())?;
        out.budget.charge_work(2)?;
        let row = inventory.operations().get(operation).ok_or_else(mismatch)?;
        if row.results.len() != 1 || !row.effects.is_empty() || row.operands.len() > 3 {
            #[cfg(test)]
            eprintln!(
                "canonical byte scalar rejected operation {operation} at {:?}: kind {:?}, results {:?}, operands {:?}, effects {:?}",
                row.coordinate, row.operation.kind, row.results, row.operands, row.effects
            );
            return Err(Error::Statement(
                "canonical byte scalar effect or arity is not modeled",
            ));
        }
        // All bounded row/type/coordinate checks and shared primitive parsing.
        out.budget.charge_work(18 + 8 * row.operands.len())?;
        let function = inventory
            .functions()
            .get(row.coordinate.block.function.0 as usize)
            .ok_or_else(mismatch)?;
        let destination = inventory
            .definitions()
            .get(row.results.start)
            .ok_or_else(mismatch)?;
        if !function.operations.contains(&operation)
            || !function.definitions.contains(&row.results.start)
            || destination.coordinate
                != (Definition::Result {
                    operation: row.coordinate,
                    result: 0,
                })
        {
            return Err(mismatch());
        }
        let scalar = scalar_type(destination.ty, width)?;
        let mut nodes = [EMPTY; 4];
        let mut inputs = [(0, ScalarV30::Unit); 3];
        for (ordinal, index) in row.operands.clone().enumerate() {
            let input = inventory.uses().get(index).ok_or_else(mismatch)?;
            let definition = inventory
                .definitions()
                .get(input.definition)
                .ok_or_else(mismatch)?;
            if input.coordinate
                != (Use::OperationOperand {
                    operation: row.coordinate,
                    operand: ordinal as u32,
                })
                || !function.definitions.contains(&input.definition)
            {
                return Err(mismatch());
            }
            let ty = scalar_type(definition.ty, width)?;
            inputs[ordinal] = (input.definition, ty);
            nodes[ordinal] = NodeV30 {
                scalar: ty,
                expression: ExpressionV30::Argument(ordinal as u32),
            };
        }
        let arguments = row.operands.len();
        let expression = canonical::operation_expression(
            &row.operation.kind,
            &[0, 1, 2][..arguments],
            scalar,
            &nodes[..arguments],
        )?;
        if let ExpressionV30::Constant(value) = expression
            && (scalar == ScalarV30::Unit || value >= (1u128 << scalar.width()))
        {
            return Err(mismatch());
        }
        nodes[arguments] = NodeV30 { scalar, expression };
        Ok(Self {
            inventory,
            operation,
            inputs,
            arguments,
            nodes,
            destination: row.results.start,
            required: out.budget.storage(),
            slot: std::ptr::from_ref(&*out.budget) as usize,
            ledger: out.budget.work_ledger_identity_v1(),
            failure: Cell::new(None),
        })
    }

    fn check(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        if let Some(error) = self.failure.get() {
            return Err(error.into());
        }
        self.retain(self.check_current(out))
    }

    fn retain<T>(&self, result: Result<T>) -> Result<T> {
        if let Err(Error::Resource(error)) = &result {
            self.failure.set(Some(*error));
        }
        result
    }

    fn check_current(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        if self.slot != std::ptr::from_ref(&*out.budget) as usize
            || self.ledger != out.budget.work_ledger_identity_v1()
        {
            return Err(Resource::Accounting.into());
        }
        out.budget.charge_work(2)?;
        if out.budget.storage() < self.required {
            return Err(Resource::Accounting.into());
        }
        if self
            .inventory
            .operations()
            .get(self.operation)
            .map(|row| row.results.start)
            != Some(self.destination)
        {
            return Err(mismatch());
        }
        Ok(())
    }

    pub(crate) fn emit_definition(&self, namespace: usize, out: &mut Writer<'_, '_>) -> Result<()> {
        self.retain(self.emit_definition_current(namespace, out))
    }

    pub(crate) fn body_descriptor_v55(
        &self,
        out: &mut Writer<'_, '_>,
    ) -> Result<CanonicalByteScalarBodyV55> {
        self.retain((|| {
            self.check(out)?;
            out.budget.charge_work(16)?;
            Ok(CanonicalByteScalarBodyV55 {
                definitions: self.inventory.definitions().len(),
                inputs: self.inputs,
                arguments: self.arguments,
                nodes: self.nodes,
                destination: self.destination,
            })
        })())
    }

    fn emit_definition_current(&self, namespace: usize, out: &mut Writer<'_, '_>) -> Result<()> {
        self.check(out)?;
        emit_graph_v30(
            &self.nodes[..=self.arguments],
            self.arguments,
            graph_key(namespace, self.operation)?,
            "canonical_byte_scalar",
            std::iter::once(Ok(Some(self.arguments))),
            None,
            out,
        )
    }

    /// Bind all five successor components. Failure is sticky and does not change
    /// values or memory. Operand reads precede the one exact destination update.
    pub(crate) fn emit_step(
        &self,
        namespace: usize,
        before: ByteMemoryStateNamesV30<'_>,
        after: ByteMemoryStateNamesV30<'_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.retain(self.emit_step_current(namespace, before, after, out))
    }

    fn emit_step_current(
        &self,
        namespace: usize,
        before: ByteMemoryStateNamesV30<'_>,
        after: ByteMemoryStateNamesV30<'_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.check(out)?;
        let key = graph_key(namespace, self.operation)?;
        write!(
            out,
            " let canonical_scalar_ok_{key} = {} && {}.len() == {}",
            before.valid,
            before.values,
            self.inventory.definitions().len()
        )
        .map_err(|_| out.error())?;
        for &(definition, scalar) in &self.inputs[..self.arguments] {
            out.budget.charge_work(1)?;
            write!(out, " && match {}[{definition}] {{ MemoryValueV30::Scalar(v) => 0 <= v < {}int, _ => false }}", before.values, 1u128 << scalar.width()).map_err(|_| out.error())?;
        }
        write!(out, ";\n let canonical_scalar_result_{key}: int = if canonical_scalar_ok_{key} {{ original_canonical_byte_scalar_trace_{key}_v30(seq![").map_err(|_| out.error())?;
        for &(definition, _) in &self.inputs[..self.arguments] {
            out.budget.charge_work(1)?;
            write!(
                out,
                "match {}[{definition}] {{ MemoryValueV30::Scalar(v) => v, _ => 0int }},",
                before.values
            )
            .map_err(|_| out.error())?;
        }
        write!(out, "])[0] }} else {{ 0int }};\n let canonical_scalar_ok_{key} = canonical_scalar_ok_{key} && 0 <= canonical_scalar_result_{key} < {}int;\n", 1u128 << self.nodes[self.arguments].scalar.width()).map_err(|_| out.error())?;
        write!(out, " let {} = if canonical_scalar_ok_{key} {{ {}.update({}int, MemoryValueV30::Scalar(canonical_scalar_result_{key})) }} else {{ {} }};\n let {} = {};\n let {} = {};\n let {} = {};\n let {} = canonical_scalar_ok_{key};\n", after.values, before.values, self.destination, before.values, after.memory, before.memory, after.generations, before.generations, after.frames, before.frames, after.valid).map_err(|_| out.error())
    }
}

fn scalar_type(ty: &Type, width: FormalIndexWidth) -> Result<ScalarV30> {
    if *ty == Type::INDEX {
        Ok(ScalarV30::Integer {
            width: match width {
                FormalIndexWidth::Bits32 => 32,
                FormalIndexWidth::Bits64 => 64,
                FormalIndexWidth::Unknown => {
                    return Err(Error::Statement("canonical byte index width is unknown"));
                }
            },
            signed: false,
        })
    } else {
        canonical::scalar(ty)
    }
}

fn graph_key(namespace: usize, operation: usize) -> Result<usize> {
    let sum = namespace
        .checked_add(operation)
        .ok_or(Resource::Arithmetic)?;
    let next = sum.checked_add(1).ok_or(Resource::Arithmetic)?;
    let triangular = if sum % 2 == 0 {
        (sum / 2).checked_mul(next)
    } else {
        sum.checked_mul(next / 2)
    }
    .ok_or(Resource::Arithmetic)?;
    triangular
        .checked_add(operation)
        .ok_or(Resource::Arithmetic.into())
}

fn headers() -> usize {
    size_of::<CanonicalByteScalarV30<'_, '_>>()
        + size_of::<Result<CanonicalByteScalarV30<'_, '_>>>()
        + size_of::<[NodeV30; 4]>()
        + size_of::<[(usize, ScalarV30); 3]>()
        + 2 * size_of::<ByteMemoryStateNamesV30<'_>>()
        + size_of::<std::ops::Range<usize>>()
        + size_of::<std::iter::Enumerate<std::ops::Range<usize>>>()
        + size_of::<std::iter::Once<Result<Option<usize>>>>()
        + 16 * size_of::<usize>()
        + 12 * size_of::<&()>()
        + 5 * size_of::<Result<()>>()
        + size_of::<Option<Resource>>()
        + size_of::<FormalIndexWidth>()
        + canonical::operation_headers_v31()
}

#[cfg(test)]
mod tests {
    use super::*;
    mod select_v54 {
        include!("original_semantic_mir_canonical_select_v54_tests.rs");
    }
    use fe2o3_kernel_ir::{
        BasicBlock, BinaryOp, BlockId, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work, ComparePredicate, Constant, Function, Module,
        Operation, OperationKind, ScalarType, Signature, StorageLayoutLimitsV1, Terminator, Type,
        UnaryOp, ValueDef, ValueId, VerifiedCanonicalKernelIrModuleV18 as Owner,
    };

    const LIMIT: usize = 100_000_000;
    const LAYOUTS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
        rows: 64,
        edges: 256,
        containment_depth: 32,
        object_bytes: 4096,
    };

    fn fixture(functions: usize, signed: bool) -> Module {
        let ty = Type::Scalar(if signed {
            ScalarType::I32
        } else {
            ScalarType::U32
        });
        let mut module = Module::new("canonical-tagged-scalar-step");
        for ordinal in 0..functions {
            let mut entry = BasicBlock::new(BlockId(0));
            for (result, result_ty, kind) in [
                (
                    2,
                    ty.clone(),
                    OperationKind::Constant(if signed {
                        Constant::I32(-5)
                    } else {
                        Constant::U32(9)
                    }),
                ),
                (
                    3,
                    ty.clone(),
                    OperationKind::Unary {
                        op: UnaryOp::Not,
                        operand: ValueId(0),
                    },
                ),
                (
                    4,
                    ty.clone(),
                    OperationKind::Binary {
                        op: BinaryOp::BitXor,
                        lhs: ValueId(0),
                        rhs: ValueId(1),
                    },
                ),
                (
                    5,
                    Type::BOOL,
                    OperationKind::Compare {
                        predicate: ComparePredicate::LessThan,
                        lhs: ValueId(1),
                        rhs: ValueId(0),
                    },
                ),
                (
                    6,
                    ty.clone(),
                    OperationKind::Binary {
                        op: BinaryOp::Add,
                        lhs: ValueId(0),
                        rhs: ValueId(1),
                    },
                ),
            ] {
                entry.operations.push(Operation::effect_free(
                    ValueDef::new(ValueId(result), result_ty),
                    kind,
                ));
            }
            entry.terminator = Some(Terminator::Return {
                values: vec![ValueId(4)],
            });
            module.functions.push(Function::internal_helper(
                format!("f{ordinal}"),
                Signature::new(vec![ty.clone(), ty.clone()], vec![ty.clone()]),
                vec![ValueId(0), ValueId(1)],
                vec![entry],
            ));
        }
        module
    }

    fn with_inventory(functions: usize, signed: bool, run: impl FnOnce(&Inventory<'_>, usize)) {
        with_module(&fixture(functions, signed), run)
    }

    fn with_module(module: &Module, run: impl FnOnce(&Inventory<'_>, usize)) {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let (owner, stored) =
            Owner::from_module_ref_with_verification_budget_v18(module, LAYOUTS, &mut budget)
                .unwrap();
        budget.reserve_storage(stored.retained_storage()).unwrap();
        let (inventory, retained) = Inventory::derive_v18(&owner, &mut budget).unwrap();
        budget.reserve_storage(retained.retained_storage()).unwrap();
        run(&inventory, budget.storage());
    }

    fn run(
        inventory: &Inventory<'_>,
        floor: usize,
        work: usize,
        storage: usize,
        body: impl FnOnce(&mut Writer<'_, '_>) -> Result<()>,
    ) -> (Result<String>, usize, usize) {
        let mut work = Work::new(work);
        let mut budget = Budget::new(&mut work, storage);
        let result = (|| {
            budget.reserve_storage(floor + super::super::super::super::SOURCE_LIMIT)?;
            let mut out = Writer::new(&mut budget)?;
            // The borrowed immutable inventory remains under its paid floor.
            let _ = inventory;
            body(&mut out)?;
            out.finish()
        })();
        let actual = (budget.work(), budget.peak_storage());
        budget.release_storage(budget.storage()).unwrap();
        (result, actual.0, actual.1)
    }

    fn names() -> (
        ByteMemoryStateNamesV30<'static>,
        ByteMemoryStateNamesV30<'static>,
    ) {
        (
            ByteMemoryStateNamesV30 {
                values: "v0",
                memory: "m0",
                generations: "g0",
                frames: "f0",
                valid: "ok0",
            },
            ByteMemoryStateNamesV30 {
                values: "v1",
                memory: "m1",
                generations: "g1",
                frames: "f1",
                valid: "ok1",
            },
        )
    }

    #[test]
    fn canonical_byte_scalar_uses_complete_definition_namespace_and_preserves_memory() {
        with_inventory(2, false, |inventory, floor| {
            let text = run(inventory, floor, LIMIT, LIMIT, |out| {
                let operation = inventory.functions()[1].operations.start + 2;
                let scalar = CanonicalByteScalarV30::derive(
                    inventory,
                    operation,
                    FormalIndexWidth::Bits64,
                    out,
                )?;
                assert!(std::ptr::eq(scalar.inventory, inventory));
                assert_eq!(
                    scalar.inputs[..2],
                    [
                        (
                            7,
                            ScalarV30::Integer {
                                width: 32,
                                signed: false
                            }
                        ),
                        (
                            8,
                            ScalarV30::Integer {
                                width: 32,
                                signed: false
                            }
                        )
                    ]
                );
                assert_eq!(scalar.destination, 11);
                assert_eq!(scalar.arguments, 2);
                scalar.emit_definition(3, out)?;
                let (before, after) = names();
                scalar.emit_step(3, before, after, out)
            })
            .0
            .unwrap();
            assert!(text.contains("ok0 && v0.len() == 14"));
            assert!(
                text.contains("match v0[7] { MemoryValueV30::Scalar(v) => 0 <= v < 4294967296int")
            );
            assert!(text.contains("((m0 as u32) ^ (m1 as u32)) as int"));
            assert!(text.contains("v0.update(11int, MemoryValueV30::Scalar"));
            assert!(text.contains("else { v0 }"));
            assert!(text.contains("let m1 = m0;\n let g1 = g0;\n let f1 = f0;"));
            assert!(text.contains("let ok1 = canonical_scalar_ok_"));
            assert!(!text.contains("op("));
            assert!(!text.contains("assume("));
        });
    }

    #[test]
    fn canonical_byte_scalar_body_descriptor_covers_every_transition_input() {
        with_inventory(1, false, |inventory, floor| {
            run(inventory, floor, LIMIT, LIMIT, |out| {
                let scalar =
                    CanonicalByteScalarV30::derive(inventory, 2, FormalIndexWidth::Bits64, out)?;
                let body = scalar.body_descriptor_v55(out)?;
                let mutations: [fn(&mut CanonicalByteScalarBodyV55); 7] = [
                    |body| body.definitions += 1,
                    |body| body.destination += 1,
                    |body| body.inputs.swap(0, 1),
                    |body| body.inputs[0].1 = ScalarV30::Bool,
                    |body| body.arguments -= 1,
                    |body| body.nodes[2].scalar = ScalarV30::Bool,
                    |body| body.nodes[2].expression = ExpressionV30::Constant(7),
                ];
                for mutate in mutations {
                    let mut different = body;
                    mutate(&mut different);
                    assert!(different != body);
                }
                assert!(scalar.body_descriptor_v55(out)? == body);
                assert!(out.text.is_empty());
                Ok(())
            })
            .0
            .unwrap();
        });
    }

    #[test]
    fn canonical_byte_scalar_shared_signed_comparison_and_constant_bits() {
        with_inventory(1, true, |inventory, floor| {
            let text = run(inventory, floor, LIMIT, LIMIT, |out| {
                for operation in 0..4 {
                    let scalar = CanonicalByteScalarV30::derive(
                        inventory,
                        operation,
                        FormalIndexWidth::Bits64,
                        out,
                    )?;
                    scalar.emit_definition(0, out)?;
                    let (before, after) = names();
                    scalar.emit_step(0, before, after, out)?;
                }
                Ok(())
            })
            .0
            .unwrap();
            assert!(text.contains("4294967291int"));
            assert!(text.contains("(!(m0 as u32)) as int"));
            assert!(text.contains(
                "if source_signed_v30(m0, 4294967296) < source_signed_v30(m1, 4294967296) { 1int } else { 0int }"
            ));
            assert!(text.contains("< 2int"));
            let start = text
                .find("original_canonical_byte_scalar_trace_9_v30")
                .unwrap();
            assert!(text[start..].contains("match v0[1]"));
        });
    }

    #[test]
    fn canonical_byte_scalar_refuses_unmodeled_or_missing_operation_before_text() {
        with_inventory(1, false, |inventory, floor| {
            for operation in [4, usize::MAX] {
                run(inventory, floor, LIMIT, LIMIT, |out| {
                    let result = CanonicalByteScalarV30::derive(
                        inventory,
                        operation,
                        FormalIndexWidth::Bits64,
                        out,
                    );
                    assert!(matches!(result, Err(Error::Statement(_))));
                    assert!(out.text.is_empty());
                    Ok(())
                })
                .0
                .unwrap();
            }
        });
    }

    #[test]
    fn canonical_byte_scalar_independent_derive_work_is_arity_scoped() {
        for functions in [1, 8, 32] {
            with_inventory(functions, false, |inventory, floor| {
                for (offset, expected) in [(0, 20), (1, 28), (2, 36), (3, 36)] {
                    for limit in [expected, expected - 1] {
                        let (result, work, _) = run(inventory, floor, limit, LIMIT, |out| {
                            let operation =
                                inventory.functions()[functions - 1].operations.start + offset;
                            CanonicalByteScalarV30::derive(
                                inventory,
                                operation,
                                FormalIndexWidth::Bits64,
                                out,
                            )
                            .map(|_| ())
                        });
                        assert_eq!(result.is_ok(), limit == expected);
                        assert_eq!(work, if limit == expected { expected } else { 2 });
                        if limit < expected {
                            assert!(
                                matches!(result, Err(Error::Resource(Resource::Work(error))) if error.actual() == expected && error.limit() == limit)
                            );
                        }
                    }
                }
            });
        }
    }

    #[test]
    fn canonical_byte_scalar_exact_and_one_short_full_emission_limits() {
        with_inventory(2, false, |inventory, floor| {
            let body = |out: &mut Writer<'_, '_>| {
                let scalar =
                    CanonicalByteScalarV30::derive(inventory, 7, FormalIndexWidth::Bits64, out)?;
                scalar.emit_definition(2, out)?;
                let (before, after) = names();
                scalar.emit_step(2, before, after, out)
            };
            let (result, work, storage) = run(inventory, floor, LIMIT, LIMIT, body);
            result.unwrap();
            let exact = run(inventory, floor, work, storage, body);
            exact.0.unwrap();
            assert_eq!((exact.1, exact.2), (work, storage));
            assert!(matches!(
                run(inventory, floor, work - 1, storage, body).0,
                Err(Error::Resource(Resource::Work(_)))
            ));
            assert!(matches!(
                run(inventory, floor, work, storage - 1, body).0,
                Err(Error::Resource(Resource::Storage(_)))
            ));
        });
    }

    #[test]
    fn canonical_byte_scalar_restored_floor_cannot_retry_caught_undercut() {
        with_inventory(1, false, |inventory, floor| {
            run(inventory, floor, LIMIT, LIMIT, |out| {
                let scalar =
                    CanonicalByteScalarV30::derive(inventory, 2, FormalIndexWidth::Bits64, out)?;
                let original = out.budget.storage();
                out.budget.release_storage(1)?;
                assert!(matches!(
                    scalar.emit_definition(0, out),
                    Err(Error::Resource(Resource::Accounting))
                ));
                out.budget.reserve_storage(1)?;
                assert_eq!(out.budget.storage(), original);
                let work = out.budget.work();
                assert!(matches!(
                    scalar.emit_definition(0, out),
                    Err(Error::Resource(Resource::Accounting))
                ));
                let (before, after) = names();
                assert!(matches!(
                    scalar.emit_step(0, before, after, out),
                    Err(Error::Resource(Resource::Accounting))
                ));
                assert_eq!(out.budget.work(), work);
                assert!(out.text.is_empty());
                Ok(())
            })
            .0
            .unwrap();
        });
    }

    #[test]
    fn canonical_byte_scalar_funded_foreign_ledger_poisoning_precedes_work() {
        with_inventory(1, false, |inventory, floor| {
            run(inventory, floor, LIMIT, LIMIT, |out| {
                let scalar =
                    CanonicalByteScalarV30::derive(inventory, 2, FormalIndexWidth::Bits64, out)?;
                let mut work = Work::new(LIMIT);
                let mut budget = Budget::new(&mut work, LIMIT);
                budget.reserve_storage(out.budget.storage())?;
                let mut foreign = Writer::new(&mut budget)?;
                let before = foreign.budget.work();
                assert!(matches!(
                    scalar.emit_definition(0, &mut foreign),
                    Err(Error::Resource(Resource::Accounting))
                ));
                assert_eq!(foreign.budget.work(), before);
                assert!(foreign.text.is_empty());
                let before = out.budget.work();
                assert!(matches!(
                    scalar.emit_definition(0, out),
                    Err(Error::Resource(Resource::Accounting))
                ));
                assert_eq!(out.budget.work(), before);
                assert!(out.text.is_empty());
                Ok(())
            })
            .0
            .unwrap();
        });
    }

    #[test]
    fn canonical_byte_scalar_header_and_graph_keys_are_independent() {
        let primitive = size_of::<(
            &fe2o3_kernel_ir::OperationKind,
            &[usize],
            ScalarV30,
            &[NodeV30],
            ExpressionV30,
            Result<ExpressionV30>,
        )>();
        let expected = size_of::<CanonicalByteScalarV30<'_, '_>>()
            + size_of::<Result<CanonicalByteScalarV30<'_, '_>>>()
            + size_of::<[NodeV30; 4]>()
            + size_of::<[(usize, ScalarV30); 3]>()
            + 2 * size_of::<ByteMemoryStateNamesV30<'_>>()
            + size_of::<std::ops::Range<usize>>()
            + size_of::<std::iter::Enumerate<std::ops::Range<usize>>>()
            + size_of::<std::iter::Once<Result<Option<usize>>>>()
            + 16 * size_of::<usize>()
            + 12 * size_of::<&()>()
            + 5 * size_of::<Result<()>>()
            + size_of::<Option<Resource>>()
            + size_of::<FormalIndexWidth>()
            + primitive;
        assert_eq!(headers(), expected);
        for limit in [expected, expected - 1] {
            let mut work = Work::new(0);
            let mut budget = Budget::new(&mut work, limit);
            assert_eq!(budget.reserve_storage(headers()).is_ok(), limit == expected);
            assert_eq!(budget.work(), 0);
        }
        let mut keys = std::collections::BTreeSet::new();
        for namespace in 0..16 {
            for operation in 0..64 {
                assert!(keys.insert(graph_key(namespace, operation).unwrap()));
            }
        }
        assert!(matches!(
            graph_key(usize::MAX, 1),
            Err(Error::Resource(Resource::Arithmetic))
        ));
    }

    #[test]
    fn canonical_byte_scalar_index_width_is_explicit_and_checks_constant_range() {
        let mut block = BasicBlock::new(BlockId(0));
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(1), Type::INDEX),
            OperationKind::Constant(Constant::Index(1u64 << 40)),
        ));
        block.terminator = Some(Terminator::Return {
            values: vec![ValueId(1)],
        });
        let mut module = Module::new("canonical-byte-explicit-index-width");
        module.functions.push(Function::internal_helper(
            "entry",
            Signature::new(vec![Type::INDEX], vec![Type::INDEX]),
            vec![ValueId(0)],
            vec![block],
        ));
        with_module(&module, |inventory, floor| {
            for width in [
                FormalIndexWidth::Bits32,
                FormalIndexWidth::Bits64,
                FormalIndexWidth::Unknown,
            ] {
                run(inventory, floor, LIMIT, LIMIT, |out| {
                    let result = CanonicalByteScalarV30::derive(inventory, 0, width, out);
                    if width == FormalIndexWidth::Bits64 {
                        let scalar = result?;
                        assert_eq!(
                            scalar.nodes[0].scalar,
                            ScalarV30::Integer {
                                width: 64,
                                signed: false
                            }
                        );
                        scalar.emit_definition(0, out)?;
                        assert!(out.text.contains("1099511627776int"));
                    } else {
                        assert!(matches!(result, Err(Error::Statement(_))));
                        assert!(out.text.is_empty());
                    }
                    Ok(())
                })
                .0
                .unwrap();
            }
        });
        assert_eq!(
            scalar_type(&Type::INDEX, FormalIndexWidth::Bits32).unwrap(),
            ScalarV30::Integer {
                width: 32,
                signed: false
            }
        );
        assert!(scalar_type(&Type::Unit, FormalIndexWidth::Bits64).is_err());
    }

    #[test]
    fn canonical_byte_scalar_rejects_foreign_ledger_and_lost_retained_floor() {
        with_inventory(1, false, |inventory, floor| {
            run(inventory, floor, LIMIT, LIMIT, |out| {
                let scalar =
                    CanonicalByteScalarV30::derive(inventory, 2, FormalIndexWidth::Bits64, out)?;
                let mut work = Work::new(LIMIT);
                let mut budget = Budget::new(&mut work, LIMIT);
                budget.reserve_storage(out.budget.storage())?;
                let mut foreign = Writer::new(&mut budget)?;
                assert!(matches!(
                    scalar.emit_definition(0, &mut foreign),
                    Err(Error::Resource(Resource::Accounting))
                ));
                assert!(foreign.text.is_empty());
                assert_eq!(foreign.budget.work(), 0);
                out.budget.release_storage(1)?;
                assert!(matches!(
                    scalar.emit_definition(0, out),
                    Err(Error::Resource(Resource::Accounting))
                ));
                assert!(out.text.is_empty());
                Ok(())
            })
            .0
            .unwrap();
        });
    }
}
