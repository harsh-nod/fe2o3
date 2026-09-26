use super::tests::{BOOL, Meter, assertion_function, constant, limits, ordinary_message, types};
use super::*;
use crate::semantic_scalar_carrier_v1::{
    ScalarCarrierErrorV1, ScalarCarrierMeterV1, exact_transparent_scalar_carrier_field_metered_v1,
};

const FIELD: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const MARKER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const CARRIER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
fn carrier_types(cycle: bool, wrong_offset: bool, indirect: bool) -> Vec<SemanticTypeDeclV1> {
    let backend = SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 16, 2),
        SemanticScalarValidityRangeV1::new(0, u16::MAX.into()),
    ));
    let properties =
        SemanticTypeAbiPropertiesV1::new(false, false).with_rustc_layout_is_noundef(true);
    let scalar = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([80; 32]),
        SemanticLayoutIdentityV1::from_sha256([80; 32]),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            2,
            2,
            SemanticFieldsShapeV1::Primitive,
            SemanticRustcVariantsV1::Single { index: 0 },
            backend,
            None,
            false,
            None,
            2,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 16,
        }),
    )
    .with_rustc_abi_properties(properties);
    let children = if cycle { vec![MARKER] } else { vec![] };
    let offsets = vec![0; children.len()];
    let order = (0..children.len() as u32).collect();
    let marker = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([81; 32]),
        SemanticLayoutIdentityV1::from_sha256([81; 32]),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            1,
            SemanticFieldsShapeV1::arbitrary(offsets.clone(), order).unwrap(),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            1,
            0,
            SemanticTypeLayoutDetailsV1::Aggregate(
                SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
            ),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(children).unwrap()),
    )
    .with_rustc_abi_properties(properties);
    let offsets = vec![u64::from(wrong_offset), 2];
    let carrier = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([82; 32]),
        SemanticLayoutIdentityV1::from_sha256([82; 32]),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            2,
            2,
            SemanticFieldsShapeV1::arbitrary(offsets.clone(), vec![0, 1]).unwrap(),
            SemanticRustcVariantsV1::Single { index: 0 },
            backend,
            None,
            false,
            None,
            2,
            0,
            SemanticTypeLayoutDetailsV1::Aggregate(
                SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
            ),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![FIELD, MARKER]).unwrap()),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(indirect, false).with_rustc_layout_is_noundef(true),
    );
    vec![scalar, marker, carrier]
}
#[derive(Debug, PartialEq, Eq)]
enum CarrierRefusal {
    Work,
    Storage,
}
struct CarrierMeter {
    work: usize,
    storage: usize,
    max_work: usize,
    max_storage: usize,
}
impl CarrierMeter {
    fn new(max_work: usize, max_storage: usize) -> Self {
        Self {
            work: 0,
            storage: 0,
            max_work,
            max_storage,
        }
    }
}
impl ScalarCarrierMeterV1 for CarrierMeter {
    type Error = CarrierRefusal;
    fn work(&mut self, n: usize) -> Result<(), Self::Error> {
        let next = self.work.checked_add(n).ok_or(CarrierRefusal::Work)?;
        if next > self.max_work {
            return Err(CarrierRefusal::Work);
        }
        self.work = next;
        Ok(())
    }
    fn reserve_storage(&mut self, n: usize) -> Result<(), Self::Error> {
        let next = self.storage.checked_add(n).ok_or(CarrierRefusal::Storage)?;
        if next > self.max_storage {
            return Err(CarrierRefusal::Storage);
        }
        self.storage = next;
        Ok(())
    }
}
#[test]
fn paid_and_compatibility_carrier_rules_preserve_exact_layout_abi_and_cycles() {
    for (cycle, offset, indirect, expected, work) in [
        (false, false, false, Some(FIELD), 9),
        (false, true, false, None, 9),
        (false, false, true, None, 9),
        (true, false, false, None, 12),
    ] {
        let types = carrier_types(cycle, offset, indirect);
        let mut meter = CarrierMeter::new(work, 3);
        assert_eq!(
            exact_transparent_scalar_carrier_field_metered_v1(&types, CARRIER, &mut meter).unwrap(),
            expected
        );
        assert_eq!(
            exact_transparent_scalar_carrier_field_v1(&types, CARRIER),
            expected
        );
        // One carrier, four offsets, three initialized visitation cells and marker recursion.
        assert_eq!(meter.work, work);
        assert_eq!(meter.storage, 3);
    }
}
#[test]
fn carrier_work_and_capacity_boundaries_fail_before_unpaid_use() {
    let types = carrier_types(false, false, false);
    let mut work = CarrierMeter::new(8, 3);
    assert_eq!(
        exact_transparent_scalar_carrier_field_metered_v1(&types, CARRIER, &mut work),
        Err(ScalarCarrierErrorV1::Meter(CarrierRefusal::Work))
    );
    assert_eq!(work.work, 8);
    assert_eq!(work.storage, 3);
    let mut storage = CarrierMeter::new(9, 2);
    assert_eq!(
        exact_transparent_scalar_carrier_field_metered_v1(&types, CARRIER, &mut storage),
        Err(ScalarCarrierErrorV1::Meter(CarrierRefusal::Storage))
    );
    assert_eq!(storage.work, 5);
    assert_eq!(storage.storage, 0);
}
#[test]
fn source_index_acceleration_does_not_change_exact_constant_ranges() {
    let types = types();
    let function = assertion_function(constant(BOOL, 1), ordinary_message());
    let mut meter = Meter::default();
    let mut analysis =
        SemanticAssertionAnalysisV1::new_metered(&types, &function, limits(), &mut meter).unwrap();
    analysis.enable_statement_index_v1(&mut meter).unwrap();
    assert!(analysis.has_statement_index_v1());
    let indexed = analysis
        .recipe_queries_v1(&mut meter)
        .range_at_operand(&constant(BOOL, 1), 0, 0)
        .unwrap();
    let scanned = analysis
        .recipe_queries_by_scanning_v1(&mut meter)
        .range_at_operand(&constant(BOOL, 1), 0, 0)
        .unwrap();
    assert_eq!(indexed, scanned);
    assert!(indexed.unwrap().is_exact(1));
}
#[test]
fn non_bool_actual_assertion_condition_never_mints_a_fact() {
    let types = types();
    let function = assertion_function(constant(super::tests::U8, 1), ordinary_message());
    let mut meter = Meter::default();
    let mut analysis =
        SemanticAssertionAnalysisV1::new_metered(&types, &function, limits(), &mut meter).unwrap();
    assert!(matches!(
        analysis
            .assertion_at_v1(SemanticBlockIdV1::from_index(0), &mut meter)
            .err()
            .unwrap(),
        SemanticAssertionMeteredErrorV1::Analysis(SemanticAssertionErrorV1::InvalidModel(
            "assertion condition is not source bool"
        ))
    ));
}
