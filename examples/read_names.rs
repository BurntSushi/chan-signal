// This example demonstrates how to do something almost useful with OS signals.
// This program asks the user to inputs names. It then outputs these names
// once the user inputs EOF. The catch is that it will *also* output these
// names if the user sends the process SIGINT or SIGTERM.
//
// This is hard or impossible to do with regular asynchronous signal handlers.
// But with a channel based API, it's easy to integrate with the rest of
// your control flow.

#[macro_use] extern crate chan;
extern crate chan_signal;

use std::error::Error;
use std::io::{self, BufRead, Write};
use std::process;
use std::thread;

use chan_signal::{Signal, notify};

fn main() {
    // It is imperative that we start listening for signals as soon as
    // possible. In particular, if `notify` is called after another thread
    // has spawned, then signal masking won't be applied to it and signal
    // handling won't work.
    //
    // See "Signal mask and pending signals" section of signal(7).
    let signal = notify(&[Signal::INT, Signal::TERM]);
    match run(signal) {
        Ok(mut names) => {
            names.sort();
            println!("You entered {} names: {}",
                     names.len(), names.connect(", "));
        }
        Err(err) => {
            writeln!(&mut io::stderr(), "{}", err).unwrap();
            process::exit(1);
        }
    }
}

type Result<T> = ::std::result::Result<T, Box<Error+Send+Sync>>;

fn run(signal: chan::Receiver<Signal>) -> Result<Vec<String>> {
    let lines = read_stdin_lines();
    let mut names = vec![];
    println!("Please enter some names, each on a new line:");
    loop {
        chan_select! {
            lines.recv() -> line => {
                match line {
                    // If the channel closed (i.e., reads EOF), then quit
                    // the loop and print what we've got.
                    //
                    // The rightward drift is painful...
                    None => break,
                    Some(line) => names.push(try!(line).trim().to_owned()),
                }
            },
            // If we get SIGINT or SIGTERM, just stop the loop and print
            // what we've got so far.
            signal.recv() => break,
        }
    }
    Ok(names)
}

// Spawns a new thread to read lines and sends the result on the returned
// channel.
fn read_stdin_lines() -> chan::Receiver<io::Result<String>> {
    let (s, r) = chan::sync(0);
    let stdin = io::stdin();
    thread::spawn(move || {
        let stdin = stdin.lock();
        for line in stdin.lines() {
            s.send(line);
        }
    });
    r
}
