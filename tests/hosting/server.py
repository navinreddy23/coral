"""A fake GitHub Enterprise, GitLab, Gitea, Bitbucket and Azure DevOps on loopback: three
proposals, their checks and a review, on each in its own shapes.

    server.py <port> <request-log> [git-root]

With a git root, the repositories under it are served too, over smart HTTP through
`git http-backend`, so the window can fetch a proposal's tip from the same host.

Serves the documented shapes Coral reads, on `<kind>.localhost`, which resolves to loopback.
GitHub, GitLab and Gitea are recognised by those names; Bitbucket and Azure DevOps are
declared for them in the window's hosts.json, as a self-hosted server is. Every request is logged with the start of its
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
def gitea_pr(n, title, sha, branch, draft=False):
    return {"number": n, "title": ("WIP: " if draft else "") + title, "state": "open",
            "merged": False, "user": {"login": "ada"},
            "head": {"ref": branch, "sha": sha}, "base": {"ref": "main", "sha": "0" * 40},
            "html_url": f"http://gitea.localhost:{PORT}/o/r/pulls/{n}",
            "updated_at": "2026-10-20T10:00:00Z"}

GITEA_PULLS = [gitea_pr(12, "Speed up the lane walk", A, "fast-lanes"),
               gitea_pr(11, "Teach blame to follow renames", B, "blame-renames"),
               gitea_pr(10, "Rework the palette", C, "palette", draft=True)]
GITEA_STATUS = {A: [{"status": "success"}, {"status": "failure",
                    "target_url": f"http://gitea.localhost:{PORT}/o/r/actions/runs/2"}],
                B: [{"status": "success"}], C: [{"status": "pending"}]}

def bb_pr(n, title, sha, branch, draft=False):
    return {"id": n, "title": title, "state": "OPEN", "draft": draft,
            "author": {"display_name": "Ada"},
            "source": {"branch": {"name": branch}, "commit": {"hash": sha[:12]}},
            "destination": {"branch": {"name": "main"}},
            "links": {"html": {"href": f"http://bitbucket.localhost:{PORT}/o/r/pull-requests/{n}"}},
            "updated_on": "2026-10-20T10:00:00+00:00", "participants": []}

BB_PULLS = [bb_pr(12, "Speed up the lane walk", A, "fast-lanes"),
            bb_pr(11, "Teach blame to follow renames", B, "blame-renames"),
            bb_pr(10, "Rework the palette", C, "palette", draft=True)]
BB_STATUS = {12: [{"state": "FAILED", "url": f"http://bitbucket.localhost:{PORT}/o/r/builds/9"}],
             11: [{"state": "SUCCESSFUL"}], 10: [{"state": "INPROGRESS"}]}
BB_COMMENTS = {12: [{"user": {"display_name": "Bob"}, "content": {"raw": "Does this keep the order stable?"},
                     "created_on": "2026-10-20T09:00:00+00:00"}]}

def az_pr(n, title, sha, branch, draft=False):
    return {"pullRequestId": n, "title": title, "status": "active", "isDraft": draft,
            "createdBy": {"displayName": "Ada"}, "sourceRefName": f"refs/heads/{branch}",
            "targetRefName": "refs/heads/main", "lastMergeSourceCommit": {"commitId": sha},
            "creationDate": "2026-10-20T10:00:00Z", "reviewers": []}

AZ_PULLS = [az_pr(12, "Speed up the lane walk", A, "fast-lanes"),
            az_pr(11, "Teach blame to follow renames", B, "blame-renames"),
            az_pr(10, "Rework the palette", C, "palette", draft=True)]
AZ_STATUS = {12: [{"state": "failed", "targetUrl": f"http://azure.localhost:{PORT}/o/p/_build/9"}],
             11: [{"state": "succeeded"}], 10: [{"state": "pending"}]}
AZ_THREADS = {12: [{"comments": [{"author": {"displayName": "Bob"}, "content": "Does this keep the order stable?",
                                  "publishedDate": "2026-10-20T09:00:00Z", "commentType": "text"}]}]}
GITEA_API, BB_API, AZ_API = "/api/v1/repos/o/r", "/2.0/repositories/o/r", "/o/p/_apis/git/repositories/r"

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

def host_repo():
    """Where the host keeps the repository: `o/r.git`, or Azure's `o/p/_git/r`."""
    azure = os.path.join(GIT_ROOT, "o", "p", "_git", "r")
    return azure if os.path.isdir(azure) else os.path.join(GIT_ROOT, "o", "r.git")


