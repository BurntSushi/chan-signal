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

After the first call to `notify` (or `notify_on`), all signals defined in the
`Signal` enum are set to *blocked*. This is necessary for synchronous signal
handling using `sigwait`.

After the signals are blocked, a new thread is spawned and immediately blocks
on a call to `sigwait`. It is only unblocked when one of the signals in
the `Signal` enum are sent to the process. Once it's unblocked, it sends the
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

extern crate bit_set;
#[macro_use] extern crate chan;
#[macro_use] extern crate lazy_static;
extern crate libc;

use std::collections::HashMap;
use std::io;
use std::mem;
use std::ptr;
use std::sync::Mutex;
use std::thread;

use bit_set::BitSet;
use chan::Sender;
use libc::consts::os::posix88::{
    SIGHUP, SIGINT, SIGQUIT, SIGILL, SIGABRT, SIGFPE, SIGKILL,
    SIGSEGV, SIGPIPE, SIGALRM, SIGTERM,
};
use libc::funcs::posix88::signal::kill;
use libc::funcs::posix88::unistd::getpid;

lazy_static! {
    static ref HANDLERS: Mutex<HashMap<Sender<Signal>, BitSet>> = {
        init();
        Mutex::new(HashMap::new())
    };
}

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
    let mut subs = HANDLERS.lock().unwrap();
    if subs.contains_key(chan) {
        subs.get_mut(chan).unwrap().insert(signal.as_sig() as usize);
    } else {
        let mut sigs = BitSet::new();
        sigs.insert(signal.as_sig() as usize);
        subs.insert((*chan).clone(), sigs);
    }
}

fn init() {
    SigSet::subscribable().thread_block_signals().unwrap();
    thread::spawn(move || {
        let mut listen = SigSet::subscribable();
        loop {
            let sig = listen.wait().unwrap();
            let subs = HANDLERS.lock().unwrap();
            for (s, sigs) in subs.iter() {
                if !sigs.contains(&(sig as usize)) {
                    continue;
                }
                chan_select! {
                    default => {},
                    s.send(Signal::new(sig)) => {},
                }
            }
        }
    });
}

/// Kill the current process. (Only used in tests.)
#[doc(hidden)]
pub fn kill_this(sig: Signal) {
    unsafe { kill(getpid(), sig.as_sig()); }
}

type Sig = libc::c_int;

/// The set of subscribable signals.
///
/// After the first call to `notify_on` (or `notify`), precisely this set of
/// signals are set to blocked status.
#[allow(missing_docs)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
    #[doc(hidden)]
    __NonExhaustiveMatch,
}

impl Signal {
    fn new(sig: Sig) -> Signal {
        match sig {
            SIGHUP => Signal::HUP,
            SIGINT => Signal::INT,
            SIGQUIT => Signal::QUIT,
            SIGILL => Signal::ILL,
            SIGABRT => Signal::ABRT,
            SIGFPE => Signal::FPE,
            SIGKILL => Signal::KILL,
            SIGSEGV => Signal::SEGV,
            SIGPIPE => Signal::PIPE,
            SIGALRM => Signal::ALRM,
            SIGTERM => Signal::TERM,
            sig => panic!("unsupported signal number: {}", sig),
        }
    }

    fn as_sig(self) -> Sig {
        match self {
            Signal::HUP => SIGHUP,
            Signal::INT => SIGINT,
            Signal::QUIT => SIGQUIT,
            Signal::ILL => SIGILL,
            Signal::ABRT => SIGABRT,
            Signal::FPE => SIGFPE,
            Signal::KILL => SIGKILL,
            Signal::SEGV => SIGSEGV,
            Signal::PIPE => SIGPIPE,
            Signal::ALRM => SIGALRM,
            Signal::TERM => SIGTERM,
            Signal::__NonExhaustiveMatch => unreachable!(),
        }
    }
}

/// Safe wrapper around sigset_t.
struct SigSet(sigset_t);

impl SigSet {
    fn empty() -> SigSet {
        let mut set = unsafe { mem::uninitialized() };
        unsafe { sigemptyset(&mut set) };
        SigSet(set)
    }

    /// Creates a new signal set with precisely the signals we're limited
    /// to subscribing to.
    fn subscribable() -> SigSet {
        let mut set = SigSet::empty();
        set.add(SIGHUP).unwrap();
        set.add(SIGINT).unwrap();
        set.add(SIGQUIT).unwrap();
        set.add(SIGILL).unwrap();
        set.add(SIGABRT).unwrap();
        set.add(SIGFPE).unwrap();
        set.add(SIGKILL).unwrap();
        set.add(SIGSEGV).unwrap();
        set.add(SIGPIPE).unwrap();
        set.add(SIGALRM).unwrap();
        set.add(SIGTERM).unwrap();
        set
    }

    fn add(&mut self, sig: Sig) -> io::Result<()> {
        unsafe { ok_errno((), sigaddset(&mut self.0, sig)) }
    }

    fn wait(&mut self) -> io::Result<Sig> {
        let mut sig: Sig = 0;
        let errno = unsafe { sigwait(&mut self.0, &mut sig) };
        ok_errno(sig, errno)
    }

    fn thread_block_signals(&self) -> io::Result<()> {
        let ecode = unsafe {
            pthread_sigmask(SIG_SETMASK, &self.0, ptr::null_mut())
        };
        ok_errno((), ecode)
    }
}

fn ok_errno<T>(ok: T, ecode: libc::c_int) -> io::Result<T> {
    if ecode != 0 { Err(io::Error::from_raw_os_error(ecode)) } else { Ok(ok) }
}

extern {
    fn sigwait(set: *mut sigset_t, sig: *mut Sig) -> Sig;
    fn sigaddset(set: *mut sigset_t, sig: Sig) -> libc::c_int;
    fn sigemptyset(set: *mut sigset_t) -> libc::c_int;
    fn pthread_sigmask(
        how: libc::c_int,
        set: *const sigset_t,
        oldset: *mut sigset_t,
    ) -> libc::c_int;
}

// Most of this was lifted out of rust-lang:rust/src/libstd/sys/unix/c.rs.

#[cfg(all(any(target_os = "linux", target_os = "android"),
          any(target_arch = "x86",
              target_arch = "x86_64",
              target_arch = "powerpc",
              target_arch = "arm",
              target_arch = "aarch64")))]
const SIG_SETMASK: libc::c_int = 2;

#[cfg(all(any(target_os = "linux", target_os = "android"),
          any(target_arch = "mips", target_arch = "mipsel")))]
const SIG_SETMASK: libc::c_int = 3;

#[cfg(any(target_os = "macos",
          target_os = "ios",
          target_os = "freebsd",
          target_os = "dragonfly",
          target_os = "bitrig",
          target_os = "netbsd",
          target_os = "openbsd"))]
const SIG_SETMASK: libc::c_int = 3;

#[cfg(all(target_os = "linux", target_pointer_width = "32"))]
#[repr(C)]
struct sigset_t {
    __val: [libc::c_ulong; 32],
}

#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
#[repr(C)]
struct sigset_t {
    __val: [libc::c_ulong; 16],
}

#[cfg(target_os = "android")]
type sigset_t = libc::c_ulong;

#[cfg(any(target_os = "macos", target_os = "ios"))]
type sigset_t = u32;

#[cfg(any(target_os = "freebsd", target_os = "dragonfly"))]
#[repr(C)]
struct sigset_t {
    bits: [u32; 4],
}

#[cfg(any(target_os = "bitrig", target_os = "netbsd", target_os = "openbsd"))]
type sigset_t = libc::c_uint;
