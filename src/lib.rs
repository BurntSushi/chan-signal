/*!
This crate provides a simplistic interface to subscribe to operating system
signals through a channel API. Use is extremely simple:

```no_run
use chan_signal::Signal;

let signal = chan_signal::notify(&[Signal::INT, Signal::TERM]);

// Blocks until this process is sent an INT or TERM signal.
// Since the channel is never closed, we can unwrap the received value.
signal.recv().unwrap();
```


# Example

When combined with `chan_select!` from the `chan` crate, one can easily
integrate signals with the rest of your program. For example, consider a
main function that waits for either normal completion of work (which is done
in a separate thread) or for a signal to be delivered:

```no_run
#[macro_use]
extern crate chan;
extern crate chan_signal;

use chan_signal::Signal;

fn main() {
    // Signal gets a value when the OS sent a INT or TERM signal.
    let signal = chan_signal::notify(&[Signal::INT, Signal::TERM]);
    // When our work is complete, send a sentinel value on `sdone`.
    let (sdone, rdone) = chan::sync(0);
    // Run work.
    ::std::thread::spawn(move || run(sdone));

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
    // Do some work.
    ::std::thread::sleep_ms(1000);
    // Quit normally.
    // Note that we don't need to send any values. We just let the
    // sending channel drop, which closes the channel, which causes
    // the receiver to synchronize immediately and always.
}
```

You can see this example in action by running `cargo run --example select`
in the root directory of this crate's
[repository](https://github.com/BurntSushi/chan-signal).

# Platform support (no Windows support)

This should work on Unix platforms supported by Rust itself.

There is no Windows support at all. I welcome others to either help me add it
or help educate me so that I may one day add it.


# How it works

Overview: uses the "spawn a thread and block on `sigwait`" approach. In
particular, it avoids standard asynchronous signal handling because it is
very difficult to do anything non-trivial inside a signal handler.

After a call to `notify`/`notify_on` (or `block`), the given signals are set
to *blocked*. This is necessary for synchronous signal handling using `sigwait`.

After the first call to `notify` (or `notify_on`), a new thread is spawned and
immediately blocks on a call to `sigwait`. It is only unblocked when one of the
signals that were masked previously by calls to `notify` etc. arrives, which now
cannot be delivered directly to any of the threads of the process, and therefore
unblocks the waiting signal watcher thread. Once it's unblocked, it sends the
signal on all subscribed channels via a non-blocking send. Once all channels
have been visited, the thread blocks on `sigwait` again.

This approach has some restrictions. Namely, your program must comply with the
following:

* Any and all threads spawned in your program **must** come after the first
  call to `notify` (or `notify_on`). This is so all spawned threads inherit
  the blocked status of signals. If a thread starts before `notify` is called,
  it will not have the correct signal mask. When a signal is delivered, the
  result is indeterminate.
* No other threads may call `sigwait`. When a signal is delivered, only one
  `sigwait` is indeterminately unblocked.


# Future work

This crate exposes the simplest API I could think of. As a result, a few
additions may be warranted:

* Expand the set of signals. (Requires figuring out platform differences.)
* Allow channel unsubscription.
* Allow callers to reset the signal mask? (Seems hard.)
* Support Windows.
*/
#![deny(missing_docs)]
#![allow(unused_imports)] // TODO: Remove

extern crate bit_set;
#[macro_use] extern crate chan;
#[macro_use] extern crate lazy_static;
extern crate libc;

#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix::*;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows::*;

use chan::Sender;


/// Create a new channel subscribed to the given signals.
///
/// The channel returned is never closed.
///
/// This is a convenience function for subscribing to multiple signals at once.
/// See the documentation of `notify_on` for details.
///
/// The channel returned has a small buffer to prevent signals from being
/// dropped.
///
/// **THIS MUST BE CALLED BEFORE ANY OTHER THREADS ARE SPAWNED IN YOUR
/// PROCESS.**
///
/// # Example
///
/// ```no_run
/// use chan_signal::Signal;
///
/// let signal = chan_signal::notify(&[Signal::INT, Signal::TERM]);
///
/// // Blocks until this process is sent an INT or TERM signal.
/// // Since the channel is never closed, we can unwrap the received value.
/// signal.recv().unwrap();
/// ```
pub fn notify(signals: &[Signal]) -> chan::Receiver<Signal> {
    let (s, r) = chan::sync(100);
    for &sig in signals {
        notify_on(&s, sig);
    }
    // dropping `s` is OK because `notify_on` acquires one.
    r
}

/// Subscribe to a signal on a channel.
///
/// When `signal` is delivered to this process, it will be sent on the channel
/// given.
///
/// Note that a signal is sent using a non-blocking send. Namely, if the
/// channel's buffer is full (or it has no buffer) and it isn't ready to
/// rendezvous, then the signal will be dropped.
///
/// There is currently no way to unsubscribe. Moreover, the channel given
/// here will be alive for the lifetime of the process. Therefore, the channel
/// will never be closed.
///
/// **THIS MUST BE CALLED BEFORE ANY OTHER THREADS ARE SPAWNED IN YOUR
/// PROCESS.**
pub fn notify_on(chan: &Sender<Signal>, signal: Signal) {
    _notify_on(chan, signal);
}

/// Block all given signals without receiving notifications.
///
/// If a signal has also been passed to `notify`/`notify_on` this function
/// does not have any effect in terms of that signal.
///
/// **THIS MUST BE CALLED BEFORE ANY OTHER THREADS ARE SPAWNED IN YOUR
/// PROCESS.**
pub fn block(signals: &[Signal]) {
    _block(signals);
}

/// Block all subscribable signals.
///
/// Calling this function effectively restores the default behavior of
/// version <= 0.2.0 of this library.
///
/// **THIS MUST BE CALLED BEFORE ANY OTHER THREADS ARE SPAWNED IN YOUR
/// PROCESS.**
pub fn block_all_subscribable() {
    _block_all_subscribable();
}

#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Signal {
    HUP,
    INT,
    QUIT,
    ILL,
    ABRT,
    FPE,
    KILL,
    SEGV,
    PIPE,
    ALRM,
    TERM,
    USR1,
    USR2,
    CHLD,
    CONT,
    STOP,
    TSTP,
    TTIN,
    TTOU,
    BUS,
    PROF,
    SYS,
    TRAP,
    URG,
    VTALRM,
    XCPU,
    XFSZ,
    IO,
    WINCH,
    #[doc(hidden)]
    __NonExhaustiveMatch,
}