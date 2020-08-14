use crate::prelude::*;
use crate::channel::{ Receiver, channel };
use crate::event;
use serde::{ Serialize, de::DeserializeOwned };
use wasm_bindgen::JsCast;
use std::marker::PhantomData;

/// Wrapper for dedicated web workers.
/// 
/// Dropping the worker immediately terminates the associated web worker, preventing
/// and messages it may have yet to process from being received.
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
pub struct Worker<O, I> {
    worker: web_sys::Worker,
    incoming: Receiver<I>,
    _phantom: PhantomData<fn(O)>
}

impl<I, O> Worker<O, I>
where
    I: Serialize + DeserializeOwned + 'static,
    O: Serialize + DeserializeOwned + 'static
{
    /// Spawns a new worker and runs the specified function in it.
    pub async fn new<T: Serialize + DeserializeOwned + 'static>(
        uri: &str, f: fn(T, Receiver<O>, WorkerSender<I>), args: &T
    ) -> Result<Self, GeneralError> {
        let worker = web_sys::Worker::new(uri)?;
        // wait for signal that web worker has spawned and is ready to receive messages
        worker.once::<event::Message>().await;

        // send the bootstrapper, user function, and user data to the worker.
        let msg: (usize, usize, Vec<u8>) = unsafe {(
            std::mem::transmute::<
                fn(web_sys::DedicatedWorkerGlobalScope, usize, Vec<u8>), _
            >(bootstrapper::<T, I, O>),
            std::mem::transmute(f),
            bincode::serialize(&args)?
        )};
        let data = bincode::serialize(&msg)?;
        let buf = js_sys::Uint8Array::from(&*data);
        worker.post_message_with_transfer(&buf, &js_sys::Array::of1(&buf.buffer()))?;

        // setup message receiver
        let (sender, incoming) = channel();
        let wrker = worker.clone();
        spawn_local(async move {
            let incoming = wrker.on::<event::Message>();
            loop {
                let msg = bincode::deserialize(
                    &incoming.next()
                        .await
                        .data()
                        .dyn_into::<js_sys::Uint8Array>()
                        .unwrap()
                        .to_vec()
                ).unwrap();
                if sender.send(msg).is_err() {
                    break
                }
            }
        });

        Ok(Worker {
            worker, incoming,
            _phantom: PhantomData
        })
    }

    pub fn try_recv(&self) -> Option<I> {
        self.incoming.try_recv().ok()
    }

    pub async fn recv(&self) -> I {
        self.incoming.recv().await.unwrap()
    }

    pub fn send(&self, v: &O) -> Result<(), GeneralError> {
        let data = bincode::serialize(v)?;
        let buf = js_sys::Uint8Array::from(&*data);
        self.worker.post_message_with_transfer(&buf, &js_sys::Array::of1(&buf.buffer()))?;
        Ok(())
    }
}

impl<I, O> Drop for Worker<O, I> {
    fn drop(&mut self) {
        self.worker.terminate();
    }
}

#[wasm_bindgen]
pub fn _web_worker_entry_point(scope: web_sys::DedicatedWorkerGlobalScope) {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));

    let scop = scope.clone();
    scope.add_event_listener_once(|e: event::Message| {
        // receive and run bootstrapper
        let (bootstrapper, userfun, userdata) = bincode::deserialize(
            &e.data()
                .dyn_into::<js_sys::Uint8Array>()
                .unwrap()
                .to_vec()
        ).unwrap();
        let bootstrapper = unsafe { std::mem::transmute::<
            usize, fn(web_sys::DedicatedWorkerGlobalScope, usize, Vec<u8>)
        >(bootstrapper) };
        bootstrapper(scop, userfun, userdata);
    }).forget();

    // notify main thread that we're ready to receive messages
    scope.post_message(&JsValue::UNDEFINED).unwrap();
}

fn bootstrapper<T, I, O>(
    scope: web_sys::DedicatedWorkerGlobalScope, userfun: usize, userdata: Vec<u8>
) where
    T: DeserializeOwned,
    I: Serialize + 'static,
    O: DeserializeOwned + 'static
{
    // extract userfun and userdata
    let userfun: fn(T, Receiver<O>, WorkerSender<I>) = unsafe { std::mem::transmute(userfun) };
    let userdata: T = bincode::deserialize(&userdata).unwrap();

    // setup incoming message receiver
    let (sender, receiver) = channel();
    let scop = scope.clone();
    spawn_local(async move {
        let incoming = scop.on::<event::Message>();
        loop {
            let msg = bincode::deserialize(
                &incoming.next()
                    .await
                    .data()
                    .dyn_into::<js_sys::Uint8Array>()
                    .unwrap()
                    .to_vec()
            ).unwrap();
            if sender.send(msg).is_err() {
                break
            }
        }
    });

    userfun(userdata, receiver, WorkerSender(scope, PhantomData));
}

#[derive(Clone)]
pub struct WorkerSender<I>(web_sys::DedicatedWorkerGlobalScope, PhantomData<fn(&I)>);

impl<I: Serialize> WorkerSender<I> {
    pub fn send(&self, v: &I) {
        let data = bincode::serialize(v).unwrap();
        let buf = js_sys::Uint8Array::from(&*data);
        self.0.post_message_with_transfer(&buf, &js_sys::Array::of1(&buf.buffer())).unwrap();
    }
}