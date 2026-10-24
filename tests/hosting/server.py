"""A fake GitHub Enterprise and GitLab on loopback: three proposals, their checks and a review.

    server.py <port> <request-log> [git-root]

With a git root, the repositories under it are served too, over smart HTTP through
`git http-backend`, so the window can fetch a proposal's tip from the same host.

Serves the documented shapes Coral reads, on `github.localhost` or `gitlab.localhost`, which
resolve to loopback and which Coral takes for a self-hosted instance of each. Every request is logged with the start of its
Authorization or PRIVATE-TOKEN header, so the test can tell the token was sent.
"""
import json, os, socket, subprocess, sys
from http.server import BaseHTTPRequestHandler, HTTPServer

PORT, LOG = int(sys.argv[1]), sys.argv[2]
GIT_ROOT = sys.argv[3] if len(sys.argv) > 3 else None
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
def mr(iid, title, sha, branch, draft=False):
    return {"iid": iid, "title": title, "state": "opened", "draft": draft,
            "author": {"username": "ada"}, "source_branch": branch, "target_branch": "main",
            "web_url": f"http://gitlab.localhost:{PORT}/o/r/-/merge_requests/{iid}",
            "updated_at": "2026-10-20T10:00:00Z", "sha": sha}

MERGE_REQUESTS = [mr(12, "Speed up the lane walk", A, "fast-lanes"),
                  mr(11, "Teach blame to follow renames", B, "blame-renames"),
                  mr(10, "Draft: rework the palette", C, "palette", draft=True)]
# By merge request, as GitLab lists them: the newest first. 12's newest failed after an older
# success, so only the newest may count.
PIPELINES = {
    12: [{"id": 9, "status": "failed", "web_url": f"http://gitlab.localhost:{PORT}/o/r/-/pipelines/9"},
         {"id": 7, "status": "success"}],
    11: [{"id": 8, "status": "success"}],
    10: [{"id": 6, "status": "running"}],
}
RUNS = {
    A: [{"status": "completed", "conclusion": "success", "html_url": "http://ci/a1"},
        {"status": "completed", "conclusion": "failure", "html_url": f"http://github.localhost:{PORT}/o/r/runs/2"}],
    B: [{"status": "completed", "conclusion": "success"}, {"status": "completed", "conclusion": "success"},
        {"status": "completed", "conclusion": "skipped"}],
    C: [{"status": "in_progress", "conclusion": None, "html_url": "http://ci/c"}],
}

# The review on proposal 12, as each host keeps it. Replies are appended, so the window can be
# seen reading back what it said.
CONVERSATION = {12: [{"user": {"login": "bob"}, "body": "Does this keep the order stable?",
                      "created_at": "2026-10-20T09:00:00Z"}]}
LINE_COMMENTS = {12: [{"user": {"login": "cy"}, "body": "Off by one here?",
                       "created_at": "2026-10-20T09:30:00Z", "path": "src/lanes.rs",
                       "line": 42, "original_line": 42}]}
REVIEWS = {12: [{"user": {"login": "dee"}, "state": "CHANGES_REQUESTED", "body": "Needs a test",
                 "submitted_at": "2026-10-20T10:00:00Z"}]}
NOTES = {12: [{"author": {"username": "bob"}, "body": "Does this keep the order stable?",
               "created_at": "2026-10-20T09:00:00.000Z", "system": False},
              {"author": {"username": "cy"}, "body": "added 1 commit",
               "created_at": "2026-10-20T09:10:00.000Z", "system": True}]}
TICK = [0]

def now():
    TICK[0] += 1
    return f"2026-10-21T10:{TICK[0]:02d}:00Z"

def reply(path, sent):
    """Records a reply, answering with what the host would; None for a path that is not one."""
    parts = path.split("/")
    if path.startswith("/api/v3/repos/o/r/issues/") and path.endswith("/comments"):
        n = int(parts[-2])
        made = {"user": {"login": "you"}, "body": sent.get("body", ""), "created_at": now()}
        CONVERSATION.setdefault(n, []).append(made)
        return made
    if path.startswith("/api/v3/repos/o/r/pulls/") and path.endswith("/reviews"):
        n = int(parts[-2])
        state = {"APPROVE": "APPROVED", "REQUEST_CHANGES": "CHANGES_REQUESTED"}.get(
            sent.get("event"), "COMMENTED")
        made = {"user": {"login": "you"}, "state": state, "body": sent.get("body", ""),
                "submitted_at": now()}
        REVIEWS.setdefault(n, []).append(made)
        return made
    if path.startswith("/api/v4/projects/o%2Fr/merge_requests/") and path.endswith("/notes"):
        n = int(parts[-2])
        made = {"author": {"username": "you"}, "body": sent.get("body", ""),
                "created_at": now().replace("Z", ".000Z"), "system": False}
        NOTES.setdefault(n, []).append(made)
        return made
    if path.startswith("/api/v4/projects/o%2Fr/merge_requests/") and path.endswith("/approve"):
        n = int(parts[-2])
        NOTES.setdefault(n, []).append({"author": {"username": "you"},
                                        "body": "approved this merge request",
                                        "created_at": now().replace("Z", ".000Z"),
                                        "system": True})
        return {"approved": True}
    return None

