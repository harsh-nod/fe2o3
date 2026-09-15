use super::{SourceBoundary, restore};

// Public inert MIR fixtures only. Source authority is supplied by the separate
// rustc collector boundary, not by these component identities.
include!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../fe2o3-pliron/src/production/semantic_ssa/defined_math_results/fixture.rs"
));

struct Fixture {
    source: AdmittedInertSemanticMirV1,
    root: SemanticFunctionDeclV1,
    helper: SemanticFunctionDeclV1,
    callables: Vec<SemanticCallableDeclV1>,
    boundary: SourceBoundary,
}

fn source_abi(tag: u8, inputs: &[u32], output: u32, kernel: bool) -> SemanticFunctionAbiV1 {
    let old = abi(tag, &[], output, kernel);
    let attrs = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    SemanticFunctionAbiV1::from_rustc_with_source_signature(
        old.identity(),
        old.layout_identity(),
        old.canon_abi(),
        old.extern_abi(),
        false,
        false,
        inputs.len() as u32,
        inputs.iter().copied().map(ty).collect(),
        ty(output),
        inputs
            .iter()
            .map(|&input| {
                SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                    ty(input),
                    if input == 1 {
                        SemanticAbiPassModeV1::Ignore
                    } else {
                        SemanticAbiPassModeV1::Direct(attrs)
                    },
                ))
            })
            .collect(),
        old.return_value().clone(),
    )
    .unwrap()
}

fn fixture() -> Fixture {
    let source = source(false, false);
    let root = function(
        135,
        source_abi(133, &[5], 0, true),
        vec![
            local(180, 0, SemanticLocalRoleV1::Return),
            local(181, 1, SemanticLocalRoleV1::Temporary),
            local(182, 5, SemanticLocalRoleV1::Argument(0)),
            local(183, 3, SemanticLocalRoleV1::Temporary),
            local(184, 4, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            block(180, vec![], call(2, vec![], 1, 1, 1)),
            block(
                181,
                vec![],
                call(
                    1,
                    vec![
                        SemanticOperandV1::Constant(SemanticConstantV1::new(
                            ty(1),
                            SemanticConstantValueV1::ZeroSized,
                        )),
                        SemanticOperandV1::Copy(place(2, 5)),
                    ],
                    0,
                    0,
                    2,
                ),
            ),
            block(182, vec![], SemanticTerminatorKindV1::Return),
        ],
        true,
    )
    .with_kernel_entry(source.functions()[0].kernel_entry().unwrap().clone());
    let helper = function(
        160,
        source_abi(160, &[1, 5], 0, false),
        vec![
            local(180, 0, SemanticLocalRoleV1::Return),
            local(181, 1, SemanticLocalRoleV1::Argument(0)),
            local(182, 2, SemanticLocalRoleV1::Temporary),
            local(183, 5, SemanticLocalRoleV1::Argument(1)),
        ],
        vec![block(
            180,
            vec![SemanticStatementV1::new(
                location(),
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    place(2, 2),
                    SemanticRvalueV1::new(
                        ty(2),
                        SemanticRvalueKindV1::Borrow {
                            kind: SemanticBorrowKindV1::Shared,
                            place: place(1, 1),
                        },
                    ),
                )),
            )],
            SemanticTerminatorKindV1::Return,
        )],
        false,
    );
    let callables = vec![
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
        terminal(
            180,
            source_abi(180, &[], 1, false),
            SemanticCompilerIntrinsicOperationV1::KernelContextIssue { context: ty(1) },
        ),
    ];
    let boundary = SourceBoundary {
        root: root.identity(),
        helper: helper.identity(),
        issuance_block: root.blocks()[0].identity(),
        issuance_terminal: SemanticFunctionIdentityV1::from_sha256([180; 32]),
        physical_arguments: 1,
        logical_arguments: 2,
    };
    Fixture {
        source,
        root,
        helper,
        callables,
        boundary,
    }
}

impl Fixture {
    fn restore(&self) -> Result<Option<SemanticFunctionDeclV1>, &'static str> {
        restore(
            &self.boundary,
            &self.root,
            SemanticFunctionIdV1::from_index(1),
            &self.helper,
            &self.callables,
        )
    }

    fn edit_block(
        &mut self,
        index: usize,
        edit: impl FnOnce(&mut Vec<SemanticStatementV1>, &mut SemanticTerminatorKindV1),
    ) {
        let mut blocks = self.root.blocks().to_vec();
        let old = &blocks[index];
        let mut statements = old.statements().to_vec();
        let mut terminator = old.terminator().kind().clone();
        edit(&mut statements, &mut terminator);
        blocks[index] = block(180 + index as u8, statements, terminator);
        self.root = function(
            135,
            self.root.abi().clone(),
            self.root.locals().to_vec(),
            blocks,
            true,
        )
        .with_kernel_entry(self.root.kernel_entry().unwrap().clone());
    }

    fn admit(&self, root: SemanticFunctionDeclV1) -> AdmittedInertSemanticMirV1 {
        let mut types = self.source.types().to_vec();
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([214; 32]),
            SemanticLayoutIdentityV1::from_sha256([214; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(4),
                4,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::float(32, 4),
                    SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 }),
        ));
        InertSemanticMirRequestV1::new_with_callables(
            self.source.target(),
            types,
            vec![],
            vec![],
            vec![],
            vec![root, self.helper.clone()],
            self.callables.clone(),
            vec![SemanticFunctionIdV1::from_index(0)],
        )
        .unwrap()
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap()
    }
}

