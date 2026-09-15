use super::*;
use fixtures::{ROOT, Shape, owner, use_site};

fn resolve(
    owner: &fe2o3_pliron::ProductionSemanticSsaOwnerV1,
    work: usize,
) -> (Result<ProductionSemanticExpressionV2, &'static str>, usize) {
    let function = owner.execution_view_for_root(ROOT).unwrap().body();
    let query = owner.source_query_for_root(ROOT, function).unwrap();
    let mut resolver =
        GpuSemanticExpressionResolverV2::new(owner.source_semantic().types(), function);
    resolver.work = work;
    let result = (|| {
        resolver.source_ssa = Some(SourceNormalizationV1::new(
            query,
            function,
            &mut resolver.work,
        )?);
        let (site, operand) = use_site(function);
        let expression = resolver
            .with_source_site_v1(site, |resolver| resolver.resolve_operand_v2(operand, 0))?;
        resolver.validate_source_expression_v1(expression)
    })();
    (result, resolver.work)
}

#[test]
fn scalar_source_resolver_retains_typed_select_both_values_and_condition_moves() {
    for shape in [Shape::Diamond, Shape::Reversed, Shape::MoveCondition] {
        let owner = owner(shape);
        let expression = resolve(&owner, 0).0.unwrap();
        let ProductionSemanticExpressionV2::Select {
            scalar,
            condition,
            when_true,
            when_false,
        } = expression
        else {
            panic!("existing typed Select")
        };
        assert_eq!(
            scalar,
            ProductionSemanticScalarTypeV2::Integer {
                signed: false,
                bits: 32
            }
        );
        assert!(matches!(
            *condition,
            ProductionSemanticExpressionV2::Symbol {
                scalar: ProductionSemanticScalarTypeV2::Bool,
                ..
            }
        ));
        let bits = |value: ProductionSemanticExpressionV2| match value {
            ProductionSemanticExpressionV2::Constant { bits, .. } => bits,
            _ => panic!("exact incoming constant"),
        };
        let expected = if matches!(shape, Shape::Reversed) {
            (7, 9)
        } else {
            (9, 7)
        };
        assert_eq!((bits(*when_true), bits(*when_false)), expected);
    }
}

#[test]
fn scalar_source_resolver_rejects_non_boolean_selector_and_partial_unchosen_arm() {
    assert_eq!(
        resolve(&owner(Shape::IntegerCondition), 0).0,
        Err("scalar SSA selector lacks an exact Boolean or proved typed-global condition")
    );
    assert_eq!(
        resolve(&owner(Shape::PartialArm), 0).0,
        Err("scalar SSA selection has a partial arithmetic domain")
    );
    for shape in [Shape::Bypass, Shape::Backedge, Shape::FalseEdge] {
        assert_eq!(resolve(&owner(shape), 0).0, Err(CLOSED));
    }
}

#[test]
fn scalar_source_resolver_uses_overwritten_definition_not_stale_merge() {
    assert!(matches!(
        resolve(&owner(Shape::Overwrite), 0).0.unwrap(),
        ProductionSemanticExpressionV2::Constant { bits: 13, .. }
    ));
}

#[test]
fn scalar_source_resolver_site_owner_cloned_operand_and_escape_gates_remain_closed() {
    let owner = owner(Shape::Diamond);
    let function = owner.execution_view_for_root(ROOT).unwrap().body();
    let cloned = function.clone();
    assert!(matches!(
        SourceNormalizationV1::new(
            owner.source_query_for_root(ROOT, function).unwrap(),
            &cloned,
            &mut 0
        ),
        Err(CUSTODY)
    ));
    let (site, operand) = use_site(function);
    let cloned_operand = operand.clone();
    let mut resolver =
        GpuSemanticExpressionResolverV2::new(owner.source_semantic().types(), function);
    resolver.source_ssa = Some(
        SourceNormalizationV1::new(
            owner.source_query_for_root(ROOT, function).unwrap(),
            function,
            &mut resolver.work,
        )
        .unwrap(),
    );
    assert_eq!(
        resolver.with_source_site_v1(site, |resolver| resolver
            .resolve_operand_v2(&cloned_operand, 0)),
        Err(CUSTODY)
    );
    assert!(resolver.source_ssa.as_ref().unwrap().active_site.is_none());
    resolver.address_escaped.insert(2);
    assert_eq!(
        resolver.with_source_site_v1(site, |resolver| resolver.resolve_operand_v2(operand, 0)),
        Err(CUSTODY)
    );
    assert!(resolver.source_ssa.as_ref().unwrap().active_site.is_none());
}

#[test]
fn scalar_source_resolver_does_not_scan_unrelated_sites_or_reset_work() {
    let baseline = resolve(&owner(Shape::Diamond), 0);
    let large = resolve(&owner(Shape::ManyNops), 0);
    assert_eq!(baseline, large);
    let owner = owner(Shape::Diamond);
    let limit = fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2;
    let exact = resolve(&owner, limit - baseline.1);
    assert_eq!(exact.0, baseline.0);
    assert_eq!(exact.1, limit);
    let short = resolve(&owner, limit - baseline.1 + 1);
    assert_eq!(
        short.0,
        Err("GPU semantic expression exceeds its bounded node budget")
    );
    assert!(short.1 > limit);
}
