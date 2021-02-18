// nephrite-add

use dotenv::dotenv;

use nephrite4_common::proj;
use nephrite4_common::util;

use proj::manifest;

use std::io::prelude::*;
use std::io;

fn main() {
    dotenv().ok();

    env_logger::init();

    let files = std::env::args_os().skip(1).
        map(|s| s.into_string().unwrap()).collect::<Vec<_>>();

    // return if no files
    if files.len() == 0 { return }

    // ensure
    let mut manifest = manifest::Manifest::new(proj::MANIFEST).unwrap();

    println!("manifest {} object loaded.", manifest.anno_map.len());

    for f in files {
        print!("adding {} ... ", f);
        io::stdout().flush().ok();

        match manifest.add(&f.to_string()) {
            Ok(_) => println!("ok"),
            Err(e) => println!("error, {}", e)
        }
    }

    println!("manifest {} object total", manifest.anno_map.len());
}
