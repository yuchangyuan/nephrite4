
use crate::proj::*;
use std::fs;
use std::env;
use std::io;

use glob::glob;

use crate::util::Id;

use std::collections::BTreeMap;

#[derive(Debug)]
pub struct Manifest {
    pub mdir: String,
    pub anno_map: BTreeMap<Id, anno::Anno>
}

impl Manifest {
    // TODO: create lock
    pub fn new(mdir: &str) -> io::Result<Manifest> {
        let mut res = Manifest {
            mdir: mdir.to_string(),
            anno_map: BTreeMap::new()
        };

        fs::create_dir_all(mdir)?;

        // NOTE: here to ensure path is relative to correct base path
        let cwd = env::current_dir()?;
        env::set_current_dir(&mdir)?;
        let paths = glob(&"**/*.meta").unwrap().
            filter_map(|x| x.ok()).collect::<Vec<_>>();
        env::set_current_dir(&cwd)?;

        for path in paths {
            match path.into_os_string().into_string() {
                Ok(mut p) => {
                    let l = p.len();
                    p.truncate(l - 5);
                    // println!("p = {}", p);
                    match anno::Anno::new(mdir, &p, true) {
                        Ok(anno) => {
                            //let g = anno.gen().unwrap();
                            //println!("{:?}", g);
                            //println!("{:?}", to_zbase32(&calc_id_buf(&g)));

                            res.anno_map.insert(anno.get_oid(), anno);
                        },
                        Err(e) => {
                            println!("Error load {}, {:?}", &p, e)
                        }
                    }
                },
                _ => ()
            }
        }

        Ok(res)
    }

    pub fn find_file(&self, name: &str) -> Option<Id> {
        // NOTE: this is slow
        for (k, v) in self.anno_map.iter() {
            match v.get_name() {
                Some(n) => if n == name {
                    return Some(k.clone())
                },
                _ => ()
            }
        }

        return None
    }

    pub fn add(&mut self, rpath: &str) -> io::Result<Id> {
        let anno = anno::Anno::new(&self.mdir, rpath, false)?;

        // TODO, should avoid dup
        let oid = anno.get_oid();
        self.anno_map.insert(oid, anno);

        Ok(oid)
    }

    pub fn update(&mut self) -> io::Result<Vec<Id>> {
        let mut res: Vec<Id> = vec![];

        let ids: Vec<Id> = self.anno_map.keys().map(|x| x.clone()).collect();

        // NOTE: get_oid() return might change after update
        for i in ids {
            let mut i1 = i.clone();
            let mut anno = self.anno_map.remove(&i).unwrap();
            match anno.update() {
                Err(e) => {
                    println!("up fail: {:?}", e);
                },
                Ok(true) => {
                    i1 = anno.get_oid();
                    res.push(i1.clone());
                },
                _ => ()
            }

            self.anno_map.insert(i1.clone(), anno);
        }

        Ok(res)
    }
}
