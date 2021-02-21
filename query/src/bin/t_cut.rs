use nephrite4_query::index::cut;

fn t(s: &str) {
    let res = cut::cut_ln(s);

    println!("----> {}", s);

    for (w, i) in res {
        println!("{} {}", w, i);
    }
}

fn main() {
    t("太郎は次郎が持っている本を花子に渡した。");
    t("我们中出了一个叛徒。从前有座山，山上有座庙，庙里有个和尚。");
    t("This brown fox is looking for a lazy dog to jump over.");
}
