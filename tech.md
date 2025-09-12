# Create user
kvctl user create alice --password --role=writer,reader

# List users
kvctl user list --format=json

# Grant role
kvctl grant --user=alice --role=admin

# Revoke API key
kvctl apikey revoke k8s-prod-app-01

# Rotate password
kvctl user passwd alice

# Lock user
kvctl user lock bob --reason="suspicious activity"



Absolutely — let’s **rethink the entire architecture through the lens of PostgreSQL**, not just for auth, but for the **full system design**.

You said:  
> *“Let’s store users the same way as stored in postgres. We use the same arch like postgres but ours is key store only and simpler.”*

That’s a brilliant north star.

---

# 🧠 REDESIGN: “PostgreSQL Architecture — But for a Simple, Single-Node Key-Value Store”

We’re not building a relational DB. We’re not doing SQL, joins, or query planning.  
But we **can** — and **should** — borrow PostgreSQL’s battle-tested architecture for:

✅ Durability  
✅ Crash Recovery  
✅ Concurrency Control  
✅ System Catalogs  
✅ Authentication & Roles  
✅ Write-Ahead Logging  
✅ Checkpointing & Snapshots  
✅ Transactional Semantics (simplified)  
✅ Extensibility & Observability

---

## 🏗️ POSTGRESQL ARCHITECTURE — CORE COMPONENTS (Simplified)

Here’s what PostgreSQL does — and how we map it to our KV store:

| PostgreSQL Component       | KVStore++ Equivalent                             | Notes |
|----------------------------|--------------------------------------------------|-------|
| **Postmaster**             | `kvstored` (main process)                        | Single process, no forking |
| **Backend Processes**      | Async request handlers (threads/async tasks)     | No process-per-connection |
| **Shared Buffer Pool**     | In-Memory Hash Table + Page Cache (optional)     | All data fits in RAM? Or cache hot keys |
| **WAL (Write-Ahead Log)**  | WAL (identical concept)                          | Append-only, fsync, replay on crash |
| **Heap / Table Storage**   | LSM or B+ Tree (our “heap”)                      | Keys are “tuples”, values are “attribute 1” |
| **System Catalogs**        | `_sys.*` internal KV entries                     | Users, roles, grants, settings — stored as keys |
| **pg_authid, pg_roles**    | `_sys.users`, `_sys.roles`, `_sys.grants`        | Same structure, stored in WAL-backed KV |
| **Checkpoints**            | Snapshots (same concept)                         | Serialize in-memory state to disk |
| **Background Writer**      | Checkpoint thread + Compaction thread            | Async, non-blocking |
| **Autovacuum**             | Optional LSM compaction / B+ tree defrag          | Not needed if append-optimized |
| **Client Protocol (libpq)**| REST/gRPC + CLI                                  | No binary protocol — keep it simple |
| **Authentication (pg_hba)**| `_sys.settings:auth` + IP-bound API keys         | Policy-driven auth |
| **Replication (WAL Send)** | Local async replica (reads WAL)                  | Single follower for HA |
| **Crash Recovery**         | WAL replay from last checkpoint                  | Identical logic |

---

## ✅ WHAT WE KEEP FROM POSTGRESQL

### 1. **Everything is a “Tuple” — But We Call It a Key-Value Pair**
- PostgreSQL stores rows as `(ctid, xmin, xmax, data...)`
- We store: `(key, value, ttl, version, created_at)`
- Metadata is embedded in value or stored as system columns (in JSON)

### 2. **WAL-Centric Design**
- Every mutation → WAL record first → then apply to in-memory structure
- On crash → replay WAL → guaranteed consistency
- WAL format: `[timestamp][tx_id][op][key][value][metadata]`

> We don’t need MVCC or `xmin/xmax` — unless you want versioning later.

### 3. **System Catalogs as First-Class Storage**
- PostgreSQL: `pg_database`, `pg_authid`, `pg_class` → stored in heap, backed by WAL
- Us: `_sys.users`, `_sys.settings`, `_sys.grants` → stored in same KV engine, backed by WAL
- No special files. No JSON configs that can drift. Fully transactional.

