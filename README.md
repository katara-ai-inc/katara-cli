# katara-cli

Command-line client for the Katara API. Use it to authenticate with an API key, read and write corpus documents, inspect agents, and list the generated Katara service clients exposed by the CLI.

## Features

- Store a Katara API key locally without printing it back to the terminal.
- Fetch, list, create, and update corpus documents.
- Fetch and list agents, with filters for organization, ID, type, and state.
- List generated Katara API service clients.
- Build a single native binary with Cargo.

## Installation

Download a release archive for your platform from GitHub Releases, or build from source with Rust.

```bash
git clone https://github.com/katara-ai-inc/katara-cli.git
cd katara-cli
cargo install --path . --locked
```

To build without installing:

```bash
cargo build --locked --release
```

The binary is written to `target/release/katara-cli` on Unix-like systems and `target/release/katara-cli.exe` on Windows.

## Authentication

The CLI uses `https://api.katara.ai` by default. Store an API key before running API-backed commands:

```bash
katara-cli auth login
```

You can also provide the key non-interactively through `--api-key` or `KATARA_API_KEY`:

```bash
KATARA_API_KEY=<api-key> katara-cli auth login
```

List stored credentials without revealing the full key:

```bash
katara-cli auth list
```

Useful environment variables:

- `KATARA_API_URL`: override the API endpoint, defaulting to `https://api.katara.ai`.
- `KATARA_API_KEY`: provide an API key for `auth login`.
- `KATARA_AUTH_FILE`: override the local auth JSON file path.

## Usage

Show the top-level command list:

```bash
katara-cli --help
```

### Documents

List documents in an organization:

```bash
katara-cli document list --org-id <org-id>
```

Fetch a document:

```bash
katara-cli document get --org-id <org-id> --id <document-id>
```

Create a text document:

```bash
katara-cli document create \
  --org-id <org-id> \
  --content "Hello from katara-cli" \
  --content-format text
```

Update an existing document:

```bash
katara-cli document update \
  --org-id <org-id> \
  --id <document-id> \
  --content "Updated content" \
  --content-format text
```

Document content can be sent as `text`, `markdown`, or `json` with `--content-format`.

### Agents

List agents:

```bash
katara-cli agent list
```

Filter agents by organization, type, and state:

```bash
katara-cli agent list \
  --org-id <org-id> \
  --agent-type website \
  --state enabled
```

Fetch an agent by ID:

```bash
katara-cli agent get --id <agent-id>
```

### Services

List generated service clients exposed by the CLI:

```bash
katara-cli services
```

## Development

This repository is a single Rust binary crate. The implementation lives in `src/main.rs`; integration tests live in `tests/e2e.rs`.

Run the test suite:

```bash
cargo test --locked
```

Build a release binary:

```bash
cargo build --locked --release
```

The live API integration test is ignored by default. To run it explicitly, provide `KATARA_E2E_API_KEY` and optionally `KATARA_E2E_API_URL`:

```bash
KATARA_E2E_API_KEY=<api-key> cargo test --locked -- --ignored
```

## Releases

GitHub Actions builds and publishes releases when a tag matching `v*` is pushed:

```bash
git tag v0.1.0
git push origin v0.1.0
```

The release workflow runs locked tests and release builds for:

- `x86_64-unknown-linux-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`
- `x86_64-pc-windows-msvc`

Each release asset includes a platform archive plus a `.sha256` checksum.
