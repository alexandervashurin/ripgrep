/*!
Предоставляет процедуры для генерации страницы руководства ripgrep в формате `roff`.
*/

use std::{collections::BTreeMap, fmt::Write};

use crate::flags::{Flag, defs::FLAGS, doc::version};

const TEMPLATE: &'static str = include_str!("template.rg.1");

/// Оборачивает `std::write!` и утверждает, что нет ошибки.
///
/// Мы пишем только в `String` в этом модуле.
macro_rules! write {
    ($($tt:tt)*) => { std::write!($($tt)*).unwrap(); }
}

/// Оборачивает `std::writeln!` и утверждает, что нет ошибки.
///
/// Мы пишем только в `String` в этом модуле.
macro_rules! writeln {
    ($($tt:tt)*) => { std::writeln!($($tt)*).unwrap(); }
}

/// Возвращает строку в формате `roff`, соответствующую всей странице
/// руководства ripgrep.
pub(crate) fn generate() -> String {
    let mut cats = BTreeMap::new();
    for flag in FLAGS.iter().copied() {
        let mut cat = cats.entry(flag.doc_category()).or_insert(String::new());
        if !cat.is_empty() {
            writeln!(cat, ".sp");
        }
        generate_flag(flag, &mut cat);
    }

    let mut out = TEMPLATE.replace("!!VERSION!!", &version::generate_digits());
    for (cat, value) in cats.iter() {
        let var = format!("!!{name}!!", name = cat.as_str());
        out = out.replace(&var, value);
    }
    out
}

/// Пишет документацию в формате `roff` для `flag` в `out`.
fn generate_flag(flag: &'static dyn Flag, out: &mut String) {
    if let Some(byte) = flag.name_short() {
        let name = char::from(byte);
        write!(out, r"\fB\-{name}\fP");
        if let Some(var) = flag.doc_variable() {
            write!(out, r" \fI{var}\fP");
        }
        write!(out, r", ");
    }

    let name = flag.name_long().replace("-", r"\-");
    write!(out, r"\fB\-\-{name}\fP");
    if let Some(var) = flag.doc_variable() {
        write!(out, r"=\fI{var}\fP");
    }
    write!(out, "\n");

    writeln!(out, ".RS 4");
    let doc = flag.doc_long().trim();
    // Convert \flag{foo} into something nicer.
    let doc = super::render_custom_markup(doc, "flag", |name, out| {
        let Some(flag) = crate::flags::parse::lookup(name) else {
            unreachable!(r"found unrecognized \flag{{{name}}} in roff docs")
        };
        out.push_str(r"\fB");
        if let Some(name) = flag.name_short() {
            write!(out, r"\-{}/", char::from(name));
        }
        write!(out, r"\-\-{}", flag.name_long().replace("-", r"\-"));
        out.push_str(r"\fP");
    });
    // Convert \flag-negate{foo} into something nicer.
    let doc = super::render_custom_markup(&doc, "flag-negate", |name, out| {
        let Some(flag) = crate::flags::parse::lookup(name) else {
            unreachable!(
                r"found unrecognized \flag-negate{{{name}}} in roff docs"
            )
        };
        let Some(name) = flag.name_negated() else {
            let long = flag.name_long();
            unreachable!(
                "found \\flag-negate{{{long}}} in roff docs but \
                 {long} does not have a negation"
            );
        };
        out.push_str(r"\fB");
        write!(out, r"\-\-{name}");
        out.push_str(r"\fP");
    });
    writeln!(out, "{doc}");
    if let Some(negated) = flag.name_negated() {
        // Флаги, которые могут быть отрицательными и не являются переключателями,
        // такие как --context-separator, несколько странные. Из-за этого
        // документация для этих флагов должна явно обсуждать семантику
        // отрицания. Но для переключателей поведение всегда одинаково.
        if flag.is_switch() {
            writeln!(out, ".sp");
            writeln!(
                out,
                r"This flag can be disabled with \fB\-\-{negated}\fP."
            );
        }
    }
    writeln!(out, ".RE");
}
