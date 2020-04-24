use std::future::Future;
use std::task::{ Context, Poll, Waker };
use std::pin::Pin;
use std::rc::Rc;
use std::cell::RefCell;

pub struct Task<T, F> {
    result: Rc<RefCell<Option<T>>>,
    task: RefCell<Option<F>>
}

pub struct Consumer<T> {
    result: Rc<RefCell<Option<T>>>,
    waker: Rc<RefCell<Option<Waker>>>
}

impl<T, F: FnOnce(Consumer<T>)> Task<T, F> {
    pub fn new(f: F) -> Self {
        Task {
            result: Rc::new(RefCell::new(None)),
            task: RefCell::new(Some(f))
        }
    }
}

impl<T, F: FnOnce(Consumer<T>)> Future for Task<T, F> {
    type Output = T;
    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<T> {
        let result = self.result.borrow_mut().take();
        match result {
            Some(v) => Poll::Ready(v),
            None => {
                let task = self.task.borrow_mut().take();
                match task {
                    Some(f) => {
                        let waker = Rc::new(RefCell::new(None));
                        f(Consumer {
                            result: self.result.clone(),
                            waker: waker.clone()
                        });
                        waker.borrow_mut().replace(ctx.waker().clone());
                        self.poll(ctx)
                    }
                    None => Poll::Pending
                }
            }
        }
    }
}

impl<T> Consumer<T> {
    pub fn consume(self, v: T) {
        self.result.borrow_mut().replace(v);
        self.waker.borrow_mut().take().map(Waker::wake);
    }
}