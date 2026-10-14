//! Folding speed: own tables vs the unicode-normalization crate, on non-ASCII names.
use unicode_normalization::UnicodeNormalization;
fn main() {
    let names: Vec<String> = (0..200_000)
        .map(|i| match i % 6 {
            0 => format!("Résumé final {i}.pdf"),
            1 => format!("日本語メモ {i}.txt"),
            2 => format!("naïve café {i}"),
            3 => format!("한국어 문서 {i}.docx"),
            4 => format!("हिंदी नोट्स {i}"),
            _ => format!("Ｒｅｐｏｒｔ ΣΊΣΥΦΟΣ {i}"),
        })
        .collect();
    let mut out = Vec::new();
    for _ in 0..2 {
        let t = std::time::Instant::now();
        for n in &names {
            out.clear();
            ef_index::fold::fold_into(n.as_bytes(), &mut out);
        }
        let own = t.elapsed();
        let t = std::time::Instant::now();
        let mut total = 0;
        for n in &names {
            let s: String = n.nfkd().flat_map(char::to_lowercase).collect();
            total += s.len();
        }
        let reference = t.elapsed();
        println!("own tables {own:?}   unicode-normalization {reference:?}  ({total})");
    }
}
