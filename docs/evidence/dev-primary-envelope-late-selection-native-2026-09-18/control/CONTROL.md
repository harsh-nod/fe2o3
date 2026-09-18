# Corrected Fixture Controller Review

Preparation only. Root input review approved the signed source and frozen
bundle, then created the exact marked remote path:

`/tmp/fe2o3-r126-primary-a0db7362-20260918.mmfeMgC8`

The containing signed commit is
`a0db73625c421e26922a1bb8cab4dec49012417c` and frozen payload SHA-256 is
`9f798dc90a24dc950beca265be80060f7588eff49a681dbd19e8fe8d27208201`.
The local bundle directory is 0700 and all 20 payload file hashes match.

Compared with the last reviewed corrected controller/helper, changes are only
the local bundle path, exact remote path, commit identity, payload identity and
remote prefix. `test_controller.py` is byte-identical. Cleanup/absence argument
wiring, full collection and hash matching before removal, visible-reference
scope, exact-owned deletion and separate absence remain unchanged. No runtime,
native protocol, test roster, workload or timing policy is changed here.

The original historical rejection remains sealed separately. The fixed fixture
is CPU-qualified but not yet native-qualified. The last shared-GPU handoff is
closed; it is neither a reservation nor a fresh admission for this run.

## Prospective Command

Only after final controller review and explicit root execution authorization:

```sh
python3 -B controller.py --root-ready-approved \
  --remote-directory /tmp/fe2o3-r126-primary-a0db7362-20260918.mmfeMgC8 \
  --output /home/harsh/.codex-tmp/fe2o3-r126-primary-envelope-late-selection-native-run-20260918
```

Capture the outer command in a separate fresh local receipt directory:
`/home/harsh/.codex-tmp/fe2o3-r126-primary-envelope-late-selection-native-outer-20260918`.

One campaign only, in positive/error/panic order with fresh per-case admission.
All existing test bounds and strict immediate/fixed delayed observations stay
unchanged. A failed case or endpoint stops later tests; there is no retry.
Preserve all outcomes and collect complete results before exact-owned cleanup.
