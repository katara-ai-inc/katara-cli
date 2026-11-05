# PROJECT KNOWLEDGE BASE

**Generated:** 2026-05-20
**Commit:** cb22439
**Branch:** master

## OVERVIEW

`katara-cli` is a single-binary Rust 2024 command-line client for the Katara API. It authenticates with an API key, reads/writes corpus documents, lists/fetches agents, and prints generated service-client names.

## STRUCTURE

```text
katara-cli/
├── Cargo.toml                 # single binary crate, edition 2024
├── Cargo.lock                 # committed lockfile; use locked Cargo commands
├── src/main.rs                # all CLI commands, clients, auth storage, unit tests
├── tests/e2e.rs               # integration tests that invoke the built binary
├── .cargo/config.toml         # Buf Cargo registry required by dependencies
├── .github/workflows/release.yml
│                                # tag-triggered build/release workflow
└── rust-toolchain.toml         # stable Rust toolchain
```

No `src/lib.rs`, workspace, `src/bin/`, examples, or benches. Keep root guidance centralized; `src/` and `tests/` are too small for child `AGENTS.md` files.

## WHERE TO LOOK

| Task | Location | Notes |
|------|----------|-------|
| Add or change CLI commands | `src/main.rs` | Clap args/enums at top, dispatch in `main`, command handlers below. |
| Auth behavior | `src/main.rs` | `auth_file_path`, `store_api_key`, `load_api_key`, `write_secret_file`, `mask_api_key`. |
| API client setup | `src/main.rs` | `authenticated_config`, `http_client`, `validate_api_uri`, service client helpers. |
| Document API commands | `src/main.rs` | `run_document`, `document_content`, `print_document`. |
| File uploads | `src/main.rs` | `DocumentUploadArgs`, `upload_request`, `content_uploader_client`; synchronous upload through a Brain or Content Uploader agent. |
| Agent API commands | `src/main.rs` | `run_agent`, `AgentTypeArg`, `AgentStateArg`, `print_agent`. |
| Help/CLI contract tests | `tests/e2e.rs` | Uses `CARGO_BIN_EXE_katara-cli`; help text is tested behavior. |
| Unit behavior tests | `src/main.rs` | `#[cfg(test)] mod tests` near the bottom. |
| Build/release automation | `.github/workflows/release.yml` | Runs on `v*` tags; action SHAs are pinned. |
| Cargo registry setup | `.cargo/config.toml` | Required because `Cargo.toml` uses `registry = "buf"`. |

## COMMANDS

```bash
cargo test --locked
cargo build --locked --release
cargo run -- --help
cargo run -- auth --help
cargo run -- document --help
cargo run -- agent --help
```

Release CI uses target-specific locked commands:

```bash
cargo test --locked --target <target-triple>
cargo build --locked --release --target <target-triple>
```

## CONVENTIONS

- Stable Rust only; `rust-toolchain.toml` pins `stable`.
- Treat `Cargo.lock` as authoritative for reproducible builds.
- Preserve `.cargo/config.toml`; use Buf's `/gen/cargo/public/` index for these public SDKs without a Buf token.
- Keep generated Connect clients and message crates on compatible runtime versions and the same Katara schema commit; the newest Buffa plugin alone may not match the published Connect plugin.
- API URL defaults to `https://api.katara.ai`.
- Plain `http://` API URLs are only allowed for localhost/loopback.
- API commands load the stored key and send it as a Bearer token.
- Auth storage uses `KATARA_AUTH_FILE` when set; otherwise platform defaults apply.
- Unix auth-file writes must remain `0600` and regular-file checked.
- Output is tab-oriented and terse; keep CLI output scriptable.
- `document upload` prints only the resulting document ID; file reads are bounded to the API's 20 MiB limit.
- Help text is part of the compatibility surface because integration tests assert it.

## TESTS

- Unit tests live in `src/main.rs` under `#[cfg(test)]`.
- Integration tests live in `tests/e2e.rs` and execute the compiled `katara-cli` binary.
- The Hello network/API e2e test is ignored by default:
  - requires `KATARA_E2E_API_KEY`
  - optionally uses `KATARA_E2E_API_URL`
  - default API URL is `https://api.katara.ai`
- The upload round-trip test is also ignored by default and writes a document left in the API for inspection. It requires `KATARA_E2E_API_KEY`, `KATARA_E2E_API_URL`, `KATARA_E2E_ORG_ID`, and `KATARA_E2E_AGENT_ID`; it uses temporary isolated auth storage and verifies CLI document readback.
- The default suite verifies upload protobuf bytes and metadata against a localhost server, including success, permission denial, and missing response IDs; it does not access a live API.
- Help tests deliberately assert that auth does not expose `oauth`, `passwordless`, or `refresh` commands.

## RELEASE NOTES

- Releases are created by pushing tags matching `v*`.
- Workflow builds four artifacts: Linux x64, macOS x64, macOS arm64, Windows x64.
- Archives are `.tar.gz` except Windows `.zip`.
- Each archive gets a `.sha256` checksum.
- Only the release job has `contents: write`; keep build jobs read-only.
- Third-party GitHub Actions are pinned to full commit SHAs. If upgrading, resolve and review new SHAs intentionally.

## ANTI-PATTERNS

- Do not print raw API keys or env-secret values.
- Do not remove `hide_env_values = true` from secret-bearing Clap args.
- Do not weaken Unix auth-file permissions below `0600`.
- Do not add non-localhost plaintext HTTP API access.
- Do not make ignored live-API tests run by default in CI.
- Do not commit generated build outputs: `target/`, `debug/`, `.rs.bk`, `.pdb`, `mutants.out*/`.
- Do not publish releases from broad tag patterns; keep `v*` unless the release policy changes.
