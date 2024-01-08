pub mod client;
pub mod types;
pub mod state;
pub mod signing;

#[cfg(not(target_arch = "wasm32"))]
pub mod state_redb;
#[cfg(target_arch = "wasm32")]
pub mod state_indexeddb;

#[cfg(test)]
mod tests;
mod debug;
#[cfg(test)]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);
