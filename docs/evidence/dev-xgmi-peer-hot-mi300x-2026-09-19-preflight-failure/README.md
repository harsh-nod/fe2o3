# Preserved Local Packaging Failure

The first persistent-hot comparison attempt used signed, doubly published source
`fe04fded75acd21efc87075bf668ecae5f0a6d28`. It stopped at the local `pack` stage:
`git archive` rejected the optional `.cargo` selector, which has no tracked files
in this commit. `git ls-files`, used by the CPU selector, permits that absence.

All seven preceding stages passed. The pack command exited 128; its process group
was reaped. No remote creation was attempted, no payload was uploaded, and no GPU
work was run. The local payload was removed, as recorded by `state.json`.

This archive retains the original tooling, protocol, local receipts, failure,
state, and payload-path record. `PROTOCOL.md` is the pre-execution protocol, not a
claim of native success. The bundled `verify.py` intentionally cannot certify
this incomplete attempt. No native or performance qualification is claimed.

The subsequent attempt filters archive selectors through the complete CPU source
map and still requires exact equality between signed archive contents and that
map. No failed command or transcript is overwritten.
