pub mod event;
pub mod worker;

pub mod prelude {
    pub use wasm_bindgen::prelude::*;
    pub use crate::event::EventTargetExt;
    pub use crate::GeneralError;
}

#[derive(Debug)]
pub enum GeneralError {
    SerdeJson(serde_json::Error),
    WebSys(wasm_bindgen::JsValue)
}

impl From<serde_json::Error> for GeneralError {
    fn from(v: serde_json::Error) -> Self {
        GeneralError::SerdeJson(v)
    }
}

impl From<wasm_bindgen::JsValue> for GeneralError {
    fn from(v: wasm_bindgen::JsValue) -> Self {
        GeneralError::WebSys(v)
    }
}

// #[wasm_bindgen]
// pub fn main() {
//     worker::TaskWorker::new(|worker| {
//         worker.run(|v| {
//             web_sys::console::log_1(&format!("worker has value {}", v).into());
//             v * v
//         }, &5, |result| {
//             web_sys::console::log_1(&format!("main thread has result {}", result).into());
//         }).unwrap()
//     }).unwrap();
// }