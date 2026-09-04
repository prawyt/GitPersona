# Building the desktop application

GitPersona Desktop uses Tauri 2, Rust, React, TypeScript, and Vite. Run the
commands below from PowerShell on Windows.

## Prerequisites

Install the following before building:

- Rust using [rustup](https://rustup.rs/)
- Node.js 22 or newer, including npm
- Microsoft C++ Build Tools with the Desktop development with C++ workload
- Microsoft Edge WebView2 Runtime

GitPersona also expects `git`, `gh`, and `ssh` to be available on `PATH` when
the application runs.

Verify the main build tools:

```powershell
rustc --version
cargo --version
node --version
npm --version
```

## Choose the Windows artifact

GitPersona can produce three Windows files:

| File | Purpose | Build command |
| --- | --- | --- |
| `gitpersona-desktop.exe` | Portable application; runs without an installer | `npm run tauri build -- --no-bundle` |
| `GitPersona_<version>_x64-setup.exe` | NSIS installer | `npm run tauri build -- --bundles nsis` |
| `GitPersona_<version>_x64_en-US.msi` | MSI installer for Windows deployment tools | `npm run tauri build -- --bundles msi` |

Use the combined installer command below when both the setup EXE and MSI are
required.

## Build the portable EXE

From the repository root, install the locked frontend dependencies and build
an optimized executable without packaging an installer:

```powershell
cd .\desktop
npm ci
npm run tauri build -- --no-bundle
```

The executable is written to:

```text
target\release\gitpersona-desktop.exe
```

The first build can take several minutes. Later builds reuse Cargo's compiled
dependencies and are normally faster.

## Build the installable EXE and MSI

From the repository root, run:

```powershell
cd .\desktop
npm ci
npm run tauri build -- --bundles msi,nsis
```

The command compiles the application once and creates both installers:

```text
target\release\bundle\nsis\GitPersona_0.6.0_x64-setup.exe
target\release\bundle\msi\GitPersona_0.6.0_x64_en-US.msi
```

The version in each filename comes from `desktop\src-tauri\tauri.conf.json`.
The compiled portable executable also remains at
`target\release\gitpersona-desktop.exe`.

Public release installers should be code-signed. Local unsigned artifacts are
suitable for development and testing but may trigger Windows security prompts.

Released artifacts are currently unsigned as well, so Windows SmartScreen warns
on first run and macOS Gatekeeper refuses to open the bundle without an explicit
override. Until signing certificates are in place, the published SHA-256 is the
verification path - see [SECURITY.md](../SECURITY.md#release-verification).

## GitHub release artifacts

The current release workflow builds Windows artifacts only. It can be started
from the GitHub Actions **Release desktop** page with **Run workflow**, or by
pushing a version tag matching `v0.6.*`.

The workflow creates a draft GitHub Release containing the NSIS setup EXE, MSI,
and a renamed portable executable such as:

```text
GitPersona_0.6.0_windows_x86_64.exe
```

The portable EXE includes a matching `.sha256` file for download verification.
The MSI and NSIS installers are also retained together as the
`gitpersona-windows-installers` workflow artifact.

Review the draft release and its artifacts before publishing it. A release tag
can be created after the intended commit has been merged:

```powershell
git tag v0.6.0
git push origin v0.6.0
```

## Rebuild after source changes

If dependencies are already installed, only the build command is required:

```powershell
cd .\desktop
npm run tauri build -- --no-bundle
```

Run the checks before distributing an executable:

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features

cd .\desktop
npm test -- --run
npm run build
```

## Clean rebuild

Use a clean rebuild when compiled dependencies are stale or a normal rebuild
behaves unexpectedly:

```powershell
cargo clean

cd .\desktop
Remove-Item -Recurse -Force .\dist -ErrorAction SilentlyContinue
Remove-Item -Recurse -Force .\node_modules -ErrorAction SilentlyContinue
npm ci
npm run tauri build -- --no-bundle
```

Keep `desktop/package-lock.json`; `npm ci` uses it to reproduce the dependency
versions used by the project.

To remove only generated Windows installers while retaining Cargo's compiled
dependency cache:

```powershell
Remove-Item -Recurse -Force .\target\release\bundle -ErrorAction SilentlyContinue
```

Run `cargo clean` only when a completely clean Rust build is needed. It removes
the entire `target` directory and can reclaim several gigabytes, but the next
build will take longer because every Rust dependency must be compiled again.

## Development build

To run the desktop application with Vite hot reload:

```powershell
cd .\desktop
npm ci
npm run tauri dev
```

Development builds are not release artifacts. Use
`npm run tauri build -- --no-bundle` for a portable release executable or
`npm run tauri build -- --bundles msi,nsis` for both Windows installers.
