use std::{ffi::OsStr, path::Path};

use crate::walk::DirEntry;

/// Возвращает true тогда и только тогда, когда эта запись считается скрытой.
///
/// Это возвращает true только если базовое имя пути начинается с `.`.
///
/// На Unix это реализует более оптимизированную проверку.
#[cfg(unix)]
pub(crate) fn is_hidden(dent: &DirEntry) -> bool {
    use std::os::unix::ffi::OsStrExt;

    if let Some(name) = file_name(dent.path()) {
        name.as_bytes().get(0) == Some(&b'.')
    } else {
        false
    }
}

/// Возвращает true тогда и только тогда, когда эта запись считается скрытой.
///
/// На Windows это возвращает true, если верно одно из следующего:
///
/// * Базовое имя пути начинается с `.`.
/// * Атрибуты файла имеют установленное свойство `HIDDEN`.
#[cfg(windows)]
pub(crate) fn is_hidden(dent: &DirEntry) -> bool {
    use std::os::windows::fs::MetadataExt;
    use winapi_util::file;

    // This looks like we're doing an extra stat call, but on Windows, the
    // directory traverser reuses the metadata retrieved from each directory
    // entry and stores it on the DirEntry itself. So this is "free."
    if let Ok(md) = dent.metadata() {
        if file::is_hidden(md.file_attributes() as u64) {
            return true;
        }
    }
    if let Some(name) = file_name(dent.path()) {
        name.to_str().map(|s| s.starts_with(".")).unwrap_or(false)
    } else {
        false
    }
}

/// Возвращает true тогда и только тогда, когда эта запись считается скрытой.
///
/// Это возвращает true только если базовое имя пути начинается с `.`.
#[cfg(not(any(unix, windows)))]
pub(crate) fn is_hidden(dent: &DirEntry) -> bool {
    if let Some(name) = file_name(dent.path()) {
        name.to_str().map(|s| s.starts_with(".")).unwrap_or(false)
    } else {
        false
    }
}

/// Удаляет `prefix` из `path` и возвращает остаток.
///
/// Если `path` не имеет префикса `prefix`, то возвращает `None`.
#[cfg(unix)]
pub(crate) fn strip_prefix<'a, P: AsRef<Path> + ?Sized>(
    prefix: &'a P,
    path: &'a Path,
) -> Option<&'a Path> {
    use std::os::unix::ffi::OsStrExt;

    let prefix = prefix.as_ref().as_os_str().as_bytes();
    let path = path.as_os_str().as_bytes();
    if prefix.len() > path.len() || prefix != &path[0..prefix.len()] {
        None
    } else {
        Some(&Path::new(OsStr::from_bytes(&path[prefix.len()..])))
    }
}

/// Удаляет `prefix` из `path` и возвращает остаток.
///
/// Если `path` не имеет префикса `prefix`, то возвращает `None`.
#[cfg(not(unix))]
pub(crate) fn strip_prefix<'a, P: AsRef<Path> + ?Sized>(
    prefix: &'a P,
    path: &'a Path,
) -> Option<&'a Path> {
    path.strip_prefix(prefix).ok()
}

/// Возвращает true, если этот путь к файлу является просто именем файла.
/// Т.е. Его родитель — пустая строка.
#[cfg(unix)]
pub(crate) fn is_file_name<P: AsRef<Path>>(path: P) -> bool {
    use std::os::unix::ffi::OsStrExt;

    use memchr::memchr;

    let path = path.as_ref().as_os_str().as_bytes();
    memchr(b'/', path).is_none()
}

/// Возвращает true, если этот путь к файлу является просто именем файла.
/// Т.е. Его родитель — пустая строка.
#[cfg(not(unix))]
pub(crate) fn is_file_name<P: AsRef<Path>>(path: P) -> bool {
    path.as_ref().parent().map(|p| p.as_os_str().is_empty()).unwrap_or(false)
}

/// Конечный компонент пути, если это обычный файл.
///
/// Если путь заканчивается на ., .. или состоит только из корня или префикса,
/// file_name вернёт None.
#[cfg(unix)]
pub(crate) fn file_name<'a, P: AsRef<Path> + ?Sized>(
    path: &'a P,
) -> Option<&'a OsStr> {
    use memchr::memrchr;
    use std::os::unix::ffi::OsStrExt;

    let path = path.as_ref().as_os_str().as_bytes();
    if path.is_empty() {
        return None;
    } else if path.len() == 1 && path[0] == b'.' {
        return None;
    } else if path.last() == Some(&b'.') {
        return None;
    } else if path.len() >= 2 && &path[path.len() - 2..] == &b".."[..] {
        return None;
    }
    let last_slash = memrchr(b'/', path).map(|i| i + 1).unwrap_or(0);
    Some(OsStr::from_bytes(&path[last_slash..]))
}

/// Конечный компонент пути, если это обычный файл.
///
/// Если путь заканчивается на ., .. или состоит только из корня или префикса,
/// file_name вернёт None.
#[cfg(not(unix))]
pub(crate) fn file_name<'a, P: AsRef<Path> + ?Sized>(
    path: &'a P,
) -> Option<&'a OsStr> {
    path.as_ref().file_name()
}
