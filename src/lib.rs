pub mod event;
pub mod worker;
pub mod task;

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

// use crate::prelude::*;
// #[wasm_bindgen]
// pub fn main() {
//     wasm_bindgen_futures::spawn_local(async {
//         let worker = worker::TaskWorker::new().await.unwrap();
//         let w1 = worker.run(|v| v, &4).await.unwrap();
//         let w2 = worker.run(|v| v * v, &5).await.unwrap();
//         let w3 = worker.run(|v| v * v * v, &8).await.unwrap();
//         web_sys::console::log_3(&w1.into(), &w2.into(), &w3.into());
//     });

//     wasm_bindgen_futures::spawn_local(async {
//         let r = task::Task::new(|c| c.consume(5)).await;
//         web_sys::console::log_1(&r.into());
//     });
// }