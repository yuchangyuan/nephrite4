pub mod types;
pub mod search;

use nephrite4_common::conf;

use crate::error::*;
use postgres::{Client, NoTls};

pub fn client(conf: &conf::Conf) -> Result<Client> {
    Ok(Client::connect(&conf.db_url(), NoTls)?)
}
