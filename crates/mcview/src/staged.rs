//! Long work spread over the browser's frames (the coordinator 2026-10-22: pressing play on a big
//! farm or smelter ran one main-thread task of 300-450 ms while the judge's session was built).
//! The work is an `async` function with pause points ([`Budget::pause`]) in its loops; the
//! browser polls it once a frame ([`Task::poll`]) with a few milliseconds of [`Budget`], the
//! native tools run it to its end at once ([`block_on`]). Both run the very same code: the same
//! ticks in the same order (the judge's seeding is exact either way).

use std::cell::Cell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};

/// When a staged piece of work pauses: never ([`Budget::none`], the native tools), or once its
/// clock passes the deadline its executor sets before each poll ([`Budget::with_clock`]).
#[derive(Clone)]
pub struct Budget {
    clock: Option<Rc<dyn Fn() -> f64>>,
    deadline: Rc<Cell<f64>>,
    pauses: Rc<Cell<u32>>,
}

impl Budget {
    /// Never pauses: the work runs to its end in one go.
    pub fn none() -> Budget {
        Budget { clock: None, deadline: Rc::new(Cell::new(f64::INFINITY)), pauses: Rc::new(Cell::new(0)) }
    }

    /// Pauses once `clock` (ms) passes the deadline ([`Budget::give`]).
    pub fn with_clock(clock: Rc<dyn Fn() -> f64>) -> Budget {
        Budget { clock: Some(clock), deadline: Rc::new(Cell::new(f64::NEG_INFINITY)), pauses: Rc::new(Cell::new(0)) }
    }

    /// The next poll may run `ms` milliseconds from now.
    pub fn give(&self, ms: f64) {
        let now = self.clock.as_ref().map(|c| c()).unwrap_or(0.0);
        self.deadline.set(now + ms);
    }

    /// How many times the work paused so far.
    pub fn pauses(&self) -> u32 {
        self.pauses.get()
    }

    /// Whether the time is up (a [`Budget::pause`] now would yield): for work that checks the
    /// clock itself between its own steps (`scene::Staged`, `scene::CameraSearch`).
    pub fn over(&self) -> bool {
        self.clock.as_ref().is_some_and(|c| c() >= self.deadline.get())
    }

    /// A pause point: yields to the executor when the time is up (the next poll goes on from here).
    pub async fn pause(&self) {
        let Some(c) = &self.clock else { return };
        if c() >= self.deadline.get() {
            self.pauses.set(self.pauses.get() + 1);
            YieldOnce(false).await;
        }
    }
}

/// Pending once, then ready.
struct YieldOnce(bool);

impl Future for YieldOnce {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<()> {
        if self.0 {
            Poll::Ready(())
        } else {
            self.0 = true;
            Poll::Pending
        }
    }
}

/// Run a piece of work to its end (its pause points come back at once: polled again).
pub fn block_on<F: Future>(f: F) -> F::Output {
    let mut f = std::pin::pin!(f);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(v) = f.as_mut().poll(&mut cx) {
            return v;
        }
    }
}

/// A piece of work polled frame by frame.
pub struct Task<T> {
    fut: Pin<Box<dyn Future<Output = T>>>,
}

impl<T> Task<T> {
    pub fn new(f: impl Future<Output = T> + 'static) -> Task<T> {
        Task { fut: Box::pin(f) }
    }

    /// Run it until its next pause; its result once it is over.
    pub fn poll(&mut self) -> Option<T> {
        let mut cx = Context::from_waker(Waker::noop());
        match self.fut.as_mut().poll(&mut cx) {
            Poll::Ready(v) => Some(v),
            Poll::Pending => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A budget pauses at its pause points once its clock passes the deadline; the work goes on
    /// where it paused and ends with the same result as run in one go.
    #[test]
    fn a_staged_sum_pauses_and_ends_the_same() {
        async fn sum(n: u64, b: Budget) -> u64 {
            let mut s = 0;
            for i in 0..n {
                s += i * i;
                b.pause().await;
            }
            s
        }
        let whole = block_on(sum(1000, Budget::none()));
        let t = Rc::new(Cell::new(0.0));
        let clock = {
            let t = t.clone();
            Rc::new(move || {
                t.set(t.get() + 1.0);
                t.get()
            })
        };
        let b = Budget::with_clock(clock);
        let mut task = Task::new(sum(1000, b.clone()));
        let mut polls = 0;
        let got = loop {
            b.give(10.0);
            polls += 1;
            if let Some(v) = task.poll() {
                break v;
            }
        };
        assert_eq!(got, whole);
        assert!(polls > 50 && b.pauses() + 1 == polls, "{polls} polls, {} pauses", b.pauses());
    }
}
