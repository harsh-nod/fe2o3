// Trusted std contents-only contracts. Outcomes, allocation/capacity, addresses,
// cost, and termination are not constrained by this supplement.
use std::alloc::Allocator;
use std::collections::{HashMap, TryReserveError};
use std::hash::{BuildHasher, Hash};
use vstd::prelude::*;

verus! {
pub assume_specification<K: Eq + Hash, V, S: BuildHasher, A: Allocator>[HashMap::<K, V, S, A>::try_reserve](map: &mut HashMap<K, V, S, A>, additional: usize)
    -> (out: Result<(), TryReserveError>)
    ensures final(map)@ == old(map)@,
;
pub assume_specification<T, A: Allocator>[Vec::<T, A>::try_reserve_exact](vec: &mut Vec<T, A>, additional: usize)
    -> (out: Result<(), TryReserveError>)
    ensures final(vec)@ == old(vec)@,
;
}
