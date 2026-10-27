# elang-server

HTTP service exposing the ELANG SDK over
`POST /{inspect,verify,validate}`.

This is a thin Axum wrapper around [`elang-reader`](../elang-reader)'s
12-stage pipeline. **No validation logic lives here** — the crate maps
HTTP requests onto `Package::open` / `Package::inspect` /
`Package::verify` / `Package::validate` and shapes the result into the
JSON the Hub expects.

Tracks [Issue #76](https://github.com/RockyDevWorks/elang-rs/issues/76).

## Why this crate exists

`RockyDevWorks/elang-hub` strictly delegates all ELANG validation to an
upstream service (REQ-HUB-001, ADR 0001). The Hub's
`src/lib/sdk/contract.ts` defines the wire contract, but until this
crate landed there was no service to call — the Hub's `/api/validate`
route returned `503 Validation is unavailable right now.`

`elang-core` is intentionally dependency-light (no `serde`), so the
HTTP framework lives here at the leaf boundary where JSON is actually
needed.

## Endpoints

| Method | Path        | Purpose                                                   | Hub contract |
|--------|-------------|-----------------------------------------------------------|--------------|
| `GET`  | `/healthz`  | Liveness probe. Not part of the SDK contract.              | —            |
| `POST` | `/inspect`  | Stages 1–7. Listing only; **no** integrity verification.   | `SdkInspectResult` |
| `POST` | `/verify`   | Stages 1–11. SHA-256 of every indexed resource verified.   | `SdkVerifyResult`  |
| `POST` | `/validate` | Stages 1–12. Full ELD semantic validation.                 | `SdkValidateResult` |

All SDK endpoints require `Content-Type: application/octet-stream` and
return `application/json`.

### Request

```bash
curl -X POST \
  --data-binary @examples/hello-en/hello-en.elang \
  http://localhost:8080/validate \
  -H 'content-type: application/octet-stream' \
  -H 'accept: application/json'
```

### Success response (`validate`)

```json
{
  "ok": true,
  "mode": "validate",
  "specVersion": "0.1.0",
  "binaryContainerVersion": "0.1.0",
  "phonemeResourcePath": "Linguistics/phonemes.eld",
  "inventory": {
    "specVersion": "0.1.0",
    "language": {
      "identifier": "urn:elang:language:rocky:hello-en:1.0.0",
      "name": "Hello EN",
      "description": null
    },
    "phonemeResourcePath": "Linguistics/phonemes.eld",
    "groupCount": 1,
    "rootCount": 5,
    "canonical": "…hex-encoded canonical phoneme bytes…"
  },
  "diagnostics": []
}
```

`inventory` is opaque to the Hub at v0.1.0 (`unknown` in TypeScript); it
is shaped here so a future Hub view-model can narrow into it without a
wire-contract change.

### Failure response

```json
{
  "ok": false,
  "stage": "inspect",
  "code": "malformed_container",
  "message": "…",
  "path": "…optional resource locator…",
  "diagnostics": [
    { "stage": "inspect", "code": "malformed_container", "message": "…" }
  ]
}
```

**SDK-content failures return `200 OK` with `ok: false`.** This is
deliberate: the Hub's `SdkClient` short-circuits on any non-2xx status
and maps it to a coarse transport code (`request_too_large`, `timeout`,
`service_unavailable`, `unknown`). Returning `200 OK` is the only way
its `parseSdkResult` path picks up the structured `SdkErrorCode`.

Transport-level failures still use their proper status:

| Situation                | Status | Hub maps to             |
|--------------------------|--------|-------------------------|
| Body or decoded data over the service cap | `413`  | `request_too_large`     |
| Handler timeout          | `504`  | `timeout`               |
| Internal error or concurrency limit reached | `503` | `service_unavailable` |
| Wrong `Content-Type`     | `415`  | `unknown`               |
| SDK content failure      | `200`  | the reported `SdkErrorCode` |

## Configuration

| Env var                         | Default    | Range                          | Purpose                                       |
|---------------------------------|------------|--------------------------------|-----------------------------------------------|
| `ELANG_SERVER_PORT`             | `8080`     | valid `u16`                    | TCP listen port                               |
| `ELANG_SERVER_MAX_BODY_BYTES`   | `10485760` | `1`–`268435456` (256 MiB cap)  | Request body cap; `413` above it              |
| `ELANG_SERVER_TIMEOUT_MS`       | `30000`    | `1`+ (clamped to 1)             | Per-request handler budget; `504` above it    |
| `ELANG_SERVER_MAX_CONCURRENT`   | `4`        | `1`–`512` (clamped)            | Max concurrent in-flight SDK requests         |

The HTTP service also caps the total decoded resource bytes and each Zstd
window at **8 MiB per SDK job**, before decompression. `/verify` and
`/validate` return `413 request_too_large` above either cap; `/inspect`
continues to list larger archives without decoding. These deployment limits
are stricter than the reader library's 8 GiB aggregate / 128 MiB window
ceilings. Raising the body or concurrency settings requires re-evaluating
VM memory; a compressed-body cap alone does not bound decoded memory.

When all concurrency permits are held, SDK requests return `503` immediately,
before buffering the body. Permits cover body buffering and SDK processing;
failed body extraction releases capacity. Body reads time out after an idle
interval of 30 seconds or the configured handler timeout, whichever is shorter.
Timed-out blocking jobs retain their permits until the work finishes.

## Running locally

```bash
# From the repo root
cargo run -p elang-server

# Or build the release binary
cargo build --release -p elang-server
./target/release/elang-server
```

Then:

```bash
curl -X POST --data-binary @examples/hello-en/hello-en.elang \
  http://localhost:8080/inspect \
  -H 'content-type: application/octet-stream'
```

## Docker

```bash
docker build -f crates/elang-server/Dockerfile -t elang-server:local .
docker run --rm -p 8080:8080 elang-server:local
```

The image is a multi-stage build: `rust:1.99-bookworm` for the build,
`debian:12-slim` (Bookworm) for the runtime. The final image carries a single
stripped binary (~2 MB) and runs as a non-root user.

## Deploying to Fly.io

```bash
# From the workspace root (fly.toml lives at the root, not in the crate)
fly launch --path . --copy-config --no-deploy --config fly.toml --dockerfile crates/elang-server/Dockerfile
fly deploy . --config fly.toml --dockerfile crates/elang-server/Dockerfile
```

The deploy argument `.` explicitly sets the workspace root as the build
context. The config and Dockerfile paths are relative to that root.

`fly.toml` pins the region (`iad`), the machine size
(`shared-cpu-1x` / 1 GB), one concurrent SDK job, and the health check (`GET /healthz`,
every 15 s). `auto_stop_machines = false` — auto-stop would add a cold
start to every interactive validation request. The allocation leaves
headroom for one SDK job under the service's decoded-byte/window caps,
the default 10 MiB body, parsing and runtime overhead. The Dockerfile
referenced by `fly.toml` (`crates/elang-server/Dockerfile`) expects the workspace
root as its build context. `--path .` and `deploy .` select that root;
both config and Dockerfile paths are relative to it.

Once deployed, point the Hub at it:

```
SDK_SERVICE_URL=https://<app-name>.fly.dev
```

## Tests

```bash
cargo test -p elang-server
```

`tests/contract.rs` drives the real router via `tower::ServiceExt::oneshot`
against the committed `examples/hello-en/hello-en.elang` fixture and
asserts the response shape matches what the Hub's `SdkClient` parses —
including the `200 OK` + `ok: false` convention for SDK failures.

## License

Apache-2.0 — same as the rest of the workspace.
