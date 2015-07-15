#[macro_use] extern crate chan;
extern crate chan_signal;

use std::thread;

use chan_signal::{Signal, notify};

fn main() {
    let signal = notify(&[Signal::INT]);
    println!("Send a TERM signal my way!");
    thread::spawn(move || thread::sleep_ms(10000));
    // thread::sleep_ms(5000);
    // block until we get a signal
    assert_eq!(signal.recv(), Some(Signal::INT));
    println!("Thanks :]");
}
