use notmecab::{Blob, Dict};

fn main() {

    // you need to acquire a mecab dictionary and place these files here manually
    let sysdic = Blob::new(std::include_bytes!("../index/ipadic-utf8/sys.dic"));
    let unkdic = Blob::new(std::include_bytes!("../index/ipadic-utf8/unk.dic"));
    let matrix = Blob::new(std::include_bytes!("../index/ipadic-utf8/matrix.bin"));
    let unkdef = Blob::new(std::include_bytes!("../index/ipadic-utf8/char.bin"));

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