### 4. **Checkpoint = Snapshot**
- PostgreSQL: writes all dirty buffers to data files, records LSN
- Us: serializes hash map to snapshot file, records last WAL offset
- Fast startup: load snapshot + replay WAL from offset

### 5. **Authentication & Roles — Identical Semantics**
- Roles have permissions
- Users are role members
- Passwords hashed + salted
- Login → issue JWT (or use API key)
- All changes WAL-logged → crash-safe

### 6. **Background Processes — But Threads/Async Tasks**
- PostgreSQL: bgwriter, walwriter, autovacuum launcher
- Us:
  - `checkpoint_thread`
  - `ttl_expirer_thread`
  - `wal_flusher_task`
  - `s3_uploader_task`
  - `metrics_collector_task`

> Single process, multi-threaded or async (tokio/epoll).

---

## ❌ WHAT WE DROP (TO KEEP IT SIMPLE)

| PostgreSQL Feature         | Why We Skip It (For Now)                         |
|----------------------------|--------------------------------------------------|
| SQL Parser / Planner       | We have GET/SET/DEL — no need for SQL            |
| Indexes (B-tree, GIN, etc) | Only primary key (the key itself) — no secondaries |
| MVCC / Snapshot Isolation  | Single-writer or per-key locking → no need       |
| TOAST (large values)       | Optional — we can cap value size (e.g., 1MB)     |
| Tablespaces                | Single data directory                            |
| Extensions (PostGIS, etc)  | Not needed — pure KV                             |
| Shared Memory / IPC        | Single process — no need for complex IPC         |
| libpq / Wire Protocol      | REST/gRPC/CLI — simpler, language-agnostic       |

---

## 🔄 ARCHITECTURE FLOW — POSTGRESQL STYLE

```
Client → [REST/gRPC Listener] → Auth → [Executor]

Executor:
  - Parse op (GET/SET/DEL)
  - Check RBAC (via _sys.roles + _sys.grants)
  - Acquire per-key lock
  → Write to WAL (fsync if configured)
  → Apply to in-memory hash table
  → (Async) Replicate to follower (if enabled)
  → (Async) Trigger TTL / stats update
  → Return response

Background Threads:
  - Checkpointer: every 5min → snapshot + truncate old WAL
  - TTL Manager: scan & delete expired keys
  - WAL Flusher: batch + fsync WAL (if not immediate)
  - S3 Exporter: upload snapshots
  - Metrics: update Prometheus gauges
```

> Identical to PostgreSQL’s flow — minus the planner and executor complexity.

---

## 💾 STORAGE LAYOUT — INSPIRED BY PGDATA

```
/data
├── WAL/                     # Write-Ahead Logs
│   ├── 00000001.wal
│   ├── 00000002.wal
│   └── ...
├── base/                    # Snapshots (like PG base backups)
│   ├── snapshot_20250405_120000.chk
│   └── snapshot_20250405_130000.manifest
├── system/                  # Optional: separate system keys (if needed)
├── pg_control               # Metadata: last WAL offset, checkpoint time, etc.
└── config.toml              # Runtime config (non-transactional)
```

> `pg_control` = our `control.json` — tracks:
> - last_checkpoint_wal_offset
> - system_catalog_version
> - encryption_nonce
> - startup_time

---

## 🚨 CRASH RECOVERY — IDENTICAL TO POSTGRESQL

1. Read `pg_control` → get last checkpoint WAL position
2. Load snapshot into memory
3. Open WAL files starting from checkpoint offset
4. Replay each record → apply mutation
5. On checksum error → halt (like PG in panic mode)
6. On success → mark “recovery completed” → start accepting connections

> This is the gold standard. We adopt it fully.

---

## 🧩 EXTENSIBILITY — LIKE POSTGRESQL CATALOGS

Want to add:

- **Secondary indexes later?** → Store as `_sys.indexes:<name>` + internal KV structure
- **TTL policies per namespace?** → `_sys.ttl_policies:users:*`
- **Audit triggers?** → `_sys.triggers:audit_on_set`

> Everything is a system key — no schema changes needed.

---

## ✅ WHY THIS IS A WIN

