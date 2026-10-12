#!/usr/bin/env python3
"""LSP parity sweep comparing `dart language-server` and `dartr language-server` on real pub packages.

Usage:
    python3 -I tools/lsp_sweep.py --count 45 --out /private/tmp/.../lspsweep/results.json
    python3 -I tools/lsp_sweep.py --packages path,collection,provider --out /private/tmp/.../lspsweep/subset.json

Follows the rules of lspsweep.md and rules-quiet.md:
- Never modifies ~/.pub-cache.
- Copies only lib/, test/, bin/, example/ (.dart files), pubspec.yaml, and analysis_options.yaml
  into --work-dir (default: /private/tmp/claude-501/-Users-dominik-Projects-dartr/2a3f573b-301e-4a14-8b44-8e2e6b74db28/scratchpad/lspsweep),
  runs `dart pub get --offline`, runs one server at a time with strict timeouts, and deletes each
  package copy immediately after comparing.
"""

import argparse
import json
import os
import queue
import re
import shutil
import subprocess
import sys
import threading
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent
INIT_JSON = REPO / "crates/dartr/tests/lsp_fixtures/dart_code_initialize.json"
DEFAULT_WORK_DIR = Path(
    "/private/tmp/claude-501/-Users-dominik-Projects-dartr/"
    "2a3f573b-301e-4a14-8b44-8e2e6b74db28/scratchpad/lspsweep"
)
DEFAULT_DART = "/Users/dominik/fvm/default/bin/cache/dart-sdk/bin/dart"
DEFAULT_FLUTTER_ROOT = "/Users/dominik/fvm/default"

EXCLUDED_PACKAGES = {
    "analyzer",
    "flutter_tools",
    "_fe_analyzer_shared",
    "analysis_server",
    "analysis_server_plugin",
    "analyzer_plugin",
    "linter",
    "custom_lint",
    "custom_lint_builder",
    "custom_lint_core",
}

ALLOWED_SUBDIRS = ("lib", "test", "bin", "example")
SKIP_DIRS = {
    ".dart_tool",
    "build",
    "ios",
    "android",
    "macos",
    "linux",
    "windows",
    "web",
    "node_modules",
    ".git",
    ".cxx",
}

KEYWORDS = set(
    """abstract as assert async await break case catch class const continue covariant
default deferred do dynamic else enum export extends extension external factory false final
finally for get hide if implements import in interface is late library mixin new null on operator
part required rethrow return sealed set show static super switch sync this throw true try typedef
var void when while with yield""".split()
)

REQUEST_NAMES = [
    "textDocument/completion",
    "textDocument/hover",
    "textDocument/definition",
    "textDocument/references",
    "textDocument/documentHighlight",
    "textDocument/signatureHelp",
    "textDocument/codeAction",
    "textDocument/documentSymbol",
    "textDocument/foldingRange",
    "textDocument/semanticTokens/full",
]


def file_uri(path: Path) -> str:
    return "file://" + str(path).replace(" ", "%20")


def parse_version_key(ver: str):
    """Parses a semver string into a comparable tuple so newest versions sort last."""
    main_part = ver.split("+")[0]
    pre = ""
    if "-" in main_part:
        main_part, pre = main_part.split("-", 1)
    nums = []
    for p in main_part.split("."):
        try:
            nums.append(int(p))
        except ValueError:
            nums.append(0)
    # Release versions sort after pre-release versions
    return (nums, 1 if not pre else 0, pre, ver)


def discover_pub_cache_packages(pub_dev: Path):
    """Returns {pkg_name: Path(newest_version_dir)} from ~/.pub-cache/hosted/pub.dev."""
    by_name = {}
    for entry in sorted(os.listdir(pub_dev)):
        full = pub_dev / entry
        if not full.is_dir():
            continue
        m = re.match(r"^([a-zA-Z0-9_]+)-(\d+\..*)$", entry)
        if not m:
            continue
        name, ver = m.group(1), m.group(2)
        if name in EXCLUDED_PACKAGES:
            continue
        if not (full / "pubspec.yaml").is_file() or not (full / "lib").is_dir():
            continue
        key = parse_version_key(ver)
        if name not in by_name or key > by_name[name][0]:
            by_name[name] = (key, ver, full)
    return {name: (ver, path) for name, (_, ver, path) in sorted(by_name.items())}


def is_flutter_package(pkg_dir: Path) -> bool:
    try:
        text = (pkg_dir / "pubspec.yaml").read_text(encoding="utf-8", errors="replace")
    except OSError:
        return False
    return bool(re.search(r"^\s*flutter\s*:", text, re.MULTILINE))


def count_dart_files_in_pkg(pkg_dir: Path) -> int:
    count = 0
    for sub in ALLOWED_SUBDIRS:
        sub_path = pkg_dir / sub
        if not sub_path.is_dir():
            continue
        for root, dirs, files in os.walk(sub_path):
            dirs[:] = [d for d in dirs if d not in SKIP_DIRS and not d.startswith(".")]
            for f in files:
                if f.endswith(".dart") and not f.endswith(
                    (".g.dart", ".freezed.dart", ".pb.dart", ".gen.dart")
                ):
                    count += 1
    return count


