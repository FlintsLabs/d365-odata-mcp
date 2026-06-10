# AI Context for d365-odata-mcp

Use this file when you want an AI assistant to understand this repository quickly before making changes or answering implementation questions.

## Project Purpose

`d365-odata-mcp` is a Rust-based Model Context Protocol server for Microsoft Dynamics 365 OData APIs.

It exposes D365 data access as MCP tools so AI clients can query Dataverse and Dynamics 365 Finance and Operations through a standard stdio JSON-RPC MCP interface.

Supported products:

- Dataverse
- Dynamics 365 Finance and Operations
- Cloud Azure AD authentication
- On-premise ADFS authentication

## High-Level Runtime Flow

```text
AI client
  -> MCP stdio JSON-RPC
  -> src/main.rs
  -> src/mcp/server.rs
  -> src/odata/client.rs
  -> D365 OData endpoint
```

The binary reads JSON-RPC messages from stdin and writes JSON-RPC responses to stdout.

The server can start even when required D365 environment variables are missing. In that state it still responds to `initialize` and `tools/list`, but actual tool calls return a configuration error.

## Important Files

| File | Purpose |
| --- | --- |
| `src/main.rs` | Binary entrypoint, MCP stdio loop, JSON-RPC request dispatch |
| `src/lib.rs` | Library module exports |
| `src/mcp/protocol.rs` | MCP and JSON-RPC structs |
| `src/mcp/server.rs` | Tool definitions and tool-call handlers |
| `src/odata/client.rs` | OData HTTP client, query building, metadata cache, delete support |
| `src/auth/mod.rs` | Azure AD and ADFS OAuth2 client credentials flow |
| `src/config/config.rs` | TOML and environment-based runtime config |
| `config/default.toml` | Example/default config |
| `README.md` | User-facing quick start and basic tool reference |
| `.agent/workflows/d365-query.md` | Antigravity/Gemini workflow examples |

## MCP Tools

Current tools are defined in `src/mcp/server.rs`.

| Tool | Purpose |
| --- | --- |
| `list_entities` | Fetch `$metadata` and list entity sets |
| `query_entity` | Query one page of records with OData query options |
| `get_entity_schema` | Fetch one sample record and list returned fields |
| `get_record` | Fetch one record by OData key |
| `delete_record` | Delete one record by OData key; requires `confirm=DELETE` |
| `get_environment_info` | Show endpoint/product/config summary |
| `get_metadata` | Parse `$metadata` for fields and navigation properties |
| `refresh_metadata` | Invalidate and refetch metadata cache |

## Configuration Model

Required environment variables:

```text
TENANT_ID
CLIENT_ID
ENDPOINT
PRODUCT
```

`CLIENT_SECRET` is required by default. If `USE_KEYCHAIN=true`, the binary reads the secret from the OS native secret store instead of requiring `CLIENT_SECRET`.

Optional environment variables:

```text
AUTH_TYPE
TOKEN_URL
RESOURCE
METADATA_CACHE_TTL
INSECURE_SSL
USE_KEYCHAIN
CLIENT_SECRET_KEYCHAIN_SERVICE
CLIENT_SECRET_KEYCHAIN_ACCOUNT
```

Environment variables override file config. Runtime config is resolved in `Config::to_runtime`.

Secret-store lookup uses `CLIENT_SECRET_KEYCHAIN_SERVICE` as the service and `CLIENT_SECRET_KEYCHAIN_ACCOUNT` as the account. If account is omitted, it defaults to `CLIENT_ID`. On macOS this maps to a generic password item that can be created with `security add-generic-password -a "<CLIENT_ID>" -s "<service>" -w "<CLIENT_SECRET>" -U`.

## Authentication

Authentication lives in `src/auth/mod.rs`.

Azure AD mode:

- `AUTH_TYPE=azure` or omitted
- token endpoint is `https://login.microsoftonline.com/{tenant}/oauth2/v2.0/token`
- uses `scope={resource}/.default`

ADFS mode:

- `AUTH_TYPE=adfs`
- uses `resource` instead of `scope`
- `TOKEN_URL` and `RESOURCE` may be provided explicitly

Access tokens are cached until close to expiry.

## OData Client Behavior

`src/odata/client.rs` owns HTTP behavior.

Key points:

- endpoint is normalized to end with `/`
- requests use bearer token authentication
- retry behavior handles `429` and server errors
- metadata is cached in memory with a configurable TTL
- `query_entity` currently fetches one page, not all pages
- `fetch_all_pages` exists but is not currently exposed as a tool

## Delete Behavior

`delete_record` is intentionally not a raw HTTP tool.

It requires:

```json
{
  "confirm": "DELETE"
}
```

The tool accepts either:

- `key`: full OData key expression without parentheses, preferred for composite keys
- `id`: simple key value, used only when `key` is not provided

Default delete requests include:

```text
If-Match: *
```

This can be overridden with `if_match`.

Do not remove the confirmation guard unless the user explicitly asks for a less safe destructive interface.

## Known Tradeoffs

- Query strings are assembled manually in `QueryOptions::to_query_string`; be careful with special characters.
- Metadata parsing is simple line-based XML parsing, not a full XML parser.
- `query_entity` caps `top` at 1000 and returns one page.
- `get_entity_schema` depends on a sample record, so empty entities return no field list.
- The server writes debug logs to `/tmp/d365-mcp.log`.

## Release Notes for Maintainers

Releases are published through GitHub Releases.

Typical flow:

```bash
cargo fmt --check
cargo check
cargo test
cargo package --allow-dirty
git tag -a vx.y.z -m "vx.y.z"
git push origin main
git push origin vx.y.z
gh release create vx.y.z --title "vx.y.z" --notes "..."
```

The GitHub workflow publishes to crates.io with Trusted Publishing when a release is published. The crates.io trusted publisher configuration must match owner `FlintsLabs`, repository `d365-odata-mcp`, and workflow filename `publish.yml`.

## Guidance for AI Assistants

Before changing behavior:

1. Read `src/mcp/server.rs` to understand tool surface area.
2. Read `src/odata/client.rs` to understand HTTP behavior.
3. Read `src/auth/mod.rs` if touching authentication.
4. Run `cargo fmt --check`, `cargo check`, and `cargo test` after Rust changes.
5. Do not commit generated `target/` build output.

If you need to explain this repo to a user, summarize it as:

> A Rust MCP server that lets AI clients query and safely operate on D365 OData endpoints using Azure AD or ADFS authentication.
