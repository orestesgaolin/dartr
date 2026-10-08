#!/usr/bin/env python3
"""Generates crates/dartr_element/src/generated/flags.rs from the element
schema (crates/dartr_element/schema/element.json, written by
tools/oracle/bin/element_schema.dart).

The analyzer stores boolean element and fragment properties in two generated
bit sets (`_ElementStorageFlag`, `_FragmentStorageFlag` in element.dart). This
script ports both enums one to one: one constant per enum value, the bit is
the enum index. The `_XElementFlags` companion enums say, for each property,
whether the fragment stores it and where the element reads it from
(`firstFragment`, `stored`, `computed`); that goes into the doc comments.

Usage: python3 tools/codegen/gen_element.py
"""

import json
import os
import re
import subprocess

ROOT = os.path.normpath(os.path.join(os.path.dirname(__file__), "..", ".."))
SCHEMA = os.path.join(ROOT, "crates/dartr_element/schema/element.json")
OUT = os.path.join(ROOT, "crates/dartr_element/src/generated/flags.rs")


def const_name(dart):
    # classElement_isAbstract -> CLASS_ELEMENT_IS_ABSTRACT
    owner, prop = dart.split("_", 1)
    snake = lambda s: re.sub(r"(?<!^)(?=[A-Z])", "_", s).upper()
    return f"{snake(owner)}_{snake(prop)}"


def companion_sources(enums):
    """(ownerLowerCamel, property) -> (fragment, element source)."""
    out = {}
    for name, values in enums.items():
        m = re.match(r"^_(\w+)Flags$", name)
        if not m or name in ("_ElementStorageFlag", "_FragmentStorageFlag"):
            continue
        owner = m.group(1)  # ClassElement, Fragment, Element, ...
        owner = owner[0].lower() + owner[1:]
        for v in values:
            out[(owner, v["name"])] = (v["fragment"], v["element"])
    return out


def gen_flags(rust_name, int_type, dart_enum, values, sources, is_element):
    lines = []
    lines.append(f"/// Dart `{dart_enum}` (element.dart): one bit per value, in enum order.")
    lines.append("#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]")
    lines.append(f"pub struct {rust_name}({int_type});")
    lines.append("")
    lines.append(f"impl {rust_name} {{")
    lines.append(f"    pub const EMPTY: {rust_name} = {rust_name}(0);")
    for i, v in enumerate(values):
        dart = v["name"]
        owner, prop = dart.split("_", 1)
        # classFragment -> classElement companion key; fragment -> fragment.
        key_owner = owner.replace("Fragment", "Element") if owner != "fragment" else "fragment"
        if owner == "element":
            key_owner = "element"
        src = sources.get((key_owner, prop))
        doc = f"    /// `{dart_enum}.{dart}`"
        if src is not None:
            frag, elem = src
            doc += f" (fragment: {str(frag).lower()}, element: {elem})"
        lines.append(doc + ".")
        lines.append(f"    pub const {const_name(dart)}: {rust_name} = {rust_name}(1 << {i});")
    names = ", ".join(f'"{v["name"]}"' for v in values)
    lines.append("")
    lines.append(f"    /// The Dart names, by bit index.")
    lines.append(f"    pub const DART_NAMES: &'static [&'static str] = &[{names}];")
    lines.append("")
    lines.append("    #[inline(always)]")
    lines.append("    pub fn contains(self, flag: Self) -> bool {")
    lines.append("        self.0 & flag.0 == flag.0")
    lines.append("    }")
    lines.append("")
    lines.append("    /// Dart `setFlag(flag, value)`.")
    lines.append("    #[inline(always)]")
    lines.append("    pub fn set(&mut self, flag: Self, value: bool) {")
    lines.append("        if value {")
    lines.append("            self.0 |= flag.0;")
    lines.append("        } else {")
    lines.append("            self.0 &= !flag.0;")
    lines.append("        }")
    lines.append("    }")
    lines.append("")
    lines.append("    #[inline(always)]")
    lines.append("    pub fn with(mut self, flag: Self) -> Self {")
    lines.append("        self.set(flag, true);")
    lines.append("        self")
    lines.append("    }")
    lines.append("")
    lines.append("    #[inline(always)]")
    lines.append(f"    pub fn bits(self) -> {int_type} {{")
    lines.append("        self.0")
    lines.append("    }")
    lines.append("")
    lines.append("    #[inline(always)]")
    lines.append(f"    pub fn from_bits(bits: {int_type}) -> Self {{")
    lines.append("        Self(bits)")
    lines.append("    }")
    lines.append("}")
    lines.append("")
    lines.append(f"impl std::ops::BitOr for {rust_name} {{")
    lines.append("    type Output = Self;")
    lines.append("    fn bitor(self, rhs: Self) -> Self {")
    lines.append(f"        {rust_name}(self.0 | rhs.0)")
    lines.append("    }")
    lines.append("}")
    lines.append("")
    lines.append(f"impl std::fmt::Debug for {rust_name} {{")
    lines.append("    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {")
    lines.append("        let set = Self::DART_NAMES")
    lines.append("            .iter()")
    lines.append("            .enumerate()")
    lines.append("            .filter(|(i, _)| self.0 & (1 << i) != 0)")
    lines.append("            .map(|(_, n)| *n);")
    lines.append(f'        f.debug_set().entries(set).finish()')
    lines.append("    }")
    lines.append("}")
    return lines


def main():
    schema = json.load(open(SCHEMA))
    enums = schema["enums"]
    sources = companion_sources(enums)
    elem = enums["_ElementStorageFlag"]
    frag = enums["_FragmentStorageFlag"]
    assert len(elem) <= 32 and len(frag) <= 64
    out = [
        "// GENERATED by tools/codegen/gen_element.py from schema/element.json. Do not edit.",
        "// Dart source: pkg/analyzer/lib/src/dart/element/element.dart",
        "// (`_ElementStorageFlag`, `_FragmentStorageFlag` and the `_*Flags` companion enums)",
        "",
    ]
    out += gen_flags("ElementFlags", "u32", "_ElementStorageFlag", elem, sources, True)
    out.append("")
    out += gen_flags("FragmentFlags", "u64", "_FragmentStorageFlag", frag, sources, False)
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "w") as f:
        f.write("\n".join(out) + "\n")
    subprocess.run(["rustfmt", "--edition", "2024", OUT], check=True)


if __name__ == "__main__":
    main()
