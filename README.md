This crate provies experimental support for responding to OS signals using
[channels](https://github.com/crossbeam-rs/crossbeam-channel). Currently, this only works on
Unix based systems, but I'd appreciate help adding Windows support.

[![Build status](https://api.travis-ci.org/BurntSushi/chan-signal.png)](https://travis-ci.org/BurntSushi/chan-signal)
[![](http://meritbadge.herokuapp.com/chan-signal)](https://crates.io/crates/chan-signal)

Dual-licensed under MIT or the [UNLICENSE](http://unlicense.org).


### Documentation

https://docs.rs/chan-signal


### Example

Use is really simple. Just ask the `chan_signal` crate to create a channel
subscribed to a set of signals. When a signal is sent to the process it will
be delivered to the channel.

```rust
use chan_signal::Signal;

let signal = chan_signal::notify(&[Signal::INT, Signal::TERM]);

// Blocks until this process is sent an INT or TERM signal.
// Since the channel is never closed, we can unwrap the received value.
signal.recv().unwrap();
```

### A realer example

When combined with `select!` from the `crossbeam-channel` crate, one can easily
integrate signals with the rest of your program. For example, consider a
main function that waits for either normal completion of work (which is done
in a separate thread) or for a signal to be delivered:

```rust
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

    // _sdone gets dropped which closes the channel and causes `rdone`
    // to unblock.
}
```

This is much easier than registering a signal handler because:

1. Signal handlers run asynchronously.
2. The code you're permitted to execute in a signal handler is extremely
   constrained (e.g., no allocation), so it is difficult to integrate
   it with the rest of your program.

Using channels, you can invent whatever flow you like and handle OS signals
just like anything else.


### How it works

TL;DR - Spawn a thread, block on `sigwait`, deliver signals, repeat.

It's
[explained a bit more in the docs](https://docs.rs/chan-signal/#how-it-works).
