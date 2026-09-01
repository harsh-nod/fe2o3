//! Same-session rustc HIR/raw-MIR to semantic-MIR scalar correspondence.

use std::{collections::HashMap, fmt};

use fe2o3_mir_model::{
    InertSourceMirScalarRefinementEvidenceV1, MAX_SOURCE_MIR_SCALAR_CERTIFICATES_V1,
    RustcSourceMirScalarObservationV1, SourceMirLocalBindingV1, SourceMirScalarOperatorV1,
    semantic_mir_v1::AdmittedInertSemanticMirV1,
};
use rustc_ast::ast::BinOpKind;
use rustc_hir::intravisit::Visitor;
use rustc_hir::{Expr, ExprKind, HirId, PatKind, QPath, def::Res, intravisit};
use rustc_middle::{
    mir::{BinOp, Operand, Place, Rvalue, StatementKind},
    ty::{Instance, TyCtxt, TyKind, UintTy},
};

use crate::{
    rustc_semantic_adapter_v1::{
        canonical_source_provenance_v1, rustc_hir_binding_sha256_v1,
        rustc_hir_expression_sha256_v1, rustc_hir_owner_sha256_v1, rustc_mir_body_sha256_v1,
    },
    rustc_semantic_plan_v1::{ProductionSemanticPreflightPlanV1, RetainedSemanticBodyProducerV1},
};

const MAX_HIR_SCALAR_CANDIDATES_V1: usize = 16_384;
const MAX_MACRO_EXPANSION_DEPTH_V1: usize = 64;

/// Authenticated, authority-free evidence retained by the production transaction.
pub(crate) struct AuthenticatedSourceMirScalarEvidenceV1 {
    records: Box<[InertSourceMirScalarRefinementEvidenceV1]>,
}

impl AuthenticatedSourceMirScalarEvidenceV1 {
    pub(crate) fn records(&self) -> &[InertSourceMirScalarRefinementEvidenceV1] {
        &self.records
    }

    pub(crate) fn revalidate(&self) -> Result<(), ProductionSourceMirScalarErrorV1> {
        for record in &self.records {
            record.revalidate().map_err(|error| {
                ProductionSourceMirScalarErrorV1::SemanticValidation(error.to_string())
            })?;
            if record.grants_authority() {
                return Err(ProductionSourceMirScalarErrorV1::AuthorityEscalation);
            }
        }
        Ok(())
    }

    pub(crate) const fn grants_authority(&self) -> bool {
        false
    }
}

#[derive(Debug)]
pub(crate) enum ProductionSourceMirScalarErrorV1 {
    MissingLocalHir,
    HirCandidateLimit,
    CertificateLimit,
    AmbiguousSourceExpression,
    SemanticMapping,
    SemanticValidation(String),
    AuthorityEscalation,
}

impl fmt::Display for ProductionSourceMirScalarErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingLocalHir => {
                formatter.write_str("source scalar candidate has no local HIR")
            }
            Self::HirCandidateLimit => {
                formatter.write_str("source scalar HIR candidate limit exceeded")
            }
            Self::CertificateLimit => {
                formatter.write_str("source scalar certificate limit exceeded")
            }
            Self::AmbiguousSourceExpression => {
                formatter.write_str("raw MIR statement matches multiple HIR scalar expressions")
            }
            Self::SemanticMapping => {
                formatter.write_str("source/raw-MIR/semantic local mapping is inconsistent")
            }
            Self::SemanticValidation(error) => write!(
                formatter,
                "source-to-MIR semantic validation failed: {error}"
            ),
            Self::AuthorityEscalation => {
                formatter.write_str("source-to-MIR evidence attempted to grant authority")
            }
        }
    }
}

