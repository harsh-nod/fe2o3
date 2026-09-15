# C2 Compiled-Negative Handoff

Read-only review, 2026-09-14. Formatted candidate map
`09292cf22fb94872326a7c054e7e5f98fcca35e961b51d410f150e9157b14bc7`.
These nineteen predictions have not been executed or qualified.

G: `crates/fe2o3-runtime/src/generated_source.rs`.
B: `crates/fe2o3-runtime/src/authorized_execution.rs`.
Test source: `authorized_execution/tests/generated_identity.rs`.
Test prefix: `authorized_execution::tests::generated_identity::`.

| Alias | Test Suffix |
| --- | --- |
| D | generated_descriptor_matches_bind_every_coordinate_bidirectionally |
| T | generated_identity_survives_transfer_and_rejects_identical_replacement |
| A | generated_transferred_source_rejects_later_artifact_substitution |
| U | generated_transferred_source_rechecks_authority_and_currentness |

Mutations 1-10 target the unique expression in GeneratedHostRosterV1::matches.

| ID | Exact Anchor / Change | Predicted Behavioral Oracle |
| --- | --- | --- |
| 1 | G: `std::sync::Arc::ptr_eq(&self.source_identity, &other.source_identity)` compares self with self instead | T:317, transferred replacement matched original |
| 2 | G: `self.count == other.count` to `self.count >= other.count` | D:286, reverse descriptor match: Count |
| 3 | G: remove `&& self.readback_bytes == other.readback_bytes` | D:282, forward descriptor match: Readback |
| 4 | G: remove `&& self.fixup_count == other.fixup_count` | D:282, forward descriptor match: Fixups |
| 5 | G: remove `&& self.dispatch_contract_sha256 == other.dispatch_contract_sha256` | D:282, forward descriptor match: Dispatch |
| 6 | G: replace buffer equality with slot comparator ignoring ordinal | D:282, forward descriptor match: Ordinal(0) |
| 7 | G: replace buffer equality with slot comparator ignoring bytes | D:282, forward descriptor match: Bytes(0) |
| 8 | G: replace buffer equality with slot comparator ignoring access | D:282, forward descriptor match: Access(0, ReadOnly) |
| 9 | G: replace buffer equality with comparator ignoring missing occupied-prefix slots | D:282, forward descriptor match: Missing(0) |
| 10 | G: compare only the declared buffer prefix | D:282, forward descriptor match: Trailing(3) |
| 11 | G: `.is_ok_and(|actual| actual.matches(expected))` to `.is_ok()` | D:290, source descriptor match: Count |
| 12 | G: prefix the immutable artifact digest inequality with `false &&` | A:354, ArtifactMismatch, length=false |
| 13 | G: artifact length inequality becomes equality AND digest mismatch | A:354, ArtifactMismatch, length=true |
| 14 | B: prefix authority object-digest inequality with `false &&` | U:391, AuthorityMismatch, field=0 |
| 15 | B: prefix authority object-length inequality with `false &&` | U:391, AuthorityMismatch, field=1 |
| 16 | B: prefix authority kernel-name inequality with `false &&` | U:391, AuthorityMismatch, field=2 |
| 17 | B: prefix authority dispatch-digest inequality with `false &&` | U:391, AuthorityMismatch, field=3 |
| 18 | B: prefix authority device-ID inequality with `false &&` | U:391, AuthorityMismatch, field=4 |
| 19 | G: remove unique `self.revalidate()?;` statement | U:396, AuthorityNotCurrent, post-transfer stale authority validation |

For 6-10, the exact original anchor is `self.buffers == other.buffers`.
For 6-8 substitute:

```rust
self.buffers.iter().zip(other.buffers.iter()).all(|(a, b)| match (a, b) {
    (Some(a), Some(b)) => P,
    (None, None) => true,
    _ => false,
})
```

P is `a.bytes == b.bytes && a.access == b.access` for 6;
`a.ordinal == b.ordinal && a.access == b.access` for 7;
`a.ordinal == b.ordinal && a.bytes == b.bytes` for 8.
For 9 and 10 respectively:

```rust
self.buffers.iter().zip(other.buffers.iter()).enumerate().all(|(i, (a, b))| (i < self.count && (a.is_none() || b.is_none())) || a == b)
self.buffers.iter().zip(other.buffers.iter()).take(self.count).all(|(a, b)| a == b)
```

For 12 the exact predicate is
`<[u8; 32]>::from(Sha256::digest(self.hsaco)) != projection.identity().object_sha256()`.
For 13 replace `!= Some(projection.finalized_hsaco_length())` with equality and
the joining `||` with `&&`. This is explicitly a paired weakening: merely
omitting the length check remains rejected by SHA-256 and is not decisive.
Mutations 14-18 need a B path override, targeting the existing
validate_authority_bindings_v1 comparisons and preserving their error bodies.

Removing only matches_roster's outer source identity check is masked by the
underlying matcher. Removing the entire buffer comparison fails at Ordinal(0),
not the other coordinate oracles. Swallowing errors inside revalidate fails
U:396 before direct revalidation at U:400; do not relabel that outcome.

Require a unique anchor, successful compilation, exactly one selected test via
`--exact --test-threads=1`, its designated failure assertion, and complete source
restoration after each mutation. Parser errors, zero selected tests and source
guards are not qualifying behavioral failures. No native authority is supplied.
