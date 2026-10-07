// Normal-return storage refinement for the actual private native owner bodies.
// C, E and the three parts are arbitrary non-Copy values, not copied identities.
// Admission/finish start AFTER the actual run_operation result and context exist.
// No callback, catch/resume, automatic Drop, allocation, compiler or ISA theorem.
//
// Custody is a conditional adapter: quarantine's returning effect is uninterpreted.
// It may abort or not return. The terminal observation is fresh and unconstrained;
// no earlier post-catch observation, stable Arc ledger, or native currentness is
// assumed. Recovery proves exact storage/error retention and entry-only extraction,
// not a terminal-observation trace or the native meaning of a false observation.
// The impossible empty accessor path is excluded by the occupied-owner invariant;
// it does not model or prove std::process::abort. Option operations use pinned vstd.

use vstd::prelude::verus as retained_pair_owned_declarations_v1;
use vstd::prelude::*;
include!("../../fe2o3-kfd/src/sdma/retained_pair/owned/declarations.rs");
include!("../../fe2o3-kfd/src/sdma/retained_pair/owned/bodies.rs");

verus! {

uninterp spec fn returning_owned_quarantine_effect<C>(before: C, after: C) -> bool;

trait Custody: Sized {
    fn terminal(&self) -> bool;

    fn quarantine(&mut self)
        ensures returning_owned_quarantine_effect(*old(self), *final(self));
}

#[verifier::external_body]
fn empty_owned_context<T>() -> T
    requires false,
{
    unimplemented!()
}

impl<Q, S, D> Parts<Q, S, D> {
    fn into_parts(self) -> (result: (Q, S, D))
        ensures result == (self.queue, self.source, self.destination),
    {
        retained_pair_owned_parts_body!(self)
    }
}

impl<C: Custody> Owned<C> {
    fn new(context: C) -> (result: Self)
        ensures result.context == Some(context),
    {
        retained_pair_owned_new_body!(context)
    }

    fn context(&self) -> (result: &C)
        requires self.context.is_some(),
        ensures *result == self.context.unwrap(),
    {
        retained_pair_owned_context_body!(self)
    }

    fn context_mut(&mut self) -> (result: &mut C)
        requires old(self).context.is_some(),
        ensures
            *result == old(self).context.unwrap(),
            final(self).context == Some(*final(result)),
    {
        retained_pair_owned_context_mut_body!(self)
    }

    fn take(&mut self) -> (result: C)
        requires old(self).context.is_some(),
        ensures
            result == old(self).context.unwrap(),
            final(self).context.is_none(),
    {
        retained_pair_owned_take_body!(self)
    }

    fn admit_post<E>(self, outcome: Result<(), E>) -> (result: Result<Self, Failure<C, E>>)
        ensures result == match outcome {
            Ok(()) => Ok(self),
            Err(error) => Err(Failure { owner: self, error, entry_refusal: true }),
        },
    {
        retained_pair_owned_admit_post_body!(self, outcome)
    }

    fn finish_post<E>(self, outcome: Result<(), E>) -> (result: Result<C, Failure<C, E>>)
        requires self.context.is_some(),
        ensures match outcome {
            Ok(()) => result == Ok(self.context.unwrap()),
            Err(error) => match result {
                Ok(_) => false,
                Err(failure) => {
                    &&& failure.error == error
                    &&& !failure.entry_refusal
                    &&& failure.owner.context.is_some()
                    &&& returning_owned_quarantine_effect(self.context.unwrap(), failure.owner.context.unwrap())
                },
            },
        },
    {
        let mut owner = self;
        retained_pair_owned_finish_post_body!(owner, outcome)
    }
}

impl<C: Custody, E> Failure<C, E> {
    fn recover_unadmitted(self) -> (result: Result<(E, C), Self>)
        requires self.owner.context.is_some(),
        ensures match result {
            Ok((error, context)) => self.entry_refusal && error == self.error
                && Some(context) == self.owner.context,
            Err(failure) => failure == self,
        },
    {
        let mut failure = self;
        retained_pair_owned_recover_body!(failure)
    }
}

}
