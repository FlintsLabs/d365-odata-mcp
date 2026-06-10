# D365 OData MCP Server

[![Crates.io](https://img.shields.io/crates/v/d365-odata-mcp.svg)](https://crates.io/crates/d365-odata-mcp)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

An MCP (Model Context Protocol) server that enables AI assistants to query **Microsoft Dynamics 365** data via OData API. Supports both **Dataverse** and **Finance & Operations (F&O)**.

## For AI Assistants

If you want an AI assistant to understand this repository before helping with changes, ask it to read [`AI_CONTEXT.md`](AI_CONTEXT.md) first. That file gives a compact overview of the architecture, runtime flow, MCP tools, safety rules, and release process.

## Features

- ✅ Full OData query support: `$filter`, `$select`, `$orderby`, `$top`, `$skip`, `$expand`, `$count`
- ✅ Cross-company queries (F&O)
- ✅ **Azure AD** authentication (Cloud D365)
- ✅ **ADFS** authentication (On-premise D365)
- ✅ Automatic token refresh
- ✅ Retry with exponential backoff
- ✅ **Metadata caching** with configurable TTL for improved performance
- ✅ Works with OpenAI Codex, Claude Desktop, Claude Code, and other MCP clients

---

## Quick Start

### Step 1: Install

```bash
cargo install d365-odata-mcp
```

### Upgrade to Latest Version

```bash
cargo install d365-odata-mcp --force
```

### Step 2: Create Azure AD App

1. Go to [Azure Portal](https://portal.azure.com)
2. Navigate to **Microsoft Entra ID** → **App registrations** → **New registration**
3. Name your app (e.g., `D365 MCP`)
4. Note down:
   - **Tenant ID** (from Overview)
   - **Client ID** (Application ID)
5. Go to **Certificates & secrets** → **New client secret** → Copy the **Secret Value**
6. Go to **API permissions** → **Add a permission**:
   - For **Dataverse**: `Dynamics CRM` → `user_impersonation`
   - For **F&O**: `Dynamics ERP` → `CustomService.FullAccess`
7. Click **Grant admin consent**

### Step 3: Configure Your AI Client

Choose your AI client below:

---

## Configuration for OpenAI Codex

Edit `~/.codex/config.toml`:

```toml
[mcp_servers.d365]
command = "d365-odata-mcp"

[mcp_servers.d365.env]
TENANT_ID = "your-tenant-id"
CLIENT_ID = "your-client-id"
CLIENT_SECRET = "your-client-secret"
ENDPOINT = "https://your-org.crm.dynamics.com/api/data/v9.2/"
PRODUCT = "dataverse"
```

### Store `CLIENT_SECRET` in the OS Secret Store

By default, `d365-odata-mcp` reads `CLIENT_SECRET` from the MCP client's environment config. To keep the secret out of `~/.codex/config.toml`, enable native secret store lookup:

```toml
[mcp_servers.d365]
command = "d365-odata-mcp"

[mcp_servers.d365.env]
TENANT_ID = "your-tenant-id"
CLIENT_ID = "your-client-id"
USE_KEYCHAIN = "true"
CLIENT_SECRET_KEYCHAIN_SERVICE = "bio-dataverse-sales-uat-client-secret"
ENDPOINT = "https://your-org.crm.dynamics.com/api/data/v9.2/"
PRODUCT = "dataverse"
```

Secret lookup uses this pair:

```text
service = CLIENT_SECRET_KEYCHAIN_SERVICE
account = CLIENT_SECRET_KEYCHAIN_ACCOUNT, or CLIENT_ID when account is omitted
```

On macOS, create or update the Keychain item with:

```bash
security add-generic-password \
  -a "<CLIENT_ID>" \
  -s "bio-dataverse-sales-uat-client-secret" \
  -w "<CLIENT_SECRET>" \
  -U
```

If your Keychain item uses a different account name, set `CLIENT_SECRET_KEYCHAIN_ACCOUNT` explicitly:

```toml
USE_KEYCHAIN = "true"
CLIENT_SECRET_KEYCHAIN_SERVICE = "bio-dataverse-sales-uat-client-secret"
CLIENT_SECRET_KEYCHAIN_ACCOUNT = "bio-sales-uat"
```

**For F&O:**
```toml
[mcp_servers.d365]
command = "d365-odata-mcp"

[mcp_servers.d365.env]
TENANT_ID = "your-tenant-id"
CLIENT_ID = "your-client-id"
CLIENT_SECRET = "your-client-secret"
ENDPOINT = "https://your-org.sandbox.operations.dynamics.com/data/"
PRODUCT = "finops"
```

**Verify installation:**
```bash
codex mcp list
```

---

## Configuration for Claude Desktop

Edit `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS) or `%APPDATA%\Claude\claude_desktop_config.json` (Windows):

```json
{
  "mcpServers": {
    "d365": {
      "command": "d365-odata-mcp",
      "env": {
        "TENANT_ID": "your-tenant-id",
        "CLIENT_ID": "your-client-id",
        "CLIENT_SECRET": "your-client-secret",
        "ENDPOINT": "https://your-org.crm.dynamics.com/api/data/v9.2/",
        "PRODUCT": "dataverse"
      }
    }
  }
}
```

---

## Configuration for Gemini (Antigravity)

Add workflow file `.agent/workflows/d365-query.md` to your project:

```markdown
---
description: How to query D365 Finance & Operations data via d365-odata-mcp
---

# D365 OData Query

Query D365 data via command line:

\```bash
export TENANT_ID="your-tenant-id"
export CLIENT_ID="your-client-id"
export CLIENT_SECRET="your-client-secret"
export ENDPOINT="https://your-org.sandbox.operations.dynamics.com/data/"
export PRODUCT="finops"

echo '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"query_entity","arguments":{"entity":"CustomersV3","top":"10"}}}' | d365-odata-mcp 2>/dev/null | jq '.result.content[0].text' -r
\```
```

---

## Available Tools

### 1. `list_entities`
List all available D365 entities:
```
"List all D365 entities"
```

### 2. `query_entity`
Query data with full OData support:

| Parameter | Description | Required |
|-----------|-------------|----------|
| `entity` | Entity name, e.g., `CustomersV3` | ✅ |
| `filter` | OData filter, e.g., `dataAreaId eq 'bc'` | ❌ |
| `select` | Fields to return, e.g., `Name,Id` | ❌ |
| `orderby` | Sort order, e.g., `CreatedDate desc` | ❌ |
| `top` | Max records (default: 50, max: 1000) | ❌ |
| `skip` | Records to skip (pagination) | ❌ |
| `expand` | Navigation properties to expand | ❌ |
| `cross_company` | `true` for cross-company (F&O only) | ❌ |
| `count` | `true` to include total count | ❌ |

**Examples:**
```
"Query CustomersV3, show first 10 records"
"Query SalesOrderHeaders where dataAreaId is 'bc', order by SalesOrderNumber desc"
"Get inventory where warehouse is 'WH01' with count"
```

### 3. `get_entity_schema`
Get available fields for an entity:
```
"Show schema for SalesOrderHeaders"
```

### 4. `get_record`
Get a single record by ID:
```
"Get customer record with ID 'CUS-001'"
```

### 5. `delete_record`
Delete a single record by OData key. This tool requires `confirm` to be exactly `DELETE`.

| Parameter | Description | Required |
|-----------|-------------|----------|
| `entity` | Entity name, e.g., `CustomersV3` | ✅ |
| `key` | OData key expression without parentheses, e.g., `dataAreaId='bc',CustomerAccount='CUS-001'` | ❌ |
| `id` | Simple record ID/key, used when `key` is not provided | ❌ |
| `if_match` | Optional `If-Match` header value (default: `*`) | ❌ |
| `confirm` | Must be exactly `DELETE` | ✅ |

**Example:**
```
"Delete CustomersV3 with key dataAreaId='bc',CustomerAccount='CUS-001' and confirm DELETE"
```

### 6. `get_environment_info`
Get D365 environment information:
```
"Show D365 environment info"
```

### 7. `get_metadata`
Get entity metadata including properties and navigation properties (expandable fields):
```
"Get metadata for CustomersV3"
"Show me the schema and expandable fields for SalesOrderHeaders"
```

### 8. `refresh_metadata`
Force refresh the cached metadata (useful when schema changes):
```
"Refresh metadata cache"
```

---

## Environment Variables

| Variable | Description | Required |
|----------|-------------|----------|
| `TENANT_ID` | Azure AD Tenant ID (or `adfs` for ADFS) | ✅ |
| `CLIENT_ID` | Azure AD/ADFS Application ID | ✅ |
| `CLIENT_SECRET` | Azure AD/ADFS Client Secret | ✅ unless `USE_KEYCHAIN=true` |
| `ENDPOINT` | D365 OData endpoint URL | ✅ |
| `PRODUCT` | `dataverse` or `finops` | ✅ |
| `AUTH_TYPE` | `azure` (default) or `adfs` | ❌ |
| `TOKEN_URL` | Custom token URL (ADFS only) | ❌ |
| `RESOURCE` | Resource/audience (ADFS only) | ❌ |
| `METADATA_CACHE_TTL` | Metadata cache TTL in seconds (default: 900 = 15 min) | ❌ |
| `INSECURE_SSL` | Skip SSL verification for self-signed certs (`true`/`false`) | ❌ |
| `USE_KEYCHAIN` | Read `CLIENT_SECRET` from the OS native secret store (`true`/`false`, default `false`) | ❌ |
| `CLIENT_SECRET_KEYCHAIN_SERVICE` | Secret store service name used when `USE_KEYCHAIN=true` | ✅ when `USE_KEYCHAIN=true` |
| `CLIENT_SECRET_KEYCHAIN_ACCOUNT` | Secret store account name; defaults to `CLIENT_ID` when omitted | ❌ |

---

## Configuration for On-Premise D365 (ADFS)

For D365 F&O on-premise with ADFS authentication:

```toml
# ~/.codex/config.toml

[mcp_servers.d365_onprem]
command = "d365-odata-mcp"

[mcp_servers.d365_onprem.env]
AUTH_TYPE = "adfs"
TENANT_ID = "adfs"
CLIENT_ID = "your-adfs-client-id"
CLIENT_SECRET = "your-adfs-secret"
TOKEN_URL = "https://your-adfs-server.com/adfs/oauth2/token"
RESOURCE = "https://your-d365-onprem.com"
ENDPOINT = "https://your-d365-onprem.com/namespaces/AXSF/data/"
PRODUCT = "finops"
```

---

## Common F&O Entities

| Entity | Description |
|--------|-------------|
| `CustomersV3` | Customer master data |
| `VendorsV2` | Vendor master data |
| `ProductsV2` | Product master data |
| `SalesOrderHeaders` | Sales order headers |
| `SalesOrderLines` | Sales order lines |
| `PurchaseOrderHeaders` | Purchase order headers |
| `PurchaseOrderLines` | Purchase order lines |
| `InventoryOnHandAggregatedByWarehouse` | Inventory on hand |

---

## OData Filter Syntax

| Operator | Example |
|----------|---------|
| `eq` | `Status eq 'Open'` |
| `ne` | `Status ne 'Closed'` |
| `gt` / `ge` | `Amount gt 1000` |
| `lt` / `le` | `Amount lt 100` |
| `and` / `or` | `Status eq 'Open' and Amount gt 100` |
| `contains` | `contains(Name, 'Corp')` |
| `startswith` | `startswith(Name, 'ABC')` |

---

## Testing

Test the server directly:
```bash
export TENANT_ID="..." CLIENT_ID="..." CLIENT_SECRET="..." ENDPOINT="..." PRODUCT="finops"
echo '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | d365-odata-mcp
```

---

## License

MIT License - see [LICENSE](LICENSE) for details.

---

## Contributing

Contributions welcome! Please open an issue or submit a PR.
