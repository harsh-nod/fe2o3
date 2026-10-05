use super::*;
use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1 as Work, Signature};
use fe2o3_mir_model::semantic_mir_v1::*;
use std::cell::{Cell, RefCell};

const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const ROOT: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(0);

fn cleanup() -> ScopedSourceCleanupV29 {
    ScopedSourceCleanupV29 {
        denied: Cell::new(false),
        fault: RefCell::new(None),
        fault_storage: Cell::new(None),
        fault_skip: Cell::new(0),
    }
}

fn scalar_source() -> AdmittedInertSemanticMirV1 {
    let scalar = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 32, 4),
        SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
    );
    let types = vec![
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([1; 32]),
            SemanticLayoutIdentityV1::from_sha256([2; 32]),
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                0,
                1,
                SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                1,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Unit,
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([3; 32]),
            SemanticLayoutIdentityV1::from_sha256([4; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(4),
                4,
                SemanticBackendReprV1::scalar(scalar),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            }),
        ),
    ];
    let attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([5; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            U32,
            SemanticAbiPassModeV1::Direct(attributes),
        ))],
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
    .unwrap();
    let source = SemanticSourceProvenanceV1::unavailable();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([6; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([7; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([8; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([9; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([10; 32]),
        source,
        abi,
        vec![
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([11; 32]),
                UNIT,
                SemanticLocalRoleV1::Return,
                source,
            ),
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([12; 32]),
                U32,
                SemanticLocalRoleV1::Argument(0),
                source,
            ),
        ],
        SemanticBlockIdV1::from_index(0),
        vec![
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([13; 32]),
                source,
                vec![],
                SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"lowerer_argument_v18_fixture".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([14; 32]),
        SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
    ));
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![ROOT],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap()
}

fn with_data<'work, R>(
    cleanup: &ScopedSourceCleanupV29,
    budget: &mut ArgumentBudgetV1<'work>,
    use_data: impl for<'scope> FnOnce(
        ArgumentViewDataV18<'scope>,
        &'scope mut ArgumentBudgetV1<'work>,
    ) -> Result<R, ProductionSemanticKirErrorV1>,
) -> Result<R, ProductionSemanticKirErrorV1> {
    let source = scalar_source();
    let target = Function::kernel_entry(
        "lowerer_argument_v18_fixture",
        Signature::new(vec![Type::Scalar(ScalarType::U32)], vec![]),
        vec![ValueId(0)],
        vec![],
    );
    let rows = [SemanticKirParameterBindingV1 {
        correspondence_owner: ROOT,
        semantic_function: ROOT,
        semantic_local: SemanticLocalIdV1::from_index(1),
        kernel_ir_value: ValueId(0),
    }];
    with_parameter_data_v18(
        &source,
        ArgumentEntryV18 {
            correspondence_owner: ROOT,
            semantic_function: ROOT,
            kernel_ir_function: &target.id,
            role: SemanticKirFunctionRoleV1::KernelEntry,
            descriptor_root: None,
        },
        &target,
        ArgumentTraceV1 {
            direct: &rows,
            components: &[],
            ignored: &[],
        },
        cleanup,
        budget,
        use_data,
    )
}

fn selected() -> ProductionSemanticKirErrorV1 {
    unsupported(0, None, None, "selected lowerer V18 argument error")
}

fn assert_selected(result: Result<(), ProductionSemanticKirErrorV1>) {
    assert!(matches!(
        result,
        Err(ProductionSemanticKirErrorV1::Unsupported {
            function: 0,
            block: None,
            statement: None,
            detail: "selected lowerer V18 argument error",
        })
    ));
}

#[test]
fn original_argument_v18_restores_selected_constructor_and_node_errors_after_linked_denial() {
    for in_node in [false, true] {
        for deny in [false, true] {
            let cleanup = cleanup();
            let mut work = Work::new(usize::MAX);
            let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
            budget.reserve_storage(29).unwrap();
            let invoked = Cell::new(0);
            let result = with_data(&cleanup, &mut budget, |view, budget| {
                if in_node {
                    view.visit_nodes_scoped(budget, |node, _| {
                        invoked.set(invoked.get() + 1);
                        assert_eq!(node.semantic_type(), U32);
                        if deny {
                            cleanup.deny_refund();
                        }
                        Err(selected())
                    })
                } else {
                    invoked.set(invoked.get() + 1);
                    if deny {
                        cleanup.deny_refund();
                    }
                    Err(selected())
                }
            });
            assert_selected(result);
            assert_eq!(invoked.get(), 1);
            assert_eq!(cleanup.is_denied(), deny);
            if deny {
                assert!(budget.storage() > 29);
            } else {
                assert_eq!(budget.storage(), 29);
            }
        }
    }
}

#[test]
fn original_argument_v18_bridge_restores_only_the_visitor_sentinel() {
    assert_selected(argument_visitor_result_v1::<()>(
        Err(source_arguments_v1::ProductionSourceArgumentErrorV1::Visitor),
        Some(selected()),
    ));
    assert!(matches!(
        argument_visitor_result_v1::<()>(
            Err(source_arguments_v1::ProductionSourceArgumentErrorV1::CorrespondenceMismatch),
            Some(selected()),
        ),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
    assert!(matches!(
        argument_visitor_result_v1::<()>(
            Err(source_arguments_v1::ProductionSourceArgumentErrorV1::Visitor),
            None,
        ),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
}

#[repr(align(4096))]
struct OwnedVisitor<'a> {
    bytes: [u8; 8192],
    cleanup: &'a ScopedSourceCleanupV29,
    dropped: &'a Cell<usize>,
    deny: bool,
    panic: bool,
}

impl OwnedVisitor<'_> {
    fn observe(&self) {
        std::hint::black_box(self);
    }
}

impl Drop for OwnedVisitor<'_> {
    fn drop(&mut self) {
        self.dropped.set(self.dropped.get() + 1);
        if self.deny {
            self.cleanup.deny_refund();
        }
        if self.panic {
            std::panic::resume_unwind(Box::new(1733_u32));
        }
    }
}

#[test]
fn original_argument_v18_owned_aligned_visitor_refuses_before_invocation_and_drops_once() {
    fn run(limit: usize) -> (bool, usize, usize, usize, usize, bool) {
        let cleanup = cleanup();
        let invoked = Cell::new(0);
        let dropped = Cell::new(0);
        let entered = Cell::new(false);
        let mut work = Work::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, limit);
        let result = with_data(&cleanup, &mut budget, |view, budget| {
            entered.set(true);
            let capture = OwnedVisitor {
                bytes: [7; 8192],
                cleanup: &cleanup,
                dropped: &dropped,
                deny: false,
                panic: false,
            };
            let invoked = &invoked;
            view.visit_nodes_scoped(budget, move |_, _| {
                capture.observe();
                invoked.set(invoked.get() + 1);
                Ok(())
            })
        });
        if result.is_err() {
            assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Storage(_)
                    )
                )
            ));
            assert!(budget.failed_storage().is_some());
        }
        assert!(!cleanup.is_denied());
        (
            result.is_ok(),
            budget.peak_storage(),
            budget.storage(),
            invoked.get(),
            dropped.get(),
            entered.get(),
        )
    }
    let measured = run(usize::MAX);
    assert!(measured.0);
    assert_eq!(
        (measured.2, measured.3, measured.4, measured.5),
        (0, 1, 1, true)
    );
    let small_peak = {
        let cleanup = cleanup();
        let mut work = Work::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        with_data(&cleanup, &mut budget, |view, budget| {
            view.visit_nodes_scoped(budget, |_, _| Ok(()))
        })
        .unwrap();
        assert_eq!(budget.storage(), 0);
        budget.peak_storage()
    };
    assert!(
        measured.1
            >= small_peak
                + std::mem::size_of::<OwnedVisitor<'_>>()
                + std::mem::align_of::<OwnedVisitor<'_>>()
    );
    assert_eq!(run(measured.1), measured);
    let short = run(measured.1 - 1);
    assert!(!short.0);
    assert_eq!((short.2, short.3, short.4, short.5), (0, 0, 1, true));
}

