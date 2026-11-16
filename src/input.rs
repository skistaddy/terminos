use evdev::*;
use libseat::{Seat, SeatEvent, SeatRef};
use rustix::fd::AsFd;
use rustix::io::*;
use std::error::Error;
use std::fs::File;
use std::path::PathBuf;
use std::sync::mpsc::*;
use std::thread;
use std::time::{Duration, Instant};

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        #[cfg(debug_assertions)]
        {
            use std::io::Write;
            let msg = format!($($arg)*);
            let formatted = msg.replace("\n", "\r\n");
            let mut stderr = std::io::stderr();
            let _ = writeln!(stderr, "{}", formatted);
            let _ = stderr.flush(); // Force immediate write to the raw terminal stream
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
    Unknown,
}

fn handle_event(event: InputEvent) -> Event {
    let delta = event.value(); // also serves as key state
    let code = event.code();

    return match event.event_type() {
        EventType::KEY => {
            let act = delta; // "action"
            // 1 = down, 2 == repeating
            return Event::Key {
                code: code,
                state: act == 1 || act == 2,
            };
        }
        EventType::RELATIVE => {
            return match code {
                0 => Event::Mouse { x: delta, y: 0 },  // mouse horizontal
                1 => Event::Mouse { x: 0, y: delta },  // mouse vertical
                6 => Event::Scroll { x: delta, y: 0 }, // horizonal scroll
                8 => Event::Scroll { x: 0, y: delta }, // vertical scroll
                _ => Event::Unknown,
            };
        }
        EventType::ABSOLUTE => {
            return match AbsoluteAxisCode(code) {
                AbsoluteAxisCode::ABS_X => Event::Mouse { x: delta, y: 0 },
                AbsoluteAxisCode::ABS_Y => Event::Mouse { x: 0, y: delta },
                _ => Event::Unknown,
            };
        }
        _ => Event::Unknown,
    };
}

fn open_device(tx: Sender<Event>, seat: &mut SeatRef, path: PathBuf) {
    let file = seat.open_device(&path).expect("failed to open device");
    let new = dup(&file.as_fd()).expect("failed to dup fd");
    fcntl_setfd(&new, FdFlags::empty()).expect("failed to set blocking mode");

    thread::spawn(move || {
        let mut device = Device::from_fd(new).expect("failed to create device");
        log!(
            "connected to input device: {}",
            device.name().unwrap_or("Unknown")
        );

        loop {
            match device.fetch_events() {
                Ok(events) => {
                    for event in events {
                        log!(
                            "Event received -> Type: {:?}, Code: {}, Value: {}",
                            event.event_type(),
                            event.code(),
                            event.value()
                        );
                        let packet = handle_event(event);
                        if tx.send(packet).is_err() {
                            break;
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => {
                    break;
                }
            }
        }
    });
}

pub fn init() -> Result<Receiver<Event>> {
    let (tx, rx) = channel();

    thread::spawn(move || {
        let mut seat = match Seat::open(move |seat, event| {
            println!("hi");

            match event {
                SeatEvent::Enable => {
                    log!("libseat session active");

                    for (path, _) in enumerate() {
                        // gets /dev/input devices
                        open_device(tx.clone(), seat, path);
                    }
                }
                SeatEvent::Disable => {
                    log!("libseat session paused");
                    seat.disable().ok();
                }
            }
        }) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("failed to open libseat: {}", e);
                return;
            }
        };

        loop {
            if let Err(e) = seat.dispatch(-1) {
                log!("libseat dispatch error: {}", e);
                break;
            }
        }
    });

    return Ok(rx);
}
