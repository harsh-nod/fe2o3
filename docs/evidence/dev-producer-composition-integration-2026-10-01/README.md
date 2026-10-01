# Producer Composition Integration Controls

Signed integration: `6c0718c1d`, above `92f18e433`.
Qualified source candidate: `6b9e5d87c4386692f7c213a95dd786ccdf42cb45`.

The root integration audit compares all sixteen imported paths against the
qualified candidate. Twelve are byte-identical. Four source guards/tests
change only reviewed native-inventory counts/hashes and their checker pins:
327 runtime inputs, plus five model schemas, replace the candidate's 323 plus
five. All three actual proof closures and the native shared macro are unchanged.
The runtime, runtime-model, accounting and KFD source trees are unchanged from
the integration parent. This does not extend the theorem to newer native
semantics outside its conditional helper contracts.

All 21 commands from the updated source-control workflow pass locally. Raw
stdout/stderr and exact argument vectors are retained in `raw/`; `result.json`
also records each imported source hash and the workflow hash. Three new commands
cover composition source binding, qualification controls and diagnostic parsing.
The ordinary native harness includes two CPU C++ callback controls. No Rust
suite, solver, GPU campaign or hosted CI job ran in this integration check.

`replay.py` is the exact local capture controller, not a portable test runner:
it retains the original checkout, fresh-output and allowed-signers paths.
An earlier identical-source CI replay used the unsigned initial cherry-pick
`3e7725f96`; its local receipts remain preserved. The commit was then signed,
and all 21 commands were rerun on `6c0718c1d`, as recorded here. Signing changed
commit metadata, not the source tree.

The independent 102-stage signed-candidate proof campaign is separate evidence;
this integration check neither reruns it nor claims full A2 or HIP/HSA parity.
