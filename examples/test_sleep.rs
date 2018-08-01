extern crate crossbeam_channel;
extern crate chan_signal;

use std::thread;
use std::time::Duration;

use chan_signal::{Signal, kill_this};

fn main() {
    let (s, r) = crossbeam_channel::bounded(1);
    chan_signal::notify_on(&s, Signal::HUP);
    thread::spawn(move || thread::sleep(Duration::from_secs(10)));
    thread::sleep(Duration::from_millis(500));
    kill_this(Signal::HUP);
    assert_eq!(r.recv(), Some(Signal::HUP));
}
