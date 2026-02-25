/*!
Предоставляет процедуры для генерации «короткой» и «длинной» документации
помощи ripgrep.

Короткая версия используется, когда дан флаг `-h`, а длинная версия
используется, когда дан флаг `--help`.
*/

use std::{collections::BTreeMap, fmt::Write};

use crate::flags::{Category, Flag, defs::FLAGS, doc::version};

const TEMPLATE_SHORT: &'static str = include_str!("template.short.help");
const TEMPLATE_LONG: &'static str = include_str!("template.long.help");

/// Оборачивает `std::write!` и утверждает, что нет ошибки.
///
/// Мы пишем только в `String` в этом модуле.
macro_rules! write {
    ($($tt:tt)*) => { std::write!($($tt)*).unwrap(); }
}

/// Генерирует короткую документацию, т.е., для `-h`.
pub(crate) fn generate_short() -> String {
    let mut cats: BTreeMap<Category, (Vec<String>, Vec<String>)> =
        BTreeMap::new();
    let (mut maxcol1, mut maxcol2) = (0, 0);
    for flag in FLAGS.iter().copied() {
        let columns =
            cats.entry(flag.doc_category()).or_insert((vec![], vec![]));
        let (col1, col2) = generate_short_flag(flag);
        maxcol1 = maxcol1.max(col1.len());
        maxcol2 = maxcol2.max(col2.len());
        columns.0.push(col1);
        columns.1.push(col2);
    }
    let mut out =
        TEMPLATE_SHORT.replace("!!VERSION!!", &version::generate_digits());
    for (cat, (col1, col2)) in cats.iter() {
        let var = format!("!!{name}!!", name = cat.as_str());
        let val = format_short_columns(col1, col2, maxcol1, maxcol2);
        out = out.replace(&var, &val);
    }
    out
}

/// Генерирует короткую документацию для одного флага.
///
/// Первый элемент соответствует имени флага, а второй элемент соответствует
/// строке документации.
fn generate_short_flag(flag: &dyn Flag) -> (String, String) {
    let (mut col1, mut col2) = (String::new(), String::new());

    // Некоторые имена переменных подходят для более длинной формы
    // документации, но они делают краткую помощь очень зашумленной.
    // Поэтому просто укоротим некоторые из них.
    let var = flag.doc_variable().map(|s| {
        let mut s = s.to_string();
        s = s.replace("SEPARATOR", "SEP");
        s = s.replace("REPLACEMENT", "TEXT");
        s = s.replace("NUM+SUFFIX?", "NUM");
        s
    });

    // Generate the first column, the flag name.
    if let Some(byte) = flag.name_short() {
        let name = char::from(byte);
        write!(col1, r"-{name}");
        write!(col1, r", ");
    }
    write!(col1, r"--{name}", name = flag.name_long());
    if let Some(var) = var.as_ref() {
        write!(col1, r"={var}");
    }

    // И теперь второй столбец с описанием.
    write!(col2, "{}", flag.doc_short());

    (col1, col2)
}

/// Пишет два столбца документации.
///
/// `maxcol1` должен быть максимальной длиной (в байтах) первого столбца,
/// а `maxcol2` должен быть максимальной длиной (в байтах) второго столбца.
fn format_short_columns(
    col1: &[String],
    col2: &[String],
    maxcol1: usize,
    _maxcol2: usize,
) -> String {
    assert_eq!(col1.len(), col2.len(), "columns must have equal length");
    const PAD: usize = 2;
    let mut out = String::new();
    for (i, (c1, c2)) in col1.iter().zip(col2.iter()).enumerate() {
        if i > 0 {
            write!(out, "\n");
        }

        let pad = maxcol1 - c1.len() + PAD;
        write!(out, "  ");
        write!(out, "{c1}");
        write!(out, "{}", " ".repeat(pad));
        write!(out, "{c2}");
    }
    out
}

/// Генерирует длинную документацию, т.е., для `--help`.
pub(crate) fn generate_long() -> String {
    let mut cats = BTreeMap::new();
    for flag in FLAGS.iter().copied() {
        let mut cat = cats.entry(flag.doc_category()).or_insert(String::new());
        if !cat.is_empty() {
            write!(cat, "\n\n");
        }
        generate_long_flag(flag, &mut cat);
    }

    let mut out =
        TEMPLATE_LONG.replace("!!VERSION!!", &version::generate_digits());
    for (cat, value) in cats.iter() {
        let var = format!("!!{name}!!", name = cat.as_str());
        out = out.replace(&var, value);
    }
    out
}

