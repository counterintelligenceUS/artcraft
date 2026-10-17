Dev Setup
=========

# ArtCraft 

ArtCraft is a Rust / Tauri app.

To set up the ArtCraft development environment,  install the following:

1. [Install Rust](https://doc.rust-lang.org/cargo/getting-started/installation.html).
2. [Install npm](https://nodejs.org/en/download) or [nvm](https://github.com/nvm-sh/nvm). (Node version `v24.13.0` works at time of writing.) 
3. The frontend's Nx and Vite dependencies are installed locally with npm; a global Nx installation is not required for the combined Unix launcher.
4. [Install Tauri CLI](https://v2.tauri.app/reference/cli/). (Version `tauri-cli 2.10.0` works at time of writing.)

**Mac and Linux Development** 

```bash
# Start the frontend and Rust/Tauri together in one terminal
./script/artcraft/unix_dev.sh

# Optionally start the free-port search at another port
ARTCRAFT_DEV_PORT=6200 ./script/artcraft/unix_dev.sh
```

Use the combined launcher instead of starting the two legacy dev scripts. It
works from any working directory when invoked by its path and installs frontend
dependencies if the local Vite installation is missing. Node.js 20+ is required.

The launcher binds Vite to `127.0.0.1`, starting at port **5193** and trying
successive ports until one is available. It leaves existing listeners alone.
Only after the real frontend socket is bound does it start Tauri with that exact
`devUrl`. HTTP and the HMR websocket share the selected port; Rust communicates
through native Tauri IPC and needs no separate HTTP port. Configuration overrides
are passed in memory, without changing the checked-in Tauri config.

The native HTTP bridge normalizes loopback development origins for first-party
API requests so an automatically selected port does not break login or media
loading. See the [desktop session regression checks](../docs/desktop-session-regression.md)
when changing ports, the launcher, or authentication.

JavaScript, TypeScript, React, and CSS changes use Vite hot reload; compatible
React component edits use Fast Refresh. Changes that cannot be hot-replaced
reload the page. Rust changes use `cargo tauri dev`'s automatic **rebuild and app
restart**, including dependent workspace crates. SQLx metadata and SQLite
migrations are also watched. Rust reload is a process restart, so it does not
preserve unsaved in-memory app state. See [Tauri's watch behavior](https://v2.tauri.app/develop/#reacting-to-source-code-changes).

After startup, a Vite configuration restart must reuse the chosen port or report
an error; it cannot silently switch to another port while Tauri uses an old URL.
Ctrl-C stops this session's frontend, Rust watcher/compiler, and desktop app.
If the Rust watcher exits, the launcher closes Vite and propagates the exit code.
It never finds or terminates another app by name or port.

The launcher retains the existing Unix defaults for `VITE_ENVIRONMENT_TYPE`,
`SQLX_OFFLINE`, and `RUSTFLAGS`. Explicit environment overrides are preserved.
It does not force WebKit's software compositor; the Linux app keeps the GPU
compositor unless `ARTCRAFT_WEBKIT_SOFTWARE=1` is set, the display looks like
X11 forwarding (`localhost:10.0`), or a direct GLX context cannot be created.
`ARTCRAFT_WEBKIT_GPU=1` keeps the GPU compositor in those cases. The launcher
uses the normal Artcraft app data and Cargo target directory; port separation
does not create an isolated account/data profile, and concurrent Rust builds
can wait on Cargo's build lock.

To verify the launcher without compiling Rust or opening a desktop window:

```bash
node --test frontend/scripts/unix-dev.test.mjs
```

The tests use real Vite listeners and HMR, with a test-owned fake Cargo process
tree. They cover occupied ports, simultaneous launches, React edits, shutdown,
Rust-process failure, and watcher arguments. To verify native Rust reload, run
the launcher, wait for the desktop window, make a Rust source edit, and check for
a rebuild/restart in the same terminal. Frontend edits should update the webview
without restarting the Rust process.

**Windows Development**

```powershell
# Run the frontend dev server
.\script\artcraft\windows_frontend_dev.ps1

# Run the Tauri Rust application
.\script\artcraft\windows_rust_dev.ps1
```

**Linux production build**

Ubuntu 24.04 and Linux Mint 22 need the Tauri and BoringSSL packages below.
BoringSSL is built with clang, which links GCC 14's libstdc++ on Ubuntu 24.04,
so `g++-13` alone is not enough. Do not set `RUSTFLAGS=-C target-feature=+crt-static`;
that flag is Windows-only.

```bash
sudo apt-get install -y build-essential curl wget file patchelf \
  libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev \
  librsvg2-dev libxdo-dev libssl-dev libfuse2t64 squashfs-tools \
  pkg-config cmake clang libclang-dev llvm nasm \
  g++-14 libstdc++-14-dev

export SQLX_OFFLINE=true
./script/artcraft/unix_build.sh
```

`SQLX_OFFLINE=true` uses the `.sqlx` cache. Run the script from the repository
root. On Linux, Tauri merges `crates/desktop/artcraft/tauri.linux.conf.json`,
which bundles a `.deb` and an AppImage and skips rpm. The script builds the
frontend from `frontend/` and then clears `beforeBuildCommand`, because Tauri
would otherwise run `nx` from the Rust crate.

The first Linux window opens at 1600×900 so it fits a 1920×1080 screen. macOS
and Windows stay at 2400×1300. WebKit's GPU compositor is the default, because
3D views use three.js WebGL. Software rendering turns WebGL off. Use it when
the window is blank:

```bash
ARTCRAFT_WEBKIT_SOFTWARE=1 artcraft
```

The same software path is selected automatically for X11 forwarding and when
creating a direct GLX context fails.

Backend services and website builds live in the separate `artcraft-services` repository.
This repository retains the desktop task database in
`_database/sql/artcraft_migrations/` and its SQLite query cache in `.sqlx/`.
Use `SQLX_OFFLINE=true cargo check -p artcraft` to check Rust without a database server.
