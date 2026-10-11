#!/usr/bin/env python3
"""Benchmark a Dart language server (dartr or `dart language-server`) for editor use.

One invocation benchmarks one server on one project:

    python3 -I bench/lsp_bench.py --server dartr --project ~/fvm/default/packages/flutter_tools
    python3 -I bench/lsp_bench.py --server dart  --project bench/corpus/analyzer-9.0.0 \\
        --cold-runs 3 --warm-runs 3 --full-runs 1 --out bench/results/lsp-analyzer-dart.json

    --server dart|dartr   `dart language-server` (DART, default: the SDK `dart` on PATH) or
                          `dartr language-server` (DARTR, default: target/release/dartr)
    --cold-runs N         startups with an empty cache (dart: a new temporary `--cache` dir;
                          dartr has no disk cache, so a cold run is the same as a warm run)
    --warm-runs N         startups with the benchmark cache (`--dart-cache`, not ~/.dartServer);
                          one uncounted start warms it first
    --full-runs N         sessions that also run the edit and request phases (benchmark cache)
    --dart-cache DIR      cache directory for warm `dart` runs
                          (default: $TMPDIR/dartr-lsp-bench-cache)
    --reps N              repetitions of each edit (default 5)
    --trace               print every message (first 200 chars) to stderr

Run it through tools/heavy.sh on a shared machine. The script prints a summary and writes all
raw numbers as JSON (--out). It never writes to the project: edits are didOpen/didChange
overlays, which are undone and closed before shutdown.

Phases of a session:
  1. startup: time from `initialize` (sent right after the process starts) to the end of the
     initial analysis: the last `$/progress` end of token ANALYZING (or `$/analyzerStatus`
     isAnalyzing:false), followed by QUIET seconds without a new analysis.
  2. edits (full runs): (a) a statement with a type error in a method body of a leaf file,
     (b) rename of a public member in a library that many files use; each is undone.
     Measured: time to the publishDiagnostics that shows the change, time to the end of the
     analysis that follows, and for (b) the first and the last publishDiagnostics of other files.
  3. requests (full runs): completion after `.` and at statement start, hover, definition,
     references, at positions taken from the edited files and from files that use the member.
Memory is the RSS sum of the server process and its children (`ps`), CPU time the sum of
`ps` cputime of the same processes (sampled every 0.25 s; exited children keep their last value).
"""

import argparse
import json
import os
import queue
import re
import shutil
import statistics
import subprocess
import sys
import tempfile
import threading
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent
INIT_JSON = REPO / "crates/dartr/tests/lsp_fixtures/dart_code_initialize.json"
QUIET = 2.0  # seconds without a new analysis after the last end

# Edit targets per project (directory name). `leaf`: a file with few importers and the line after
# which a statement is inserted (a method body start). `api`: a library used by many files, the
# declaration line and the member that is renamed.
TARGETS = {
    "flutter_tools": {
        "leaf": ("lib/src/commands/doctor.dart", r"^  Future<FlutterCommandResult> runCommand\(\) async \{$"),
        "api": ("lib/src/base/logger.dart", r"^  void printTrace\(String message\);$", "printTrace"),
    },
    "flutter": {
        "leaf": ("lib/src/material/badge.dart", r"^  Widget build\(BuildContext context\) \{$"),
        "api": ("lib/src/widgets/framework.dart",
                r"^  T\? dependOnInheritedWidgetOfExactType<T extends InheritedWidget>",
                "dependOnInheritedWidgetOfExactType"),
    },
    "analyzer-9.0.0": {
        "leaf": ("lib/src/lint/options_rule_validator.dart",
                 r"^  void validate\(DiagnosticReporter reporter, YamlMap options\) \{$"),
        "api": ("lib/src/error/listener.dart", r"^  Diagnostic atNode\($", "atNode"),
    },
    "visible-app": {
        "leaf": ("lib/feedback/view/feedback_submitting_page.dart",
                 r"^  Widget build\(BuildContext context\) \{$"),
        "api": ("packages/simple_date/lib/src/simple_date.dart",
                r"^  DateTime toDateTime\(\) =>", "toDateTime"),
    },
}
ERROR_LINE = "    int lspBenchError = 'x';\n"
KEYWORDS = set("""abstract as assert async await break case catch class const continue covariant
default deferred do dynamic else enum export extends extension external factory false final
finally for get hide if implements import in interface is late library mixin new null on operator
part required rethrow return sealed set show static super switch sync this throw true try typedef
var void when while with yield""".split())
SKIP_DIRS = {".dart_tool", "build", "node_modules", ".git", "ios", "android", "macos", "linux",
             "windows", "web"}


