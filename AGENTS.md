# Repository Guidelines

## Project Structure & Module Organization

BitPeek is a Zed extension with an integrated Rust language server.

- `src/bitpeek.rs`: Zed extension entry point (WASM). Launches the bundled
  Rust LSP binary and forwards LSP settings.
- `src/server.rs`: native Rust binary implementing the language server
  (hover text and code actions over stdio). This replaces the former
  Node.js server.
- `extension.toml`: extension metadata, supported languages, and
  language-server registration.
- `screenshots/`: README images. Keep filenames descriptive, for example
  `hex-1bit-set-hover.png`.
- `language_server/`: (deprecated) former Node.js server. No longer used
  by the extension; kept for reference only.

There is no dedicated test directory at present.

## Build, Test, and Development Commands

- `CARGO_TARGET_DIR=/tmp/zed-bitpeek-target cargo check`:
  compile-check both the native LSP binary and the WASM extension lib
  on the host target (the binary is the primary thing to check).
- `CARGO_TARGET_DIR=/tmp/zed-bitpeek-target cargo check --lib --target wasm32-unknown-unknown`:
  compile-check only the WASM extension.
- `CARGO_TARGET_DIR=/tmp/zed-bitpeek-target cargo check --bin bitpeek-ls`:
  compile-check only the native LSP binary.
- `cargo build --bin bitpeek-ls --release`:
  build the LSP binary for local testing (the extension expects it at
  `target/release/bitpeek-ls`).
- `git diff --check`: catch whitespace errors before committing.

## Coding Style & Naming Conventions

Use Rust 2024 idioms in `src/bitpeek.rs` and keep extension callbacks
small. Use four-space indentation in Rust. Prefer clear helper names
such as `formatted_hover_value`, `parse_number_at_position`, and
`macro_replacement_actions`.

The `src/server.rs` binary uses `#[cfg(not(target_arch = "wasm32"))]`
so it only compiles on native targets; the extension lib is gated with
`#[cfg(target_arch = "wasm32")]`.

## Testing Guidelines

There is no formal automated test suite. For language-server behavior,
build the binary and run it under a manual LSP session or test harness.
For extension changes, run `cargo check`. If hover formatting changes,
inspect README screenshots or update them alongside the behavior.

## Commit Guidelines

Follow the existing kernel-style subject format:
`area: concise imperative summary`, for example
`server: add group spacing setting` or `docs: add AGENTS.md`.

Keep commit subject and body lines under 78 characters.
AI-assisted commits are identified by a

```text
Assisted-by: Codex:gpt-5.5
```

Trailer.
