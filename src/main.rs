#![allow(unused_imports)]
#![allow(unused)]

mod app;
mod canvas;
mod input;

use clap::Parser;
use std::error::Error;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;

#[derive(Parser, Debug)]
#[command(version)]
struct Args {
    /// Specifies a config file to boot with
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Starts debugging session
    #[arg(short, long)]
    debug: bool,
}

// fn input_test(events: Receiver<Event>) {
//     for event in events {
//         match event {
//             Event::Key { code, state } => {
//                 if code == 1 {
//                     break;
//                 }
//                 let status = if state { "down" } else { "up" };
//                 log!("key: {code} {status}");
//             }
//             Event::Mouse { x, y } => {
//                 log!("mouse changed: {x}, {y}");
//             }
//             Event::Scroll { x, y } => {
//                 log!("scrolled: {x}, {y}");
//             }
//             Event::Unknown => {}
//         }
//     }
// }

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    // if args.input {
    //     println!("starting input test");
    //     input_test(events);
    //     return Ok(());
    // }

    app::App::init(true)
    // app::App::init(args.debug)
}
