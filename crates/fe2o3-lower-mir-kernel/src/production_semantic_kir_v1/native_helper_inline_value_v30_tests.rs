//! Synthetic actual-KIR leaf tests: no source authority is minted here.
use super::super::native_helper_value_template_v1::Ledger;
use super::*;
use fe2o3_kernel_ir::{
    AssemblyConstraint, AssemblyEffect, AssemblyOperand, AssemblyOperandKind, AssemblyOption,
    AssemblySourceIdentity, BasicBlock, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrWorkBudgetV1 as Work, InlineAssembly, InlineAssemblyTarget, Signature,
};
type Expression = NormalizedScalarExpressionV1;
const U32: Scalar = Scalar::Integer {
    signed: false,
    bits: 32,
};
struct TestMeter<'w> {
    budget: Budget<'w>,
    failed: bool,
}
impl Meter for TestMeter<'_> {
    fn work(&mut self, n: usize) -> Result<(), Error> {
        self.budget.charge_work(n).map_err(|_| {
            self.failed = true;
            "work"
        })
    }
    fn reserve(&mut self, n: usize) -> Result<(), Error> {
        self.budget.reserve_storage(n).map_err(|_| {
            self.failed = true;
            "storage"
        })
    }
    fn release(&mut self, n: usize) -> Result<(), Error> {
        self.budget.release_storage(n).map_err(|_| "accounting")
    }
    fn exhausted(&self) -> bool {
        self.failed
    }
    fn storage(&self) -> Result<usize, Error> {
        Ok(self.budget.storage())
    }
    fn identity(&mut self) -> Result<Ledger, Error> {
        Ok(Ledger {
            slot: self as *const Self as usize,
            work: self.budget.work_ledger_identity_v1(),
        })
    }
}
fn run<T>(
    work: usize,
    storage: usize,
    action: impl FnOnce(&mut TestMeter<'_>) -> T,
) -> (T, usize, usize, usize) {
    let mut work = Work::new(work);
    let mut meter = TestMeter {
        budget: Budget::new(&mut work, storage),
        failed: false,
    };
    meter.budget.reserve_storage(4096).unwrap();
    let result = action(&mut meter);
    (
        result,
        meter.budget.storage(),
        meter.budget.work(),
        meter.budget.peak_storage(),
    )
}
fn constant(bits: u64) -> Expression {
    Expression::Constant { scalar: U32, bits }
}
fn function(mnemonic: &str) -> Function {
    let mut operands = vec![
        AssemblyOperand::output(0, AssemblyConstraint::Vgpr32),
        AssemblyOperand::input(ValueId(0), AssemblyConstraint::Vgpr32),
    ];
    if mnemonic != "v_mov_b32" {
        operands.push(AssemblyOperand::input(
            ValueId(1),
            AssemblyConstraint::Vgpr32,
        ));
    }
    let operation = Operation::effect_free(
        ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U32)),
        OperationKind::InlineAssembly(InlineAssembly {
            target: InlineAssemblyTarget::AmdGpuGfx942,
            source: AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]),
            mnemonic: mnemonic.into(),
            operands,
            options: BTreeSet::from([AssemblyOption::NoMemory]),
            declared_effects: BTreeSet::new(),
        }),
    );
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(operation);
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(2)],
    });
    Function::internal_helper(
        "synthetic_helper_isa",
        Signature::new(
            vec![Type::Scalar(ScalarType::U32); 2],
            vec![Type::Scalar(ScalarType::U32)],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    )
}
fn with_value<T>(
    function: &Function,
    meter: &mut dyn Meter,
    inspect: impl FnOnce(&Expression) -> T,
) -> Result<T, Error> {
    let template = derive(function, |_, _, _| Err("unexpected helper call"), meter)?;
    let result = template.instantiate(&[constant(u32::MAX.into()), constant(2)], meter);
    template.destroy(meter)?;
    let (expression, storage) = result?;
    let observed = inspect(&expression);
    drop(expression);
    meter.release(storage)?;
    Ok(observed)
}
#[test]
fn all_six_native_inline_values_keep_exact_u32_operands_and_modular_contract() {
    for (mnemonic, operation) in [
        ("v_mov_b32", None),
        ("v_add_u32", Some(ProductionSemanticBinaryOpV2::Add)),
        ("v_sub_u32", Some(ProductionSemanticBinaryOpV2::Subtract)),
        ("v_and_b32", Some(ProductionSemanticBinaryOpV2::BitAnd)),
        ("v_or_b32", Some(ProductionSemanticBinaryOpV2::BitOr)),
        ("v_xor_b32", Some(ProductionSemanticBinaryOpV2::BitXor)),
    ] {
        let original = function(mnemonic);
        let before = original.clone();
        let expected = operation.map_or_else(
            || constant(u32::MAX.into()),
            |operation| Expression::Binary {
                operation,
                scalar: U32,
                overflow: ProductionOverflowContractV2::Wrapping,
                lhs: Box::new(constant(u32::MAX.into())),
                rhs: Box::new(constant(2)),
            },
        );
        let (result, storage, _, _) = run(1_000_000, 1 << 24, |meter| {
            with_value(&original, meter, |actual| {
                assert_eq!(*actual, expected, "{mnemonic}")
            })
        });
        result.unwrap();
        assert_eq!(storage, 4096);
        assert_eq!(
            original, before,
            "interpretation must not erase or rewrite the ISA"
        );
    }
}
#[test]
fn native_inline_malformed_options_effects_registers_types_and_uses_refuse() {
    for mutation in 0..14 {
        let mut function = function("v_sub_u32");
        let body = function.body.as_mut().unwrap();
        let operation = &mut body.blocks[0].operations[0];
        let OperationKind::InlineAssembly(assembly) = &mut operation.kind else {
            unreachable!()
        };
        match mutation {
            0 => {
                assembly.options.insert(AssemblyOption::Pure);
            }
            1 => {
                assembly.options.clear();
            }
            2 => {
                assembly.options.insert(AssemblyOption::ReadOnly);
            }
            3 => {
                assembly.declared_effects.insert(AssemblyEffect::ReadGlobal);
            }
            4 => {
                assembly.mnemonic = "s_mov_b32".into();
                assembly.operands.pop();
                for operand in &mut assembly.operands {
                    operand.constraint = AssemblyConstraint::Sgpr32;
                }
            }
            5 => assembly.operands[1].constraint = AssemblyConstraint::Sgpr32,
            6 => assembly.operands[1].kind = AssemblyOperandKind::Input(ValueId(500)),
            7 => assembly.operands[1].kind = AssemblyOperandKind::Input(ValueId(2)),
            8 => assembly.operands[0].kind = AssemblyOperandKind::Output { result_index: 1 },
            9 => assembly.source.statement = [0; 32],
            10 => operation.results[0].ty = Type::Scalar(ScalarType::I32),
            11 => {
                operation
                    .results
                    .push(ValueDef::new(ValueId(3), Type::BOOL));
            }
            12 => assembly.mnemonic = "v_mul_lo_u32".into(),
            13 => function.signature.parameters[0] = Type::Scalar(ScalarType::U64),
            _ => unreachable!(),
        }
        let (result, storage, _, _) = run(1_000_000, 1 << 24, |meter| {
            with_value(&function, meter, |_| ())
        });
        assert!(result.is_err(), "mutation {mutation}");
        assert_eq!(storage, 4096, "mutation {mutation}");
    }
}
#[test]
fn native_inline_parent_does_not_change_checked_or_partial_operand_contracts() {
    let mut function = function("v_add_u32");
    let operations = &mut function.body.as_mut().unwrap().blocks[0].operations;
    let OperationKind::InlineAssembly(assembly) = &mut operations[0].kind else {
        unreachable!()
    };
    assembly.operands[1].kind = AssemblyOperandKind::Input(ValueId(3));
    operations.insert(
        0,
        Operation::checked_binary(
            ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U32)),
            ValueDef::new(ValueId(4), Type::BOOL),
            CheckedBinaryOperator::Subtract,
            ValueId(0),
            ValueId(1),
        ),
    );
    let (result, storage, _, _) = run(1_000_000, 1 << 24, |meter| {
        with_value(&function, meter, |expression| {
            let Expression::Binary { overflow, lhs, .. } = expression else {
                panic!("missing ISA value")
            };
            assert_eq!(*overflow, ProductionOverflowContractV2::Wrapping);
            assert!(matches!(
                **lhs,
                Expression::Binary {
                    overflow: ProductionOverflowContractV2::Checked,
                    ..
                }
            ));
        })
    });
    result.unwrap();
    assert_eq!(storage, 4096);
    let first = &mut function.body.as_mut().unwrap().blocks[0].operations[0];
    first.kind = OperationKind::Binary {
        op: BinaryOp::Subtract,
        lhs: ValueId(0),
        rhs: ValueId(1),
    };
    first.results.truncate(1);
    let (result, storage, _, _) = run(1_000_000, 1 << 24, |meter| {
        with_value(&function, meter, |_| ())
    });
    assert!(result.is_err());
    assert_eq!(storage, 4096);
}
#[test]
fn native_inline_exact_work_and_storage_cutoffs_preserve_nonzero_floor() {
    let function = function("v_sub_u32");
    let probe = |meter: &mut TestMeter<'_>| with_value(&function, meter, |_| ());
    let (result, storage, work, peak) = run(1_000_000, 1 << 24, probe);
    result.unwrap();
    assert_eq!(storage, 4096);
    for (work, peak, succeeds) in [
        (work, peak, true),
        (work - 1, peak, false),
        (work, peak - 1, false),
    ] {
        let (result, storage, _, _) = run(work, peak, probe);
        assert_eq!(result.is_ok(), succeeds);
        assert_eq!(storage, 4096);
    }
}
