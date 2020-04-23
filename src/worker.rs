use crate::prelude::*;
use crate::event;
use serde::{ Serialize, Deserialize, de::DeserializeOwned };
use std::rc::Rc;
use std::cell::RefCell;
use std::collections::VecDeque;

#[wasm_bindgen]
pub fn _web_worker_entry_point(scope: web_sys::DedicatedWorkerGlobalScope) {
    let scop = scope.clone();
    scope.add_event_listener_once(move |e: event::Message| {
        match e.data().into_serde().unwrap() {
            WorkerKind::Tasks => {
                let scope = scop.clone();
                scop.add_event_listener(move |e: event::Message| {
                    let (fun, code, data) = e.data().into_serde().unwrap();
                    unsafe {
                        let fun = std::mem::transmute::<
                            usize, fn(&web_sys::DedicatedWorkerGlobalScope, usize, String)
                        >(fun);
                        fun(&scope, code, data);
                    }
                }).forget();
            }
            WorkerKind::Dedicated(f) => {
                let fun = unsafe {
                    std::mem::transmute::<usize, fn(web_sys::DedicatedWorkerGlobalScope)>(f)
                };
                fun(scop);
            }
        }
    });
    scope.post_message(&JsValue::UNDEFINED).unwrap();
}

fn invoke<T: DeserializeOwned, R: Serialize>(
    scope: &web_sys::DedicatedWorkerGlobalScope, code: usize, args: String
) {
    unsafe {
        let code = std::mem::transmute::<usize, fn(T) -> R>(code);
        let args = serde_json::from_str(&args).unwrap();
        let result = code(args);
        scope.post_message(&JsValue::from_serde(&result).unwrap()).unwrap();
    }
}

/// Web Worker wrapper for parallel tasks.
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
pub struct TaskWorker {
    worker: web_sys::Worker,
    incoming: Rc<event::ListenerHandle>,
    futures: Rc<RefCell<VecDeque<(Box<dyn FnOnce(event::Message)>, Rc<event::ListenerHandle>)>>>

}

impl TaskWorker {
    pub fn new(later: impl FnOnce(TaskWorker) + 'static) -> Result<(), GeneralError> {
        let worker = web_sys::Worker::new("worker.js")?;
        let wrker = worker.clone();
        wrker.add_event_listener_once(move |_: event::Message| {
            worker.post_message(&JsValue::from_serde(&WorkerKind::Tasks).unwrap()).unwrap();
            let futures = Rc::new(RefCell::new(VecDeque::new()));
            let fut = futures.clone();
            later(TaskWorker {
                futures,
                incoming: Rc::new(worker.add_event_listener(
                    move |e| fut.borrow_mut().pop_front().unwrap().0(e)
                )),
                worker,
            })
        });
        Ok(())
    }

    /// Run a function in the web worker with the specified arguments.
    /// 
    /// Unfortunately, wasm does not support shared memory right now, so we can't
    /// send closures to the web worker. The best alternative I could come up with
    /// is to serialize the required data and deserialize it in the web worker.
    pub fn run<T, R>(&self, code: fn(T) -> R, args: &T, done: impl FnOnce(R) + 'static)
        -> Result<(), GeneralError>
    where
        T: Serialize + DeserializeOwned,
        R: Serialize + DeserializeOwned + 'static
    {
        let msg = unsafe { (
            std::mem::transmute::<
                fn(&web_sys::DedicatedWorkerGlobalScope, usize, String), usize
            >(invoke::<T, R>),
            std::mem::transmute::<fn(T) -> R, usize>(code),
            serde_json::to_string(args)?
        ) };

        self.futures.borrow_mut().push_back((Box::new(move |e| {
            done(e.data().into_serde::<R>().unwrap())
        }), self.incoming.clone()));

        self.worker.post_message(&JsValue::from_serde(&msg)?).map_err(Into::into)
    }

    /// Run a serializeable closure in the web worker.
    /// 
    /// You can probably use `serde_closure` to get one of these.
    pub fn run_closure<R: Serialize + DeserializeOwned + 'static>(
        &self,
        code: impl FnOnce() -> R + Serialize + DeserializeOwned,
        done: impl FnOnce(R) + 'static
    ) -> Result<(), GeneralError> {
        self.run(|f| f(), &code, done)
    }
}

#[derive(Serialize, Deserialize)]
enum WorkerKind {
    Tasks,
    Dedicated(usize)
}