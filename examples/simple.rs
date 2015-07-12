// This is a minimal example that demonstrates how to listen and respond to
// OS signals. Namely, it requests to be notified about a SIGINT (usually
// ^C in your terminal) and blocks until it gets one.

#[macro_use] extern crate chan;
extern crate chan_signal;

use chan_signal::{Signal, notify};

fn main() {
    let signal = notify(&[Signal::INT]);
    println!("Send a TERM signal my way!");
    // block until we get a signal
    assert_eq!(signal.recv(), Some(Signal::INT));
    println!("Thanks :]");
}
