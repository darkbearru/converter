use converter_core::dsl::headwords::{display_headword, expand_headwords};

#[test]
fn разворачивает_варианты_без_потери_дубликатов() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(expand_headwords("colo(u)r", 64)?, ["color", "colour"]);
    assert_eq!(expand_headwords("{the }colo(u)r", 64)?, ["color", "colour"]);
    assert_eq!(display_headword("{the }colo(u)r")?, "the colour");
    assert_eq!(expand_headwords("a(b(c))", 64)?, ["a", "ab", "abc"]);
    assert_eq!(expand_headwords(r"word\(s\)\{x\}", 64)?, ["word(s){x}"]);
    assert_eq!(
        expand_headwords("a(a)(b)(c)(d)(e)(f)(g)", 64)
            .expect_err("Больше 64 вариантов")
            .code,
        "headword_variants_limit"
    );
    assert_eq!(
        expand_headwords("word(unclosed", 64)
            .expect_err("Не закрыта скобка")
            .code,
        "invalid_headword"
    );
    Ok(())
}
