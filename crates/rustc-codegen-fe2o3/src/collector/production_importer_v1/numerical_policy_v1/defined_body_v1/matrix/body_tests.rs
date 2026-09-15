use super::*;
use rustc_middle::mir::{Operand, Rvalue, START_BLOCK, StatementKind, TerminatorKind};

pub(super) fn check<'tcx>(
    tcx: TyCtxt<'tcx>,
    bind: Instance<'tcx>,
    bind_types: [Ty<'tcx>; 5],
    narrow: Instance<'tcx>,
    narrow_types: [Ty<'tcx>; 7],
    projection: Instance<'tcx>,
) {
    let bind_body = tcx.instance_mir(bind.def);
    let narrow_body = tcx.instance_mir(narrow.def);
    let projection_body = tcx.instance_mir(projection.def);
    assert!(body::bind(tcx, bind, bind_body, bind_types));
    assert!(body::narrow(
        tcx,
        narrow,
        narrow_body,
        narrow_types,
        projection
    ));
    assert!(body::projection(
        tcx,
        projection,
        projection_body,
        narrow_types
    ));
    for mutation in 0..4 {
        let mut changed = bind_body.clone();
        let StatementKind::Assign(assignment) =
            &mut changed.basic_blocks.as_mut()[START_BLOCK].statements[0].kind
        else {
            panic!()
        };
        let Rvalue::Aggregate(_, operands) = &mut assignment.1 else {
            panic!()
        };
        match mutation {
            0 => operands.raw.swap(0, 1),
            1 | 2 => {
                let index = mutation - 1;
                let Operand::Copy(place) = operands.raw[index] else {
                    panic!()
                };
                operands.raw[index] = Operand::Move(place);
            }
            3 => {
                operands.raw.remove(1);
            }
            _ => unreachable!(),
        }
        assert!(
            !body::bind(tcx, bind, &changed, bind_types),
            "Bind mutation {mutation}"
        );
    }
    for mutation in 0..4 {
        let mut changed = narrow_body.clone();
        let entry = &mut changed.basic_blocks.as_mut()[START_BLOCK];
        if mutation == 0 {
            entry.terminator_mut().kind = TerminatorKind::Return;
        } else {
            let TerminatorKind::Call {
                args,
                target,
                destination,
                ..
            } = &mut entry.terminator_mut().kind
            else {
                panic!()
            };
            match mutation {
                1 => {
                    let Operand::Copy(place) = args[0].node else {
                        panic!()
                    };
                    args[0].node = Operand::Move(place);
                }
                2 => *target = Some(START_BLOCK),
                3 => destination.local = rustc_middle::mir::RETURN_PLACE,
                _ => unreachable!(),
            }
        }
        assert!(
            !body::narrow(tcx, narrow, &changed, narrow_types, projection),
            "narrow mutation {mutation}"
        );
    }
    assert!(!body::narrow(tcx, narrow, narrow_body, narrow_types, bind));
    let mut changed = narrow_body.clone();
    let exit = rustc_middle::mir::BasicBlock::from_usize(1);
    let StatementKind::Assign(assignment) =
        &mut changed.basic_blocks.as_mut()[exit].statements[0].kind
    else {
        panic!()
    };
    let Rvalue::Aggregate(_, operands) = &mut assignment.1 else {
        panic!()
    };
    let Operand::Copy(place) = operands.raw[0] else {
        panic!()
    };
    operands.raw[0] = Operand::Move(place);
    assert!(!body::narrow(
        tcx,
        narrow,
        &changed,
        narrow_types,
        projection
    ));
    let mut changed = projection_body.clone();
    let StatementKind::Assign(assignment) =
        &mut changed.basic_blocks.as_mut()[START_BLOCK].statements[0].kind
    else {
        panic!()
    };
    let Rvalue::Use(Operand::Copy(place)) = &mut assignment.1 else {
        panic!()
    };
    let mut projections = place.projection.to_vec();
    let rustc_middle::mir::ProjectionElem::Field(_, ty) = projections[1] else {
        panic!()
    };
    projections[1] =
        rustc_middle::mir::ProjectionElem::Field(rustc_abi::FieldIdx::from_usize(1), ty);
    place.projection = tcx.mk_place_elems(&projections);
    assert!(!body::projection(tcx, projection, &changed, narrow_types));
    assert!(!body::projection(
        tcx,
        narrow,
        projection_body,
        narrow_types
    ));
    let mut types = narrow_types;
    types.swap(0, 2);
    assert!(!body::narrow(tcx, narrow, narrow_body, types, projection));
    assert!(!body::projection(tcx, projection, projection_body, types));
}
