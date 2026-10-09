"""Generates the fixtures of `tests/minimal_edits.rs` with the Dart SDK on
PATH (3.13.3): for each case, the unformatted source, the formatted source
(`dart format`) and the edits of `dart language-server` for
`textDocument/formatting` or `textDocument/rangeFormatting`.

Usage: python3 -I generate.py  (writes <case>.json next to this script)
"""
import json
import os
import queue
import shutil
import subprocess
import sys
import tempfile
import threading

HERE = os.path.dirname(os.path.abspath(__file__))

NEEDS = """import 'dart:math';
class  A{
int x=1  ;
  void f( int a,int b ){ if(a>b){print( 'a' );} else {print('b');}
  }
  List<int> get items => [1,2,3,];
}
void main(){var a=A();a.f(1,2);
  var veryLongVariableNameNumberOne = max(1000000, 2000000) + max(3000000, 4000000) + 5;
// comment
  print(veryLongVariableNameNumberOne);}
"""

FILES = {
    "pubspec.yaml": "name: p\nenvironment:\n  sdk: ^3.13.0\n",
    "lib/needs.dart": NEEDS,
    "lib/range_split.dart": "void main() {\nprint(1);\n      print(2);\n    print(3);\n}\n",
    "lib/unicode_crlf.dart": "// \U0001F600 émoji\r\nvar  s = '\U0001F600';\r\nvoid f( ){print( s );   /* \U0001F600 */ print(s);}\r\n",
    "lib/short_style.dart": "// @dart = 3.6\nvoid f(int a,int b,{int c=1}){var list=[a,b,c,];print(list);}\n",
    "lib/comments.dart": "/// Doc   \nclass B {   // trailing   \n  /* block */ int y;   \n  B(this.y,);\n}\n",
    "lib/shifts.dart": "var a = 1>>2;\nvar b = <List<List<int>>>[];\nvar c = a  >>>  1;\n",
    "wide/pubspec.yaml": "name: wide\nenvironment:\n  sdk: ^3.13.0\n",
    "wide/analysis_options.yaml": "formatter:\n  page_width: 120\n  trailing_commas: preserve\n",
    "wide/lib/needs.dart": NEEDS,
}

# name: (file, range or None, dart.lineLength or None)
CASES = {
    "needs_full": ("lib/needs.dart", None, None),
    "needs_range": ("lib/needs.dart", ((2, 0), (4, 3)), None),
    "needs_line_length_40": ("lib/needs.dart", None, 40),
    "wide_options_preserve": ("wide/lib/needs.dart", None, None),
    "range_split": ("lib/range_split.dart", ((2, 0), (2, 15)), None),
    "unicode_crlf": ("lib/unicode_crlf.dart", None, None),
    "short_style": ("lib/short_style.dart", None, None),
    "comments": ("lib/comments.dart", None, None),
    "shifts": ("lib/shifts.dart", None, None),
}


