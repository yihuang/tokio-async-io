use std::io;
use std::os::unix::io::AsRawFd;

use mio::{unix::EventedFd, Evented, Poll, PollOpt, Ready, Token};
use tokio::io::PollEvented;

/// Wrapper to impl Evented
struct RawFdWrapper<T: AsRawFd>(T);

impl<T: AsRawFd> Evented for RawFdWrapper<T> {
    fn register(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        EventedFd(&self.0.as_raw_fd()).register(poll, token, interest, opts)
    }
    fn reregister(
        &self,
        poll: &Poll,
        token: Token,
        interest: Ready,
        opts: PollOpt,
    ) -> io::Result<()> {
        EventedFd(&self.0.as_raw_fd()).reregister(poll, token, interest, opts)
    }
    fn deregister(&self, poll: &Poll) -> io::Result<()> {
        EventedFd(&self.0.as_raw_fd()).deregister(poll)
    }
}

pub struct Async<T: AsRawFd>(PollEvented<RawFdWrapper<T>>);

impl<T: AsRawFd> Async<T> {
    pub fn new(io: T) -> io::Result<Self> {
        Ok(Self(PollEvented::new(RawFdWrapper(io))?))
    }

    pub fn get_ref(&self) -> &T {
        &self.0.get_ref().0
    }

    pub async fn read_with<R>(&self, mut op: impl FnMut(&T) -> io::Result<R>) -> io::Result<R> {
        loop {
            match op(self.get_ref()) {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                res => return res,
            }
            self.readable().await?;
        }
    }

    pub async fn write_with<R>(&self, mut op: impl FnMut(&T) -> io::Result<R>) -> io::Result<R> {
        loop {
            match op(self.get_ref()) {
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {}
                res => return res,
            }
            self.writable().await?;
        }
    }

    async fn readable(&self) -> io::Result<()> {
        let mut polled = false;
        futures::future::poll_fn(|cx| {
            if polled {
                futures::task::Poll::Ready(Ok(()))
            } else {
                self.0.clear_read_ready(cx, Ready::readable())?;
                polled = true;
                futures::task::Poll::Pending
            }
        })
        .await
    }

    async fn writable(&self) -> io::Result<()> {
        let mut polled = false;
        futures::future::poll_fn(|cx| {
            if polled {
                futures::task::Poll::Ready(Ok(()))
            } else {
                self.0.clear_read_ready(cx, Ready::writable())?;
                polled = true;
                futures::task::Poll::Pending
            }
        })
        .await
    }
}