def file_uri(path):
    return "file://" + str(path).replace(" ", "%20")


# --- process tree metrics -----------------------------------------------------------------------

def parse_cputime(s):
    days = 0
    if "-" in s:
        d, s = s.split("-", 1)
        days = int(d)
    parts = [float(p) for p in s.split(":")]
    secs = 0.0
    for p in parts:
        secs = secs * 60 + p
    return days * 86400 + secs


def process_tree(root):
    """{pid: (rss_kb, cpu_s)} of root and its descendants."""
    out = subprocess.run(["ps", "-A", "-o", "pid=,ppid=,rss=,time="], capture_output=True,
                         text=True).stdout
    rows = {}
    children = {}
    for line in out.splitlines():
        f = line.split()
        if len(f) < 4:
            continue
        pid, ppid = int(f[0]), int(f[1])
        rows[pid] = (int(f[2]), parse_cputime(f[3]))
        children.setdefault(ppid, []).append(pid)
    tree = {}
    stack = [root]
    while stack:
        p = stack.pop()
        if p in rows:
            tree[p] = rows[p]
            stack.extend(children.get(p, []))
    return tree


class Sampler(threading.Thread):
    """Samples RSS and CPU time of the process tree every 0.25 s."""

    def __init__(self, pid):
        super().__init__(daemon=True)
        self.pid = pid
        self.cpu = {}  # pid -> last cpu seconds
        self.rss_kb = 0
        self.peak_rss_kb = 0
        self.procs = 0
        self.lock = threading.Lock()
        self.stop = threading.Event()

    def sample(self):
        tree = process_tree(self.pid)
        with self.lock:
            for p, (_, c) in tree.items():
                self.cpu[p] = max(c, self.cpu.get(p, 0.0))
            if tree:
                self.rss_kb = sum(r for r, _ in tree.values())
                self.peak_rss_kb = max(self.peak_rss_kb, self.rss_kb)
                self.procs = len(tree)

    def run(self):
        while not self.stop.wait(0.25):
            self.sample()

    def now(self):
        """Samples once and returns (rss MB, cpu s, processes)."""
        self.sample()
        with self.lock:
            return self.rss_kb / 1024, sum(self.cpu.values()), self.procs


# --- LSP client ---------------------------------------------------------------------------------