- **Proven**: PostgreSQL’s architecture has survived 25+ years of production abuse.
- **Safe**: WAL + checkpoint = no data loss.
- **Consistent**: All state — including users, roles, settings — is transactional.
- **Observable**: Just like PG, you can inspect system state via internal keys.
- **Simple**: We strip away SQL, planner, MVCC — keep only what KV needs.
- **Familiar**: DBAs and SREs will feel at home.

---

## 🧭 NEXT STEPS — IMPLEMENTATION READY

We now have a **PostgreSQL-inspired, WAL-backed, crash-safe, RBAC-secured, single-node key-value store** — with:

- System catalogs as internal keys (`_sys.*`)
- WAL + checkpoint recovery
- API keys + JWT auth
- Background workers
- Prometheus metrics
- S3 snapshot export
- CLI admin tool (`kvctl`)

---

✅ **Architecture Finalized. Fully PostgreSQL-inspired. Simpler. Production-hardened.**

> This is no longer “Redis with durability” — this is **“PostgreSQL’s soul in a key-value body.”**

---

🟢 Say **“BEGIN CODING”** when you’re ready — and specify:
- Preferred language (**Rust** recommended for safety + async + ecosystem)
- Build system (Cargo, CMake, Bazel?)
- Target platform (Linux only? macOS for dev?)

I’ll start with:
1. WAL format + fsync logic
2. In-memory hash table + locking
3. System catalog bootstrap (`_sys.*`)
4. Auth layer (API key + JWT)
5. REST/gRPC skeleton

Let me know when to begin.


Perfect — let’s build the **mental model first**.

You’re not just building a key-value store — you’re building a **production-grade, durable, observable, secure system** with Redis-like simplicity and PostgreSQL’s architectural integrity.

We’ll break the entire system into **7 Core Modules**, each with:

- 🎯 Purpose
- 🧠 Mental Model (how to think about it)
- 🔄 Responsibilities
- 📦 Key Data Structures / Interfaces
- ⚙️ Dependencies
- 🚫 What It Does NOT Do

This is your **architectural map** — refer back to it during coding to stay aligned.

---

# 🧠 MENTAL MODEL — KVSTORE++ MODULE ARCHITECTURE

---

## 🧩 MODULE 1: WAL (Write-Ahead Log)

> “The source of truth. All mutations must pass through here first.”

### 🎯 Purpose
Guarantee durability and crash recovery. Every write is appended + optionally fsynced before being applied to memory.

### 🧠 Mental Model
Think of it as a **transactional ledger**. Like a bank’s accounting book — nothing is “final” until it’s written here. On crash, replay this log to rebuild state.

### 🔄 Responsibilities
- Append serialized operations (SET/DEL/INCR) with metadata
- Fsync based on policy (`every_write`, `every_100ms`, etc.)
- Rotate files when size exceeds limit
- Support replay from offset (for recovery)
- Validate checksums on replay

### 📦 Key Structures
```rust
struct WalEntry {
    timestamp: u64,
    key: String,
    value: Vec<u8>,
    version: u64,          // for future MVCC or CAS
    ttl: Option<u64>,
    op_type: OpType,       // SET, DEL, INCR, CAS
    checksum: u32,
}

struct WalManager {
    current_file: File,
    current_offset: u64,
    sync_policy: SyncPolicy,
}
```

### ⚙️ Dependencies
- None (lowest layer)
- Uses filesystem + OS I/O

### 🚫 What It Does NOT Do
- ❌ Does NOT apply changes to memory
- ❌ Does NOT understand keys or values semantically
- ❌ Does NOT handle concurrency — caller must serialize writes

---

## 🧩 MODULE 2: Storage Engine (In-Memory + Snapshot)

> “The working state. Fast, lock-protected, recoverable.”

### 🎯 Purpose
Hold the current key-value state in memory. Support fast reads/writes. Serialize to disk for snapshots.

### 🧠 Mental Model
Think of it as a **bank vault’s active register** — constantly updated, but backed by the ledger (WAL). At intervals, take a photo (snapshot) to speed up recovery.

### 🔄 Responsibilities
- In-memory hash table (sharded for concurrency)
- Per-key locking (fine-grained)
- Apply WAL entries to state
- Serialize entire state to snapshot file
- Load snapshot into memory
- Handle TTL expiration (background)

