use crate::args::Args;

use rustix::fd::{AsFd, OwnedFd};
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

use rustix::fs::{Mode, OFlags, open};
use rustix::ioctl::{IntegerSetter, ioctl};

pub fn force_tty() -> rustix::io::Result<()> {
    const KDSETMODE: rustix::ioctl::Opcode = 0x4B3A;
    const KD_TEXT: usize = 0;

    let tty = open("/dev/tty", OFlags::RDWR, Mode::empty())?;
    unsafe { ioctl(&tty, IntegerSetter::<KDSETMODE>::new_usize(KD_TEXT)) }
}

static DEBUG: AtomicBool = AtomicBool::new(false);
pub fn debug() -> bool {
    DEBUG.load(Relaxed)
}

static TTY: AtomicBool = AtomicBool::new(false);
pub fn tty() -> bool {
    TTY.load(Relaxed)
}

#[derive(Debug)]
pub enum Event {
    Start {
        cards: Vec<OwnedFd>,
        inputs: Vec<evdev::Device>,
    },
    Stop,
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

    pub fn init() -> Result<App, Box<dyn Error>> {
        let (tx, rx) = channel();
        return crate::session::init(Self { tx, rx });
    }

    pub fn run(args: Args) -> Result<(), Box<dyn Error>> {
        DEBUG.store(args.debug, Relaxed);
        TTY.store(args.input_test, Relaxed);

        let app = Self::init()?;
        let mut canvas = crate::canvas::Canvas::new();

        while let Ok(event) = app.rx.recv() {
            match event {
                Event::Start { cards, inputs } => {
                    crate::input::catch(app.tx.clone(), inputs);
                    canvas = crate::canvas::Canvas::start(app.tx.clone(), cards);
                }
                Event::Key { code: 1, .. } => {
                    canvas.release();
                    break;
                }
                Event::Debug(m) => app.debug(m),
                Event::Log(m) => app.log(m),
                e if tty() => app.log(format!("{e:?}")), // input test output
                _ => {}
            }
        }

        // if args.input_test {
        //     if debug() {
        //         println!("debug active");
        //     }
        //     TTY.store(true, Relaxed);
        //     log!("starting input test");
        //     return crate::test::input_events(app);
        // }
        //
        Ok(())
    }
}

// defines app log methods used by mpsc::channel
pub trait SenderMethods {
    fn log(&self, msg: impl Into<String>);
    fn debug(&self, msg: impl Into<String>);
}
impl SenderMethods for Sender<Event> {
    fn log(&self, msg: impl Into<String>) {
        let _ = self.send(Event::Log(msg.into()));
    }
    fn debug(&self, msg: impl Into<String>) {
        let _ = self.send(Event::Debug(msg.into()));
    }
}
