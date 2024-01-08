use cfg_if::cfg_if;

#[macro_export]
macro_rules! noop_println {
    () => {};
    ($($arg:tt)*) => {};
}

cfg_if! {
    if #[cfg(not(feature = "debug"))] {
        pub use noop_println as debug_println;
    } else if #[cfg(not(target_arch = "wasm32"))] {
        pub use std::println as debug_println;
    } else if #[cfg(target_arch = "wasm32")] {
        pub use wasm_bindgen_test::console_log as debug_println;
    }
}
