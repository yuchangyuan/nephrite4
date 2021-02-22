use notmecab::{Blob, Dict};
use zstd;

fn main() {

    // you need to acquire a mecab dictionary and place these files here manually
    let a = std::include_bytes!("../index/ipadic-utf8/sys.dic.zst");
    let sysdic = Blob::new(zstd::stream::decode_all(&a[..]).unwrap());

    let a = std::include_bytes!("../index/ipadic-utf8/unk.dic.zst");
    let unkdic = Blob::new(zstd::stream::decode_all(&a[..]).unwrap());

    let a = std::include_bytes!("../index/ipadic-utf8/matrix.bin.zst");
    let matrix = Blob::new(zstd::stream::decode_all(&a[..]).unwrap());

    let a = std::include_bytes!("../index/ipadic-utf8/char.bin.zst");
    let unkdef = Blob::new(zstd::stream::decode_all(&a[..]).unwrap());

    println!("init...");
    let dict = Dict::load(sysdic, unkdic, matrix, unkdef).unwrap();
    println!("load done...");

    let input = "太郎は次郎が持っている本を花子に渡した。";
    println!("input: {}", input);

    let (toks, res) = dict.tokenize(input).unwrap();

    println!("res = {}, toks = {:?}", res, toks);

    for tok in toks {
        println!("{}: cost {}, real cost {}, range {:?}, kind {:?}, original_id {}, feature_offset {}",
                 &input[tok.range.clone()],
                 tok.cost, tok.real_cost, tok.range,
                 tok.kind, tok.original_id, tok.feature_offset);
    }
}
