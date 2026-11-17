use crate::canvas;
use crate::input;
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
    Unknown,
}

pub struct App {
    pub debug: bool,
    tx: Sender<Event>,
    rx: Receiver<Event>,
}

static DEBUG: AtomicBool = AtomicBool::new(false);

pub fn debug() -> bool {
    DEBUG.load(Relaxed)
}

impl App {
    fn send(&self, event: Event) {
        if self.tx.send(event).is_err() {
            return;
        }
    }

    fn log(&self, string: impl Into<String>) {
        if self.debug {
            log!("{}", string.into());
        }
    }

    pub fn init(debug: bool) -> Result<(), Box<dyn Error>> {
        DEBUG.store(debug, Relaxed);
        let (tx, rx) = channel();
        let mut app = Self { debug, tx, rx };
        input::init(app.tx.clone())?;

        while let Ok(event) = app.rx.recv() {
            match event {
                Event::Key { code, state } => {
                    if code == 1 {
                        break;
                    }
                    app.log(format!("{}", code));
                }
                Event::Log(string) => {
                    app.log(string);
                }
                _ => {}
            }
        }

        Ok(())
    }
}

pub trait SenderLog {
    fn log(&self, msg: impl Into<String>);
}

impl SenderLog for Sender<Event> {
    fn log(&self, msg: impl Into<String>) {
        let _ = self.send(Event::Log(msg.into()));
    }
}
