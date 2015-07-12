extern crate bit_set;
#[macro_use] extern crate chan;
#[macro_use] extern crate lazy_static;
extern crate libc;

use std::collections::HashMap;
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

pub fn notify(signals: &[Signal]) -> chan::Receiver<Signal> {
    let (s, r) = chan::sync(1);
    for &sig in signals {
        notify_on(&s, sig);
    }
    // dropping `s` is OK because `notify` acquires one.
    r
}

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
    SigSet::posix88().mask_thread().unwrap();
    thread::spawn(move || {
        let mut listen = SigSet::posix88();
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

#[doc(hidden)]
pub fn kill_this(sig: Signal) {
    unsafe { kill(getpid(), sig.as_sig()); }
}

type Sig = libc::c_int;

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
        }
    }
}

struct SigSet(sigset_t);

impl SigSet {
    fn empty() -> SigSet {
        let mut set = unsafe { mem::uninitialized() };
        unsafe { sigemptyset(&mut set) };
        SigSet(set)
    }

    fn posix88() -> SigSet {
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

    fn add(&mut self, sig: Sig) -> Result<(), ()> {
        if unsafe { sigaddset(&mut self.0, sig) } != 0 {
            Err(())
        } else {
            Ok(())
        }
    }

    fn wait(&mut self) -> Result<Sig, ()> {
        let mut sig: Sig = 0;
        if unsafe { sigwait(&mut self.0, &mut sig) } != 0 {
            Err(())
        } else {
            Ok(sig)
        }
    }

    fn mask_thread(&self) -> Result<(), ()> {
        let err = unsafe {
            pthread_sigmask(SIG_SETMASK, &self.0, ptr::null_mut())
        };
        if err != 0 {
            Err(())
        } else {
            Ok(())
        }
    }
}

// Sizes are taken from: /usr/include/bits/sigset.h
// namely: # define _SIGSET_NWORDS  (1024 / (8 * sizeof (unsigned long int)))
#[repr(C)]
#[cfg(target_pointer_width = "32")]
struct sigset_t {
    __val: [libc::c_ulong; 32],
}

#[repr(C)]
#[cfg(target_pointer_width = "64")]
struct sigset_t {
    __val: [libc::c_ulong; 16],
}

// Taken from /usr/include/bits/sigaction.h
const SIG_SETMASK: libc::c_int = 2;

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