def copy_package_skeleton(src_dir: Path, dst_dir: Path):
    """Copies only lib/, test/, bin/, example/ (.dart files), pubspec.yaml, and analysis_options.yaml."""
    if dst_dir.exists():
        shutil.rmtree(dst_dir)
    dst_dir.mkdir(parents=True, exist_ok=True)
    for cfg in ("pubspec.yaml", "analysis_options.yaml"):
        src_cfg = src_dir / cfg
        if src_cfg.is_file():
            shutil.copy2(src_cfg, dst_dir / cfg)
    for sub in ALLOWED_SUBDIRS:
        src_sub = src_dir / sub
        if not src_sub.is_dir():
            continue
        for root, dirs, files in os.walk(src_sub):
            dirs[:] = [d for d in dirs if d not in SKIP_DIRS and not d.startswith(".")]
            rel_root = Path(root).relative_to(src_dir)
            for f in files:
                if f.endswith(".dart"):
                    target_dir = dst_dir / rel_root
                    target_dir.mkdir(parents=True, exist_ok=True)
                    shutil.copy2(Path(root) / f, target_dir / f)


def list_candidate_dart_files(pkg_dir: Path):
    out = []
    for sub in ALLOWED_SUBDIRS:
        sub_path = pkg_dir / sub
        if not sub_path.is_dir():
            continue
        for root, dirs, files in os.walk(sub_path):
            dirs[:] = sorted(d for d in dirs if d not in SKIP_DIRS and not d.startswith("."))
            for f in sorted(files):
                if f.endswith(".dart") and not f.endswith(
                    (".g.dart", ".freezed.dart", ".pb.dart", ".gen.dart")
                ):
                    out.append(Path(root) / f)
    return out


def spread(items, n):
    if len(items) <= n or n <= 0:
        return list(items)
    step = len(items) / n
    return [items[int(k * step)] for k in range(n)]


# --- Dart source masking and position sampling (adapted from lsp_completion.rs & lsp_navigation.rs) ---


def is_ident_char(c: str) -> bool:
    return c.isalnum() or c == "_" or c == "$"


def code_mask(text: str) -> str:
    """Returns `text` with comments and string contents replaced by spaces/S (preserving newlines and length)."""
    chars = list(text)
    out = list(text)
    n = len(chars)
    i = 0
    while i < n:
        c = chars[i]
        nxt = chars[i + 1] if i + 1 < n else ""
        if c == "/" and nxt == "/":
            while i < n and chars[i] != "\n":
                out[i] = " "
                i += 1
        elif c == "/" and nxt == "*":
            while i < n and not (chars[i] == "*" and (i + 1 < n and chars[i + 1] == "/")):
                if chars[i] != "\n":
                    out[i] = " "
                i += 1
            if i < n:
                out[i] = " "
                if i + 1 < n:
                    out[i + 1] = " "
                i += 2
        elif c in ("'", '"'):
            raw = i > 0 and chars[i - 1] in ("r", "R") and (i < 2 or not is_ident_char(chars[i - 2]))
            triple = i + 2 < n and chars[i + 1] == c and chars[i + 2] == c
            delim_len = 3 if triple else 1
            start = i
            i += delim_len
            while i < n:
                if not raw and chars[i] == "\\":
                    i += 2
                    continue
                if chars[i] == c and (
                    not triple or (i + 2 < n and chars[i + 1] == c and chars[i + 2] == c)
                ):
                    i += delim_len
                    break
                if not triple and chars[i] == "\n":
                    break
                i += 1
            for k in range(start, min(i, n)):
                if chars[k] != "\n":
                    out[k] = "S"
        else:
            i += 1
    return "".join(out)


def utf16_positions(text: str):
    """Returns [(line, utf16_col)] for each char index in `text` (plus end)."""
    out = []
    line = 0
    col = 0
    for c in text:
        out.append((line, col))
        if c == "\n":
            line += 1
            col = 0
        else:
            col += 2 if ord(c) > 0xFFFF else 1
    out.append((line, col))
    return out


