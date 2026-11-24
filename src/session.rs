use crate::imports::*;

struct Session {
    tx: Sender<Event>,
    seat: Seat,
    pending: Rc<Cell<Option<SeatEvent>>>,
    held: Vec<SeatDevice>,
}

pub fn init(app: App) -> Result<App, Box<dyn Error>> {
    let tx = app.tx.clone(); // obligatory tx clone bc of thread borrow
    spawn(move || {
        let session = match Session::open(tx.clone()) {
            Ok(mut s) => s.handle(),
            Err(e) => Ok(log!(tx, "session: failed to open seat")),
        };

        if let Err(e) = session {
            log!(tx, "session: {e}");
        }
    });
    return Ok(app);
}

impl Session {
    fn open(tx: Sender<Event>) -> Result<Self, Box<dyn Error>> {
        let pending = Rc::new(Cell::new(None));
        let p_clone = pending.clone(); // have to clone just like tx

        let seat = Seat::open(move |_, event| {
            p_clone.set(Some(event));
            if tty() {
                force_tty();
            }
        });

        let held = Vec::<SeatDevice>::new();

        Ok(Self {
            tx,
            seat: seat?,
            pending,
            held,
        })
    }

    fn handle(&mut self) -> Result<(), Box<dyn Error>> {
        loop {
            self.seat.dispatch(-1);
            match self.pending.take() {
                Some(SeatEvent::Enable) => self.enable(),
                Some(SeatEvent::Disable) => self.disable(),
                None => Ok(()),
            };
        }
    }

    fn enable(&mut self) -> Result<(), Box<dyn Error>> {
        if tty() {
            force_tty();
        }
        log!(self.tx, "seat session enabled");

        let inputs = self.open_inputs();

        match self.open_cards() {
            Some(cards) => {
                self.tx.send(Event::Start { inputs, cards });
            }
            None => {
                log!(self.tx, "no DRI cards found");
                self.tx.send(Event::Stop)?
            }
        }

        Ok(())
    }

    fn disable(&mut self) -> Result<(), Box<dyn Error>> {
        self.tx.send(Event::Stop);
        for device in self.held.drain(..) {
            self.seat.close_device(device);
        }
        self.seat.disable()?;
        Ok(())
    }

    fn open_dup(&mut self, path: &Path) -> Result<OwnedFd, Box<dyn Error>> {
        let device = self.seat.open_device(&path)?;
        let fd = dup(device.as_fd())?;
        self.held.push(device);
        Ok(fd)
    }

    fn open_inputs(&mut self) -> Vec<Device> {
        let mut devices = Vec::<Device>::new();

        for (path, _) in enumerate() {
            match self.open_dup(&path) {
                Ok(fd) => match Device::from_fd(fd) {
                    Ok(device) => devices.push(device),
                    Err(e) => log!(self.tx, "evdev {path:?}: {e}"),
                },
                Err(e) => {
                    log!(self.tx, "couldn't open {path:?}: {e}");
                }
            }
        }

        devices
    }

    fn open_cards(&mut self) -> Option<Vec<OwnedFd>> {
        let mut cards = Vec::<OwnedFd>::new();

        for dri in read_dir("/dev/dri").ok()?.flatten() {
            let path = dri.path();

            if path
                .file_name()
                .and_then(|p| p.to_str())
                .is_some_and(|p| p.starts_with("card"))
            {
                match self.open_dup(&path) {
                    Ok(fd) => cards.push(fd),
                    Err(e) => log!(self.tx, "couldn't open {path:?}: {e}"),
                }
            }
        }

        if !cards.is_empty() { Some(cards) } else { None }
    }
}
