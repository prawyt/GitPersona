# Contributing to GitPersona

Thanks for helping make multiple-account Git workflows safer.

1. Open an issue for significant behavioral or configuration-format changes.
2. Keep authentication delegated to GitHub CLI, the operating-system credential store, or OpenSSH. Code that reads or stores tokens is out of scope.
3. Add tests for changes to profile validation, Git configuration, remote parsing, account checks, or hook behavior.
4. Run formatting, Clippy with warnings denied, and the complete test suite before submitting a pull request.
5. Avoid changing existing hooks, global Git identity settings, SSH configuration, or repository remotes as a side effect.

All contributions are licensed under the MIT License.