class Server:
    def __init__(self, root):
        self.p = subprocess.Popen(
            ["dart", "language-server", "--protocol=lsp"],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        self.q = queue.Queue()
        self.config = {}
        self.id = 0
        threading.Thread(target=self._read, daemon=True).start()
        self.request("initialize", {
            "processId": None, "rootUri": "file://" + root,
            "capabilities": {"workspace": {"configuration": True}}})
        self.notify("initialized", {})
        self.wait_for_configuration()
        # The analysis contexts exist after the first analysis.
        while True:
            m = self.q.get(timeout=120)
            if m and m.get("method") == "$/analyzerStatus" and not m["params"]["isAnalyzing"]:
                break

    def wait_for_configuration(self):
        """Answers the next `workspace/configuration` request; the server
        handles the answer before later messages."""
        while True:
            m = self.q.get(timeout=120)
            if m is None:
                raise SystemExit("server closed")
            if m.get("method") == "workspace/configuration":
                self.send({"jsonrpc": "2.0", "id": m["id"],
                           "result": [self.config for _ in m["params"]["items"]]})
                return
            if "method" in m and "id" in m:
                self.send({"jsonrpc": "2.0", "id": m["id"], "result": None})

    def _read(self):
        f = self.p.stdout
        while True:
            length = None
            while True:
                line = f.readline()
                if not line:
                    self.q.put(None)
                    return
                line = line.decode().strip()
                if not line:
                    if length is not None:
                        break
                    continue
                if line.startswith("Content-Length:"):
                    length = int(line.split(":")[1])
            self.q.put(json.loads(f.read(length)))

    def send(self, m):
        b = json.dumps(m).encode()
        self.p.stdin.write(b"Content-Length: %d\r\n\r\n" % len(b) + b)
        self.p.stdin.flush()

    def notify(self, method, params):
        self.send({"jsonrpc": "2.0", "method": method, "params": params})

    def request(self, method, params):
        self.id += 1
        i = self.id
        self.send({"jsonrpc": "2.0", "id": i, "method": method, "params": params})
        while True:
            m = self.q.get(timeout=120)
            if m is None:
                raise SystemExit("server closed")
            if "method" in m and "id" in m:
                result = None
                if m["method"] == "workspace/configuration":
                    result = [self.config for _ in m["params"]["items"]]
                self.send({"jsonrpc": "2.0", "id": m["id"], "result": result})
            elif "method" not in m and m.get("id") == i:
                return m

    def close(self):
        self.request("shutdown", None)
        self.notify("exit", None)
        self.p.wait(30)


def dart_format(root, rel, page_width):
    args = ["dart", "format", "-o", "show", "--summary=none"]
    if page_width:
        args.append("--page-width=%d" % page_width)
    out = subprocess.run(args + [rel], cwd=root, capture_output=True, check=True)
    return out.stdout.decode()


def to_offsets(text):
    """Line starts in UTF-16 code units (LSP positions)."""
    starts = [0]
    units = 0
    chars = list(text)
    for i, c in enumerate(chars):
        units += 2 if ord(c) > 0xFFFF else 1
        if c == "\n" or (c == "\r" and (i + 1 >= len(chars) or chars[i + 1] != "\n")):
            starts.append(units)
    return starts


def apply_edits(text, edits):
    u = text.encode("utf-16-le")
    starts = to_offsets(text)
    off = lambda p: starts[p["line"]] + p["character"]
    for e in sorted(edits, key=lambda e: off(e["range"]["start"]), reverse=True):
        s, t = off(e["range"]["start"]) * 2, off(e["range"]["end"]) * 2
        u = u[:s] + e["newText"].encode("utf-16-le") + u[t:]
    return u.decode("utf-16-le")


def main():
    root = os.path.realpath(tempfile.mkdtemp(prefix="dartr_minimal_edits_"))
    try:
        for rel, content in FILES.items():
            path = os.path.join(root, rel)
            os.makedirs(os.path.dirname(path), exist_ok=True)
            with open(path, "w", newline="") as f:
                f.write(content)
        server = Server(root)
        for name, (rel, rng, line_length) in CASES.items():
            if server.config.get("lineLength") != line_length:
                server.config = {"lineLength": line_length} if line_length else {}
                server.notify("workspace/didChangeConfiguration", {"settings": None})
                server.wait_for_configuration()
            uri = "file://" + os.path.join(root, rel)
            options = {"tabSize": 2, "insertSpaces": True}
            if rng:
                (sl, sc), (el, ec) = rng
                response = server.request("textDocument/rangeFormatting", {
                    "textDocument": {"uri": uri}, "options": options,
                    "range": {"start": {"line": sl, "character": sc},
                              "end": {"line": el, "character": ec}}})
            else:
                response = server.request("textDocument/formatting", {
                    "textDocument": {"uri": uri}, "options": options})
            edits = response["result"]
            unformatted = FILES[rel]
            formatted = dart_format(root, rel, line_length)
            assert edits, (name, response)
            if not rng:
                # The edits of the server produce the output of `dart format`.
                assert apply_edits(unformatted, edits) == formatted, name
            case = {"file": rel, "unformatted": unformatted, "formatted": formatted, "edits": edits}
            if rng:
                case["range"] = {"start": {"line": rng[0][0], "character": rng[0][1]},
                                 "end": {"line": rng[1][0], "character": rng[1][1]}}
            with open(os.path.join(HERE, name + ".json"), "w") as f:
                json.dump(case, f, indent=1, ensure_ascii=False, sort_keys=True)
                f.write("\n")
            print(name, len(edits), "edits")
        server.close()
    finally:
        shutil.rmtree(root)


if __name__ == "__main__":
    sys.exit(main())
