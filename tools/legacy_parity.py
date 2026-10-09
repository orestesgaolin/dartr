#!/usr/bin/env python3
"""Record a legacy analysis-server JSONL session.

This is an evidence tool for the Rust differential integration test. It runs
the Dart 3.13.3 analysis server or a dartr binary, sends the same dartdev and
IntelliJ request sequences, and writes every JSON message in protocol order.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import queue
import subprocess
import sys
import threading
import time


QUIET_SECONDS = 0.4
TIMEOUT_SECONDS = 60.0


class Client:
    def __init__(self, command: list[str]) -> None:
        self.process = subprocess.Popen(
            command,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            bufsize=1,
        )
        self.messages: queue.Queue[dict | None] = queue.Queue()
        self.log: list[dict] = []
        self.responses: dict[str, dict] = {}
        self.errors: dict[str, list[dict]] = {}
        self.analyzing = False
        self.analysis_started = 0
        self.analysis_completed = 0
        threading.Thread(target=self._read_stdout, daemon=True).start()

    def _read_stdout(self) -> None:
        assert self.process.stdout is not None
        for line in self.process.stdout:
            line = line.strip()
            if line:
                self.messages.put(json.loads(line))
        self.messages.put(None)

    def send(self, identifier: str, method: str, params: dict) -> None:
        message = {"id": identifier, "method": method, "params": params}
        assert self.process.stdin is not None
        self.process.stdin.write(json.dumps(message, separators=(",", ":")) + "\n")
        self.process.stdin.flush()

    def _handle(self, message: dict) -> None:
        self.log.append(message)
        identifier = message.get("id")
        if identifier is not None:
            self.responses[str(identifier)] = message
        if message.get("event") == "analysis.errors":
            params = message.get("params", {})
            self.errors[params.get("file", "")] = params.get("errors", [])
        if message.get("event") == "server.status":
            state = message.get("params", {}).get("analysis", {}).get("isAnalyzing")
            if state is True:
                self.analyzing = True
                self.analysis_started += 1
            elif state is False:
                self.analyzing = False
                self.analysis_completed += 1

    def wait_response(self, identifier: str) -> dict:
        deadline = time.monotonic() + TIMEOUT_SECONDS
        while identifier not in self.responses:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError(f"no response for {identifier}")
            message = self.messages.get(timeout=remaining)
            if message is None:
                raise RuntimeError(f"server exited before response {identifier}")
            self._handle(message)
        return self.responses[identifier]

    def settle(self, previous_completions: int, expect_analysis: bool = True) -> None:
        deadline = time.monotonic() + TIMEOUT_SECONDS
        quiet_deadline = time.monotonic() + QUIET_SECONDS
        while time.monotonic() < deadline:
            try:
                message = self.messages.get(timeout=max(0.0, quiet_deadline - time.monotonic()))
            except queue.Empty:
                if not self.analyzing and (
                    not expect_analysis or self.analysis_completed > previous_completions
                ):
                    return
                quiet_deadline = time.monotonic() + QUIET_SECONDS
                continue
            if message is None:
                raise RuntimeError("server exited while settling")
            self._handle(message)
            quiet_deadline = time.monotonic() + QUIET_SECONDS
        raise TimeoutError("analysis did not complete")

    def close(self) -> tuple[int, str]:
        assert self.process.stdin is not None
        self.process.stdin.close()
        code = self.process.wait(timeout=30)
        assert self.process.stderr is not None
        return code, self.process.stderr.read()


def command_for(server: str) -> list[str]:
    if server == "dart":
        dart = os.environ.get("DART_BIN", "dart")
        return [dart, "language-server", "--protocol=analyzer", "--client-id=legacy-parity"]
    return [server, "language-server", "--protocol=analyzer", "--client-id=legacy-parity"]


def run(client: Client, root: Path) -> None:
    source = root / "lib" / "main.dart"
    client.wait_response("connected") if False else None
    client.send("1", "server.setSubscriptions", {"subscriptions": ["STATUS"]})
    client.wait_response("1")
    complete = client.analysis_completed
    client.send(
        "2",
        "analysis.setAnalysisRoots",
        {"included": [str(root)], "excluded": [], "packageRoots": {}},
    )
    client.wait_response("2")
    client.settle(complete)
    client.send("3", "analysis.getErrors", {"file": str(source)})
    client.wait_response("3")
    client.send("4", "analysis.setSubscriptions", {"subscriptions": {"OUTLINE": [str(source)]}})
    client.wait_response("4")
    client.send("5", "analysis.setPriorityFiles", {"files": [str(source)]})
    client.wait_response("5")

    # Add an overlay with a supplementary-plane character before the error.
    overlay = "void main() {\n  print('😀');\n  print('broken';\n}\n"
    complete = client.analysis_completed
    client.send("6", "analysis.updateContent", {"files": {str(source): {"type": "add", "content": overlay}}})
    client.wait_response("6")
    client.settle(complete)

    # SourceEdit offsets and lengths are UTF-16 code units. Insert ')' after
    # the quote on the third line; the emoji contributes two code units.
    quote_offset = len("void main() {\n  print('") + 2 + len("');\n  print('broken'")
    complete = client.analysis_completed
    client.send("7", "analysis.updateContent", {"files": {str(source): {"type": "change", "edits": [{"offset": quote_offset, "length": 0, "replacement": ")"}]}}})
    client.wait_response("7")
    client.settle(complete)

    complete = client.analysis_completed
    client.send("8", "analysis.updateContent", {"files": {str(source): {"type": "remove"}}})
    client.wait_response("8")
    client.settle(complete)
    client.send("9", "not.aRealRequest", {})
    client.wait_response("9")
    client.send("10", "server.shutdown", {})
    client.wait_response("10")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--server", default="dart", help="'dart' or path to dartr")
    parser.add_argument("--root", type=Path, default=Path("crates/dartr/tests/legacy_fixtures/project"))
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    client = Client(command_for(args.server))
    run(client, args.root.resolve())
    code, stderr = client.close()
    text = "\n".join(json.dumps(message, sort_keys=True) for message in client.log) + "\n"
    if args.output:
        args.output.write_text(text, encoding="utf-8")
    else:
        sys.stdout.write(text)
    if stderr:
        sys.stderr.write(stderr)
    return code


if __name__ == "__main__":
    raise SystemExit(main())
