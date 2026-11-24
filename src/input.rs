use crate::imports::*;

fn handle_event(event: InputEvent) -> Event {
    let delta = event.value(); // also serves as key state
    let code = event.code();

    match event.event_type() {
        EventType::KEY => {
            let act = delta; // "action"
            // 1 = down, 2 = repeating, 0 = up
            Event::Key {
                code: code,
                state: act == 1 || act == 2,
            }
        }
        EventType::RELATIVE => {
            match code {
                0 => Event::Mouse { x: delta, y: 0 }, // mouse horizontal
                1 => Event::Mouse { x: 0, y: delta }, // mouse vertical
                6 => Event::Scroll { x: delta, y: 0 }, // horizonal scroll
                8 => Event::Scroll { x: 0, y: delta }, // vertical scroll
                _ => Event::Unknown,
            }
        }
        EventType::ABSOLUTE => match AbsoluteAxisCode(code) {
            AbsoluteAxisCode::ABS_X => Event::Mouse { x: delta, y: 0 },
            AbsoluteAxisCode::ABS_Y => Event::Mouse { x: 0, y: delta },
            _ => Event::Unknown,
        },
        _ => Event::Unknown,
    }
}

// converts libseat device to evdev device
fn convert(path: SeatDevice, tx: Sender<Event>) -> Option<Device> {
    match dup(&path.as_fd()) {
        Ok(file) => match evdev::Device::from_fd(file) {
            Ok(mut device) => Some(device),
            Err(e) => {
                log!(tx, "failed to create device from fd: {e}");
                None
            }
        },
        Err(e) => {
            log!("failed to dup fd: {e}");
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

                    log!(
                        tx,
                        "connected to input device: {}",
                        device.name().unwrap_or("Unknown")
                    );

                    match poll.registry().register(
                        &mut source,
                        token,
                        Interest::READABLE,
                    ) {
                        Ok(()) => {
                            log!(tx, "poll registered device: {device}",);
                            devices.push(device);
                        }
                        Err(e) => {
                            log!(tx, "poll failed to register device {e}");
                            continue;
                        }
                    }
                }
            }
            Err(e) => {
                log!(tx, "failed to create device from file: {e}")
            }
        }
    }
    devices
}

fn poll_register(tx: Sender<Event>, devices: &Vec<Device>, poll: &Poll) {
    for (i, device) in devices.iter().enumerate() {
        let token = Token(i); // token maps to devices index
        let mut source = SourceFd(&device.as_raw_fd());

        match poll
            .registry()
            .register(&mut source, token, Interest::READABLE)
        {
            Ok(()) => {
                log!(
                    tx,
                    "input: connected to device: {}",
                    device.name().unwrap_or("Unknown")
                );
            }
            Err(e) => {
                log!(
                    tx,
                    "input: failed to connect to device: {}",
                    device.name().unwrap_or("Unknown")
                );
            }
        }
    }
}

fn init_poll(tx: Sender<Event>, mut devices: Vec<evdev::Device>) {
    let mut poll = match Poll::new() {
        Ok(p) => p,
        Err(e) => {
            log!(tx, "failed to create poll: {e}");
            return;
        }
    };

    let mut events = Events::with_capacity(128);
    poll_register(tx.clone(), &devices, &poll);

    loop {
        if let Err(e) = poll.poll(&mut events, None) {
            log!(tx, "poll error: {}", e);
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
                    Err(_) => return,
                }
            }
        }
    }
}

pub fn catch(
    tx: Sender<Event>,
    mut devices: Vec<Device>,
) -> Result<(), Box<dyn Error>> {
    spawn(move || {
        if tty() {
            force_tty();
        }

        init_poll(tx.clone(), devices);
    });

    Ok(())
}
