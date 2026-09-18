use super::*;

#[derive(Default)]
struct TestMeter {
    work: usize,
    limit: usize,
    storage: usize,
    storage_limit: usize,
    peak: usize,
    exhausted: bool,
    ledger: Option<Box<fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1>>,
}

impl TestMeter {
    fn generous() -> Self {
        Self {
            limit: 1_000_000,
            storage_limit: 16 * 1024 * 1024,
            ..Self::default()
        }
    }
}

impl Meter for TestMeter {
    fn work(&mut self, amount: usize) -> Result<(), Error> {
        let next = self.work.checked_add(amount).ok_or("work overflow")?;
        if next > self.limit {
            self.exhausted = true;
            return Err("work limit");
        }
        self.work = next;
        Ok(())
    }
    fn reserve(&mut self, bytes: usize) -> Result<(), Error> {
        let next = self.storage.checked_add(bytes).ok_or("storage overflow")?;
        if next > self.storage_limit {
            self.exhausted = true;
            return Err("storage limit");
        }
        self.storage = next;
        self.peak = self.peak.max(next);
        Ok(())
    }
    fn release(&mut self, bytes: usize) -> Result<(), Error> {
        self.storage = self.storage.checked_sub(bytes).ok_or("storage underflow")?;
        Ok(())
    }
    fn exhausted(&self) -> bool {
        self.exhausted
    }
    fn storage(&self) -> Result<usize, Error> {
        Ok(self.storage)
    }
    fn identity(&mut self) -> Result<Ledger, Error> {
        let slot = self as *mut Self as usize;
        let work = self.ledger.get_or_insert_with(|| {
            Box::new(fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(
                usize::MAX,
            ))
        });
        let budget = fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(work, 0);
        Ok(Ledger {
            slot,
            work: budget.work_ledger_identity_v1(),
        })
    }
}

const U32: Scalar = Scalar::Integer {
    signed: false,
    bits: 32,
};
const I32: Scalar = Scalar::Integer {
    signed: true,
    bits: 32,
};

fn subtract(meter: &mut dyn Meter) -> Template {
    let mut template = Template::new(&[U32, U32], meter).unwrap();
    let left = template.push(U32, Kind::Parameter(0), meter).unwrap();
    let right = template.push(U32, Kind::Parameter(1), meter).unwrap();
    let result = template
        .push(
            U32,
            Kind::Binary(
                ProductionSemanticBinaryOpV2::Subtract,
                ProductionOverflowContractV2::Wrapping,
                left,
                right,
            ),
            meter,
        )
        .unwrap();
    template.finish(result, U32).unwrap();
    template
}

fn constant(bits: u64) -> Expression {
    Expression::Constant { scalar: U32, bits }
}

fn release_expression(expression: Expression, bytes: usize, meter: &mut dyn Meter) {
    drop(expression);
    meter.release(bytes).unwrap();
}

#[test]
fn exact_noncommutative_arguments_are_substituted_without_symbol_placeholders() {
    let mut meter = TestMeter::generous();
    let template = subtract(&mut meter);
    let (actual, bytes) = template
        .instantiate(&[constant(7), constant(11)], &mut meter)
        .unwrap();
    assert_eq!(
        actual,
        Expression::Binary {
            operation: ProductionSemanticBinaryOpV2::Subtract,
            scalar: U32,
            overflow: ProductionOverflowContractV2::Wrapping,
            lhs: Box::new(constant(7)),
            rhs: Box::new(constant(11)),
        }
    );
    release_expression(actual, bytes, &mut meter);
    template.destroy(&mut meter).unwrap();
    assert_eq!(meter.storage, 0);
}

#[test]
fn nested_templates_keep_each_call_arguments_separate() {
    let mut meter = TestMeter::generous();
    let callee = subtract(&mut meter);
    let mut caller = Template::new(&[U32, U32], &mut meter).unwrap();
    let x = caller.push(U32, Kind::Parameter(0), &mut meter).unwrap();
    let y = caller.push(U32, Kind::Parameter(1), &mut meter).unwrap();
    let xy = caller.append_call(&callee, &[x, y], &mut meter).unwrap();
    let yx = caller.append_call(&callee, &[y, x], &mut meter).unwrap();
    let result = caller.append_call(&callee, &[xy, yx], &mut meter).unwrap();
    caller.finish(result, U32).unwrap();
    let (actual, bytes) = caller
        .instantiate(&[constant(7), constant(11)], &mut meter)
        .unwrap();
    let Expression::Binary { lhs, rhs, .. } = &actual else {
        panic!("missing call result");
    };
    assert_ne!(lhs, rhs);
    assert_eq!(actual.template_validate().unwrap().nodes, 7);
    release_expression(actual, bytes, &mut meter);
    caller.destroy(&mut meter).unwrap();
    callee.destroy(&mut meter).unwrap();
    assert_eq!(meter.storage, 0);
}

