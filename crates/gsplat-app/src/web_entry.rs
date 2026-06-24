//! The wasm entry point that wasm-bindgen/trunk invoke on page load: wire up
//! browser-friendly panics and logging, then start the viewer.

use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Info).expect("init browser logger");
    crate::app::run();
}
