// Jackson Coxson

pub mod app;
pub mod error_template;
pub mod forge_component;

pub mod blog;
#[cfg(feature = "ssr")]
pub mod context;
#[cfg(feature = "ssr")]
pub mod fileserv;
#[cfg(feature = "ssr")]
pub mod forge;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::*;
    console_error_panic_hook::set_once();
    leptos::mount_to_body(App);
}

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
extern "C" {
    #[wasm_bindgen::prelude::wasm_bindgen(js_namespace = hljs)]
    fn highlightAll();
}

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn reload() {
    if let Some(window) = web_sys::window() {
        window.location().reload().unwrap();
    }
}

#[cfg(not(feature = "hydrate"))]
#[allow(non_snake_case)]
fn highlightAll() {}

#[cfg(not(feature = "hydrate"))]
#[allow(non_snake_case)]
fn reload() {}
