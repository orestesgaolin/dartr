#!/usr/bin/env python3
"""Writes Mermaid charts of the benchmark results into docs/BENCHMARKS.md.

    python3 -I bench/charts.py

Reads bench/results/ (written by bench/compare.sh, bench/memory.sh and bench/lsp_bench.py; git-ignored):
  compare-<corpus>.json, compare-<corpus>-cold.json   dart (warm, empty cache) vs dartr analyze
  memory-<corpus>.json                                peak RSS of dart (warm) vs dartr analyze
  threads-<corpus>.json                               dartr with 1, 4 and all threads
  lsp2-<project>-<server>.json                        language server benchmark
and replaces the text between the markers <!-- charts:begin --> and <!-- charts:end -->.
A chart is left out when its input files are missing.
"""
import json
import os
import statistics

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RESULTS = os.path.join(ROOT, "bench", "results")
DOC = os.path.join(ROOT, "docs", "BENCHMARKS.md")
BEGIN, END = "<!-- charts:begin -->", "<!-- charts:end -->"

CORPORA = [("visible-app", "app"), ("flutter", "flutter"), ("analyzer-9.0.0", "analyzer"),
           ("flutter_tools", "flutter_tools")]
LSP_PROJECTS = [("app", "app"), ("flutter", "flutter"), ("analyzer", "analyzer"),
                ("flutter_tools", "flutter_tools")]
# dartr analyze wall time over the port (seconds), from the measurements in this file.
HISTORY = [
    ("10-10 19:45 (busy)", {"app": 24.945, "flutter": 7.380, "analyzer": 1.424, "flutter_tools": 3.665}),
    ("10-11 03:00 (quiet)", {"app": 17.919, "flutter": 2.975, "analyzer": 0.856, "flutter_tools": 0.805}),
    ("10-11 13:00 (busy)", {"app": 22.007, "flutter": 1.752, "analyzer": 0.840, "flutter_tools": 0.871}),
]
COLORS = ["#0175C2", "#9CC8EA", "#DEA584", "#8A4F2A", "#C0602A"]


def load(name):
    try:
        with open(os.path.join(RESULTS, name)) as f:
            return json.load(f)
    except (OSError, ValueError):
        return None


def num(v):
    return f"{v:.3g}" if v < 10 else f"{v:.1f}"


def chart(title, axis, labels, series, maximum):
    """A horizontal bar chart; `series` is a list of value lists, one bar per (label, series)
    pair, drawn as separate series (-100 = no bar) so each series has its own color."""
    xs, bars = [], [[] for _ in series]
    for i, label in enumerate(labels):
        for k, values in enumerate(series):
            if values[i] is None:
                continue
            xs.append(f'"{label[k]}"')
            for j in range(len(series)):
                bars[j].append(num(values[i]) if j == k else "-100")
    palette = ", ".join(COLORS[: len(series)])
    lines = [
        "```mermaid",
        "---",
        "config:",
        "  xyChart:",
        f"    height: {max(240, 34 * len(xs))}",
        "  themeVariables:",
        "    xyChart:",
        f'      plotColorPalette: "{palette}"',
        "---",
        "xychart-beta horizontal",
        "    %% -100 = no bar: one series per measurement gives each its own color",
        f'    title "{title}"',
        f"    x-axis [{', '.join(xs)}]",
        f'    y-axis "{axis}" 0 --> {num(maximum)}',
    ]
    lines += [f"    bar [{', '.join(b)}]" for b in bars]
    lines.append("```")
    return "\n".join(lines)


def mean_of(data, command):
    for r in (data or {}).get("results", []):
        if r["command"] == command:
            return r["mean"]
    return None


def median_gb_of(data, command):
    for r in (data or {}).get("results", []):
        if r["command"] == command:
            if "median_gb" in r:
                return r["median_gb"]
            if "median_bytes" in r:
                return r["median_bytes"] / (1024 ** 3)
    return None


def analyze_charts():
    out = []
    rows = []
    for corpus, short in CORPORA:
        warm, cold = load(f"compare-{corpus}.json"), load(f"compare-{corpus}-cold.json")
        if not warm:
            continue
        rows.append((short, mean_of(warm, "dart"), mean_of(cold, "dart (empty cache)"),
                     mean_of(warm, "dartr")))
    for group, names in (("Large projects", ("app", "flutter")),
                         ("Smaller packages", ("analyzer", "flutter_tools"))):
        sel = [r for r in rows if r[0] in names]
        if not sel:
            continue
        labels = [(f"{s}, dart warm", f"{s}, dart empty", f"{s}, dartr") for s, *_ in sel]
        series = [[r[1] for r in sel], [r[2] for r in sel], [r[3] for r in sel]]
        top = max(v for s in series for v in s if v is not None) * 1.1
        out.append(chart(f"dartr analyze vs dart analyze, {group.lower()}: wall time (s)",
                         "seconds", labels, series, top))
    mem = []
    for corpus, short in CORPORA:
        m = load(f"memory-{corpus}.json")
        if m:
            mem.append((short, median_gb_of(m, "dart"), median_gb_of(m, "dartr")))
    mem = [r for r in mem if r[1] is not None and r[2] is not None]
    if mem:
        labels = [(f"{s}, dart", f"{s}, dartr") for s, _, _ in mem]
        series = [[r[1] for r in mem], [r[2] for r in mem]]
        top = max(v for s in series for v in s) * 1.1
        out.append(chart("Peak memory of dartr analyze vs dart analyze (GB)", "GB", labels,
                         series, top).replace(
            f'plotColorPalette: "{COLORS[0]}, {COLORS[1]}"',
            f'plotColorPalette: "{COLORS[0]}, {COLORS[2]}"'))
    if out:
        out.insert(0, "### `dartr analyze` against `dart analyze`\n\nDark blue: dart, warm cache. "
                      "Light blue: dart, empty cache. Orange: dartr. Source: `bench/compare.sh`, "
                      "`bench/memory.sh`.")
    return out


