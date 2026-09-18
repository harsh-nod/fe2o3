//! Test-only observations of existing owners and request preparation. No
//! alternate compile/proof path and no source/N/refinement composition claim.
use super::*;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Case {
    Direct,
    Nested,
    Swapped,
    Alternate,
}
impl Case {
    pub(crate) fn feature(self) -> &'static str {
        match self {
            Self::Direct => "defined-helper-reference",
            Self::Nested => "defined-helper-reference-nested",
            Self::Swapped => "defined-helper-reference-swapped",
            Self::Alternate => "defined-helper-reference-alternate",
        }
    }
    fn helpers(self) -> usize {
        if self == Self::Nested { 2 } else { 1 }
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub(crate) struct Observation {
    source_count: usize,
    request_count: usize,
    source_helpers: usize,
    native_helpers: usize,
    native_calls: usize,
    semantic_digest: [u8; 32],
    native_digest: [u8; 32],
    native_length: u64,
    request_shape_checked: bool,
    gpu_expression_exact: bool,
    reference_expression_exact: bool,
    expression_relation_exact: bool,
    issue: Option<String>,
}

struct Active {
    case: Case,
    observation: Observation,
}
thread_local! {
    static ACTIVE: RefCell<Option<Active>> = const { RefCell::new(None) };
}
struct Restore(Option<Active>);
impl Drop for Restore {
    fn drop(&mut self) {
        ACTIVE.with(|slot| *slot.borrow_mut() = self.0.take());
    }
}
pub(crate) fn observe<R>(case: Case, run: impl FnOnce() -> R) -> (R, Observation) {
    let old = ACTIVE.with(|slot| {
        slot.replace(Some(Active {
            case,
            observation: Observation::default(),
        }))
    });
    let restore = Restore(old);
    let result = run();
    let observation = ACTIVE.with(|slot| slot.borrow_mut().take().unwrap().observation);
    drop(restore);
    (result, observation)
}

pub(crate) fn observe_source(source: &fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1) {
    ACTIVE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(active) = slot.as_mut() else { return };
        let observation = &mut active.observation;
        observation.source_count += 1;
        if observation.source_count != 1 {
            observation.issue = Some("duplicate source observation".into());
            return;
        }
        let semantic = source.semantic_ssa().source_semantic();
        observation.semantic_digest = *semantic.semantic_sha256().as_bytes();
        observation.source_helpers = semantic
            .functions()
            .iter()
            .filter(|f| {
                f.role() == fe2o3_mir_model::semantic_mir_v1::SemanticFunctionRoleV1::InternalHelper
            })
            .count();
        let native = source.executable();
        observation.native_digest = *native.canonical().identity().digest();
        observation.native_length = native.canonical().identity().canonical_length();
        let module = native.module();
        observation.native_helpers = module
            .functions
            .iter()
            .filter(|f| f.role == fe2o3_kernel_ir::FunctionRole::InternalHelper)
            .count();
        observation.native_calls = module
            .functions
            .iter()
            .filter_map(|f| f.body.as_ref())
            .flat_map(|body| &body.blocks)
            .flat_map(|block| &block.operations)
            .filter(|op| match &op.kind {
                fe2o3_kernel_ir::OperationKind::Call { callee, .. } => {
                    module.functions.iter().any(|f| {
                        &f.id == callee && f.role == fe2o3_kernel_ir::FunctionRole::InternalHelper
                    })
                }
                _ => false,
            })
            .count();
    });
}

