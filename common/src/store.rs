use std::{collections::{BTreeMap, BTreeSet}, process::{self, Command, Stdio}};

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

use crate::git;

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

pub type ObjType = git::Type;

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
pub struct CommitResult {
    pub obj_list: Vec<Id>,
    pub oid: Option<Id>,
}

pub fn ref_remote(name: &str) -> String {
    format!("refs/remotes/{}/{}", name, Store::LOCALHOST)
}

pub fn ref_local(name: &str) -> String {
    format!("refs/heads/{}", name)
}

impl Store {
    pub const LOCALHOST: &'static str = "localhost";

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

    pub fn git_show_ref(&self, git_ref: &str) -> Result<Option<Id>> {
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

    pub fn git_show_ref_all(&self) -> Result<BTreeMap<String, Id>> {
        let git = Command::new("git")
            .env(ENV_GIT_DIR, &self.root)
            .arg("show-ref")
            .output()
            .expect("failed to execute git-show-ref");

        let mut res = BTreeMap::new();

        // not exist
        if !git.status.success() { return Ok(res) } // TODO, return error

        for ln in str::from_utf8(&git.stdout).unwrap().lines() {
            let mut i = ln.split(' ');
            let mut id = [0;32];
            hex::decode_to_slice(&i.next().unwrap(), &mut id).unwrap();
            let name = i.next().unwrap();

            res.insert(name.into(), id);
        }

        Ok(res)
    }

    pub fn git_update_ref(&self, git_ref: &str, commit: &Id) -> Result<()> {
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

    fn git_cat_file(&self, tp: &str, oid: &Id) -> Result<Vec<u8>> {
        let git = Command::new("git")
            .env(ENV_GIT_DIR, &self.root)
            .arg("cat-file")
            .arg(tp)
            .arg(&hex::encode(oid))
            .output()
            .expect("failed to execute git-cat-file");

        // not exist
        if !git.status.success() {
            return err("git update-ref fail")
        }

        Ok(git.stdout)
    }

    //
    pub fn read_commit(&self, oid: &Id) -> Result<git::Commit> {
        let raw = self.git_cat_file("commit", oid)?;
        let buf = str::from_utf8(&raw).unwrap();

        let mut slice = &buf[..];
        let mut ln: &str;
        let mut parent: Vec<Id> = vec![];
        let mut tree: Id = [0;32];

        loop {
            let t = slice.find('\n');

            ln = "";
            if let Some(n) = t {
                ln = &slice[..n+1];
                slice = &slice[n+1..];
            }

            let ln1 = ln.trim();

            debug!("read_commit: line = {}", ln1);
            if ln1.starts_with("tree ") {
                tree = util::to_id(&hex::decode(ln1[5..].trim()).unwrap());
                debug!("read_commit: tree = {}", hex::encode(&tree));
            }
            else if ln1.starts_with("parent ") {
                let pid = util::to_id(&hex::decode(ln1[6..].trim()).unwrap());
                debug!("read_commit: parent = {}", hex::encode(&pid));
                parent.push(pid);
            }
            else if ln1.is_empty() {
                break
            }
        }

        Ok(git::Commit {
            parent,
            tree,
            comment: slice.to_string()
        })
    }

    pub fn read_commit_anno(&self, oid: &Id, full: bool) -> Result<Anno> {
        let commit = self.read_commit(oid)?;
        let res = Anno::decode(&commit.parent[..], &commit.tree, &commit.comment, full)?;

        Ok(res)
    }

    pub fn read_tree(&self, oid: &Id) -> Result<git::Tree> {
        let raw = self.git_cat_file("tree", &oid)?;

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

            debug!("mode: raw = {:?}, idx0 {}, idx1 {}", raw, idx0, idx1);
            let mode = str::from_utf8(&raw[idx0..idx1]).unwrap();
            idx0 = idx1 + 1;
            idx1 = idx0;

            if !next_byte_match(0, &mut idx1) { break; }
            debug!("name: raw = {:?}, idx0 {}, idx1 {}", raw, idx0, idx1);
            let name = str::from_utf8(&raw[idx0 .. idx1]).unwrap();

            idx0 = idx1 + 1;
            idx1 = idx0 + OID_LEN;

            let id = util::to_id(&raw[idx0 .. idx1]);

            let mode_i = i32::from_str_radix(&mode, 8).unwrap();

            res.push(git::TreeEntry { mode: ObjType::from_mode(mode_i),
                                      name: name.to_string(),
                                      oid: id });

            idx0 = idx1;
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

        self.commit_tree(&anno.pid, &anno.fid, time, msg)
    }

    pub fn commit(&mut self, manifest: &mut Manifest) -> Result<CommitResult> {
        self.update_time()?;

        let mut res = CommitResult { obj_list: vec![], oid: None };

        for (name, anno) in manifest.anno_map.iter_mut() {
            println!("--> {}", name);

            //
            let func = |st: St, an: &mut Anno| -> Result<()> {
                if st == St::MFile {
                    an.fid = self.import(an.get_file_path()
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

        // update localhost cset tip
        let parent = match self.git_show_ref(&ref_remote(Self::LOCALHOST))? {
            Some(id) => vec![id],
            None => vec![],
        };

        let cset_commit = self.commit_tree(&parent, &tree,
                                     self.date, "")?;

        self.git_update_ref(&ref_remote(Self::LOCALHOST), &cset_commit)?;

        println!("commit {} {}",
                 &util::to_zbase32(&cset_commit)[..8],
                 &hex::encode(&cset_commit));

        res.oid = Some(cset_commit);


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

    // walk from ref_remote(changeset) to ref_local(changeset)
    pub fn walk_cset(&self, changeset: &str) -> Result<Vec<(Id, BTreeSet<Id>)>> {
        let from = self.git_show_ref(&ref_remote(changeset))?.unwrap();
        let to_opt = self.git_show_ref(&ref_local(changeset))?;

        debug!("walk_cset: {}, from {} to {}",
               &changeset, &hex::encode(&from[..5]),
               &to_opt.map_or("none".to_string(),
                              |x| hex::encode(&x[..5])));

        let mut res = vec![];
        let mut remain = vec![from];

        while !remain.is_empty() {
            let id = remain.pop().unwrap();

            if let Some(to) = to_opt {
                if to == id {
                    debug!("walk_cset: stop at {}", &hex::encode(&id)[..10]);
                    continue
                }
            }

            // this branch done
            let commit = self.read_commit(&id)?;

            debug!("walk_cset: commit = {:?}", commit);

            let tid = commit.tree;
            let parent = commit.parent;

            let tree = self.read_tree(&tid)?.into_iter()
                .filter(|te|
                        if let ObjType::Commit = te.mode { true }
                        else {false})
                .map(|te| te.oid)
                .collect();


            res.push((id.clone(), tree));

            for x in parent.into_iter() {
                remain.push(x)
            }
        }

        Ok(res)
    }
}

#[cfg(test)]
mod test {
    // TODO, test walk
}
