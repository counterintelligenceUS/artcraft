#!/usr/bin/env bash
# Production build of the Tauri desktop app on Linux and macOS.
#
# Run from the repository root. Requires Rust, cargo-tauri 2, and Node 20+.
# Linux (Ubuntu 24.04 / Linux Mint 22) also needs:
#   sudo apt-get install -y build-essential curl wget file patchelf \
#     libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev \
#     librsvg2-dev libxdo-dev libssl-dev libfuse2t64 squashfs-tools \
#     pkg-config cmake clang libclang-dev llvm nasm \
#     g++-14 libstdc++-14-dev
# BoringSSL (the wreq TLS stack) is built with clang, which on Ubuntu 24.04
# links against GCC 14's libstdc++. g++-13 alone is not enough.
# `tauri build` with bundle target "all" also tries to build an rpm. This
# script asks for deb and AppImage on Linux so rpmbuild is not required.

set -euxo pipefail

echo "Building production Artcraft..."
echo ""

root_dir=$(pwd)
frontend_path="${root_dir}/frontend"
rust_crate_path="${root_dir}/crates/desktop/artcraft"

# Build with the desktop app configuration.
config_path="${rust_crate_path}/tauri.conf.json"

# NB: The "frontend dev" script sets "production" too, so this must only control the
# hostnames we use, not minification, etc.
export VITE_ENVIRONMENT_TYPE="production"
export SQLX_OFFLINE=true

pushd "${frontend_path}" || exit

npm install

# beforeBuildCommand in tauri.conf.json is `npx nx run artcraft:build`, and
# Tauri runs it with cwd in the Rust crate. nx is installed under frontend/,
# so that command does not see it. Build here, then skip the hook.
npx nx run artcraft:build

popd || exit

export TAURI_FRONTEND_PATH="${frontend_path}"
export TAURI_APP_PATH="${rust_crate_path}"

cd "${rust_crate_path}"
# On Linux, "all" also tries to build an rpm, which needs rpmbuild.
if [[ "$(uname -s)" == "Linux" ]]; then
  cargo tauri build \
    --bundles deb,appimage \
    --config "${config_path}" \
    --config '{"build":{"beforeBuildCommand":""}}'
else
  cargo tauri build \
    --config "${config_path}" \
    --config '{"build":{"beforeBuildCommand":""}}'
fi

echo "Done!"

date "+Finished on %A, %B %e - %H:%M:%S (local timezone)"

