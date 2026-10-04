use super::*;
use crate::portable_reference_v1::signature::ReferenceReturnShapeV1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_mir_model::semantic_mir_v1::{SemanticExternAbiV1, SemanticFunctionSafetyV1};

const LIMIT: usize = 100_000_000;
const SOURCE: usize = crate::mixed_optimizer_refinement_v26::SOURCE_LIMIT;

fn signature(scalar: Scalar) -> Signature {
    Signature::new(
        vec![Input::Scalar(scalar)].into_boxed_slice(),
        vec![Input::Scalar(scalar)].into_boxed_slice(),
        ReferenceReturnShapeV1::Unit,
        SemanticExternAbiV1::Rust,
        SemanticFunctionSafetyV1::Safe,
        false,
    )
    .unwrap()
}

fn run(
    value: &Expression,
    scalar: Scalar,
    work: usize,
    storage: usize,
) -> (Result<(Scalar, String)>, usize, usize, usize) {
    let mut work = Work::new(work);
    let mut budget = Budget::new(&mut work, storage);
    budget.reserve_storage(37).unwrap();
    let result = budget.with_prepaid_scope(37, 1, 1, SOURCE, |budget| {
        let signature = signature(scalar);
        let context = Context {
            signature: &signature,
            relations: &[Relation::ScalarInput {
                argument: 0,
                scalar,
            }],
            index_bits: 64,
            rank: 1,
        };
        let mut out = Writer::new(budget)?;
        let result = context.expression(value, &mut out)?;
        Ok((result, out.finish()?))
    });
    let state = (budget.work(), budget.storage(), budget.peak_storage());
    (result, state.0, state.1, state.2)
}

fn constant(scalar: Scalar, bits: u128) -> Expression {
    Expression::Constant(Constant::Scalar { scalar, bits })
}

#[test]
fn registered_cpu_expression_uses_actual_scalar_arguments_and_exact_float_bits() {
    for scalar in [
        Scalar::Bool,
        Scalar::U8,
        Scalar::I32,
        Scalar::Usize,
        Scalar::F32,
        Scalar::F64,
    ] {
        let (result, _, floor, _) = run(
            &Expression::KernelScalarArgument { argument: 0 },
            scalar,
            LIMIT,
            LIMIT,
        );
        assert_eq!(result.unwrap(), (scalar, "arguments[0int]".to_owned()));
        assert_eq!(floor, 37);
    }
    for (scalar, bits) in [
        (Scalar::F32, 0x7fc0_0042),
        (Scalar::F64, 0x7ff8_0000_0000_0042),
    ] {
        assert_eq!(
            run(&constant(scalar, bits), scalar, LIMIT, LIMIT)
                .0
                .unwrap(),
            (scalar, format!("MemoryValueV30::Scalar({bits}int)"))
        );
    }
}

#[test]
fn registered_cpu_expression_refuses_loads_checked_arithmetic_and_foreign_arguments() {
    for expression in [
        Expression::KernelScalarArgument { argument: 1 },
        Expression::PointCoordinate { axis: 1 },
        constant(Scalar::U8, 256),
        Expression::InputLoad {
            reference_argument: 0,
            index: Box::new(constant(Scalar::Usize, 0)),
        },
        Expression::Binary {
            operation: Binary::Add,
            lhs: Box::new(constant(Scalar::U32, 1)),
            rhs: Box::new(constant(Scalar::U32, 2)),
            checked: false,
        },
        Expression::Binary {
            operation: Binary::BitXor,
            lhs: Box::new(constant(Scalar::U32, 1)),
            rhs: Box::new(constant(Scalar::U32, 2)),
            checked: true,
        },
    ] {
        let (result, _, floor, _) = run(&expression, Scalar::U32, LIMIT, LIMIT);
        assert!(matches!(result, Err(Error::Statement(_))));
        assert_eq!(floor, 37);
    }
}

#[test]
fn registered_cpu_expression_operator_table_matches_the_independent_scalar_model() {
    for (operator, integer, float) in [
        (Binary::Equal, 4, 16),
        (Binary::NotEqual, 5, 17),
        (Binary::LessThan, 6, 18),
        (Binary::LessEqual, 7, 19),
        (Binary::GreaterThan, 8, 20),
        (Binary::GreaterEqual, 9, 21),
    ] {
        assert_eq!(
            binary(operator, Scalar::I32).unwrap(),
            (integer, Scalar::Bool)
        );
        assert_eq!(
            binary(operator, Scalar::F64).unwrap(),
            (float, Scalar::Bool)
        );
    }
    for (operator, code) in [(Binary::BitAnd, 1), (Binary::BitOr, 2), (Binary::BitXor, 3)] {
        assert_eq!(binary(operator, Scalar::U32).unwrap(), (code, Scalar::U32));
    }
    for (operator, code) in [
        (Binary::Add, 11),
        (Binary::Subtract, 12),
        (Binary::Multiply, 13),
        (Binary::Divide, 14),
        (Binary::Remainder, 15),
    ] {
        assert_eq!(binary(operator, Scalar::F32).unwrap(), (code, Scalar::F32));
    }
}

#[test]
fn registered_cpu_expression_has_exact_work_storage_and_one_short_refusals() {
    let expression = Expression::Binary {
        operation: Binary::BitXor,
        lhs: Box::new(Expression::KernelScalarArgument { argument: 0 }),
        rhs: Box::new(constant(Scalar::U32, 17)),
        checked: false,
    };
    let measured = run(&expression, Scalar::U32, LIMIT, LIMIT);
    assert!(measured.0.is_ok());
    let exact = run(&expression, Scalar::U32, measured.1, measured.3);
    assert_eq!(exact.0.unwrap(), measured.0.unwrap());
    assert_eq!((exact.1, exact.2, exact.3), (measured.1, 37, measured.3));
    for (work, storage) in [(measured.1 - 1, measured.3), (measured.1, measured.3 - 1)] {
        let short = run(&expression, Scalar::U32, work, storage);
        assert!(matches!(short.0, Err(Error::Resource(_))));
        assert_eq!(short.2, 37);
    }
}
