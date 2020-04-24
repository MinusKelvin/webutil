use crate::prelude::*;
use crate::event;
use crate::task::Task;
use serde::{ Serialize, de::DeserializeOwned };
use std::rc::Rc;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::collections::VecDeque;

#[wasm_bindgen]
pub fn _web_worker_entry_point(scope: web_sys::DedicatedWorkerGlobalScope) {
    let scop = scope.clone();
    scop.add_event_listener_once(move |e: event::Message| {
        // Extract bundle for passing code+data to worker
        // invoker is a monomorphization of worker_setup that'll deserialize the data and prepare
        // incoming messages to be deserialized and given to the handler function
        let (invoker, handler, data) = e.data().into_serde().unwrap();
        let invoker: fn(web_sys::DedicatedWorkerGlobalScope, usize, String) =
            unsafe { std::mem::transmute::<usize, _>(invoker) };
        invoker(scope, handler, data);
    });
    scop.post_message(&JsValue::UNDEFINED).unwrap();
}

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
pub struct Worker<I, O> {
    worker: web_sys::Worker,
    outgoing: PhantomData<fn(I)>,
    incoming: PhantomData<fn() -> O>
}

impl<I, O> Worker<I, O> where
    I: Serialize + DeserializeOwned + 'static,
    O: Serialize + DeserializeOwned + 'static
{
    pub async fn new<T: Serialize + DeserializeOwned + 'static>(
        handler: fn(&Sender<O>, &Rc<RefCell<T>>, I), args: &T
    ) -> Result<Self, GeneralError> {

        // Create bundle for passing code+data to worker
        // 1. function to deserialize context, install listeners, and ser/de messages
        // 2. user-provided function to handle incoming messages
        // 3. context for message handling function
        let msg: (usize, usize, String) = unsafe {(
            std::mem::transmute::<fn(_, _, _), _>(worker_setup::<T, I, O>),
            std::mem::transmute(handler),
            serde_json::to_string(args)?
        )};
        let js_msg = JsValue::from_serde(&msg)?;

        Task::new(|consumer| match web_sys::Worker::new("worker.js") {
            Ok(worker) => worker.clone().add_event_listener_once(move |_: event::Message|
                consumer.consume(worker.post_message(&js_msg)
                    .map_err(Into::into)
                    .map(|_| Worker {
                        worker,
                        outgoing: PhantomData,
                        incoming: PhantomData
                    })
                )
            ),
            Err(e) => consumer.consume(Err(e.into()))
        }).await
    }

    pub fn add_listener(&self, mut f: impl FnMut(O) + 'static) -> event::ListenerHandle {
        self.worker.add_event_listener(move |e: event::Message| {
            f(e.data().into_serde().unwrap())
        })
    }

    pub fn send(&self, v: &I) -> Result<(), GeneralError> {
        self.worker.post_message(&JsValue::from_serde(v)?).map_err(Into::into)
    }
}

impl<I, O> Drop for Worker<I, O> {
    fn drop(&mut self) {
        self.worker.terminate();
    }
}

fn worker_setup<T, I, O>(
    scope: web_sys::DedicatedWorkerGlobalScope, handler: usize, data: String
) where
    T: DeserializeOwned + 'static,
    I: DeserializeOwned + 'static,
    O: Serialize + 'static
{
    let handler: fn(&Sender<O>, &Rc<RefCell<T>>, I) = unsafe {
        std::mem::transmute(handler)
    };
    // deserialize context and wrap it in something the handler could reference in other places
    // such as in the message listeners for its own workers.
    let data = Rc::new(RefCell::new(serde_json::from_str(&data).unwrap()));
    let sender = Sender(scope.clone(), PhantomData);

    // install handler, which will live for as long as the worker lives
    scope.add_event_listener(move |e: event::Message| {
        handler(&sender, &data, e.data().into_serde().unwrap());
    }).forget();
}

#[derive(Clone)]
pub struct Sender<O>(web_sys::DedicatedWorkerGlobalScope, PhantomData<fn(&O)>);

impl<O: Serialize> Sender<O> {
    pub fn send(&self, v: &O) -> Result<(), GeneralError> {
        self.0.post_message(&JsValue::from_serde(v)?).map_err(Into::into)
    }
}

/// Api for task execution in a worker.
/// 
/// This interface has the same setup requirements as the `Worker` interface.
pub struct TaskWorker {
    worker: Worker<(usize, usize, String), String>,
    incoming: event::ListenerHandle,
    futures: Rc<RefCell<VecDeque<Box<dyn FnOnce(String)>>>>
}

impl TaskWorker {
    pub async fn new() -> Result<Self, GeneralError> {
        let worker = Worker::new(|send, _, (invoker, code, data)| {
            let invoker: fn(&Sender<String>, usize, String) =
                unsafe { std::mem::transmute(invoker) };
            invoker(send, code, data);
        }, &()).await?;

        let futures: Rc<RefCell<VecDeque<Box<dyn FnOnce(String)>>>> =
            Rc::new(RefCell::new(VecDeque::new()));

        let fut = futures.clone();
        let incoming = worker.add_listener(
            move |e| fut.borrow_mut().pop_front().unwrap()(e)
        );

        Ok(TaskWorker {
            worker, futures, incoming
        })
    }

    pub async fn run<T, R>(&self, f: fn(T) -> R, args: &T) -> Result<R, GeneralError> where
        T: Serialize + DeserializeOwned + 'static,
        R: Serialize + DeserializeOwned + 'static
    {
        // Create bundle for passing code+data to worker
        // 1. function to ser/de input and output
        // 2. user-supplied task function
        // 3. user-supplied input
        let msg = unsafe {(
            std::mem::transmute::<fn(_, _, _), _>(task_invoker::<T, R>),
            std::mem::transmute(f),
            serde_json::to_string(args)?
        )};
        Task::new(|consumer| {
            // once we actually send off the work, we need to prepare to receive the result
            // thankfully it seems that messages are sent sequentially
            match self.worker.send(&msg) {
                Ok(()) => {
                    // queue a function to deserialize the result of the task and give it to the
                    // returned future.
                    self.futures.borrow_mut().push_back(
                        Box::new(|msg| consumer.consume(
                            serde_json::from_str(&msg).map_err(Into::into)
                        )
                    ));
                }
                Err(e) => consumer.consume(Err(e))
            }
        }).await
    }
}

fn task_invoker<T: DeserializeOwned, R: Serialize>(
    send: &Sender<String>, code: usize, data: String
) {
    // reconstruct the user function and user data
    let args = serde_json::from_str(&data).unwrap();
    let f: fn(T) -> R = unsafe { std::mem::transmute(code) };
    // do task
    let result = f(args);
    // return the result back to the main thread
    send.send(&serde_json::to_string(&result).unwrap()).unwrap();
}