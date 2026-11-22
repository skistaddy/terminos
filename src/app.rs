use crate::imports::*;

pub fn force_tty() -> rustix::io::Result<()> {
    const KDSETMODE: rustix::ioctl::Opcode = 0x4B3A;
    const KD_TEXT: usize = 0;

    let tty = open("/dev/tty", OFlags::RDWR, fsMode::empty())?;
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

#[macro_export]
macro_rules! log {
    ($sender:expr, $($arg:tt)*) => {{
        #[cfg(debug_assertions)]
        {
            let _ = ($sender).send(Event::Log(format!($($arg)*)));
        }
    }};
    ($($arg:tt)*) => {{
        #[cfg(debug_assertions)]
        if debug() {
            println!($($arg)*);
        }
    }};
}

pub struct Layer<'a> {
    pub id: u64,
    pub dst: Rect,
    pub src_offset: (u32, u32),
    pub z_order: i32,
    pub visible: bool,
    pub frame: Frame<'a>,
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
    Flip(usize),
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

    pub fn init() -> Result<App, Box<dyn Error>> {
        let (tx, rx) = channel();
        crate::session::init(Self { tx, rx })
    }

    pub fn run(args: Args) -> Result<(), Box<dyn Error>> {
        DEBUG.store(args.debug, Relaxed);
        TTY.store(args.input_test, Relaxed);

        let app = Self::init()?;
        let mut canvas = crate::canvas::Canvas::new(app.tx.clone());
        let wait = std::time::Duration::from_millis(16);

        loop {
            let start = std::time::Instant::now();

            loop {
                match app.rx.try_recv() {
                    Ok(Event::Start { cards, inputs }) => {
                        crate::input::catch(app.tx.clone(), inputs);
                        canvas.start(cards);
                        canvas.paint(0);
                    }
                    Ok(Event::Key { code: 1, .. }) => {
                        canvas.release();
                        return Ok(());
                    }
                    Ok(Event::Debug(m)) => log!("{m:?}"),
                    Ok(Event::Log(m)) => log!("{m:?}"),
                    Ok(Event::Flip(op_id)) => {
                        canvas.outputs[op_id].flipped();
                    }
                    Ok(e) if tty() => log!("{e:?}"),
                    Ok(_) => {}
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        return Ok(());
                    }
                }
            }

            let remaining = wait.saturating_sub(start.elapsed());
            if !remaining.is_zero() {
                std::thread::sleep(remaining);
            }
        }
    }
}
