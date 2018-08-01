## **This crate has reached its end-of-life and is now deprecated.**

The intended successor of the `chan` crate is the
[`crossbeam-channel`](https://github.com/crossbeam-rs/crossbeam-channel)
crate. Its API is strikingly similar, but comes with a much better `select!`
macro, better performance, a better test suite and an all-around better
implementation.

If you were previously using this crate for signal handling, then it is
simple to reproduce a similar API with `crossbeam-channel` and the
[`signal-hook`](https://github.com/vorner/signal-hook)
crate. For example, here's `chan-signal`'s `notify` function:

```rust
extern crate crossbeam_channel as channel;
extern crate signal_hook;

fn notify(signals: &[c_int]) -> Result<channel::Receiver<c_int>> {
    let (s, r) = channel::bounded(100);
    let signals = signal_hook::iterator::Signals::new(signals)?;
    thread::spawn(move || {
        for signal in signals.forever() {
            s.send(signal);
        }
    });
    Ok(r)
}
```

This crate may continue to receives bug fixes, but should otherwise be
considered dead.


chan-signal
===========

This crate provies experimental support for responding to OS signals using
[channels](https://github.com/BurntSushi/chan). Currently, this only works on
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

When combined with `chan_select!` from the `chan` crate, one can easily
integrate signals with the rest of your program. For example, consider a
main function that waits for either normal completion of work (which is done
in a separate thread) or for a signal to be delivered:

```rust
#[macro_use]
extern crate chan;
extern crate chan_signal;

use std::thread;
use std::time::Duration;

use chan_signal::Signal;

fn main() {
    // Signal gets a value when the OS sent a INT or TERM signal.
    let signal = chan_signal::notify(&[Signal::INT, Signal::TERM]);
    // When our work is complete, send a sentinel value on `sdone`.
    let (sdone, rdone) = chan::sync(0);
    // Run work.
    thread::spawn(move || run(sdone));

    // Wait for a signal or for work to be done.
    chan_select! {
        signal.recv() -> signal => {
            println!("received signal: {:?}", signal)
        },
        rdone.recv() => {
            println!("Program completed normally.");
        }
    }
}

fn run(_sdone: chan::Sender<()>) {
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