pub(crate) fn derive_production_source_mir_scalar_evidence_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    semantic: &AdmittedInertSemanticMirV1,
) -> Result<AuthenticatedSourceMirScalarEvidenceV1, ProductionSourceMirScalarErrorV1> {
    let mut records = Vec::new();
    for (function, producer) in plan.function_producers().iter().zip(plan.body_producers()) {
        derive_function_v1(tcx, function.instance, producer, semantic, &mut records)?;
    }
    let evidence = AuthenticatedSourceMirScalarEvidenceV1 {
        records: records.into_boxed_slice(),
    };
    evidence.revalidate()?;
    Ok(evidence)
}

struct HirCandidateV1<'tcx> {
    expression: &'tcx Expr<'tcx>,
    operator: SourceMirScalarOperatorV1,
    left: HirId,
    right: HirId,
}

struct HirCandidateVisitorV1<'tcx> {
    values: Vec<HirCandidateV1<'tcx>>,
    exceeded: bool,
}

impl<'tcx> Visitor<'tcx> for HirCandidateVisitorV1<'tcx> {
    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if let ExprKind::Binary(operator, left, right) = expression.kind
            && let (Some(operator), Some(left), Some(right)) = (
                source_operator_v1(operator.node),
                direct_local_v1(left),
                direct_local_v1(right),
            )
        {
            if self.values.len() == MAX_HIR_SCALAR_CANDIDATES_V1 {
                self.exceeded = true;
                return;
            }
            self.values.push(HirCandidateV1 {
                expression,
                operator,
                left,
                right,
            });
        }
        intravisit::walk_expr(self, expression);
    }
}

