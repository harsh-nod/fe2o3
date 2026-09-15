// A conjunct is retained once, in the same ascending-ID query order as the
// original bitmap. No predicate is omitted and no new fact is inferred.
struct Facts {
    ids: [Id; NODES],
    len: usize,
}

impl Facts {
    fn new() -> Self {
        Self {
            ids: [0; NODES],
            len: 0,
        }
    }
    fn push_descending(&mut self, id: Id) -> Result<()> {
        if self.len == NODES || (self.len != 0 && self.ids[self.len - 1] <= id) {
            return Err(Error::Graph);
        }
        self.ids[self.len] = id;
        self.len += 1;
        Ok(())
    }
    fn ascending(&self) -> impl Iterator<Item = Id> + '_ {
        self.ids[..self.len].iter().rev().copied()
    }
}
