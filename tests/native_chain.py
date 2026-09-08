#!/usr/bin/env python3
"""Keyless regtest funding for native runtime tests. No production fixture injection."""
import json
import signal
import subprocess
import time
from pathlib import Path

root = Path(__file__).resolve().parent.parent
directory = root / "build/native-regtest"
directory.mkdir(parents=True, exist_ok=True)
(directory / "bitcoin.conf").write_text("[regtest]\nrpcport=19443\n")
node = subprocess.Popen(["bitcoind", f"-datadir={directory}", "-regtest", "-server",
                         "-disablewallet", "-listen=0", "-nosettings", "-persistmempool=0"],
                        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
server = None


def rpc(method, *args):
    out = subprocess.run(["bitcoin-cli", f"-datadir={directory}", "-regtest", "-rpcwait",
                          method, *map(str, args)], check=True, capture_output=True, text=True)
    return json.loads(out.stdout)


def address(fixture):
    descriptor = (root / "tests/fixtures" / fixture).read_text().strip().split("#")[0].replace("<0;1>", "0")
    descriptor = rpc("getdescriptorinfo", descriptor)["descriptor"]
    return rpc("deriveaddresses", descriptor, "[0,0]")[0]


def stop(*_):
    raise SystemExit(0)


signal.signal(signal.SIGTERM, stop)
try:
    rpc("getblockcount")
    rpc("generatetoaddress", "3", address("single-sig.txt"))
    rpc("generatetoaddress", "100", address("two-of-three.txt"))
    server = subprocess.Popen(["python3", str(root / "tests/esplora_regtest.py"),
                               "--datadir", str(directory), "--port", "3002",
                               "--port-file", str(root / "build/native-chain-port")])
    while server.poll() is None and node.poll() is None:
        time.sleep(1)
finally:
    for child in [server, node]:
        if child is not None:
            child.terminate()
            child.wait(timeout=15)
