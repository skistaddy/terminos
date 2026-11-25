use crate::imports::*;

struct Buffer {
    db: DumbBuffer,
    fb: framebuffer::Handle,
}

pub struct MappedBuffer<'map> {
    mapping: drm::control::dumbbuffer::DumbMapping<'map>,
    pub pitch: u32,
    pub w: u32,
    pub h: u32,
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
    pub w: u32,
    pub h: u32,
}

impl Output {
    pub fn flipped(&mut self) {
        self.flipping = false;
    }
}

pub struct Canvas<'a> {
    pub cards: Vec<Card>,
    pub layers: Vec<Layer<'a>>,
    crtcs: Vec<crtcHandle>,
    pub outputs: Vec<Output>,
    pub tx: Sender<Event>,
}

impl<'a> Canvas<'a> {
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

    pub fn new(tx: Sender<Event>) -> Canvas<'a> {
        Self {
            cards: Vec::new(),
            layers: Vec::new(),
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
                w: width,
                h: height,
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
        let w = output.w;
        let h = output.h;
        let next = 1 - output.buffer; // 1 - 1 = 0, 1 - 0 = 1
        let buf = &mut output.buffers[next];
        let pitch = buf.db.pitch();

        let card = &mut self.cards[output.card_id];
        let mapping = card.map_dumb_buffer(&mut buf.db)?;

        Ok(MappedBuffer {
            mapping,
            pitch,
            w,
            h,
        })
    }

    pub fn paint(
        &mut self,
        op_id: usize,
        layers: &[Layer],
    ) -> Result<(), Box<dyn Error>> {
        let next = {
            let output = &self.outputs[op_id];
            1 - output.buffer
        };

        let pixels = {
            let pitch = self.outputs[op_id].buffers[next].db.pitch() as usize;
            let width = self.outputs[op_id].w as usize;
            let height = self.outputs[op_id].h as usize;
            let mut pixels = vec![0u8; pitch * height];

            println!("{layers:?}");
            let mut layers: Vec<&Layer<'_>> =
                layers.iter().filter(|l| l.visible).collect();
            layers.sort_by_key(|l| l.z);

            for layer in layers {
                let Frame::Dumb {
                    w,
                    h,
                    pitch: frame_pitch,
                    format,
                    pixels: frame_pixels,
                    damage,
                } = layer.frame
                else {
                    continue;
                };

                let rects = match damage {
                    Damage::None => continue,
                    Damage::Full => &[Rect {
                        x: 0,
                        y: 0,
                        w: w as i32,
                        h: h as i32,
                    }],
                    Damage::Rects(rects) => rects,
                };

                for rect in rects {
                    // max = absolute value
                    let x0 = rect.x.max(0) as usize;
                    let y0 = rect.y.max(0) as usize;
                    // min = ceiling
                    let x1 = ((rect.x + rect.w).max(0) as usize).min(width);
                    let y1 = ((rect.y + rect.h).max(0) as usize).min(height);

                    if x0 >= x1 || y0 >= y1 {
                        continue;
                    }
                    let size = (x1 - x0) * 4; // rect width * pixel size
                    let fp = frame_pitch as usize; // frame pitch
                    let col = x0 * 4;

                    for y in y0..y1 {
                        // y index + starting x * pixel size
                        let d_start = y * fp + col; // damage start
                        let d_end = d_start + size; // damage end
                        let row = &frame_pixels[d_start..d_end];

                        let b_start = y * pitch + col;
                        let b_end = b_start + size;
                        pixels[b_start..b_end].copy_from_slice(row);
                    }
                }
            }
            pixels
        };

        self.map(op_id)?.mapping.as_mut().copy_from_slice(&pixels);
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
