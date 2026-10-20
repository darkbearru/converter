use converter_core::dsl::headwords::{display_headword, expand_headwords};

#[test]
fn скобки_в_несортируемом_тексте_сохраняются_только_для_показа()
-> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(expand_headwords("item{(}s{)}", 64)?, ["items"]);
    assert_eq!(display_headword("item{(}s{)}")?, "item(s)");
    assert_eq!(
        expand_headwords("{(note) }colo(u)r", 64)?,
        ["color", "colour"]
    );
    assert_eq!(display_headword("{(note) }colo(u)r")?, "(note) colo(u)r");
    assert_eq!(expand_headwords("word({(}s{)})", 64)?, ["word", "words"]);
    Ok(())
}

#[test]
fn несортируемая_пунктуация_не_отменяет_проверку_структуры_и_лимитов() {
    for raw in ["word{(", "word}", "word(unclosed", "word)", "word{note\\"] {
        assert_eq!(
            expand_headwords(raw, 64)
                .expect_err("Повреждённая структура заголовка")
                .code,
            "invalid_headword"
        );
    }
    assert_eq!(
        expand_headwords("{(}a(a)(b)(c)(d)(e)(f)(g){)}", 64)
            .expect_err("Лимит поисковых вариантов")
            .code,
        "headword_variants_limit"
    );
    let nested = format!("{}word{}", "{".repeat(130), "}".repeat(130));
    assert_eq!(
        expand_headwords(&nested, 64)
            .expect_err("Лимит вложенности")
            .code,
        "invalid_headword"
    );
}

#[test]
fn разворачивает_варианты_без_потери_дубликатов() -> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(expand_headwords("colo(u)r", 64)?, ["color", "colour"]);
    assert_eq!(expand_headwords("{the }colo(u)r", 64)?, ["color", "colour"]);
    assert_eq!(display_headword("{the }colo(u)r")?, "the colo(u)r");
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