class Client:
    def __init__(self, argv, trace=False):
        self.trace = trace
        self.proc = subprocess.Popen(argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                     stderr=subprocess.DEVNULL)
        self.q = queue.Queue()
        self.next_id = 0
        self.diags = {}  # uri -> list
        self.publishes = []  # (time, uri)
        self.open_progress = 0
        self.begins = []  # times
        self.ends = []  # times
        self.status_methods = set()
        self.responses = {}
        threading.Thread(target=self._reader, daemon=True).start()

    def _reader(self):
        out = self.proc.stdout
        while True:
            length = None
            while True:
                line = out.readline()
                if not line:
                    self.q.put(None)
                    return
                line = line.strip()
                if not line:
                    if length is not None:
                        break
                    continue
                if line.lower().startswith(b"content-length:"):
                    length = int(line.split(b":")[1])
            body = out.read(length)
            try:
                self.q.put((time.monotonic(), json.loads(body)))
            except ValueError:  # cut off at exit
                self.q.put(None)
                return

    def send(self, msg):
        body = json.dumps(msg).encode()
        self.proc.stdin.write(b"Content-Length: %d\r\n\r\n" % len(body) + body)
        self.proc.stdin.flush()

    def notify(self, method, params):
        self.send({"jsonrpc": "2.0", "method": method, "params": params})

    def request(self, method, params, timeout=600):
        self.next_id += 1
        rid = self.next_id
        t0 = time.monotonic()
        self.send({"jsonrpc": "2.0", "id": rid, "method": method, "params": params})
        deadline = t0 + timeout
        while rid not in self.responses:
            if not self.pump(deadline):
                raise TimeoutError(f"no response to {method}")
        r = self.responses.pop(rid)
        return r[0] - t0, r[1]

    def pump(self, deadline):
        """Handles one message. False at the deadline or when the server closed its output."""
        try:
            item = self.q.get(timeout=max(0.0, deadline - time.monotonic()))
        except queue.Empty:
            return False
        if item is None:
            raise RuntimeError("server closed its output")
        t, m = item
        if self.trace:
            print(f"[{t:.3f}] {json.dumps(m)[:200]}", file=sys.stderr)
        method = m.get("method")
        if method is None:
            self.responses[m.get("id")] = (t, m)
            return True
        params = m.get("params") or {}
        if "id" in m:
            result = None
            if method == "workspace/configuration":
                result = [{} for _ in params.get("items", [])]
            elif method == "workspace/applyEdit":
                result = {"applied": True}
            self.send({"jsonrpc": "2.0", "id": m["id"], "result": result})
        elif method == "textDocument/publishDiagnostics":
            self.diags[params["uri"]] = params.get("diagnostics", [])
            self.publishes.append((t, params["uri"]))
        elif method == "$/progress" and params.get("token") == "ANALYZING":
            self.status_methods.add("$/progress")
            kind = (params.get("value") or {}).get("kind")
            if kind == "begin":
                self.open_progress += 1
                self.begins.append(t)
            elif kind == "end":
                self.open_progress = max(0, self.open_progress - 1)
                self.ends.append(t)
        elif method == "$/analyzerStatus":
            self.status_methods.add("$/analyzerStatus")
            if params.get("isAnalyzing"):
                self.open_progress = 1
                self.begins.append(t)
            else:
                self.open_progress = 0
                self.ends.append(t)
        return True

    def drain(self, seconds):
        deadline = time.monotonic() + seconds
        while self.pump(deadline):
            pass

    def wait_analysis_done(self, after, timeout, require_end=True, no_begin_grace=5.0):
        """Waits for the end of the analysis that started after `after`, followed by QUIET
        seconds without a new one. Returns the time of the last end, or None when no analysis
        started within `no_begin_grace` seconds (if not require_end)."""
        deadline = after + timeout
        while True:
            now = time.monotonic()
            ends = [e for e in self.ends if e >= after]
            begins = [b for b in self.begins if b >= after]
            if self.open_progress == 0 and ends and now - ends[-1] >= QUIET:
                return ends[-1]
            if not require_end and not begins and now - after >= no_begin_grace:
                return None
            if now > deadline:
                raise TimeoutError("analysis did not finish")
            self.pump(min(deadline, now + 0.1))

    def wait_for(self, pred, timeout):
        deadline = time.monotonic() + timeout
        while not pred():
            if time.monotonic() >= deadline:
                raise TimeoutError("condition not met")
            self.pump(deadline)

    def shutdown(self):
        try:
            self.request("shutdown", None, timeout=30)
            self.notify("exit", None)
            self.proc.stdin.close()
            self.proc.wait(timeout=30)
        except Exception:
            pass
        if self.proc.poll() is None:
            self.proc.kill()
            self.proc.wait()


# --- positions ----------------------------------------------------------------------------------

def code_lines(text):
    """(index, line) of lines in bodies (indent >= 4), without comments, imports or strings."""
    for i, line in enumerate(text.split("\n")):
        s = line.strip()
        if len(line) - len(line.lstrip()) < 4 or s.startswith(("//", "*", "/*", "import ", "export ")):
            continue
        yield i, line


def spread(items, n):
    if len(items) <= n:
        return items
    step = len(items) / n
    return [items[int(k * step)] for k in range(n)]


def positions(files, n):
    """Positions for the request phase in `files` ({path: text})."""
    dot, stmt, ident = [], [], []
    for path, text in files.items():
        for i, line in code_lines(text):
            code = line.split("//")[0]
            for m in re.finditer(r"\b[a-z_]\w*\.(?=[a-z_]\w*)", code):
                if "'" not in code[:m.end()] and '"' not in code[:m.end()]:
                    dot.append((path, i, m.end()))
            m = re.match(r"^(\s+)[a-z_]\w*[.(].*;\s*$", code)
            if m and not code.strip().split("(")[0].split(".")[0] in KEYWORDS:
                stmt.append((path, i, len(m.group(1))))
            for m in re.finditer(r"(?<=[.\s(])([A-Za-z_]\w{2,})\b", code):
                if m.group(1) not in KEYWORDS and "'" not in code[:m.start()] \
                        and '"' not in code[:m.start()]:
                    ident.append((path, i, m.start() + 1))
    return {"dot": spread(dot, n), "stmt": spread(stmt, n), "ident": spread(ident, n)}


