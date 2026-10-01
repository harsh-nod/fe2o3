# Distributed Operation Codec Verification V1

This boundary covers the actual 431-byte operation description encoder and
untrusted decoder. Their four executable shared bodies implement digest writes,
digest reads, complete encoding, and complete decoding. Native methods and Verus
functions invoke those same bodies. The lower primitive and header/u64 field
proof closures remain unchanged and retain their separately qualified scopes.

The full eleven-input proof specifies the exact wire layout and decoder decision,
including length, domain, schema, reserved bytes, coordinate validation, and
error precedence. Composed theorem functions call the actual encoder and decoder
to establish roundtrip behavior for arbitrary coordinates, including invalid
ones, and canonicality for successful decoding. Identity constructors and byte
getters are source-bound safe expressions, not trusted equality bridges. There
is no new assumption, external body, or strengthened decoder entry precondition.

## Qualification Gates

The measured unfiltered positive result is 99 verified and zero errors. The
source guard binds all 308 native model files, eleven proof inputs, actual
forwarding methods, constants, composed theorem calls, and portable diagnostic
and mutation metadata. Its source tests construct forty exact actual-body
mutations without invoking a compiler or solver.

The complete unsigned capture recorded three full 99/0 positives and forty
full-root mutant outcomes. Those captures have zero accepted negative
classifications and zero qualified kills. The strict classifier authenticates
the complete source closure and exact ordered diagnostic span trees. It permits
only the observed, role-bound digest recommendation and compiler question-mark
auxiliary frame; logical primary spans remain nonempty and source-attributed.
Frontend errors, timeouts, resource-limit errors, or unknown diagnostics cannot
be logical kills. The earlier resource-limited capture remains rejected.

Signed candidate `d0d24274c69043ae1205376489cf41179740a564` now passes its
fresh 47-stage campaign: three full 99/0 positives, including independent
relocation and a closing proof, all forty calibrated actual-body negatives,
and all 47 owned process closures. Independent agent and root audits agree on
the raw results, commands, source/tool continuity and diagnostic classifications.
Historical replay still contributes zero qualified negatives. Every mutant
invocation uses the full root, not selected functions. Equivalent finish
omission and initial-fill changes are not counted as logical negatives.

The [public evidence packet](evidence/dev-distributed-codec-operation-2026-10-01/README.md)
preserves the exact durable source/campaign archive and signed incremental
bundle, plus a fresh clean-environment integration run. Two earlier local
integration attempts are excluded because their inherited environment captured
an authentication header; their originals remain private. All 27 source-workflow commands
pass after metadata-only source-guard rebinding; the inherited primitive,
field, queued-query and journal-observer proof inputs remain unchanged.
Prior CPU executables, toolchains and some historical replay prerequisites
remain external, so this is not a self-contained campaign rerunner.

## Native CPU Reuse

The unchanged native implementation has 1,130 passing full debug model tests
with 19 existing manual ignores, 21 passing focused release tests, and strict
all-target/all-feature Clippy acceptance. Five operation test families cover
nonuniform encoding, all truncations and extensions, every byte-position/value
decode differential, invalid coordinates and error precedence, and digest
capacity/cursor panic behavior. Reuse is explicit: all 317 actual compiler
dependencies retain their bytes and modes, and all 16 original/retained test
ELFs and raw execution records remain bound. No new CPU build is claimed for
proof-only or metadata successors.

This boundary does not verify the complete 488-byte publication-receipt codec,
authenticated sessions, transport, receipt authority, native GPU behavior,
HIP/HSA parity, performance parity, or milestone completion. No operation-codec
runtime performance comparison is claimed.