#[test]
fn exact_erased_context_becomes_the_issuer_move_and_replays() {
    use fe2o3_mir_model::semantic_direct_call_expansion_v1::*;
    let f = fixture();
    f.admit(f.root.clone());
    let restored = f.restore().unwrap().unwrap();
    let SemanticTerminatorKindV1::Call(call) = restored.blocks()[1].terminator().kind() else {
        panic!()
    };
    assert_eq!(
        call.arguments(),
        &[
            SemanticOperandV1::Move(place(1, 1)),
            SemanticOperandV1::Copy(place(2, 5))
        ]
    );
    assert_eq!(restored.blocks()[0], f.root.blocks()[0]);
    assert_eq!(restored.blocks()[2], f.root.blocks()[2]);
    assert_eq!(restored.locals(), f.root.locals());
    assert_eq!(restored.export(), f.root.export());
    let admitted = f.admit(restored);
    let expansion =
        SemanticCallExpansionV1::try_new(&admitted, SemanticCallExpansionLimitsV1::default())
            .unwrap();
    expansion.verify_replay(&admitted).unwrap();
    let expanded = &expansion.roots()[0];
    assert!(expanded.body().blocks().iter().flat_map(|block| block.statements()).any(|statement| matches!(
        statement.kind(), SemanticStatementKindV1::Assign(assignment)
            if matches!(assignment.value().kind(), SemanticRvalueKindV1::Use(SemanticOperandV1::Move(source)) if source.ty() == ty(1))
    )));
    assert!(expanded.body().blocks().iter().flat_map(|block| block.statements()).any(|statement| matches!(
        statement.kind(), SemanticStatementKindV1::Assign(assignment)
            if matches!(assignment.value().kind(), SemanticRvalueKindV1::Borrow { kind: SemanticBorrowKindV1::Shared, place } if place.ty() == ty(1))
    )));
}

#[test]
fn retained_source_move_is_not_rewritten() {
    let mut f = fixture();
    f.root = f.restore().unwrap().unwrap();
    assert_eq!(f.restore().unwrap(), None);
}

#[test]
fn missing_issuer_cannot_be_replaced_by_a_zst_type() {
    let mut f = fixture();
    f.edit_block(0, |_, terminator| {
        *terminator = SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::Goto,
            SemanticBlockIdV1::from_index(1),
        ))
    });
    assert_eq!(
        f.restore(),
        Err("erased context transfer has no retained issuer")
    );
}

#[test]
fn wrong_helper_and_issuance_source_are_rejected() {
    let mut f = fixture();
    f.boundary.helper = SemanticFunctionIdentityV1::from_sha256([99; 32]);
    assert_eq!(
        f.restore(),
        Err("context transfer does not match the authenticated root/helper ABI boundary")
    );
    f.boundary.helper = f.helper.identity();
    f.boundary.issuance_terminal = SemanticFunctionIdentityV1::from_sha256([99; 32]);
    assert_eq!(
        f.restore(),
        Err("erased context transfer does not follow its exact retained issuer")
    );
    f.boundary.issuance_terminal = SemanticFunctionIdentityV1::from_sha256([180; 32]);
    f.boundary.issuance_block = SemanticBlockIdentityV1::from_sha256([99; 32]);
    assert_eq!(
        f.restore(),
        Err("erased context transfer does not follow its exact retained issuer")
    );
}

#[test]
fn killed_restarted_or_overwritten_issuer_is_rejected() {
    for kind in [
        SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(1)),
        SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(1)),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(1, 1),
            SemanticRvalueV1::new(
                ty(1),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                    ty(1),
                    SemanticConstantValueV1::ZeroSized,
                ))),
            ),
        )),
    ] {
        let mut f = fixture();
        f.edit_block(1, |statements, _| {
            statements.push(SemanticStatementV1::new(location(), kind))
        });
        assert_eq!(
            f.restore(),
            Err("erased context result is touched before its logical move")
        );
    }
}

#[test]
fn bypass_and_backedge_cannot_reissue_the_transfer() {
    for target in [0, 1] {
        let mut f = fixture();
        f.edit_block(2, |_, terminator| {
            *terminator = SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::Goto,
                SemanticBlockIdV1::from_index(target),
            ))
        });
        assert_eq!(
            f.restore(),
            Err(if target == 0 {
                "erased context issuer can be entered again"
            } else {
                "erased context transfer has a bypass or repeated entry"
            })
        );
    }
}

#[test]
fn substituted_context_and_nonforwarded_physical_operand_are_rejected() {
    let mut f = fixture();
    f.edit_block(1, |_, terminator| {
        *terminator = call(
            1,
            vec![
                SemanticOperandV1::Constant(SemanticConstantV1::new(
                    ty(3),
                    SemanticConstantValueV1::ZeroSized,
                )),
                SemanticOperandV1::Copy(place(2, 5)),
            ],
            0,
            0,
            2,
        )
    });
    assert_eq!(
        f.restore(),
        Err("erased context transfer changed the exact owned context type or ABI")
    );
    f.edit_block(1, |_, terminator| {
        *terminator = call(
            1,
            vec![
                SemanticOperandV1::Constant(SemanticConstantV1::new(
                    ty(1),
                    SemanticConstantValueV1::ZeroSized,
                )),
                SemanticOperandV1::Copy(place(1, 1)),
            ],
            0,
            0,
            2,
        )
    });
    assert_eq!(
        f.restore(),
        Err("context wrapper does not forward its physical arguments exactly")
    );
}
