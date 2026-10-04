//! Bounded actual operand graphs retained by the closed program recognizer.

use super::ConditionalFillProgramErrorV1 as Error;

const MAX_NODES: usize = 512;
const MAX_STEPS: usize = 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Id(pub(super) u32);

impl Id {
    pub(crate) const fn index(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Ty {
    Output,
    MutableOutput,
    Witness,
    SharedWitness,
    Index,
    U64,
    U32,
    Bool,
    Pointer,
    Unit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Expr {
    Output,
    ThreadIndex,
    GlobalX,
    Use(Id),
    SharedBorrow(Id),
    MutableBorrow(Id),
    IndexGet(Id),
    IntegerTruncate(Id),
    Bitcast(Id),
    Truncate(Id),
    Length(Id),
    Less(Id, Id),
    Zero,
    Select(Id, Id, Id),
    Base(Id),
    Offset(Id, Id),
    WriteAccepted,
    Unit,
}

/// Function, block, source statement/terminator or KIR operation, result local/SSA,
/// and source type index (u32::MAX for KIR). A parameter uses a sentinel block.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Origin(pub(crate) [u32; 5]);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Node {
    pub(crate) origin: Origin,
    pub(crate) ty: Ty,
    pub(crate) expression: Expr,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Effect {
    MirWrite {
        output: Id,
        index: Id,
        value: Id,
    },
    KirStore {
        pointer: Id,
        predicate: Id,
        value: Id,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct Recipe {
    pub(crate) nodes: Vec<Node>,
    pub(crate) effect: Option<(Origin, Effect)>,
    // Complete checked semantic step -> KIR span, including administrative steps.
    pub(crate) spans: Vec<(Origin, [u32; 3])>,
}

impl Recipe {
    pub(super) fn push(&mut self, origin: Origin, expression: Expr) -> Result<Id, Error> {
        use Expr as E;
        if self.nodes.len() == MAX_NODES {
            return Err(Error::Source("recipe node limit"));
        }
        let ty = |id: Id| self.nodes.get(id.0 as usize).map(|node| node.ty);
        let result = match expression {
            E::Output => Ty::Output,
            E::ThreadIndex => Ty::Witness,
            E::GlobalX | E::Zero => Ty::Index,
            E::Use(input) => ty(input).ok_or(Error::Source("recipe use"))?,
            E::SharedBorrow(input) if ty(input) == Some(Ty::Witness) => Ty::SharedWitness,
            E::MutableBorrow(input) if ty(input) == Some(Ty::Output) => Ty::MutableOutput,
            E::IndexGet(input) if ty(input) == Some(Ty::SharedWitness) => Ty::U64,
            E::IntegerTruncate(input) | E::Truncate(input) if ty(input) == Some(Ty::U64) => Ty::U32,
            E::Bitcast(input) if ty(input) == Some(Ty::Index) => Ty::U64,
            E::Length(input) if ty(input) == Some(Ty::Output) => Ty::Index,
            E::Less(lhs, rhs) if ty(lhs) == Some(Ty::Index) && ty(rhs) == Some(Ty::Index) => {
                Ty::Bool
            }
            E::Select(condition, yes, no)
                if ty(condition) == Some(Ty::Bool)
                    && ty(yes) == Some(Ty::Index)
                    && ty(no) == Some(Ty::Index) =>
            {
                Ty::Index
            }
            E::Base(input) if ty(input) == Some(Ty::Output) => Ty::Pointer,
            E::Offset(base, index)
                if ty(base) == Some(Ty::Pointer) && ty(index) == Some(Ty::Index) =>
            {
                Ty::Pointer
            }
            E::WriteAccepted if matches!(self.effect, Some((_, Effect::MirWrite { .. }))) => {
                Ty::Bool
            }
            E::Unit => Ty::Unit,
            _ => return Err(Error::Source("recipe operand type")),
        };
        let id = Id(self.nodes.len() as u32);
        self.nodes.push(Node {
            origin,
            ty: result,
            expression,
        });
        Ok(id)
    }

    pub(super) fn set_effect(&mut self, origin: Origin, effect: Effect) -> Result<(), Error> {
        let ty = |id: Id| self.nodes.get(id.0 as usize).map(|node| node.ty);
        let valid = match effect {
            Effect::MirWrite {
                output,
                index,
                value,
            } => {
                ty(output) == Some(Ty::MutableOutput)
                    && ty(index) == Some(Ty::Witness)
                    && ty(value) == Some(Ty::U32)
            }
            Effect::KirStore {
                pointer,
                predicate,
                value,
            } => {
                ty(pointer) == Some(Ty::Pointer)
                    && ty(predicate) == Some(Ty::Bool)
                    && ty(value) == Some(Ty::U32)
            }
        };
        if !valid || self.effect.is_some() {
            return Err(Error::Source("recipe effect"));
        }
        self.effect = Some((origin, effect));
        Ok(())
    }

    pub(super) fn span(&mut self, origin: Origin, span: (u32, u32, u32)) -> Result<(), Error> {
        if self.spans.len() == MAX_STEPS {
            return Err(Error::Source("recipe step limit"));
        }
        self.spans.push((origin, [span.0, span.1, span.2]));
        Ok(())
    }

    pub(crate) fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = b"F2FILLR1".to_vec();
        let put = |bytes: &mut Vec<u8>, values: &[u32]| {
            for value in values {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        };
        put(&mut bytes, &[self.nodes.len() as u32]);
        for node in &self.nodes {
            put(&mut bytes, &node.origin.0);
            bytes.push(node.ty as u8);
            use Expr as E;
            let (tag, count, operands) = match node.expression {
                E::Output => (0, 0, [0; 3]),
                E::ThreadIndex => (1, 0, [0; 3]),
                E::GlobalX => (2, 0, [0; 3]),
                E::Use(a) => (3, 1, [a.0, 0, 0]),
                E::SharedBorrow(a) => (4, 1, [a.0, 0, 0]),
                E::MutableBorrow(a) => (5, 1, [a.0, 0, 0]),
                E::IndexGet(a) => (6, 1, [a.0, 0, 0]),
                E::IntegerTruncate(a) => (7, 1, [a.0, 0, 0]),
                E::Bitcast(a) => (8, 1, [a.0, 0, 0]),
                E::Truncate(a) => (9, 1, [a.0, 0, 0]),
                E::Length(a) => (10, 1, [a.0, 0, 0]),
                E::Less(a, b) => (11, 2, [a.0, b.0, 0]),
                E::Zero => (12, 0, [0; 3]),
                E::Select(a, b, c) => (13, 3, [a.0, b.0, c.0]),
                E::Base(a) => (14, 1, [a.0, 0, 0]),
                E::Offset(a, b) => (15, 2, [a.0, b.0, 0]),
                E::WriteAccepted => (16, 0, [0; 3]),
                E::Unit => (17, 0, [0; 3]),
            };
            bytes.push(tag);
            put(&mut bytes, &operands[..count]);
        }
        match self.effect {
            None => bytes.push(0),
            Some((origin, effect)) => {
                let (tag, operands) = match effect {
                    Effect::MirWrite {
                        output,
                        index,
                        value,
                    } => (1, [output.0, index.0, value.0]),
                    Effect::KirStore {
                        pointer,
                        predicate,
                        value,
                    } => (2, [pointer.0, predicate.0, value.0]),
                };
                bytes.push(tag);
                put(&mut bytes, &origin.0);
                put(&mut bytes, &operands);
            }
        }
        put(&mut bytes, &[self.spans.len() as u32]);
        for (origin, span) in &self.spans {
            put(&mut bytes, &origin.0);
            put(&mut bytes, span);
        }
        bytes
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    const ORIGIN: Origin = Origin([0, 0, 0, 0, 0]);

    pub(crate) fn alias_store_value(recipe: &mut Recipe) {
        let (origin, effect) = recipe.effect.take().unwrap();
        let effect = match effect {
            Effect::MirWrite {
                output,
                index,
                value,
            } => Effect::MirWrite {
                output,
                index,
                value: recipe.push(ORIGIN, Expr::Use(value)).unwrap(),
            },
            Effect::KirStore {
                pointer,
                predicate,
                value,
            } => Effect::KirStore {
                pointer,
                predicate,
                value: recipe.push(ORIGIN, Expr::Use(value)).unwrap(),
            },
        };
        recipe.set_effect(origin, effect).unwrap();
    }

    pub(crate) fn replace_value_with_zero(recipe: &mut Recipe) {
        let zero = recipe.push(ORIGIN, Expr::Zero).unwrap();
        let wide = recipe.push(ORIGIN, Expr::Bitcast(zero)).unwrap();
        let zero = recipe.push(ORIGIN, Expr::Truncate(wide)).unwrap();
        let (origin, effect) = recipe.effect.take().unwrap();
        let effect = match effect {
            Effect::MirWrite { output, index, .. } => Effect::MirWrite {
                output,
                index,
                value: zero,
            },
            Effect::KirStore {
                pointer, predicate, ..
            } => Effect::KirStore {
                pointer,
                predicate,
                value: zero,
            },
        };
        recipe.set_effect(origin, effect).unwrap();
    }

    #[test]
    fn recipe_checks_backward_edges_types_effects_and_bounds() {
        let mut recipe = Recipe::default();
        assert!(recipe.push(ORIGIN, Expr::Use(Id(0))).is_err());
        assert!(recipe.push(ORIGIN, Expr::WriteAccepted).is_err());
        let output = recipe.push(ORIGIN, Expr::Output).unwrap();
        assert!(recipe.push(ORIGIN, Expr::SharedBorrow(output)).is_err());
        let output = recipe.push(ORIGIN, Expr::MutableBorrow(output)).unwrap();
        let index = recipe.push(ORIGIN, Expr::ThreadIndex).unwrap();
        let shared = recipe.push(ORIGIN, Expr::SharedBorrow(index)).unwrap();
        let raw = recipe.push(ORIGIN, Expr::IndexGet(shared)).unwrap();
        let value = recipe.push(ORIGIN, Expr::IntegerTruncate(raw)).unwrap();
        assert!(
            recipe
                .set_effect(
                    ORIGIN,
                    Effect::MirWrite {
                        output,
                        index: raw,
                        value
                    }
                )
                .is_err()
        );
        let effect = Effect::MirWrite {
            output,
            index,
            value,
        };
        recipe.set_effect(ORIGIN, effect).unwrap();
        assert!(recipe.set_effect(ORIGIN, effect).is_err());
        recipe.push(ORIGIN, Expr::WriteAccepted).unwrap();
        while recipe.nodes.len() < MAX_NODES {
            recipe.push(ORIGIN, Expr::Unit).unwrap();
        }
        assert!(recipe.push(ORIGIN, Expr::Unit).is_err());
        for _ in 0..MAX_STEPS {
            recipe.span(ORIGIN, (0, 0, 0)).unwrap();
        }
        assert!(recipe.span(ORIGIN, (0, 0, 0)).is_err());
    }
}
