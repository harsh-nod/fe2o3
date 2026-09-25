# Shared Planner Safety Development

This packet covers the complete production reconciliation body executed inside
a finite-graph Verus projection. It is not full Context/journal refinement,
final proof qualification, native qualification or a performance result.
The source candidate is signed commit
`7935918fcb08b931a70a41015a039f0b7c1e08c2`.

The proof covers exact selected-dependency paths and identities, bounded stack
and fuel, validation-gated observations and settlement, success/quiescence
classification, monotonic status/cursor progress and returned-error values.
The [claim document](../../runtime-completion-reconciliation-v1.md) gives the
precise boundary. The table model uses executable Vec lookup, not production
HashMap allocation. Its 1,048,576-node bound is separate from the 256-entry
dependency/stack bound and 513-iteration per-pass fuel.

## Checks

All fourteen phases of the fresh signed-source run pass:

| Check | Result |
| --- | --- |
| Baseline and source signatures | Both pass with the expected signer |
| Source-checker calibration | Eight groups pass |
| Allowlisted syntax comparison | Normalized bodies match |
| Opening and closing Verus tool closure | 190 files / 129,019,839 bytes match |
| Opening and closing whole-root proofs | 49 obligations each; no errors or diagnostics |
| GNU runtime, all features | 1,416 pass, 22 hardware-only ignores |
| musl runtime, all features | 1,416 pass, 22 hardware-only ignores |
| Runtime doctests | 46 pass |
| Default-feature check | Pass |
| All-feature/all-target strict Clippy | Pass |
| Workspace formatting | Pass |

The independently measured source brackets match all 5,963 inputs. Hardware
ignores are not execution passes. This standalone root is not the separate
1,303-obligation journal root or a global proof-registration result.

Source normalization admits only the reviewed macro annotations, hygienic names,
bounded while-loop fuel, immediate dependency dereference, three Result bindings
and explicit Observe identity fields. Exact normalized Rustfmt output must match
the signed extraction baseline. Absolute Git/Rustfmt executable hashes and
baseline blob hashes are checked; injected Git/PATH/config state is rejected or
excluded. This is allowlisted syntax correspondence, not a semantic-equivalence
proof. These executable pins are not a whole-host dynamic-library closure claim.

## Evidence Gap

Earlier development attempts and interrupted qualification output lived under
`/dev/shm`. On continuation, the runner handle was missing, no matching runner
was active, and both prior scratch/target paths were absent. Their removal cause
is unknown. Those raw logs were not retained, and no successful qualification or
cleanup is claimed for those attempts. The fresh run uses persistent log storage
and an ephemeral, rebuildable Cargo target. Its retained absence observation is
not a reconstructed proof receipt.

## Retention And Cleanup

[retention.json](retention.json) inventories all 69 fresh-run artifacts copied
byte-for-byte into `retained/`. They include command/process records, raw outputs,
source brackets, source comparison and the explicit earlier-evidence gap.
The fourteen recorded phase groups were confirmed absent before collection.

The fresh build target was not visible in an initial sandboxed filesystem check.
The subsequent outside-sandbox collector found it, retained the persistent logs,
and removed both exact owned scratch and target directories. Independent absence
and retained-hash checks pass in [cleanup-after.json](cleanup-after.json).
The removed paths accounted for 1,940,582,400 allocated bytes. No remote resources
were created. The earlier development directories were separately confirmed
absent outside the sandbox; their removal is not attributed to this collector.
`SHA256SUMS` seals this packet except itself.

## Remaining Gates

Concrete finite-projection outcome witnesses, exact successful-validator
contracts, authenticated logical mutations and relocated replay remain open.
The synthetic settlement marker is neither a monotonic journal prefix nor proof
of concrete dependency custody after failure. An empty-dependency Unknown-input
fixture would be a projection fixture, not a constructor-origin real Context
trace. Real journal observations can evolve as dependencies settle; their
correspondence with stored projection observations must be established rather
than assumed.

The next native concurrency gate separately needs admitted short/long artifacts
and the actual async owner. Existing launch-ordered two-stream tests do not
establish out-of-order owner behavior or physical overlap. A1/A2 and the accepted
Native R125, Admission R118B C1/C2/C3 and Resources R116/V3 checkpoints remain
unchanged.

## Reproduction

From the committed source with pinned tools/dependencies and an existing owned
Cargo target outside the source tree:

```sh
python3 -I -B docs/evidence/dev-completion-reconciliation-proof-2026-09-25/run.py --output ABSOLUTE_OUTPUT --target ABSOLUTE_TARGET --verus ABSOLUTE_VERUS
```

Output and target must be canonical, mutually disjoint and outside the source
tree. The runner retains bounded command/process records and measures both source
brackets. It does not provide a clean-room build or relocated replay audit.
