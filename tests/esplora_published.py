#!/usr/bin/env python3
"""Local public-signature UI fixture server. NOT Bitcoin Core or a real Signet chain.

Serves the published Ledger prevouts in one synthetic Merkle-consistent block. It
accepts only the exact already-published signed transaction and reports a synthetic
mempool observation. No signing keys, mining, upstream connections or broadcasts.
"""
import argparse
import hashlib
import http.server
import json
import socketserver
import threading
from pathlib import Path


def double_sha(value):
    return hashlib.sha256(hashlib.sha256(value).digest()).digest()


class PublishedIndex:
    def __init__(self, fixture):
        self.fixture = fixture
        self.lock = threading.Lock()
        self.posts = 0
        self.final = fixture["final"]
        self.block = fixture["block"]
        self.transactions = {row["txid"]: row for row in self.block["transactions"]}
        self.scripts = {}
        for txid, row in self.transactions.items():
            for n, output in enumerate(row["outputs"]):
                self.scripts[f"{txid}:{n}"] = output["scripthash"]
        self.validate_header(fixture["genesis"])
        self.validate_header(self.block)

    @staticmethod
    def validate_header(block):
        raw = bytes.fromhex(block["header"])
        if len(raw) != 80 or double_sha(raw)[::-1].hex() != block["hash"]:
            raise ValueError("Public fixture header hash mismatch")

    def status(self, txid):
        if txid == self.final["txid"] and self.posts:
            return {"confirmed": False}
        if txid not in self.transactions:
            raise KeyError(txid)
        return {"confirmed": True, "block_height": 1,
                "block_hash": self.block["hash"], "block_time": self.block["time"]}

    def history(self, script):
        rows = list(self.transactions.values())
        if self.posts:
            rows.insert(0, self.final)
        return [{"txid": row["txid"], "status": self.status(row["txid"])} for row in rows
                if script in ({output["scripthash"] for output in row["outputs"]}
                              | {self.scripts.get(outpoint) for outpoint in row["inputs"]})]

    def proof(self, txid):
        ids = list(self.transactions)
        position = ids.index(txid)
        cursor = position
        layer = [bytes.fromhex(value)[::-1] for value in ids]
        branch = []
        while len(layer) > 1:
            if len(layer) % 2:
                layer.append(layer[-1])
            branch.append(layer[cursor ^ 1][::-1].hex())
            layer = [double_sha(layer[n] + layer[n + 1]) for n in range(0, len(layer), 2)]
            cursor >>= 1
        if layer[0] != bytes.fromhex(self.block["header"])[36:68]:
            raise ValueError("Public fixture Merkle root mismatch")
        return {"block_height": 1, "pos": position, "merkle": branch}

    def get(self, path):
        parts = path.strip("/").split("/")
        if parts == ["_fixture_state"]:
            return {"posts": self.posts}  # Bounded non-wallet diagnostics for the test.
        if parts == ["blocks", "tip", "height"]:
            return "1"
        if parts == ["block-height", "0"]:
            return self.fixture["genesis"]["hash"]
        if parts == ["block-height", "1"]:
            return self.block["hash"]
        if len(parts) == 3 and parts[0] == "block" and parts[2] == "header":
            for block in [self.fixture["genesis"], self.block]:
                if parts[1] == block["hash"]:
                    return block["header"]
        if len(parts) == 3 and parts[0] == "scripthash" and parts[2] == "txs":
            return self.history(parts[1])
        if len(parts) == 3 and parts[0] == "tx":
            if parts[2] == "raw":
                row = self.final if parts[1] == self.final["txid"] and self.posts else self.transactions[parts[1]]
                return bytes.fromhex(row["raw"])
            if parts[2] == "merkle-proof":
                return self.proof(parts[1])
        raise KeyError(path)

    def submit(self, body):
        if body != self.final["raw"].encode("ascii"):
            raise ValueError("Only the exact public fixture transaction is accepted")
        self.posts += 1
        return self.final["txid"]


def server(fixture, port=0):
    index = PublishedIndex(fixture)

    class Handler(http.server.BaseHTTPRequestHandler):
        def log_message(self, *args):
            pass

        def respond(self, value, status=200):
            payload = value if isinstance(value, bytes) else (value if isinstance(value, str) else json.dumps(value)).encode()
            self.send_response(status)
            self.send_header("Content-Length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)

        def do_GET(self):
            try:
                with index.lock:
                    value = index.get(self.path)
                self.respond(value)
            except (KeyError, ValueError):
                self.respond("Public fixture request refused", 404)

        def do_POST(self):
            try:
                count = int(self.headers.get("Content-Length", "0"))
                if self.path != "/tx" or count != len(index.final["raw"]):
                    raise ValueError()
                self.connection.settimeout(5)
                body = self.rfile.read(count)
                with index.lock:
                    txid = index.submit(body)
                self.respond(txid)
            except (ValueError, TimeoutError):
                self.respond("Public fixture submission refused", 400)

    class LoopbackServer(http.server.ThreadingHTTPServer):
        # Accept the scanner's concurrent local connections even while a simulator
        # host is busy. Wallet connection/request timeouts remain production values.
        request_queue_size = 64

        def server_bind(self):
            # HTTPServer normally calls getfqdn here. A local test endpoint needs
            # no reverse DNS, which can stall for tens of seconds on macOS hosts.
            socketserver.TCPServer.server_bind(self)
            self.server_name = "localhost"
            self.server_port = self.server_address[1]

    host = LoopbackServer(("127.0.0.1", port), Handler)
    host.fixture_index = index
    return host


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixture", type=Path, default=Path(__file__).parent / "fixtures/native-signing.json")
    parser.add_argument("--port", type=int, default=3003)
    parser.add_argument("--port-file", type=Path, required=True)
    args = parser.parse_args()
    print("Loading public fixture and binding loopback", flush=True)
    host = server(json.loads(args.fixture.read_text()), args.port)
    temporary = args.port_file.with_suffix(".tmp")
    temporary.write_text(str(host.server_port))
    temporary.replace(args.port_file)
    print("Public fixture server ready", flush=True)
    host.serve_forever()
