// nephrite-commit
use nephrite4_common::proj;
use nephrite4_common::conf;
use nephrite4_common::store;

use proj::manifest;
use store::*;
use conf::Conf;

use log::debug;

fn main() {
    env_logger::init();

    let conf = Conf::read();
    // store init
    let mut store = Store::new(&conf).unwrap();

    let mut manifest = manifest::Manifest::new(proj::MANIFEST).unwrap();

    println!("manifest {} object loaded.", manifest.anno_map.len());

    // do commit
    let commit = store.commit(&mut manifest).unwrap();

    debug!("commit -> {:?}", commit);
}
