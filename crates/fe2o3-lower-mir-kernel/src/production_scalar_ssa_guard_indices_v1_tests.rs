use super::super::tests::{FLOOR, ROOT, STORAGE, WORK, materialize};
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

#[test]
fn private_operand_index_must_rejoin_actual_event_site_role_and_resolution() {
    let (owner, report) = materialize(false);
    let certificate = report.certificates()[0];
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = FLOOR + owner.retained_analysis_storage_v1();
    budget.reserve_storage(floor).unwrap();
    resources::scoped(&mut budget, |budget| {
        let mut index = Indices::derive(&owner, budget)?;
        let guard = certificate.guard();
        let site = Site::Statement {
            block: SsaBlockIdV1::new(guard.block().block().index()),
            statement: guard.statement(),
        };
        let variable = SsaVariableIdV1::new(certificate.induction().local().index());
        let expected = index.operand(&owner, ROOT, site, 0, variable, budget)?;
        assert!(matches!(expected, SsaValueV1::BlockArgument { .. }));
        let key = (
            ROOT.index(),
            0,
            guard.block().block().index(),
            guard.statement(),
            0,
        );
        let position = index
            .operands
            .binary_search_by_key(&key, |row| row.0)
            .unwrap();
        let original = index.operands[position].1;
        let other = index.operands.iter().find(|row| row.0 != key).unwrap().1;
        index.operands[position].1 = other;
        assert!(matches!(
            index.operand(&owner, ROOT, site, 0, variable, budget),
            Err(Error::Mismatch("guard exact source occurrence key"))
        ));
        index.operands[position].1 = usize::MAX;
        assert!(matches!(
            index.operand(&owner, ROOT, site, 0, variable, budget),
            Err(Error::Mismatch("guard source operand index target"))
        ));
        index.operands[position].1 = original;
        assert!(matches!(
            index.operand(&owner, ROOT, site, 0, SsaVariableIdV1::new(99), budget),
            Err(Error::Mismatch("guard source operand resolution"))
        ));
        assert_eq!(
            index.operand(&owner, ROOT, site, 0, variable, budget)?,
            expected
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn private_block_index_must_rejoin_exact_source_coordinate_and_n_block() {
    let (owner, report) = materialize(false);
    let certificate = report.certificates()[0];
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = FLOOR + owner.retained_analysis_storage_v1();
    budget.reserve_storage(floor).unwrap();
    resources::scoped(&mut budget, |budget| {
        let mut index = Indices::derive(&owner, budget)?;
        let (inventory, receipt) = Inventory::derive(owner.original().executable(), budget)?;
        budget.reserve_storage(receipt.retained_storage())?;
        let function = &owner.emission.capture.functions[owner.emission.aliases[0].emitted];
        let block = certificate.header().block();
        let expected = index
            .block(&owner, &inventory, function, block, budget)?
            .coordinate;
        let key = (ROOT.index(), ROOT.index(), block.index());
        let position = index
            .blocks
            .binary_search_by_key(&key, |row| row.0)
            .unwrap();
        let original = index.blocks[position].1;
        index.blocks[position].1 = index.blocks.iter().find(|row| row.0 != key).unwrap().1;
        assert!(matches!(
            index.block(&owner, &inventory, function, block, budget),
            Err(Error::Mismatch("guard exact source block key"))
        ));
        index.blocks[position].1 = usize::MAX;
        assert!(matches!(
            index.block(&owner, &inventory, function, block, budget),
            Err(Error::Mismatch("guard source block index target"))
        ));
        index.blocks[position].1 = original;
        assert_eq!(
            index
                .block(&owner, &inventory, function, block, budget)?
                .coordinate,
            expected
        );
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn incoming_census_counts_duplicate_and_bypass_edge_occurrences_without_deduplication() {
    let (owner, report) = materialize(false);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let floor = FLOOR + owner.retained_analysis_storage_v1();
    budget.reserve_storage(floor).unwrap();
    resources::scoped(&mut budget, |budget| {
        let indices = Indices::derive(&owner, budget)?;
        let mut incoming = Incoming::prepare(&owner, budget)?;
        let (inventory, receipt) = Inventory::derive(owner.original().executable(), budget)?;
        budget.reserve_storage(receipt.retained_storage())?;
        incoming.populate(&inventory, budget)?;
        let function = &owner.emission.capture.functions[owner.emission.aliases[0].emitted];
        let body = indices.block(
            &owner,
            &inventory,
            function,
            report.certificates()[0].body_entry().block(),
            budget,
        )?;
        let edge = inventory
            .edges()
            .iter()
            .find(|edge| edge.target == body.coordinate)
            .unwrap();
        assert!(incoming.unique(body.coordinate, edge.coordinate, budget)?);
        let row = incoming
            .rows
            .binary_search_by_key(&body.coordinate, |row| row.0)
            .unwrap();
        let original = incoming.rows[row];
        incoming.observe(body.coordinate, edge.coordinate, budget)?;
        assert!(!incoming.unique(body.coordinate, edge.coordinate, budget)?);
        incoming.rows[row] = original;
        let bypass = Edge {
            source: body.coordinate,
            successor: 0,
        };
        incoming.observe(body.coordinate, bypass, budget)?;
        assert!(!incoming.unique(body.coordinate, bypass, budget)?);
        assert!(!incoming.unique(body.coordinate, edge.coordinate, budget)?);
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.storage(), floor);
}
