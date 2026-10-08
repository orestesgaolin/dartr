# `interface` fixtures

Oracle output of mode `interface` (docs/design/semantics.md §5.2,
`tools/oracle/bin/interface.dart`) for a sample of SDK classes. `dartr dump
interface` must give the same interfaces once the linker builds the SDK
elements (unit B6 wires `dartr_typesystem::interface_dump`).

- `dart_core_async_sample.oracle.jsonl`: the lines of `oracle interface
  dart:core dart:async`, with `"interfaces"` reduced to `Iterable`, `List`,
  `Map`, `int`, `num` (`dart:core`) and `Future` (`dart:async`), in library
  order. Everything else in each kept interface (Object members, private
  names, substituted and combined signatures) is unchanged.

Regenerate (from the repository root):

```sh
dart run tools/oracle/bin/oracle.dart interface dart:core dart:async | python3 -c '
import json, sys
want = {"dart:core": {"Iterable", "List", "Map", "num", "int"}, "dart:async": {"Future"}}
for line in sys.stdin:
    d = json.loads(line)
    d["interfaces"] = [i for i in d["interfaces"] if i["n"] in want[d["path"]]]
    print(json.dumps(d, separators=(",", ":"), ensure_ascii=False))
' > crates/dartr/tests/fixtures/interface/dart_core_async_sample.oracle.jsonl
```

The full oracle output is deterministic: two runs, with the inputs in either
order, give the same bytes for each library.

The differential test that runs today (without the linker) is
`crates/dartr_typesystem/tests/interface_dump_test.rs` with
`crates/dartr_typesystem/tests/fixtures/interface/sample.dart`.