class Handler(BaseHTTPRequestHandler):
    def git(self):
        """Hands the request to `git http-backend` and relays its CGI answer."""
        path, _, query = self.path.partition("?")
        length = int(self.headers.get("Content-Length") or 0)
        env = dict(os.environ, GIT_PROJECT_ROOT=GIT_ROOT, GIT_HTTP_EXPORT_ALL="1",
                   PATH_INFO=path, QUERY_STRING=query, REQUEST_METHOD=self.command,
                   CONTENT_TYPE=self.headers.get("Content-Type", ""),
                   CONTENT_LENGTH=str(length), REMOTE_ADDR="::1")
        if self.headers.get("Content-Encoding"):
            env["HTTP_CONTENT_ENCODING"] = self.headers["Content-Encoding"]
        if self.headers.get("Git-Protocol"):
            env["GIT_PROTOCOL"] = self.headers["Git-Protocol"]
        out = subprocess.run(["git", "http-backend"], input=self.rfile.read(length),
                             env=env, capture_output=True).stdout
        head, _, body = out.partition(b"\r\n\r\n")
        status = 200
        headers = []
        for line in head.decode().split("\r\n"):
            name, _, value = line.partition(": ")
            if name.lower() == "status":
                status = int(value.split()[0])
            elif name:
                headers.append((name, value))
        self.send_response(status)
        for name, value in headers:
            self.send_header(name, value)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self):
        if GIT_ROOT and ".git/" in self.path:
            return self.git()
        length = int(self.headers.get("Content-Length") or 0)
        sent = json.loads(self.rfile.read(length) or b"{}")
        with open(LOG, "a") as log:
            log.write(f"POST {self.path} auth={self.headers.get('Authorization', '')[:12]}"
                      f" private={self.headers.get('PRIVATE-TOKEN', '')[:5]} {json.dumps(sent)}\n")
        made = reply(self.path.split("?")[0], sent)
        if made is None:
            self.send_response(404); self.end_headers(); return
        data = json.dumps(made).encode()
        self.send_response(201)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        if GIT_ROOT and ".git/" in self.path:
            return self.git()
        with open(LOG, "a") as log:
            log.write(f"{self.path} auth={self.headers.get('Authorization', '')[:12]}"
                      f" private={self.headers.get('PRIVATE-TOKEN', '')[:5]}\n")
        path = self.path.split("?")[0]
        body = None
        if path == "/api/v3/repos/o/r/pulls":
            body = PULLS
        elif path == "/api/v4/projects/o%2Fr/merge_requests":
            body = MERGE_REQUESTS
        elif path.startswith("/api/v4/projects/o%2Fr/merge_requests/") and path.endswith("/pipelines"):
            body = PIPELINES.get(int(path.split("/")[-2]), [])
        elif path.endswith("/check-runs"):
            sha = path.split("/")[-2]
            body = {"total_count": len(RUNS.get(sha, [])), "check_runs": RUNS.get(sha, [])}
        elif path.endswith("/status"):
            body = {"state": "pending", "statuses": []}
        elif path.startswith("/api/v3/repos/o/r/issues/") and path.endswith("/comments"):
            body = CONVERSATION.get(int(path.split("/")[-2]), [])
        elif path.startswith("/api/v3/repos/o/r/pulls/") and path.endswith("/comments"):
            body = LINE_COMMENTS.get(int(path.split("/")[-2]), [])
        elif path.startswith("/api/v3/repos/o/r/pulls/") and path.endswith("/reviews"):
            body = REVIEWS.get(int(path.split("/")[-2]), [])
        elif path.startswith("/api/v4/projects/o%2Fr/merge_requests/") and path.endswith("/notes"):
            body = NOTES.get(int(path.split("/")[-2]), [])
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
