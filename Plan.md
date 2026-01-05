## Plan: MCP for D365/Dataverse OData

TL;DR — Build an MCP connector that authenticates via Azure AD, discovers entity metadata, performs an initial full load then incremental delta pulls (change-tracking or timestamp), handles paging and throttling with exponential backoff, maps EDM schema to a canonical model, and provides observability and tests for safe production runs.

### Steps
1. Create config and secrets: add `config/default.toml` and `src/config/config.rs` to load endpoints, tenant, client_id, entities, and concurrency settings.
2. Implement auth: add `src/auth/mod.rs` for Azure AD client-credentials and certificate flows; integrate secure secret retrieval (Key Vault / env).
3. Implement OData client: add `src/odata/client.rs` providing `ODataClient` to call service root, fetch `$metadata`, follow `@odata.nextLink`, batch requests, and parse responses.
4. Implement ingest orchestration: add `src/ingest/orchestrator.rs` and `src/ingest/delta_tracker.rs` to run initial full loads, persist high-water marks/change tokens, and schedule delta syncs per-entity.
5. Implement transform & observability: add `src/ingest/transform.rs` and `src/observability/mod.rs` for schema mapping, type conversions, structured logging, metrics, and OpenTelemetry traces.
6. Tests & docs: add `tests/unit/odata_client_tests.rs`, `tests/integration/dataverse_integration.rs`, `README.md`, and `tools/sample_queries.md` with onboarding and sample queries.

### Further Considerations
1. Clarify product scope: Dataverse vs Dynamics Finance & Ops vs both — endpoint paths and features differ.
2. Confirm auth mode(s): client-credentials (app-only) vs delegated user flows — affects permissions and consent.
3. Delta capability: confirm whether change-tracking is enabled for target tables; otherwise use timestamp-based incremental pulls.

### Implementation Artifacts (suggested)
- `src/config/config.rs` — typed config loader for endpoints, tenant, client_id, entities, paging, and concurrency.
- `config/default.toml` — example configuration template for endpoints, entities, paging, concurrency, and observability.
- `src/auth/mod.rs` — `AzureAdAuth` to acquire tokens (secret/cert/federated), cache tokens, and refresh on expiry.
- `src/odata/client.rs` — `ODataClient` for building requests, handling headers, following `@odata.nextLink`, batching, and parsing responses.
- `src/ingest/orchestrator.rs` — `IngestOrchestrator` coordinating initial full loads, delta syncs, retry policies, and concurrency limits.
- `src/ingest/delta_tracker.rs` — persist and retrieve change tokens or high-water timestamps per entity/environment.
- `src/ingest/transform.rs` — schema mapping and type conversion helpers.
- `src/observability/mod.rs` — structured logging, Prometheus metrics, OpenTelemetry tracing.
- `tests/unit/odata_client_tests.rs` and `tests/integration/dataverse_integration.rs` — unit and integration tests.
- `tools/sample_queries.md` — canonical OData queries and tooling snippets for onboarding.

### Risks & Open Questions
- Which Dynamics product(s) to support (Dataverse, Finance & Ops, or both)?
- Will auth be app-only (client credentials) or require delegated flows?
- Is source on-prem or cloud-only?
- What are expected throughput and SLA requirements (rows/sec, latency)?
- Are change-tracking APIs enabled, or must we rely on timestamp filtering?

---

If you want, I can now scaffold the suggested files and add code skeletons for the auth and OData client.
