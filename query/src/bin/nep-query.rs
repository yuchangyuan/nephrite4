// nephrite-query

use clap::{Arg, App, AppSettings};

use nephrite4_common::conf;
use nephrite4_common::util;

use util::Id;

//use serde_json::Value;
//use serde_json::map::Map;

use nephrite4_query::db;
use db::search;

fn main() {
    env_logger::init();

    let matches =
        App::new("query")
        .arg(Arg::with_name("num")
             .short("n")
             .long("num")
             .takes_value(true)
             .help("limit number of record, default 10"))
        .arg(Arg::with_name("all")
             .long("all")
             .help("search for all record, include obsolete"))
        .arg(Arg::with_name("append")
             .long("append")
             .help("append to exist result"))
        .arg(Arg::with_name("clear")
             .long("clear")
             .help("clear previous result"))
        .arg(Arg::with_name("save")
             .short("s")
             .long("save")
             .help("save search result to table")
             .takes_value(true))
        .setting(AppSettings::TrailingVarArg)
        .arg(Arg::from_usage("<patterns>... 'search patterns'"))
        .get_matches();

    let num = matches.value_of("num").unwrap_or("10").parse().unwrap_or(10);
    let all = matches.is_present("all");
    let qs: Vec<_> = matches.values_of("patterns").unwrap().collect();

    //println!("num {}, all {}, patt {:?}", num, all, qs);

    // db init
    let conf = conf::Conf::read();

    //let store = store::Store::new(&conf);

    // ensure
    let mut client = db::client(&conf).unwrap();

    let patt = qs.iter().map(|e| search::to_search(e)).collect::<Vec<_>>();

    let res = search::search(&mut client, &patt, all, num).unwrap();

    let mut fids: Vec<Id> = vec![];

    for (id, fid) in res.into_iter() {
        let attr = search::get_attr(&mut client, &id).unwrap();

        print!("{} {} ",
                 &util::to_zbase32(&fid)[..7],
               &util::to_zbase32(&id)[..7]);

        for m in attr.into_iter() {
            println!("- {}", serde_json::to_string(&m).unwrap());
        }

        fids.push(fid);
    }

    let mut clear = true;
    let mut append = false;
    let mut sel_name = "__last".to_string();

    //
    match matches.value_of("save") {
        Some(sel) => {
            clear = matches.is_present("clear");
            append = matches.is_present("append");
            sel_name = sel.to_string();
        },
        _ => ()
    };

    search::sel_save(&mut client, &sel_name, &fids, clear, append).unwrap();

    println!("\n{} record {} to '{}'",
             fids.len(),
             if append { "appended" } else { "saved" },
             &sel_name);

}