#[test]
fn original_argument_v18_owned_visitor_drop_precedes_outer_refund_and_keeps_raw_payload() {
    // Drop-origin panic is tested only after success. A destructor panicking
    // during raw unwind retains ordinary Rust double-panic behavior.
    for mode in 0..4 {
        let cleanup = cleanup();
        let dropped = Cell::new(0);
        let invoked = Cell::new(0);
        let mut work = Work::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        budget.reserve_storage(41).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_data(&cleanup, &mut budget, |view, budget| {
                let capture = OwnedVisitor {
                    bytes: [9; 8192],
                    cleanup: &cleanup,
                    dropped: &dropped,
                    deny: mode != 3,
                    panic: mode == 3,
                };
                let invoked = &invoked;
                view.visit_nodes_scoped(budget, move |_, _| {
                    capture.observe();
                    invoked.set(invoked.get() + 1);
                    match mode {
                        1 => Err(selected()),
                        2 => std::panic::resume_unwind(Box::new(1732_u32)),
                        _ => Ok(()),
                    }
                })
            })
        }));
        assert_eq!((invoked.get(), dropped.get()), (1, 1));
        match mode {
            0 => assert!(matches!(
                result.unwrap(),
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Accounting
                    )
                )
            )),
            1 => assert_selected(result.unwrap()),
            2 => assert_eq!(*result.unwrap_err().downcast::<u32>().unwrap(), 1732),
            _ => assert_eq!(*result.unwrap_err().downcast::<u32>().unwrap(), 1733),
        }
        assert_eq!(cleanup.is_denied(), mode != 3);
        if mode == 3 {
            assert_eq!(budget.storage(), 41);
        } else {
            assert!(budget.storage() > 41);
        }
    }
}