def branch_missing(path, sent):
    """The host's refusal of a proposal from a branch it does not have; None when it has it."""
    if path in ("/api/v3/repos/o/r/pulls", GITEA_API + "/pulls"):
        head = sent.get("head", "")
    elif path == "/api/v4/projects/o%2Fr/merge_requests":
        head = sent.get("source_branch", "")
    elif path == BB_API + "/pullrequests":
        head = sent.get("source", {}).get("branch", {}).get("name", "")
    elif path == AZ_API + "/pullrequests":
        head = sent.get("sourceRefName", "").removeprefix("refs/heads/")
    else:
        return None
    if GIT_ROOT is None:
        return None
    known = subprocess.run(["git", "--git-dir", host_repo(),
                            "rev-parse", "--verify", "--quiet", f"refs/heads/{head}"],
                           capture_output=True).returncode == 0
    if known:
        return None
    if "/api/v3/" in path:
        return {"message": "Validation Failed",
                "errors": [{"resource": "PullRequest", "field": "head", "code": "invalid"}]}
    if path.startswith(BB_API):
        return {"type": "error", "error": {"message": "source branch not found"}}
    return {"message": "404 Branch Not Found"}


def reply(path, sent):
    """Records a reply, answering with what the host would; None for a path that is not one."""
    parts = path.split("/")
    # A new proposal, numbered after the newest, and listed from then on.
    if path == "/api/v3/repos/o/r/pulls":
        made = pr(max(p["number"] for p in PULLS) + 1, sent.get("title", ""), "d" * 40,
                  sent.get("head", ""), draft=bool(sent.get("draft")))
        made["user"] = {"login": "you"}
        made["base"]["ref"] = sent.get("base", "main")
        PULLS.insert(0, made)
        return made
    if path == "/api/v4/projects/o%2Fr/merge_requests":
        made = mr(max(m["iid"] for m in MERGE_REQUESTS) + 1, sent.get("title", ""), "d" * 40,
                  sent.get("source_branch", ""))
        made["author"] = {"username": "you"}
        made["target_branch"] = sent.get("target_branch", "main")
        MERGE_REQUESTS.insert(0, made)
        return made
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
    if path == GITEA_API + "/pulls":
        made = gitea_pr(max(p["number"] for p in GITEA_PULLS) + 1, sent.get("title", ""), "d" * 40,
                        sent.get("head", ""))
        made["user"] = {"login": "you"}
        GITEA_PULLS.insert(0, made)
        return made
    if path == BB_API + "/pullrequests":
        made = bb_pr(max(p["id"] for p in BB_PULLS) + 1, sent.get("title", ""), "d" * 40,
                     sent["source"]["branch"]["name"], draft=bool(sent.get("draft")))
        made["author"] = {"display_name": "You"}
        BB_PULLS.insert(0, made)
        return made
    if path == AZ_API + "/pullrequests":
        made = az_pr(max(p["pullRequestId"] for p in AZ_PULLS) + 1, sent.get("title", ""), "d" * 40,
                     sent["sourceRefName"].removeprefix("refs/heads/"), draft=bool(sent.get("isDraft")))
        made["createdBy"] = {"displayName": "You"}
        AZ_PULLS.insert(0, made)
        return made
    if path.startswith(GITEA_API + "/issues/") and path.endswith("/comments"):
        made = {"user": {"login": "you"}, "body": sent.get("body", ""), "created_at": now()}
        CONVERSATION.setdefault(int(parts[-2]), []).append(made)
        return made
    if path.startswith(GITEA_API + "/pulls/") and path.endswith("/reviews"):
        made = {"user": {"login": "you"}, "state": sent.get("event", "COMMENT"),
                "body": sent.get("body", ""), "submitted_at": now()}
        REVIEWS.setdefault(int(parts[-2]), []).append(made)
        return made
    if path.startswith(BB_API + "/pullrequests/") and path.endswith("/comments"):
        made = {"user": {"display_name": "You"}, "content": {"raw": sent["content"]["raw"]},
                "created_on": now()}
        BB_COMMENTS.setdefault(int(parts[-2]), []).append(made)
        return made
    if path.startswith(BB_API + "/pullrequests/") and path.endswith(("/approve", "/request-changes")):
        n = int(parts[-2])
        for p in BB_PULLS:
            if p["id"] == n:
                p["participants"].append({"user": {"display_name": "You"},
                                          "approved": path.endswith("/approve"),
                                          "state": "approved" if path.endswith("/approve") else "changes_requested",
                                          "participated_on": now()})
        return {"approved": path.endswith("/approve")}
    if path.startswith(AZ_API + "/pullrequests/") and path.endswith("/threads"):
        made = {"comments": [{"author": {"displayName": "You"}, "content": sent["comments"][0]["content"],
                              "publishedDate": now(), "commentType": "text"}]}
        AZ_THREADS.setdefault(int(parts[-2]), []).append(made)
        return made
    if path.startswith("/api/v4/projects/o%2Fr/merge_requests/") and path.endswith("/approve"):
        n = int(parts[-2])
        NOTES.setdefault(n, []).append({"author": {"username": "you"},
                                        "body": "approved this merge request",
                                        "created_at": now().replace("Z", ".000Z"),
                                        "system": True})
        return {"approved": True}
    return None