### 📦 Key Structures
```rust
struct KvEntry {
    value: Vec<u8>,
    version: u64,
    expires_at: Option<u64>,
    created_at: u64,
}

struct MemoryStore {
    shards: Vec<RwLock<HashMap<String, KvEntry>>>, // 256 shards
    ttl_queue: BinaryHeap<TtlEvent>,               // for background expiry
}

struct SnapshotManager {
    last_snapshot_at: u64,
    last_wal_offset: u64,
}
```

### ⚙️ Dependencies
- WAL (for recovery)
- Concurrency primitives (locks, atomics)

### 🚫 What It Does NOT Do
- ❌ Does NOT handle auth or API
- ❌ Does NOT compress/encrypt (handled at WAL/snapshot layer)
- ❌ Does NOT talk to network

---

## 🧩 MODULE 3: System Catalog (`_sys.*`)

> “The system’s own metadata — stored as keys, managed like user data.”

### 🎯 Purpose
Store users, roles, grants, settings — using the same engine. WAL-backed. No external files.

### 🧠 Mental Model
Think of it as **PostgreSQL’s system catalogs** — but every “table” is just a reserved key prefix (`_sys.users:*`). Fully integrated, no special paths.

### 🔄 Responsibilities
- Bootstrap default roles/users on first start
- Validate and store user credentials (hashed)
- Resolve permissions for RBAC
- Store global settings (auth, audit, performance)
- Expose for inspection via `kvctl`

### 📦 Key Structures
```rust
// _sys.users:<username>
struct User {
    oid: u32,
    username: String,
    password_hash: String, // scrypt/argon2
    is_active: bool,
    roles: Vec<String>,
}

// _sys.roles:<role>
struct Role {
    name: String,
    permissions: Vec<String>, // ["GET", "SET", "DEL"]
}

// _sys.settings:auth
struct AuthSettings {
    min_password_length: u8,
    session_timeout_sec: u32,
}
```

### ⚙️ Dependencies
- Storage Engine (to read/write `_sys.*` keys)
- WAL (durability)
- Auth Module (to resolve permissions)

### 🚫 What It Does NOT Do
- ❌ Does NOT handle password hashing directly (delegates to crypto module)
- ❌ Does NOT expose via API directly — accessed through Auth Module

---

## 🧩 MODULE 4: Auth & RBAC

> “Who are you, and what are you allowed to do?”

### 🎯 Purpose
Authenticate clients (API key or JWT), authorize operations based on roles.

### 🧠 Mental Model
Think of it as a **bouncer + rulebook**. Checks ID (API key/JWT), looks up roles, checks if the requested operation (SET/DEL) is allowed.

### 🔄 Responsibilities
- Validate API key → map to user + permissions
- Validate JWT → verify signature, check expiry, map to user
- Enforce RBAC: is user allowed to perform this op on this key?
- Log auth events to audit log
- Integrate with System Catalog to load roles/users

### 📦 Key Structures
```rust
struct AuthContext {
    user: String,
    permissions: Vec<String>,
    source_ip: IpAddr,
    auth_method: AuthMethod, // ApiKey, Jwt
}

trait Authorizer {
    fn authorize(&self, op: &str, key: &str) -> Result<()>;
}
```

### ⚙️ Dependencies
- System Catalog (to load user/role data)
- Crypto (to verify JWT/hash)
- Audit Log (to log attempts)

### 🚫 What It Does NOT Do
- ❌ Does NOT store users — delegates to System Catalog
- ❌ Does NOT handle TLS — that’s the API layer’s job

---

## 🧩 MODULE 5: API Layer (REST + gRPC + CLI)

> “The front door. Speaks JSON, Protobuf, and shell commands.”

### 🎯 Purpose
Expose operations to clients. Handle connection, parsing, auth, routing, serialization.

### 🧠 Mental Model
Think of it as a **receptionist + translator**. Accepts requests in multiple languages (REST/gRPC/CLI), validates them, hands them to the executor.

### 🔄 Responsibilities
- Start HTTP/gRPC servers
- Parse requests → map to core operations
- Enforce auth (via Auth Module)
- Serialize responses (JSON/Protobuf)
- Handle pipelining/batching for throughput
- Serve metrics (`/metrics`) and health (`/health`)

