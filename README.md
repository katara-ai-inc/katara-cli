# katara-cli

Command-line client for the Katara API. Use it to authenticate with an API key, read and write corpus documents, inspect agents, and list the generated Katara service clients exposed by the CLI.

## Features

- Store a Katara API key locally without printing it back to the terminal.
- Fetch, list, create, and update corpus documents.
- Upload local files through a Brain or Content Uploader agent.
- Fetch and list agents, with filters for organization, ID, type, and state.
- List generated Katara API service clients.
- Build a single native binary with Cargo.

## Installation

### Prebuilt binaries (recommended for agents)

Download an archive and its matching `.sha256` file from the same
[GitHub Release](https://github.com/katara-ai-inc/katara-cli/releases). No Rust
toolchain is needed to run a prebuilt binary.

| Platform | Archive |
| --- | --- |
| Linux x64 (glibc 2.35+) | `katara-cli-x86_64-unknown-linux-gnu.tar.gz` |
| Linux ARM64 (glibc 2.35+) | `katara-cli-aarch64-unknown-linux-gnu.tar.gz` |
| macOS Intel | `katara-cli-x86_64-apple-darwin.tar.gz` |
| macOS Apple Silicon | `katara-cli-aarch64-apple-darwin.tar.gz` |
| Windows x64 | `katara-cli-x86_64-pc-windows-msvc.zip` |

1. Inspect the host OS/architecture and select a published release containing both files.
2. Download only from this repository's GitHub Releases. Verify the archive checksum
   with `sha256sum -c <archive>.sha256` (Linux), `shasum -a 256 -c <archive>.sha256`
   (macOS), or `Get-FileHash -Algorithm SHA256` (PowerShell).
3. Extract the single `katara-cli` / `katara-cli.exe` binary and install in a
   user-writable directory. Do not use `sudo`, overwrite another installation, or
   edit persistent PATH settings without authorization.
4. Run `katara-cli --help` and `katara-cli document upload --help`. Stop if the
   release lacks upload support; do not silently install an older incompatible CLI.

If the release has no matching assets, report that it is not installable rather
than constructing another download host or claiming installation succeeded.
Linux binaries are dynamically linked and support glibc 2.35+ (Ubuntu 22.04 or
equivalent) with the normal `libgcc_s.so.1` runtime. CI checks ELF requirements
against this baseline. Older glibc, Alpine/musl, Windows ARM64, and other platforms
need a separately supported build. For a user-local installation, use the full
installed path or configure PATH in the shell that will run the commands; an
installer subprocess cannot change its parent shell's PATH.

### Agent Skill

Install the portable [Katara Skill](https://github.com/katara-ai-inc/katara-skills/tree/main/skills/katara)
for agent-guided CLI installation, user-created API keys, explicit upload targets,
and MCP knowledge workflows. Installing the skill does not install the CLI or grant
access. Have the user create their own least-privilege API key in Katara, then enter
it locally with `katara-cli auth login`; never put it in chat or an installer.

### Build from source

Source builds require current stable Rust (edition 2024; the RPC SDK requires Rust 1.88 or newer).
The checked-in Cargo configuration uses Buf's public registry; no Buf token is needed.

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

`auth login --api-url` does not persist the endpoint. For staging, set
`KATARA_API_URL=https://api.stage.katara.ai` for subsequent commands, or pass
`--api-url https://api.stage.katara.ai` to each API command. Use a separate
`KATARA_AUTH_FILE` when switching environments to avoid replacing your normal credential.

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

Upload a local file (prints the resulting document ID):

```bash
katara-cli document upload \
  --api-url https://api.stage.katara.ai \
  --org-id <org-id> \
  --agent-id <brain-or-content-uploader-agent-id> \
  --file ./example.txt \
  --content-type text/plain
```

Files must be nonempty and no larger than 20 MiB. Only the basename, not the local
path, is sent to the API. `--content-type` is optional; the API detects it when omitted.
The synchronous Content Uploader path supports text/Markdown; Brain uploads use the
server's configured conversion providers. Availability and supported binary formats
depend on the server. The CLI does not create agents or change permissions.

Optional metadata: `--source-id`, `--language`, repeated `--tag-id`, and
`--idempotency-key`. For a Brain upload retry, reuse the same key and unchanged
file/metadata; do not reuse a key for a different intent. The API defaults the source
ID to the filename if omitted. Verify the returned ID with `document get`.

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

Agent filters follow the current API, including `brain`. Retired taxonomy and
classifier types are no longer accepted.

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

The upload test is opt-in, **writes a document**, and leaves it in the API for
inspection. To run only that test against staging, provide the key through the
environment (never commit it), plus an authorized organization and agent:

```bash
export KATARA_E2E_API_URL=https://api.stage.katara.ai
export KATARA_E2E_ORG_ID=<org-id>
export KATARA_E2E_AGENT_ID=<brain-or-content-uploader-agent-id>
cargo test --locked --test e2e cli_file_upload_round_trips -- --ignored --exact --nocapture
```

It invokes the actual CLI for login, file upload, and document readback, using a
temporary auth file that is removed afterward. Missing required variables fail the
test rather than silently reporting a pass. Running all ignored tests requires these
upload variables as well as the Hello service's permission.

## Releases

GitHub Actions builds and publishes releases when a tag matching `v*` is pushed:

```bash
git tag v0.1.0
git push origin v0.1.0
```

The release workflow runs locked tests, native binary smoke checks, and release
builds on all five platforms before publishing anything:

- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `x86_64-apple-darwin`
- `aarch64-apple-darwin`
- `x86_64-pc-windows-msvc`

Each archive contains one binary and has a matching `.sha256` checksum. A final
verification job checks that every platform archive and checksum is present and
valid before attaching them to the GitHub Release for the exact pushed tag
(creating that release if needed). Build jobs are read-only; only publication has
`contents: write`. Pinned actions and the public Buf registry require no Buf token.

Relevant pull requests exercise the same build matrix and upload workflow
artifacts, but **never create or modify a GitHub Release**. Pushing a tag is an
explicit publication action; neither opening a PR nor installing the skill tags a
release. Existing tags/releases are not repaired automatically by a workflow edit.

Packaging regressions can be checked locally with Python 3.11+:

```bash
python3 -m unittest discover -s scripts -p 'test_*.py' -v
```
