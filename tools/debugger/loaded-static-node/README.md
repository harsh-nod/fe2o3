# Bounded static Node CPU launcher

This package binds a complete request to the CPU reader/adapter and a prebound report descriptor. It starts no child, shell, debugger or kernel. Installation does not authorize a historical operation. The [qualification note](../../../docs/static-node-loaded-launch-qualification-20260929.md) distinguishes fixture-free controls, the actual draft-path CLI run, the separately qualified installed-path run, and repository publication.

Root subsequently passed 263 combined installed fixture-free controls, including the 61 launcher controls, an actual installed-path CLI invocation with exit zero, and one opt-in complete-readback control. Publication is an independent stage; none of these CPU observations activates historical or native work.

## Fixed entry and explicit Buffer APIs

The executable entry is `launch-main.mjs`. Do not import that entry as a library: it intentionally starts its invocation. Other library modules perform no filesystem IO at import time.

`admitLaunchBootstrap(Buffer)` admits an externally bound request whole pin, exact name, six identity fields and owner; finite UTC/elapsed/resource policies; fixed runtime/entry; six output roles; independently pinned terminals; and inherited stdout FD 1. The request cannot grant its own read authority. Bootstrap is 1–65,536 bytes, passed as one canonical base64 CLI argument.

`readPinnedRequest(specBuffer, provider, {guard})` returns a private complete Buffer only after exact-or-refuse chunks of at most 64 KiB, EOF, whole SHA-256, named/descriptor/owner brackets, cleanup and final currentness. No partial Buffer succeeds.

`admitStaticLaunchRequest(requestBuffer, bootstrapBuffer, fixedContext)` admits closed UTF-8 JSON, bounded whole-pinned graph/plan/policy parts, the immutable historical-protocol digest, and an exact static import map. Named entries, independently unselected alias targets and terminals must agree on kind and content. The request name cannot overlap a graph entry or alias target. Absence is not an empty-file pin, and identical hashes do not collapse named roles. Admission uses the existing adapter parser behind an immediately denying pure guard, without input IO.

`executeStaticLaunch(bootstrapBuffer, {requestProvider, bootstrapGuard, reportGuard, context, invokeAdapter, summarySink})` is the injected seam. A trusted sink implements `before(spec)`, `write(buffer, offset, length)` and `after(spec, bytes)`; none is request-supplied code. `runStaticNodeFilesystem(Buffer)` binds only the fixed reviewed provider, adapter and inherited FD 1. The CLI admits no extra arguments, Node flags, import paths or commands. Its environment must be exactly `LANG=C`, `LC_ALL=C`, `PATH=/usr/bin:/bin`; root clears it before Node starts. A post-load check cannot undo loader effects.

## Controls and fresh fixture preparation

Run the 61 fixture-free controls explicitly:

~~~sh
node --test tools/debugger/loaded-static-node/launch-controls.test.mjs \
  tools/debugger/loaded-static-node/launch-fixture-controls.test.mjs
~~~

The 53 launch controls and 8 fixture-preparation controls use in-memory providers and synthetic metadata. They neither launch the CLI nor open a historical roster.

`buildSyntheticLaunchRequest(inputBuffer)` produces complete request, bootstrap-template and expected-result Buffers from explicit root-supplied source/fixture/output/policy metadata. `bindSyntheticBootstrap(requestBytes, templateBytes, bindingBytes)` adds the completely checked request identity and the already-open stdout descriptor. The helper creates no files or process and acquires no authority.

Root's separate harness must create fresh target/relative-alias/empty/absent inputs, establish a private output directory and finite current policies, pin the runtime and all selected sources/terminals, preopen stdout, then invoke the fixed entry. No shell redirection may replace the descriptor after binding. Root owns source pre/post checks, child/loader/environment/FD custody, capture limits and complete readback.

The opt-in control is deliberately not auto-discovered and fails when its explicit configuration is missing:

~~~sh
FE2O3_STATIC_LAUNCH_READBACK=ROOT_BOUND_CANONICAL_BASE64 \
  node --test tools/debugger/loaded-static-node/real-filesystem-launch-controls.mjs
~~~

Its closed configuration contains current finite UTC/elapsed bounds, actual process exit, expected bytes/pin and three complete request-spec-shaped objects for stdout, final evidence and retained temporary. It completely checks those three files, then calls the pure result verifier; it launches no process and writes no files. Root separately verifies all six output roles. The host-specific qualification harness, raw requests/receipts/captures and historical seed are not installed.

## Failure and scope boundaries

Bootstrap, adapter and report budgets are distinct. Adapter phases are precheck → two-pass historical-form read → postcheck. A failure preserves later phases as not-started. The final bootstrap snapshot includes the final pre-dispatch probe/refusal. The report clock starts on first report check but keeps the same absolute UTC expiry; reporting cannot renew scope.

First failure remains primary, including non-Error thrown values; later cleanup/report failures remain separate. Attempted reservations survive denial. Cleanup after expiry permits descriptor close, not fresh content IO. The evidence writer uses exclusive temporary creation and no-replace hard-link publication, never overwrite-capable rename. Partial/retained output custody is not deleted. Path brackets do not prove ancestor writer exclusion.

Report serialization is bounded before effects. Writes are exact-or-refuse, at most 64 KiB, with no retry; short/throwing writes can leave uncertain partial captures. FD 1 is not closed or fsynced by this package. Its serialized summary is intent, not proof of its own later write. Root must bind FD 1 to the outer-stdout path, preserve actual exit/receipt and completely read outputs. A kill, expired scope or failed preflight may leave no durable diagnostic.

Six distinct output roles are reader-observation-temporary, reader-observation-final, outer-receipt, outer-stdout, outer-stderr and root-readback. The package writes only temporary/final evidence and inherited stdout. Static22-module/71-edge import closure is not an OS loader/ELF or global IO attestation. Resource probes, outer supervision and terminal evidence remain separate domains.

The fresh synthetic qualification does not activate the historical operation, refresh stale pins, close its fourteen evidence obligations, renew native coordination, or provide GDB/attach/inferior/GPU/physical visualization. No broad milestone promotion follows.
