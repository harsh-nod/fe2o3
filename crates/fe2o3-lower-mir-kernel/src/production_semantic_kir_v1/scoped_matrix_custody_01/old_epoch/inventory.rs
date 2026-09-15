use super::super::typed_inventory::{NONE, filled, push};
use super::reverse_types::ReverseTypes;
use super::*;

#[derive(Clone, Copy)]
struct Row {
    block: u32,
    statement: u32, // NONE means the terminator, after all statements.
    next: u32,
}

pub(in super::super) struct Index<'a> {
    declarations: &'a [SemanticTypeDeclV1],
    body: &'a SemanticFunctionDeclV1,
    heads: Vec<u32>,
    rows: Vec<Row>,
    candidates: Vec<Option<u32>>,
    reverse_types: ReverseTypes,
}

impl<'a> Index<'a> {
    pub(in super::super) fn new(
        declarations: &'a [SemanticTypeDeclV1],
        body: &'a SemanticFunctionDeclV1,
        charge: &mut dyn FnMut(usize) -> Result<()>,
    ) -> Result<Self> {
        charge(
            (std::mem::size_of::<Self>() + std::mem::size_of::<Census<'_, '_>>())
                .div_ceil(std::mem::size_of::<usize>()),
        )?;
        let mut result = Self {
            declarations,
            body,
            heads: filled(declarations.len(), NONE, charge)?,
            rows: Vec::new(),
            candidates: filled(body.blocks().len(), None, charge)?,
            reverse_types: ReverseTypes::new(declarations, charge)?,
        };
        for (block, value) in body.blocks().iter().enumerate() {
            charge(1)?;
            let mut census = Census {
                index: &mut result,
                block: u32::try_from(block).map_err(|_| mismatch())?,
                statement: NONE,
            };
            for (statement, value) in value.statements().iter().enumerate() {
                charge(1)?;
                census.statement = u32::try_from(statement)
                    .ok()
                    .filter(|&n| n != NONE)
                    .ok_or_else(mismatch)?;
                census.statement(body, value.kind(), charge)?;
            }
            census.statement = NONE;
            census.terminator(body, value.terminator().kind(), charge)?;
        }
        Ok(result)
    }

    pub(in super::super) fn first_use(
        &mut self,
        declarations: &[SemanticTypeDeclV1],
        graph: &mut Graph<'_>,
        after: u32,
        seeds: &[SemanticTypeIdV1],
    ) -> Result<Option<(u32, Option<u32>)>> {
        graph.charge(14 + seeds.len())?;
        if !std::ptr::eq(graph.body, self.body)
            || !std::ptr::eq(declarations, self.declarations)
            || self.body.blocks().get(after as usize).is_none()
            || seeds.is_empty()
            || seeds
                .iter()
                .any(|ty| declarations.get(ty.index() as usize).is_none())
        {
            return Err(mismatch());
        }
        graph.charge(self.candidates.len())?;
        self.candidates.fill(None);
        let marked = self
            .reverse_types
            .classify(seeds, &mut |n| graph.charge(n))?;
        for (ty, &head) in self.heads.iter().enumerate() {
            graph.charge(1)?;
            if head == NONE {
                continue;
            }
            graph.charge(1)?;
            if !marked.contains(ty) {
                continue;
            }
            let mut next = head;
            while next != NONE {
                graph.charge(3)?;
                let row = &self.rows[next as usize];
                let best = &mut self.candidates[row.block as usize];
                *best = Some(best.map_or(row.statement, |old| old.min(row.statement)));
                next = row.next;
            }
        }
        // Same numeric source order and exact all-edge reachability as the old
        // scan, including loops into earlier blocks and dead rejection sites.
        for (block, &statement) in self.candidates.iter().enumerate() {
            graph.charge(1)?;
            if let Some(statement) = statement {
                if graph.reaches(after, block as u32)? {
                    return Ok(Some((
                        block as u32,
                        (statement != NONE).then_some(statement),
                    )));
                }
            }
        }
        Ok(None)
    }
}

struct Census<'a, 'b> {
    index: &'b mut Index<'a>,
    block: u32,
    statement: u32,
}

impl TypeUses for Census<'_, '_> {
    fn contains(
        &mut self,
        ty: SemanticTypeIdV1,
        charge: &mut dyn FnMut(usize) -> Result<()>,
    ) -> Result<bool> {
        charge(3)?;
        let head = self
            .index
            .heads
            .get_mut(ty.index() as usize)
            .ok_or_else(mismatch)?;
        if *head == NONE || self.index.rows[*head as usize].block != self.block {
            let row = u32::try_from(self.index.rows.len())
                .ok()
                .filter(|&n| n != NONE)
                .ok_or_else(mismatch)?;
            push(
                &mut self.index.rows,
                Row {
                    block: self.block,
                    statement: self.statement,
                    next: *head,
                },
                charge,
            )?;
            *head = row;
        }
        // Recording a dependency is not a type-absence proof. False forces the
        // exhaustive visitor through every disjunct so no sibling is omitted.
        Ok(false)
    }
}
