use std::process::{self, Command, Stdio};

use crate::util;
use crate::conf::Conf;
use crate::proj::{anno, manifest};
use util::Id;

use crate::error::*;

use std::io;

use std::io::Write;

use std::io::BufReader;

// for read_until
use std::io::BufRead;
// for read_to_string
use std::io::Read;

//use std::ffi::OsString;

use anno::{St, Anno};
use manifest::Manifest;

use chrono::prelude::*;

use serde_cbor::value as cv;

use hex;
use std::str;

use log::debug;

const BUP_CMD: &'static str = "bup256";
const ENV_BUP_DIR: &'static str = "BUP_DIR";
const ENV_BUP_FORCE_TTY: &'static str = "BUP_FORCE_TTY";

const ENV_GIT_DIR: &'static str = "GIT_DIR";
const ENV_GIT_AUTHOR_DATE: &'static str = "GIT_AUTHOR_DATE";
const ENV_GIT_COMMITTER_DATE: &'static str = "GIT_COMMITTER_DATE";

const OID_LEN: usize = 32;

fn min_uniq_len(list: &Vec<Id>) -> usize {
    let len = list.len();

    'outer: for res in 1..33 {
        for i in 1..len {
            for j in 0..i {
                if list[i][..res] == list[j][..res] {
                    continue 'outer;
                }
            }
        }

        return res;
    }

    32
}


#[derive(Debug, Copy, Clone)]
pub enum ObjType {
    // 160000
    Commit,
    // 040000
    Tree,
    // file: 100755 or 100644
    // symlink: 120000
    Blob(i32),
    Tag,
}

impl ObjType {
    fn mode(&self) -> i32 {
        match self {
            ObjType::Commit  => 0o160000,
            ObjType::Tree    => 0o040000,
            ObjType::Blob(m) => *m,
            ObjType::Tag     => 0,
        }
    }

    pub fn from_mode(m: i32) -> ObjType {
        match m {
            0o160000 => ObjType::Commit,
            0o040000 => ObjType::Tree,
            _ => ObjType::Blob(m),
        }
    }
}

fn type2mode(t: &ObjType) -> i32 {
    t.mode()
}

#[derive(Debug, Clone)]
pub struct Store {
    pub root: String,

    date: u64,
    zone: String,
}

#[derive(Debug, Clone)]
pub struct Commit {
    pub obj_list: Vec<Id>,
    pub inc_head: Option<Id>,
}

impl Store {
    pub const INC_REF: &'static str = "refs/heads/inc";

    pub fn new(conf: &Conf) -> Result<Store> {
        let mut res = Store {
            root: conf.root(),
            date: 0,
            zone: "+0000".to_string(),
        };

        res.update_time()?;

        Ok(res)
    }

    fn update_time(&mut self) -> Result<()> {
        let local: DateTime<Local> = Local::now();

        // update time
        self.date = local.timestamp() as u64;

        // update zone
        let zone = local.offset().fix().local_minus_utc();

        let zone_sign = if zone < 0 { "-" } else { "+" };
        let zone_abs = zone.abs() / 60;
        let zone_hr = zone_abs / 60;
        let zone_min = zone_abs - zone_hr * 60;

        self.zone = format!("{}{:02}{:02}",
                            zone_sign, zone_hr, zone_min).to_string();

        Ok(())
    }

    pub fn show_ref(&self, git_ref: &str) -> Result<Option<Id>> {
        let git = Command::new("git")
            .env(ENV_GIT_DIR, &self.root)
            .arg("show-ref")
            .arg("--verify")
            .arg("--hash")
            .arg(git_ref)
            .output()
            .expect("failed to execute git-show-ref");

        // not exist
        if !git.status.success() { return Ok(None) }

        let out = String::from_utf8_lossy(&git.stdout);
        let mut res = [0;32];
        hex::decode_to_slice(&out.trim(), &mut res).unwrap();

        Ok(Some(res))
    }

    fn update_ref(&self, git_ref: &str, commit: &Id) -> Result<()> {
        let git = Command::new("git")
            .env(ENV_GIT_DIR, &self.root)
            .arg("update-ref")
            .arg(git_ref)
            .arg(&hex::encode(commit))
            .output()
            .expect("failed to execute git-update-ref");

        // not exist
        if !git.status.success() {
            return err("git update-ref fail")
        }

        Ok(())
    }

