//! `civ-wasm` — the thin wasm32 shim that lets the offline replay viewer's tool playground run the
//! REAL Rust tool dispatch (`civ_eval::run_tool`) against whatever board it is displaying, so no tool
//! is ever re-implemented in JavaScript (parity with the eval is the whole point).
//!
//! We deliberately do NOT use `wasm-bindgen`: `civ-core`/`civ-eval` are dependency-free pure Rust, and
//! a hand-rolled C ABI keeps the whole path dependency-free and buildable with only the stock
//! `wasm32-unknown-unknown` target (no `wasm-pack`/`wasm-bindgen-cli` needed). The JS side marshals
//! strings across the boundary by hand (see `viewer/src/main.ts`), which is a small, well-understood
//! amount of glue. `wasm32-unknown-unknown` defaults to `panic = "abort"`, and `run_tool` /
//! `from_viewer_json` are total (they return `error:`/`Err` strings, never panic), so no unwinding or
//! panic hook is required.
//!
//! ABI (all pointers are into wasm linear memory; all lengths are byte counts of UTF-8):
//!   - `alloc(len) -> ptr`                     : reserve `len` bytes; JS writes input UTF-8 there.
//!   - `dealloc(ptr, len)`                     : free a buffer previously returned by `alloc`.
//!   - `run_tool(board_ptr, board_len, name_ptr, name_len, args_ptr, args_len) -> ptr`
//!     Returns a pointer to a length-prefixed result buffer: 4 little-endian bytes of `u32`
//!     payload length, followed by that many UTF-8 bytes. JS reads the length, copies the bytes,
//!     then calls `dealloc(ptr, 4 + len)`.

use std::mem;

use civ_core::Board;
use civ_eval::{run_tool as eval_run_tool, ToolCall};

/// Reserve `len` bytes in wasm linear memory and hand the pointer to JS (which fills it with the
/// UTF-8 of one input string). The `Vec`'s ownership is leaked to the caller; reclaim it via
/// [`dealloc`] with the SAME `len`.
#[no_mangle]
pub extern "C" fn alloc(len: usize) -> *mut u8 {
    let mut buf = Vec::<u8>::with_capacity(len);
    let ptr = buf.as_mut_ptr();
    mem::forget(buf);
    ptr
}

/// Free a buffer previously produced by [`alloc`] (input strings) or by [`run_tool`] (the
/// length-prefixed result, freed with `len == 4 + payload_len`).
///
/// # Safety
/// `ptr`/`len` must be exactly a `(pointer, capacity)` pair from a prior `alloc`/`run_tool` call.
#[no_mangle]
pub unsafe extern "C" fn dealloc(ptr: *mut u8, len: usize) {
    if !ptr.is_null() && len != 0 {
        drop(Vec::from_raw_parts(ptr, 0, len));
    }
}

/// Read a `(ptr, len)` pair from wasm memory as a `&str`. Invalid UTF-8 falls back to empty.
///
/// # Safety
/// `ptr`/`len` must describe a live, initialized buffer of `len` UTF-8 bytes.
unsafe fn str_at<'a>(ptr: *const u8, len: usize) -> &'a str {
    if ptr.is_null() || len == 0 {
        return "";
    }
    let bytes = std::slice::from_raw_parts(ptr, len);
    std::str::from_utf8(bytes).unwrap_or("")
}

/// Pack a result string into a freshly-allocated, length-prefixed buffer and leak it to JS.
fn pack(out: String) -> *mut u8 {
    let bytes = out.into_bytes();
    let len = bytes.len();
    let mut buf = Vec::with_capacity(4 + len);
    buf.extend_from_slice(&(len as u32).to_le_bytes());
    buf.extend_from_slice(&bytes);
    let ptr = buf.as_mut_ptr();
    mem::forget(buf);
    ptr
}

/// Build a `Board` from the viewer's export JSON and run the named tool with `args_json` (a flat JSON
/// object string), returning the tool's exact output string via a length-prefixed buffer. A bad board
/// JSON yields an `error: ...` string (never a trap), so the viewer can show it inline.
///
/// # Safety
/// The three `(ptr, len)` pairs must each describe a live UTF-8 buffer (as written by JS after
/// [`alloc`]). The returned pointer must be freed by JS via [`dealloc`] with `len = 4 + payload_len`.
#[no_mangle]
pub unsafe extern "C" fn run_tool(
    board_ptr: *const u8,
    board_len: usize,
    name_ptr: *const u8,
    name_len: usize,
    args_ptr: *const u8,
    args_len: usize,
) -> *mut u8 {
    let board_json = str_at(board_ptr, board_len);
    let name = str_at(name_ptr, name_len);
    let args = str_at(args_ptr, args_len);

    let out = match Board::from_viewer_json(board_json) {
        Ok(board) => {
            let call = ToolCall {
                id: String::new(),
                name: name.to_string(),
                args_json: args.to_string(),
            };
            eval_run_tool(&board, &call)
        }
        Err(e) => format!("error: {e}"),
    };
    pack(out)
}
