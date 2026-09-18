//! Private transcript fixtures only; these ordinary carriers are not issuance authority.

use super::*;
use rustc_middle::mir::{BasicBlock, Location};
use rustc_middle::ty::TyCtxt;

impl<'tcx> BoundContextEntryV29<'tcx> {
    pub(crate) fn commitment_test_fixture_v29(
        tcx: TyCtxt<'tcx>,
        root: Instance<'tcx>,
        function: SemanticFunctionIdV1,
    ) -> Self {
        let occurrence = |base: u32, unwind| flow::CallOccurrenceV1 {
            location: Location {
                block: BasicBlock::from_u32(base + 1),
                statement_index: (base + 2) as usize,
            },
            destination: Local::from_u32(base + 3),
            target: BasicBlock::from_u32(base + 4),
            unwind,
        };
        let raw = tcx.instance_mir(root.def);
        Self {
            root,
            function,
            helper: root,
            helper_function: SemanticFunctionIdV1::from_index(7),
            issuer: root,
            context: raw.local_decls[Local::from_u32(0)].ty,
            optimized_body: raw,
            original_mir_sha256: [9; 32],
            original: flow::AuthenticatedFlowV1 {
                issuer: root,
                issuance: occurrence(10, UnwindAction::Continue),
                helper_call: occurrence(20, UnwindAction::Unreachable),
            },
            issuance: BoundCallV29 {
                raw: occurrence(30, UnwindAction::Continue),
                block: SemanticBlockIdV1::from_index(51),
                destination: SemanticLocalIdV1::from_index(52),
                target: SemanticBlockIdV1::from_index(53),
                consumed: false,
            },
            helper_call: BoundCallV29 {
                raw: occurrence(40, UnwindAction::Unreachable),
                block: SemanticBlockIdV1::from_index(61),
                destination: SemanticLocalIdV1::from_index(62),
                target: SemanticBlockIdV1::from_index(63),
                consumed: false,
            },
            helper_argument: Some(Local::from_u32(33)),
            semantic_helper_argument: SemanticLocalIdV1::from_index(52),
            arguments: 3,
        }
    }

    pub(crate) fn mutate_commitment_test_field_v29(&mut self, field: usize) {
        match field {
            1 => self.original_mir_sha256[0] ^= 1,
            2 => self.function = SemanticFunctionIdV1::from_index(self.function.index() + 1),
            3 => self.helper_function = SemanticFunctionIdV1::from_index(8),
            4..24 => {
                let site = match (field - 4) / 5 {
                    0 => &mut self.original.issuance,
                    1 => &mut self.original.helper_call,
                    2 => &mut self.issuance.raw,
                    3 => &mut self.helper_call.raw,
                    _ => unreachable!(),
                };
                match (field - 4) % 5 {
                    0 => {
                        site.location.block =
                            BasicBlock::from_usize(site.location.block.index() + 1)
                    }
                    1 => site.location.statement_index += 1,
                    2 => site.destination = Local::from_usize(site.destination.index() + 1),
                    3 => site.target = BasicBlock::from_usize(site.target.index() + 1),
                    4 => {
                        site.unwind = if site.unwind == UnwindAction::Unreachable {
                            UnwindAction::Continue
                        } else {
                            UnwindAction::Unreachable
                        };
                    }
                    _ => unreachable!(),
                }
            }
            24..30 => {
                let site = if field < 27 {
                    &mut self.issuance
                } else {
                    &mut self.helper_call
                };
                match (field - 24) % 3 {
                    0 => site.block = SemanticBlockIdV1::from_index(site.block.index() + 1),
                    1 => {
                        site.destination =
                            SemanticLocalIdV1::from_index(site.destination.index() + 1)
                    }
                    2 => site.target = SemanticBlockIdV1::from_index(site.target.index() + 1),
                    _ => unreachable!(),
                }
            }
            30 => self.helper_argument = None,
            31 => self.helper_argument = Some(Local::from_u32(34)),
            32 => self.arguments += 1,
            _ => panic!("not a mutable context commitment field: {field}"),
        }
    }
}
