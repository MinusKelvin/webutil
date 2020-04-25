use std::future::Future;
use std::task::{ Poll, Context, Waker };
use std::rc::Rc;
use std::pin::Pin;
use std::cell::RefCell;
use std::collections::VecDeque;

pub struct Sender<T>(Rc<RefCell<ChannelState<T>>>);

pub struct Receiver<T>(Rc<RefCell<ChannelState<T>>>);

struct ChannelState<T> {
    recvs: u32,
    waker: Option<Waker>,
    senders: u32,
    queue: VecDeque<T>
}

impl<T> Sender<T> {
    pub fn send(&self, v: T) -> Result<(), T> {
        let mut state = self.0.borrow_mut();
        if state.recvs > 0 {
            state.queue.push_back(v);
            if let Some(waker) = state.waker.take() {
                waker.wake()
            }
            Ok(())
        } else {
            Err(v)
        }
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        let mut state = self.0.borrow_mut();
        state.senders -= 1;
        if state.senders == 0 {
            if let Some(waker) = state.waker.take() {
                waker.wake();
            }
        }
    }
}

impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        self.0.borrow_mut().senders += 1;
        Sender(self.0.clone())
    }
}

struct RecvFuture<'a, T>(&'a Receiver<T>);
impl<T> Future for RecvFuture<'_, T> {
    type Output = Option<T>;
    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Option<T>> {
        match self.0.try_recv() {
            Ok(v) => Poll::Ready(Some(v)),
            Err(TryRecvError::Closed) => Poll::Ready(None),
            Err(TryRecvError::Empty) => {
                self.0 .0.borrow_mut().waker.replace(ctx.waker().clone());
                Poll::Pending
            }
        }
    }
}

impl<T> Receiver<T> {
    pub async fn recv(&self) -> Option<T> {
        RecvFuture(&self).await
    }

    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        let mut state = self.0.borrow_mut();
        match state.queue.pop_front() {
            Some(v) => Ok(v),
            None => if state.senders == 0 {
                Err(TryRecvError::Closed)
            } else {
                Err(TryRecvError::Empty)
            }
        }
    }
}

impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        self.0.borrow_mut().recvs -= 1;
    }
}

impl<T> Clone for Receiver<T> {
    fn clone(&self) -> Self {
        self.0.borrow_mut().recvs += 1;
        Receiver(self.0.clone())
    }
}

pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let state = Rc::new(RefCell::new(ChannelState {
        recvs: 1,
        senders: 1,
        waker: None,
        queue: VecDeque::new()
    }));
    (Sender(state.clone()), Receiver(state))
}

#[derive(Debug, Eq, PartialEq)]
pub enum TryRecvError {
    Empty,
    Closed
}

struct OneshotState<T> {
    v: Option<T>,
    waker: Option<Waker>,
    recv_exists: bool,
    send_exists: bool
}

pub fn oneshot<T>() -> (Oneshot<T>, Once<T>) {
    let state = Rc::new(RefCell::new(OneshotState {
        v: None,
        waker: None,
        recv_exists: true,
        send_exists: true
    }));
    (Oneshot(state.clone()), Once(state))
}

pub struct Oneshot<T>(Rc<RefCell<OneshotState<T>>>);

pub struct Once<T>(Rc<RefCell<OneshotState<T>>>);

impl<T> Oneshot<T> {
    pub fn resolve(self, v: T) -> Result<(), T> {
        let mut state = self.0.borrow_mut();
        if state.recv_exists {
            state.v.replace(v);
            if let Some(waker) = state.waker.take() {
                waker.wake()
            }
            Ok(())
        } else {
            Err(v)
        }
    }
}

impl<T> Once<T> {
    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        let mut state = self.0.borrow_mut();
        match state.v.take() {
            Some(v) => Ok(v),
            None if state.send_exists => Err(TryRecvError::Empty),
            None => Err(TryRecvError::Closed)
        }
    }
}

impl<T> Drop for Oneshot<T> {
    fn drop(&mut self) {
        let mut state = self.0.borrow_mut();
        state.send_exists = false;
        if let Some(waker) = state.waker.take() {
            waker.wake();
        }
    }
}

impl<T> Drop for Once<T> {
    fn drop(&mut self) {
        self.0.borrow_mut().recv_exists = false;
    }
}

impl<T> Future for Once<T> {
    type Output = Option<T>;
    fn poll(self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Option<T>> {
        match self.try_recv() {
            Ok(v) => Poll::Ready(Some(v)),
            Err(TryRecvError::Closed) => Poll::Ready(None),
            Err(TryRecvError::Empty) => {
                self.0.borrow_mut().waker.replace(ctx.waker().clone());
                Poll::Pending
            }
        }
    }
}