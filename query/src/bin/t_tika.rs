use nephrite4_common::conf;
use nephrite4_query::index::tika;
use nephrite4_query::error::*;

use std::env;
use log::{info};

fn main() -> Result<()> {
    env_logger::init();

    let conf = conf::Conf::read();

    let tika = tika::Tika::new(&conf)?;

    for arg in env::args() {
        info!("parse: {}", &arg);

        let res = tika.parse(&arg)?;
        info!("result: {}", &res);
    }

    Ok(())
}
