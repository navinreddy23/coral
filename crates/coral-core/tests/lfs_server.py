"""A Git LFS lock server on loopback, for the locking tests.

    lfs_server.py <port-file>

Speaks the File Locking API git-lfs uses (docs/api/locking.md in git-lfs): create, list,
verify and unlock. Locks live in memory. Every lock is owned by "Fixture User" unless the
request carries an `X-Lock-Owner` header, which is how a test plays a second person. Binds an
ephemeral port and writes it to the given file, so parallel tests do not collide.
"""

import json
import sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse

LOCKS = {}
NEXT = [1]
ME = "Fixture User"


def lock_json(lock):
    return {"id": lock["id"], "path": lock["path"], "locked_at": "2026-10-24T09:00:00Z",
            "owner": {"name": lock["owner"]}}


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def reply(self, code, body):
        data = json.dumps(body).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/vnd.git-lfs+json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def body(self):
        length = int(self.headers.get("Content-Length") or 0)
        return json.loads(self.rfile.read(length) or b"{}")

    def do_GET(self):
        url = urlparse(self.path)
        if not url.path.endswith("/locks"):
            return self.reply(404, {"message": "not found"})
        query = parse_qs(url.query)
        locks = list(LOCKS.values())
        if "path" in query:
            locks = [l for l in locks if l["path"] == query["path"][0]]
        self.reply(200, {"locks": [lock_json(l) for l in locks]})

    def do_POST(self):
        url = urlparse(self.path).path
        body = self.body()
        owner = self.headers.get("X-Lock-Owner", ME)
        if url.endswith("/locks/verify"):
            ours = [lock_json(l) for l in LOCKS.values() if l["owner"] == owner]
            theirs = [lock_json(l) for l in LOCKS.values() if l["owner"] != owner]
            return self.reply(200, {"ours": ours, "theirs": theirs})
        if url.endswith("/locks"):
            path = body.get("path", "")
            held = next((l for l in LOCKS.values() if l["path"] == path), None)
            if held is not None:
                return self.reply(409, {"lock": lock_json(held), "message": "already created lock"})
            lock = {"id": str(NEXT[0]), "path": path, "owner": owner}
            NEXT[0] += 1
            LOCKS[lock["id"]] = lock
            return self.reply(201, {"lock": lock_json(lock)})
        if url.endswith("/unlock"):
            lock_id = url.rstrip("/").split("/")[-2]
            lock = LOCKS.get(lock_id)
            if lock is None:
                return self.reply(404, {"message": "unable to find lock"})
            if lock["owner"] != owner and not body.get("force"):
                return self.reply(403, {"message": f"lock is owned by {lock['owner']}"})
            del LOCKS[lock_id]
            return self.reply(200, {"lock": lock_json(lock)})
        self.reply(404, {"message": "not found"})


def main():
    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    with open(sys.argv[1], "w") as f:
        f.write(str(server.server_address[1]))
    server.serve_forever()


if __name__ == "__main__":
    main()
