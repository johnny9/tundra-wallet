"""Bounded, cookie-authenticated loopback RPC for the disposable keyless test node.

The caller serializes access. Reusing HTTP avoids a bitcoin-cli process launch for
every Esplora read on slow simulator hosts. No signing keys or remote RPC endpoints.
"""
import base64
import configparser
import http.client
import json
from pathlib import Path


class RegtestRPC:
    def __init__(self, directory):
        directory = Path(directory)
        config = configparser.ConfigParser()
        config.read(directory / "bitcoin.conf", encoding="utf-8")
        self.port = config.getint("regtest", "rpcport")
        if not 1 <= self.port <= 65535:
            raise ValueError("Invalid disposable regtest RPC port")
        cookie = (directory / "regtest/.cookie").read_bytes().strip()
        if not cookie or len(cookie) > 1024 or b":" not in cookie:
            raise ValueError("Invalid disposable regtest RPC cookie")
        self.authorization = "Basic " + base64.b64encode(cookie).decode("ascii")
        self.connection = None
        self.next_id = 0

    def __call__(self, method, *values):
        if method not in {"getbestblockhash", "getblockcount", "getblock", "getblockhash", "getblockheader"}:
            raise ValueError("RPC method is outside the read-only test fixture")
        self.next_id += 1
        # bitcoin-cli previously converted this one textual boolean for raw headers.
        params = [False if value == "false" else value for value in values]
        body = json.dumps({"jsonrpc": "2.0", "id": self.next_id, "method": method, "params": params})
        for attempt in range(2):
            try:
                if self.connection is None:
                    self.connection = http.client.HTTPConnection("127.0.0.1", self.port, timeout=15)
                self.connection.request("POST", "/", body, {"Authorization": self.authorization,
                                                            "Content-Type": "application/json"})
                response = self.connection.getresponse()
                payload = response.read(16 * 1024 * 1024 + 1)
                if response.status != 200 or len(payload) > 16 * 1024 * 1024:
                    raise ValueError("Disposable regtest RPC response failed")
                decoded = json.loads(payload)
                if decoded.get("id") != self.next_id or decoded.get("error") is not None or "result" not in decoded:
                    raise ValueError("Disposable regtest RPC result failed")
                return decoded["result"]
            except (OSError, http.client.HTTPException):
                if self.connection is not None:
                    self.connection.close()
                self.connection = None
                if attempt:
                    raise ValueError("Disposable regtest RPC connection failed") from None
