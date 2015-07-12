#[macro_use]
extern crate chan;
extern crate chan_signal;

use chan_signal::{Signal, kill_this};

fn main() {
    let (s1, r1) = chan::sync(1);
    let (s2, r2) = chan::sync(1);
    chan_signal::notify_on(&s1, Signal::HUP);
    chan_signal::notify_on(&s2, Signal::HUP);
    kill_this(Signal::HUP);
    assert_eq!(r1.recv(), Some(Signal::HUP));
    assert_eq!(r2.recv(), Some(Signal::HUP));
}
