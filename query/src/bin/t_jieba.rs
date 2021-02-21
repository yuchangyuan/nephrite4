use jieba_rs::Jieba;

fn main() {
    println!("init");
    let jieba = Jieba::new();
    println!("done");
    let words = jieba.cut("我们中出了一个叛徒", false);

    println!("words = {:?}", words);
    assert_eq!(words, vec!["我们", "中", "出", "了", "一个", "叛徒"]);
}
