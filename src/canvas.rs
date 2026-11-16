use libseat::{Device, Seat, SeatEvent};
use std::fs;

pub trait Canvas {}

pub fn init() {
    let mut drm_devices: Vec<Device> = Vec::new();

    let mut seat = Seat::open(move |seat, event| match event {
        SeatEvent::Enable => {
            if let Ok(cards) = fs::read_dir("/dev/dri") {
                for card in cards.flatten() {
                    let path = card.path();
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        // Match card0, card1, etc., ignoring renderD* and controlD*
                        if name.starts_with("card") {
                            match seat.open_device(&path) {
                                Ok(device) => {
                                    // `device` implements AsFd / AsRawFd — pass to your DRM crate
                                    drm_devices.push(device);
                                }
                                Err(e) => eprintln!("Failed to open {:?}: {:?}", path, e),
                            }
                        }
                    }
                }
            }
        }
        SeatEvent::Disable => {
            // Drop devices or pause rendering before acknowledging disable
            drm_devices.clear();
            seat.disable().ok();
        }
        _ => {}
    })
    .expect("failed to open libseat session");

    // Dispatch once (or in your main loop) so SeatEvent::Enable actually executes
    seat.dispatch(0).expect("failed to dispatch seat events");
}
