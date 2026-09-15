//! Batch-owned reverse structural-type edges. Every query computes a fresh
//! least fixed point from the exact scope seeds and opaque rejection roots.
use super::super::typed_inventory::{NONE, filled, push};
use super::*;

const OPAQUE: u8 = 2;
const OLD: u8 = 1;

pub(super) struct ReverseTypes {
    heads: Vec<u32>,
    edges: Vec<(u32, u32)>, // Parent type, next edge in this child's list.
    flags: Vec<u8>,
    pending: Vec<u32>,
}

// Only a completed query exposes membership; errors cannot expose partial marks.
pub(super) struct Marked<'a>(&'a [u8]);

impl Marked<'_> {
    pub(super) fn contains(&self, ty: usize) -> bool {
        self.0[ty] & OLD != 0
    }
}

impl ReverseTypes {
    pub(super) fn new(
        declarations: &[SemanticTypeDeclV1],
        charge: &mut dyn FnMut(usize) -> Result<()>,
    ) -> Result<Self> {
        charge(std::mem::size_of::<Self>().div_ceil(std::mem::size_of::<usize>()))?;
        let mut result = Self {
            heads: filled(declarations.len(), NONE, charge)?,
            edges: Vec::new(),
            flags: filled(declarations.len(), 0, charge)?,
            // Mark-before-enqueue guarantees at most T pending entries. No
            // allocation/growth occurs during a seed-dependent query.
            pending: filled(declarations.len(), 0, charge)?,
        };
        for (parent, declaration) in declarations.iter().enumerate() {
            charge(1)?;
            let parent = u32::try_from(parent).map_err(|_| mismatch())?;
            match declaration.shape() {
                SemanticTypeShapeV1::Pointer(pointer) => {
                    result.edge(parent, pointer.pointee(), charge)?
                }
                SemanticTypeShapeV1::Array { element, .. }
                | SemanticTypeShapeV1::Slice { element } => {
                    result.edge(parent, *element, charge)?
                }
                SemanticTypeShapeV1::Tuple(fields)
                | SemanticTypeShapeV1::Aggregate(fields)
                | SemanticTypeShapeV1::Union(fields) => {
                    for &field in fields.fields() {
                        result.edge(parent, field, charge)?;
                    }
                }
                SemanticTypeShapeV1::Enum {
                    discriminant,
                    variants,
                } => {
                    result.edge(parent, *discriminant, charge)?;
                    for variant in variants {
                        charge(1)?;
                        for &field in variant.fields().fields() {
                            result.edge(parent, field, charge)?;
                        }
                    }
                }
                SemanticTypeShapeV1::FunctionPointer {
                    arguments,
                    return_type,
                    ..
                } => {
                    result.edge(parent, *return_type, charge)?;
                    for &argument in arguments.fields() {
                        result.edge(parent, argument, charge)?;
                    }
                }
                SemanticTypeShapeV1::Opaque => result.flags[parent as usize] = OPAQUE,
                SemanticTypeShapeV1::Unit
                | SemanticTypeShapeV1::Never
                | SemanticTypeShapeV1::Scalar(_)
                | SemanticTypeShapeV1::ValidityScalar(_) => {}
            }
        }
        Ok(result)
    }

    fn edge(
        &mut self,
        parent: u32,
        child: SemanticTypeIdV1,
        charge: &mut dyn FnMut(usize) -> Result<()>,
    ) -> Result<()> {
        charge(2)?;
        let head = self
            .heads
            .get_mut(child.index() as usize)
            .ok_or_else(mismatch)?;
        let next = u32::try_from(self.edges.len())
            .ok()
            .filter(|&n| n != NONE)
            .ok_or_else(mismatch)?;
        push(&mut self.edges, (parent, *head), charge)?;
        *head = next;
        Ok(())
    }

    pub(super) fn classify(
        &mut self,
        seeds: &[SemanticTypeIdV1],
        charge: &mut dyn FnMut(usize) -> Result<()>,
    ) -> Result<Marked<'_>> {
        charge(
            1 + seeds.len()
                + std::mem::size_of::<Marked<'_>>().div_ceil(std::mem::size_of::<usize>()),
        )?;
        if seeds.is_empty()
            || seeds
                .iter()
                .any(|ty| ty.index() as usize >= self.flags.len())
        {
            return Err(mismatch());
        }
        let mut len = 0;
        // Reset every prior scope's marks before even enqueuing the new roots.
        // Opaque is immutable source metadata, not a retained query answer.
        charge(self.flags.len())?;
        for flag in &mut self.flags {
            *flag &= OPAQUE;
        }
        for ty in 0..self.flags.len() {
            charge(1)?;
            if self.flags[ty] == OPAQUE {
                self.enqueue(ty as u32, &mut len, charge)?;
            }
        }
        for &ty in seeds {
            self.enqueue(ty.index(), &mut len, charge)?;
        }
        while len != 0 {
            charge(2)?;
            len -= 1;
            let ty = self.pending[len];
            let mut next = self.heads[ty as usize];
            while next != NONE {
                charge(2)?;
                let (parent, following) = self.edges[next as usize];
                self.enqueue(parent, &mut len, charge)?;
                next = following;
            }
        }
        Ok(Marked(&self.flags))
    }

    fn enqueue(
        &mut self,
        ty: u32,
        len: &mut usize,
        charge: &mut dyn FnMut(usize) -> Result<()>,
    ) -> Result<()> {
        charge(3)?;
        let flag = self.flags.get_mut(ty as usize).ok_or_else(mismatch)?;
        if *flag & OLD == 0 {
            let slot = self.pending.get_mut(*len).ok_or_else(mismatch)?;
            *slot = ty;
            *len += 1;
            *flag |= OLD;
        }
        Ok(())
    }
}