def sample_file_positions(text: str, max_ident=12, max_comp=10, max_sig=6):
    """Samples ~30 positions in `text` for navigation/hover, completion, and signatureHelp."""
    mask = code_mask(text)
    pos_map = utf16_positions(text)
    n = len(mask)

    idents = []
    dots = []
    stmts = []
    sigs = []

    depth = 0
    i = 0
    while i < n:
        c = mask[i]
        if c == "{":
            depth += 1
        elif c == "}":
            depth = max(0, depth - 1)

        # After `.` of a member access
        if (
            c == "."
            and i > 0
            and (is_ident_char(mask[i - 1]) or mask[i - 1] == ")")
            and not mask[i - 1].isdigit()
            and mask[i - 1] != "."
            and i + 1 < n
            and (mask[i + 1].isalpha() or mask[i + 1] == "_")
        ):
            dots.append(pos_map[i + 1])

        # After `(` or `,` for signatureHelp
        if c in ("(", ",") and depth > 0:
            # Check not control-flow keyword before `(`
            if c == "(":
                j = i
                while j > 0 and is_ident_char(mask[j - 1]):
                    j -= 1
                w = mask[j:i]
                if w not in ("if", "for", "while", "switch", "catch", "assert"):
                    sigs.append(pos_map[min(i + 1, n)])
            else:
                sigs.append(pos_map[min(i + 1, n)])

        # Statement start inside body
        if depth >= 2 and c not in (" ", "\n", "\r", "\t", "}") and is_ident_char(c):
            j = i
            while j > 0 and mask[j - 1] in (" ", "\t"):
                j -= 1
            if j > 0 and mask[j - 1] == "\n":
                k = j - 1
                while k > 0 and mask[k - 1] in (" ", "\t", "\n", "\r"):
                    k -= 1
                if k > 0 and mask[k - 1] in (";", "{", "}"):
                    stmts.append(pos_map[i])

        # Identifiers
        if c.isalpha() or c == "_" or c == "$":
            start = i
            while i < n and is_ident_char(mask[i]):
                i += 1
            word = mask[start:i]
            if word not in KEYWORDS and "S" not in word and len(word) >= 2:
                mid = start + (i - start) // 2
                idents.append(pos_map[mid])
            continue

        i += 1

    sampled_idents = spread(idents, max_ident)
    half_comp = max_comp // 2
    sampled_comp = spread(dots, half_comp) + spread(stmts, max_comp - half_comp)
    if len(sampled_comp) < max_comp:
        extra = [p for p in spread(idents, max_comp) if p not in sampled_comp]
        sampled_comp.extend(extra[: max_comp - len(sampled_comp)])
    sampled_sigs = spread(sigs, max_sig)

    return {
        "ident": sampled_idents,
        "completion": sampled_comp,
        "signatureHelp": sampled_sigs,
    }


# --- LSP Client -----------------------------------------------------------------------------------


class LspClient:
    def __init__(self, argv, env=None):
        self.argv = argv
        self.proc = subprocess.Popen(
            argv,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            env=env,
        )
        self.q = queue.Queue()
        self.next_id = 0
        self.diags = {}  # uri -> list of diagnostics
        self.open_progress = 0
        self.progress_ends = 0
        self.responses = {}
        self.server_requests = []
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
            except Exception:
                self.q.put(None)
                return

    def send(self, msg):
        body = json.dumps(msg).encode("utf-8")
        self.proc.stdin.write(b"Content-Length: %d\r\n\r\n" % len(body) + body)
        self.proc.stdin.flush()

    def notify(self, method, params):
        self.send({"jsonrpc": "2.0", "method": method, "params": params})

    def request(self, method, params, timeout=120.0):
        self.next_id += 1
        rid = self.next_id
        self.send({"jsonrpc": "2.0", "id": rid, "method": method, "params": params})
        deadline = time.monotonic() + timeout
        while rid not in self.responses:
            if not self.pump(deadline):
                raise TimeoutError(f"Timeout waiting for {method} (id={rid})")
        return self.responses.pop(rid)

    def pump(self, deadline, quiet_timeout=None):
        wait = max(0.0, deadline - time.monotonic())
        if quiet_timeout is not None:
            wait = min(wait, quiet_timeout)
        try:
            item = self.q.get(timeout=wait)
        except queue.Empty:
            return False
        if item is None:
            raise RuntimeError(f"Server closed output unexpectedly: {self.argv}")
        _, m = item
        method = m.get("method")
        if method is None:
            self.responses[m.get("id")] = m
            return True
        params = m.get("params") or {}
        if "id" in m:
            self.server_requests.append((method, params))
            result = None
            if method == "workspace/configuration":
                result = [{} for _ in params.get("items", [])]
            elif method == "workspace/applyEdit":
                result = {"applied": True}
            self.send({"jsonrpc": "2.0", "id": m["id"], "result": result})
        elif method == "textDocument/publishDiagnostics":
            self.diags[params.get("uri", "")] = params.get("diagnostics", [])
        elif method == "$/progress":
            kind = (params.get("value") or {}).get("kind")
            if kind == "begin":
                self.open_progress += 1
            elif kind == "end":
                self.open_progress = max(0, self.open_progress - 1)
                self.progress_ends += 1
        elif method == "$/analyzerStatus":
            if params.get("isAnalyzing"):
                self.open_progress = 1
            else:
                self.open_progress = 0
                self.progress_ends += 1
        return True

    def settle(self, expect_analysis=True, timeout=180.0, grace=10.0, quiet=0.4):
        ends_before = self.progress_ends
        now = time.monotonic()
        deadline = now + timeout
        while True:
            got = self.pump(deadline, quiet_timeout=quiet)
            if not got:
                analysis_done = (not expect_analysis) or (self.progress_ends > ends_before)
                if self.open_progress == 0 and analysis_done:
                    return
            if time.monotonic() >= deadline:
                raise TimeoutError(f"Server did not settle within {timeout}s: {self.argv}")

    def shutdown(self):
        try:
            if self.proc.poll() is None:
                self.request("shutdown", None, timeout=15.0)
                self.notify("exit", None)
                self.proc.stdin.close()
                self.proc.wait(timeout=10.0)
        except Exception:
            pass
        finally:
            if self.proc.poll() is None:
                self.proc.kill()
                self.proc.wait()


