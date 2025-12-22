use aho_corasick::{AhoCorasick, MatchKind};

fn main() {

    let patterns = &["Samwise", "Sam"];
    let haystack = "Test Samwise";

    let ac = AhoCorasick::builder()
        .match_kind(MatchKind::LeftmostFirst)
        .build(patterns)
        .unwrap();
    let mat = ac.find(haystack).expect("should have a match");
    assert_eq!("Samwise", &haystack[mat.start()..mat.end()]);

}