//! Library surface for `shivya-cli`. The production target is the `shivya-cli`
//! binary in `src/main.rs`; this `lib.rs` re-exports the application-facing
//! pieces (the [`bridge`] module containing [`WorkloadMeshProxy`]) so that
//! integration tests under `tests/` and external embedders can drive the
//! same code path the binary uses, without going through the UDS / Tokio
//! daemon shell.
//!
//! Nothing here owns runtime state; everything is plain data + math. The
//! Tokio reactor and UDP sockets continue to live in `main.rs` and
//! `orchestrator.rs` exclusively.

pub mod bridge;
