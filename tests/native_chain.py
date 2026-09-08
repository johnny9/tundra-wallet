#!/usr/bin/env python3
"""Keyless regtest funding for native runtime tests. No production fixture injection."""
import argparse
import json
import signal
import socket
import subprocess
import tempfile
import time
from pathlib import Path

root = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--port", type=int, default=3002)
parser.add_argument("--port-file", type=Path, default=root / "build/native-chain-port")
args = parser.parse_args()
# Refuse to disturb a running test chain or leave a stale ready signal on failure.
with socket.socket() as probe:
    probe.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    probe.bind(("127.0.0.1", args.port))
args.port_file.unlink(missing_ok=True)
(root / "build").mkdir(exist_ok=True)
temporary = tempfile.TemporaryDirectory(prefix="native-regtest-", dir=root / "build")
directory = Path(temporary.name)
with socket.socket() as probe:
    probe.bind(("127.0.0.1", 0))
    rpc_port = probe.getsockname()[1]
(directory / "bitcoin.conf").write_text(f"[regtest]\nrpcport={rpc_port}\n")
node = server = None


def rpc(method, *args):
    out = subprocess.run(["bitcoin-cli", f"-datadir={directory}", "-regtest",
                          method, *map(str, args)], check=True, capture_output=True,
                         text=True, timeout=15)
    return json.loads(out.stdout)


def address(fixture):
    descriptor = (root / "tests/fixtures" / fixture).read_text().strip().split("#")[0].replace("<0;1>", "0")
    descriptor = rpc("getdescriptorinfo", descriptor)["descriptor"]
    return rpc("deriveaddresses", descriptor, "[0,0]")[0]


def stop(*_):
    raise SystemExit(0)


signal.signal(signal.SIGTERM, stop)
try:
    node = subprocess.Popen(["bitcoind", f"-datadir={directory}", "-regtest", "-server",
                             "-disablewallet", "-listen=0", "-nosettings", "-persistmempool=0"],
                            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    deadline = time.monotonic() + 30
    while True:
        if node.poll() is not None or time.monotonic() > deadline:
            raise RuntimeError("Disposable Bitcoin Core node did not become ready")
        try:
            rpc("getblockcount")
            break
        except subprocess.CalledProcessError:
            time.sleep(0.1)
    rpc("generatetoaddress", "3", address("single-sig.txt"))
    rpc("generatetoaddress", "100", address("two-of-three.txt"))
    server = subprocess.Popen(["python3", str(root / "tests/esplora_regtest.py"),
                               "--datadir", str(directory), "--port", str(args.port),
                               "--port-file", str(args.port_file)])
    while server.poll() is None and node.poll() is None:
        time.sleep(1)
    raise RuntimeError("A native test-chain process exited unexpectedly")
finally:
    for child in [server, node]:
        if child is not None:
            if child.poll() is None:
                child.terminate()
                try:
                    child.wait(timeout=15)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.wait()
    args.port_file.unlink(missing_ok=True)
    temporary.cleanup()
