// nephrite-index

use nephrite4_common::conf;

use nephrite4_query::index;
use nephrite4_common::store::Store;

use hex;

//use serde_json::Value;
//use serde_json::map::Map;

use nephrite4_query::error::*;

fn main() -> Result<()> {
    env_logger::init();

    let conf = conf::Conf::read();
    let indexer = index::Indexer::new(&conf)?;

    let inc_ref = indexer.store.show_ref(Store::INC_REF)?.unwrap();

    let list = indexer.walk(&inc_ref, &None)?;

    for (commit, tree) in list {
        println!("{}: {}", hex::encode(commit), hex::encode(tree));

        for (tp, name, id) in indexer.store.read_tree(&tree)? {
            println!("  {} {} {:?}", hex::encode(id), name, tp);
        }
    }

    Ok(())
}