def dart_files(root):
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in SKIP_DIRS and not d.startswith(".")]
        for f in filenames:
            if f.endswith(".dart") and not f.endswith((".g.dart", ".gen.dart", ".pb.dart")):
                yield Path(dirpath) / f


def users_of(root, member, limit):
    """Files that use `.member`, and the positions of the uses."""
    pat = re.compile(r"\.(" + re.escape(member) + r")\b")
    found = []
    for p in sorted(dart_files(root)):
        try:
            text = p.read_text()
        except (UnicodeDecodeError, OSError):
            continue
        hits = [(i, m.start(1) + 1) for i, line in enumerate(text.split("\n"))
                for m in pat.finditer(line.split("//")[0])]
        if hits:
            found.append((p, text, hits))
    return spread(found, limit)


# --- session ------------------------------------------------------------------------------------

def server_argv(args, cache):
    if args.server == "dart":
        argv = [args.dart, "language-server", "--protocol=lsp", "--client-id=VS-Code",
                "--client-version=3.120.0"]
        if cache:
            argv.append(f"--cache={cache}")
        return argv
    return [args.dartr, "language-server", "--protocol=lsp", "--client-id=VS-Code",
            "--client-version=3.120.0"]


def init_params(root):
    text = INIT_JSON.read_text()
    text = (text.replace("${ROOT_PATH}", str(root)).replace("${ROOT_URI}", file_uri(root))
            .replace("${ROOT_NAME}", root.name))
    return json.loads(text)


def loadavg():
    return subprocess.run(["sysctl", "-n", "vm.loadavg"], capture_output=True,
                          text=True).stdout.strip().strip("{} ")


def diag_summary(c):
    files = sum(1 for d in c.diags.values() if d)
    by_sev = {}
    for d in c.diags.values():
        for x in d:
            s = x.get("severity", 1)
            by_sev[s] = by_sev.get(s, 0) + 1
    return {"files_with_diagnostics": files, "errors": by_sev.get(1, 0),
            "warnings": by_sev.get(2, 0), "infos": by_sev.get(3, 0) + by_sev.get(4, 0)}


def key(diags):
    return sorted((d["range"]["start"]["line"], d["range"]["start"]["character"],
                   str(d.get("code")), d.get("message", "")) for d in diags)


class Doc:
    def __init__(self, c, path):
        self.c, self.path, self.uri = c, path, file_uri(path)
        self.text = path.read_text()
        self.version = 1
        c.notify("textDocument/didOpen", {"textDocument": {
            "uri": self.uri, "languageId": "dart", "version": 1, "text": self.text}})

    def change(self, start, end, new):
        self.version += 1
        self.c.notify("textDocument/didChange", {
            "textDocument": {"uri": self.uri, "version": self.version},
            "contentChanges": [{"range": {"start": {"line": start[0], "character": start[1]},
                                          "end": {"line": end[0], "character": end[1]}},
                                "text": new}]})
        return time.monotonic()

    def close(self):
        self.c.notify("textDocument/didClose", {"textDocument": {"uri": self.uri}})


def line_of(text, pattern):
    for i, line in enumerate(text.split("\n")):
        if re.search(pattern, line):
            return i
    raise SystemExit(f"pattern not found: {pattern}")


