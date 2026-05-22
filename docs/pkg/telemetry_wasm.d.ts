/* tslint:disable */
/* eslint-disable */

export class MindCore {
    free(): void;
    [Symbol.dispose](): void;
    event_count_in_episode(): number;
    events_ingested(): bigint;
    constructor();
    observe(subject: string, predicate: string, object: string): void;
    self_similarity(subject: string, predicate: string, object: string): number;
    signature_hex(): string;
}

export class ShivyaSimulation {
    free(): void;
    [Symbol.dispose](): void;
    add_edge(u_label: string, v_label: string, initial_state: number): void;
    add_vertex(label: string, initial_state: number): number;
    agent_free_energy(obs_0: number, obs_1: number): number;
    agent_update_beliefs(obs_0: number, obs_1: number): Float64Array;
    get_agent_beliefs(): Float64Array;
    get_edge_state(idx: number): number;
    get_edge_u(idx: number): number;
    get_edge_v(idx: number): number;
    get_edges_count(): number;
    get_triangles_count(): number;
    get_vertex_label(idx: number): string;
    get_vertex_state(idx: number): number;
    get_vertices_count(): number;
    constructor();
    reconcile_flows(delta_s: Float64Array): Float64Array;
}

export class SubstrateOrchestrator {
    free(): void;
    [Symbol.dispose](): void;
    inject_stress(node_id: number): boolean;
    constructor();
    reset(): void;
    step(inputs: Float64Array): string;
    trigger_apoptosis(node_id: number): boolean;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_shivyasimulation_free: (a: number, b: number) => void;
    readonly __wbg_substrateorchestrator_free: (a: number, b: number) => void;
    readonly shivyasimulation_add_edge: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly shivyasimulation_add_vertex: (a: number, b: number, c: number, d: number) => number;
    readonly shivyasimulation_agent_free_energy: (a: number, b: number, c: number) => number;
    readonly shivyasimulation_agent_update_beliefs: (a: number, b: number, c: number) => [number, number];
    readonly shivyasimulation_get_agent_beliefs: (a: number) => [number, number];
    readonly shivyasimulation_get_edge_state: (a: number, b: number) => number;
    readonly shivyasimulation_get_edge_u: (a: number, b: number) => number;
    readonly shivyasimulation_get_edge_v: (a: number, b: number) => number;
    readonly shivyasimulation_get_edges_count: (a: number) => number;
    readonly shivyasimulation_get_triangles_count: (a: number) => number;
    readonly shivyasimulation_get_vertex_label: (a: number, b: number) => [number, number];
    readonly shivyasimulation_get_vertex_state: (a: number, b: number) => number;
    readonly shivyasimulation_get_vertices_count: (a: number) => number;
    readonly shivyasimulation_new: () => number;
    readonly shivyasimulation_reconcile_flows: (a: number, b: number, c: number) => [number, number];
    readonly substrateorchestrator_inject_stress: (a: number, b: number) => number;
    readonly substrateorchestrator_new: () => number;
    readonly substrateorchestrator_reset: (a: number) => void;
    readonly substrateorchestrator_step: (a: number, b: number, c: number) => [number, number];
    readonly substrateorchestrator_trigger_apoptosis: (a: number, b: number) => number;
    readonly __wbg_mindcore_free: (a: number, b: number) => void;
    readonly mindcore_event_count_in_episode: (a: number) => number;
    readonly mindcore_events_ingested: (a: number) => bigint;
    readonly mindcore_new: () => number;
    readonly mindcore_observe: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
    readonly mindcore_self_similarity: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => number;
    readonly mindcore_signature_hex: (a: number) => [number, number];
    readonly sm_codebook_free: (a: number) => void;
    readonly sm_codebook_new: (a: number, b: number) => number;
    readonly sm_hypervector_similarity: (a: number, b: number) => number;
    readonly sm_memory_free: (a: number) => void;
    readonly sm_memory_new: (a: number) => number;
    readonly sm_memory_update: (a: number, b: number, c: number, d: number) => void;
    readonly sm_memory_working_memory: (a: number, b: number) => void;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
