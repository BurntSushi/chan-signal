#[macro_use] extern crate chan;
extern crate chan_signal;

use std::thread;
use std::time::Duration;

use chan_signal::{Signal, notify};

fn main() {
    let signal = notify(&[Signal::INT]);
    println!("Send a INT signal my way!");
    thread::spawn(move || thread::sleep(Duration::from_secs(10)));
    // block until we get a signal
    assert_eq!(signal.recv(), Some(Signal::INT));
    println!("Thanks :]");
}