def edit_leaf(c, root, target, reps, timeout):
    rel, anchor = target
    doc = Doc(c, root / rel)
    c.drain(1.0)
    c.wait_analysis_done(time.monotonic() - 1.0, timeout, require_end=False, no_begin_grace=1.0)
    line = line_of(doc.text, anchor) + 1
    base = key(c.diags.get(doc.uri, []))
    out = []
    for _ in range(reps):
        n0 = len(c.publishes)
        t0 = doc.change((line, 0), (line, 0), ERROR_LINE)
        c.wait_for(lambda: any(d["range"]["start"]["line"] == line and d.get("severity") == 1
                               for d in c.diags.get(doc.uri, []))
                   and any(u == doc.uri for _, u in c.publishes[n0:]), timeout)
        t_diag = time.monotonic()
        t_done = c.wait_analysis_done(t0, timeout, require_end=False)
        edit = {"diag_ms": (t_diag - t0) * 1000,
                "analysis_ms": None if t_done is None else (max(t_done, t_diag) - t0) * 1000,
                "publishes": len(c.publishes) - n0}
        n0 = len(c.publishes)
        t0 = doc.change((line, 0), (line + 1, 0), "")
        c.wait_for(lambda: key(c.diags.get(doc.uri, [])) == base
                   and any(u == doc.uri for _, u in c.publishes[n0:]), timeout)
        t_diag = time.monotonic()
        t_done = c.wait_analysis_done(t0, timeout, require_end=False)
        undo = {"diag_ms": (t_diag - t0) * 1000,
                "analysis_ms": None if t_done is None else (max(t_done, t_diag) - t0) * 1000,
                "publishes": len(c.publishes) - n0}
        out.append({"edit": edit, "undo": undo})
    return doc, out


def edit_api(c, root, target, reps, timeout):
    rel, decl, member = target
    doc = Doc(c, root / rel)
    c.drain(1.0)
    c.wait_analysis_done(time.monotonic() - 1.0, timeout, require_end=False, no_begin_grace=1.0)
    line = line_of(doc.text, decl)
    col = doc.text.split("\n")[line].index(member) + len(member)
    out = []
    for _ in range(reps):
        base = {u: key(d) for u, d in c.diags.items()}

        def measure(t0, n0):
            def first_dep():
                return next((t for t, u in c.publishes[n0:]
                             if u != doc.uri and key(c.diags.get(u, [])) != base.get(u, [])),
                            None)
            t_done = c.wait_analysis_done(t0, timeout, require_end=False)
            try:
                # A server that publishes after the end of the analysis gets 10 s more.
                c.wait_for(lambda: first_dep() is not None, 10)
            except TimeoutError:
                pass
            t_first = first_dep()
            others = [t for t, u in c.publishes[n0:] if u != doc.uri]
            changed = sum(1 for u, d in c.diags.items() if key(d) != base.get(u, []))
            return {"first_dep_ms": None if t_first is None else (t_first - t0) * 1000,
                    "last_dep_ms": (max(others) - t0) * 1000 if others else None,
                    "analysis_ms": None if t_done is None else (t_done - t0) * 1000,
                    "publishes": len(c.publishes) - n0, "files_changed": changed}

        n0 = len(c.publishes)
        t0 = doc.change((line, col), (line, col), "LspBench")
        edit = measure(t0, n0)
        # Undo: the baseline is again the state before the edit.
        n0 = len(c.publishes)
        t0 = doc.change((line, col), (line, col + len("LspBench")), "")
        t_done = c.wait_analysis_done(t0, timeout, require_end=False)
        restored_others = [t for t, u in c.publishes[n0:] if u != doc.uri]
        still = sum(1 for u, d in c.diags.items() if key(d) != base.get(u, []))
        undo = {"last_dep_ms": (max(restored_others) - t0) * 1000 if restored_others else None,
                "analysis_ms": None if t_done is None else (t_done - t0) * 1000,
                "publishes": len(c.publishes) - n0, "files_not_restored": still}
        out.append({"edit": edit, "undo": undo})
    return doc, out


def summarize_result(method, result):
    if result is None:
        return 0
    if method == "textDocument/completion":
        items = result.get("items", []) if isinstance(result, dict) else result
        return len(items)
    if method == "textDocument/hover":
        return 1
    if isinstance(result, list):
        return len(result)
    return 1


