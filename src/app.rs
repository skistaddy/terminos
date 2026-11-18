use crate::args::Args;
use crate::canvas;
use crate::input;
use crate::test;

use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::sync::mpsc::{Receiver, Sender, channel};

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        if cfg!(debug_assertions) {
            println!("{}", format!($($arg)*));
        }
    };
}

#[derive(Debug)]
pub enum Event {
    Key {
        code: u16,
        state: bool, // true = down, false = up
    },
    Mouse {
        x: i32,
        y: i32,
    },
    Scroll {
        x: i32,
        y: i32,
    },
    Log(String),
    Debug(String),
    Unknown,
}

static DEBUG: AtomicBool = AtomicBool::new(false);
pub fn debug() -> bool {
    DEBUG.load(Relaxed)
}

static TTY: AtomicBool = AtomicBool::new(false);
pub fn tty() -> bool {
    TTY.load(Relaxed)
}

pub struct App {
    pub tx: Sender<Event>,
    pub rx: Receiver<Event>,
}

impl App {
    pub fn send(&self, event: Event) {
        if self.tx.send(event).is_err() {
            return;
        }
    }

    pub fn log(&self, string: impl Into<String>) {
        log!("{}", string.into());
    }

    pub fn debug(&self, string: impl Into<String>) {
        if debug() {
            log!("{}", string.into());
        }
    }

    pub fn init() -> Result<Self, Box<dyn Error>> {
        let (tx, rx) = channel();
        input::init(tx.clone());
        Ok(Self { tx, rx })
    }

    pub fn run(args: Args) -> Result<(), Box<dyn Error>> {
        DEBUG.store(args.debug, Relaxed);

        let app = match Self::init() {
            Ok(a) => a,
            Err(e) => {
                log!("startup error: {e}");
                return Err(e);
            }
        };

        if args.input_test {
            if debug() {
                println!("debug active");
            }
            TTY.store(true, Relaxed);
            log!("starting input test");
            return test::input_events(app);
        }

        Ok(())
    }
}

// defines app log event used mpsc::channel
pub trait SenderHelpers {
    fn log(&self, msg: impl Into<String>);
    fn debug(&self, msg: impl Into<String>);
}
impl SenderHelpers for Sender<Event> {
    fn log(&self, msg: impl Into<String>) {
        let _ = self.send(Event::Log(msg.into()));
    }
    fn debug(&self, msg: impl Into<String>) {
        let _ = self.send(Event::Debug(msg.into()));
    }
}
