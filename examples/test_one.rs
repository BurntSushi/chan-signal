#[macro_use]
extern crate chan;
extern crate chan_signal;

use chan_signal::{Signal, kill_this};

fn main() {
    let (s, r) = chan::sync(1);
    chan_signal::notify_on(&s, Signal::HUP);
    kill_this(Signal::HUP);
    assert_eq!(r.recv(), Some(Signal::HUP));
}