### 📦 Key Structures
```rust
struct ApiRequest {
    op: OpType,
    key: String,
    value: Option<Vec<u8>>,
    ttl: Option<u64>,
}

struct ApiResponse {
    success: bool,
    value: Option<Vec<u8>>,
    error: Option<String>,
}
```

### ⚙️ Dependencies
- Auth Module
- Storage Engine (to execute ops)
- WAL (indirectly — via Storage Engine)
- Metrics (to track latency, counts)

### 🚫 What It Does NOT Do
- ❌ Does NOT apply business logic — delegates to Storage Engine
- ❌ Does NOT handle persistence — that’s WAL’s job

---

## 🧩 MODULE 6: Background Workers

> “The silent maintainers. Keep the system healthy.”

### 🎯 Purpose
Run periodic or async tasks: checkpointing, TTL expiry, metrics, S3 upload.

### 🧠 Mental Model
Think of them as **janitors + accountants**. They don’t serve customers, but without them, the system gets messy or breaks.

### 🔄 Responsibilities
- **CheckpointThread**: every N seconds → snapshot + truncate old WAL
- **TtlExpirer**: scan and delete expired keys
- **MetricsCollector**: update Prometheus gauges (ops/sec, memory, WAL size)
- **S3Uploader**: upload snapshots to S3 (async, non-blocking)
- **ReplicaStreamer**: (optional) stream WAL to local replica

### 📦 Key Structures
```rust
trait BackgroundWorker {
    fn start(&self);
    fn stop(&self);
}

struct CheckpointWorker {
    interval: Duration,
    storage: Arc<StorageEngine>,
    wal: Arc<WalManager>,
}
```

### ⚙️ Dependencies
- Storage Engine
- WAL
- Cloud (S3 client)
- Metrics

### 🚫 What It Does NOT Do
- ❌ Does NOT handle client requests
- ❌ Does NOT block the main thread

---

## 🧩 MODULE 7: Operational Tooling (`kvctl` + Observability)

> “The admin’s Swiss Army knife.”

### 🎯 Purpose
Allow operators to inspect, debug, and manage the system.

### 🧠 Mental Model
Think of it as **pgAdmin + psql + pg_stat_activity — but for KV**.

### 🔄 Responsibilities
- CLI tool (`kvctl`) with subcommands:
  - `kvctl keys --pattern "user:*"`
  - `kvctl wal tail`
  - `kvctl snapshot create`
  - `kvctl user create ...`
- Embedded metrics endpoint (`/metrics`)
- Profiling endpoint (`/debug/pprof`)
- WAL debugger (step through entries)
- Health checks

### 📦 Key Structures
```rust
struct KvctlCommand {
    name: String,
    handler: fn(args: Vec<String>) -> Result<()>,
}

struct MetricsExporter {
    gauges: HashMap<String, Gauge>,
    histograms: HashMap<String, Histogram>,
}
```

### ⚙️ Dependencies
- Storage Engine (to scan keys)
- WAL (to read entries)
- System Catalog (to manage users)
- Auth (to enforce admin role)

### 🚫 What It Does NOT Do
- ❌ Does NOT modify production state by default (unless explicitly commanded)
- ❌ Does NOT replace monitoring systems — integrates with them (Prometheus)

---

# 🧭 SYSTEM FLOW — REQUEST LIFECYCLE (MENTAL MODEL)

```
Client → [API Layer]
          ↓
      [Auth Module] → Reject if unauthorized
          ↓
   [Storage Engine] → Acquire per-key lock
          ↓
       [WAL] → Append + fsync (if configured)
          ↓
   [Storage Engine] → Apply to in-memory hash
          ↓
   [Background Workers] → (async) update TTL, metrics, maybe replicate
          ↓
      [API Layer] → Return response
```

> Every write flows through WAL first → then memory.  
> Every read hits memory (with lock).  
> Every admin task uses `kvctl` → talks to same engine.

---

# 🗺️ DEPENDENCY GRAPH (SIMPLIFIED)

