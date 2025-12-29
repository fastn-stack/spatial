/* tslint:disable */
/* eslint-disable */

/**
 * Fetch hub info from the server (public endpoint)
 * Returns JSON with hub_id52
 */
export function fetch_hub_info(): Promise<string>;

/**
 * Get the spoke's alias
 */
export function spoke_alias(): string;

/**
 * Get the configured hub's ID52
 */
export function spoke_hub_id52(): string;

/**
 * Get the configured hub's URL
 */
export function spoke_hub_url(): string;

/**
 * Get the current spoke's ID52
 */
export function spoke_id52(): string;

/**
 * Get spoke info as JSON
 */
export function spoke_info(): string;

/**
 * Initialize a new spoke with hub connection info
 * Returns the spoke's ID52 on success
 */
export function spoke_init(hub_id52: string, hub_url: string, alias: string): Promise<string>;

/**
 * Initialize spoke with just alias and password
 * Fetches hub info automatically and registers with the hub
 * Returns the spoke's ID52 on success
 */
export function spoke_init_simple(alias: string, password: string): Promise<string>;

/**
 * Check if spoke is initialized in browser storage
 */
export function spoke_is_initialized(): Promise<boolean>;

/**
 * Load an existing spoke from browser storage
 * Returns the spoke's ID52 on success
 */
export function spoke_load(): Promise<string>;

/**
 * Load or initialize spoke - initializes if not exists
 * Returns the spoke's ID52 on success
 */
export function spoke_load_or_init(hub_id52: string, hub_url: string, alias: string): Promise<string>;

/**
 * Send a request to the hub
 * Returns the response payload as JSON string
 */
export function spoke_send_request(target_hub: string, app: string, instance: string, command: string, payload_json: string): Promise<string>;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly fetch_hub_info: () => any;
  readonly spoke_alias: () => [number, number, number, number];
  readonly spoke_hub_id52: () => [number, number, number, number];
  readonly spoke_hub_url: () => [number, number, number, number];
  readonly spoke_id52: () => [number, number, number, number];
  readonly spoke_info: () => [number, number, number, number];
  readonly spoke_init: (a: number, b: number, c: number, d: number, e: number, f: number) => any;
  readonly spoke_init_simple: (a: number, b: number, c: number, d: number) => any;
  readonly spoke_is_initialized: () => any;
  readonly spoke_load: () => any;
  readonly spoke_load_or_init: (a: number, b: number, c: number, d: number, e: number, f: number) => any;
  readonly spoke_send_request: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => any;
  readonly wasm_bindgen__convert__closures_____invoke__h5ffa2bfe0fd18603: (a: number, b: number, c: any) => void;
  readonly wasm_bindgen__closure__destroy__hbb880aedd86604fe: (a: number, b: number) => void;
  readonly wasm_bindgen__convert__closures_____invoke__hb184f56a98b66f0a: (a: number, b: number) => number;
  readonly wasm_bindgen__convert__closures_____invoke__h935634e9731e5bb4: (a: number, b: number, c: any, d: any) => void;
  readonly __wbindgen_malloc: (a: number, b: number) => number;
  readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
  readonly __wbindgen_exn_store: (a: number) => void;
  readonly __externref_table_alloc: () => number;
  readonly __wbindgen_externrefs: WebAssembly.Table;
  readonly __externref_table_dealloc: (a: number) => void;
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
