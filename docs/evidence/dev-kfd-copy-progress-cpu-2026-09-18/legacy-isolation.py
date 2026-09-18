#!/usr/bin/env python3
"""Check the exact six inserted lines; this is not a Rust semantic proof."""

import hashlib
from pathlib import Path
import subprocess

root = Path(__file__).resolve().parents[3]
base = "3d2c93a748bfbde7e37093376d121eea67258649"
path = "crates/fe2o3-runtime/examples/gfx942-runtime-directional-window-benchmark.rs"
before = subprocess.check_output(["git", "show", f"{base}:{path}"], cwd=root)
type_anchor = b"type BenchmarkResult<T> = Result<T, Box<dyn Error>>;\n"
main_anchor = b"    let args = std::env::args().skip(1).collect::<Vec<_>>();\n"
module = b'\n#[path = "support/directional_copy_diagnostic.rs"]\nmod diagnostic;\n'
dispatch = (
    b"    if let Some(config) = diagnostic::parse_config_v1(&args)? {\n"
    b"        return diagnostic::run_v1(config, &mut std::io::stdout().lock());\n"
    b"    }\n"
)
assert before.count(type_anchor) == before.count(main_anchor) == 1
expected = before.replace(type_anchor, type_anchor + module).replace(
    main_anchor, main_anchor + dispatch
)


def check(source):
    assert source == expected, "legacy source changed outside exact opt-in insertions"


after = (root / path).read_bytes()
check(after)
for old, new in (
    (b"Duration::from_micros(50)", b"Duration::from_micros(51)"),
    (b"fe2o3.async-copy-benchmark.v1", b"fe2o3.async-copy-benchmark.v2"),
    (b"let elapsed = start.elapsed()", b"let elapsed = Instant::now().elapsed()"),
):
    assert after.count(old) == 1
    try:
        check(after.replace(old, new))
    except AssertionError:
        pass
    else:
        raise AssertionError("legacy mutation escaped")
print(f"base_commit={base}")
print(f"base_sha256={hashlib.sha256(before).hexdigest()}")
print(f"source_sha256={hashlib.sha256(after).hexdigest()}")
print("exact_opt_in_insertions=6 legacy_mutations_rejected=3")
