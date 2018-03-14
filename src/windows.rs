extern crate kernel32;
extern crate winapi;

use std::collections::{HashSet, HashMap};
use std::io;
use std::sync::Mutex;

use bit_set::BitSet;
use chan::Sender;

use self::winapi::shared::minwindef::{BOOL, DWORD, FALSE, TRUE};
use super::Signal;

const CTRL_C_EVENT: DWORD = 0;
const CTRL_BREAK_EVENT: DWORD = 1;
const CTRL_CLOSE_EVENT: DWORD = 2;

lazy_static! {
    static ref HANDLERS: Mutex<HashMap<Sender<Signal>, HashSet<Signal>>> = {
        init().unwrap();
        Mutex::new(HashMap::new())
    };

    static ref BLOCK: Mutex<HashSet<Signal>> = Mutex::new(HashSet::new());
}

unsafe extern "system" fn ctrl_handler(ctrl_type: DWORD) -> BOOL {
    let sig = Signal::new(ctrl_type);
    if !BLOCK.lock().unwrap().contains(&sig) {
        return FALSE;
    }

    let subs = HANDLERS.lock().unwrap();
    for (s, sigs) in subs.iter() {
        if sigs.contains(&sig) {
            s.send(sig);
        }
    }

    TRUE
}

#[doc(hidden)]
pub fn _notify_on(chan: &Sender<Signal>, signal: Signal) {
    let mut subs = HANDLERS.lock().unwrap();
    if subs.contains_key(chan) {
        subs.get_mut(chan).unwrap().insert(signal);
    } else {
        let mut sigs = HashSet::new();
        sigs.insert(signal);
        subs.insert((*chan).clone(), sigs);
    }

    // Make sure that the signal that we want notifications on is blocked
    // It does not matter if we block the same signal twice.
    _block(&[signal]);
}

#[doc(hidden)]
pub fn _block(signals: &[Signal]) {
    let mut blocks = BLOCK.lock().unwrap();
    for signal in signals {
        blocks.insert(*signal);
    }
}

#[doc(hidden)]
pub fn _block_all_subscribable() {
    for signal in &[Signal::INT, Signal::TERM] {
        _block(&[*signal]);
    }
}

fn init() -> Result<(), io::Error> {
    unsafe {
        if kernel32::SetConsoleCtrlHandler(Some(ctrl_handler), TRUE) == FALSE {
            return Err(io::Error::last_os_error());
        }
    }

    Ok(())
}

impl Signal {
    fn new(sig: DWORD) -> Signal {
        match sig {
            CTRL_C_EVENT => Signal::INT,
            CTRL_BREAK_EVENT => Signal::INT,
            CTRL_CLOSE_EVENT => Signal::TERM,
            _ => panic!("unsupported win signal {:?}", sig)
        }
    }
}