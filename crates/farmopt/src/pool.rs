//! Parallel maps sized from the machine (the box's cores, less a reserve for the other agents:
//! AGENTS.md "Shared boxes"), and the thread count every stage uses.

use std::sync::atomic::{AtomicUsize, Ordering};

/// The threads a search uses: `FARMOPT_THREADS` if set, else the machine's cores less 1/16
/// (at least 1).
pub fn threads() -> usize {
    if let Some(n) = std::env::var("FARMOPT_THREADS").ok().and_then(|v| v.parse::<usize>().ok()) {
        return n.max(1);
    }
    let n = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8);
    (n - n / 16).max(1)
}

/// `f` over `items` on `threads` threads, results in input order. Items are taken in order (the
/// first ones finish first), so a caller can rank them best first.
pub fn par_map<T: Sync, R: Send>(items: &[T], threads: usize, f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let next = AtomicUsize::new(0);
    let n = items.len();
    let mut out: Vec<Option<R>> = (0..n).map(|_| None).collect();
    let slots: Vec<std::sync::Mutex<&mut Option<R>>> = out.iter_mut().map(std::sync::Mutex::new).collect();
    std::thread::scope(|s| {
        for _ in 0..threads.clamp(1, n.max(1)) {
            s.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= n {
                        break;
                    }
                    let r = f(&items[i]);
                    if let Ok(mut slot) = slots[i].lock() {
                        **slot = Some(r);
                    }
                }
            });
        }
    });
    drop(slots);
    out.into_iter().map(|r| r.expect("every item mapped")).collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn par_map_keeps_the_order() {
        let v: Vec<u64> = (0..1000).collect();
        let r = super::par_map(&v, 7, |x| x * x);
        assert_eq!(r, v.iter().map(|x| x * x).collect::<Vec<_>>());
        assert!(super::par_map(&Vec::<u8>::new(), 4, |x| *x).is_empty());
    }
}
