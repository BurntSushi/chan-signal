use libc;
use libc::{
    // POSIX.1-2008, minus SIGPOLL (not in some BSD, use SIGIO)
    SIGHUP, SIGINT, SIGQUIT, SIGILL, SIGABRT, SIGFPE, SIGKILL,
    SIGSEGV, SIGPIPE, SIGALRM, SIGTERM, SIGUSR1, SIGUSR2,
    SIGCHLD, SIGCONT, SIGSTOP, SIGTSTP, SIGTTIN, SIGTTOU,
    SIGBUS, SIGPROF, SIGSYS, SIGTRAP, SIGURG, SIGVTALRM,
    SIGXCPU, SIGXFSZ,

    // Common Extensions (SIGINFO and SIGEMT not in libc)
    SIGIO,
    SIGWINCH,

    SIG_BLOCK,
    SIG_SETMASK,
};
use libc::kill;
use libc::getpid;

use std::collections::HashMap;
use std::io;
use std::mem;
use std::ptr;
use std::sync::Mutex;
use std::thread;

use bit_set::BitSet;
use chan::Sender;
use super::Signal;

lazy_static! {
    static ref HANDLERS: Mutex<HashMap<Sender<Signal>, BitSet>> = {
        init();
        Mutex::new(HashMap::new())
    };
}

#[doc(hidden)]
pub fn _notify_on(chan: &Sender<Signal>, signal: Signal) {
    let mut subs = HANDLERS.lock().unwrap();
    if subs.contains_key(chan) {
        subs.get_mut(chan).unwrap().insert(signal.as_sig() as usize);
    } else {
        let mut sigs = BitSet::new();
        sigs.insert(signal.as_sig() as usize);
        subs.insert((*chan).clone(), sigs);
    }

    // Make sure that the signal that we want notifications on is blocked
    // It does not matter if we block the same signal twice.
    _block(&[signal]);
}

#[doc(hidden)]
pub fn _block(signals: &[Signal]) {
    let mut block = SigSet::empty();
    for signal in signals {
        block.add(signal.as_sig()).unwrap();
    }
    block.thread_block_signals().unwrap();
}

#[doc(hidden)]
pub fn _block_all_subscribable() {
    SigSet::subscribable().thread_block_signals().unwrap();
}

fn init() {
    // First:
    // Get the curren thread_mask. (We cannot just overwrite the threadmask with
    // an empty one because this function is executed lazily.
    let saved_mask = SigSet::current().unwrap();

    // Then:
    // Block all signals in this thread. The signal mask will then be inherited
    // by the worker thread.
    SigSet::subscribable().thread_set_signal_mask().unwrap();
    thread::spawn(move || {
        let mut listen = SigSet::subscribable();

        loop {
            let sig = listen.wait().unwrap();
            let subs = HANDLERS.lock().unwrap();
            for (s, sigs) in subs.iter() {
                if !sigs.contains(sig as usize) {
                    continue;
                }
                chan_select! {
                    default => {},
                    s.send(Signal::new(sig)) => {},
                }
            }
        }
    });

    // Now:
    // Reset to the previously saved sigmask.
    // This whole procedure is necessary, as we cannot rely on the worker thread
    // starting fast enough to set its signal mask. Otherwise an early SIGTERM or
    // similar may take down the process even though the main thread has blocked
    // the signal.
    saved_mask.thread_set_signal_mask().unwrap();
}

type Sig = libc::c_int;

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
            SIGUSR1 => Signal::USR1,
            SIGUSR2 => Signal::USR2,
            SIGCHLD => Signal::CHLD,
            SIGCONT => Signal::CONT,
            SIGSTOP => Signal::STOP,
            SIGTSTP => Signal::TSTP,
            SIGTTIN => Signal::TTIN,
            SIGTTOU => Signal::TTOU,
            SIGBUS => Signal::BUS,
            SIGPROF => Signal::PROF,
            SIGSYS => Signal::SYS,
            SIGTRAP => Signal::TRAP,
            SIGURG => Signal::URG,
            SIGVTALRM => Signal::VTALRM,
            SIGXCPU => Signal::XCPU,
            SIGXFSZ => Signal::XFSZ,
            SIGIO => Signal::IO,
            SIGWINCH => Signal::WINCH,
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
            Signal::USR1 => SIGUSR1,
            Signal::USR2 => SIGUSR2,
            Signal::CHLD => SIGCHLD,
            Signal::CONT => SIGCONT,
            Signal::STOP => SIGSTOP,
            Signal::TSTP => SIGTSTP,
            Signal::TTIN => SIGTTIN,
            Signal::TTOU => SIGTTOU,
            Signal::BUS => SIGBUS,
            Signal::PROF => SIGPROF,
            Signal::SYS => SIGSYS,
            Signal::TRAP => SIGTRAP,
            Signal::URG => SIGURG,
            Signal::VTALRM => SIGVTALRM,
            Signal::XCPU => SIGXCPU,
            Signal::XFSZ => SIGXFSZ,
            Signal::IO => SIGIO,
            Signal::WINCH => SIGWINCH,
            Signal::__NonExhaustiveMatch => unreachable!(),
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

    fn current() -> io::Result<SigSet> {
        let mut set = unsafe { mem::uninitialized() };
        let ecode = unsafe {
            pthread_sigmask(SIG_SETMASK, ptr::null_mut(), &mut set)
        };
        ok_errno(SigSet(set), ecode)
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
        set.add(SIGUSR1).unwrap();
        set.add(SIGUSR2).unwrap();
        set.add(SIGCHLD).unwrap();
        set.add(SIGCONT).unwrap();
        set.add(SIGSTOP).unwrap();
        set.add(SIGTSTP).unwrap();
        set.add(SIGTTIN).unwrap();
        set.add(SIGTTOU).unwrap();
        set.add(SIGBUS).unwrap();
        set.add(SIGPROF).unwrap();
        set.add(SIGSYS).unwrap();
        set.add(SIGTRAP).unwrap();
        set.add(SIGURG).unwrap();
        set.add(SIGVTALRM,).unwrap();
        set.add(SIGXCPU).unwrap();
        set.add(SIGXFSZ).unwrap();
        set.add(SIGIO).unwrap();
        set.add(SIGWINCH).unwrap();
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
            pthread_sigmask(SIG_BLOCK, &self.0, ptr::null_mut())
        };
        ok_errno((), ecode)
    }

    fn thread_set_signal_mask(&self) -> io::Result<()> {
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