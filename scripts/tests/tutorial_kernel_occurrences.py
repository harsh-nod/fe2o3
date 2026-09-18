#!/usr/bin/env python3
"""Physical Rust declaration inventory tests, not compilation evidence."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]


def load_validator():
    path = ROOT / "scripts/validate-tutorial-kernel-manifest.py"
    specification = importlib.util.spec_from_file_location("tutorial_occurrences", path)
    assert specification is not None and specification.loader is not None
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    return module


class FunctionOccurrenceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.validator = load_validator()

    def scan(self, source):
        return self.validator.ordinary_rust_function_items(source)

    def assert_items(self, source, names, attributed):
        records = self.scan(source)
        self.assertEqual([record["kernelSymbol"] for record in records], names)
        self.assertEqual([record["attributedKernel"] for record in records], attributed)
        self.assertEqual(
            [record["functionUtf8Offset"] for record in records],
            sorted(record["functionUtf8Offset"] for record in records),
        )
        encoded = source.encode("utf-8")
        for record in records:
            self.assertEqual(
                set(record), {"kernelSymbol", "functionUtf8Offset", "attributedKernel"}
            )
            self.assertIs(type(record["functionUtf8Offset"]), int)
            self.assertIs(type(record["attributedKernel"]), bool)
            offset = record["functionUtf8Offset"]
            self.assertTrue(encoded[offset:].startswith(record["kernelSymbol"].encode("utf-8")))
        return records

    def test_repeated_kernel_attributes_are_one_occurrence(self):
        source = (
            '#[cfg_attr(feature = "a", kernel(typed))]\n'
            '#[allow(dead_code)] #[cfg_attr(feature = "b", kernel)]\n'
            "pub fn transform() {}\n"
        )
        records = self.assert_items(source, ["transform"], [True])
        self.assertEqual(records[0]["functionUtf8Offset"], source.index("transform"))
        # The historical helper is deliberately not changed to occurrence semantics.
        self.assertEqual(
            self.validator.ordinary_attributed_kernel_names(source),
            ["transform", "transform"],
        )

    def test_same_name_declarations_remain_distinct_in_physical_order(self):
        source = (
            '#[cfg(feature = "a")] #[kernel] fn transform() {}\n'
            '#[cfg(feature = "b")] #[kernel] fn transform() {}\n'
            "mod helpers { fn transform() {} }\n"
        )
        records = self.assert_items(source, ["transform"] * 3, [True, True, False])
        self.assertEqual(len({item["functionUtf8Offset"] for item in records}), 3)

    def test_qualified_paths_cfg_and_attribute_order(self):
        for attributes in [
            "#[fe2o3_device::kernel(typed)] #[allow(dead_code)]",
            "#[allow(dead_code)] #[::fe2o3_device :: kernel(typed)]",
            '#[cfg_attr(feature = "disabled", fe2o3_device::kernel(typed))]',
            '#[cfg_attr(any(), cfg_attr(all(), fe2o3_device::kernel))]',
            "#[r#kernel] #[allow(dead_code)]",
            "#[\u03bc::kernel]",
        ]:
            with self.subTest(attributes=attributes):
                source = attributes + ' pub(crate) unsafe extern "C" fn real() {}'
                self.assert_items(source, ["real"], [True])
        self.assert_items(
            '#[doc = "kernel"] #[cfg(feature = "kernel")] fn helper() {}',
            ["helper"], [False],
        )

    def test_bare_helpers_methods_and_declarations_are_retained(self):
        source = (
            "fn helper() { fn nested() {} }\n"
            "impl Drop for Value { fn drop(&mut self) {} }\n"
            "trait Action { fn perform(&self); }\n"
            'extern "C" { safe fn external(); }\n'
            "type Callback = fn(u32) -> u32;\n"
            "static CALLBACK: Option<unsafe fn()> = None;\n"
        )
        self.assert_items(
            source, ["helper", "nested", "drop", "perform", "external"], [False] * 5
        )

    def test_comments_literals_and_macro_rules_templates_are_excluded(self):
        source = r'''
// #[kernel] fn comment() {}
/* outer fn block() {} /* nested #[kernel] fn nested_comment() {} */ */
const TEXT: &str = "#[kernel] fn string() {}";
const RAW: &str = r###"fn raw_string() {}"###;
const BYTE: &[u8] = br##"#[kernel] fn byte_string() {}"##;
const C: &CStr = c"fn c_string() {}";
const CHARACTER: char = '{';
const ESCAPED: char = '\'';
macro_rules! curly { () => { #[kernel] fn template_a() {} }; }
macro_rules! round (() => { fn template_b() {} });
macro_rules! square [() => { fn template_c() {} }];
#[kernel] fn actual() {}
fn helper() {}
'''
        self.assert_items(source, ["actual", "helper"], [True, False])

    def test_inner_and_non_function_attributes_do_not_leak(self):
        source = (
            "#![kernel]\nfn bare() {}\n"
            "#[kernel] mod nested { fn inside() {} }\n"
            "#[some_attribute(fn not_an_item())] fn attributed_helper() {}\n"
            "#[kernel] struct Holder;\nfn after_struct() {}\n"
        )
        self.assert_items(
            source, ["bare", "inside", "attributed_helper", "after_struct"], [False] * 4
        )

    def test_utf8_offsets_include_multibyte_prefix_and_identifiers(self):
        source = '// \u03bb\U0001f600\nconst TEXT: &str = "\u00e9";\n#[kernel] fn \u03c0() {}\nfn e\u0301() {}\n'
        records = self.assert_items(source, ["\u03c0", "e\u0301"], [True, False])
        for record in records:
            position = source.index(record["kernelSymbol"] + "()")
            self.assertEqual(record["functionUtf8Offset"], len(source[:position].encode("utf-8")))

    def test_raw_identifier_offset_points_to_the_complete_name_token(self):
        source = "// \u03bb\n#[kernel] fn r#type() {}\n"
        record, = self.scan(source)
        self.assertEqual(record, {
            "kernelSymbol": "type",
            "functionUtf8Offset": len(source[:source.index("r#type")].encode("utf-8")),
            "attributedKernel": True,
        })

    def test_unicode_identifier_suffix_is_not_a_function_keyword(self):
        self.assert_items(
            "use e\u0301fn as renamed;\nfn actual() {}\n",
            ["actual"], [False],
        )

    def test_malformed_delimiters_and_literals_fail_closed(self):
        for source in [
            "#[kernel fn missing() {}",
            "fn wrong() {]",
            "fn open() {",
            'const TEXT: &str = "unterminated',
            'const RAW: &str = r#"unterminated',
            "/* unterminated",
            "macro_rules! absent",
        ]:
            with self.subTest(source=source), self.assertRaises(SystemExit):
                self.scan(source)

    def test_source_and_occurrence_bounds_are_checked(self):
        with mock.patch.object(self.validator, "MAX_ATTRIBUTED_SOURCE_BYTES", 12):
            self.assertEqual(self.scan("fn tiny() {}"), [{
                "kernelSymbol": "tiny", "functionUtf8Offset": 3, "attributedKernel": False,
            }])
            for source in ["x" * 13, "\u03bb" * 7]:
                with self.subTest(source=source), mock.patch.object(
                    self.validator, "_rust_code_without_comments_and_literals"
                ) as mask, self.assertRaisesRegex(SystemExit, "source bound"):
                    self.scan(source)
                mask.assert_not_called()
        with mock.patch.object(self.validator, "MAX_KERNEL_PAIR_RECORDS", 2):
            self.assertEqual(len(self.scan("fn a() {} fn b() {}")), 2)
            with self.assertRaisesRegex(SystemExit, "count bound"):
                self.scan("fn a() {} fn b() {} fn c() {}")
        with self.assertRaisesRegex(SystemExit, "invalid Unicode"):
            self.scan("\ud800")

    def test_masking_keeps_coordinates_after_nonzero_literal_offsets(self):
        source = r'''const A: &str = r##"quoted \" fn fake() {}"##;
const B: char = '\n';
const C: &str = "escaped \" fn hidden() {}";
#[kernel] fn real() {}
'''
        masked = self.validator._rust_code_without_comments_and_literals(source)
        self.assertEqual(len(masked), len(source))
        self.assertEqual(
            [index for index, char in enumerate(masked) if char == "\n"],
            [index for index, char in enumerate(source) if char == "\n"],
        )
        self.assert_items(source, ["real"], [True])


if __name__ == "__main__":
    unittest.main()
