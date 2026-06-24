//! Drag-and-drop loading of real splat files (`.ply` / `.spz`) in the browser.
//!
//! The browser hands us a `File`; reading its bytes is async (a `Promise`), so
//! we read + parse in a spawned future and drop the resulting `Gaussians` into a
//! shared inbox. The render loop ([`crate::app`]) drains the inbox on its next
//! frame and swaps the splat in — no blocking, no channels.

use std::io::Cursor;

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wgpu_3dgs_viewer::core::{Gaussians, GaussiansSource};

use crate::app::LoadInbox;

/// Register `dragover`/`drop` handlers on the document so dropping a splat file
/// anywhere on the page loads it. The closures live for the page's lifetime.
pub fn setup_drag_and_drop(inbox: LoadInbox) {
    let document = web_sys::window()
        .expect("browser window")
        .document()
        .expect("document");

    // A drop only fires if dragover cancels the default (which is "navigate").
    let on_dragover = Closure::<dyn FnMut(web_sys::DragEvent)>::new(|e: web_sys::DragEvent| {
        e.prevent_default();
    });
    document
        .add_event_listener_with_callback("dragover", on_dragover.as_ref().unchecked_ref())
        .expect("add dragover listener");
    on_dragover.forget();

    let on_drop = Closure::<dyn FnMut(web_sys::DragEvent)>::new(move |e: web_sys::DragEvent| {
        e.prevent_default();
        let Some(file) = e
            .data_transfer()
            .and_then(|dt| dt.files())
            .and_then(|files| files.get(0))
        else {
            return;
        };

        let name = file.name();
        let inbox = inbox.clone();
        wasm_bindgen_futures::spawn_local(async move {
            match read_bytes(&file).await {
                Ok(bytes) => match parse(&bytes, &name) {
                    Some(gaussians) => {
                        log::info!("loaded {name} ({} bytes)", bytes.len());
                        *inbox.borrow_mut() = Some(gaussians);
                    }
                    None => log::error!("'{name}' is not a readable .ply or .spz splat"),
                },
                Err(_) => log::error!("failed to read '{name}'"),
            }
        });
    });
    document
        .add_event_listener_with_callback("drop", on_drop.as_ref().unchecked_ref())
        .expect("add drop listener");
    on_drop.forget();
}

/// Read a dropped file's full contents into bytes (async `File.arrayBuffer()`).
async fn read_bytes(file: &web_sys::File) -> Result<Vec<u8>, JsValue> {
    let buffer = wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await?;
    Ok(js_sys::Uint8Array::new(&buffer).to_vec())
}

/// Parse bytes as a splat, trying the extension-implied format first.
fn parse(bytes: &[u8], name: &str) -> Option<Gaussians> {
    let order = if name.to_lowercase().ends_with(".spz") {
        [GaussiansSource::Spz, GaussiansSource::Ply]
    } else {
        [GaussiansSource::Ply, GaussiansSource::Spz]
    };
    order
        .into_iter()
        .find_map(|source| Gaussians::read_from(&mut Cursor::new(bytes), source).ok())
}
