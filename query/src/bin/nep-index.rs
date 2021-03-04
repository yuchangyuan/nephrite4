// nephrite-index

use nephrite4_common::conf;
use nephrite4_common::store::Store;

use hex;

//use serde_json::Value;
//use serde_json::map::Map;

use nephrite4_query::{error::*, index};

fn main() -> Result<()> {
    env_logger::init();

    let conf = conf::Conf::read();
    let mut indexer = index::Indexer::new(&conf)?;

    indexer.index_all()?;

    Ok(())
}
