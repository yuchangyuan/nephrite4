
use crate::proj::*;
use std::fs;
use std::env;
use std::io;

use glob::glob;

//use crate::util::Id;

use std::collections::BTreeMap;

#[derive(Debug)]
pub struct Manifest {
    pub mdir: String,
    pub anno_map: BTreeMap<String, anno::Anno>
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
                    match anno::Anno::new(mdir, &p, false) {
                        Ok(anno) => {
                            //let g = anno.gen().unwrap();
                            //println!("{:?}", g);
                            //println!("{:?}", to_zbase32(&calc_id_buf(&g)));
                            let name = p.to_string();
                            res.anno_map.insert(name, anno);
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


    pub fn add(&mut self, rpath: &str) -> io::Result<String> {
        let anno = anno::Anno::new(&self.mdir, rpath, false)?;
        let name = rpath.to_string();
        self.anno_map.insert(name.clone(), anno);

        Ok(name)
    }
}
