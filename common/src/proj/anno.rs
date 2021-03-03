// annotation

use crate::util;
use crate::util::Id;

use crate::error::*;

use std::io;
use std::io::prelude::*;

use std::path::{Path, PathBuf};
use std::fs::File;

use std::collections::BTreeMap;

use std::fs;
//use std::fs::metadata;

use serde_cbor;
use serde_cbor::value as cv;

//use std::time::SystemTime;
//use std::time::UNIX_EPOCH;

use std::str;

use filetime::FileTime;

use lazy_static::lazy_static;
use log::debug;

#[derive(PartialEq, Eq)]
pub enum St {
    Ready, // '.'
    MMeta, // 'm'
    MFile, // 'M'
}

pub fn st2chr(st: St) -> String {
    match st {
        St::Ready => ".",
        St::MMeta => "m",
        St::MFile => "M",
    }.to_string()
}

#[derive(Debug)]
pub struct Anno {
    // meta
    pub pid: Vec<Id>, // may empty
    pub fid: Id, // use fid when mtime match
    pub anno_hash: Id, // use to check whether yaml/meta need update

    // yaml
    pub data: BTreeMap<String, cv::Value>,

    // manifest dir
    mdir: String,
    // relative path
    rpath: String,
}

lazy_static! {
    static ref PREDEFINED: Vec<String> = {
        let v = vec!["name", "type", "size", "note",
                     "rate", "tag", "node", "level"];
        v.iter().map(|v| v.to_string()).collect()
    };
}

impl Anno {
    pub fn proc_op(&mut self, ops: &[String], allow_new: bool) -> u64 {
        // +xxx, -xxx -> tag+xxx, tag-xxx
        // xxx=xxx -> set kv
        // xxx+xxx -> append
        // xxx-xxx -> remove
        fn add(data: &mut BTreeMap<String, cv::Value>,
               key: &str, val: &str, an: bool) -> bool {
            let key_s = key.to_string();
            let mut res = true;

            let exists = data.contains_key(key);
            if !exists && !PREDEFINED.contains(&key_s) && !an {
                return false;
            }

            let vstr = cv::Value::Text(val.to_string());

            if exists {
                let mut v = data.remove(&key_s).unwrap();

                match v {
                    cv::Value::Array(ref mut a) => {
                        for z in a.iter() {
                            if &vstr == z {
                                res = false;
                            }
                        }

                        if res {
                            a.push(vstr);
                        }
                    },
                    _ => {
                        if vstr == v {
                            res = false;
                        }

                        if res {
                            v = cv::Value::Array(vec![v, vstr]);
                        }
                    }
                }

                data.insert(key_s, v);
            }
            else {
                data.insert(key_s, vstr);
            }

            res
        }

        fn set(data: &mut BTreeMap<String, cv::Value>,
               key: &str, val: &str, an: bool) -> bool {
            let exists = data.contains_key(key);
            let key_s = key.to_string();
            if !exists && !PREDEFINED.contains(&key_s) && !an {
                return false;
            }
            let vstr = cv::Value::Text(val.to_string());
            data.insert(key_s, vstr);
            true
        }

        fn del(data: &mut BTreeMap<String, cv::Value>,
               key: &str, val: &str) -> bool {
            let exists = data.contains_key(key);
            if !exists { return false; }

            let key_s = key.to_string();

            let old = data.remove(key).unwrap();
            let vstr = cv::Value::Text(val.to_string());

            match &old {
                cv::Value::Text(ref _t) => {
                    if _t == &val { return true; }

                    data.insert(key.to_string(), old);
                    return false;
                },

                // TODO: in-place modify old
                cv::Value::Array(_x) => {
                    let mut a: Vec<cv::Value> = vec![];
                    let mut res = false;

                    for x in _x.iter() {
                        if &vstr == x {
                            res = true;
                        }
                        else {
                            a.push(x.clone())
                        }
                    }

                    if !a.is_empty() {
                        data.insert(key_s, cv::Value::Array(a));
                    }

                    return res;
                },

                _ => (),
            };

            false
        }

        let mut cnt = 0;
        let ref mut data = self.data;

        //println!("before -- {:?}", data);

        for op in ops.iter() {
            if op.starts_with("+") {
                if add(data, "tag", &op[1..], allow_new) { cnt += 1 }
            }
            else if op.starts_with("-") {
                if del(data, "tag", &op[1..]) { cnt += 1 }
            }
            else if op.contains("+") {
                let p = op.find("+").unwrap();
                if add(data, &op[..p], &op[p+1..], allow_new) { cnt += 1 }
            }
            else if op.contains("-") {
                let p = op.find("-").unwrap();
                if del(data, &op[..p], &op[p+1..]) { cnt += 1 }
            }
            else if op.contains("=") {
                let p = op.find("=").unwrap();
                if set(data, &op[..p], &op[p+1..], allow_new) { cnt += 1 }
            }
        }

        //println!("after -- {:?}", data);

        cnt
    }

