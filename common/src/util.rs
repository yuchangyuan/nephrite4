use std::fs::File;
use std::path::Path;

use crypto::sha2::Sha256;
use crypto::digest::Digest;

use std::io;
use std::io::Read;

use serde_json;

#[cfg(test)]
use serde_json::json;

pub type Id = [u8;32];

pub fn calc_id(file: &str) -> io::Result<Id> {
    //Err("ic calc fail".to_string())
    let mut res = [0u8; 32];
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 1024 * 1024]; // 8K buf
    let mut fh = File::open(Path::new(file))?;

    loop {
        let sz = fh.read(&mut buf)?;
        hasher.input(&mut buf[0..sz]);
        if sz < buf.len() {
            break;
        }
    }

    hasher.result(&mut res);

    Ok(res)
}

pub fn calc_id_buf(buf: &[u8]) -> Id {
    let mut hasher = Sha256::new();
    let mut res = [0u8; 32];

    hasher.input(buf);
    hasher.result(&mut res);

    res
}

// "abcdefghijklmnopqrstuvwxyz234567" base32
/*
const base32_index: [u8; 32] =
    [ 97, 98, 99,100,101,102,103,104,
      105,106,107,108,109,110,111,112,
      113,114,115,116,117,118,119,120,
      121,122, 50, 51, 52, 53, 54, 55];
*/

// "ybndrfg8ejkmcpqxot1uwisza345h769" zbase32
const ZBASE32_INDEX: [u8; 32] =
    [121, 98,110,100,114,102,103, 56,
     101,106,107,109, 99,112,113,120,
     111,116, 49,117,119,105,115,122,
     97, 51, 52, 53,104, 55, 54, 57];

/*
z = map ord "ybndrfg8ejkmcpqxot1uwisza345h769"
map (\x -> let r = length $ takeWhile (/= x) z in if r == 32 then 0 else r) [0..255]
*/
const ZBASE32_INDEX_R: [u8; 256] =
    [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
     0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
     18,0,25,26,27,30,29,7,31,0,0,0,0,0,0,0,0,0,
     0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
     24,1,12,3,8,5,6,28,21,9,10,0,11,2,16,13,14,4,22,17,19,0,20,15,0,23,
     0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
     0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
     0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,
     0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0];

pub fn to_zbase32(dat: &[u8]) -> String {
    let mut res: Vec<u8> = vec![];
    let mut len = 0;
    let mut val = 0;

    for d in dat {
        //  xxxx     yyyyyyyy
        // val/len    d / 8
        let mut i = (val << (5 - len)) + (d >> (8 - (5 - len))) as i32;
        res.push(ZBASE32_INDEX[i as usize]);
        len = 8 - (5 - len);
        val = (*d as i32) & ((1 << len) - 1);
        // println!("{:02x}, {}", val, len);
        if len >= 5 {
            i = val >> (len - 5);
            res.push(ZBASE32_INDEX[i as usize]);
            len -= 5;
            val &= (1 << len) - 1;
        }
    }

    // no padding, left align
    if len > 0 {
        let i = val << (5 - len);
        res.push(ZBASE32_INDEX[i as usize]);
    }

    String::from_utf8(res).unwrap()
}

pub fn from_zbase32(s: &str) -> Vec<u8> {
    let mut res: Vec<u8> = vec![];
    let mut len = 0;
    let mut val = 0u32;

    for d in s.as_bytes() {
        len += 5;
        val = (val << 5) + ZBASE32_INDEX_R[*d as usize] as u32;

        if len >= 8 {
            len -= 8;
            res.push((val >> len) as u8);
            val &= (1 << len) - 1;
        }
    }

    if len > 0 {
        res.push((val << (8 - len)) as u8)
    }

    res
}

pub fn zbase32_to_id(s: &str) -> Id {
    let mut id = [0u8;32];

    for (i, d) in from_zbase32(s).iter().enumerate() {
        if i < 32 { id[i] = *d; }
    }

    id
}

use serde_json::Value;

pub fn json_do_map_str<F>(v: &mut serde_json::Value, f: &F)
    where F: Fn(&str) -> String
{
    let is_str = v.is_string();

    if is_str {
        *v = Value::String(f(v.as_str().unwrap()));
        return
    }

    match v {
        &mut Value::Array(ref mut x) => {
            for mut i in x.iter_mut() {
                json_do_map_str(&mut i, f);
            }
        },
        &mut Value::Object(ref mut x) => {
            for (_, mut i) in x.iter_mut() {
                json_do_map_str(&mut i, f);
            }
        }
        _ => ()
    }
}

pub fn to_id(slice: &[u8]) -> Id {
    let mut res: Id = [0u8;32];

    for (i, b) in slice.iter().enumerate() {
        if i < 32 { res[i] = *b }
    }

    res
}


#[test]
fn test_to_zbase32()
{
    assert_eq!(to_zbase32(&[245, 87, 189, 12]), "6im54dy");
    assert_eq!(to_zbase32(&[0x10, 0x11, 0x10]), "nyety");

    assert_eq!(
        to_zbase32(&[104,101,108,108,111,44,32,119,111,114,108,100,10]),
        "pb1sa5dxfoo8q551pt1yw");
}

#[test]
fn test_from_zbase32()
{
    assert_eq!(from_zbase32("6im54d"), &[245, 87, 189, 12]);
    // NOTE: need trail zero
    assert_eq!(from_zbase32("nyety"), &[0x10, 0x11, 0x10, 0]);
    assert_eq!(
        from_zbase32("pb1sa5dxfoo8q551pt1yw"),
        &[104,101,108,108,111,44,32,119,111,114,108,100,10, 0]);
}

#[test]
fn test_json_do_map_str()
{
    let mut js = json!({"a": "z1z2", "b": "zzqq"});
    json_do_map_str(&mut js, &|s| s.replace("z", "##"));
    assert_eq!(js, json!({"a": "##1##2", "b" : "####qq"}));

    let mut js1: serde_json::Value =
        serde_json::from_str("{\"a\": \"a\\u0000b\"}").unwrap();
    assert_eq!(js1, json!({"a": "a\0b"}));
}
