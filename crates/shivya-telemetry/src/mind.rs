//! Thin wasm-bindgen wrapper around [`shivya_mind::Memory`].
//!
//! Exposes the cognitive core to the in-browser cockpit through three
//! affordances:
//!
//! * [`MindCore::observe`] ingests one (subject, predicate, object) event.
//! * [`MindCore::self_similarity`] returns how strongly an event hypothesis
//!   already lines up with current working memory in `[-1, 1]`.
//! * [`MindCore::signature_hex`] returns the first 8 bytes of the packed
//!   bipolar working-memory vector as a 16-char hex string -- a stable,
//!   compact fingerprint suitable for a row of UI cells.
//!
//! Counters [`MindCore::events_ingested`] and
//! [`MindCore::event_count_in_episode`] feed the panel's headline numbers.

use std::sync::Arc;
use wasm_bindgen::prelude::*;

use shivya_mind::{Codebook, Event, Memory};

#[wasm_bindgen]
pub struct MindCore {
    memory: Memory,
    events_ingested: u64,
}

#[wasm_bindgen]
impl MindCore {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        let codebook = Arc::new(Codebook::with_default_salt());
        let memory = Memory::new(codebook);
        Self {
            memory,
            events_ingested: 0,
        }
    }

    pub fn observe(&mut self, subject: &str, predicate: &str, object: &str) {
        let event = Event::new(subject, predicate, object);
        self.memory.update(&event);
        self.events_ingested = self.events_ingested.saturating_add(1);
    }

    pub fn self_similarity(&mut self, subject: &str, predicate: &str, object: &str) -> f32 {
        let event = Event::new(subject, predicate, object);
        self.memory.fact_strength(&event)
    }

    pub fn signature_hex(&mut self) -> String {
        // First two u32 words of the working-memory hypervector, big-endian.
        // 2 words * 4 bytes = 8 bytes => 16 hex chars.
        let wm = self.memory.working_memory();
        let mut out = String::with_capacity(16);
        for w in 0..2 {
            for b in wm.data[w].to_be_bytes().iter() {
                out.push_str(&format!("{:02x}", b));
            }
        }
        out
    }

    pub fn events_ingested(&self) -> u64 {
        self.events_ingested
    }

    pub fn event_count_in_episode(&self) -> usize {
        self.memory.event_count_in_episode()
    }
}

impl Default for MindCore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observe_increments_counter_and_changes_signature() {
        let mut core = MindCore::new();
        let sig_before = core.signature_hex();
        core.observe("agent", "perceived", "obs_pp");
        assert_eq!(core.events_ingested(), 1);
        let sig_after = core.signature_hex();
        // After a single deterministic ingest the sign-projection should
        // change at least one of the first eight bytes of working memory.
        assert_ne!(sig_before, sig_after, "signature should evolve on ingest");
    }

    #[test]
    fn self_similarity_of_observed_event_is_above_random() {
        let mut core = MindCore::new();
        // Prime memory with one fact and check that asking about the
        // same fact yields a similarity well above a random hypervector's
        // ~0.0 baseline (we use 0.2 as a wide safety margin around the
        // theoretical 5/sqrt(D)=0.05 5-sigma bound).
        core.observe("device", "emits", "telemetry");
        let s = core.self_similarity("device", "emits", "telemetry");
        assert!(s > 0.2, "self-similarity = {s}, expected > 0.2");
    }
}
