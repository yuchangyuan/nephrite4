pub mod cut;
//pub mod tika;

use nephrite4_common::conf;
use nephrite4_common::proj;
use nephrite4_common::util;

use postgres::Client;

use serde_json;
use serde_cbor;

use conf::Conf;

use util::Id;
use proj::anno;

use std::io;

use std::collections::BTreeMap;
use std::iter::FromIterator;

use crate::error::*;

const ANNO_NAME: &'static str = "name";
const ANNO_NOTE: &'static str = "note";
const TIKA_CONTENT: &'static str = "X-TIKA:content";
const CONTENT_TYPE: &'static str = "Content-Type";

fn id2ref(id: &Id) -> &[u8] {
    &id[0..]
}

pub fn import_anno(conf: &Conf, client: &mut Client,
                   id: &Id, anno: &anno::Anno)
                   -> Result<()> {
    // insert/update anno
    let mut trans = client.transaction()?;

    let empty_id_vec: Vec<&[u8]> = vec![];

    let pids: Vec<&[u8]> = anno.pid.iter()
        .map(|i| id2ref(i)).collect();

    trans.execute(
        concat!("insert into obj.anno (id, pid, rid) ",
                "values ($1, $2, $3) ",
                "on conflict (id) do nothing"),
        &[&id2ref(id),
          &pids,
          &id2ref(&anno.ref_oid)])?;

    trans.execute(
        concat!("insert into obj.file (id) ",
                "values ($1) ",
                "on conflict (id) do nothing"),
        &[&id2ref(&anno.ref_oid)])?;

    trans.execute("DELETE FROM obj.doc where id = $1",
                  &[&id2ref(id)])?;

    let j = serde_cbor::to_vec(&anno.data).map_err(
        |e| io::Error::new(io::ErrorKind::Other, e))?;

    let mut m: serde_json::value::Map<String, serde_json::Value> =
        serde_cbor::from_slice(&j)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    let stmt_fts = trans.prepare(
        concat!("INSERT INTO obj.fts (id, rel, doc) ",
                "VALUES ($1, $2, $3)"))?;


    let id_ref = id2ref(id);

    // use file name to first line
    let name = m.get(ANNO_NAME)
        .map(|v| v.as_str().unwrap_or(""))
        .unwrap_or("")
        .to_string();

    let mut doc: String = name.clone();

    // m, remove note
    match m.remove(ANNO_NOTE) {
        // we only process array
        Some(serde_json::Value::Array(v)) => {
            doc = v
                .iter()
                .fold(name.clone(), |acc, e| {
                    acc + "\n" + e.as_str().unwrap_or("")
                });
        },
        Some(serde_json::Value::String(s)) => {
            doc += &("\n".to_string() + &s);
        },
        _ => ()
    }

    // to fts
    let docs = cut::cut("text/plain", &doc);

    for (rel, doc) in docs.into_iter() {
        if doc.data.len() > 0 {
            trans.execute(&stmt_fts, &[&id_ref, &(rel as i64), &doc])?;
            //println!("rel: {}, doc: {:?}", rel, doc);
        }
    }


    let mut js = serde_json::Value::Object(m);

    trans.execute(concat!("INSERT INTO obj.doc (id, attr) ",
                          "VALUES ($1, $2)"),
                  &[&id_ref, &js])?;

    Ok(trans.commit()?)
}

// TODO: may better here use ref type for data
pub fn import_file(conf: &Conf,
                   client: &mut Client, id: &Id,
                   data: Vec<BTreeMap<String, serde_json::Value>>)
                   -> Result<()> {
    let mut trans = client.transaction()?;
    let id_ref = id2ref(id);

    trans.execute("DELETE FROM obj.doc where id = $1",
                  &[&id2ref(id)])?;
    trans.execute("DELETE FROM obj.fts where id = $1",
                  &[&id2ref(id)])?;

    let stmt = trans.prepare(concat!("INSERT INTO obj.doc (id, attr) ",
                                     "VALUES ($1, $2)"))?;

    let stmt_fts = trans.prepare(
        concat!("INSERT INTO obj.fts (id, rel, doc) ",
                "VALUES ($1, $2, $3)"))?;

    for m in data.into_iter() {
        let mut m1: serde_json::Map<String, serde_json::Value> =
            serde_json::Map::from_iter(m.into_iter());

        // get mt
        let mt = m1.get(CONTENT_TYPE)
            .map(|e| e.as_str().unwrap_or(""))
            .unwrap_or("")
            .to_string();

        // delete X-TIKA:content
        match m1.remove(TIKA_CONTENT) {
            Some(serde_json::Value::String(s)) => {
                // to fts
                let docs = cut::cut(&mt, &s);

                for (rel, doc) in docs.into_iter() {
                    if doc.data.len() > 0 {
                        trans.execute(&stmt_fts, &[&id_ref, &(rel as i64), &doc])?;
                        //println!("rel: {}, doc: {:?}", rel, doc);
                    }
                }
            },
            _ => ()
        }

        let mut js = serde_json::Value::Object(m1);

        util::json_do_map_str(&mut js, &|s| s.replace("\0", ""));

        //println!("m = {:?}", js);

        trans.execute(&stmt, &[&id_ref, &js])?;
    }

    Ok(trans.commit()?)
}