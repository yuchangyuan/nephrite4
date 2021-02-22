use crate::db::types::TsVector;

use serde_json;

use std::collections::{BTreeMap, BTreeSet};

use lazy_static::lazy_static;

use jieba_rs::Jieba;
use jieba_rs::TokenizeMode;

use rust_stemmers::{Algorithm, Stemmer};

use notmecab::{Blob, Dict};

use whatlang;
use whatlang::{Info, Lang};

use log::debug;
use substring::Substring;

use zstd;

const PUNCTUATION: [char;42] = [
    '.', ',', '"', '\'', '?', '!', ':', ';',
    '(', ')', '[', ']', '{', '}', '\\',
    '+', '-', '*', '/', '|','、',
    '。', '，', '”', '“', '？', '！', '：', '；',
    '$', '?','_', '“','”',
    '（', '）', '·', '「', '」',
    '～',
    '《', '》'];

lazy_static! {
    static ref PUNCTUATION_SET: BTreeSet<char> = {
        PUNCTUATION.iter().cloned().collect()
    };

    static ref STOP_WORD_SET: BTreeSet<String> = {
        let mut res: BTreeSet<String> =
            PUNCTUATION.iter().map(|c| c.to_string()).collect();

        std::include_str!("stop_word_en.txt")
            .lines().for_each(|l| { res.insert(l.trim().to_string()); });

        std::include_str!("stop_word_cn.txt")
            .lines().for_each(|l| { res.insert(l.trim().to_string()); });

        std::include_str!("stop_word_ja.txt")
            .lines().for_each(|l| { res.insert(l.trim().to_string()); });

        ['0','1','2','3','4','5','6','7','8','9']
            .iter()
            .for_each(|s| {res.insert(s.to_string());});

        res
    };

    static ref JIEBA: Jieba = Jieba::new();
    static ref STEMMER_EN: Stemmer = Stemmer::create(Algorithm::English);
    static ref NOTMECAB_DICT: Dict = {
        let a = std::include_bytes!("ipadic-utf8/sys.dic.zst");
        let sysdic = Blob::new(zstd::stream::decode_all(&a[..]).unwrap());

        let a = std::include_bytes!("ipadic-utf8/unk.dic.zst");
        let unkdic = Blob::new(zstd::stream::decode_all(&a[..]).unwrap());

        let a = std::include_bytes!("ipadic-utf8/matrix.bin.zst");
        let matrix = Blob::new(zstd::stream::decode_all(&a[..]).unwrap());

        let a = std::include_bytes!("ipadic-utf8/char.bin.zst");
        let unkdef = Blob::new(zstd::stream::decode_all(&a[..]).unwrap());

        Dict::load(sysdic, unkdic, matrix, unkdef).unwrap()
    };
}

// TODO, here usage of substring is not optimize
fn split_non_latin_words(ln: &str) -> Vec<String> {
    let mut words = vec![];
    let mut word = vec![];
    let mut test_latin = true;

    for c in ln.chars() {
        word.push(c);

        if !(test_latin && c.is_alphabetic()) {
            test_latin = false;

            if PUNCTUATION_SET.contains(&c) {
                let word_s: String = word.into_iter().collect();
                words.push(word_s);

                word = vec![];
                test_latin = true
            }
        }
    }

    if !word.is_empty() {
        let word_s: String = word.into_iter().collect();
        words.push(word_s);
    }

    return words;
}

fn cut_cn_en(off: usize, ln: &str) -> Vec<(String, usize)> {
    let mut res = vec![];

    for tk in JIEBA.tokenize(ln, TokenizeMode::Search, false /* hmm */) {
        let mut word = tk.word.trim().to_lowercase();

        //println!("off: {}, word: {}", off, word);

        if word.len() == 0 { continue; }
        if STOP_WORD_SET.contains(&word) { continue; }

        word = STEMMER_EN.stem(&word).to_string();

        // convert char offset to byte offset
        let start1 = ln.substring(0, tk.start).len();
        res.push((word, start1 + off));
    }

    res
}

fn cut_ja(off: usize, ln: &str) -> Vec<(String, usize)> {
    let mut res = vec![];

    if let Ok((toks, _)) = NOTMECAB_DICT.tokenize(ln) {
        for tok in toks {
            // here start in byte
            let start = tok.range.start;
            let word = &ln[tok.range];

            if STOP_WORD_SET.contains(word) { continue; }

            res.push((word.to_string(), off + start));
        }
    }

    res
}

pub fn cut_ln(ln: &str) -> Vec<(String, usize)> {
    let words = split_non_latin_words(ln);

    debug!("words({}) = {:?}", words.len(), words);

    let mut res = vec![];
    let mut offset = 0;

    for word in words {
        let mut is_ja = false;

        if let Some(info) = whatlang::detect(&word) {
            if info.lang() == Lang::Jpn { is_ja = true }
        }

        if is_ja {
            res.append(&mut cut_ja(offset, &word));
        }
        else {
            res.append(&mut cut_cn_en(offset, &word));
        }

        offset += word.len();
    }

    res
}

pub fn cut(_mt: &str, c: &str) -> Vec<(u64, TsVector)> {
    // NOTE: currently, not split doc
    // should split for large document
    let mut rel = 0;
    let mut res: Vec<(u64, TsVector)> = vec![];

    let l1 = c.replace(|c| c == '\r' || c == '\n' || c == '\0', " ");

    let data: Vec<(String, u64)> =
        cut_ln(&l1).into_iter().map(|(a,b)| (a, b as u64)).collect();

    let mut d1: &mut BTreeMap<String, BTreeSet<(u8, u16)>> = &mut BTreeMap::new();

    for (v, p) in data.into_iter() {
        if p >= rel + 16384 {
            res.push((rel, TsVector { data: d1.clone() }));
            rel += 16384;
            d1.clear();
        }

        let mut slen = 0;
        {
            let mut s = d1.entry(v).or_insert(BTreeSet::new());
            s.insert((0u8, 1 + p as u16));
            slen = s.len();
        }

        // remain 1 cap
        if slen >= 254 {
            res.push((rel, TsVector { data: d1.clone() }));
            d1.clear()
        }
    }

    //println!("d1 = {:?}", d1);
    if d1.len() > 0 {
        res.push((rel, TsVector { data: d1.clone() }));
    }

    res
}

#[cfg(test)]
fn t_cut_ln_cn_(s: &str) {
    let res = cut_ln(s);

    //println!("----> {}", s);

    for (w, i) in res {
        //println!("{} {}", w, i);
        assert_eq!(&s[i .. i+w.len()], w);
    }
}

#[test]
fn t_cut_ln_cn()
{
    t_cut_ln_cn_("太郎は次郎が持っている本を花子に渡した。");
    t_cut_ln_cn_("我们中出了一个好人。从前有座山，山上有座庙，庙里有个和尚。");
    //t_cut_ln_("This brown fox is looking for a lazy dog to jump over.");
}