#[test]
fn caller_source_symbol_is_an_ordinary_argument_not_a_template_parameter() {
    let mut meter = TestMeter::generous();
    let template = subtract(&mut meter);
    let symbol = Expression::Symbol {
        symbol: 0,
        scalar: U32,
    };
    let (actual, bytes) = template
        .instantiate(&[symbol.clone(), constant(3)], &mut meter)
        .unwrap();
    let Expression::Binary { lhs, .. } = &actual else {
        panic!("missing binary");
    };
    assert_eq!(lhs.as_ref(), &symbol);
    release_expression(actual, bytes, &mut meter);
    template.destroy(&mut meter).unwrap();
}

#[test]
fn foreign_edges_parameter_ordinals_and_call_types_are_refused() {
    let mut meter = TestMeter::generous();
    let callee = subtract(&mut meter);
    let mut caller = Template::new(&[I32], &mut meter).unwrap();
    assert!(caller.push(U32, Kind::Parameter(0), &mut meter).is_err());
    assert!(caller.push(I32, Kind::Parameter(1), &mut meter).is_err());
    assert!(
        caller
            .push(
                I32,
                Kind::Unary(
                    ProductionSemanticUnaryOpV2::Not,
                    Value {
                        arena: caller.brand,
                        index: 0
                    }
                ),
                &mut meter
            )
            .is_err()
    );
    let x = caller.push(I32, Kind::Parameter(0), &mut meter).unwrap();
    assert!(caller.append_call(&callee, &[x], &mut meter).is_err());
    assert!(caller.append_call(&callee, &[x, x], &mut meter).is_err());
    assert!(caller.finish(x, U32).is_err());
    caller.destroy(&mut meter).unwrap();
    callee.destroy(&mut meter).unwrap();
    assert_eq!(meter.storage, 0);
}

#[test]
fn final_wire_validation_rejects_malformed_typed_operator() {
    let mut meter = TestMeter::generous();
    let mut template = Template::new(&[U32], &mut meter).unwrap();
    let x = template.push(U32, Kind::Parameter(0), &mut meter).unwrap();
    let bad = template
        .push(
            Scalar::Bool,
            Kind::Unary(ProductionSemanticUnaryOpV2::Not, x),
            &mut meter,
        )
        .unwrap();
    template.finish(bad, Scalar::Bool).unwrap();
    let floor = meter.storage;
    assert!(template.instantiate(&[constant(1)], &mut meter).is_err());
    assert_eq!(meter.storage, floor);
    template.destroy(&mut meter).unwrap();
}

#[test]
fn exponential_tree_growth_is_rejected_while_arena_is_small() {
    let mut meter = TestMeter::generous();
    let mut template = Template::new(&[U32], &mut meter).unwrap();
    let mut value = template.push(U32, Kind::Parameter(0), &mut meter).unwrap();
    for _ in 0..12 {
        value = template
            .push(
                U32,
                Kind::Binary(
                    ProductionSemanticBinaryOpV2::BitXor,
                    ProductionOverflowContractV2::Wrapping,
                    value,
                    value,
                ),
                &mut meter,
            )
            .unwrap();
    }
    assert_eq!(template.nodes.len(), 13);
    assert_eq!(template.nodes[value.index].tree_nodes, 8191);
    assert!(
        template
            .push(
                U32,
                Kind::Unary(ProductionSemanticUnaryOpV2::Not, value),
                &mut meter
            )
            .is_ok()
    );
    assert!(
        template
            .push(
                U32,
                Kind::Binary(
                    ProductionSemanticBinaryOpV2::BitXor,
                    ProductionOverflowContractV2::Wrapping,
                    value,
                    value,
                ),
                &mut meter
            )
            .is_err()
    );
    template.destroy(&mut meter).unwrap();
}

