#![allow(unused_imports)]
#![allow(unused)]

mod app;
mod args;
mod canvas;
mod frame;
mod imports;
mod input;
mod session;

use crate::imports::*;

fn main() -> Result<(), Box<dyn Error>> {
    crate::app::App::run(Args::parse())
}
