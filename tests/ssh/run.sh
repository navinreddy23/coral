#!/usr/bin/env bash
#
# Does a pinned ssh key actually decide which account git authenticates as?
#
# The engine's own tests answer that by asking ssh to resolve its identity list, which needs
# no server. This answers it the other way round, by standing a server up and seeing which
# account arrives — because the failure being guarded against is not a wrong identity list, it
# is a clone that succeeds as the wrong person and reports the repository as missing.
#
# Everything here is generated and thrown away: two dummy keys, a host key, an sshd on a high
# port, an ssh-agent of its own, and two bare repositories on this machine. No key, host or
# repository belonging to anybody is touched, and the agent is a separate process so the one
# the user is running is never added to.
set -euo pipefail

say() { printf '%s\n' "$*"; }
fail() { printf 'FAIL: %s\n' "$*" >&2; exit 1; }

for tool in sshd ssh ssh-keygen ssh-agent ssh-add git; do
    command -v "$tool" >/dev/null 2>&1 || [ -x "/usr/sbin/$tool" ] \
        || { say "skipped: no $tool on this machine"; exit 0; }
done
SSHD=$(command -v sshd || echo /usr/sbin/sshd)

DIR=$(mktemp -d)
AGENT_PID=""
SSHD_PID=""
EXTRA_PIDS=""
cleanup() {
    [ -n "$SSHD_PID" ] && kill "$SSHD_PID" 2>/dev/null || true
    [ -n "$AGENT_PID" ] && kill "$AGENT_PID" 2>/dev/null || true
    for pid in $EXTRA_PIDS; do kill "$pid" 2>/dev/null || true; done
    rm -rf "$DIR"
}
trap cleanup EXIT

# ---------------------------------------------------------------- keys and repositories
mkdir -p "$DIR/keys" "$DIR/srv/alice" "$DIR/srv/bob"
for who in host alice bob; do
    ssh-keygen -q -t ed25519 -N '' -C "coral-fixture-$who" -f "$DIR/keys/$who"
done
chmod 600 "$DIR/keys/"*

# Only bob has the project. Alice reaching for it is the failure being reproduced: the clone
# authenticates perfectly well and is then told the repository does not exist.
git init -q --bare -b main "$DIR/srv/bob/project.git"
work="$DIR/work"
git init -q "$work"
git -C "$work" config user.email fixture@coral.test
git -C "$work" config user.name Fixture
echo hello > "$work/readme.md"
git -C "$work" add -A
git -C "$work" -c commit.gpgsign=false commit -qm "the commit only bob can see"
git -C "$work" push -q "$DIR/srv/bob/project.git" HEAD:refs/heads/main

