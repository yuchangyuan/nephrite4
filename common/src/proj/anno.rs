// annotation

use crate::util;
use crate::util::Id;

use std::io;
use std::io::prelude::*;

use std::path::{Path, PathBuf};
use std::fs::File;

use std::collections::HashMap;
use std::collections::BTreeMap;

use std::fs;
//use std::fs::metadata;

use serde_cbor;
use serde_cbor::value as cv;

use serde_cbor::ser::to_vec;
//use std::time::SystemTime;
//use std::time::UNIX_EPOCH;

use std::str;

use filetime::FileTime;

use lazy_static::lazy_static;
use log::debug;

#[derive(Debug)]
pub struct Anno {
    pub pid: Vec<Id>, // may empty
    pub ref_oid: Id,
    pub ref_pid: Vec<Id>, // may empty

    pub data: HashMap<String, cv::Value>,

    // manifest dir
    mdir: String,
    // relative path
    rpath: String,
}

pub const SIZE_LIMIT: usize = 1024*1024;

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
        fn add(data: &mut HashMap<String, cv::Value>,
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

        fn set(data: &mut HashMap<String, cv::Value>,
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

        fn del(data: &mut HashMap<String, cv::Value>,
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

    fn get_init_oid_path(&self) -> PathBuf {
        Path::new(&self.mdir)
            .join(self.rpath.to_string() + ".init_oid")
    }

    pub fn get_file_path(&self) -> PathBuf {
        Path::new(&self.mdir).parent().unwrap().join(&self.rpath)
    }

    pub fn data_get(&self, key: &str) -> Option<&cv::Value> {
        self.data.get(key.into())
    }

    fn parse_meta(&mut self) -> io::Result<()> {
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
            else if l.starts_with("# ref_pid") {
                t = 3;
            }
            else if l.len() == 52 { // ceiling(256 / 5)
                let id = util::zbase32_to_id(&l);

                if t == 1 {
                    self.pid.push(id);
                }
                else if t == 2 {
                    self.ref_oid = id;
                }
                else if t == 3 {
                    self.ref_pid.push(id);
                }
            }
        }

        Ok(())
    }

    pub fn parse_yaml(&mut self) -> io::Result<()> {
        let mut file = File::open(self.get_yaml_path())?;
        let mut content = String::new();
        file.read_to_string(&mut content)?;

        let v: cv::Value = serde_yaml::from_str(&content).map_err(
            |e| io::Error::new(io::ErrorKind::Other,
                               e))?;

        self.data = HashMap::new();

        if let cv::Value::Map(m) = v {
            for (k1, v1) in m {
                if let cv::Value::Text(k2) = k1 {
                    self.data.insert(k2.to_string(), v1.clone());
                }
            }
        }

        Ok(())
    }

    pub fn update_pid(&mut self, rpath: &str) -> bool {
        let mut res: bool = false;
        let f = Path::new(&self.mdir).parent().unwrap().join(rpath);

        if f.is_file() {
            res = true;

            match util::calc_id(
                &f.into_os_string().into_string().unwrap()) {
                Ok(ref_pid) =>
                    self.ref_pid.push(ref_pid),
                _ => ()
            }

            match Anno::new(&self.mdir, rpath, true) {
                Ok(_anno) => {
                    // TODO, merge tag
                    //anno.data["tag"]
                },
                _ => ()
            }
        }

        res
    }


    // usually return ok
    pub fn match_pid(&mut self) -> bool {
        let rpath = self.rpath.to_string();
        let mut v: Vec<&str> = rpath.split('.').collect();

        let ext = v.pop().unwrap().to_lowercase();
        let mut same_level = true;

        loop {
            if v.len() < 1 || v.last().unwrap().contains("/") {
                break
            }

            //println!("ext = {}, join = {}", ext, v.join(".") + ".EXT");

            // TODO, should not hard encoding,
            // specify handle JPG file
            if ext == "jpg" {
                if self.update_pid(&(v.join(".") + ".CR2")) {
                    return true
                }

                if self.update_pid(&(v.join(".") + ".RAF")) {
                    return true
                }
            }


            if self.update_pid(&v.join(".")) { return true }

            if !same_level && self.update_pid(&(v.join(".") + "." +  &ext)) {
                return true
            }

            v.pop().unwrap();
            same_level = false;
        }

        false
    }

    pub fn update_meta(&mut self) -> io::Result<()> {
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

        if mt > 0 {
            self.data.insert("mtime".into(), cv::Value::Integer(mt as i128));
        }

        Ok(())
    }

    // init a anno struct
    pub fn new(mdir: &str, rpath: &str, load_only: bool)
               -> io::Result<Anno> {
        let m = Path::new(mdir);
        let f = m.parent().unwrap().join(rpath);

        //println!("m = {:?}, f = {:?}, rpath={:?}", m, f, rpath);


        // check mdir exit, check file exist
        if !m.is_dir() || !f.is_file() {
            return Err(io::Error::new(io::ErrorKind::NotFound,
                                      "file not exist"));
        }

        let mut res = Anno {
            pid: vec![],
            ref_oid: [0;32],
            ref_pid: vec![],
            data: HashMap::new(),
            mdir: mdir.into(),
            rpath: rpath.into(),
        };

        // if exist yaml & meta, then load
        if res.get_yaml_path().is_file() && res.get_meta_path().is_file() {
            debug!("new: load yaml & meta");
            res.parse_meta()?;
            res.parse_yaml()?;
        }
        else {
            if load_only {
                return Err(io::Error::new(io::ErrorKind::NotFound,
                                          "manifest not exist"));
            }
            debug!("new: create yaml & meta");

            // init yaml & meta
            // pid should be []
            res.ref_oid = util::calc_id(&f.clone().into_os_string().
                                        into_string().unwrap())?;

            // TODO, find ref pid
            // ab.cc.dd.ee -> ab.cc.dd -> ab.cc, ab
            // ab.cc.JPG/jpg -> ab.cc.CR2 -> ab.cc.RAF
            res.match_pid();

            // update fn & size
            res.data.insert("name".into(),
                            cv::Value::Text(rpath.into()));

            res.update_meta()?;

            // save to file
            res.save()?;
        }

        Ok(res)
    }

    // save file
    pub fn save(&mut self) -> io::Result<()> {
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
                                        util::to_zbase32(&self.ref_oid)))?;

            meta.write_fmt(format_args!("\n# ref_pid\n"))?;
            for i in self.ref_pid.clone() {
                meta.write_fmt(format_args!("{}\n", util::to_zbase32(&i)))?;
            }
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

    pub fn gen(&self) -> io::Result<Vec<u8>> {
        // ordered data
        let mut data: BTreeMap<_, _> = self.data
            .clone().into_iter().collect();
        let v1 = cv::Value::Array(self.pid.iter()
                                  .map(|x| cv::Value::Bytes(x.to_vec()))
                                  .collect());
        let v2 = cv::Value::Bytes(self.ref_oid.to_vec());
        let v3 = cv::Value::Array(self.ref_pid.iter()
                                  .map(|x| cv::Value::Bytes(x.to_vec()))
                                  .collect());
        data.insert("_".into(), cv::Value::Array(vec![v1, v2, v3]));

        Ok(to_vec(&data).unwrap())
    }

    pub fn decode(slice: &[u8]) -> io::Result<Anno> {
        fn other_err(s: &'static str) -> io::Error {
            io::Error::new(io::ErrorKind::Other, s)
        }

        let data: HashMap<String, cv::Value> =
            serde_cbor::from_slice(slice)
            .map_err(|_e| other_err("cbor decode err"))?;

        fn is_id_ary(v: &cv::Value) -> bool {
            if let cv::Value::Array(_a) = v {
                for e in _a {
                    if let cv::Value::Bytes(_) = e {
                    }
                    else {
                        return false;
                    }
                }

                true
            }
            else {
                false
            }
        }

        fn to_id(slice: &[u8]) -> Id {
            let mut res: Id = [0u8;32];

            for (i, b) in slice.iter().enumerate() {
                if i < 32 { res[i] = *b }
            }

            res
        }

        fn decode_id_ary(v: &cv::Value) -> Vec<Id> {
            match v {
                cv::Value::Array(_a) => {
                    _a.iter().map(|e| match e {
                        cv::Value::Bytes(_b) => to_id(&_b),
                        _ => panic!("should not happen")
                    }).collect()
                },
                _ => {
                    panic!("should not happen")
                }
            }
        }

        let mut res = Anno {
            pid: vec![],
            ref_oid: [0u8;32],
            ref_pid: vec![],
            data: data,
            mdir: ".".into(),
            rpath: ".".into()
        };

        let m = res.data.remove("_").ok_or(other_err("format err"))?;

        let v = match m {
            cv::Value::Array(_v) => _v,
            _ => return Err(other_err("meta is not array")),
        };

        if v.len() != 3 {
            return Err(other_err("len of meta array is not 3"));
        }

        if let cv::Value::Bytes(ref _b) = v[1] {
            res.ref_oid = to_id(_b);
        }
        else {
            return Err(other_err("ref_oid err"));
        }

        if !is_id_ary(&v[0]) || !is_id_ary(&v[2]) {
            return Err(other_err("pid or ref_pid is not array"));
        }

        res.pid = decode_id_ary(&v[0]);
        res.ref_pid = decode_id_ary(&v[2]);

        Ok(res)
    }

    pub fn decode_file<P: AsRef<Path>>(p: P) -> io::Result<Anno> {
        let t = tree_magic::from_filepath(p.as_ref());

        // cbor not recognized
        if t != "application/cbor"
            && t != "application/octet-stream"
            && t != "text/plain"
        {
            return Err(io::Error::new(io::ErrorKind::Other,
                                      format!("not cbor, {}", t)));
        }

        let meta = fs::metadata(&p)?;
        if meta.len() > SIZE_LIMIT as u64 {
            return Err(io::Error::new(io::ErrorKind::Other, "over size"));
        }

        // read cbor
        let mut buf: Vec<u8> = vec![];
        let mut file = File::open(p)?;
        file.read_to_end(&mut buf)?;

        Anno::decode(&buf)
    }

    pub fn get_oid(&self) -> Id {
        let cbor = self.gen().unwrap();
        util::calc_id_buf(&cbor)
    }

    // update yaml & meta
    pub fn update(&mut self) -> io::Result<bool> {
        let mut res = false;

        // update _ref_oid in meta file if necessary
        let meta_m = fs::metadata(self.get_meta_path())?.modified()?;
        let yaml_m = fs::metadata(self.get_yaml_path())?.modified()?;
        let file_m = fs::metadata(self.get_file_path())?.modified()?;

        if file_m != meta_m  {
            res = true;

            // update ref_oid
            self.ref_oid = util::calc_id(&self.get_file_path()
                                         .into_os_string()
                                         .into_string().unwrap())?;

            // update meta
            self.update_meta()?;
        }

        if file_m != yaml_m {
            res = true;

            self.update_meta()?;
        }

        if res {
            self.save()?;

            // check file size limit
            let cbor = self.gen().unwrap();
            if cbor.len() > SIZE_LIMIT {
                return Err(io::Error::new(io::ErrorKind::Other,
                                          "anno size exceed limit"));
            }
        }

        Ok(res)
    }

    // 0: ok, 1: need update, 2: need commit, -1: error
    pub fn status(&self) -> i8 {
        match self.status_() {
            Ok(x) => x,
            _ => -1
        }
    }

    // when export from store or commit to store, will generate .init_oid file
    pub fn get_init_oid(&self) -> io::Result<Id> {
        let mut content = String::new();
        let mut file = File::open(self.get_init_oid_path())?;
        file.read_to_string(&mut content)?;

        let id = util::zbase32_to_id(&content);

        Ok(id)
    }

    fn status_(&self) -> io::Result<i8> {
        let meta_m = fs::metadata(self.get_meta_path())?.modified()?;
        let yaml_m = fs::metadata(self.get_yaml_path())?.modified()?;
        let file_m = fs::metadata(self.get_file_path())?.modified()?;

        //println!("{:?}, {:?}, {:?}",
        //file_m, yaml_m, meta_m);

        if file_m != meta_m { return Ok(1); }
        if file_m != yaml_m { return Ok(1); }

        let _id = self.get_oid();

        match self.get_init_oid() {
            Ok(id) => {
                if id != self.get_oid() {
                    return Ok(2)
                }
            },
            _ => return Ok(2)
        }

        Ok(0)
    }

    // return new status
    pub fn commit<F>(&mut self, commit_func: F) -> io::Result<i8>
        where F: Fn(&Anno) -> io::Result<()>
    {
        let st = self.status();
        if st != 2 { return Ok(st); }

        commit_func(self)?;

        // update pid
        let oid = self.get_oid();
        self.pid = vec![oid];

        self.save()?;

        let init_oid = self.get_oid();
        let mut f = File::create(self.get_init_oid_path())?;
        f.write(util::to_zbase32(&init_oid).as_bytes())?;

        Ok(self.status())
    }

    pub fn get_name(&self) -> Option<String> {
        match self.data_get("name") {
            Some(&cv::Value::Text(ref v)) => Some(v.clone()),
            _ => None
        }
    }
}
