use crate::app::*;
use std::error::Error;

pub fn input_events(app: App) -> Result<(), Box<dyn Error>> {
    while let Ok(event) = app.rx.recv() {
        match event {
            Event::Key { code, state } => {
                if code == 1 {
                    break;
                }
                app.log(format!("key: {code}"));
            }
            Event::Mouse { x, y } => {
                app.log(format!("mouse: {x}, {y}"));
            }
            Event::Log(string) => {
                app.log(string);
            }
            Event::Debug(string) => {
                app.debug(string);
            }
            _ => {}
        }
    }

    Ok(())
}
