"""A fake GitHub Enterprise on loopback: three pull requests and the checks on each.

    server.py <port> <request-log>

Serves the documented shapes Coral reads, on `github.localhost`, which resolves to loopback and
which Coral takes for an Enterprise instance. Every request is logged with the start of its
Authorization header, so the test can tell the token was sent.
"""
import json, socket, sys
from http.server import BaseHTTPRequestHandler, HTTPServer

PORT, LOG = int(sys.argv[1]), sys.argv[2]
A, B, C = "a" * 40, "b" * 40, "c" * 40

def pr(n, title, sha, branch, draft=False):
    return {"number": n, "title": title, "state": "open", "draft": draft, "merged_at": None,
            "user": {"login": "ada"}, "head": {"ref": branch, "sha": sha},
            "base": {"ref": "main", "sha": "0" * 40},
            "html_url": f"http://github.localhost:{PORT}/o/r/pull/{n}",
            "updated_at": "2026-10-20T10:00:00Z"}

PULLS = [pr(12, "Speed up the lane walk", A, "fast-lanes"),
         pr(11, "Teach blame to follow renames", B, "blame-renames"),
         pr(10, "Draft: rework the palette", C, "palette", draft=True)]
RUNS = {
    A: [{"status": "completed", "conclusion": "success", "html_url": "http://ci/a1"},
        {"status": "completed", "conclusion": "failure", "html_url": f"http://github.localhost:{PORT}/o/r/runs/2"}],
    B: [{"status": "completed", "conclusion": "success"}, {"status": "completed", "conclusion": "success"},
        {"status": "completed", "conclusion": "skipped"}],
    C: [{"status": "in_progress", "conclusion": None, "html_url": "http://ci/c"}],
}

class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        with open(LOG, "a") as log:
            log.write(f"{self.path} auth={self.headers.get('Authorization', '')[:12]}\n")
        path = self.path.split("?")[0]
        body = None
        if path == "/api/v3/repos/o/r/pulls":
            body = PULLS
        elif path.endswith("/check-runs"):
            sha = path.split("/")[-2]
            body = {"total_count": len(RUNS.get(sha, [])), "check_runs": RUNS.get(sha, [])}
        elif path.endswith("/status"):
            body = {"state": "pending", "statuses": []}
        if body is None:
            self.send_response(404); self.end_headers(); return
        data = json.dumps(body).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)
    def log_message(self, *a):
        pass

class Loopback6(HTTPServer):
    address_family = socket.AF_INET6

Loopback6(("::1", PORT), Handler).serve_forever()
