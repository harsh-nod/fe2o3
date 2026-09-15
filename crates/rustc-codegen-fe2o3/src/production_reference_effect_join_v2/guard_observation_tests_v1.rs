//! Observes the real extractor and bijection without changing their result.
//! Test-only data is neither a reference binding nor a production proof.
use super::{CompilerExtractedGpuOutputEffectV1, PreparedReferenceOutputV2};
use fe2o3_pliron::{ProductionRankedKernelV1, ProductionSemanticExpressionV2};
use std::cell::RefCell;

#[derive(Default)]
pub(crate) struct Observation {
    pub(crate) kernel: Option<ProductionRankedKernelV1>,
    pub(crate) effects: Option<Vec<CompilerExtractedGpuOutputEffectV1>>,
    pub(crate) paired: bool,
    pub(crate) values: Option<(
        ProductionSemanticExpressionV2,
        ProductionSemanticExpressionV2,
    )>,
}

thread_local! {
    static OBSERVATION: RefCell<Option<Observation>> = const { RefCell::new(None) };
}

pub(crate) fn with_observation<R>(run: impl FnOnce() -> R) -> (R, Observation) {
    OBSERVATION.with(|slot| {
        let mut slot = slot.borrow_mut();
        assert!(
            slot.is_none(),
            "nested guard observations are not supported"
        );
        *slot = Some(Observation::default());
    });
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            OBSERVATION.with(|slot| {
                slot.borrow_mut().take();
            });
        }
    }
    let reset = Reset;
    let result = run();
    let observation = OBSERVATION.with(|slot| slot.borrow_mut().take().unwrap());
    drop(reset);
    (result, observation)
}

pub(super) fn extracted(effects: &[CompilerExtractedGpuOutputEffectV1]) {
    OBSERVATION.with(|slot| {
        if let Some(observation) = slot.borrow_mut().as_mut() {
            assert!(
                observation.effects.is_none(),
                "fixture must retain exactly one root extraction"
            );
            assert_eq!(effects.len(), 1, "bounded point-output fixture");
            observation.effects = Some(effects.to_vec());
        }
    });
}

pub(super) fn paired() {
    OBSERVATION.with(|slot| {
        if let Some(observation) = slot.borrow_mut().as_mut() {
            assert!(observation.effects.is_some());
            assert!(!observation.paired);
            observation.paired = true;
        }
    });
}

pub(super) fn prepared_values(outputs: &[PreparedReferenceOutputV2]) {
    OBSERVATION.with(|slot| {
        if let Some(observation) = slot.borrow_mut().as_mut() {
            assert!(observation.paired);
            assert!(observation.values.is_none());
            let [output] = outputs else {
                panic!("bounded point-output fixture");
            };
            observation.values = Some((
                output.gpu_expression.clone(),
                output.reference_expression.clone(),
            ));
        }
    });
}

pub(super) fn prepared_kernel(kernel: &ProductionRankedKernelV1) {
    OBSERVATION.with(|slot| {
        if let Some(observation) = slot.borrow_mut().as_mut() {
            assert!(observation.paired && observation.values.is_some());
            assert!(observation.kernel.is_none());
            assert!(kernel.blocks().len() <= 256, "bounded source fixture");
            assert!(
                kernel
                    .blocks()
                    .iter()
                    .all(|block| block.operations().len() <= 1024)
            );
            observation.kernel = Some(kernel.clone());
        }
    });
}

pub(crate) fn assert_runtime_extent_guards(
    kernel: &ProductionRankedKernelV1,
    count: usize,
    length: u64,
) {
    use fe2o3_pliron::{
        ProductionRankedOperationV1 as Op, ProductionRankedTerminatorV1 as Term,
        ProductionRankedValueV1 as Value,
    };
    let constants = kernel
        .blocks()
        .iter()
        .flat_map(|block| block.operations())
        .filter_map(|operation| match operation {
            Op::IndexConstant { result, value } if *value == length => Some(Value::Local(*result)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let extents = kernel.blocks()[0]
        .operations()
        .iter()
        .filter_map(|operation| match operation {
            Op::ViewInSpace {
                shape,
                dynamic_extents,
                ..
            } if shape == &[0] => {
                let [extent] = dynamic_extents.as_slice() else {
                    panic!("one source slice extent")
                };
                assert!(
                    matches!(extent, Value::Argument(_)),
                    "length is still a runtime parameter"
                );
                Some(*extent)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(extents.len(), count);
    for extent in extents {
        assert!(kernel.blocks().iter().any(|block| {
            matches!(block.terminator(), Term::IndexEqual { lhs, rhs, .. }
                if (*lhs == extent && constants.contains(rhs)) || (*rhs == extent && constants.contains(lhs)))
        }), "source length guard must survive preparation, not become an assumed fact");
    }
}

pub(crate) fn assert_proved_load_selection(
    gpu: &ProductionSemanticExpressionV2,
    reference: &ProductionSemanticExpressionV2,
) {
    use fe2o3_pliron::ProductionSemanticScalarTypeV2;
    let ProductionSemanticExpressionV2::Select {
        condition,
        when_true,
        when_false,
        ..
    } = gpu
    else {
        panic!("expected a source-derived load selection, got {gpu:?}");
    };
    assert_eq!(
        **condition,
        ProductionSemanticExpressionV2::Constant {
            scalar: ProductionSemanticScalarTypeV2::Bool,
            bits: 1,
        }
    );
    assert!(matches!(reference, ProductionSemanticExpressionV2::Load(_)));
    assert_eq!(when_true.as_ref(), reference, "same exact read event");
    assert_eq!(
        **when_false,
        ProductionSemanticExpressionV2::Constant {
            scalar: reference.scalar(),
            bits: 0,
        }
    );
}

#[test]
fn source_guard_observation_is_not_reused_after_unwind_or_another_run() {
    assert!(std::panic::catch_unwind(|| with_observation(|| panic!("fixture unwind"))).is_err());
    for _ in 0..2 {
        let (value, observation) = with_observation(|| 42);
        assert_eq!(value, 42);
        assert!(observation.effects.is_none());
        assert!(!observation.paired);
        assert!(observation.values.is_none());
        assert!(observation.kernel.is_none());
    }
}
