use nephrite4_common::conf;
use nephrite4_query::index::tika;
use nephrite4_query::error::*;

use std::{env, process::{Child, Command, Stdio}, thread, time};
use log::info;

fn cat(p: &str) -> Result<Child> {
    let cat = Command::new("cat")
        .arg(p)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to start bup process");

    Ok(cat)
}

fn main() -> Result<()> {
    env_logger::init();

    let conf = conf::Conf::read();

    let tika = tika::Tika::new(&conf)?;

    for arg in env::args() {
        info!("parse: {}", &arg);

        let res = tika.parse_file(&arg)?;
        info!("result: {}", &res);

        let mut ch = cat(&arg)?;

        {
            let stdout = ch.stdout.take().unwrap();
            let res1 = tika::tika_res(&tika.parse_from_fd(stdout)?)?;
            info!("result1: {:?}", &res1);
        }

        ch.wait()?;
    }

    thread::sleep(time::Duration::from_secs(1));

    Ok(())
}
