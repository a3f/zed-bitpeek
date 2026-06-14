# Repository Guidelines

## Project Structure & Module Organization

BitPeek is a Zed extension with a small Rust wrapper and a bundled Node language server.

- `src/bitpeek.rs`: Zed extension entry point. It launches the bundled server and forwards LSP settings.
- `language_server/src/server.js`: language-server implementation for hover text and code actions.
- `language_server/dist/server.cjs`: generated bundle loaded by the extension. Rebuild and commit it when `server.js` changes.
- `extension.toml`: extension metadata, supported languages, and language-server registration.
- `screenshots/`: README images. Keep filenames descriptive, for example `hex-1bit-set-hover.png`.

There is no dedicated test directory at present.

## Build, Test, and Development Commands

- `cd language_server && npm install`: install language-server dependencies.
- `cd language_server && npm run build`: bundle `src/server.js` into `dist/server.cjs`.
- `node --check language_server/src/server.js`: syntax-check the source server.
- `node --check language_server/dist/server.cjs`: syntax-check the generated bundle.
- `CARGO_TARGET_DIR=/tmp/zed-bitpeek-target cargo check`: compile-check the Rust extension without depending on the repo-local target directory.
- `git diff --check`: catch whitespace errors before committing.

## Coding Style & Naming Conventions

Use Rust 2024 idioms in `src/bitpeek.rs` and keep extension callbacks small. JavaScript is ESM in source and bundled to CommonJS for Zed. Use two-space indentation in JS and four-space indentation in Rust. Prefer clear helper names such as `formattedHoverValue`, `parseNumberAtPosition`, and `macroReplacementActions`.

Do not hand-edit `language_server/dist/server.cjs`; change `language_server/src/server.js` and rebuild.

## Testing Guidelines

There is no formal automated test suite. For language-server behavior, add focused smoke checks with `node --input-type=module -e '...'` when practical, and verify both source and bundle with `node --check`. For extension changes, run `cargo check`. If hover formatting changes, inspect README screenshots or update them alongside the behavior.

## Commit Guidelines

Follow the existing kernel-style subject format: `area: concise imperative summary`, for example `server: add group spacing setting` or `docs: add AGENTS.md`.

Keep commit subject and body lines under 78 characters. AI-assisted commits are identified by a

```text
Assisted-by: Codex:gpt-5.5
```

Trailer.
