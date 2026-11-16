use evdev::*;
use libseat::{Seat, SeatEvent, SeatRef};
use rustix::fd::AsFd;
use rustix::io::*;
use std::error::Error;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::sync::mpsc::*;
use std::thread;
use std::time::{Duration, Instant};

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

fn open_device(tx: Sender<Event>, seat: &mut SeatRef, path: PathBuf) {
    let file = seat.open_device(&path).expect("failed to open device");
    let new = dup(&file.as_fd()).expect("failed to dup fd");
    fcntl_setfd(&new, FdFlags::empty()).expect("failed to set blocking mode");

    thread::spawn(move || {
        let mut device = Device::from_fd(new).expect("failed to create device");
        println!(
            "connected to input device: {}",
            device.name().unwrap_or("Unknown")
        );

        loop {
            match device.fetch_events() {
                Ok(events) => {
                    for event in events {
                        println!(
                            "Event received -> Type: {:?}, Code: {}, Value: {}",
                            event.event_type(),
                            event.code(),
                            event.value()
                        );
                        match event.event_type() {
                            EventType::KEY => {
                                let act = event.value(); // "action"
                                // 1 = down
                                if act == 1 || act == 0 {
                                    let packet = Event::Key {
                                        code: event.code(),
                                        state: act == 1,
                                    };

                                    if tx.send(packet).is_err() {
                                        break;
                                    }
                                }
                            }
                            EventType::ABSOLUTE => {
                                let packet = match event.code() {
                                    0 => {
                                        // mouse horizontal
                                        Event::Mouse {
                                            x: event.value(),
                                            y: 0,
                                        }
                                    }
                                    1 => {
                                        // mouse vertical
                                        Event::Mouse {
                                            x: 0,
                                            y: event.value(),
                                        }
                                    }
                                    6 => {
                                        // horizonal scroll
                                        Event::Scroll {
                                            x: event.value(),
                                            y: 0,
                                        }
                                    }
                                    8 => {
                                        // vertical scroll
                                        Event::Scroll {
                                            x: 0,
                                            y: event.value(),
                                        }
                                    }
                                    _ => Event::Unknown,
                                };

                                if tx.send(packet).is_err() {
                                    break;
                                }
                            }
                            _ => (),
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
            match event {
                SeatEvent::Enable => {
                    println!("libseat session active");

                    for (path, name) in enumerate() {
                        // gets /dev/input devices
                        open_device(tx.clone(), seat, path);
                    }
                }
                SeatEvent::Disable => {
                    println!("libseat session paused");
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
                eprintln!("libseat dispatch error: {}", e);
                break;
            }
        }
    });

    return Ok(rx);
}
