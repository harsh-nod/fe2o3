# Retained-Pair Owned Storage

This development slice shares the private production storage declarations and
transition bodies with `retained_pair_owned_storage_v1.rs`. It does not establish
RuntimeContext integration, native completion authority, or HIP/HSA parity.

## Production Shape

The public API still owns one concrete directional queue and two concrete
sessions. Only the private `Parts` and `Failure` types are generalized so the
same bodies can be checked with arbitrary non-Copy owner and error values.
There is no public provider, raw-owner accessor, or self-reference.

`Owned<C>` stores `Option<C>`. Successful extraction uses the actual
`Option::take`, leaving the holder empty before its Rust destructor runs.
Admission and finish still call `run_operation` exactly once before the shared
post-operation body. Failed finish retains its error and quarantines custody;
it cannot be recovered as a nonterminal entry refusal.

Recovery samples `terminal()` at that invocation. A prior post-callback result
does not freeze account interiors or authorize later recovery. A failed finish
short-circuits without sampling terminal state during recovery.

## Theorem Boundary

The root starts with the actual post-operation context and returned result.
Its storage values and errors are opaque, non-Copy generic values, not numeric
identity surrogates. Under the occupied-holder precondition, it proves:

- Construction retains the given context in `Some`.
- Accessors return the retained context; mutable access reconnects the final
  referent to the final holder.
- Taking returns the exact context and leaves `None`.
- Admission retains the exact post-operation owner and original error.
- Successful finish extracts the post-operation context.
- Failed finish retains the exact error and occupied post-quarantine storage,
  marked as a finish failure rather than an entry refusal.
- Recovery refusal returns its complete input unchanged. Recovery success
  returns the exact stored error and context and implies an entry refusal.
- Converting `Parts` returns its three values in their original order.

The returning quarantine relation is deliberately uninterpreted. In particular,
the failed-finish theorem does not prove native owner identity across quarantine,
idempotence, release, or eventual return. The terminal call is unconstrained:
the recovery theorem does not prove an observation trace or characterize every
successful recovery by a stable terminal-state predicate.

The proof uses pinned vstd Option specifications. An external adapter with
`requires false` represents the unreachable empty-accessor branch. Production
still aborts there; no behavior of `std::process::abort` is assumed reachable.
This is not a `--no-cheating` root.

## Separate Gates

CPU regressions check tuple addresses, a terminal observation that changes after
admission, failed-finish recovery without another observation, and an opaque
move-only error. Existing owner tests cover admission/finish and occupied Drop.
CPU results and formal source-body results must be reported separately.

Automatic Rust Drop, panic/catch/resume behavior, abort ordering, arbitrary field
destructors, compiler lowering, Arc account contents, native mapping validity,
operational fences, and the RuntimeContext facade remain outside this theorem.
The explicit environment assumption of the experimental retained-pair profile
is unchanged. No new native or performance result is implied by this slice.