#[test]
fn substitution_size_is_checked_before_output_tree_allocation() {
    let mut meter = TestMeter::generous();
    let mut template = Template::new(&[U32], &mut meter).unwrap();
    let mut value = template.push(U32, Kind::Parameter(0), &mut meter).unwrap();
    for _ in 0..12 {
        value = template
            .push(
                U32,
                Kind::Binary(
                    ProductionSemanticBinaryOpV2::BitXor,
                    ProductionOverflowContractV2::Wrapping,
                    value,
                    value,
                ),
                &mut meter,
            )
            .unwrap();
    }
    template.finish(value, U32).unwrap();
    let argument = Expression::Unary {
        operation: ProductionSemanticUnaryOpV2::Not,
        scalar: U32,
        operand: Box::new(constant(1)),
    };
    let floor = meter.storage;
    let before = meter.peak;
    assert!(template.instantiate(&[argument], &mut meter).is_err());
    assert_eq!(meter.storage, floor);
    assert!(meter.peak - before < 1024);
    template.destroy(&mut meter).unwrap();
}

#[test]
fn exact_and_one_short_instantiation_budgets_restore_the_input_floor() {
    let mut setup = TestMeter::generous();
    let template = subtract(&mut setup);
    let floor = setup.storage;
    let start = setup.work;
    setup.peak = floor;
    let (actual, bytes) = template
        .instantiate(&[constant(7), constant(11)], &mut setup)
        .unwrap();
    let work = setup.work - start;
    let peak = setup.peak;
    release_expression(actual, bytes, &mut setup);
    for (work_limit, storage_limit, succeeds) in [
        (work, peak, true),
        (work - 1, peak, false),
        (work, peak - 1, false),
    ] {
        let mut meter = TestMeter {
            limit: work_limit,
            storage: floor,
            storage_limit,
            peak: floor,
            ..TestMeter::default()
        };
        let result = template.instantiate(&[constant(7), constant(11)], &mut meter);
        assert_eq!(result.is_ok(), succeeds);
        if let Ok((actual, bytes)) = result {
            release_expression(actual, bytes, &mut meter);
        }
        assert_eq!(meter.storage, floor);
    }
    template.destroy(&mut setup).unwrap();
}

#[test]
fn failed_capacity_growth_preserves_existing_template_reservation() {
    let mut meter = TestMeter::generous();
    let mut template = Template::new(&[], &mut meter).unwrap();
    for bits in 0..8 {
        template
            .push(U32, Kind::Constant(bits), &mut meter)
            .unwrap();
    }
    let floor = meter.storage;
    meter.storage_limit = floor;
    assert!(template.push(U32, Kind::Constant(8), &mut meter).is_err());
    assert_eq!(template.nodes.len(), 8);
    assert_eq!(meter.storage, floor);
    template.destroy(&mut meter).unwrap();
    assert_eq!(meter.storage, 0);
}

#[test]
fn arena_brand_rejects_same_ordinal_and_stale_values_without_changing_output() {
    let mut meter = TestMeter::generous();
    let mut first = Template::new(&[U32], &mut meter).unwrap();
    let foreign = first.push(U32, Kind::Parameter(0), &mut meter).unwrap();
    first.finish(foreign, U32).unwrap();
    let mut second = Template::new(&[U32], &mut meter).unwrap();
    let local = second.push(U32, Kind::Parameter(0), &mut meter).unwrap();
    assert_eq!(foreign.index, local.index);
    assert_ne!(foreign.arena, local.arena);
    assert!(second.scalar(foreign).is_err());
    assert!(
        second
            .push(
                U32,
                Kind::Unary(ProductionSemanticUnaryOpV2::Not, foreign),
                &mut meter
            )
            .is_err()
    );
    assert!(second.finish(foreign, U32).is_err());
    second.finish(local, U32).unwrap();
    let (a, a_bytes) = first.instantiate(&[constant(7)], &mut meter).unwrap();
    let (b, b_bytes) = second.instantiate(&[constant(7)], &mut meter).unwrap();
    assert_eq!(a, b);
    assert_eq!(a_bytes, b_bytes);
    release_expression(a, a_bytes, &mut meter);
    release_expression(b, b_bytes, &mut meter);
    first.destroy(&mut meter).unwrap();
    let third = Template::new(&[U32], &mut meter).unwrap();
    assert_ne!(third.brand, foreign.arena);
    assert!(third.scalar(foreign).is_err());
    third.destroy(&mut meter).unwrap();
    second.destroy(&mut meter).unwrap();
    assert_eq!(meter.storage, 0);
}
