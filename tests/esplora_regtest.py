#!/usr/bin/env python3
"""Test-only Esplora facade over an actual keyless Bitcoin Core regtest node.

No wallets or private keys are created. Every response comes from RPC block/tx data.
This is an integration harness, not a supported production chain server.
"""
import argparse
import hashlib
import http.server
import json
import threading
from pathlib import Path
from regtest_rpc import RegtestRPC

p = argparse.ArgumentParser()
p.add_argument("--datadir", required=True)
p.add_argument("--port-file", required=True)
p.add_argument("--port", type=int, default=0)
args = p.parse_args()


rpc = RegtestRPC(args.datadir)


def sha(value):
    return hashlib.sha256(value).digest()


class Index:
    def __init__(self):
        self.tip = None
        self.lock = threading.Lock()

    def refresh(self):
        tip = rpc("getbestblockhash")
        if tip == self.tip:
            return
        self.blocks, self.transactions, self.histories, self.scripts = {}, {}, {}, {}
        for height in range(rpc("getblockcount") + 1):
            block = rpc("getblock", rpc("getblockhash", height), 2)
            self.blocks[height] = block
            for tx in block["tx"]:
                txid = tx["txid"]
                status = {"confirmed": True, "block_height": height, "block_hash": block["hash"], "block_time": block["time"]}
                self.transactions[txid] = (tx, status)
                hashes = set()
                for vout in tx["vout"]:
                    script = bytes.fromhex(vout["scriptPubKey"]["hex"])
                    script_hash = sha(script).hex()
                    self.scripts[(txid, vout["n"])] = script_hash
                    hashes.add(script_hash)
                for vin in tx["vin"]:
                    if "txid" in vin:
                        script_hash = self.scripts.get((vin["txid"], vin["vout"]))
                        if script_hash:
                            hashes.add(script_hash)
                for script_hash in hashes:
                    self.histories.setdefault(script_hash, []).insert(0, {"txid": txid, "status": status})
        self.tip = tip

    def proof(self, txid):
        _, status = self.transactions[txid]
        ids = [t["txid"] for t in self.blocks[status["block_height"]]["tx"]]
        position = ids.index(txid)
        cursor = position
        layer = [bytes.fromhex(t)[::-1] for t in ids]
        branch = []
        while len(layer) > 1:
            if len(layer) % 2:
                layer.append(layer[-1])
            branch.append(layer[cursor ^ 1][::-1].hex())
            layer = [sha(sha(layer[i] + layer[i + 1])) for i in range(0, len(layer), 2)]
            cursor >>= 1
        return {"block_height": status["block_height"], "merkle": branch, "pos": position}


index = Index()


class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_GET(self):
        try:
            with index.lock:
                index.refresh()
                parts = self.path.strip("/").split("/")
                if parts == ["blocks", "tip", "height"]:
                    value = str(max(index.blocks))
                elif parts[0] == "block-height":
                    value = index.blocks[int(parts[1])]["hash"]
                elif parts[0] == "block" and parts[2] == "header":
                    value = rpc("getblockheader", parts[1], "false")
                elif parts[0] == "scripthash":
                    rows = index.histories.get(parts[1], [])
                    if len(parts) == 5:
                        rows = rows[next(i for i, row in enumerate(rows) if row["txid"] == parts[4]) + 1:]
                    value = rows[:25]
                elif parts[0] == "tx" and parts[2] == "raw":
                    value = bytes.fromhex(index.transactions[parts[1]][0]["hex"])
                elif parts[0] == "tx" and parts[2] == "merkle-proof":
                    value = index.proof(parts[1])
                else:
                    self.send_error(404)
                    return
            payload = value if isinstance(value, bytes) else (value if isinstance(value, str) else json.dumps(value)).encode()
            self.send_response(200)
            self.send_header("Content-Length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)
        except (KeyError, StopIteration, ValueError):
            self.send_error(500)


server = http.server.ThreadingHTTPServer(("127.0.0.1", args.port), Handler)
# Readiness includes indexing the actual chain. The first mobile request must not
# build the entire block index inside the app's 15-second HTTP deadline.
# Slow hosted macOS runners must not turn an unready test backend into an app failure.
with index.lock:
    index.refresh()
# Readers test for file existence; publish the complete port atomically so they can
# never see an empty file and accidentally construct an endpoint using default port 80.
ready = Path(args.port_file)
pending = ready.with_name(ready.name + ".tmp")
pending.write_text(str(server.server_port))
pending.replace(ready)
server.serve_forever()
