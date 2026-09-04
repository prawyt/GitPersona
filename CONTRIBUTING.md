# Contributing to GitPersona

Thanks for helping make multiple-account Git workflows safer.

1. Open an issue for significant behavioral or configuration-format changes.
2. Keep authentication delegated to GitHub CLI, the operating-system credential store, or OpenSSH. Code that reads or stores tokens is out of scope.
3. Add tests for changes to profile validation, Git configuration, remote parsing, account checks, or hook behavior.
4. Run formatting, Clippy with warnings denied, and the complete test suite before submitting a pull request.
5. Avoid changing existing hooks, global Git identity settings, SSH configuration, or repository remotes as a side effect.
6. Route every `git` subprocess through `Runner::run_git` or `Runner::run_git_in`. They strip the environment variables that would redirect Git away from the selected repository; calling `run("git", ...)` directly reopens that hole.
7. `desktop/src/types.ts` is a hand-maintained mirror of the `serde` types in `src/api.rs`, `src/remote.rs`, and `src/check.rs`. Change a type that crosses the Tauri IPC boundary and you must update that file in the same commit.

## Running CI

Continuous integration runs automatically when a pull request is opened,
updated, or reopened. To re-run it without pushing a commit, add the `run-ci`
label; the workflow removes the label once it has consumed it.

Before submitting, run what CI runs:

```console
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo deny check
cd desktop && npm ci && npm run lint && npm run format:check && npm test && npm run build
```

## Security

Report vulnerabilities privately - see [SECURITY.md](SECURITY.md). A check that
reports `ok` when the identity is actually wrong is a security bug, not a
correctness nit.

All contributions are licensed under the MIT License.

