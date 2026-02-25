/*!
Модули для генерации документации для флагов ripgrep.
*/

pub(crate) mod help;
pub(crate) mod man;
pub(crate) mod version;

/// Ищет вхождения `\tag{...}` в `doc` и вызывает `replacement` для
/// каждого такого найденного тега.
///
/// Первый аргумент, данный `replacement`, — это значение тега, `...`. Второй
/// аргумент — это буфер, который накапливает полный текст замены.
///
/// Поскольку эта функция предназначена только для использования в строках
/// документации, записанных в исходный код программы, вызывающие должны
/// паниковать в `replacement`, если есть какие-либо ошибки или
/// неожиданные обстоятельства.
fn render_custom_markup(
    mut doc: &str,
    tag: &str,
    mut replacement: impl FnMut(&str, &mut String),
) -> String {
    let mut out = String::with_capacity(doc.len());
    let tag_prefix = format!(r"\{tag}{{");
    while let Some(offset) = doc.find(&tag_prefix) {
        out.push_str(&doc[..offset]);

        let start = offset + tag_prefix.len();
        let Some(end) = doc[start..].find('}').map(|i| start + i) else {
            unreachable!(r"found {tag_prefix} without closing }}");
        };
        let name = &doc[start..end];
        replacement(name, &mut out);
        doc = &doc[end + 1..];
    }
    out.push_str(doc);
    out
}