fn scalar() -> ProductionSemanticScalarTypeV2 {
    ProductionSemanticScalarTypeV2::Integer {
        signed: false,
        bits: 32,
    }
}
fn argument(index: u32) -> ProductionSemanticExpressionV2 {
    ProductionSemanticExpressionV2::Symbol {
        symbol: crate::reference_effect_v1::kernel_scalar_symbol_v2(index).unwrap(),
        scalar: scalar(),
    }
}
fn binary(
    operation: ProductionSemanticBinaryOpV2,
    lhs: ProductionSemanticExpressionV2,
    rhs: ProductionSemanticExpressionV2,
) -> ProductionSemanticExpressionV2 {
    ProductionSemanticExpressionV2::Binary {
        operation,
        scalar: scalar(),
        overflow: ProductionOverflowContractV2::Wrapping,
        lhs: Box::new(lhs),
        rhs: Box::new(rhs),
    }
}
fn leaf(left: u32, right: u32, alternate: bool) -> ProductionSemanticExpressionV2 {
    binary(
        if alternate {
            ProductionSemanticBinaryOpV2::BitOr
        } else {
            ProductionSemanticBinaryOpV2::BitAnd
        },
        argument(left),
        ProductionSemanticExpressionV2::Unary {
            operation: ProductionSemanticUnaryOpV2::Not,
            scalar: scalar(),
            operand: Box::new(argument(right)),
        },
    )
}
fn expected(
    case: Case,
) -> (
    ProductionSemanticExpressionV2,
    ProductionSemanticExpressionV2,
) {
    let reference = if case == Case::Nested {
        binary(
            ProductionSemanticBinaryOpV2::BitXor,
            leaf(1, 0, false),
            argument(0),
        )
    } else {
        leaf(0, 1, false)
    };
    let gpu = match case {
        Case::Direct | Case::Nested => reference.clone(),
        Case::Swapped => leaf(1, 0, false),
        Case::Alternate => leaf(0, 1, true),
    };
    (gpu, reference)
}

fn expression(
    kernel: &ProductionRankedKernelV1,
    value: ProductionRankedValueV1,
) -> Option<&ProductionSemanticExpressionV2> {
    let mut expressions = kernel
        .blocks()
        .iter()
        .flat_map(|b| b.operations())
        .filter_map(|op| match op {
            ProductionRankedOperationV1::SemanticExpression {
                result, expression, ..
            } if value == ProductionRankedValueV1::Local(*result) => Some(expression),
            _ => None,
        });
    let result = expressions.next()?;
    expressions.next().is_none().then_some(result)
}

