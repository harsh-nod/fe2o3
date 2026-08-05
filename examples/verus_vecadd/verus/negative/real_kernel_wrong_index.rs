use vstd::prelude::*;
use vstd::std_specs::ops::*;

include!("../../../vecadd/src/vecadd_body.rs");

verus! {

#[derive(Copy, Clone)]
pub struct MutatedThreadIndex {
    pub linear: usize,
}

impl MutatedThreadIndex {
    pub fn get(&self) -> (linear: usize)
        ensures
            linear == self.linear,
    {
        self.linear
    }
}

/// Mutation: every launch witness is mapped to output element zero. The real
/// shared body is expanded through this adapter below.
pub mod mutated_thread {
    use super::MutatedThreadIndex;

    pub fn index_1d(_thread: MutatedThreadIndex) -> (result: MutatedThreadIndex)
        ensures
            result.linear == 0,
    {
        MutatedThreadIndex { linear: 0 }
    }
}

pub struct MutatedDisjointSlice {
    pub values: Vec<f32>,
}

impl MutatedDisjointSlice {
    pub fn get_mut(
        &mut self,
        index: MutatedThreadIndex,
    ) -> (element: Option<&mut f32>)
        ensures
            match element {
                Some(element) => {
                    &&& index.linear < old(self).values@.len()
                    &&& *element == old(self).values@[index.linear as int]
                    &&& final(self).values@ == old(self).values@.update(
                        index.linear as int,
                        *final(element),
                    )
                }
                None => {
                    &&& index.linear >= old(self).values@.len()
                    &&& final(self).values@ == old(self).values@
                }
            },
    {
        if index.linear < self.values.len() {
            Some(&mut self.values[index.linear])
        } else {
            None
        }
    }
}

/// Expected failure: a nonzero launch witness writes element zero, violating
/// the identity-owned frame condition and demonstrating the resulting write
/// collision between distinct launch witnesses.
pub fn mutated_shared_body_claims_identity_frame(
    thread: MutatedThreadIndex,
    a: &[f32],
    b: &[f32],
    mut output: MutatedDisjointSlice,
) -> (result: MutatedDisjointSlice)
    requires
        output.values@.len() > 1,
        thread.linear < output.values@.len(),
        thread.linear != 0,
        a@.len() == output.values@.len(),
        b@.len() == output.values@.len(),
        a@[0].add_req(b@[0]),
    ensures
        result.values@[0] == output.values@[0], // mutated_shared_body_claims_identity_frame
{
    vecadd_kernel_body!(mutated_thread, (thread), a, b, output);
    output
}

} // verus!
