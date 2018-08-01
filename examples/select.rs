#[macro_use]
extern crate crossbeam_channel;
extern crate chan_signal;

use std::thread;
use std::time::Duration;

use chan_signal::Signal;

fn main() {
    // Signal gets a value when the OS sent a INT or TERM signal.
    let signal = chan_signal::notify(&[Signal::INT, Signal::TERM]);
    // When our work is complete, send a sentinel value on `sdone`.
    let (sdone, rdone) = crossbeam_channel::bounded(0);
    // Run work.
    thread::spawn(move || run(sdone));

    // Wait for a signal or for work to be done.
    select! {
        recv(signal, signal) => {
            println!("received signal: {:?}", signal)
        },
        recv(rdone) => {
            println!("Program completed normally.");
        }
    }
}

fn run(_sdone: crossbeam_channel::Sender<()>) {
    println!("Running work for 5 seconds.");
    println!("Can you send a signal quickly enough?");
    // Do some work.
    thread::sleep(Duration::from_secs(5));
}