# ------------------------------------------------------------------------------- the host
# One shell account, many git accounts, told apart by which key opened the connection. This is
# how a hosting provider does it, and it is what makes "the wrong key" show up as "no such
# repository" rather than as an authentication error.
cat > "$DIR/shim" <<'SHIM'
#!/usr/bin/env bash
set -euo pipefail
account="$1"
root="$(dirname "$0")/srv/$account"
verb=${SSH_ORIGINAL_COMMAND%% *}
path=${SSH_ORIGINAL_COMMAND#* }
path=${path//\'/}
# Which account each connection arrived as, for the sections that follow the key through every
# stage. One line per connection.
echo "$account $verb" >> "$(dirname "$0")/seen"
if [ ! -d "$root/$path" ]; then
    echo "ERROR: The project you were looking for could not be found." >&2
    exit 128
fi
exec "$verb" "$root/$path"
SHIM
chmod +x "$DIR/shim"

{
    printf 'command="%s alice",no-pty,no-agent-forwarding %s\n' "$DIR/shim" "$(cat "$DIR/keys/alice.pub")"
    printf 'command="%s bob",no-pty,no-agent-forwarding %s\n' "$DIR/shim" "$(cat "$DIR/keys/bob.pub")"
} > "$DIR/authorized_keys"
chmod 600 "$DIR/authorized_keys"

PORT=$(python3 -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1]);s.close()')
cat > "$DIR/sshd_config" <<CONF
Port $PORT
ListenAddress 127.0.0.1
HostKey $DIR/keys/host
AuthorizedKeysFile $DIR/authorized_keys
PidFile $DIR/sshd.pid
UsePAM no
PasswordAuthentication no
KbdInteractiveAuthentication no
StrictModes no
LogLevel ERROR
CONF
"$SSHD" -f "$DIR/sshd_config" -D >"$DIR/sshd.log" 2>&1 &
SSHD_PID=$!
for _ in $(seq 1 40); do
    (exec 3<>/dev/tcp/127.0.0.1/"$PORT") 2>/dev/null && break
    sleep 0.25
done
kill -0 "$SSHD_PID" 2>/dev/null || { cat "$DIR/sshd.log" >&2; fail "sshd would not start"; }

# ------------------------------------------------------------------------------ the client
# An agent holding alice first, which is the part that makes this bug appear at all: ssh offers
# the identities the agent holds before the ones it was pointed at, in the agent's own order.
eval "$(ssh-agent -s -a "$DIR/agent.sock" 2>/dev/null)" >/dev/null
AGENT_PID=$SSH_AGENT_PID
SSH_ASKPASS_REQUIRE=never ssh-add -q "$DIR/keys/alice" "$DIR/keys/bob" 2>/dev/null

# And a config naming alice for this host, which is what somebody with two accounts writes.
cat > "$DIR/ssh_config" <<CONF
Host coral.test
    HostName 127.0.0.1
    Port $PORT
    User $(id -un)
    IdentityFile $DIR/keys/alice
    IdentitiesOnly yes
    StrictHostKeyChecking no
    UserKnownHostsFile /dev/null
CONF

# The user's config is at `-F` because ssh takes its own path from the passwd entry, not from
# the environment, so a test cannot move it. Same file, same level, same precedence.
alias_url="coral.test:project.git"
# What the alias stood for. A pinned repository has to say it in the URL, since `-F none` gives
# up the config that used to supply it.
direct_url="ssh://$(id -un)@127.0.0.1:$PORT/project.git"
# The host key is generated and thrown away with the rest, so there is nothing to remember.
throwaway="-o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR"

attempt() {
    rm -rf "$DIR/out"
    GIT_SSH_COMMAND="$2" git clone -q "$1" "$DIR/out" 2>"$DIR/clone.err"
}

say "the host is up on 127.0.0.1:$PORT, with alice first in its own agent"

# 1. No pin at all. The config decides, the config says alice, and alice has nothing.
if attempt "$alias_url" "ssh -F $DIR/ssh_config"; then
    fail "alice should not be able to clone bob's project"
fi
grep -q "could not be found" "$DIR/clone.err" || fail "expected the host's own refusal"
say "ok   no pin: the config picks alice, and alice cannot see the project"

# 2. The pin Coral used to write. It loses to the config, which is the bug.
if attempt "$alias_url" "ssh -F $DIR/ssh_config -i $DIR/keys/bob -o IdentitiesOnly=yes"; then
    fail "the old command should have lost to the config, so this machine cannot show the bug"
fi
say "ok   old pin: -i and IdentitiesOnly=yes are not enough, alice still wins"

# 3. The pin Coral writes now.
attempt "$direct_url" "ssh -F none -i $DIR/keys/bob -o IdentitiesOnly=yes $throwaway" \
    || { cat "$DIR/clone.err" >&2; fail "the pinned key did not reach bob"; }
[ -f "$DIR/out/readme.md" ] || fail "cloned, but not bob's project"
say "ok   new pin: -F none makes the chosen key the only key, and bob's project arrives"

# 4. Fetching afterwards uses only what the clone recorded, which is what makes the pin stick.
git -C "$DIR/out" config core.sshCommand \
    "ssh -F none -i $DIR/keys/bob -o IdentitiesOnly=yes $throwaway"
git -C "$DIR/out" fetch -q origin || fail "a later fetch did not use the recorded key"
say "ok   a later fetch uses the key the clone recorded, with no help from the environment"

# 5. And the alias really is gone, which is the price the interface warns about.
if attempt "$alias_url" "ssh -F none -i $DIR/keys/bob -o IdentitiesOnly=yes $throwaway"; then
    fail "-F none should not have resolved the config's alias"
fi
say "ok   the price: a pinned repository no longer resolves a host alias from the config"

# --------------------------------------------------------------------------- submodules
# A submodule is a separate repository, and git clones it in a child process running in that
# submodule's own configuration. The `core.sshCommand` the superproject was cloned with is not
# part of it, so without help the submodule is reached as whoever the agent offers first.
bob_ssh="ssh -F none -i $DIR/keys/bob -o IdentitiesOnly=yes $throwaway"
alice_ssh="ssh -F none -i $DIR/keys/alice -o IdentitiesOnly=yes $throwaway"

git init -q --bare -b main "$DIR/srv/bob/tools.git"
tools="$DIR/toolwork"
git init -q -b main "$tools"
git -C "$tools" config user.email fixture@coral.test
git -C "$tools" config user.name Fixture
echo tools > "$tools/tool.txt"
git -C "$tools" add -A
git -C "$tools" -c commit.gpgsign=false commit -qm "the tool only bob can see"
git -C "$tools" push -q "$DIR/srv/bob/tools.git" HEAD:refs/heads/main
tools_head=$(git -C "$tools" rev-parse HEAD)

# A superproject naming it over ssh. The gitlink is written by hand rather than by `submodule
# add`, which would clone it here and prove nothing.
git init -q --bare -b main "$DIR/srv/bob/super.git"
sup="$DIR/superwork"
git init -q -b main "$sup"
git -C "$sup" config user.email fixture@coral.test
git -C "$sup" config user.name Fixture
printf '[submodule "vendor/tools"]\n\tpath = vendor/tools\n\turl = ssh://%s@127.0.0.1:%s/tools.git\n' \
    "$(id -un)" "$PORT" > "$sup/.gitmodules"
git -C "$sup" add .gitmodules
git -C "$sup" update-index --add --cacheinfo "160000,$tools_head,vendor/tools"
git -C "$sup" -c commit.gpgsign=false commit -qm "the superproject"
git -C "$sup" push -q "$DIR/srv/bob/super.git" HEAD:refs/heads/main

# A fresh clone of it each time, pinned the way Coral pins one, so each attempt starts from the
# state a user is actually in.
super_url="ssh://$(id -un)@127.0.0.1:$PORT/super.git"
fresh_super() {
    rm -rf "$DIR/super"
    GIT_SSH_COMMAND="$bob_ssh" git clone -q "$super_url" "$DIR/super" 2>"$DIR/clone.err" \
        || { cat "$DIR/clone.err" >&2; fail "the superproject itself did not clone"; }
    git -C "$DIR/super" config core.sshCommand "$bob_ssh"
}

# 6. The mechanism. The superproject's own command is made to record every time it is run; a
# submodule update then never runs it, which is the whole fault in one assertion.
fresh_super
cat > "$DIR/recorder" <<RECORD
#!/usr/bin/env bash
echo ran >> "$DIR/used"
exec $bob_ssh "\$@"
RECORD
chmod +x "$DIR/recorder"
git -C "$DIR/super" config core.sshCommand "$DIR/recorder"
rm -f "$DIR/used"
git -C "$DIR/super" submodule update --init >/dev/null 2>&1 || true
[ -s "$DIR/used" ] \
    && fail "the superproject's core.sshCommand reached the submodule; the bug is not present"
say "ok   a submodule reads none of the superproject's core.sshCommand"

# 7. What that costs. The environment stands for whatever the config and the agent would have
# decided on their own, which for somebody with two accounts is the wrong one.
fresh_super
if GIT_SSH_COMMAND="$alice_ssh" git -C "$DIR/super" submodule update --init \
    >/dev/null 2>"$DIR/sub.err"; then
    fail "alice should not be able to fetch bob's submodule"
fi
grep -q "could not be found" "$DIR/sub.err" \
    || { cat "$DIR/sub.err" >&2; fail "expected the host's own refusal"; }
say "ok   the wrong key reports a missing repository, never a wrong key"

# 8. The fix: the key in the environment, which is inherited by every git the update spawns.
fresh_super
GIT_SSH_COMMAND="$bob_ssh" git -C "$DIR/super" submodule update --init >/dev/null 2>"$DIR/sub.err" \
    || { cat "$DIR/sub.err" >&2; fail "the key in the environment did not reach the submodule"; }
[ -f "$DIR/super/vendor/tools/tool.txt" ] || fail "cloned, but not bob's tool"
say "ok   the key in the environment reaches the submodule's own clone"

# 9. And it has to be recorded there, because a fetch from inside the submodule is a git that
# reads only the submodule's configuration.
git -C "$DIR/super/vendor/tools" config core.sshCommand "$bob_ssh"
git -C "$DIR/super/vendor/tools" fetch -q origin \
    || fail "a fetch from inside the submodule did not use the key recorded there"
say "ok   a fetch from inside the submodule uses the key its own clone records"

# ------------------------------------------------------------------ coral's own command
# Everything above spells the command by hand. This runs the one Coral writes, for a key chosen
# from outside ~/.ssh: in a folder with a space in its name, and with no `.pub` beside it, which
# is how a key copied onto a Windows machine usually arrives.
if [ -n "${CORAL_BIN:-}" ]; then
    CORAL=$(cd "$(dirname "$CORAL_BIN")" && pwd)/$(basename "$CORAL_BIN")
    mkdir -p "$DIR/keys with space"
    cp "$DIR/keys/bob" "$DIR/keys with space/bob's key"
    chmod 600 "$DIR/keys with space/bob's key"

    # Coral's command is `ssh -F none ...`, which would read and write the real known_hosts. An
    # `ssh` earlier on PATH adds the throwaway host options first, where ssh lets them win, and
    # leaves every word of Coral's own command as it was.
    mkdir -p "$DIR/bin"
    printf '#!/usr/bin/env bash\nexec %s %s "$@"\n' "$(command -v ssh)" "$throwaway" > "$DIR/bin/ssh"
    chmod +x "$DIR/bin/ssh"

    # 10. A clone with the chosen key reaches bob, and the key is recorded as Coral spelled it.
    rm -rf "$DIR/into"
    mkdir -p "$DIR/into"
    PATH="$DIR/bin:$PATH" "$CORAL" clone "$direct_url" --into "$DIR/into" \
        --ssh-key "$DIR/keys with space/bob's key" --quiet >"$DIR/coral.out" 2>&1 \
        || { cat "$DIR/coral.out" >&2; fail "coral did not clone with a key from outside ~/.ssh"; }
    [ -f "$DIR/into/project/readme.md" ] || fail "coral cloned, but not bob's project"
    recorded=$(git -C "$DIR/into/project" config core.sshCommand)
    case "$recorded" in
        *"keys with space/bob'\''s key"*) ;;
        *) fail "the recorded command does not name the chosen key: $recorded" ;;
    esac
    say "ok   coral clones with a key from a folder with a space, with no .pub beside it"

    # 11. And a later fetch runs that recorded command, quoting and all, through git's own sh.
    PATH="$DIR/bin:$PATH" git -C "$DIR/into/project" fetch -q origin \
        || fail "a fetch with coral's recorded command did not reach bob"
    say "ok   a later fetch uses the command coral recorded"

    # 12. The same clone pinned to alice is refused by the host, although the agent holds bob
    # too. So it was the chosen key that reached bob above, not the agent's.
    rm -rf "$DIR/into"
    mkdir -p "$DIR/into"
    if PATH="$DIR/bin:$PATH" "$CORAL" clone "$direct_url" --into "$DIR/into" \
        --ssh-key "$DIR/keys/alice" --quiet >"$DIR/coral.out" 2>&1; then
        fail "coral cloned bob's project with alice's key"
    fi
    grep -q "could not be found" "$DIR/coral.out" \
        || { cat "$DIR/coral.out" >&2; fail "expected the host's own refusal"; }
    say "ok   the same command with alice's key is refused, so the chosen key is the one used"
