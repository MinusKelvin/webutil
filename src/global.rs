use crate::prelude::*;
use crate::channel::{ oneshot, Receiver, channel };
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;

#[wasm_bindgen]
extern "C" {
    fn setInterval(closure: &Closure<dyn FnMut()>, period: u32) -> i32;
    fn clearInterval(handle: i32);
    fn setTimeout(closure: &Closure<dyn FnMut()>, delay: u32) -> i32;
}

pub fn set_interval(period: u32, f: impl FnMut() + 'static) -> IntervalHandle {
    let closure = Closure::wrap(Box::new(f) as Box<dyn FnMut()>);
    let id = setInterval(&closure, period);
    IntervalHandle(id, Some(closure))
}

pub fn set_timeout(delay: u32, f: impl FnOnce() + 'static) -> IntervalHandle {
    let closure = Closure::once(f);
    let id = setTimeout(&closure, delay);
    IntervalHandle(id, Some(closure))
}

pub fn request_animation_frame(f: impl FnOnce(f64) + 'static) -> AnimationFrameHandle {
    let closure = Closure::once(f);
    let id = web_sys::window().unwrap()
        .request_animation_frame(closure.as_ref().unchecked_ref())
        .unwrap();
    AnimationFrameHandle(id, Some(closure))
}

pub fn interval(period: u32) -> IntervalStream {
    let (s, r) = channel();
    let handle = set_interval(period, move || s.send(()).ok().unwrap());
    IntervalStream(r, handle)
}

pub async fn later(delay: u32) {
    let (s, r) = oneshot();
    set_timeout(delay, || s.resolve(()).ok().unwrap()).forget();
    r.await.unwrap()
}

pub async fn animation_frame() -> f64 {
    let (s, r) = oneshot();
    request_animation_frame(|now| s.resolve(now).ok().unwrap()).forget();
    r.await.unwrap()
}

pub struct IntervalHandle(i32, Option<Closure<dyn FnMut()>>);

impl IntervalHandle {
    pub fn forget(mut self) {
        self.1.take().unwrap().forget();
    }
}

impl Drop for IntervalHandle {
    fn drop(&mut self) {
        if self.1.is_some() {
            clearInterval(self.0);
        }
    }
}

pub struct AnimationFrameHandle(i32, Option<Closure<dyn FnMut(f64)>>);

impl AnimationFrameHandle {
    pub fn forget(mut self) {
        self.1.take().unwrap().forget();
    }
}

impl Drop for AnimationFrameHandle {
    fn drop(&mut self) {
        if self.1.is_some() {
            web_sys::window().unwrap().cancel_animation_frame(self.0).unwrap();
        }
    }
}

pub struct IntervalStream(Receiver<()>, IntervalHandle);

impl IntervalStream {
    pub fn try_next(&self) -> Option<()> {
        self.0.try_recv().ok()
    }

    pub async fn next(&self) {
        self.0.recv().await.unwrap()
    }
}