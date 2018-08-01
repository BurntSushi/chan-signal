extern crate crossbeam_channel;
extern crate chan_signal;

use chan_signal::{Signal, kill_this, block};

fn main() {
    block(&[Signal::TERM]);
    let (s, r) = crossbeam_channel::bounded(1);
    chan_signal::notify_on(&s, Signal::HUP);
    kill_this(Signal::TERM);
    kill_this(Signal::HUP);
    assert_eq!(r.recv(), Some(Signal::HUP));
}