def thread_charts():
    rows = []
    for corpus, short in CORPORA:
        d = load(f"threads-{corpus}.json")
        if not d:
            continue
        rows.append((short, [mean_of(d, c) for c in ("dartr 1 thread", "dartr 4 threads",
                                                    "dartr all cores")]))
    if not rows:
        return []
    out = ["### Thread scaling of `dartr analyze`\n\nBlue: 1 thread. Light blue: 4 threads. "
           "Orange: all cores. Measured on a busy machine (see the thread-scaling section)."]
    for short, values in rows:
        labels = [(f"{short}, 1 thread", f"{short}, 4 threads", f"{short}, all cores")]
        out.append(chart(f"{short}: wall time by thread count (s)", "seconds", labels,
                         [[values[0]], [values[1]], [values[2]]], max(values) * 1.1))
    return out


def lsp(project, server):
    d = load(f"lsp2-{project}-{server}.json")
    if not d:
        return None
    full = d["full"][0]
    req = full["requests"]

    def med(key):
        return statistics.median(req[key]["times_ms"])

    cold = [x["startup"]["ready_ms"] / 1000 for x in d.get("cold", [])]
    warm = [x["startup"]["ready_ms"] / 1000 for x in d.get("warm", [])]
    return {
        "cold": statistics.median(cold) if cold else None,
        "warm": statistics.median(warm) if warm else None,
        "hover": med("hover"), "dot": med("completion_dot"), "stmt": med("completion_stmt"),
        "refs": med("references"), "refs_first": req["references"]["first_ms"],
        "rss": full["startup"]["rss_mb"] / 1024,
    }


def lsp_charts():
    data = [(short, lsp(p, "dart"), lsp(p, "dartr")) for p, short in LSP_PROJECTS]
    data = [d for d in data if d[1] and d[2]]
    if not data:
        return []
    out = ["### Language server against `dart language-server`\n\nBlue: dart. Orange: dartr. "
           "Source: `bench/lsp_bench.py`. Startup: dark blue dart cold, light blue dart warm, "
           "orange dartr (no disk cache, so cold and warm are the same)."]
    for group, names in (("large projects", ("app", "flutter")),
                         ("smaller packages", ("analyzer", "flutter_tools"))):
        sel = [x for x in data if x[0] in names]
        if not sel:
            continue
        labels = [(f"{s}, dart cold", f"{s}, dart warm", f"{s}, dartr") for s, _, _ in sel]
        series = [[d["cold"] for _, d, _ in sel], [d["warm"] for _, d, _ in sel],
                  [r["cold"] for _, _, r in sel]]
        top = max(v for s in series for v in s if v is not None) * 1.1
        out.append(chart(f"Language server startup, {group} (s)", "seconds", labels, series, top))
    for key, title in (("dot", "Completion after a dot, median (ms)"),
                       ("stmt", "Completion at statement start, median (ms)"),
                       ("refs", "References, median of repeated requests (ms)"),
                       ("refs_first", "First references request (ms)"),
                       ("rss", "Memory after startup (GB)")):
        labels = [(f"{s}, dart", f"{s}, dartr") for s, _, _ in data]
        series = [[d[key] for _, d, _ in data], [r[key] for _, _, r in data]]
        top = max(v for s in series for v in s) * 1.1
        unit = "GB" if key == "rss" else "ms"
        out.append(chart(title, unit, labels, [series[0], series[1]], top).replace(
            f'plotColorPalette: "{COLORS[0]}, {COLORS[1]}"',
            f'plotColorPalette: "{COLORS[0]}, {COLORS[2]}"'))
    return out


def history_chart():
    labels = [tuple(f"{name}, {when.split(' (')[0]}" for when, _ in HISTORY)
              for _, name in CORPORA]
    series = [[values[name] for _, name in CORPORA] for _, values in HISTORY]
    top = max(v for s in series for v in s) * 1.1
    return ["### `dartr analyze` over time\n\nOne bar per measurement, oldest first: 10-10 19:45 "
            "(busy machine), 10-11 03:00 (quiet machine), 10-11 13:00 (busy machine, after CPU round 5).",
            chart("dartr analyze wall time over the port (s)", "seconds", labels, series, top)
            .replace(f'plotColorPalette: "{COLORS[0]}, {COLORS[1]}, {COLORS[2]}"',
                     f'plotColorPalette: "{COLORS[2]}, {COLORS[3]}, {COLORS[4]}"')]


def main():
    parts = ["## Charts", "Generated by `python3 -I bench/charts.py` from `bench/results/`. "
             "The tables below hold the exact numbers and the method of each measurement."]
    for section in (analyze_charts(), lsp_charts(), history_chart(), thread_charts()):
        parts += section
    block = BEGIN + "\n\n" + "\n\n".join(parts) + "\n\n" + END
    with open(DOC) as f:
        doc = f.read()
    if BEGIN in doc and END in doc:
        doc = doc[: doc.index(BEGIN)] + block + doc[doc.index(END) + len(END):]
    else:
        title_end = doc.index("\n", doc.index("# ")) + 1
        intro_end = doc.find("\n## ", title_end)
        intro_end = len(doc) if intro_end < 0 else intro_end + 1
        doc = doc[:intro_end] + "\n" + block + "\n\n" + doc[intro_end:]
    with open(DOC, "w") as f:
        f.write(doc)
    print(f"wrote {sum(p.count('```mermaid') for p in parts)} charts to {DOC}")


if __name__ == "__main__":
    main()
