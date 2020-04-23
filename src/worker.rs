use crate::prelude::*;
use crate::event;
use serde::{ Serialize, de::DeserializeOwned };

#[wasm_bindgen]
pub fn _web_worker_entry_point(scope: web_sys::WorkerGlobalScope) {
    let scop = scope.clone();
    scope.add_event_listener(move |e: event::Message| {
        let (fun, code, data) = e.data().into_serde().unwrap();
        unsafe {
            let fun = std::mem::transmute::<
                usize, fn(&web_sys::WorkerGlobalScope, usize, String)
            >(fun);
            fun(&scop, code, data);
        }
    }).forget();
}

fn invoke<T: DeserializeOwned>(scope: &web_sys::WorkerGlobalScope, code: usize, args: String) {
    unsafe {
        let code = std::mem::transmute::<usize, fn(&web_sys::WorkerGlobalScope, T)>(code);
        let args = serde_json::from_str(&args).unwrap();
        code(scope, args);
    }
}

/// Dedicated Web Workers convenience wrapper.
/// 
/// This interfaces requires that you build using `--target no-modules` and that
/// a `worker.js` file exists with the following content:
/// ```js
/// importScripts("./<your-app>.js");
/// const { _web_worker_entry_point } = wasm_bindgen;
/// async function run() {
///     await wasm_bindgen("./<your-app>_bg.wasm");
///     _web_worker_entry_point(self);
/// }
/// run();
/// ```
pub struct Worker(web_sys::Worker);

impl Worker {
    pub fn new() -> Result<Self, JsValue> {
        Ok(Self(web_sys::Worker::new("worker.js")?))
    }

    /// Run a function in the web worker with the specified arguments.
    /// 
    /// Unfortunately, wasm does not support shared memory right now, so we can't
    /// send closures to the web worker. The best alternative I could come up with
    /// is to serialize the required data and deserialize it in the web worker.
    pub fn run<T: Serialize + DeserializeOwned>(
        &self, code: fn(&web_sys::WorkerGlobalScope, T), args: &T
    ) -> Result<Result<(), JsValue>, serde_json::Error> {
        let msg = unsafe { (
            std::mem::transmute::<
                fn(&web_sys::WorkerGlobalScope, usize, String), usize
            >(invoke::<T>),
            std::mem::transmute::<fn(&web_sys::WorkerGlobalScope, T), usize>(code),
            serde_json::to_string(args)?
        ) };
        Ok(self.0.post_message(&JsValue::from_serde(&msg)?))
    }

    /// Run a serializeable closure in the web worker.
    /// 
    /// You can probably use `serde_closure` to get one of these.
    pub fn run_closure(
        &self, code: impl FnOnce(&web_sys::WorkerGlobalScope) + Serialize + DeserializeOwned
    ) -> Result<Result<(), JsValue>, serde_json::Error> {
        self.run(|s, f| f(s), &code)
    }
}