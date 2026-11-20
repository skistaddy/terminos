use crate::app::*;
use drm::buffer::{Buffer, DrmFourcc, Handle};
use drm::control::{Device, connector, dumbbuffer::DumbBuffer, framebuffer};
use rustix::fd::{AsFd, BorrowedFd, OwnedFd};
use std::error::Error;
use std::sync::mpsc::{Receiver, Sender};

struct Card(OwnedFd);
impl AsFd for Card {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}
impl drm::Device for Card {}
impl drm::control::Device for Card {}

// comments represent the intended shape once connections are fully
// iterated over. connection ~= monitor, crtc ~= output driver kinda
struct Output {
    //connector: connector::Handle
    //crtc:: crtc::Handle
    card: Card,
    db: DumbBuffer,
    fb: framebuffer::Handle,
    pub width: u32,
    pub height: u32,
}

pub struct Canvas {
    //cards: Vec<Card>
    outputs: Vec<Output>,
}

pub fn connect(tx: Sender<Event>, fds: Vec<OwnedFd>) -> Vec<Output> {
    let cards = fds.into_iter().map(|f| Card(f));
    let mut screens = Vec::<Output>::new();

    for card in cards {
        let res = card.resource_handles().expect("no info");

        let connection = res
            .clone()
            .connectors()
            .iter()
            .filter_map(|&c| card.get_connector(c, true).ok())
            .find(|c| c.state() == connector::State::Connected);

        match connection {
            Some(c) => {
                let handle = c.handle();
                let mode = c.modes().first().expect("cannot access mode");
                let crtc = res.crtcs().first().expect("crtc");
                let (w, h) = mode.size();
                let (width, height) = (w as u32, h as u32);

                let db = card
                    .create_dumb_buffer((width, height), DrmFourcc::Xrgb8888, 32)
                    .expect("create dumbbuffer");
                let fb = card
                    .add_framebuffer(&db, 24, 32)
                    .expect("create framebuffer");

                card.set_crtc(*crtc, Some(fb), (0, 0), &[handle], Some(*mode));

                screens.push(Output {
                    card,
                    db,
                    fb,
                    width,
                    height,
                })
            }

            None => tx.debug("card not connected"),
        }
    }
    screens
}

impl Canvas {
    pub fn new() -> Canvas {
        return Self {
            outputs: Vec::<Output>::new(),
        };
    }

    pub fn start(tx: Sender<Event>, fds: Vec<OwnedFd>) -> Canvas {
        let outputs = connect(tx, fds);
        return Self { outputs };
    }

    pub fn release(&self) {
        for output in self.outputs.iter() {
            let handle = output.db.handle();
            output.card.close_buffer(handle);
        }
    }
}
