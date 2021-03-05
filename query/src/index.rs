pub mod cut;
pub mod tika;

use log::debug;
use nephrite4_common::{conf, store};
use nephrite4_common::proj;
use nephrite4_common::util;

use postgres::Client;

use serde_json;
use serde_cbor;

use conf::Conf;

use util::Id;
use proj::anno;

use std::{collections::BTreeSet, io};

use std::collections::BTreeMap;
use std::iter::FromIterator;

use crate::{db, error::*};

const ANNO_NAME: &'static str = "name";
const ANNO_NOTE: &'static str = "note";
const TIKA_CONTENT: &'static str = "X-TIKA:content";
const CONTENT_TYPE: &'static str = "Content-Type";

fn id2ref(id: &Id) -> &[u8] {
    &id[0..]
}

fn oid_exist_(client: &mut Client, id: &Id) -> Result<bool> {
    let id_ref: &[u8] = &id[..];
    for _row in client.query("select id from obj.anno where id = $1",
                            &[&id_ref])? {
        return Ok(true)
    }

    for _row in client.query("select id from obj.file where id = $1",
                            &[&id_ref])? {
        return Ok(true)
    }

    return Ok(false)
}

fn _last_anno_(client: &mut Client) -> Result<Option<Id>> {
    for row in client.query(
        "select id from obj.anno order by modified desc limit 1", &[])? {
        let id: Vec<u8> = row.get(0);
        return Ok(Some(util::to_id(&id)));
    }

    Ok(None)
}

fn import_anno_(client: &mut Client,
                id: &Id, anno: &anno::Anno)
                -> Result<()> {
    // insert/update anno
    let mut trans = client.transaction()?;

    let _empty_id_vec: Vec<&[u8]> = vec![];

    let pids: Vec<&[u8]> = anno.pid.iter()
        .map(|i| id2ref(i)).collect();

    trans.execute(
        concat!("insert into obj.anno (id, pid, fid) ",
                "values ($1, $2, $3) ",
                "on conflict (id) do nothing"),
        &[&id2ref(id),
          &pids,
          &id2ref(&anno.fid)])?;

    trans.execute(
        concat!("insert into obj.file (id) ",
                "values ($1) ",
                "on conflict (id) do nothing"),
        &[&id2ref(&anno.fid)])?;

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


    let js = serde_json::Value::Object(m);

    trans.execute(concat!("INSERT INTO obj.doc (id, attr) ",
                          "VALUES ($1, $2)"),
                  &[&id_ref, &js])?;

    Ok(trans.commit()?)
}

fn import_file_(client: &mut Client, id: &Id,
                // NOTE: extracted data maybe recursive
                // for tar.gz, each element is a file
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

pub struct Indexer {
    pub store: store::Store,
    pub client: Client,
    pub tika: tika::Tika,

    done_set: BTreeSet<Id>,
}

impl Indexer {
    pub fn new(conf: &Conf) -> Result<Indexer> {
        let store = store::Store::new(conf)?;
        let client = db::client(conf)?;
        let tika = tika::Tika::new(conf)?;

        let done_set = BTreeSet::new();

        Ok(Indexer { store, client, tika, done_set })
    }

    pub fn is_done(&self, id: &Id) -> bool {
        self.done_set.contains(id)
    }

    // import single "file"
    pub fn import_file(&mut self, id: &Id) -> Result<()> {
        let mut bup = self.store.spawn_bup_join(id)?;

        let res: String;
        {
            let stdout = bup.stdout.take().unwrap();
            res = self.tika.parse_from_fd(stdout)?;
        }
        bup.wait()?;

        let json = tika::tika_res(&res)?;

        import_file_(&mut self.client, &id, json)?;

        Ok(())
    }

    pub fn import_anno(&mut self, id: &Id, with_file: bool) -> Result<()> {
        let anno = self.store.read_commit(id, false)?;

        import_anno_(&mut self.client, id, &anno)?;

        if with_file {
            self.import_file(&anno.fid)?;
        }

        Ok(())
    }

    pub fn index_cset(&mut self, cset: &str) -> Result<usize> {
        let mut list = self.store.walk_cset(cset)?;
        list.reverse();

        let mut res = 0;

        for (cid, aid_set) in list.into_iter() {
            println!("index changeset {}", &hex::encode(&cid[..5]));
            for aid in aid_set {
                if oid_exist_(&mut self.client, &aid)? {
                    println!("  {} exist, skip", &hex::encode(&aid[..5]));
                    continue;
                }

                self.import_anno(&aid, true)?;
                println!("  {} imported", &hex::encode(&aid[..5]));
                res += 1;
            }

            self.store.update_ref(&store::ref_local(&cset), &cid)?;
        }

        Ok(res)
    }

    pub fn index_cset_all(&mut self) -> Result<usize> {
        let refs: Vec<String> =
            self.store.show_ref_all()?.keys()
            .filter(|x| x.starts_with("refs/remotes/") &&
                    x.ends_with("/localhost"))
            .map(|x| x
                 .strip_prefix("refs/remotes/").unwrap()
                 .strip_suffix("/localhost").unwrap()
                 .to_string())
            .collect();

        debug!("index_cset_all: all refs {:?}", refs);

        let mut res = 0;
        for r in refs {
            println!("index changeset for '{}'", &r);
            match self.index_cset(&r) {
                Ok(x) => res += x,
                Err(_) => ()
            }
        }

        Ok(res)
    }
}