    fn cat_file(&self, tp: &str, commit: &Id) -> Result<Vec<u8>> {
        let git = Command::new("git")
            .env(ENV_GIT_DIR, &self.root)
            .arg("cat-file")
            .arg(tp)
            .arg(&hex::encode(commit))
            .output()
            .expect("failed to execute git-cat-file");

        // not exist
        if !git.status.success() {
            return err("git update-ref fail")
        }

        Ok(git.stdout)
    }

    pub fn read_commit(&self, commit: &Id, full: bool) -> Result<Anno> {
        let raw = self.cat_file("commit", commit)?;
        let buf = str::from_utf8(&raw).unwrap();

        let mut slice = &buf[..];
        let mut ln: &str;
        let mut pid_list: Vec<Id> = vec![];
        let mut tree: Id = [0;32];

        loop {
            let t = slice.find('\n');

            ln = "";
            if let Some(n) = t {
                ln = &slice[..n+1];
                slice = &slice[n..];
            }

            let ln1 = ln.trim();

            if ln1.starts_with("tree ") {
                tree = util::to_id(&hex::decode(ln1[5..].trim()).unwrap());
            }
            else if ln1.starts_with("parent ") {
                let pid = util::to_id(&hex::decode(ln1[6..].trim()).unwrap());
                pid_list.push(pid);
            }
            else if ln1.is_empty() {
                break
            }
        }

        let res = Anno::decode(&pid_list[..], &tree, slice, full)?;

        Ok(res)
    }

    pub fn read_tree(&self, commit: &Id) -> Result<Vec<(ObjType, String, Id)>> {
        let raw = self.cat_file("tree", commit)?;

        let mut idx0 = 0;
        let mut idx1 = 0;

        let next_byte_match = |b: u8, idx: &mut usize| -> bool {
            while *idx < raw.len() {
                if raw[*idx] == b { return true; }
                *idx += 1;
            }

            false
        };

        let mut res = vec![];

        loop {
            if !next_byte_match(b' ', &mut idx1) { break; }

            let mode = str::from_utf8(&raw[idx0..idx1]).unwrap();
            idx0 = idx1 + 1;
            idx1 = idx0;

            if !next_byte_match(0, &mut idx1) { break; }
            let name = str::from_utf8(&raw[idx0 .. idx1]).unwrap();

            idx0 = idx1 + 1;
            idx1 = idx0 + OID_LEN;

            let id = util::to_id(&raw[idx0 .. idx1]);

            let mode_i = ("0o".to_string() + &mode[..]).parse::<i32>().unwrap();

            res.push((ObjType::from_mode(mode_i), name.to_string(), id))
        }

        Ok(res)
    }

    fn write_tree(&self, list: &[(ObjType, &Id, &str)]) -> Result<Id> {
        /* format
        tree <size>\0
        <mode> <name>\0<oid>
        <mode> <name>\0<oid>
        ...
         */
        let mut git = Command::new("git")
            .env(ENV_GIT_DIR, &self.root)
            .arg("hash-object")
            .arg("-t")
            .arg("tree")
            .arg("--stdin")
            .arg("-w")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to execute git-hash-object");

        {
            let mut stdin = git.stdin.take().unwrap();

            for (tp, id, name) in list {
                stdin.write_fmt(format_args!("{:o} {}\0", type2mode(tp), name))?;
                stdin.write_all(*id)?;
            }
        }

        // close stdin

        let mut stdout = git.stdout.take().unwrap();

        let mut out = String::new();
        stdout.read_to_string(&mut out)?;
        let out1 = out.trim();

        assert_eq!(out1.len(), OID_LEN * 2);

        let mut res = [0;32];
        hex::decode_to_slice(out1, &mut res).unwrap();

        debug!("write tree {}", out1);

        Ok(res)
    }


    fn commit_tree(&self, parents: &[Id], tree: &Id,
                   time: u64, msg: &str) -> Result<Id> {
        let date = format!("{} {}", time, self.zone);

        let mut pid_list = vec![];

        for parent in parents.iter() {
            pid_list.push("-p".to_string());
            pid_list.push(hex::encode(parent));
        }

        let git = Command::new("git")
            .env(ENV_GIT_DIR, &self.root)
            .env(ENV_GIT_AUTHOR_DATE, &date)
            .env(ENV_GIT_COMMITTER_DATE, &date)
            .arg("commit-tree")
            .args(&pid_list)
            .arg("-m")
            .arg(msg)
            .arg(hex::encode(tree))
            .output()
            .expect("failed to execute git-commit-tree");

        let out = String::from_utf8_lossy(&git.stdout);

        let mut res = [0;32];

        hex::decode_to_slice(&out.trim(), &mut res).unwrap();

        Ok(res)
    }

