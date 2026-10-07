// Trusted standard-library supplement, not verified allocator implementation.
// Match vstd's Vec::try_reserve contents-only contract. Neither outcome is
// assumed, and no capacity, address, cost or termination fact is provided.
use std::alloc::Allocator;
use std::collections::{HashMap, HashSet, TryReserveError};
use std::hash::{BuildHasher, Hash};
use vstd::prelude::*;

verus! {
pub assume_specification<K: Eq + Hash, S: BuildHasher, A: Allocator>[HashSet::<K, S, A>::try_reserve](set: &mut HashSet<K, S, A>, additional: usize)
    -> (out: Result<(), TryReserveError>)
    ensures final(set)@ == old(set)@,
;
pub assume_specification<K: Eq + Hash, V, S: BuildHasher, A: Allocator>[HashMap::<K, V, S, A>::try_reserve](map: &mut HashMap<K, V, S, A>, additional: usize)
    -> (out: Result<(), TryReserveError>)
    ensures final(map)@ == old(map)@,
;
}
