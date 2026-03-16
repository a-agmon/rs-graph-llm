# Distributed Checkpointing Strategy

## Status: DOCUMENTED (not implemented as library code)

## Overview

LanceSessionStorage provides time travel and versioning for single-node deployments.
For distributed (multi-process, multi-node) deployments, the following strategies
enable shared state without adding distributed coordination to the library itself.

## Approach: Shared Object Storage

Lance datasets can be stored on S3-compatible object storage. Multiple processes
can read/write to the same Lance dataset path:

```
s3://my-bucket/sessions.lance
```

### How it works

1. **Write path**: `LanceSessionStorage::save()` appends a new version to the
   Lance dataset on S3. Lance uses optimistic concurrency — if two writers
   conflict, one retries automatically.

2. **Read path**: `LanceSessionStorage::get()` reads the latest version from S3.
   Lance caches locally for performance.

3. **Time travel**: `get_at_version(version)` reads a specific version from S3.
   All versions are retained (Lance append-only semantics).

### Configuration

```rust
// Production: Lance on S3
let storage = LanceSessionStorage::new("s3://my-bucket/sessions.lance");

// Development: local filesystem
let storage = LanceSessionStorage::new("/tmp/sessions.lance");
```

### Limitations

- **No distributed locks**: Two processes can write the same session simultaneously.
  Lance handles version conflicts at the dataset level, but application-level
  conflicts (two processes updating the same session) need external coordination.

- **Eventual consistency**: S3 provides strong read-after-write consistency for
  new objects, but list operations may be eventually consistent.

### When you need stronger guarantees

For workflows requiring strict serialization of session updates:

1. **PostgresSessionStorage** with row-level locks (`SELECT FOR UPDATE`)
2. **Redis-backed storage** with distributed locks (not yet implemented)
3. **Application-level partitioning**: assign each session to a specific process

## Recommendation

For most use cases, `LanceSessionStorage` on S3 with the current append-only
versioning is sufficient. The time travel capability makes debugging distributed
workflows much easier — you can always inspect what state each process saw.

For strict transactional guarantees, use `PostgresSessionStorage` which already
handles concurrent access via SQL transactions.
