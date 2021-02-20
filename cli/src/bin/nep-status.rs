// nephrite-status
use dotenv::dotenv;

use nephrite4_common::proj;
use nephrite4_common::util;

use proj::manifest;

//use std::io;

fn main() {
    dotenv().ok();

    env_logger::init();

    let manifest = manifest::Manifest::new(proj::MANIFEST).unwrap();

    println!("manifest {} object loaded.", manifest.anno_map.len());

    for (name, anno) in manifest.anno_map.iter() {
        let st_sym = match anno.status() {
            Ok(st) => proj::anno::st2chr(st),
            _ => "E".to_string()
        };

        let pid0 = if anno.pid.is_empty() {
            "       ".to_string()
        }
        else {
            util::to_zbase32(&anno.pid[0])
        };

        println!("{} {} -> {} : {:?}",
                 &pid0[..8],
                 &util::to_zbase32(&anno.ref_oid)[..8],
                 st_sym,
                 name);
    }
}
