pub mod event;
pub mod global;
pub mod channel;
// pub mod worker;

pub mod prelude {
    pub use wasm_bindgen::prelude::*;
    pub use wasm_bindgen_futures::spawn_local;
    pub use crate::event::EventTargetExt;
    pub use crate::GeneralError;
}

#[derive(Debug)]
pub enum GeneralError {
    SerdeJson(serde_json::Error),
    Bincode(bincode::Error),
    WebSys(wasm_bindgen::JsValue)
}

impl From<serde_json::Error> for GeneralError {
    fn from(v: serde_json::Error) -> Self {
        GeneralError::SerdeJson(v)
    }
}

impl From<bincode::Error> for GeneralError {
    fn from(v: bincode::Error) -> Self {
        GeneralError::Bincode(v)
    }
}

impl From<wasm_bindgen::JsValue> for GeneralError {
    fn from(v: wasm_bindgen::JsValue) -> Self {
        GeneralError::WebSys(v)
    }
}

use crate::prelude::*;
#[wasm_bindgen]
pub fn main() {
    spawn_local(async {
        let r = web_sys::window().unwrap().on::<event::KeyDown>();
        loop {
            let e = r.next().await;
            web_sys::console::log_1(&e);
        }
    });

    spawn_local(async {
        let e = web_sys::window().unwrap().once::<event::Click>().await;
        web_sys::console::log_1(&e);
    });
}