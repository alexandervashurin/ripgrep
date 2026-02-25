use std::borrow::Cow;

use bstr::{ByteSlice, ByteVec};

/// Конечный компонент пути, если это обычный файл.
///
/// Если путь заканчивается на `..` или состоит только из корня или префикса,
/// file_name вернёт `None`.
pub(crate) fn file_name<'a>(path: &Cow<'a, [u8]>) -> Option<Cow<'a, [u8]>> {
    if path.is_empty() {
        return None;
    }
    let last_slash = path.rfind_byte(b'/').map(|i| i + 1).unwrap_or(0);
    let got = match *path {
        Cow::Borrowed(path) => Cow::Borrowed(&path[last_slash..]),
        Cow::Owned(ref path) => {
            let mut path = path.clone();
            path.drain_bytes(..last_slash);
            Cow::Owned(path)
        }
    };
    if got == &b".."[..] {
        return None;
    }
    Some(got)
}

/// Возвращает расширение файла по имени файла пути.
///
/// Обратите внимание, что это НЕ соответствует семантике
/// std::path::Path::extension. А именно, расширение включает `.` и
/// сопоставление в остальном более либерально. В частности, расширение:
///
/// * None, если данное имя файла пусто;
/// * None, если нет встроенного `.`;
/// * В противном случае, часть имени файла, начинающаяся с последнего `.`.
///
/// Например, имя файла `.rs` имеет расширение `.rs`.
///
/// N.B. Это сделано для того, чтобы некоторые оптимизации сопоставления glob
/// были проще. А именно, шаблон вида `*.rs` очевидно пытается сопоставить
/// файлы с расширением `rs`, но он также соответствует файлам вида `.rs`,
/// которые не имеют расширения согласно std::path::Path::extension.
pub(crate) fn file_name_ext<'a>(
    name: &Cow<'a, [u8]>,
) -> Option<Cow<'a, [u8]>> {
    if name.is_empty() {
        return None;
    }
    let last_dot_at = match name.rfind_byte(b'.') {
        None => return None,
        Some(i) => i,
    };
    Some(match *name {
        Cow::Borrowed(name) => Cow::Borrowed(&name[last_dot_at..]),
        Cow::Owned(ref name) => {
            let mut name = name.clone();
            name.drain_bytes(..last_dot_at);
            Cow::Owned(name)
        }
    })
}

/// Нормализует путь для использования `/` в качестве разделителя везде,
/// даже на платформах, которые распознают другие символы в качестве разделителей.
#[cfg(unix)]
pub(crate) fn normalize_path(path: Cow<'_, [u8]>) -> Cow<'_, [u8]> {
    // UNIX использует только /, так что всё хорошо.
    path
}

/// Нормализует путь для использования `/` в качестве разделителя везде,
/// даже на платформах, которые распознают другие символы в качестве разделителей.
#[cfg(not(unix))]
pub(crate) fn normalize_path(mut path: Cow<[u8]>) -> Cow<[u8]> {
    use std::path::is_separator;

    for i in 0..path.len() {
        if path[i] == b'/' || !is_separator(char::from(path[i])) {
            continue;
        }
        path.to_mut()[i] = b'/';
    }
    path
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use bstr::{B, ByteVec};

    use super::{file_name_ext, normalize_path};

    macro_rules! ext {
        ($name:ident, $file_name:expr, $ext:expr) => {
            #[test]
            fn $name() {
                let bs = Vec::from($file_name);
                let got = file_name_ext(&Cow::Owned(bs));
                assert_eq!($ext.map(|s| Cow::Borrowed(B(s))), got);
            }
        };
    }

    ext!(ext1, "foo.rs", Some(".rs"));
    ext!(ext2, ".rs", Some(".rs"));
    ext!(ext3, "..rs", Some(".rs"));
    ext!(ext4, "", None::<&str>);
    ext!(ext5, "foo", None::<&str>);

    macro_rules! normalize {
        ($name:ident, $path:expr, $expected:expr) => {
            #[test]
            fn $name() {
                let bs = Vec::from_slice($path);
                let got = normalize_path(Cow::Owned(bs));
                assert_eq!($expected.to_vec(), got.into_owned());
            }
        };
    }

    normalize!(normal1, b"foo", b"foo");
    normalize!(normal2, b"foo/bar", b"foo/bar");
    #[cfg(unix)]
    normalize!(normal3, b"foo\\bar", b"foo\\bar");
    #[cfg(not(unix))]
    normalize!(normal3, b"foo\\bar", b"foo/bar");
    #[cfg(unix)]
    normalize!(normal4, b"foo\\bar/baz", b"foo\\bar/baz");
    #[cfg(not(unix))]
    normalize!(normal4, b"foo\\bar/baz", b"foo/bar/baz");
}
