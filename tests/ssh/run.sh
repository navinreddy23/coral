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
cleanup() {
    [ -n "$SSHD_PID" ] && kill "$SSHD_PID" 2>/dev/null || true
    [ -n "$AGENT_PID" ] && kill "$AGENT_PID" 2>/dev/null || true
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

say "passed"
