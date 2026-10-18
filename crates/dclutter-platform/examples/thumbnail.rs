//  DClutter — swipe-based file triage
//  Copyright 2026 Dexter Jethro Enriquez
//  Licensed under the Apache License, Version 2.0.

//! Reports what preview DClutter gets for each file, and how long it took:
//!
//!     cargo run -p dclutter-platform --example thumbnail -- "C:\Users\me\Downloads\Report.pdf" [more files…]
//!
//! Add `--save <folder>` first to also write each preview there as a PNG.

use std::path::PathBuf;
use std::time::Instant;

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let save_to = match args.first().map(String::as_str) {
        Some("--save") => {
            let folder = PathBuf::from(args.get(1).expect("--save needs a folder"));
            args.drain(..2);
            Some(folder)
        }
        _ => None,
    };

    for arg in args {
        let path = PathBuf::from(&arg);
        let started = Instant::now();
        let preview = dclutter_platform::thumbnail(&path, 768);
        let took = started.elapsed().as_millis();
        let name = path.file_name().map_or(arg.clone(), |n| n.to_string_lossy().into_owned());
        match preview {
            Some(t) => {
                println!("{:?} {}x{} ({} KB) in {took} ms  {name}", t.kind, t.width, t.height, t.png.len() / 1024);
                if let Some(folder) = &save_to {
                    std::fs::write(folder.join(format!("{name}.png")), &t.png).expect("couldn't save the preview");
                }
            }
            None => println!("none in {took} ms  {name}"),
        }
    }
}