    fn get_meta_path(&self) -> PathBuf {
        Path::new(&self.mdir).join(self.rpath.to_string() + ".meta")
    }

    fn get_yaml_path(&self) -> PathBuf {
        Path::new(&self.mdir).join(self.rpath.to_string() + ".yaml")
    }

    pub fn get_file_path(&self) -> PathBuf {
        Path::new(&self.mdir).parent().unwrap().join(&self.rpath)
    }

    pub fn data_get(&self, key: &str) -> Option<&cv::Value> {
        self.data.get(key.into())
    }

    fn parse_meta(&mut self) -> Result<()> {
        let mut content = String::new();
        let mut file = File::open(self.get_meta_path())?;
        file.read_to_string(&mut content)?;

        // 1 -> _pid, 2 -> _ref_oid, 3 -> ref_pid
        let mut t: i32 = 0;

        // cbor
        // { "_": [[pids], [ref_oid], [ref_pid]] }

        for l in content.lines() {
            if l.starts_with("# pid") {
                t = 1;
            }
            else if l.starts_with("# ref_oid") {
                t = 2;
            }
            else if l.starts_with("# anno_hash") {
                t = 3;
            }
            else {
                if l.len() == 52 { // ceiling(256 / 5)
                    let id = util::zbase32_to_id(&l);

                    if t == 1 {
                        self.pid.push(id);
                    }
                    else if t == 2 {
                        self.fid = id;
                    }
                    else if t == 3 {
                        self.anno_hash = id;
                    }
                }
            }
        }

        Ok(())
    }


    pub fn parse_yaml(&mut self) -> Result<()> {
        let mut file = File::open(self.get_yaml_path())?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;

        self.parse_yaml_(&content)
    }

    fn parse_yaml_(&mut self, content: &str) -> Result<()> {
        let v: cv::Value = serde_yaml::from_str(content)?;

        self.data = BTreeMap::new();

        if let cv::Value::Map(m) = v {
            for (k1, v1) in m {
                if let cv::Value::Text(k2) = k1 {
                    self.data.insert(k2.to_string(), v1.clone());
                }
            }
        }

        Ok(())
    }

    pub fn update_meta(&mut self) -> Result<()> {
        let f = self.get_file_path();
        let mut mt = 0u64;

        fn get_ms(meta: &fs::Metadata) -> u64 {
            let ft = FileTime::from_last_modification_time(&meta);
            let sec = ft.unix_seconds() as u64 * 1000;
            let ns = ft.nanoseconds() as u64 / 1000000;
            sec + ns
        }

        match fs::metadata(&f) {
            Ok(meta) => {
                self.data.insert("size".into(),
                                 cv::Value::Integer(meta.len() as i128));
                mt = get_ms(&meta);
            },
            _ => ()
        }


        let t = tree_magic::from_filepath(&f);

        self.data.insert("type".into(), cv::Value::Text(t));

        match fs::metadata(&self.get_yaml_path()) {
            Ok(meta) => {
                let mt1 = get_ms(&meta);
                if mt1 > mt { mt = mt1; }
            },
            _ => ()
        }

        // mtime is any newer of anno(yaml) & file
        self.data.insert("mtime".into(), cv::Value::Integer(mt as i128));

        Ok(())
    }

    // init a anno struct
    pub fn new(mdir: &str, rpath: &str, load_only: bool)
               -> Result<Anno> {
        let m = Path::new(mdir);
        let f = m.parent().unwrap().join(rpath);

        //println!("m = {:?}, f = {:?}, rpath={:?}", m, f, rpath);


        // check mdir exit, check file exist
        if !m.is_dir() || !f.is_file() {
            return Err(Error::IO(io::Error::new(io::ErrorKind::NotFound,
                                                "file not exist")));
        }

        let mut res = Anno {
            pid: vec![],
            anno_hash: [0;32],
            fid: [0;32],

            data: BTreeMap::new(),
            mdir: mdir.into(),
            rpath: rpath.into(),
        };

        // if exist yaml & meta, then load
        if res.get_yaml_path().is_file() && res.get_meta_path().is_file() {
            debug!("new: load yaml & meta");
            res.parse_meta()?;
            res.parse_yaml()?;

            if !load_only {
                res.sync()?;
            }
        }
        else {
            if load_only {
                return Err(Error::IO(io::Error::new(io::ErrorKind::NotFound,
                                                    "manifest not exist")));
            }
            debug!("new: create yaml & meta");

            // init yaml & meta

            // update fn & size
            res.data.insert("name".into(),
                            cv::Value::Text(rpath.into()));

            res.update_meta()?;
            res.anno_hash = res.get_hash();

            // save to file
            res.save()?;
        }

        Ok(res)
    }

