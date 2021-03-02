use postgres::Client;

//use std::io;

use nephrite4_common::util;
use util::Id;

use crate::error::*;

#[derive(Debug, Clone)]
pub enum Search {
    Tag(String, bool), // + -> true, - -> false
    Attr(String, bool, String, bool),
    Fts(String)
}

pub fn to_search(s: &str) -> Search {
    // tags
    if s.starts_with("+") {
        return Search::Tag(s[1..].to_string(), true);
    }

    if s.starts_with("-") {
        return Search::Tag(s[1..].to_string(), false);
    }

    // attributes
    let mut idx: Vec<_> = s.match_indices(":").collect();
    match idx.pop() {
        Some((i, _)) => {
            let k = s[0..i].to_string();
            let v = s[i+1..].to_string();

            fn need_match(x: &str) -> bool {
                // TODO: add support for escape
                x.contains(|x| x == '_' || x == '%')
            }

            let b0 = need_match(&k);
            let b1 = need_match(&v);

            return Search::Attr(k, b0, v, b1);
        },
        _ => ()
    }

    // fts
    Search::Fts(s.to_string())
}

// TODO: fix for '"' in tag
pub fn gen_sql(s: &Search) -> String {
    /*fn quote(s: &str) -> String {
        s.replace("'", "''")
    }*/

    match s {
        &Search::Tag(ref x, b) => {
            if b {
                format!(
                    concat!(
                        "select fid from obj.doc where attr ? 'tag' and ",
                        "(attr->>'tag' = '{}' or ",
                        "attr->>'tag' like '%\"{}\"%')"),
                    x, x)
            }
            else {
                format!(
                    concat!("select fid from obj.doc group by fid ",
                            "having not bool_or(attr->>'tag' = '{}' or ",
                            "attr->>'tag' like '%\"{}\"%')"),
                    x, x)
            }
        },

        &Search::Attr(ref k, true, ref v, b1) => {
            let op0 = if b1 { "ilike" } else { "=" };
            let op1 = if b1 { "ilike" } else { "like" };

            format!(
                concat!("select z0.fid from obj.doc as z0, ",
                        "lateral (select * from jsonb_each_text(z0.attr) ",
                        "where \"key\" ilike '{}' and ",
                        "(\"value\" {} '{}' or ",
                        "\"value\" {} '%\"{}\"%')) as z1"),
                k, op0, v, op1, v)
        },

        &Search::Attr(ref k, false, ref v, b1) => {
            let op0 = if b1 { "ilike" } else { "=" };
            let op1 = if b1 { "ilike" } else { "like" };

            format!(
                concat!("select fid from obj.doc ",
                        "where (attr->>'{}' {} '{}') or ",
                        "(attr->>'{}' {} '%\"{}\"%')"),
                k, op0, v, k, op1, v)
        },

        &Search::Fts(ref x) => {
            // TODO: call cut for words
            let x1 = x.replace("%", ":*");

            format!(
                concat!("select fid from obj.fts where ",
                        "to_tsquery('{}') @@ doc"),
                x1)
        }
    }
}

// lim <= 0, unlimit
pub fn search(client: &mut Client, patt: &[Search], all: bool, limit: i64)
              -> Result<Vec<(Id, Id)>> {

    let mut y = 0;
    let mut sql = format!(
        concat!(
            "select distinct x.id, x.rid from ",
            "(select id, rid from obj.anno ",
            "{}",
            ") as x"),
        if all { "" } else { "where obsolete = false" });

    for s in patt.iter() {
        let s = gen_sql(s);
        sql = format!("{}, ({}) as y{}", sql, s, y);
        y += 1;
    }

    sql += " where ";

    for i in 0..y {
        sql = format!("{} y{}.fid = x.rid and ",
                      sql, i);
    }

    sql += " true";

    if limit > 0 {
        sql += &format!(" limit {}", limit);
    }

    let mut res: Vec<_> = vec![];

    //let stmt = client.prepare(&sql)?;

    for row in &client.query(&sql[..], &[])? {
        let c0: Vec<u8> = row.get(0);
        let c1: Vec<u8> = row.get(1);

        res.push((util::to_id(&c0), util::to_id(&c1)));
    }

    Ok(res)
}

use serde_json::Value;
use serde_json::map::Map;

pub fn get_attr_(client: &mut Client, id: &Id, sql: &str)
                 -> Result<Vec<Map<String, Value>>> {
    let mut res: Vec<Map<String, Value>> = vec![];

    let id_ref: &[u8] = &id[0..];

    for row in &client.query(sql, &[&id_ref])? {
        let v: Value = row.get(0);

        match v {
            Value::Object(m) => res.push(m),
            _ => ()
        }
    }

    Ok(res)
}

pub fn get_attr(client: &mut Client, id: &Id)
                -> Result<Vec<Map<String, Value>>> {
    let sql = "select attr from obj.doc where id = $1";
    get_attr_(client, id, sql)
}

pub fn get_attr_f2a(client: &mut Client, id: &Id, all: bool)
                    -> Result<Vec<Map<String, Value>>> {
    let mut sql = concat!("select distinct doc.attr from obj.doc, obj.anno ",
                          "where anno.rid = $1 and anno.id = doc.id").to_string();
    if !all {
        sql = sql + " and anno.obsolete = false";
    }

    get_attr_(client, id, &sql)
}

pub fn sel_save(client: &mut Client, sel: &str, ids: &[Id],
                clear: bool, append: bool)
                -> Result<()> {
    // insert/update anno
    let mut trans = client.transaction()?;

    if clear {
        trans.execute("delete from sel.tmp where sel = $1",
                      &[&sel])?;
    }
    else if !append {
        let rows = trans.query("select sel from sel.tmp where sel = $1 limit 1",
                               &[&sel])?;
        if rows.len() > 0 {
            return err(&format!("sel '{}' already exist", sel));
        }
    }

    let stmt = trans.prepare(
        concat!("insert into sel.tmp (sel, id) ",
                "values ($1, $2) ",
                "on conflict (sel, id) do nothing"))?;

    for id in ids.iter() {
        let id_ref: &[u8] = &id[0..];
        trans.execute(&stmt, &[&sel, &id_ref])?;
    }

    Ok(trans.commit()?)
}

pub fn sel_load(client: &mut Client, sel: &str, tmp: bool)
                -> Result<Vec<Id>> {
    let mut res: Vec<Id> = vec![];

    let table = if tmp { "sel.tmp" } else { "sel.pers" };

    for row in &client.query(
        &format!("select id from {} where sel = $1", table)[..], &[&sel])? {
        let c0: Vec<u8> = row.get(0);

        res.push(util::to_id(&c0));
    }

    Ok(res)
}

pub fn sel_list(client: &mut Client, tmp: bool) -> Result<Vec<String>> {
    let mut res: Vec<String> = vec![];

    let table = if tmp { "sel.tmp" } else { "sel.pers" };
    for row in &client.query(&format!("select sel from {} group by sel",
                                      table)[..], &[])? {
        let s: String = row.get(0);
        res.push(s);
    }

    Ok(res)
}
