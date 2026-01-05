---
description: How to query D365 Finance & Operations data via d365-odata-mcp
---

# D365 OData MCP Workflow

This workflow allows Antigravity to query Microsoft Dynamics 365 Finance & Operations data via the `d365-odata-mcp` command-line tool.

## Prerequisites

1. **Install d365-odata-mcp:**
   ```bash
   cargo install d365-odata-mcp
   ```

2. **Environment Variables (set in shell or Codex config):**
   - `TENANT_ID` - Azure AD tenant ID
   - `CLIENT_ID` - Azure AD client/app ID
   - `CLIENT_SECRET` - Azure AD client secret
   - `ENDPOINT` - D365 OData endpoint URL (e.g., `https://org.sandbox.operations.dynamics.com/data/`)
   - `PRODUCT` - `finops` for F&O, `dataverse` for Dataverse

---

## Available Tools

### 1. list_entities
List all available D365 entities/tables:
```bash
echo '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"list_entities","arguments":{}}}' | d365-odata-mcp 2>/dev/null
```

### 2. query_entity (Full OData Support)
Query data from a D365 entity with full OData parameters:

**Parameters:**
| Param | Description | Required |
|-------|-------------|----------|
| `entity` | Entity set name, e.g., `CustomersV3` | ✅ |
| `select` | Comma-separated fields, e.g., `Name,Id,Status` | ❌ |
| `filter` | OData filter, e.g., `dataAreaId eq 'bc'` | ❌ |
| `orderby` | Sort order, e.g., `CreatedDate desc` | ❌ |
| `top` | Max records (default: 50, max: 1000) | ❌ |
| `skip` | Skip N records (pagination) | ❌ |
| `expand` | Navigation properties to expand | ❌ |
| `cross_company` | `true` for cross-company (F&O only) | ❌ |
| `count` | `true` to include total count | ❌ |

**Examples:**

```bash
# Basic query
echo '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"query_entity","arguments":{"entity":"CustomersV3","top":"10"}}}' | d365-odata-mcp 2>/dev/null

# With filter and orderby
echo '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"query_entity","arguments":{"entity":"SalesOrderHeaders","filter":"dataAreaId eq '\''bc'\''","orderby":"SalesOrderNumber desc","top":"5"}}}' | d365-odata-mcp 2>/dev/null

# Cross-company with count
echo '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"query_entity","arguments":{"entity":"rvl_DE_SummaryPickingTrade","filter":"dataAreaId eq '\''bc'\'' and rvl_TradeDocument ne '\''Packed'\''","orderby":"rvl_ApproveTradeDate desc","cross_company":"true","count":"true","top":"10"}}}' | d365-odata-mcp 2>/dev/null

# With select (specific fields)
echo '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"query_entity","arguments":{"entity":"VendorsV2","select":"VendorAccountNumber,VendorName,dataAreaId","filter":"dataAreaId eq '\''bc'\''","top":"5"}}}' | d365-odata-mcp 2>/dev/null

# Pagination with skip
echo '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"query_entity","arguments":{"entity":"ProductsV2","top":"10","skip":"20"}}}' | d365-odata-mcp 2>/dev/null
```

### 3. get_entity_schema
Get available fields for an entity by fetching a sample record:
```bash
echo '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"get_entity_schema","arguments":{"entity":"SalesOrderHeaders"}}}' | d365-odata-mcp 2>/dev/null
```

### 4. get_record
Get a specific record by ID:
```bash
echo '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"get_record","arguments":{"entity":"CustomersV3","id":"CUS-000001"}}}' | d365-odata-mcp 2>/dev/null
```

### 5. get_environment_info
Get information about the connected D365 environment:
```bash
echo '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"get_environment_info","arguments":{}}}' | d365-odata-mcp 2>/dev/null
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
| `Warehouses` | Warehouse master data |

---

## Parse JSON Response

To extract just the text result:
```bash
echo '...' | d365-odata-mcp 2>/dev/null | jq '.result.content[0].text' -r
```

---

## Filter Expression Syntax

| Operator | Description | Example |
|----------|-------------|---------|
| `eq` | Equal | `Status eq 'Open'` |
| `ne` | Not equal | `Status ne 'Closed'` |
| `gt` | Greater than | `Amount gt 1000` |
| `ge` | Greater or equal | `Amount ge 500` |
| `lt` | Less than | `Amount lt 100` |
| `le` | Less or equal | `Amount le 1000` |
| `and` | Logical AND | `Status eq 'Open' and Amount gt 100` |
| `or` | Logical OR | `Status eq 'Open' or Status eq 'Pending'` |
| `contains` | Contains string | `contains(Name, 'Corp')` |
| `startswith` | Starts with | `startswith(Name, 'ABC')` |
| `endswith` | Ends with | `endswith(Name, 'Ltd')` |