def is_git(path):
    """Whether a request is git's own: smart HTTP, on either path shape."""
    return GIT_ROOT is not None and (".git/" in path or "/_git/" in path) and (
        "/info/refs" in path or "git-upload-pack" in path or "git-receive-pack" in path)


def who(headers):
    """The start of whatever credential came, Basic decoded, for the log."""
    auth = headers.get("Authorization", "")
    if auth.startswith("Basic "):
        import base64
        try:
            return "basic=" + base64.b64decode(auth[6:]).decode()[:16]
        except ValueError:
            return "basic=?"
    return f"auth={auth[:12]}"


class Handler(BaseHTTPRequestHandler):
    def git(self):
        """Hands the request to `git http-backend` and relays its CGI answer."""
        path, _, query = self.path.partition("?")
        # A push starts unauthenticated, and the host answers that with a challenge, which is
        # what makes git ask its credential helper for the token.
        pushing = "git-receive-pack" in path or "service=git-receive-pack" in query
        if pushing and not self.headers.get("Authorization"):
            self.send_response(401)
            self.send_header("WWW-Authenticate", 'Basic realm="fake"')
            self.send_header("Content-Length", "0")
            self.end_headers()
            return
        length = int(self.headers.get("Content-Length") or 0)
        env = dict(os.environ, GIT_PROJECT_ROOT=GIT_ROOT, GIT_HTTP_EXPORT_ALL="1",
                   PATH_INFO=path, QUERY_STRING=query, REQUEST_METHOD=self.command,
                   CONTENT_TYPE=self.headers.get("Content-Type", ""),
                   CONTENT_LENGTH=str(length), REMOTE_ADDR="::1")
        # http-backend takes a push only from a signed-in user, as the hosts do.
        if self.headers.get("Authorization"):
            env["REMOTE_USER"] = "you"
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

    def do_PUT(self):
        length = int(self.headers.get("Content-Length") or 0)
        sent = json.loads(self.rfile.read(length) or b"{}")
        with open(LOG, "a") as log:
            log.write(f"PUT {self.path} {who(self.headers)} {json.dumps(sent)}\n")
        path = self.path.split("?")[0]
        if path.startswith(AZ_API + "/pullrequests/") and "/reviewers/" in path:
            n = int(path.split("/")[-3])
            for p in AZ_PULLS:
                if p["pullRequestId"] == n:
                    p["reviewers"].append({"displayName": "You", "vote": sent.get("vote", 0)})
            return self.answer(200, {"vote": sent.get("vote", 0)})
        self.send_response(404); self.end_headers()

    def answer(self, status, body):
        data = json.dumps(body).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_POST(self):
        if is_git(self.path):
            return self.git()
        length = int(self.headers.get("Content-Length") or 0)
        sent = json.loads(self.rfile.read(length) or b"{}")
        with open(LOG, "a") as log:
            log.write(f"POST {self.path} {who(self.headers)}"
                      f" private={self.headers.get('PRIVATE-TOKEN', '')[:5]} {json.dumps(sent)}\n")
        refused = branch_missing(self.path.split("?")[0], sent)
        made = refused or reply(self.path.split("?")[0], sent)
        if made is None:
            self.send_response(404); self.end_headers(); return
        data = json.dumps(made).encode()
        # GitHub answers 422 for a head it does not have, GitLab 404.
        self.send_response(201 if refused is None else (422 if "/api/v3/" in self.path else 404))
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        if is_git(self.path):
            return self.git()
        with open(LOG, "a") as log:
            log.write(f"{self.path} {who(self.headers)}"
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
        elif path.startswith("/api/v3/") and path.endswith("/status"):
            body = {"state": "pending", "statuses": []}
        elif path.startswith("/api/v3/repos/o/r/issues/") and path.endswith("/comments"):
            body = CONVERSATION.get(int(path.split("/")[-2]), [])
        elif path.startswith("/api/v3/repos/o/r/pulls/") and path.endswith("/comments"):
            body = LINE_COMMENTS.get(int(path.split("/")[-2]), [])
        elif path.startswith("/api/v3/repos/o/r/pulls/") and path.endswith("/reviews"):
            body = REVIEWS.get(int(path.split("/")[-2]), [])
        elif path.startswith("/api/v4/projects/o%2Fr/merge_requests/") and path.endswith("/notes"):
            body = NOTES.get(int(path.split("/")[-2]), [])
        elif path == GITEA_API + "/pulls":
            body = GITEA_PULLS
        elif path.startswith(GITEA_API + "/commits/") and path.endswith("/status"):
            body = {"state": "pending", "statuses": GITEA_STATUS.get(path.split("/")[-2], [])}
        elif path.startswith(GITEA_API + "/issues/") and path.endswith("/comments"):
            body = CONVERSATION.get(int(path.split("/")[-2]), [])
        elif path.startswith(GITEA_API + "/pulls/") and path.endswith("/reviews"):
            body = [r if r["state"] != "CHANGES_REQUESTED" else {**r, "state": "REQUEST_CHANGES"}
                    for r in REVIEWS.get(int(path.split("/")[-2]), [])]
        elif path == BB_API + "/pullrequests":
            body = {"values": BB_PULLS, "pagelen": 50}
        elif path.startswith(BB_API + "/pullrequests/") and path.endswith("/statuses"):
            body = {"values": BB_STATUS.get(int(path.split("/")[-2]), [])}
        elif path.startswith(BB_API + "/pullrequests/") and path.endswith("/comments"):
            body = {"values": BB_COMMENTS.get(int(path.split("/")[-2]), [])}
        elif path.startswith(BB_API + "/pullrequests/"):
            n = int(path.split("/")[-1])
            body = next((p for p in BB_PULLS if p["id"] == n), None)
        elif path == AZ_API + "/pullrequests":
            body = {"count": len(AZ_PULLS), "value": AZ_PULLS}
        elif path.startswith(AZ_API + "/pullrequests/") and path.endswith("/statuses"):
            body = {"value": AZ_STATUS.get(int(path.split("/")[-2]), [])}
        elif path.startswith(AZ_API + "/pullrequests/") and path.endswith("/threads"):
            body = {"value": AZ_THREADS.get(int(path.split("/")[-2]), [])}
        elif path.startswith(AZ_API + "/pullrequests/"):
            n = int(path.split("/")[-1])
            body = next((p for p in AZ_PULLS if p["pullRequestId"] == n), None)
        elif path == "/o/_apis/connectionData":
            body = {"authenticatedUser": {"id": "you-0001"}}
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
