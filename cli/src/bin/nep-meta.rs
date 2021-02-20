// nephrite-meta

use dotenv::dotenv;

use nephrite4_common::proj;
use proj::manifest;

use log::debug;

fn main() {
    dotenv().ok();

    env_logger::init();

    // ensure
    let mut manifest = manifest::Manifest::new(proj::MANIFEST).unwrap();

    println!("manifest {} object loaded.", manifest.anno_map.len());

    // get id
    let args = std::env::args_os().skip(1).
        map(|s| s.into_string().unwrap()).collect::<Vec<_>>();

    let mut ops: Vec<String> = vec![];
    let mut files: Vec<String> = vec![];

    let mut is_op = true;
    let mut allow_new = false;

    for a in args.into_iter() {
        if is_op {
            if a == "--new" {
                allow_new = true;
                continue;
            }

            if a == "--" {
                is_op = false;
                continue;
            }

            if a.contains(|c| c == '+' || c == '-' || c == '=') {
                ops.push(a);
            }
            else {
                files.push(a);
                is_op = false;
            }
        }
        else {
            files.push(a)
        }
    }

    debug!("ops -- {:?}, files -- {:?}", ops, files);

    for f in files.into_iter() {
        let anno_opt = manifest.anno_map.get_mut(&f);
        if anno_opt.is_none() { continue; }
        let anno = anno_opt.unwrap();

        let n = anno.proc_op(&ops, allow_new);
        anno.save().unwrap();

        println!("U {} -- {}", n, f);
    }
}
