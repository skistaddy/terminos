use crate::imports::*;

struct Buffer {
    db: DumbBuffer,
    fb: framebuffer::Handle,
}

pub struct MappedBuffer<'map> {
    mapping: drm::control::dumbbuffer::DumbMapping<'map>,
    pub pitch: u32,
    pub width: u32,
    pub height: u32,
}

pub struct Card(OwnedFd);
impl AsFd for Card {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}
impl DrmDevice for Card {}
impl ControlDevice for Card {}
impl Card {
    // buffer allocate
    fn balloc(
        &self,
        width: u32,
        height: u32,
    ) -> Result<Buffer, Box<dyn Error>> {
        let db =
            self.create_dumb_buffer((width, height), DrmFourcc::Xrgb8888, 32)?;
        let fb = self.add_framebuffer(&db, 24, 32)?;

        Ok(Buffer { db, fb })
    }
}

// comments represent the intended shape once connections are fully
// iterated over. connection ~= monitor, crtc ~= output driver kinda
pub struct Output {
    conn: drm::control::connector::Handle,
    crtc: drm::control::crtc::Handle,
    card_id: usize,
    buffers: Vec<Buffer>,
    buffer: usize,
    flipping: bool,
    pub width: u32,
    pub height: u32,
}

impl Output {
    pub fn flipped(&mut self) {
        self.flipping = false;
    }
}

pub struct Canvas {
    pub cards: Vec<Card>,
    crtcs: Vec<crtcHandle>,
    pub outputs: Vec<Output>,
    pub tx: Sender<Event>,
}

impl Canvas {
    pub fn flip(&mut self, op_id: usize) -> Result<(), Box<dyn Error>> {
        let (c_id, crtc, target, flipping) = {
            let output = &self.outputs[op_id];
            (
                output.card_id,
                output.crtc,
                1 - output.buffer,
                output.flipping,
            )
        };

        if flipping {
            return Ok(());
        }

        let fb = self.outputs[op_id].buffers[target].fb;
        self.cards[c_id].page_flip(crtc, fb, PageFlipFlags::EVENT, None)?;
        self.outputs[op_id].flipping = true;
        Ok(())
    }

    pub fn new(tx: Sender<Event>) -> Canvas {
        Self {
            cards: Vec::new(),
            crtcs: Vec::new(),
            outputs: Vec::new(),
            tx: tx,
        }
    }

    fn find_crtc(
        &mut self,
        info: &ResourceHandles,
    ) -> Result<crtcHandle, Box<dyn Error>> {
        let crtcs = info.crtcs();
        for c in crtcs.into_iter() {
            if !self.crtcs.contains(c) {
                self.crtcs.push(*c);
                return Ok(*c);
            }
        }

        log!(self.tx, "no CRTCs available");
        Err("no crtcs".into())
    }

    fn connect(
        &mut self,
        info: ResourceHandles,
        i: usize,
    ) -> Result<(), Box<dyn Error>> {
        let conns: Vec<Info> = info
            .connectors()
            .iter()
            .filter_map(|c| self.cards[i].get_connector(*c, true).ok())
            .collect();

        for conn in conns {
            let mode = match conn.modes().first() {
                Some(m) => {
                    log!(self.tx, "found mode for connection");
                    m
                }
                None => {
                    //self.tx.debug(format!("no display mode for {conn:?}"));
                    continue;
                }
            };
            let crtc = self.find_crtc(&info)?;
            let (w, h) = mode.size();
            let (width, height) = (w as u32, h as u32);
            let mut buffers = Vec::<Buffer>::new();

            match self.cards[i].balloc(width, height) {
                Ok(buffer) => buffers.push(buffer),
                Err(e) => {
                    log!(self.tx, "initial buffer failed on {conn:?}: {e}");
                    self.release();
                    continue;
                }
            }

            if !buffers.is_empty() {
                match self.cards[i].balloc(width, height) {
                    Ok(buffer) => buffers.push(buffer),
                    Err(e) => {
                        log!(
                            self.tx,
                            "secondary buffer failed on {conn:?}: {e}"
                        );
                        self.release();
                        continue;
                    }
                }
            }

            if let Err(err) = self.cards[i].set_crtc(
                crtc,
                Some(buffers[0].fb),
                (0, 0),
                &[conn.handle()],
                Some(*mode),
            ) {
                self.release();
                log!(self.tx, "failed to set crtc on {conn:?}: {err}");
                continue;
            }

            self.outputs.push(Output {
                conn: conn.handle(),
                crtc: crtc,
                card_id: i,
                buffers: buffers,
                buffer: 0,
                flipping: false,
                width: width,
                height: height,
            });
        }

        Ok(())
    }

    pub fn start(&mut self, fds: Vec<OwnedFd>) -> Result<(), Box<dyn Error>> {
        self.outputs.clear();
        self.cards = fds.into_iter().map(Card).collect();

        // iterate over vector length instead of vector to prevent borrow
        for i in 0..self.cards.len() {
            let info = self.cards[i].resource_handles()?;
            self.listen(i)?;
            self.connect(info, i);
        }

        Ok(())
    }

    pub fn map(
        &mut self,
        op_i: usize,
    ) -> Result<MappedBuffer<'_>, Box<dyn Error>> {
        let output = &mut self.outputs[op_i];
        let width = output.width;
        let height = output.height;
        let next = 1 - output.buffer; // 1 - 1 = 0, 1 - 0 = 1
        let buf = &mut output.buffers[next];
        let pitch = buf.db.pitch();

        let card = &mut self.cards[output.card_id];
        let mapping = card.map_dumb_buffer(&mut buf.db)?;

        Ok(MappedBuffer {
            mapping,
            pitch,
            width,
            height,
        })
    }

    pub fn paint(
        &mut self,
        op_id: usize,
        //_layers: &[Layer<'_>],
    ) -> Result<(), Box<dyn Error>> {
        let next = {
            let output = &self.outputs[op_id];
            1 - output.buffer
        };

        let pixel: [u8; 4] = [0x80, 0x30, 0x1e, 0x00];

        {
            let mut mapped = self.map(op_id)?;
            let pitch = mapped.pitch as usize;
            let width = mapped.width as usize;
            let height = mapped.height as usize;
            let pixels = mapped.mapping.as_mut();

            for y in 0..height {
                let start = y * pitch;
                for x in 0..width {
                    let scan = start + x * 4;
                    pixels[scan..scan + 4].copy_from_slice(&pixel);
                }
            }
        }

        let card_id = self.outputs[op_id].card_id;
        let card = &self.cards[card_id];
        self.flip(op_id);

        Ok(())
    }

    pub fn release(&mut self) {
        for op in self.outputs.drain(..) {
            if let Some(card) = self.cards.get(op.card_id) {
                for buff in op.buffers {
                    card.destroy_framebuffer(buff.fb);
                    card.close_buffer(buff.db.handle());
                }
            }
        }
        self.outputs.clear();
        self.cards.clear();
    }

    fn listen(&self, i: usize) -> Result<(), Box<dyn Error>> {
        let dupe = dup(self.cards[i].as_fd())?;
        let tx = self.tx.clone();

        spawn(move || {
            let card = Card(dupe);
            loop {
                let events = match card.receive_events() {
                    Ok(events) => events,
                    Err(_) => return,
                };

                for event in events {
                    if let DrmEvent::PageFlip { .. } = event
                        && tx.send(Event::Flip(i)).is_err()
                    {
                        return;
                    }
                }
            }
        });

        Ok(())
    }
}