fn derive_function_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    producer: &RetainedSemanticBodyProducerV1,
    semantic: &AdmittedInertSemanticMirV1,
    records: &mut Vec<InertSourceMirScalarRefinementEvidenceV1>,
) -> Result<(), ProductionSourceMirScalarErrorV1> {
    let Some(local) = instance.def_id().as_local() else {
        return Ok(());
    };
    let hir_body = tcx
        .hir_maybe_body_owned_by(local)
        .ok_or(ProductionSourceMirScalarErrorV1::MissingLocalHir)?;
    let hir_owner_sha256 = rustc_hir_owner_sha256_v1(tcx, local)
        .ok_or(ProductionSourceMirScalarErrorV1::MissingLocalHir)?;
    let mut parameters = HashMap::new();
    for (ordinal, parameter) in hir_body.params.iter().enumerate() {
        let PatKind::Binding(_, binding, _, None) = parameter.pat.kind else {
            continue;
        };
        let raw_local = u32::try_from(ordinal + 1)
            .map_err(|_| ProductionSourceMirScalarErrorV1::SemanticMapping)?;
        parameters.insert(binding, raw_local);
    }
    let mut visitor = HirCandidateVisitorV1 {
        values: Vec::new(),
        exceeded: false,
    };
    visitor.visit_body(hir_body);
    if visitor.exceeded {
        return Err(ProductionSourceMirScalarErrorV1::HirCandidateLimit);
    }
    let typeck = tcx.typeck(local);
    let raw_body = tcx.instance_mir(instance.def);
    let rustc_mir_body_sha256 = rustc_mir_body_sha256_v1(tcx, instance);
    for raw_block in raw_body.basic_blocks.indices() {
        let semantic_block = producer
            .raw_to_semantic_blocks
            .get(raw_block.index())
            .copied()
            .ok_or(ProductionSourceMirScalarErrorV1::SemanticMapping)?;
        let raw_data = &raw_body.basic_blocks[raw_block];
        let block_producer = producer
            .blocks
            .iter()
            .find(|block| block.rustc_block as usize == raw_block.index())
            .ok_or(ProductionSourceMirScalarErrorV1::SemanticMapping)?;
        for (statement_index, statement) in raw_data.statements.iter().enumerate() {
            let StatementKind::Assign(assignment) = &statement.kind else {
                continue;
            };
            let (destination, rvalue) = &**assignment;
            let Rvalue::BinaryOp(raw_operator, operands) = rvalue else {
                continue;
            };
            let Some(operator) = mir_operator_v1(*raw_operator, tcx.sess.overflow_checks()) else {
                continue;
            };
            let Some(left_raw) = direct_mir_local_v1(&operands.0) else {
                continue;
            };
            let Some(right_raw) = direct_mir_local_v1(&operands.1) else {
                continue;
            };
            if !destination.projection.is_empty()
                || !matches!(
                    rvalue.ty(&raw_body.local_decls, tcx).kind(),
                    TyKind::Uint(UintTy::U32)
                )
            {
                continue;
            }
            let source = block_producer
                .statements
                .get(statement_index)
                .ok_or(ProductionSourceMirScalarErrorV1::SemanticMapping)?
                .provenance;
            let matching = visitor
                .values
                .iter()
                .filter(|candidate| {
                    candidate.operator == operator
                        && parameters.get(&candidate.left) == Some(&left_raw)
                        && parameters.get(&candidate.right) == Some(&right_raw)
                        && matches!(
                            typeck.expr_ty(candidate.expression).kind(),
                            TyKind::Uint(UintTy::U32)
                        )
                        && canonical_source_provenance_v1(
                            tcx,
                            candidate.expression.span,
                            MAX_MACRO_EXPANSION_DEPTH_V1,
                        )
                        .is_ok_and(|candidate_source| candidate_source.provenance() == source)
                })
                .collect::<Vec<_>>();
            let [candidate] = matching.as_slice() else {
                if matching.is_empty() {
                    continue;
                }
                return Err(ProductionSourceMirScalarErrorV1::AmbiguousSourceExpression);
            };
            let destination_raw = u32::try_from(destination.local.index())
                .map_err(|_| ProductionSourceMirScalarErrorV1::SemanticMapping)?;
            let semantic_statement = u32::try_from(statement_index)
                .map_err(|_| ProductionSourceMirScalarErrorV1::SemanticMapping)?;
            let expression_identity = rustc_hir_expression_sha256_v1(tcx, candidate.expression);
            let observation = RustcSourceMirScalarObservationV1 {
                rustc_hir_owner_sha256: hir_owner_sha256,
                source_expression_sha256: expression_identity,
                rustc_mir_body_sha256,
                source,
                semantic_function: producer.function,
                semantic_block: semantic_block.index(),
                semantic_statement,
                operator,
                left: local_binding_v1(tcx, producer, semantic, candidate.left, left_raw)?,
                right: local_binding_v1(tcx, producer, semantic, candidate.right, right_raw)?,
                destination: local_binding_v1(
                    tcx,
                    producer,
                    semantic,
                    candidate.expression.hir_id,
                    destination_raw,
                )?,
            };
            if records.len() == MAX_SOURCE_MIR_SCALAR_CERTIFICATES_V1 {
                return Err(ProductionSourceMirScalarErrorV1::CertificateLimit);
            }
            records.push(
                InertSourceMirScalarRefinementEvidenceV1::from_rustc_observations(
                    semantic,
                    vec![observation],
                )
                .map_err(|error| {
                    ProductionSourceMirScalarErrorV1::SemanticValidation(error.to_string())
                })?,
            );
        }
    }
    Ok(())
}

fn local_binding_v1(
    tcx: TyCtxt<'_>,
    producer: &RetainedSemanticBodyProducerV1,
    semantic: &AdmittedInertSemanticMirV1,
    source_binding: HirId,
    raw_local: u32,
) -> Result<SourceMirLocalBindingV1, ProductionSourceMirScalarErrorV1> {
    let semantic_local = producer
        .raw_to_semantic_locals
        .get(raw_local as usize)
        .copied()
        .ok_or(ProductionSourceMirScalarErrorV1::SemanticMapping)?;
    let local = semantic
        .functions()
        .get(producer.function.index() as usize)
        .and_then(|function| function.locals().get(semantic_local.index() as usize))
        .ok_or(ProductionSourceMirScalarErrorV1::SemanticMapping)?;
    Ok(SourceMirLocalBindingV1::new(
        rustc_hir_binding_sha256_v1(tcx, source_binding),
        raw_local,
        semantic_local,
        *local.identity().as_bytes(),
    ))
}

