use crate::app::*;
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

fn handle_event(event: InputEvent) -> Event {
    let delta = event.value(); // also serves as key state
    let code = event.code();

    return match event.event_type() {
        EventType::KEY => {
            let act = delta; // "action"
            // 1 = down, 2 = repeating, 0 = up
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
fn convert(path: libseat::Device, tx: Sender<Event>) -> Option<Device> {
    match dup(&path.as_fd()) {
        Ok(file) => match evdev::Device::from_fd(file) {
            Ok(mut device) => Some(device),
            Err(e) => {
                tx.debug(format!("failed to create device from fd: {}", e));
                None
            }
        },
        Err(e) => {
            tx.debug(format!("failed to dup fd: {}", e));
            None
        }
    }
}

fn register(poll: &Poll, seat: &mut SeatRef, tx: Sender<Event>) -> Vec<Device> {
    let mut devices = Vec::<Device>::new();

    for (path, info) in enumerate() {
        let name = info.name().unwrap_or("Unknown");

        match seat.open_device(&path) {
            Ok(file) => {
                if let Some(mut device) = convert(file, tx.clone()) {
                    let token = Token(devices.len()); // token maps to devices index
                    let raw = device.as_raw_fd();
                    let mut source = SourceFd(&raw);

                    tx.debug(format!(
                        "connected to input device: {}",
                        device.name().unwrap_or("Unknown")
                    ));

                    match poll
                        .registry()
                        .register(&mut source, token, Interest::READABLE)
                    {
                        Ok(()) => {
                            tx.debug(format!("poll registered device: {}", raw));
                            devices.push(device);
                        }
                        Err(e) => {
                            tx.debug(format!("poll failed to register device {}", e));
                            continue;
                        }
                    }
                }
            }
            Err(e) => tx.debug(format!("failed to create device from file: {}", e)),
        }
    }
    devices
}

use rustix::fs::{Mode, OFlags, open};
use rustix::ioctl::{IntegerSetter, ioctl};

fn force_text_mode() -> rustix::io::Result<()> {
    const KDSETMODE: rustix::ioctl::Opcode = 0x4B3A;
    const KD_TEXT: usize = 0;

    let tty = open("/dev/tty", OFlags::RDWR, Mode::empty())?;
    unsafe { ioctl(&tty, IntegerSetter::<KDSETMODE>::new_usize(KD_TEXT)) }
}

fn init_poll(tx: Sender<Event>, seat: &mut SeatRef) {
    if debug() || tty() {
        force_text_mode();
    }
    tx.debug("libseat session active");
    let mut poll = match Poll::new() {
        Ok(p) => p,
        Err(e) => {
            tx.debug(format!("failed to create poll: {}", e));
            return;
        }
    };

    let mut events = Events::with_capacity(128);
    let mut devices = register(&poll, seat, tx.clone());

    loop {
        if let Err(e) = poll.poll(&mut events, None) {
            tx.debug(format!("poll error: {}", e));
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

pub fn init(tx: Sender<Event>) -> Result<(), Box<dyn Error>> {
    thread::spawn(move || {
        let og_tx = tx.clone(); // tx is borrowed by the thread below

        let mut seat = match Seat::open(move |seat, event| {
            tx.debug("libseat opened");
            match event {
                SeatEvent::Enable => init_poll(tx.clone(), seat),
                SeatEvent::Disable => {
                    tx.debug("libseat session paused");
                    seat.disable().ok();
                }
            }
        }) {
            Ok(s) => s,
            Err(e) => {
                og_tx.debug(format!("failed to open libseat: {}", e));
                return;
            }
        };

        loop {
            if let Err(e) = seat.dispatch(-1) {
                og_tx.debug(format!("libseat dispatch error: {}", e));
                break;
            }
        }
    });

    Ok(())
}
