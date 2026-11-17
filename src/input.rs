use evdev::*;
use libseat::{Seat, SeatEvent, SeatRef};
use mio::unix::SourceFd;
use mio::{Events, Interest, Poll, Token};
use rustix::fd::{AsFd, OwnedFd};
use rustix::io::dup;
use std::collections::HashMap;
use std::error::Error;
use std::io::ErrorKind;
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use std::sync::mpsc::*;
use std::thread;

#[macro_export]
macro_rules! log {
    ($($arg:tt)*) => {
        if cfg!(debug_assertions) {
            use std::io::Write;
            let msg = format!($($arg)*);
            let formatted = msg.replace("\n", "\r\n");
            let mut stderr = std::io::stderr();
            let _ = writeln!(stderr, "{}", formatted);
            let _ = stderr.flush();
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

// converts libseat device to evdev device
fn convert(path: libseat::Device) -> Option<Device> {
    match dup(&path.as_fd()) {
        Ok(file) => match evdev::Device::from_fd(file) {
            Ok(mut device) => Some(device),
            Err(e) => {
                log!("failed to create device from fd: {}", e);
                None
            }
        },
        Err(e) => {
            log!("failed to dup fd: {}", e);
            None
        }
    }
}

fn register(poll: &Poll, seat: &mut SeatRef) -> Vec<Device> {
    let mut devices = Vec::<Device>::new();

    for (path, info) in enumerate() {
        let name = info.name().unwrap_or("Unknown");

        match seat.open_device(&path) {
            Ok(file) => {
                if let Some(mut device) = convert(file) {
                    let token = Token(devices.len()); // token maps to devices index
                    let raw = device.as_raw_fd();
                    let mut source = SourceFd(&raw);

                    log!(
                        "connected to input device: {}",
                        device.name().unwrap_or("Unknown")
                    );

                    match poll
                        .registry()
                        .register(&mut source, token, Interest::READABLE)
                    {
                        Ok(()) => {
                            log!("poll registered device: {}", raw);
                            devices.push(device);
                        }
                        Err(e) => {
                            log!("poll failed to register device {}", e);
                            continue;
                        }
                    }
                }
            }
            Err(e) => log!("failed to create device from file: {}", e),
        }
    }
    devices
}

fn init_poll(tx: Sender<Event>, seat: &mut SeatRef) {
    log!("libseat session active");
    let mut poll = match Poll::new() {
        Ok(p) => p,
        Err(e) => {
            log!("failed to create poll: {}", e);
            return;
        }
    };

    let mut events = Events::with_capacity(128);
    let mut devices = register(&poll, seat);

    loop {
        if let Err(e) = poll.poll(&mut events, None) {
            log!("poll errror: {}", e);
            break;
        }

        for event in events.iter() {
            let index = event.token().0;

            if let Some(device) = devices.get_mut(index) {
                match device.fetch_events() {
                    Ok(packets) => {
                        for p in packets {
                            if tx.send(handle_event(p)).is_err() {
                                return;
                            }
                        }
                    }
                    Err(e) if e.kind() == ErrorKind::WouldBlock => continue,
                    Err(_) => {
                        break;
                    }
                }
            }
        }
    }
}

pub fn init() -> Result<Receiver<Event>, Box<dyn Error>> {
    let (tx, rx) = channel();

    thread::spawn(move || {
        let mut seat = match Seat::open(move |seat, event| {
            log!("libseat opened");
            match event {
                SeatEvent::Enable => init_poll(tx.clone(), seat),
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
