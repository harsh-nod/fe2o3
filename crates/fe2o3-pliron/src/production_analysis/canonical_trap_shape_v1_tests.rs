use super::super::super::tests::{noop, with_checked};
use super::super::tests::fixture;
use super::*;
use fe2o3_kernel_ir::{CanonicalKirFunctionCoordinateV1, ValueId};

#[test]
fn shape_keeps_exact_owner_declarations_shared_sink_and_ordered_payloads() {
    for declaration in 0..=2 {
        let module = fixture(declaration);
        with_checked(&module, |checked, budget| {
            let original = checked.inventory(budget).unwrap().owner();
            let bytes = original.canonical().canonical_bytes().to_vec();
            let floor = budget.storage();
            with_canonical_trap_shape_v1(checked, budget, |shape, budget| {
                assert!(std::ptr::eq(shape.owner(budget)?, original));
                assert_eq!(shape.owner(budget)?.canonical().canonical_bytes(), bytes);
                assert_eq!(shape.owner(budget)?.module().kernels, module.kernels);
                assert_eq!(shape.module_function_count(budget)?, 3);
                assert_eq!(shape.definition_count(budget)?, 2);
                for (ordinal, coordinate) in (0..3).filter(|i| *i != declaration).enumerate() {
                    assert_eq!(
                        shape.definition_coordinate(ordinal, budget)?,
                        CanonicalKirFunctionCoordinateV1(coordinate as u32)
                    );
                }
                assert_eq!(shape.pair_count(budget)?, 1);
                let pair = shape.pair(0, budget)?;
                assert_eq!(
                    pair.declaration(),
                    CanonicalKirFunctionCoordinateV1(declaration as u32)
                );
                assert_eq!(pair.incoming_edges(), 0..2);
                let first = shape.incoming_edge(0, budget)?;
                let second = shape.incoming_edge(1, budget)?;
                assert!(first.success_when());
                assert!(!second.success_when());
                assert_eq!(first.condition().value, ValueId(40));
                assert_eq!(
                    first.definition().coordinate,
                    second.definition().coordinate
                );
                assert_eq!(first.success().arguments, &[ValueId(41)]);
                assert_eq!(second.success().arguments, &[ValueId(42)]);
                assert!(first.failure().arguments.is_empty());
                assert_eq!(first.failure().target, pair.terminal_block());
                assert_eq!(second.failure().target, pair.terminal_block());
                Ok(())
            })
            .unwrap();
            assert_eq!(budget.storage(), floor);
            assert_eq!(original.canonical().canonical_bytes(), bytes);
        });
    }
}

#[test]
fn shape_queries_charge_once_without_minting_native_reports() {
    with_checked(&fixture(1), |checked, budget| {
        with_canonical_trap_shape_v1(checked, budget, |shape, budget| {
            let before = budget.work();
            shape.owner(budget)?;
            shape.module_function_count(budget)?;
            shape.definition_count(budget)?;
            shape.definition_coordinate(0, budget)?;
            shape.pair_count(budget)?;
            shape.pair(0, budget)?;
            shape.incoming_edge(0, budget)?;
            assert_eq!(budget.work(), before + 7);
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn empty_pair_census_is_not_an_empty_definition_roster() {
    with_checked(&noop(), |checked, budget| {
        with_canonical_trap_shape_v1(checked, budget, |shape, budget| {
            assert_eq!(shape.pair_count(budget)?, 0);
            assert!(shape.definition_count(budget)? > 0);
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn invalid_query_is_sticky_and_next_callback_recovers() {
    with_checked(&fixture(1), |checked, budget| {
        let floor = budget.storage();
        let result = with_canonical_trap_shape_v1(checked, budget, |shape, budget| {
            assert!(shape.incoming_edge(usize::MAX, budget).is_err());
            assert!(shape.pair_count(budget).is_err());
            Ok(())
        });
        assert!(result.is_err());
        assert_eq!(budget.storage(), floor);
        with_canonical_trap_shape_v1(checked, budget, |shape, budget| {
            assert_eq!(shape.pair_count(budget)?, 1);
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn callback_panic_drops_payload_and_restores_entry_before_recovery() {
    struct Payload(std::sync::Arc<std::sync::atomic::AtomicBool>);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }
    with_checked(&fixture(1), |checked, budget| {
        let dropped = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let floor = budget.storage();
        let result = with_canonical_trap_shape_v1(checked, budget, |_, _| -> Result<(), Failure> {
            std::panic::panic_any(Payload(dropped.clone()));
        });
        assert!(result.is_err());
        assert!(dropped.load(std::sync::atomic::Ordering::SeqCst));
        assert_eq!(budget.storage(), floor);
        with_canonical_trap_shape_v1(checked, budget, |shape, budget| {
            shape.owner(budget)?;
            Ok(())
        })
        .unwrap();
    });
}

#[test]
fn orphan_terminal_and_incomplete_incoming_are_refused_before_callback() {
    for orphan in [false, true] {
        let mut module = fixture(1);
        let body = module
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "zeta")
            .unwrap()
            .body
            .as_mut()
            .unwrap();
        if orphan {
            body.blocks[0].terminator = Some(Terminator::Branch {
                target: fe2o3_kernel_ir::BlockId(92),
                arguments: vec![ValueId(41)],
            });
            body.blocks[1].terminator = Some(Terminator::Branch {
                target: fe2o3_kernel_ir::BlockId(93),
                arguments: vec![ValueId(42)],
            });
        } else {
            body.blocks[1].terminator = Some(Terminator::Branch {
                target: fe2o3_kernel_ir::BlockId(94),
                arguments: vec![],
            });
        }
        with_checked(&module, |checked, budget| {
            let floor = budget.storage();
            assert!(
                with_canonical_trap_shape_v1(checked, budget, |_, _| -> Result<(), Failure> {
                    panic!("invalid shape reached callback")
                })
                .is_err()
            );
            assert_eq!(budget.storage(), floor);
        });
    }
}
