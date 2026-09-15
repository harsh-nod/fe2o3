use super::*;
use dialect_kernel::{IndexType, IndexUnknownOp};
use pliron::builtin::{ops::FuncOp, types::FunctionType};

#[test]
fn exact_environment_casts_keep_unknown_sparse_values_and_preserve_origin() {
    let mut context = Context::new();
    dialect_kernel::register_dialect(
        &mut context,
        &pliron::dialect::DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
    )
    .unwrap();
    let index = IndexType::get(&context).into();
    let ty = FunctionType::get(&context, vec![index], vec![]);
    let function = FuncOp::new(&mut context, "environment_cast".try_into().unwrap(), ty);
    let entry = function.get_entry_block(&context);
    let argument = entry.deref(&context).get_argument(0);
    let mut casts = Vec::new();
    for bits in [8, 16, 32, 64] {
        let cast = IndexUnsignedCastOp::new(&mut context, argument, bits);
        cast.get_operation().insert_at_back(entry, &context);
        casts.push((bits, cast.result(&context)));
    }
    let unknown = IndexUnknownOp::new(&mut context);
    unknown.get_operation().insert_at_back(entry, &context);
    let unknown_value = unknown.result(&context);
    let unresolved = IndexUnsignedCastOp::new(&mut context, unknown_value, 8);
    unresolved.get_operation().insert_at_back(entry, &context);
    let ret = ReturnOp::new(&mut context);
    ret.get_operation().insert_at_back(entry, &context);
    pliron::operation::verify_operation(function.get_operation(), &context).unwrap();
    let sparse = crate::analyze_pliron_sparse_indices_v1(&context, &function).unwrap();
    for (bits, value) in casts {
        assert_eq!(sparse.fact(value), crate::SparseIndexFactV1::Unknown);
        assert_eq!(
            evaluate_trace_value_with_origin_v1(&context, &sparse, &[], &HashMap::new(), value, 0),
            None
        );
        for source in [255, 256, u64::MAX] {
            let environment = HashMap::from([(argument, source)]);
            assert_eq!(
                evaluate_trace_value_with_origin_v1(&context, &sparse, &[], &environment, value, 0),
                Some((source & (u64::MAX >> (64 - bits)), true))
            );
            assert_eq!(
                evaluate_trace_value_v1(
                    &context,
                    &sparse,
                    &[],
                    &environment,
                    unresolved.result(&context),
                    0
                ),
                None
            );
        }
    }
}
