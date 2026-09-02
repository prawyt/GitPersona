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

## Build the executable

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

## Build Windows installers

Run the build without `--no-bundle` to generate the configured Windows
installer formats:

```powershell
cd .\desktop
npm ci
npm run tauri build
```

Installer artifacts are written below:

```text
target\release\bundle\
```

Public release installers should be code-signed. Local unsigned artifacts are
suitable for development and testing but may trigger Windows security prompts.

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

## Development build

To run the desktop application with Vite hot reload:

```powershell
cd .\desktop
npm ci
npm run tauri dev
```

Development builds are not release artifacts. Use `npm run tauri build` when
you need an optimized executable or installer.