    // save file
    pub fn save(&mut self) -> Result<()> {
        let file_meta = fs::metadata(self.get_file_path())?;
        let ft = FileTime::from_last_modification_time(&file_meta);

        // save meta
        {
            let mut meta = File::create(self.get_meta_path())?;

            //println!("meta = {:?}", meta);

            meta.write_fmt(format_args!("# pid\n"))?;
            for i in self.pid.clone() {
                meta.write_fmt(format_args!("{}\n", util::to_zbase32(&i)))?;
            }

            meta.write_fmt(format_args!("\n# ref_oid\n{}\n",
                                        util::to_zbase32(&self.fid)))?;

            meta.write_fmt(format_args!("\n# anno_hash\n{}\n",
                                        util::to_zbase32(&self.anno_hash)))?;
        }

        //println!("ft = {:?}", ft);
        filetime::set_file_times(self.get_meta_path(), ft, ft)?;

        //let v: cv::Value = serde_yaml::from_str(&content).unwrap();
        //self.data = v.as_object().unwrap().clone();

        // save yaml
        {
            let mut yaml = File::create(self.get_yaml_path())?;

            //println!("yaml = {:?}", yaml);

            let y = serde_yaml::to_string(&self.data).unwrap();
            yaml.write_all(y.as_bytes())?;
        }

        filetime::set_file_times(self.get_yaml_path(), ft, ft)?;

        // to avoid time round error
        filetime::set_file_times(self.get_file_path(), ft, ft)?;

        Ok(())
    }


    // decode from commit, not cbor
    pub fn decode(parents: &[Id], ref_oid: &Id, yaml: &str,
                  full: bool) -> Result<Anno> {
        let pid = parents.iter().map(|i| i.clone()).collect();
        let mut res = Anno {
            pid,
            fid: ref_oid.clone(),
            anno_hash: [0;32],
            data: BTreeMap::new(),
            mdir: ".".to_string(),
            rpath: "".to_string(),
        };

        // update data
        if yaml.starts_with("---") {
            res.parse_yaml_(yaml)?;
        }
        else {
            res.parse_yaml_(&("---\n".to_string() + yaml))?;
        };

        if full {
            // update anno_hash
            res.anno_hash = res.get_hash();

            // update rpath
            if let Some(name) = res.get_name() {
                res.rpath = name;
            }
        }


        Ok(res)
    }

    pub fn get_hash(&self) -> Id {
        let yaml = serde_yaml::to_vec(&self.data).unwrap();
        util::calc_id_buf(&yaml)
    }

    pub fn gen_yaml(&self) -> Result<String> {
        Ok(serde_yaml::to_string(&self.data)?)
    }

    // update yaml & meta, return changed status
    // but not save
    pub fn sync(&mut self) -> Result<bool> {
        let mut res = false;

        // update _ref_oid in meta file if necessary
        let meta_m = fs::metadata(self.get_meta_path())?.modified()?;
        let yaml_m = fs::metadata(self.get_yaml_path())?.modified()?;
        let file_m = fs::metadata(self.get_file_path())?.modified()?;

        if file_m != meta_m  {
            res = true;

            // update meta
            self.update_meta()?;
        }

        if file_m != yaml_m {
            res = true;

            self.update_meta()?;
        }

        Ok(res)
    }

    // NOTE: when file change, mtime or size part of anno will change
    pub fn status(&self) -> Result<St> {
        if self.fid == [0;32] { return Ok(St::MFile); }
        if self.pid.is_empty() { return Ok(St::MFile); }

        let meta_m = fs::metadata(self.get_meta_path())?.modified()?;
        //let yaml_m = fs::metadata(self.get_yaml_path())?.modified()?;
        let file_m = fs::metadata(self.get_file_path())?.modified()?;


        //println!("{:?}, {:?}, {:?}",
        //file_m, yaml_m, meta_m);

        if file_m != meta_m { return Ok(St::MFile); }

        // check actual yaml hash
        if self.get_hash() != self.anno_hash {
            return Ok(St::MMeta);
        }

        Ok(St::Ready)
    }

    // TODO
    // NOTE: only
    pub fn commit<F>(&mut self, mut commit_func: F) -> Result<()>
        where F: FnMut(St, &mut Anno) -> Result<()>
    {
        let st = self.status()?;
        if st == St::Ready { return Ok(()) }

        // sync file status
        self.sync()?;

        // here should, update pid & ref_oid
        commit_func(st, self)?;

        // update anno_hash
        self.anno_hash = self.get_hash();

        self.save()?;

        Ok(())
    }

    pub fn get_name(&self) -> Option<String> {
        match self.data_get("name") {
            Some(&cv::Value::Text(ref v)) => Some(v.clone()),
            _ => None
        }
    }
}
