# D365 OData MCP Server

MCP (Model Context Protocol) Server สำหรับ Microsoft Dynamics 365 (Dataverse & Finance/Operations) ผ่าน OData API

## 🚀 Quick Start

### 1. Build

```bash
cargo build --release
```

Binary จะอยู่ที่ `target/release/d365-odata-mcp`

### 2. Configure Environment Variables

```bash
export TENANT_ID="your-azure-tenant-id"
export CLIENT_ID="your-app-client-id"
export CLIENT_SECRET="your-client-secret"
export ENDPOINT="https://yourorg.crm.dynamics.com/api/data/v9.2/"
export PRODUCT="dataverse"  # or "finops"
```

### 3. Run

```bash
./target/release/d365-odata-mcp
```

---

## 🔧 Claude Desktop / Cursor / Codex Setup

Edit config file:
- **Claude Desktop**: `~/Library/Application Support/Claude/claude_desktop_config.json`
- **Codex CLI**: `~/.codex/config.json`

```json
{
  "mcpServers": {
    "d365-odata": {
      "command": "/path/to/d365-odata-mcp",
      "env": {
        "TENANT_ID": "your-tenant-id",
        "CLIENT_ID": "your-client-id",
        "CLIENT_SECRET": "your-client-secret",
        "ENDPOINT": "https://yourorg.crm.dynamics.com/api/data/v9.2/",
        "PRODUCT": "dataverse"
      }
    }
  }
}
```

---

## 📋 Available Tools

| Tool | Description |
|------|-------------|
| `list_entities` | List all available D365 entities/tables |
| `query_entity` | Query data from an entity with filters |
| `get_entity_schema` | Get entity fields by viewing sample record |
| `get_record` | Get single record by ID |
| `get_environment_info` | Get connected environment info |

### Example Prompts

```
"List all entities available in D365"
"Query the contacts entity, show me first 10 records"
"Get schema for the accounts entity"
"Find contact with ID abc123-def456..."
```

---

## 🔐 Azure AD Setup

1. ไปที่ [Azure Portal](https://portal.azure.com) → Azure Active Directory → App Registrations
2. สร้าง App Registration ใหม่
3. ไปที่ Certificates & secrets → New client secret
4. ไปที่ API permissions → Add permission:
   - **Dataverse**: `Dynamics CRM` → `user_impersonation`
   - **F&O**: `Dynamics ERP` → `Odata.ReadWrite.All`
5. Grant admin consent

---

## 📁 API Endpoints

| Product | Endpoint Format |
|---------|-----------------|
| Dataverse | `https://org.crm.dynamics.com/api/data/v9.2/` |
| Finance & Ops | `https://org.operations.dynamics.com/data/` |

---

## 🧪 Testing

```bash
# Test with OpenAI Codex CLI
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | ./target/release/d365-odata-mcp

# Test tools/list
echo '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' | ./target/release/d365-odata-mcp
```

---

## 📜 License

MIT
