const CONVERTER_CREDIT: &str =
    "Конвертировано с помощью Converter — https://darkbearru.github.io/converter/";
const CONVERTER_CREDIT_EN: &str =
    "Converted with Converter — https://darkbearru.github.io/converter/";

struct Section<'a> {
    language: Option<&'a str>,
    lines: Vec<&'a str>,
}

fn language(line: &str) -> Option<&str> {
    let (name, value) = line.split_once(char::is_whitespace)?;
    if name != "#LANGUAGE" {
        return None;
    }
    value.trim().strip_prefix('"')?.strip_suffix('"')
}

fn standalone_part(line: &str) -> bool {
    let words: Vec<_> = line.split_whitespace().collect();
    let [label, part, separator, total] = words.as_slice() else {
        return false;
    };
    ((*label == "Часть" && *separator == "из") || (*label == "Part" && *separator == "of"))
        && part.parse::<usize>().is_ok_and(|part| part > 0)
        && total.parse::<usize>().is_ok_and(|total| total > 1)
}

fn credit(line: &str) -> Option<&'static str> {
    let lower = line.trim_start().to_lowercase();
    if ["конвертировано ", "сконвертировано ", "конвертация:"]
        .iter()
        .any(|prefix| lower.starts_with(prefix))
    {
        Some(CONVERTER_CREDIT)
    } else if [
        "converted by ",
        "converted with ",
        "converted using ",
        "converted via ",
        "conversion by ",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix))
    {
        Some(CONVERTER_CREDIT_EN)
    } else {
        None
    }
}

fn section_text(section: &Section<'_>, merged: bool, add_credit: bool) -> String {
    let section_credit = section.language.map(|language| {
        if language == "Russian" {
            CONVERTER_CREDIT
        } else {
            CONVERTER_CREDIT_EN
        }
    });
    let mut lines = Vec::new();
    let mut has_credit = false;
    for line in &section.lines {
        if merged && standalone_part(line) {
            continue;
        }
        if let Some(original_credit) = credit(line) {
            if !has_credit {
                lines.push(section_credit.unwrap_or(original_credit));
                has_credit = true;
            }
        } else {
            lines.push(*line);
        }
    }
    while lines.last().is_some_and(|line| line.trim().is_empty()) {
        lines.pop();
    }
    if add_credit && !has_credit {
        lines.push(section_credit.unwrap_or(CONVERTER_CREDIT));
    }
    let body = lines.join("\n");
    if let Some(language) = section.language {
        format!("{language}:\n{body}")
    } else {
        body
    }
}

pub(super) fn description(annotation: &str, merged: bool) -> String {
    let mut sections = vec![Section {
        language: None,
        lines: Vec::new(),
    }];
    for line in annotation.lines() {
        if let Some(language) = language(line) {
            sections.push(Section {
                language: Some(language),
                lines: Vec::new(),
            });
        } else if let Some(section) = sections.last_mut() {
            section.lines.push(line);
        }
    }
    let multilingual = sections.len() > 1;
    sections
        .iter()
        .map(|section| section_text(section, merged, !multilingual || section.language.is_some()))
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}
