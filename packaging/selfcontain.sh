#!/usr/bin/env bash
# Makes an installed prefix carry its own shared libraries.
#
# The point is the machine this is going to, not the one it was built on. A git linked against
# 22.04's libcurl asks for symbols an older distribution's libcurl does not export under the
# same version tag, and the failure is a cloning that stops with an undefined symbol rather
# than anything a user could act on. So every library that is not part of the base system
# contract is copied in beside the binaries and reached through an rpath relative to them.
#
# glibc itself is deliberately left out. It cannot be relocated this way, it is forward
# compatible, and shipping a second copy beside the host's is how a process ends up with two.
set -euo pipefail

prefix="${1:?usage: selfcontain PREFIX}"
lib="$prefix/lib"
mkdir -p "$lib"

# The ELF magic, rather than `file`. `file` pads its output into columns when it is given
# more than one name, and a parser that did not expect the padding quietly matched nothing —
# which looks exactly like a prefix that needs no libraries.
elf() {
  [ "$(dd if="$1" bs=4 count=1 status=none | od -An -tx1 | tr -d ' \n')" = "7f454c46" ]
}

elves() {
  local path
  while IFS= read -r path; do
    elf "$path" && printf '%s\n' "$path"
  done < <(find "$prefix/bin" "$prefix/libexec" -type f -perm -u+x 2>/dev/null)
}

# The base system provides these, and they are forward compatible.
system() {
  case "$1" in
    */libc.so.*|*/libm.so.*|*/libpthread.so.*|*/libdl.so.*|*/librt.so.*|*/libresolv.so.*) return 0 ;;
    */ld-linux*|*/libgcc_s.so.*) return 0 ;;
    *) return 1 ;;
  esac
}

# Everything reachable, not only the first level: a copied library has dependencies of its own.
collect() {
  local pending=("$@") seen=() next
  while [ ${#pending[@]} -gt 0 ]; do
    next=("${pending[@]}")
    pending=()
    for binary in "${next[@]}"; do
      while read -r so; do
        [ -n "$so" ] || continue
        system "$so" && continue
        local name
        name="$(basename "$so")"
        case " ${seen[*]-} " in *" $name "*) continue ;; esac
        seen+=("$name")
        cp -Lf "$so" "$lib/$name"
        pending+=("$lib/$name")
      done < <(ldd "$binary" 2>/dev/null | awk '/=> \//{print $3}')
    done
  done
}

mapfile -t binaries < <(elves)
[ ${#binaries[@]} -gt 0 ] || { echo "selfcontain: nothing executable under $prefix" >&2; exit 1; }
collect "${binaries[@]}"

# Each binary reaches the private lib directory relative to itself, so the prefix can be moved
# anywhere and still find it. Libraries sit beside each other, so theirs is just $ORIGIN.
for binary in "${binaries[@]}"; do
  up="$(realpath --relative-to="$(dirname "$binary")" "$lib")"
  patchelf --set-rpath "\$ORIGIN/$up" "$binary"
done
for so in "$lib"/*; do
  [ -f "$so" ] && patchelf --set-rpath '$ORIGIN' "$so"
done

echo "selfcontain: $prefix carries $(ls -1 "$lib" | wc -l) libraries"