else
    say "skipped: coral's own command (CORAL_BIN is not set; run it through just ssh-test)"
fi

# ------------------------------------------------------------------- first contact with a host
# Everything above tells ssh to accept any host and remember none, which is exactly why a clone
# of a host ssh had never seen was never tried: with no terminal to ask "continue connecting?",
# ssh refused it and the window said only "Could not read from remote repository". This half
# leaves strict checking as ssh ships it and gives it a known_hosts of its own, empty at first.
if [ -n "${CORAL_BIN:-}" ]; then
    : > "$DIR/known_hosts"
    : > "$DIR/plain_config"
    mkdir -p "$DIR/strict"
    # The isolated known_hosts first, where ssh lets it win. A config only when the command
    # brought none: Coral's pinned one says `-F none`, and the owner's must never be read.
    cat > "$DIR/strict/ssh" <<WRAP
#!/usr/bin/env bash
extra=(-o UserKnownHostsFile=$DIR/known_hosts -o GlobalKnownHostsFile=/dev/null)
case " \$* " in *" -F "*) ;; *) extra+=(-F $DIR/plain_config) ;; esac
exec $(command -v ssh) "\${extra[@]}" "\$@"
WRAP
    chmod +x "$DIR/strict/ssh"

    # No controlling terminal, as under a desktop launcher or on Windows, where Coral's ssh has
    # nowhere to ask but Coral itself.
    detached() {
        if command -v setsid >/dev/null 2>&1; then
            PATH="$DIR/strict:$PATH" setsid -w "$CORAL" "$@"
        else
            PATH="$DIR/strict:$PATH" "$CORAL" "$@"
        fi
    }
    # The one field of the JSON envelope a section asserts on.
    field() {
        python3 -c 'import json,sys
d = json.load(sys.stdin)
for k in sys.argv[1:]:
    d = d[k]
print(d)' "$@"
    }

    host="[127.0.0.1]:$PORT"
    fingerprint=$(ssh-keygen -lf "$DIR/keys/host.pub" | awk '{print $2}')
    bob_key="$DIR/keys/bob"
    fresh_into() { rm -rf "$DIR/into"; mkdir -p "$DIR/into"; }
    # Every connection since the mark arrived as bob, and there was at least one.
    only_bob_since() {
        local lines
        lines=$(tail -n +"$(($1 + 1))" "$DIR/seen")
        [ -n "$lines" ] || fail "$2: the host saw no connection at all"
        if grep -qv '^bob ' <<<"$lines"; then
            fail "$2: the host saw someone other than bob: $lines"
        fi
    }
    mark() { wc -l < "$DIR/seen"; }

    # 13. A clone of a host ssh has never seen stops, naming the key the host presented.
    fresh_into
    if detached --json clone "$direct_url" --into "$DIR/into" --ssh-key "$bob_key" --quiet \
        >"$DIR/first.json" 2>"$DIR/first.err"; then
        fail "a host ssh has never seen was connected to without anybody trusting it"
    fi
    [ "$(field error code < "$DIR/first.json")" = unknown_host_key ] \
        || { cat "$DIR/first.json" "$DIR/first.err" >&2; fail "expected unknown_host_key"; }
    [ "$(field error hostKey fingerprint < "$DIR/first.json")" = "$fingerprint" ] \
        || fail "the fingerprint reported is not the host's"
    [ "$(field error hostKey host < "$DIR/first.json")" = "$host" ] \
        || fail "the host reported is not the one connected to"
    [ ! -s "$DIR/known_hosts" ] || fail "a key was recorded that nobody trusted"
    say "ok   a host ssh has not seen is reported with the fingerprint it presented"

    # 14. Trusting some other fingerprint changes nothing: ssh compares, and records nothing.
    fresh_into
    if detached clone "$direct_url" --into "$DIR/into" --ssh-key "$bob_key" --quiet \
        --trust-host-key "$host=SHA256:notthehostskeynotthehostskeynotthehostskey0" \
        >"$DIR/wrong.out" 2>&1; then
        fail "a clone went ahead trusting a fingerprint that is not the host's"
    fi
    grep -q unknown_host_key "$DIR/wrong.out" || { cat "$DIR/wrong.out" >&2; fail "expected unknown_host_key"; }
    [ ! -s "$DIR/known_hosts" ] || fail "a key was recorded under the wrong fingerprint"
    say "ok   a fingerprint that is not the host's is refused, and nothing is recorded"

    # 15. Trusted, the clone goes ahead, and ssh itself records the key.
    fresh_into
    at=$(mark)
    detached clone "$direct_url" --into "$DIR/into" --ssh-key "$bob_key" --quiet \
        --trust-host-key "$host=$fingerprint" >"$DIR/trusted.out" 2>&1 \
        || { cat "$DIR/trusted.out" >&2; fail "the trusted clone failed"; }
    [ -f "$DIR/into/project/readme.md" ] || fail "cloned, but not bob's project"
    grep -q "^\[127.0.0.1\]:$PORT ssh-ed25519 " "$DIR/known_hosts" \
        || fail "ssh did not record the trusted key: $(cat "$DIR/known_hosts")"
    only_bob_since "$at" "the trusted clone"
    say "ok   trusted, the clone goes ahead as bob and ssh records the host"

    # 16. Every later stage offers the same key, with the agent holding alice first throughout.
    project="$DIR/pinned"
    mv "$DIR/into/project" "$project"
    git -C "$project" config user.email fixture@coral.test
    git -C "$project" config user.name Fixture
    at=$(mark)
    detached --repo "$project" fetch >"$DIR/stage.out" 2>&1 \
        || { cat "$DIR/stage.out" >&2; fail "fetch after first contact"; }
    detached --repo "$project" pull >"$DIR/stage.out" 2>&1 \
        || { cat "$DIR/stage.out" >&2; fail "pull after first contact"; }
    echo more >> "$project/readme.md"
    git -C "$project" -c commit.gpgsign=false commit -qam "a commit bob pushes"
    detached --repo "$project" push >"$DIR/stage.out" 2>&1 \
        || { cat "$DIR/stage.out" >&2; fail "push after first contact"; }
    detached --repo "$project" submodule-add "ssh://$(id -un)@127.0.0.1:$PORT/tools.git" \
        vendor/tools --ssh-key "$bob_key" >"$DIR/stage.out" 2>&1 \
        || { cat "$DIR/stage.out" >&2; fail "submodule add after first contact"; }
    [ -f "$project/vendor/tools/tool.txt" ] || fail "the submodule added is not bob's tool"
    fresh_into
    detached clone "$super_url" --into "$DIR/into" --ssh-key "$bob_key" --quiet \
        >"$DIR/stage.out" 2>&1 || { cat "$DIR/stage.out" >&2; fail "clone of the superproject"; }
    detached --repo "$DIR/into/super" submodule-init >"$DIR/stage.out" 2>&1 \
        || { cat "$DIR/stage.out" >&2; fail "submodule init after first contact"; }
    [ -f "$DIR/into/super/vendor/tools/tool.txt" ] || fail "the submodule initialised is not bob's tool"
    only_bob_since "$at" "fetch, pull, push, submodule add, clone and submodule init"
    grep -q '^bob git-receive-pack' "$DIR/seen" || fail "the push never reached the host"
    say "ok   fetch, pull, push, submodule add and submodule init all offer bob and only bob"

    # 17. With no key chosen at all the question is the same, and the agent's key goes ahead.
    : > "$DIR/known_hosts"
    bob_agent=$(SSH_AGENT_PID='' ssh-agent -s -a "$DIR/agent-bob.sock" 2>/dev/null)
    EXTRA_PIDS="${EXTRA_PIDS:-} $(sed -n 's/^SSH_AGENT_PID=\([0-9]*\);.*/\1/p' <<<"$bob_agent")"
    SSH_AUTH_SOCK="$DIR/agent-bob.sock" SSH_ASKPASS_REQUIRE=never ssh-add -q "$DIR/keys/bob" 2>/dev/null
    fresh_into
    if (export SSH_AUTH_SOCK="$DIR/agent-bob.sock"
        detached --json clone "$direct_url" --into "$DIR/into" --quiet) \
        >"$DIR/agent.json" 2>/dev/null; then
        fail "an agent clone reached a host nobody trusted"
    fi
    [ "$(field error code < "$DIR/agent.json")" = unknown_host_key ] \
        || { cat "$DIR/agent.json" >&2; fail "expected unknown_host_key with no key chosen"; }
    at=$(mark)
    (export SSH_AUTH_SOCK="$DIR/agent-bob.sock"
     detached clone "$direct_url" --into "$DIR/into" --quiet \
        --trust-host-key "$host=$fingerprint") >"$DIR/agent.out" 2>&1 \
        || { cat "$DIR/agent.out" >&2; fail "the trusted agent clone failed"; }
    only_bob_since "$at" "the agent clone"
    say "ok   with no key chosen, the same question, and the agent's key once trusted"

    # 18. A key with a passphrase that no agent holds still fails, and fails at once: Coral's
    # askpass answers only the question about a host, never a passphrase.
    ssh-keygen -q -t ed25519 -N 'a passphrase' -C coral-fixture-carol -f "$DIR/keys/carol"
    chmod 600 "$DIR/keys/carol"
    fresh_into
    session=$(command -v setsid || true)
    status=0
    (unset SSH_AUTH_SOCK
     PATH="$DIR/strict:$PATH" timeout 30 $session "$CORAL" clone "$direct_url" \
        --into "$DIR/into" --ssh-key "$DIR/keys/carol" --quiet) >"$DIR/carol.out" 2>&1 || status=$?
    [ "$status" -ne 0 ] || fail "a key with a passphrase was used with nobody to type it"
    [ "$status" -ne 124 ] || fail "a key with a passphrase hung instead of failing"
    grep -q "Permission denied (publickey)" "$DIR/carol.out" \
        || { cat "$DIR/carol.out" >&2; fail "ssh's own reason did not reach the message"; }
    say "ok   a passphrase nobody can type fails at once, and says why"

    # 19. https is untouched: with no credential it fails as it always did, and Coral's askpass,
    # which git would otherwise ask for a user name, says nothing.
    HTTP_PORT=$(python3 -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1]);s.close()')
    python3 - "$HTTP_PORT" <<'PY' &