# --- Normalization and Comparison -----------------------------------------------------------------


def canonical_json(v) -> str:
    return json.dumps(v, sort_keys=True, separators=(",", ":"))


def insert_text_of(item: dict) -> str:
    if "textEditText" in item:
        return item["textEditText"]
    te = item.get("textEdit")
    if isinstance(te, dict) and "newText" in te:
        return te["newText"]
    if "insertText" in item:
        return item["insertText"]
    return item.get("label", "")


def normalize_completion(resp: dict) -> dict:
    if "error" in resp:
        return {"error": resp["error"].get("code")}
    res = resp.get("result")
    if res is None:
        return {"isIncomplete": False, "labels": [], "kinds": [], "insertText": [], "top20": []}
    if isinstance(res, list):
        items = res
        is_incomplete = False
    else:
        items = res.get("items") or []
        is_incomplete = bool(res.get("isIncomplete", False))

    # Separate snippets (kind 15) from ranked items as in lsp_completion.rs, or include all:
    # Let's include all items in labels/kinds/insertText, and for top20 sort by (sortText, label)
    labels = sorted(i.get("label", "") for i in items)
    kinds = sorted((i.get("label", ""), i.get("kind")) for i in items)
    inserts = sorted((i.get("label", ""), insert_text_of(i)) for i in items)

    # First 20 in sortText order (breaking ties on equal sortText by label, kind, insertText
    # while also recording raw sortText + label for top 20)
    ranked = [i for i in items if i.get("kind") != 15]
    ranked_sorted = sorted(
        ranked,
        key=lambda i: (
            i.get("sortText", ""),
            i.get("label", ""),
            i.get("kind") or 0,
            insert_text_of(i),
        ),
    )
    top20 = [
        {
            "label": i.get("label", ""),
            "kind": i.get("kind"),
            "sortText": i.get("sortText", ""),
            "insertText": insert_text_of(i),
        }
        for i in ranked_sorted[:20]
    ]
    return {
        "isIncomplete": is_incomplete,
        "labels": labels,
        "kinds": kinds,
        "insertText": inserts,
        "top20": top20,
    }


def normalize_hover(resp: dict):
    if "error" in resp:
        return {"error": resp["error"].get("code")}
    res = resp.get("result")
    if res is None:
        return None
    return res.get("contents")


def normalize_definition(resp: dict):
    if "error" in resp:
        return {"error": resp["error"].get("code")}
    res = resp.get("result")
    if res is None:
        return None
    if isinstance(res, list):
        return sorted(res, key=canonical_json)
    return res


def normalize_references(resp: dict):
    if "error" in resp:
        return {"error": resp["error"].get("code")}
    res = resp.get("result")
    if res is None:
        return None
    if isinstance(res, list):
        locs = sorted(res, key=canonical_json)
        return {"count": len(locs), "locations": locs}
    return res


def normalize_document_highlight(resp: dict):
    if "error" in resp:
        return {"error": resp["error"].get("code")}
    res = resp.get("result")
    if res is None:
        return None
    if isinstance(res, list):
        return sorted(res, key=canonical_json)
    return res


def normalize_signature_help(resp: dict):
    if "error" in resp:
        return {"error": resp["error"].get("code")}
    return resp.get("result")


def sort_workspace_edit(edit):
    if not isinstance(edit, dict):
        return edit
    edit = dict(edit)
    if isinstance(edit.get("documentChanges"), list):
        edit["documentChanges"] = sorted(
            edit["documentChanges"],
            key=lambda c: (
                (c.get("textDocument") or {}).get("uri")
                or c.get("uri")
                or c.get("oldUri")
                or ""
            ),
        )
    return edit


def normalize_code_action(resp: dict, resolved_edit=None):
    if "error" in resp:
        return {"error": resp["error"].get("code")}
    res = resp.get("result")
    if res is None:
        return None
    actions = [
        {"title": a.get("title"), "kind": a.get("kind")}
        for a in res
        if isinstance(a, dict)
    ]
    first_edit = sort_workspace_edit(resolved_edit)
    return {"actions": actions, "firstQuickFixEdit": first_edit}


def normalize_document_symbol(resp: dict):
    if "error" in resp:
        return {"error": resp["error"].get("code")}
    return resp.get("result")


