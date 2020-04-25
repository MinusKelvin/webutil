use crate::prelude::*;
use wasm_bindgen::closure::Closure;
use std::mem::ManuallyDrop;

#[wasm_bindgen]
extern "C" {
    fn setInterval(closure: &Closure<dyn FnMut()>, period: u32) -> i32;
    fn clearInterval(handle: i32);
    fn setTimeout(closure: &JsValue, delay: u32) -> i32;
}

pub fn set_interval(period: u32, f: impl FnMut() + 'static) -> IntervalHandle {
    let closure = Closure::wrap(Box::new(f) as Box<dyn FnMut()>);
    let id = setInterval(&closure, period);
    IntervalHandle {
        id,
        closure: ManuallyDrop::new(closure)
    }
}

pub struct IntervalHandle {
    id: i32,
    closure: ManuallyDrop<Closure<dyn FnMut()>>
}

impl IntervalHandle {
    pub fn forget(mut self) {
        unsafe {
            ManuallyDrop::take(&mut self.closure).forget();
        }
        std::mem::forget(self);
    }
}

impl Drop for IntervalHandle {
    fn drop(&mut self) {
        clearInterval(self.id);
        unsafe {
            ManuallyDrop::drop(&mut self.closure);
        }
    }
}