fn direct_local_v1(expression: &Expr<'_>) -> Option<HirId> {
    let ExprKind::Path(QPath::Resolved(None, path)) = expression.kind else {
        return None;
    };
    let Res::Local(binding) = path.res else {
        return None;
    };
    Some(binding)
}

fn direct_mir_local_v1(operand: &Operand<'_>) -> Option<u32> {
    let (Operand::Copy(place) | Operand::Move(place)) = operand else {
        return None;
    };
    direct_place_local_v1(*place)
}

fn direct_place_local_v1(place: Place<'_>) -> Option<u32> {
    place.projection.is_empty().then(|| place.local.as_u32())
}

fn source_operator_v1(operation: BinOpKind) -> Option<SourceMirScalarOperatorV1> {
    match operation {
        BinOpKind::Add => Some(SourceMirScalarOperatorV1::Add),
        BinOpKind::Sub => Some(SourceMirScalarOperatorV1::Subtract),
        BinOpKind::Mul => Some(SourceMirScalarOperatorV1::Multiply),
        BinOpKind::BitAnd => Some(SourceMirScalarOperatorV1::BitAnd),
        BinOpKind::BitOr => Some(SourceMirScalarOperatorV1::BitOr),
        BinOpKind::BitXor => Some(SourceMirScalarOperatorV1::BitXor),
        _ => None,
    }
}