    fn commit_anno(&self, anno: &Anno) -> Result<Id> {
        let mut time = self.date;

        if let Some(cv::Value::Integer(_i)) = anno.data.get("mtime") {
            time = (*_i / 1000) as u64;
        };

        let yaml = anno.gen_yaml()?;
        let msg = yaml.trim_start_matches('-').trim();

        self.commit_tree(&anno.pid, &anno.ref_oid, time, msg)
    }

    pub fn commit(&mut self, manifest: &mut Manifest) -> Result<Commit> {
        self.update_time()?;

        let mut res = Commit { obj_list: vec![], inc_head: None };

        for (name, anno) in manifest.anno_map.iter_mut() {
            println!("--> {}", name);

            //
            let func = |st: St, an: &mut Anno| -> Result<()> {
                if st == St::MFile {
                    an.ref_oid = self.import(an.get_file_path()
                                             .to_str()
                                             .unwrap())?;
                };

                // create commit
                let pid = self.commit_anno(an)?;

                println!("    commit {} {}",
                         &util::to_zbase32(&pid)[..8],
                         &hex::encode(&pid));

                res.obj_list.push(pid.clone());

                an.pid.clear();
                an.pid.push(pid);

                Ok(())
            };

            anno.commit(func)?
        }

        if res.obj_list.is_empty() { return Ok(res) }

        // create tree for new commit
        let len = min_uniq_len(&res.obj_list);

        let mut list = vec![];
        for id in res.obj_list.iter() {
            let name = hex::encode(&id[..len]);
            list.push((ObjType::Commit, id, name));
        }

        let list1: Vec<(ObjType, &Id, &str)> =
            list.iter().map(|(a, b, c)| (*a, *b, &c[..])).collect();

        let tree = self.write_tree(&list1[..])?;

        println!("---");
        println!("tree   {} {}",
                 &util::to_zbase32(&tree)[..8],
                 &hex::encode(tree));

        // update inc
        let inc = self.show_ref(Self::INC_REF)?;
        let inc_parent = match inc {
            Some(id) => vec![id],
            None => vec![],
        };

        let inc_commit = self.commit_tree(&inc_parent, &tree,
                                     self.date, "")?;

        self.update_ref(Self::INC_REF, &inc_commit)?;

        println!("commit {} {}",
                 &util::to_zbase32(&inc_commit)[..8],
                 &hex::encode(&inc_commit));

        res.inc_head = Some(inc_commit);


        Ok(res)
    }

    pub fn spawn_bup_join(&self, id: &Id) -> Result<process::Child> {
        let bup = Command::new(BUP_CMD)
            .env(ENV_BUP_DIR, self.root.clone())
            .env("LC_ALL", "C")
            .arg("join")
            .arg(hex::encode(id))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;

        Ok(bup)
    }

    pub fn import(&self, path: &str) -> Result<Id> {
        let mut bup = Command::new(BUP_CMD)
            .env(ENV_BUP_DIR, self.root.clone())
            .env(ENV_BUP_FORCE_TTY, "3")
            .env("LC_ALL", "C")
            .arg("split")
            .arg("-t")
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("Failed to start bup process");

        let mut buf = vec![];
        let io2 = bup.stderr.take().unwrap();
        let mut rdr2 = BufReader::new(io2);

        loop {
            buf.clear();

            rdr2.read_until(b'\r', &mut buf)?;

            if buf.is_empty() { break }

            let ln = String::from_utf8_lossy(&buf);

            eprint!("\r{}", ln);
            io::stderr().flush()?;
        }

        // should wait to avoid zombie
        bup.wait()?;

        let mut out = String::new();
        let mut io1 = bup.stdout.take().unwrap();
        io1.read_to_string(&mut out)?;

        let out1 = out.trim();
        assert_eq!(out1.len(), 2 * OID_LEN);

        let mut res = [0;32];
        hex::decode_to_slice(out1, &mut res).unwrap();

        println!("    tree   {} {}", &util::to_zbase32(&res)[..8], out1);

        Ok(res)
    }
}