def normalize_folding_range(resp: dict):
    if "error" in resp:
        return {"error": resp["error"].get("code")}
    return resp.get("result")


def normalize_semantic_tokens(resp: dict):
    if "error" in resp:
        return {"error": resp["error"].get("code")}
    res = resp.get("result")
    if res is None:
        return None
    return res.get("data")


def init_params(root: Path) -> dict:
    text = INIT_JSON.read_text(encoding="utf-8")
    text = (
        text.replace("${ROOT_PATH}", str(root))
        .replace("${ROOT_URI}", file_uri(root))
        .replace("${ROOT_NAME}", root.name)
    )
    params = json.loads(text)
    params["initializationOptions"]["completionBudgetMilliseconds"] = 100000
    return params


def run_server_on_package(
    argv,
    env,
    pkg_root: Path,
    sampled_files=None,
    file_plans=None,
    max_files=4,
):
    """Runs one LSP server session on `pkg_root`.
    If `sampled_files` and `file_plans` are None (the `dart` oracle run), selects the files and
    diagnostics and returns `(sampled_files, file_plans, responses, elapsed)`.
    For `dartr`, reuses the exact `sampled_files` and `file_plans` from the `dart` run!
    """
    t0 = time.monotonic()
    c = LspClient(argv, env=env)
    responses = {m: [] for m in REQUEST_NAMES}
    try:
        c.request("initialize", init_params(pkg_root), timeout=60.0)
        c.notify("initialized", {})
        c.settle(expect_analysis=True, timeout=120.0, grace=15.0)

        if sampled_files is None:
            all_files = list_candidate_dart_files(pkg_root)
            # Prioritize files that have diagnostics from initial analysis so codeAction is well tested
            with_diags = [f for f in all_files if c.diags.get(file_uri(f))]
            without_diags = [f for f in all_files if not c.diags.get(file_uri(f))]
            chosen = spread(with_diags, max(1, max_files // 2))
            rem = max_files - len(chosen)
            chosen.extend(spread(without_diags, rem))
            if len(chosen) < max_files:
                for f in with_diags:
                    if f not in chosen and len(chosen) < max_files:
                        chosen.append(f)
            sampled_files = sorted(chosen)

        # Open all sampled files
        file_texts = {}
        for f in sampled_files:
            text = f.read_bytes().decode("utf-8", errors="replace")
            file_texts[f] = text
            c.notify(
                "textDocument/didOpen",
                {
                    "textDocument": {
                        "uri": file_uri(f),
                        "languageId": "dart",
                        "version": 1,
                        "text": text,
                    }
                },
            )
        c.settle(expect_analysis=False, timeout=60.0, grace=3.0)

        if file_plans is None:
            file_plans = {}
            for f in sampled_files:
                uri = file_uri(f)
                pos = sample_file_positions(file_texts[f])
                diags = (c.diags.get(uri) or [])[:8]
                file_plans[str(f)] = {
                    "ident": pos["ident"],
                    "completion": pos["completion"],
                    "signatureHelp": pos["signatureHelp"],
                    "diagnostics": diags,
                }

        for f in sampled_files:
            uri = file_uri(f)
            rel = str(f.relative_to(pkg_root))
            plan = file_plans[str(f)]

            # 1. Completion
            for line, col in plan["completion"]:
                r = c.request(
                    "textDocument/completion",
                    {
                        "textDocument": {"uri": uri},
                        "position": {"line": line, "character": col},
                        "context": {"triggerKind": 1},
                    },
                )
                responses["textDocument/completion"].append(
                    (rel, (line, col), normalize_completion(r))
                )

            # 2-5. Hover, definition, references, documentHighlight at ident positions
            for line, col in plan["ident"]:
                pos_obj = {"line": line, "character": col}
                r_hover = c.request(
                    "textDocument/hover",
                    {"textDocument": {"uri": uri}, "position": pos_obj},
                )
                responses["textDocument/hover"].append(
                    (rel, (line, col), normalize_hover(r_hover))
                )

                r_def = c.request(
                    "textDocument/definition",
                    {"textDocument": {"uri": uri}, "position": pos_obj},
                )
                responses["textDocument/definition"].append(
                    (rel, (line, col), normalize_definition(r_def))
                )

                r_refs = c.request(
                    "textDocument/references",
                    {
                        "textDocument": {"uri": uri},
                        "position": pos_obj,
                        "context": {"includeDeclaration": True},
                    },
                )
                responses["textDocument/references"].append(
                    (rel, (line, col), normalize_references(r_refs))
                )

                r_hl = c.request(
                    "textDocument/documentHighlight",
                    {"textDocument": {"uri": uri}, "position": pos_obj},
                )
                responses["textDocument/documentHighlight"].append(
                    (rel, (line, col), normalize_document_highlight(r_hl))
                )

            # 6. signatureHelp
            for line, col in plan["signatureHelp"]:
                r_sig = c.request(
                    "textDocument/signatureHelp",
                    {
                        "textDocument": {"uri": uri},
                        "position": {"line": line, "character": col},
                    },
                )
                responses["textDocument/signatureHelp"].append(
                    (rel, (line, col), normalize_signature_help(r_sig))
                )

            # 7. codeAction at diagnostics
            for d in plan["diagnostics"]:
                rng = d["range"]
                loc = (rng["start"]["line"], rng["start"]["character"])
                code_name = str(d.get("code", ""))
                r_ca = c.request(
                    "textDocument/codeAction",
                    {
                        "textDocument": {"uri": uri},
                        "range": rng,
                        "context": {"diagnostics": [d], "only": ["quickfix"]},
                    },
                )
                first_edit = None
                res_list = r_ca.get("result")
                if isinstance(res_list, list) and res_list:
                    first = res_list[0]
                    if isinstance(first, dict):
                        first_edit = first.get("edit")
                        if first_edit is None and first.get("command"):
                            cmd = first["command"]
                            if isinstance(cmd, dict) and cmd.get("command") == "dart.edit.codeAction.apply":
                                before = len(c.server_requests)
                                c.request("workspace/executeCommand", cmd)
                                for m_name, m_params in c.server_requests[before:]:
                                    if m_name == "workspace/applyEdit":
                                        first_edit = m_params.get("edit", m_params)
                                        break
                responses["textDocument/codeAction"].append(
                    (f"{rel} ({code_name})", loc, normalize_code_action(r_ca, first_edit))
                )

            # 8-10. documentSymbol, foldingRange, semanticTokens/full
            r_sym = c.request(
                "textDocument/documentSymbol",
                {"textDocument": {"uri": uri}},
            )
            responses["textDocument/documentSymbol"].append(
                (rel, (0, 0), normalize_document_symbol(r_sym))
            )

            r_fold = c.request(
                "textDocument/foldingRange",
                {"textDocument": {"uri": uri}},
            )
            responses["textDocument/foldingRange"].append(
                (rel, (0, 0), normalize_folding_range(r_fold))
            )

            r_sem = c.request(
                "textDocument/semanticTokens/full",
                {"textDocument": {"uri": uri}},
            )
            responses["textDocument/semanticTokens/full"].append(
                (rel, (0, 0), normalize_semantic_tokens(r_sem))
            )

        for f in sampled_files:
            c.notify("textDocument/didClose", {"textDocument": {"uri": file_uri(f)}})
    finally:
        c.shutdown()

    elapsed = time.monotonic() - t0
    return sampled_files, file_plans, responses, elapsed


def describe_diff(method: str, d_val, r_val) -> str:
    if method == "textDocument/completion":
        if not isinstance(d_val, dict) or not isinstance(r_val, dict):
            return f"dart={d_val} dartr={r_val}"
        parts = []
        if d_val.get("isIncomplete") != r_val.get("isIncomplete"):
            parts.append(
                f"isIncomplete: dart={d_val.get('isIncomplete')} dartr={r_val.get('isIncomplete')}"
            )
        if d_val.get("labels") != r_val.get("labels"):
            ds = set(d_val.get("labels", []))
            rs = set(r_val.get("labels", []))
            miss = sorted(ds - rs)[:8]
            extra = sorted(rs - ds)[:8]
            parts.append(f"labels(miss={miss}, extra={extra})")
        elif d_val.get("kinds") != r_val.get("kinds"):
            dk = set(map(tuple, d_val.get("kinds", [])))
            rk = set(map(tuple, r_val.get("kinds", [])))
            parts.append(f"kinds(dart={sorted(dk - rk)[:5]}, dartr={sorted(rk - dk)[:5]})")
        elif d_val.get("insertText") != r_val.get("insertText"):
            di = set(map(tuple, d_val.get("insertText", [])))
            ri = set(map(tuple, r_val.get("insertText", [])))
            parts.append(f"insertText(dart={sorted(di - ri)[:5]}, dartr={sorted(ri - di)[:5]})")
        if d_val.get("top20") != r_val.get("top20"):
            dt = [(x["label"], x["sortText"], x["kind"], x["insertText"]) for x in d_val.get("top20", [])]
            rt = [(x["label"], x["sortText"], x["kind"], x["insertText"]) for x in r_val.get("top20", [])]
            for idx, (x, y) in enumerate(zip(dt, rt)):
                if x != y:
                    parts.append(f"top20[{idx}]: dart={x} vs dartr={y}")
                    break
            else:
                parts.append(f"top20 len: dart={len(dt)} vs dartr={len(rt)}")
        return "; ".join(parts) or "diff"

    if method == "textDocument/references":
        if isinstance(d_val, dict) and isinstance(r_val, dict):
            d_locs = {canonical_json(x): x for x in d_val.get("locations", [])}
            r_locs = {canonical_json(x): x for x in r_val.get("locations", [])}
            only_d = [d_locs[k] for k in sorted(d_locs.keys() - r_locs.keys())[:5]]
            only_r = [r_locs[k] for k in sorted(r_locs.keys() - d_locs.keys())[:5]]
            return (
                f"count: dart={d_val.get('count')} dartr={r_val.get('count')}; "
                f"only_dart={json.dumps(only_d)} only_dartr={json.dumps(only_r)}"
            )

    if method == "textDocument/codeAction":
        if isinstance(d_val, dict) and isinstance(r_val, dict):
            da = d_val.get("actions", [])
            ra = r_val.get("actions", [])
            if da != ra:
                return f"actions: dart={da} vs dartr={ra}"
            return (
                f"firstQuickFixEdit differs: dart={json.dumps(d_val.get('firstQuickFixEdit'))[:250]} "
                f"vs dartr={json.dumps(r_val.get('firstQuickFixEdit'))[:250]}"
            )

    if method == "textDocument/semanticTokens/full" and isinstance(d_val, list) and isinstance(r_val, list):
        def decode_tokens(arr):
            out = []
            line, col = 0, 0
            for i in range(0, len(arr), 5):
                dl, dc, ln, ty, mod = arr[i:i+5]
                if dl > 0:
                    line += dl
                    col = dc
                else:
                    col += dc
                out.append((line + 1, col + 1, ln, ty, mod))
            return out
        dt, rt = decode_tokens(d_val), decode_tokens(r_val)
        diffs_found = []
        for idx in range(max(len(dt), len(rt))):
            dv = dt[idx] if idx < len(dt) else None
            rv = rt[idx] if idx < len(rt) else None
            if dv != rv:
                diffs_found.append(f"tok[{idx}]: dart={dv} vs dartr={rv}")
                if len(diffs_found) >= 5:
                    break
        return f"len: dart={len(dt)} dartr={len(rt)}; " + "; ".join(diffs_found)

    s_d = json.dumps(d_val, ensure_ascii=False)
    s_r = json.dumps(r_val, ensure_ascii=False)
    if len(s_d) > 300:
        s_d = s_d[:300] + "..."
    if len(s_r) > 300:
        s_r = s_r[:300] + "..."
    return f"dart={s_d} | dartr={s_r}"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--pub-dev", default=os.path.expanduser("~/.pub-cache/hosted/pub.dev"))
    ap.add_argument("--work-dir", default=str(DEFAULT_WORK_DIR))
    ap.add_argument("--dart", default=DEFAULT_DART if os.path.exists(DEFAULT_DART) else (shutil.which("dart") or "dart"))
    ap.add_argument("--dartr", default=str(REPO / "target/release/dartr"))
    ap.add_argument("--count", type=int, default=42, help="Number of packages to test")
    ap.add_argument("--max-files", type=int, default=4, help="Sampled files per package")
    ap.add_argument("--packages", default=None, help="Comma-separated list of package names")
    ap.add_argument("--out", default=None, help="Output JSON report path")
    ap.add_argument("--append-to", default=None, help="Existing JSON report to merge into")
    args = ap.parse_args()

    pub_dev = Path(args.pub_dev).expanduser().resolve()
    work_dir = Path(args.work_dir).expanduser().resolve()
    work_dir.mkdir(parents=True, exist_ok=True)

    env = os.environ.copy()
    dart_bin_dir = str(Path(args.dart).resolve().parent)
    env["PATH"] = f"{dart_bin_dir}:{env.get('PATH', '')}"
    env["FLUTTER_ROOT"] = DEFAULT_FLUTTER_ROOT
    env["PUB_CACHE"] = os.path.expanduser("~/.pub-cache")
    env["GIT_CONFIG_GLOBAL"] = "/dev/null"

    all_pkgs = discover_pub_cache_packages(pub_dev)
    if args.packages:
        wanted = [p.strip() for p in args.packages.split(",") if p.strip()]
        candidates = [(p, all_pkgs[p][0], all_pkgs[p][1]) for p in wanted if p in all_pkgs]
    else:
        pure_dart = []
        flutter_pkgs = []
        for name, (ver, pdir) in all_pkgs.items():
            n_files = count_dart_files_in_pkg(pdir)
            if n_files < 2 or n_files > 150:
                continue
            if is_flutter_package(pdir):
                flutter_pkgs.append((name, ver, pdir))
            else:
                pure_dart.append((name, ver, pdir))
        # Interleave pure Dart and Flutter packages deterministically
        pure_sample = spread(pure_dart, args.count * 3)
        flut_sample = spread(flutter_pkgs, args.count * 3)
        candidates = []
        for i in range(max(len(pure_sample), len(flut_sample))):
            if i < len(pure_sample):
                candidates.append(pure_sample[i])
            if i < len(flut_sample):
                candidates.append(flut_sample[i])

    dart_argv = [
        args.dart,
        "language-server",
        "--protocol=lsp",
        "--client-id=VS-Code",
        "--client-version=3.144.0",
    ]
    dartr_argv = [
        args.dartr,
        "language-server",
        "--protocol=lsp",
        "--client-id=VS-Code",
        "--client-version=3.144.0",
    ]

    totals = {m: {"total": 0, "identical": 0, "diffs": []} for m in REQUEST_NAMES}
    completion_sub = {
        "isIncomplete": 0,
        "labels": 0,
        "kinds": 0,
        "insertText": 0,
        "top20": 0,
    }
    tested_packages = []
    already_tested = set()
    if args.append_to and os.path.isfile(args.append_to):
        existing = json.loads(Path(args.append_to).read_text(encoding="utf-8"))
        totals = existing["totals"]
        completion_sub = existing["completion_breakdown"]
        tested_packages = existing["packages"]
        already_tested = {p["package"] for p in tested_packages}

    for pkg_name, ver, src_dir in candidates:
        if pkg_name in already_tested:
            continue
        if len(tested_packages) >= args.count and not args.packages:
            break
        pkg_work = work_dir / f"{pkg_name}-{ver}"
        try:
            copy_package_skeleton(src_dir, pkg_work)
            pub_res = subprocess.run(
                [args.dart, "pub", "get", "--offline"],
                cwd=pkg_work,
                env=env,
                capture_output=True,
                text=True,
                timeout=60,
            )
            if pub_res.returncode != 0:
                continue

            is_flut = is_flutter_package(src_dir)
            sampled_files, file_plans, dart_res, dart_time = run_server_on_package(
                dart_argv,
                env,
                pkg_work,
                max_files=args.max_files,
            )
            _, _, dartr_res, dartr_time = run_server_on_package(
                dartr_argv,
                env,
                pkg_work,
                sampled_files=sampled_files,
                file_plans=file_plans,
                max_files=args.max_files,
            )

            pkg_summary = {
                "package": pkg_name,
                "version": ver,
                "flutter": is_flut,
                "files": len(sampled_files),
                "dart_time_s": round(dart_time, 2),
                "dartr_time_s": round(dartr_time, 2),
                "requests": {},
            }

            pkg_diffs = 0
            for m in REQUEST_NAMES:
                d_list = dart_res[m]
                r_list = dartr_res[m]
                m_total = len(d_list)
                m_same = 0
                for (rel, pos, d_val), (_, _, r_val) in zip(d_list, r_list):
                    totals[m]["total"] += 1
                    if m == "textDocument/completion" and isinstance(d_val, dict) and isinstance(r_val, dict):
                        for k in completion_sub:
                            if d_val.get(k) == r_val.get(k):
                                completion_sub[k] += 1
                    if d_val == r_val:
                        m_same += 1
                        totals[m]["identical"] += 1
                    else:
                        pkg_diffs += 1
                        if len(totals[m]["diffs"]) < 300:
                            totals[m]["diffs"].append(
                                {
                                    "package": f"{pkg_name}-{ver}",
                                    "file": rel,
                                    "position": [pos[0] + 1, pos[1] + 1],
                                    "summary": describe_diff(m, d_val, r_val),
                                }
                            )
                pkg_summary["requests"][m] = {"identical": m_same, "total": m_total}

            tested_packages.append(pkg_summary)
            idx = len(tested_packages)
            print(
                f"[{idx}/{args.count}] {pkg_name}-{ver} ({'flutter' if is_flut else 'dart'}, "
                f"{len(sampled_files)} files): dart={dart_time:.1f}s dartr={dartr_time:.1f}s "
                f"diffs={pkg_diffs}",
                flush=True,
            )
            if args.out:
                out_path = Path(args.out)
                out_path.parent.mkdir(parents=True, exist_ok=True)
                out_path.write_text(
                    json.dumps(
                        {
                            "packages_tested": len(tested_packages),
                            "packages": tested_packages,
                            "totals": totals,
                            "completion_breakdown": completion_sub,
                        },
                        indent=2,
                    ),
                    encoding="utf-8",
                )
        except Exception as e:
            print(f"[SKIP/ERR] {pkg_name}-{ver}: {e}", file=sys.stderr, flush=True)
        finally:
            shutil.rmtree(pkg_work, ignore_errors=True)

    print("\n=== SWEEP SUMMARY ===")
    print(f"Packages tested: {len(tested_packages)}")
    for m in REQUEST_NAMES:
        t = totals[m]["total"]
        s = totals[m]["identical"]
        pct = (100.0 * s / t) if t else 100.0
        print(f"  {m:<34} {s:5d} / {t:<5d} ({pct:6.2f}%)")
    if totals["textDocument/completion"]["total"] > 0:
        ct = totals["textDocument/completion"]["total"]
        print("  completion breakdown:")
        for k, v in completion_sub.items():
            print(f"    {k:<16} {v:5d} / {ct:<5d} ({100.0 * v / ct:6.2f}%)")

    report = {
        "packages_tested": len(tested_packages),
        "packages": tested_packages,
        "totals": totals,
        "completion_breakdown": completion_sub,
    }
    if args.out:
        out_path = Path(args.out)
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(json.dumps(report, indent=2), encoding="utf-8")


if __name__ == "__main__":
    main()
