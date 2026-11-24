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
            let msg = format!($($arg)*);
            let _ = ($sender).send(Event::Log(msg));
        }
    }};
    ($($arg:tt)*) => {{
        #[cfg(debug_assertions)]
        if debug() {
            println!($($arg)*);
        }
    }};
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
    pub fn init() -> Result<App, Box<dyn Error>> {
        let (tx, rx) = channel();
        crate::session::init(Self { tx, rx })
    }

    pub fn run(args: Args) -> Result<(), Box<dyn Error>> {
        DEBUG.store(args.debug, Relaxed);
        TTY.store(args.text, Relaxed);

        let mut msgs = vec![];

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
                        let pixel: [u8; 4] = [0x80, 0x30, 0x1e, 0x00];

                        let layer = Layer {
                            id: 1,
                            dst: Rect {
                                x: 0,
                                y: 0,
                                w: canvas.outputs[0].width,
                                h: canvas.outputs[0].height,
                            },
                            scroll: (0, 0),
                            z: -1,
                            visible: true,
                            frame: Frame {
                                data: FrameData {},
                                damage: Damage::None,
                            },
                        };
                        canvas.paint(0, &[layer]);
                    }
                    Ok(Event::Stop) => {
                        if debug() {
                            println!("{}", msgs.join("\n").to_string());
                        }
                        canvas.release();
                        return Ok(());
                    }
                    Ok(Event::Key { code: 1, .. }) => {
                        app.tx.send(Event::Stop);
                    }
                    Ok(Event::Debug(m)) => log!("{m:?}"),
                    Ok(Event::Log(m)) => {
                        msgs.push(m.clone());
                        log!("{m:?}");
                    }
                    Ok(Event::Flip(op_id)) => {
                        canvas.outputs[op_id].flipped();
                        log!("flipped");
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
