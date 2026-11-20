use crate::app::*;
use drm::buffer::DrmFourcc;
use drm::control::{Device as ControlDevice, connector, dumbbuffer::DumbBuffer, framebuffer};
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

struct Screen {
    card: Card,
    db: DumbBuffer,
    fb: framebuffer::Handle,
    pub width: u32,
    pub height: u32,
}

pub struct Canvas {
    screens: Vec<Screen>,
    state: bool,
}

pub fn connect(tx: Sender<Event>, fds: Vec<OwnedFd>) -> Vec<Screen> {
    let cards = fds.into_iter().map(|f| Card(f));
    let mut screens = Vec::<Screen>::new();

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

                screens.push(Screen {
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

pub fn start(tx: Sender<Event>, fds: Vec<OwnedFd>) {
    connect(tx, fds);
}

impl Canvas {
    // pub fn fill(&mut self, rgb: u32) -> Result<(), Box<dyn Error>> {
    //     let mut map = self.card.map_dumb_buffer(&mut self.db)?;
    //     for px in map.as_mut().chunks_exact_mut(4) {
    //         px.copy_from_slice(&rgb.to_le_bytes()); // 0x00RRGGBB
    //     }
    //     Ok(())
    // }
}
