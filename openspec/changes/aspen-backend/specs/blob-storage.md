# Blob Storage — iroh-blobs via aspen

## Summary

File attachments, screenshots, tool outputs, and cached LLM responses
are stored in iroh-blobs (BLAKE3 content-addressed) instead of the local
filesystem.  Aspen manages blob replication, garbage collection, and P2P
transfer.  In standalone mode, blobs remain local files.

## What Becomes a Blob

| Source | Current Storage | Target Storage |
|--------|----------------|----------------|
| Matrix file attachments | `~/.clankers/received/` | iroh-blob |
| Screenshot tool output | temp file | iroh-blob |
| File read results (large) | in-memory | iroh-blob (if > 1MB) |
| Session JSONL exports | `~/.clankers/sessions/` | iroh-blob |
| Plugin WASM binaries | `plugins/*.wasm` | iroh-blob |
| Router response cache | redb | iroh-blob (values > 64KB) |

Small values (config, session turns, usage entries) stay in the KV store.
The blob store is for binary data and large text outputs.

## BlobRef

```rust
/// Reference to a content-addressed blob.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobRef {
    /// BLAKE3 hash of the content
    pub hash: iroh_blobs::Hash,
    /// Size in bytes
    pub size: u64,
    /// MIME type (if known)
    pub content_type: Option<String>,
    /// Original filename (if applicable)
    pub filename: Option<String>,
}
```

Blob references are stored in the KV store alongside the metadata:

```
clankers:sessions:{id}:blobs:{hash}     → BlobRef (JSON)
clankers:plugins:{name}:wasm            → BlobRef (JSON)
clankers:router:cache:{key}:blob        → BlobRef (JSON)
```

## ClankerBlobStore Trait

```rust
#[async_trait]
pub trait ClankerBlobStore: Send + Sync + 'static {
    /// Store bytes, returns content-addressed reference.
    async fn store(&self, data: &[u8], meta: BlobMeta) -> Result<BlobRef>;

    /// Store from a file path (zero-copy if possible).
    async fn store_file(&self, path: &Path, meta: BlobMeta) -> Result<BlobRef>;

    /// Fetch blob content by hash.
    async fn fetch(&self, hash: &iroh_blobs::Hash) -> Result<Vec<u8>>;

    /// Fetch blob to a local file path.
    async fn fetch_to_file(&self, hash: &iroh_blobs::Hash, dest: &Path) -> Result<()>;

    /// Check if a blob exists locally.
    async fn exists(&self, hash: &iroh_blobs::Hash) -> bool;
}
```

### Standalone implementation

```rust
/// Filesystem-backed blob store (content-addressed directory).
pub struct LocalBlobStore {
    root: PathBuf,  // ~/.clankers/blobs/
}

impl LocalBlobStore {
    async fn blob_path(&self, hash: &iroh_blobs::Hash) -> PathBuf {
        let hex = hash.to_hex();
        // Two-level fanout: ab/cd/abcdef...
        self.root.join(&hex[..2]).join(&hex[2..4]).join(&hex)
    }
}
```

### Cluster implementation

```rust
/// Blob store backed by aspen's iroh-blobs integration.
pub struct AspenBlobStore {
    client: AspenClient,
}

#[async_trait]
impl ClankerBlobStore for AspenBlobStore {
    async fn store(&self, data: &[u8], meta: BlobMeta) -> Result<BlobRef> {
        let hash = self.client.blob_add_bytes(data).await?;
        // Aspen handles replication to other nodes automatically
        Ok(BlobRef {
            hash,
            size: data.len() as u64,
            content_type: meta.content_type,
            filename: meta.filename,
        })
    }

    async fn fetch(&self, hash: &iroh_blobs::Hash) -> Result<Vec<u8>> {
        // Aspen tries local first, then fetches from peers via P2P
        self.client.blob_read_to_bytes(hash).await
    }
}
```

## Garbage Collection

Aspen's blob GC uses **protection tags**.  Each blob referenced by a KV
entry gets a protection tag that prevents collection:

```rust
// When storing a blob ref in KV:
client.blob_protect(blob_ref.hash, &format!("clankers:session:{}", session_id)).await?;

// When deleting a session:
client.blob_unprotect(blob_ref.hash, &format!("clankers:session:{}", session_id)).await?;
// If no other protection tags remain, blob becomes eligible for GC
```

## P2P Transfer

When a user on Node A resumes a session that was created on Node B,
the blobs are fetched transparently via iroh's P2P transfer:

```
Node A                                      Node B
  │                                           │
  ├─ Load session meta from KV                │
  ├─ Session has blob refs                    │
  ├─ blob_read(hash)                          │
  │   ├─ Check local blob store → miss        │
  │   ├─ Ask iroh for providers of hash       │
  │   ├─ ─── QUIC blob request ───────────>   │
  │   │                                       ├─ Serve blob from local store
  │   │   <─── QUIC blob response ────────    │
  │   ├─ Cache locally                        │
  │   └─ Return bytes                         │
```

This is completely transparent to the agent — it just calls
`blob_store.fetch(hash)` and gets bytes back.

## Size Limits

| Context | Max Blob Size | Rationale |
|---------|--------------|-----------|
| Tool output | 10 MB | Prevent runaway reads |
| File attachment | 50 MB | Matrix default limit |
| WASM plugin | 20 MB | Hyperlight limit |
| Cache entry | 5 MB | LLM responses are text |
| Session export | 100 MB | Large conversation histories |
