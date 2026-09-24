#!/usr/bin/env python3
"""CPU tests of actual comparator callbacks; requires ROCm headers, not a GPU."""

from pathlib import Path
import resource
import shutil
import signal
import subprocess
import tempfile
import unittest


DIRECTORY = Path(__file__).resolve().parent


class HotCallbackTests(unittest.TestCase):
    def check_callbacks(self, backend, sanitize=False, mutation=None):
        with tempfile.TemporaryDirectory(prefix="fe2o3-hot-callbacks-") as folder:
            root = Path(folder)
            names = (
                "native_benchmark_args.hpp", "xgmi_peer_benchmark_common.hpp",
                "xgmi_peer_segments_common.hpp", f"xgmi_peer_{backend}.cpp",
                "xgmi_peer_hot_callbacks_test.cpp",
            )
            for name in names:
                shutil.copyfile(DIRECTORY / name, root / name)
            if mutation == "slot-zero":
                path = root / f"xgmi_peer_{backend}.cpp"
                source = path.read_text()
                start = source.index("static uint64_t copy_persistent_")
                end = source.index("static bool validate_persistent_", start)
                body = source[start:end]
                self.assertEqual(body.count("buffers.source[slot]"), 1)
                path.write_text(source[:start] + body.replace("buffers.source[slot]", "buffers.source[0]") + source[end:])
            elif mutation == "short-circuit":
                path = root / "xgmi_peer_benchmark_common.hpp"
                source = path.read_text()
                old = "const bool destination_valid = visit(slot, false);"
                self.assertEqual(source.count(old), 1)
                path.write_text(source.replace(old, "const bool destination_valid = source_valid && visit(slot, false);"))
            executable = root / "callbacks"
            flags = ["-DTEST_HIP", "-D__HIP_PLATFORM_AMD__"] if backend == "hip" else []
            if sanitize:
                flags += ["-fsanitize=undefined", "-fno-sanitize-recover=all"]
            subprocess.run([
                "/usr/bin/g++", "-std=c++17", "-O2", "-Wall", "-Wextra", "-Werror",
                "-pedantic", "-ffunction-sections", "-fdata-sections", "-Wl,--gc-sections",
                "-isystem", "/opt/rocm/include", *flags,
                str(root / "xgmi_peer_hot_callbacks_test.cpp"), "-o", str(executable),
            ], check=True, capture_output=True, timeout=120)
            dynamic = subprocess.check_output(["/usr/bin/readelf", "-d", str(executable)], timeout=10)
            self.assertNotIn(b"libamdhip", dynamic)
            self.assertNotIn(b"libhsa", dynamic)
            result = subprocess.run([str(executable)], capture_output=True, text=True,
                                    timeout=10, preexec_fn=lambda: resource.setrlimit(resource.RLIMIT_CORE, (0, 0)))
            if mutation:
                self.assertEqual(result.returncode, -signal.SIGABRT)
                self.assertIn("persistent-hot callback assertion failed\n", result.stderr)
                self.assertNotIn("actual hot callbacks: pass", result.stdout)
            else:
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stdout, "actual hot callbacks: pass (mock APIs, no GPU runtime linked)\n")
                expected = "".join(
                    f"{backend.upper()} persistent-hot {buffer} mismatch at direction {direction} slot {slot}\n"
                    for depth in (1, 16, 32) for direction in (0, 1)
                    for slot in (0, depth // 2, depth - 1)
                    for buffer in ("source", "destination")
                )
                self.assertEqual(result.stderr, expected)

    def test_hip_callbacks(self):
        self.check_callbacks("hip")

    def test_hsa_callbacks(self):
        self.check_callbacks("hsa")

    def test_hip_ubsan(self):
        self.check_callbacks("hip", sanitize=True)

    def test_hsa_ubsan(self):
        self.check_callbacks("hsa", sanitize=True)

    def test_hip_slot_zero_mutation(self):
        self.check_callbacks("hip", mutation="slot-zero")

    def test_hsa_slot_zero_mutation(self):
        self.check_callbacks("hsa", mutation="slot-zero")

    def test_short_circuit_mutation(self):
        self.check_callbacks("hsa", mutation="short-circuit")


if __name__ == "__main__":
    unittest.main()
