extern crate crossbeam_channel;
extern crate chan_signal;

use chan_signal::{Signal, kill_this};

fn main() {
    let r_usr1 = chan_signal::notify(&[Signal::USR1, Signal::ALRM]);
    kill_this(Signal::USR1);
    kill_this(Signal::ALRM);
    assert_eq!(r_usr1.recv(), Some(Signal::USR1));
    assert_eq!(r_usr1.recv(), Some(Signal::ALRM));

    let (s, r_usr2) = crossbeam_channel::bounded(1);
    chan_signal::notify_on(&s, Signal::USR2);
    kill_this(Signal::USR2);
    assert_eq!(r_usr2.recv(), Some(Signal::USR2));

    // The following will terminate the process, as it is NOT blocked
    // by the main thread.
    kill_this(Signal::TERM);
    unreachable!();
}
