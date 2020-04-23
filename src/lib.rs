pub mod event;
pub mod worker;

pub mod prelude {
    pub use wasm_bindgen::prelude::*;
    pub use crate::event::EventTargetExt;
}