/// Пишет сгенерированную документацию для `flag` в `out`.
fn generate_long_flag(flag: &dyn Flag, out: &mut String) {
    if let Some(byte) = flag.name_short() {
        let name = char::from(byte);
        write!(out, r"    -{name}");
        if let Some(var) = flag.doc_variable() {
            write!(out, r" {var}");
        }
        write!(out, r", ");
    } else {
        write!(out, r"    ");
    }

    let name = flag.name_long();
    write!(out, r"--{name}");
    if let Some(var) = flag.doc_variable() {
        write!(out, r"={var}");
    }
    write!(out, "\n");

    let doc = flag.doc_long().trim();
    let doc = super::render_custom_markup(doc, "flag", |name, out| {
        let Some(flag) = crate::flags::parse::lookup(name) else {
            unreachable!(r"found unrecognized \flag{{{name}}} in --help docs")
        };
        if let Some(name) = flag.name_short() {
            write!(out, r"-{}/", char::from(name));
        }
        write!(out, r"--{}", flag.name_long());
    });
    let doc = super::render_custom_markup(&doc, "flag-negate", |name, out| {
        let Some(flag) = crate::flags::parse::lookup(name) else {
            unreachable!(
                r"found unrecognized \flag-negate{{{name}}} in --help docs"
            )
        };
        let Some(name) = flag.name_negated() else {
            let long = flag.name_long();
            unreachable!(
                "found \\flag-negate{{{long}}} in --help docs but \
                 {long} does not have a negation"
            );
        };
        write!(out, r"--{name}");
    });

    let mut cleaned = remove_roff(&doc);
    if let Some(negated) = flag.name_negated() {
        // Флаги, которые могут быть отрицательными и не являются переключателями,
        // такие как --context-separator, несколько странные. Из-за этого
        // документация для этих флагов должна явно обсуждать семантику
        // отрицания. Но для переключателей поведение всегда одинаково.
        if flag.is_switch() {
            write!(cleaned, "\n\nЭтот флаг может быть отключен с помощью --{negated}.");
        }
    }
    let indent = " ".repeat(8);
    let wrapopts = textwrap::Options::new(71)
        // Обычно я не против разрыва на дефисах, но документация ripgrep
        // включает много имен флагов, и они, в свою очередь, содержат дефисы.
        // Разрыв имен флагов по строкам не очень хорош.
        .word_splitter(textwrap::WordSplitter::NoHyphenation);
    for (i, paragraph) in cleaned.split("\n\n").enumerate() {
        if i > 0 {
            write!(out, "\n\n");
        }
        let mut new = paragraph.to_string();
        if paragraph.lines().all(|line| line.starts_with("    ")) {
            // Переиндентируем, но не заполняем заново, чтобы сохранить
            // разрывы строк в примерах кода/оболочки.
            new = textwrap::indent(&new, &indent);
        } else {
            new = new.replace("\n", " ");
            new = textwrap::refill(&new, &wrapopts);
            new = textwrap::indent(&new, &indent);
        }
        write!(out, "{}", new.trim_end());
    }
}

/// Удаляет синтаксис roff из `v` так, чтобы результат был приблизительно
/// читаемым простым текстом.
///
/// Это в основном смесь эвристик, основанных на конкретном roff,
/// используемом в документации для флагов в этом инструменте. Если новые
/// виды roff используются в документации, то это может потребовать
/// обновления для их обработки.
fn remove_roff(v: &str) -> String {
    let mut lines = vec![];
    for line in v.trim().lines() {
        assert!(!line.is_empty(), "roff не должен иметь пустых строк");
        if line.starts_with(".") {
            if line.starts_with(".IP ") {
                let item_label = line
                    .split(" ")
                    .nth(1)
                    .expect("первый аргумент для .IP")
                    .replace(r"\(bu", r"•")
                    .replace(r"\fB", "")
                    .replace(r"\fP", ":");
                lines.push(format!("{item_label}"));
            } else if line.starts_with(".IB ") || line.starts_with(".BI ") {
                let pieces = line
                    .split_whitespace()
                    .skip(1)
                    .collect::<Vec<_>>()
                    .concat();
                lines.push(format!("{pieces}"));
            } else if line.starts_with(".sp")
                || line.starts_with(".PP")
                || line.starts_with(".TP")
            {
                lines.push("".to_string());
            }
        } else if line.starts_with(r"\fB") && line.ends_with(r"\fP") {
            let line = line.replace(r"\fB", "").replace(r"\fP", "");
            lines.push(format!("{line}:"));
        } else {
            lines.push(line.to_string());
        }
    }
    // Сжимаем множественные смежные разрывы абзацев в один.
    lines.dedup_by(|l1, l2| l1.is_empty() && l2.is_empty());
    lines
        .join("\n")
        .replace(r"\fB", "")
        .replace(r"\fI", "")
        .replace(r"\fP", "")
        .replace(r"\-", "-")
        .replace(r"\\", r"\")
}