fn mir_operator_v1(operation: BinOp, overflow_checks: bool) -> Option<SourceMirScalarOperatorV1> {
    match operation {
        BinOp::Add if !overflow_checks => Some(SourceMirScalarOperatorV1::Add),
        BinOp::Sub if !overflow_checks => Some(SourceMirScalarOperatorV1::Subtract),
        BinOp::Mul if !overflow_checks => Some(SourceMirScalarOperatorV1::Multiply),
        BinOp::BitAnd => Some(SourceMirScalarOperatorV1::BitAnd),
        BinOp::BitOr => Some(SourceMirScalarOperatorV1::BitOr),
        BinOp::BitXor => Some(SourceMirScalarOperatorV1::BitXor),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, process::Command};

    use fe2o3_mir_model::semantic_mir_v1::*;
    use rustc_driver::{Callbacks, Compilation};
    use rustc_hir::def::DefKind;
    use rustc_interface::interface::Compiler;

    use super::*;
    use crate::{
        rustc_semantic_plan_v1::{
            RetainedSemanticBlockProducerV1, RetainedSemanticBodyProducerV1,
            RetainedSemanticSourceProducerV1,
        },
        test_temp_dir::TestTempDir,
    };

    fn bytes(tag: u8) -> [u8; 32] {
        [tag; 32]
    }

    fn source_for_span(tcx: TyCtxt<'_>, span: rustc_span::Span) -> SemanticSourceProvenanceV1 {
        canonical_source_provenance_v1(tcx, span, MAX_MACRO_EXPANSION_DEPTH_V1)
            .unwrap()
            .provenance()
    }

    fn direct_value(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
        SemanticAbiValueV1::new(
            ty,
            SemanticAbiPassModeV1::Direct(
                SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                    SemanticAbiExtensionV1::None,
                    0,
                    None,
                )
                .unwrap(),
            ),
        )
    }

    fn test_semantic_and_producer<'tcx>(
        tcx: TyCtxt<'tcx>,
        instance: Instance<'tcx>,
    ) -> (AdmittedInertSemanticMirV1, RetainedSemanticBodyProducerV1) {
        let raw = tcx.instance_mir(instance.def);
        assert_eq!(raw.arg_count, 2);
        assert_eq!(raw.local_decls.len(), 3);
        let ty = SemanticTypeIdV1::from_index(0);
        let type_decl = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(1)),
            SemanticLayoutIdentityV1::from_sha256(bytes(2)),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(4),
                4,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, 32, 4),
                    SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            }),
        );
        let function_source = source_for_span(tcx, tcx.def_span(instance.def_id()));
        let locals = raw
            .local_decls
            .iter_enumerated()
            .map(|(local, declaration)| {
                assert!(matches!(declaration.ty.kind(), TyKind::Uint(UintTy::U32)));
                let role = match local.as_u32() {
                    0 => SemanticLocalRoleV1::Return,
                    ordinal => SemanticLocalRoleV1::Argument(ordinal - 1),
                };
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256(bytes(10 + local.as_u32() as u8)),
                    ty,
                    role,
                    source_for_span(tcx, declaration.source_info.span),
                )
            })
            .collect::<Vec<_>>();
        let place = |local: u32| {
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
        };
        let mut semantic_blocks = Vec::new();
        let mut retained_blocks = Vec::new();
        for (raw_block, data) in raw.basic_blocks.iter_enumerated() {
            let mut statements = Vec::new();
            let mut retained_statements = Vec::new();
            for statement in &data.statements {
                let source = source_for_span(tcx, statement.source_info.span);
                retained_statements.push(RetainedSemanticSourceProducerV1::from_test_provenance(
                    source,
                ));
                let kind = match &statement.kind {
                    StatementKind::Assign(assignment) => {
                        let (destination, rvalue) = &**assignment;
                        let Rvalue::BinaryOp(BinOp::BitXor, operands) = rvalue else {
                            panic!("fixture produced an unexpected assignment: {rvalue:?}")
                        };
                        let destination = direct_place_local_v1(*destination).unwrap();
                        let left = direct_mir_local_v1(&operands.0).unwrap();
                        let right = direct_mir_local_v1(&operands.1).unwrap();
                        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                            place(destination),
                            SemanticRvalueV1::new(
                                ty,
                                SemanticRvalueKindV1::Binary {
                                    operation: SemanticBinaryOpV1::BitXor,
                                    left: SemanticOperandV1::Copy(place(left)),
                                    right: SemanticOperandV1::Copy(place(right)),
                                },
                            ),
                        ))
                    }
                    other => panic!("fixture produced an unexpected statement: {other:?}"),
                };
                statements.push(SemanticStatementV1::new(source, kind));
            }
            assert!(matches!(
                data.terminator().kind,
                rustc_middle::mir::TerminatorKind::Return
            ));
            let block_source = source_for_span(tcx, data.terminator().source_info.span);
            semantic_blocks.push(
                SemanticBasicBlockV1::new(
                    SemanticBlockIdentityV1::from_sha256(bytes(20 + raw_block.as_u32() as u8)),
                    block_source,
                    statements,
                    SemanticTerminatorV1::new(block_source, SemanticTerminatorKindV1::Return),
                )
                .unwrap(),
            );
            retained_blocks.push(RetainedSemanticBlockProducerV1 {
                identity: SemanticBlockIdentityV1::from_sha256(bytes(
                    20 + raw_block.as_u32() as u8,
                )),
                rustc_block: raw_block.as_u32(),
                source: RetainedSemanticSourceProducerV1::from_test_provenance(block_source),
                statements: retained_statements.into_boxed_slice(),
                terminator: RetainedSemanticSourceProducerV1::from_test_provenance(block_source),
            });
        }
        let abi = SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256(bytes(30)),
            SemanticLayoutIdentityV1::from_sha256(bytes(31)),
            SemanticCanonAbiV1::Rust,
            false,
            false,
            vec![direct_value(ty), direct_value(ty)],
            direct_value(ty),
        )
        .unwrap();
        let function = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256(bytes(32)),
            SemanticFunctionRoleV1::KernelRoot,
            SemanticItemDefinitionIdentityV1::from_sha256(bytes(33)),
            SemanticMonomorphizationIdentityV1::from_sha256(bytes(34)),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(35)),
            SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(36)),
            function_source,
            abi,
            locals,
            SemanticBlockIdV1::from_index(0),
            semantic_blocks,
        )
        .unwrap();
        let semantic = InertSemanticMirRequestV1::new(
            SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(37))),
            vec![type_decl],
            vec![],
            vec![],
            vec![],
            vec![function],
            vec![SemanticFunctionIdV1::from_index(0)],
        )
        .unwrap()
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
        let producer = RetainedSemanticBodyProducerV1 {
            function: SemanticFunctionIdV1::from_index(0),
            source: RetainedSemanticSourceProducerV1::from_test_provenance(function_source),
            locals: Box::new([]),
            raw_to_semantic_locals: (0..raw.local_decls.len())
                .map(|index| SemanticLocalIdV1::from_index(index as u32))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            entry: SemanticBlockIdV1::from_index(0),
            blocks: retained_blocks.into_boxed_slice(),
            raw_to_semantic_blocks: (0..raw.basic_blocks.len())
                .map(|index| SemanticBlockIdV1::from_index(index as u32))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
            debug_scopes: Box::new([]),
            debug_variables: Box::new([]),
            debug_capture_gap: None,
        };
        (semantic, producer)
    }

    #[derive(Default)]
    struct SourceMirCallbacksV1 {
        result: Option<Result<usize, String>>,
    }

    impl Callbacks for SourceMirCallbacksV1 {
        fn after_analysis<'tcx>(&mut self, _compiler: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
            let result = (|| {
                let definition = tcx
                    .iter_local_def_id()
                    .find(|definition| {
                        tcx.def_kind(definition.to_def_id()) == DefKind::Fn
                            && tcx.item_name(definition.to_def_id()).as_str() == "supported"
                    })
                    .ok_or_else(|| "missing supported function".to_owned())?;
                let instance = Instance::mono(tcx, definition.to_def_id());
                let (semantic, producer) = test_semantic_and_producer(tcx, instance);
                let mut records = Vec::new();
                derive_function_v1(tcx, instance, &producer, &semantic, &mut records)
                    .map_err(|error| error.to_string())?;
                for record in &records {
                    record.revalidate().map_err(|error| error.to_string())?;
                }
                Ok(records.len())
            })();
            self.result = Some(result);
            Compilation::Stop
        }
    }

    #[test]
    fn operator_relation_rejects_overflow_checked_and_substituted_operations() {
        assert_eq!(
            mir_operator_v1(BinOp::Add, false),
            Some(SourceMirScalarOperatorV1::Add)
        );
        assert_eq!(mir_operator_v1(BinOp::Add, true), None);
        assert_ne!(
            mir_operator_v1(BinOp::Sub, false),
            Some(SourceMirScalarOperatorV1::Add)
        );
        assert_eq!(
            source_operator_v1(BinOpKind::BitXor),
            Some(SourceMirScalarOperatorV1::BitXor)
        );
    }

    #[test]
    fn real_rustc_bitxor_expression_produces_one_authenticated_record() {
        let directory = TestTempDir::create("fe2o3-source-mir-scalar-v1");
        let source: PathBuf = directory.path().join("fixture.rs");
        let output = directory.path().join("fixture.rmeta");
        fs::write(
            &source,
            "#[inline(never)]\npub fn supported(left: u32, right: u32) -> u32 { left ^ right }\n",
        )
        .unwrap();
        let sysroot = crate::process_execution::capture_output(
            Command::new("rustc").args(["--print", "sysroot"]),
        )
        .unwrap();
        assert!(sysroot.status.success());
        let args = vec![
            "rustc".to_owned(),
            "--crate-name".to_owned(),
            "fe2o3_source_mir_scalar_fixture".to_owned(),
            "--crate-type".to_owned(),
            "lib".to_owned(),
            "--edition".to_owned(),
            "2024".to_owned(),
            "--emit".to_owned(),
            "metadata".to_owned(),
            "--sysroot".to_owned(),
            String::from_utf8(sysroot.stdout).unwrap().trim().to_owned(),
            "-Coverflow-checks=on".to_owned(),
            "-o".to_owned(),
            output.display().to_string(),
            source.display().to_string(),
        ];
        let mut callbacks = SourceMirCallbacksV1::default();
        rustc_driver::run_compiler(&args, &mut callbacks);
        assert_eq!(callbacks.result.unwrap().unwrap(), 1);
    }
}