import http.server, sys
class Refuse(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(401)
        self.send_header("WWW-Authenticate", 'Basic realm="fixture"')
        self.end_headers()
    def log_message(self, *a):
        pass
http.server.HTTPServer(("127.0.0.1", int(sys.argv[1])), Refuse).serve_forever()
PY
    HTTP_PID=$!
    EXTRA_PIDS="${EXTRA_PIDS:-} $HTTP_PID"
    sleep 0.5
    fresh_into
    if detached clone "http://127.0.0.1:$HTTP_PORT/project.git" --into "$DIR/into" --quiet \
        >"$DIR/https.out" 2>&1; then
        fail "a clone needing a password went ahead without one"
    fi
    kill "$HTTP_PID" 2>/dev/null || true
    grep -q "terminal prompts disabled" "$DIR/https.out" \
        || { cat "$DIR/https.out" >&2; fail "expected git's own refusal"; }
    if grep -qi askpass "$DIR/https.out"; then
        cat "$DIR/https.out" >&2
        fail "git asked Coral's askpass for an https user name"
    fi
    say "ok   https is left as it was: no credential, git's own refusal, no askpass"

    # 20. A host whose key has changed is refused, loudly, and never offered for trust.
    kill "$SSHD_PID" 2>/dev/null || true
    wait "$SSHD_PID" 2>/dev/null || true
    rm -f "$DIR/keys/host" "$DIR/keys/host.pub"
    ssh-keygen -q -t ed25519 -N '' -C coral-fixture-host2 -f "$DIR/keys/host"
    "$SSHD" -f "$DIR/sshd_config" -D >"$DIR/sshd.log" 2>&1 &
    SSHD_PID=$!
    for _ in $(seq 1 40); do
        (exec 3<>/dev/tcp/127.0.0.1/"$PORT") 2>/dev/null && break
        sleep 0.25
    done
    # known_hosts still holds the key trusted above, which the host no longer presents.
    if detached --json --repo "$project" fetch >"$DIR/changed.json" 2>/dev/null; then
        fail "a host with a different key was fetched from"
    fi
    [ "$(field error code < "$DIR/changed.json")" = git_error ] \
        || { cat "$DIR/changed.json" >&2; fail "a changed key must not be offered for trust"; }
    field error message < "$DIR/changed.json" | grep -q "REMOTE HOST IDENTIFICATION HAS CHANGED" \
        || { cat "$DIR/changed.json" >&2; fail "the message does not say the key changed"; }
    say "ok   a changed host key is refused, says so, and is never offered for trust"
else
    say "skipped: first contact (CORAL_BIN is not set; run it through just ssh-test)"
fi

say "passed"
