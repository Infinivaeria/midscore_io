/* tslint:disable */
/* eslint-disable */

export function chat(server_url: string, team: string, message: string): Promise<any>;

export function game_turn(server_url: string, team: string, player: string, message: string, game_prompt: string): Promise<any>;

export function health(server_url: string): Promise<any>;

export function history(server_url: string, team: string): Promise<any>;

export function routes(server_url: string): Promise<any>;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly chat: (a: number, b: number, c: number, d: number, e: number, f: number) => any;
    readonly game_turn: (a: number, b: number, c: number, d: number, e: number, f: number, g: number, h: number, i: number, j: number) => any;
    readonly health: (a: number, b: number) => any;
    readonly history: (a: number, b: number, c: number, d: number) => any;
    readonly routes: (a: number, b: number) => any;
    readonly wasm_bindgen_982db05ac8fe1ebe___convert__closures_____invoke___js_sys_52f83f0c3ed405b2___Function_fn_wasm_bindgen_982db05ac8fe1ebe___JsValue_____wasm_bindgen_982db05ac8fe1ebe___sys__Undefined___js_sys_52f83f0c3ed405b2___Function_fn_wasm_bindgen_982db05ac8fe1ebe___JsValue_____wasm_bindgen_982db05ac8fe1ebe___sys__Undefined_______true_: (a: number, b: number, c: any, d: any) => void;
    readonly wasm_bindgen_982db05ac8fe1ebe___convert__closures_____invoke___wasm_bindgen_982db05ac8fe1ebe___JsValue__core_ea52b0ce1aceee9f___result__Result_____wasm_bindgen_982db05ac8fe1ebe___JsError___true_: (a: number, b: number, c: any) => [number, number];
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_destroy_closure: (a: number, b: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
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