```
            +------------------+
            |   Operational    |
            |    Tooling       |
            +--------+---------+
                     |
            +--------v---------+
            |      API         +------------------+
            +--------+---------+                  |
                     |                            |
            +--------v---------+         +--------v---------+
            |      Auth        |         | Background       |
            +--------+---------+         | Workers          |
                     |                   +--------+---------+
            +--------v---------+                  |
            |  System Catalog  |                  |
            +--------+---------+                  |
                     |                            |
            +--------v---------+         +--------v---------+
            |  Storage Engine  <-------->+       WAL          |
            +------------------+         +------------------+
```

> WAL and Storage Engine are the core. Everything else builds on them.

---

✅ **Mental Model Complete.**

You now have a **clear, modular, production-grade architecture** inspired by PostgreSQL, simplified for KV, with Redis-like API.

Each module has a single responsibility.  
Each layer depends only on what’s below it.  
Everything is crash-safe, observable, and secure.

---

🟢 When you’re ready, say:

> **“BEGIN CODING — START WITH WAL MODULE IN RUST”**

(or specify language + build system)

I’ll generate the first clean, production-ready module with tests, error handling, and comments — ready for integration.



# Run all tests
cargo test -- --test-threads=1

# Run specific test
cargo test test_wal_append_and_replay

# Run with logging
RUST_LOG=info cargo test -- --nocapture


🚀 USAGE 
# Start service
sudo systemctl start kvstore-plus-plus

# Stop service
sudo systemctl stop kvstore-plus-plus

# Restart service
sudo systemctl restart kvstore-plus-plus

# Enable auto-start on boot
sudo systemctl enable kvstore-plus-plus

# Check status
sudo systemctl status kvstore-plus-plus

# View live logs
journalctl -u kvstore-plus-plus -f

# Check resource usage
systemctl show kvstore-plus-plus --property=MemoryCurrent,ActiveState,SubState




# Build benchmark tool
cargo build --release --bin kvb

# Run GET-heavy test
./target/release/kvb get-heavy --url http://localhost:8080 --concurrency 50 --duration 30

# Run SET-heavy test
./target/release/kvb set-heavy --url http://localhost:8080 --value-size 256 --concurrency 25 --duration 30

# Run mixed test
./target/release/kvb mixed --url http://localhost:8080 --read-ratio 0.8 --concurrency 100 --duration 60

# Run full suite
./target/release/kvb suite --url http://localhost:8080 --concurrency 100 --duration 60 --output-prefix "prod-bench"

# Output files:
# - prod-bench.json (detailed results)
# - prod-bench.csv (for spreadsheets)
# - Text report to console



# Build
cargo build --release --bin dummy-load-server

# Run with 5000 ops/sec, custom ratios
LOAD_OPS_PER_SEC=5000 \
LOAD_GET_RATIO=0.7 \
LOAD_SET_RATIO=0.2 \
LOAD_DEL_RATIO=0.05 \
LOAD_INCR_RATIO=0.05 \
METRICS_PORT=9095 \
./target/release/dummy-load-server --target-url http://localhost:8080

# In another terminal, view metrics
curl http://localhost:9095/metrics

# Output:
# # HELP dummy_load_total_ops Total operations performed
# # TYPE dummy_load_total_ops counter
# dummy_load_total_ops 12345
# # HELP dummy_load_total_errors Total errors encountered
# # TYPE dummy_load_total_errors counter
# dummy_load_total_errors 0
📊 MONITOR YOUR SYSTEM 

While the dummy server runs, monitor your KVStore++ metrics: 

# View KVStore++ metrics
curl http://localhost:9091/metrics

# Example output:
# kvstore_wal_size_bytes 45056
# kvstore_key_count 8923
# kvstore_memory_usage_bytes 892300
# kvstore_connections_active{role="writer"} 1


VISUALIZE WITH PROMETHEUS + GRAFANA 

    Install Prometheus (if not already)
    Add scrape config to prometheus.yml:
     

scrape_configs:
  - job_name: 'kvstore'
    static_configs:
      - targets: ['localhost:9091']  # Your KVStore++ metrics

  - job_name: 'dummy-load'
    static_configs:
      - targets: ['localhost:9095']  # Dummy load server metrics
    Start Prometheus
    Import Grafana dashboard or create panels for:
        KVStore++ ops/sec (calculated from kvstore_key_count rate)
        WAL size growth
        Connection counts
        Dummy server error rate
         
     