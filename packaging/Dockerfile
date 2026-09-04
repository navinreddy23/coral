# The image the portable Linux bundle is built in.
#
# An AppImage does not bundle glibc and cannot: the format assumes the base system provides it,
# and glibc is forward-compatible only — a binary linked against 2.39 will not load on 2.35,
# whatever else is packed beside it. So the one thing that decides how portable the result is
# is the glibc it was built against, and building on the desktop distribution of the day makes
# an AppImage that runs on that distribution and nothing older.
#
# 22.04 rather than something older because it is the first Ubuntu carrying libwebkit2gtk-4.1,
# which Tauri 2 requires. Its glibc is 2.35, which covers Ubuntu 22.04 and 24.04, Debian 12,
# Fedora 36 and every current distribution.
FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive

# `file` and `desktop-file-utils` are wanted by the AppImage tooling itself, and `xdg-utils`
# supplies the `xdg-open` the bundler copies into the AppDir for the opener plugin; the rest is
# what Tauri links against.
RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential curl wget file ca-certificates git pkg-config \
        libwebkit2gtk-4.1-dev libxdo-dev libayatana-appindicator3-dev librsvg2-dev \
        libssl-dev desktop-file-utils xdg-utils \
    && rm -rf /var/lib/apt/lists/*

# The toolchain goes somewhere every user can read, not into root's home. The container is run
# as the invoking user so that nothing it writes into the bind-mounted repository comes back
# owned by root — which is not a tidiness point: a `node_modules` left owned by root cannot be
# removed without sudo and stops the host building at all until it is.
ENV RUSTUP_HOME=/opt/rustup
ENV CARGO_HOME=/opt/cargo
ENV PATH=/opt/cargo/bin:$PATH

# Pinned rather than "stable": a toolchain that moves under a release build is a release that
# cannot be reproduced.
ARG RUST_VERSION=1.98.0
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
      | sh -s -- -y --default-toolchain ${RUST_VERSION} --profile minimal \
    && cargo install tauri-cli --version "^2" --locked \
    && chmod -R a+rX /opt/rustup /opt/cargo

ARG NODE_MAJOR=24
RUN curl -fsSL https://deb.nodesource.com/setup_${NODE_MAJOR}.x | bash - \
    && apt-get install -y --no-install-recommends nodejs \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /work

# A git of Coral's own, for the `bundled_git` option of `just build-linux-portable`.
#
# The reason it exists: Coral shells out to git for every mutation and needs 2.40 or newer, and
# a colleague's laptop running a long-term-support distribution has 2.34. Shipping one removes
# the requirement rather than explaining it.
#
# Built here rather than taken from apt, because 22.04's own git is 2.34 — the very version
# that is too old. Pinned and checksummed: a release that cannot be reproduced is not a release.
#
# git is GPL-2.0-only. Distributing these binaries carries the obligation to offer the
# corresponding source; the version and its origin are recorded beside the bundle and in
# CREDITS.md so that offer can be honoured.
ARG GIT_VERSION=2.55.0
ARG GIT_SHA256=457fdb04dc8728e007d4688695e6912e6f680727920f2a40bf11eacc17505357
RUN apt-get update && apt-get install -y --no-install-recommends \
        libcurl4-openssl-dev libexpat1-dev zlib1g-dev patchelf \
    && curl -fsSLo /tmp/git.tar.xz \
        "https://mirrors.edge.kernel.org/pub/software/scm/git/git-${GIT_VERSION}.tar.xz" \
    && echo "${GIT_SHA256}  /tmp/git.tar.xz" | sha256sum -c - \
    && tar -xf /tmp/git.tar.xz -C /tmp \
    # No Tcl/Tk, gettext, Perl or Python: Coral drives git through plumbing and never reaches
    # gitk, `git send-email` or the translated messages, and every one of those left out is an
    # interpreter the target machine no longer has to have. Hardlinks become symlinks because
    # the bundler copies the tree, and a hundred hardlinks to a five-megabyte binary copy as a
    # hundred five-megabyte files.
    && make -C "/tmp/git-${GIT_VERSION}" -j"$(nproc)" \
        prefix=/opt/git-bundle \
        NO_TCLTK=1 NO_GETTEXT=1 NO_PERL=1 NO_PYTHON=1 NO_INSTALL_HARDLINKS=1 \
        install \
    # Unstripped, the tree is 160MB: the builtins are symlinks to one binary, but that binary
    # carries its debug symbols and there are a dozen more like it. Nobody debugs the git
    # inside a bundle from the bundle.
    && find /opt/git-bundle -type f -perm -u+x -exec sh -c \
        'head -c4 "$1" | grep -q "^.ELF" && strip --strip-unneeded "$1" || true' _ {} \; \
    && printf '%s\n' \
        "This directory holds git ${GIT_VERSION}, unmodified, built from" \
        "https://mirrors.edge.kernel.org/pub/software/scm/git/git-${GIT_VERSION}.tar.xz" \
        "sha256 ${GIT_SHA256}" \
        "" \
        "git is free software under the GNU General Public License version 2." \
        "The corresponding source is the tarball above." \
        > /opt/git-bundle/SOURCE.txt \
    && rm -rf /tmp/git.tar.xz "/tmp/git-${GIT_VERSION}" /var/lib/apt/lists/*

COPY selfcontain.sh /usr/local/bin/selfcontain
RUN chmod +x /usr/local/bin/selfcontain && selfcontain /opt/git-bundle \
    && chmod -R a+rX /opt/git-bundle
