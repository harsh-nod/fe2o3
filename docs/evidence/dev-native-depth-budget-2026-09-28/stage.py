"""Capture pinned helpers and current adapters once into an ordinary owned directory."""
import hashlib
from pathlib import Path
from types import ModuleType

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
OLD = REPO / "docs/evidence/dev-native-producer-mi300x-2026-09-24"
FILES = ("stage.py", "protocol.py", "native.py", "profile.py", "scale.py", "verify_build.py",
         "campaign.py", "verify.py", "test_profile.py", "test_protocol.py")
PINS = {
    "native-base.py": (OLD / "native.py", "19cfa66a24ecf30d028673ca4d1110cf36f2b51b1bf47734c4c9365e687709a4"),
    "protocol-base.py": (OLD / "protocol.py", "384cf35d5f3471c4fe0b2228e39963f647383fbbb63e2475d64d643be2d40be1"),
    "campaign-base.py": (OLD / "campaign.py", "d4afb6869125903f2f7e654db943b67d95266700d0d763ee67ddaa93b15a71d3"),
    "verify-base.py": (OLD / "verify.py", "017830908daa2a9383f67f5038606434f890635249f82fa8ddbe602dfa4598b9"),
    "oracle.py": (REPO / "crates/fe2o3-runtime/fixtures/trusted-gfx942-mixed-duration-v1/oracle.py",
                  "67846f1e7332f04f0dcafcefa4da5bfad5a769501dbb62b900053be675830625"),
    **{name: (OLD / "raw/campaign1/payload" / name, digest) for name, digest in {
        "base.py": "df740a80c83c94af02f7e7dfb1203dcf7c1eafbde0010e41219e4aac40b34bb7",
        "recorder.py": "3960b587fb80c45af05f8c906ef7067ccc8b3a28d4ad161e8ff7a10ccd4f354c",
        "observer.py": "5d37b09bf0823854c6a45b914d866c042c1e65486cd96c6301285a0147ae6c51",
        "topology.py": "f16f8415badc3b44b72b840fe1c3191afc8dea4b577548e45b020f9b99768714"}.items()}}


def capture(path, digest=None):
    if path.is_symlink() or not path.is_file():
        raise ValueError("ordinary captured helper")
    raw = path.read_bytes()
    if digest is not None and hashlib.sha256(raw).hexdigest() != digest:
        raise ValueError("pinned helper: " + str(path))
    return raw


def module(path, digest=None, replace=None):
    raw = capture(path, digest)
    if replace:
        old, new = replace
        if raw.count(old) != 1:
            raise ValueError("unique adapter site")
        raw = raw.replace(old, new)
    value = ModuleType("async_" + path.stem)
    value.__file__ = str(path)
    exec(compile(raw, str(path), "exec"), value.__dict__)
    return value


def stage(destination):
    captured = {name: capture(HERE / name) for name in FILES}
    captured.update({name: capture(path, digest) for name, (path, digest) in PINS.items()})
    destination.mkdir()
    for name, raw in captured.items():
        with (destination / name).open("xb") as output:
            output.write(raw)
    return {name: hashlib.sha256(raw).hexdigest() for name, raw in captured.items()}