def request_phase(c, root, target, n):
    rel, _decl, member = target["api"]
    users = users_of(root, member, 6)
    paths = [root / target["leaf"][0], root / rel] + [p for p, _, _ in users]
    docs = {}
    for p in paths:
        if p not in docs:
            docs[p] = Doc(c, p)
    c.drain(1.0)
    c.wait_analysis_done(time.monotonic() - 1.0, 600, require_end=False, no_begin_grace=1.0)
    pos = positions({p: d.text for p, d in docs.items()}, n)
    refs = spread([(p, i, col) for p, _, hits in users for i, col in hits], max(1, n // 2))
    kinds = [
        ("completion_dot", "textDocument/completion", pos["dot"], {}),
        ("completion_stmt", "textDocument/completion", pos["stmt"], {}),
        ("hover", "textDocument/hover", pos["ident"], {}),
        ("definition", "textDocument/definition", pos["ident"], {}),
        ("references", "textDocument/references", refs, {"context": {"includeDeclaration": True}}),
    ]
    out = {}
    for name, method, plist, extra in kinds:
        times, sizes = [], []
        first = None
        for k, (p, line, ch) in enumerate([plist[0]] + plist if plist else []):
            params = {"textDocument": {"uri": file_uri(p)},
                      "position": {"line": line, "character": ch}, **extra}
            if method == "textDocument/completion":
                params["context"] = {"triggerKind": 2 if name == "completion_dot" else 1,
                                     **({"triggerCharacter": "."} if name == "completion_dot"
                                        else {})}
            dt, resp = c.request(method, params)
            if k == 0:
                first = dt * 1000  # warm-up, not counted
                continue
            times.append(dt * 1000)
            sizes.append(-1 if "error" in resp else summarize_result(method, resp.get("result")))
        out[name] = {"first_ms": first, "times_ms": times, "result_sizes": sizes}
    return docs, out


def session(args, root, cold, full):
    temp_cache = tempfile.mkdtemp(prefix="lspbench-cache-") if cold and args.server == "dart" else None
    cache = temp_cache if cold else (args.dart_cache if args.server == "dart" else None)
    load_before = loadavg()
    t0 = time.monotonic()
    c = Client(server_argv(args, cache), trace=args.trace)
    sampler = Sampler(c.proc.pid)
    sampler.start()
    res = {"cold": cold, "full": full, "load_before": load_before}
    try:
        dt, resp = c.request("initialize", init_params(root))
        c.notify("initialized", {})
        res["initialize_ms"] = (time.monotonic() - t0) * 1000
        t_done = c.wait_analysis_done(t0, args.timeout)
        rss, cpu, procs = sampler.now()
        res["startup"] = {"ready_ms": (t_done - t0) * 1000, "rss_mb": rss, "cpu_s": cpu,
                          "processes": procs, "analysis_cycles": len(c.ends),
                          "status_methods": sorted(c.status_methods),
                          "diagnostics": diag_summary(c)}
        if full:
            target = TARGETS[root.name]
            cpu0 = cpu
            leaf_doc, leaf = edit_leaf(c, root, target["leaf"], args.reps, args.timeout)
            api_doc, api = edit_api(c, root, target["api"], args.reps, args.timeout)
            rss, cpu, _ = sampler.now()
            res["edits"] = {"leaf": leaf, "api": api, "rss_mb": rss, "cpu_s": cpu - cpu0}
            cpu0 = cpu
            docs, req = request_phase(c, root, target, args.positions)
            rss, cpu, _ = sampler.now()
            res["requests"] = dict(req, rss_mb=rss, cpu_s=cpu - cpu0)
            for d in [leaf_doc, api_doc] + list(docs.values()):
                d.close()
        res["peak_rss_mb"] = sampler.peak_rss_kb / 1024
        res["load_after"] = loadavg()
    finally:
        sampler.stop.set()
        c.shutdown()
        if temp_cache:
            shutil.rmtree(temp_cache, ignore_errors=True)
    return res


# --- summary ------------------------------------------------------------------------------------

def med_max(xs):
    xs = [x for x in xs if x is not None]
    if not xs:
        return "-"
    return f"{statistics.median(xs):.0f} / {max(xs):.0f}"


def med_p95(xs):
    if not xs:
        return "-"
    s = sorted(xs)
    p95 = s[min(len(s) - 1, int(round(0.95 * (len(s) - 1))))]
    return f"{statistics.median(s):.0f} / {p95:.0f}"


def print_summary(report):
    print(f"== {report['server']} on {report['project']}")
    for kind in ("cold", "warm"):
        runs = report[kind]
        if runs:
            r = [x["startup"]["ready_ms"] / 1000 for x in runs]
            cpu = [x["startup"]["cpu_s"] for x in runs]
            rss = [x["startup"]["rss_mb"] for x in runs]
            print(f"startup {kind}: ready s {', '.join(f'{x:.1f}' for x in r)}; "
                  f"cpu s {', '.join(f'{x:.1f}' for x in cpu)}; rss MB "
                  f"{', '.join(f'{x:.0f}' for x in rss)}; load {runs[0]['load_before']}")
    for x in report["full"]:
        s = x["startup"]
        print(f"full: ready {s['ready_ms'] / 1000:.1f} s, cpu {s['cpu_s']:.1f} s, "
              f"rss {s['rss_mb']:.0f} MB, diagnostics {s['diagnostics']}, "
              f"status {s['status_methods']}")
        e = x["edits"]
        leaf, api = e["leaf"], e["api"]
        print("  leaf edit diag / analysis ms (median / max): "
              f"{med_max([r['edit']['diag_ms'] for r in leaf])}, "
              f"{med_max([r['edit']['analysis_ms'] for r in leaf])}; undo "
              f"{med_max([r['undo']['diag_ms'] for r in leaf])}, "
              f"{med_max([r['undo']['analysis_ms'] for r in leaf])}")
        print("  api edit first dep / last dep / analysis ms: "
              f"{med_max([r['edit']['first_dep_ms'] for r in api])}, "
              f"{med_max([r['edit']['last_dep_ms'] for r in api])}, "
              f"{med_max([r['edit']['analysis_ms'] for r in api])}; files changed "
              f"{[r['edit']['files_changed'] for r in api]}; undo last dep / analysis "
              f"{med_max([r['undo']['last_dep_ms'] for r in api])}, "
              f"{med_max([r['undo']['analysis_ms'] for r in api])}; not restored "
              f"{[r['undo']['files_not_restored'] for r in api]}")
        print(f"  after edits: rss {e['rss_mb']:.0f} MB, edit cpu {e['cpu_s']:.1f} s")
        q = x["requests"]
        for name in ("completion_dot", "completion_stmt", "hover", "definition", "references"):
            r = q[name]
            print(f"  {name}: median / p95 ms {med_p95(r['times_ms'])}, first "
                  f"{r['first_ms']:.0f}, n {len(r['times_ms'])}, sizes {r['result_sizes']}")
        print(f"  after requests: rss {q['rss_mb']:.0f} MB, cpu {q['cpu_s']:.1f} s; "
              f"peak rss {x['peak_rss_mb']:.0f} MB")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--server", choices=["dart", "dartr"], required=True)
    ap.add_argument("--project", required=True)
    ap.add_argument("--dart", default=os.environ.get("DART") or shutil.which("dart"))
    ap.add_argument("--dartr", default=os.environ.get("DARTR")
                    or str(REPO / "target/release/dartr"))
    ap.add_argument("--cold-runs", type=int, default=0)
    ap.add_argument("--warm-runs", type=int, default=0)
    ap.add_argument("--full-runs", type=int, default=1)
    ap.add_argument("--dart-cache", default=os.path.join(
        os.environ.get("TMPDIR", "/tmp"), "dartr-lsp-bench-cache"),
        help="cache directory for warm dart runs (default: $TMPDIR/dartr-lsp-bench-cache)")
    ap.add_argument("--reps", type=int, default=5)
    ap.add_argument("--positions", type=int, default=20)
    ap.add_argument("--timeout", type=float, default=900)
    ap.add_argument("--trace", action="store_true")
    ap.add_argument("--out")
    args = ap.parse_args()
    root = Path(args.project).expanduser().resolve()
    report = {"server": args.server, "project": str(root), "cold": [], "warm": [], "full": [],
              "binary": args.dart if args.server == "dart" else args.dartr}
    for _ in range(args.cold_runs):
        report["cold"].append(session(args, root, cold=True, full=False))
    if args.warm_runs or args.full_runs:
        # Uncounted start: fills the benchmark's Dart cache (--dart-cache) and the file system cache.
        report["warmup"] = session(args, root, cold=False, full=False)
    for _ in range(args.warm_runs):
        report["warm"].append(session(args, root, cold=False, full=False))
    for _ in range(args.full_runs):
        report["full"].append(session(args, root, cold=False, full=True))
    if args.out:
        Path(args.out).write_text(json.dumps(report, indent=1))
    print_summary(report)


if __name__ == "__main__":
    main()
