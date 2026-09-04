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