fn check_request(
    request: &CompilerOwnedReferenceEffectRequestV2,
    case: Case,
) -> Result<(bool, bool, bool), &'static str> {
    let [site] = request.requests.as_slice() else {
        return Err("exactly one prepared effect request");
    };
    let operation = request
        .kernel
        .blocks()
        .get(site.block)
        .and_then(|b| b.operations().get(site.operation))
        .ok_or("actual prepared request coordinate")?;
    let ProductionRankedOperationV1::RequestEffectRefinement { contract, subjects } = operation
    else {
        return Err("actual RequestEffectRefinement operation");
    };
    if *subjects != site.subjects || subjects.safe_reference_kind() != SafeReferenceKindV2::Mir {
        return Err("retained exact request subjects");
    }
    let write = contract.gpu_write_site();
    let op = request
        .kernel
        .blocks()
        .get(write.block() as usize)
        .and_then(|b| b.operations().get(write.operation() as usize))
        .ok_or("actual ValueAccess coordinate")?;
    let ProductionRankedOperationV1::ValueAccess {
        kind,
        view,
        indices,
        value,
    } = op
    else {
        return Err("actual ValueAccess operation");
    };
    if *kind != dialect_kernel::AccessKindAttr::Write
        || *view != contract.view()
        || indices != contract.indices()
        || *value != contract.gpu_value()
        || contract.reference_output_site().argument() != 2
        || indices.len() != 1
        || contract.gpu_coordinates() != contract.reference_coordinates()
        || contract.gpu_coordinates().len() != 1
        || request.proof_timeout_seconds != LOCAL_PROOF_TIMEOUT_SECONDS_V2
    {
        return Err("exact write/reference site and coordinate join");
    }
    for value in [
        contract.gpu_domain(),
        contract.reference_domain(),
        contract.gpu_precondition(),
        contract.reference_precondition(),
    ] {
        if expression(&request.kernel, value)
            != Some(&ProductionSemanticExpressionV2::Constant {
                scalar: ProductionSemanticScalarTypeV2::Bool,
                bits: 1,
            })
        {
            return Err("exact prepared domain and precondition");
        }
    }
    let all = request.kernel.blocks().iter().flat_map(|b| b.operations());
    if all
        .clone()
        .filter(|op| matches!(op, ProductionRankedOperationV1::ValueAccess { .. }))
        .count()
        != 1
        || all
            .filter(|op| {
                matches!(
                    op,
                    ProductionRankedOperationV1::RequestEffectRefinement { .. }
                )
            })
            .count()
            != 1
    {
        return Err("complete single-effect prepared roster");
    }
    let gpu = expression(&request.kernel, contract.gpu_value()).ok_or("unique GPU expression")?;
    let reference = expression(&request.kernel, contract.reference_value())
        .ok_or("unique reference expression")?;
    let (expected_gpu, expected_reference) = expected(case);
    Ok((
        gpu == &expected_gpu,
        reference == &expected_reference,
        (gpu == reference) == matches!(case, Case::Direct | Case::Nested),
    ))
}
pub(super) fn observe_request(request: &CompilerOwnedReferenceEffectRequestV2) {
    ACTIVE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(active) = slot.as_mut() else { return };
        let observation = &mut active.observation;
        observation.request_count += 1;
        if observation.request_count != 1 {
            observation.issue = Some("duplicate prepared request".into());
            return;
        }
        match check_request(request, active.case) {
            Ok((gpu, reference, relation)) => {
                observation.request_shape_checked = true;
                observation.gpu_expression_exact = gpu;
                observation.reference_expression_exact = reference;
                observation.expression_relation_exact = relation;
            }
            Err(error) => observation.issue = Some(error.into()),
        }
    });
}
impl Observation {
    pub(crate) fn validate(&self, case: Case) -> Result<(), String> {
        if self.source_count != 1
            || self.request_count != 1
            || self.issue.is_some()
            || self.source_helpers != case.helpers()
            || self.native_helpers != case.helpers()
            || self.native_calls != case.helpers()
            || self.native_length == 0
            || self.semantic_digest == [0; 32]
            || self.native_digest == [0; 32]
            || !self.request_shape_checked
            || !self.gpu_expression_exact
            || !self.reference_expression_exact
            || !self.expression_relation_exact
        {
            return Err(format!(
                "incomplete or altered source/request observation: {self:?}"
            ));
        }
        Ok(())
    }
}

#[test]
fn operand_and_callee_controls_do_not_match_the_original_recipe() {
    let direct = expected(Case::Direct);
    for case in [Case::Swapped, Case::Alternate] {
        let changed = expected(case);
        assert_ne!(changed.0, direct.0);
        assert_eq!(changed.1, direct.1);
        assert_ne!(changed.0, changed.1);
    }
    assert_eq!(expected(Case::Nested).0, expected(Case::Nested).1);
}

#[test]
fn inactive_nested_and_unwinding_observations_restore_the_outer_scope() {
    assert!(ACTIVE.with(|slot| slot.borrow().is_none()));
    let (_, outer) = observe(Case::Direct, || {
        let (_, inner) = observe(Case::Nested, || ());
        assert!(inner.validate(Case::Nested).is_err());
        assert_eq!(
            ACTIVE.with(|slot| slot.borrow().as_ref().unwrap().case),
            Case::Direct
        );
        let panic = std::panic::catch_unwind(|| observe(Case::Alternate, || panic!("test unwind")));
        assert!(panic.is_err());
        assert_eq!(
            ACTIVE.with(|slot| slot.borrow().as_ref().unwrap().case),
            Case::Direct
        );
    });
    assert!(outer.validate(Case::Direct).is_err());
    assert!(ACTIVE.with(|slot| slot.borrow().is_none()));
}
