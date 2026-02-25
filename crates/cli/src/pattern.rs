use std::{ffi::OsStr, io, path::Path};

use bstr::io::BufReadExt;

use crate::escape::{escape, escape_os};

/// Ошибка, возникающая, когда шаблон не может быть преобразован в валидный UTF-8.
///
/// Назначение этой ошибки — предоставить более целевой режим отказа для
/// шаблонов, записанных конечными пользователями, которые не являются
/// валидным UTF-8.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvalidPatternError {
    original: String,
    valid_up_to: usize,
}

impl InvalidPatternError {
    /// Возвращает индекс в данной строке, до которого валидный UTF-8 был
    /// проверен.
    pub fn valid_up_to(&self) -> usize {
        self.valid_up_to
    }
}

impl std::error::Error for InvalidPatternError {}

impl std::fmt::Display for InvalidPatternError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "найден невалидный UTF-8 в шаблоне по смещению байта {}: {} \
             (отключите режим Unicode и используйте шестнадцатеричные \
             экранирующие последовательности для сопоставления произвольных \
             байтов в шаблоне, например, '(?-u)\\xFF')",
            self.valid_up_to, self.original,
        )
    }
}

impl From<InvalidPatternError> for io::Error {
    fn from(paterr: InvalidPatternError) -> io::Error {
        io::Error::new(io::ErrorKind::Other, paterr)
    }
}

/// Преобразует OS-строку в шаблон регулярного выражения.
///
/// Это преобразование завершается ошибкой, если данный шаблон не является
/// валидным UTF-8, в этом случае предоставляется целевая ошибка с большей
/// информацией о том, где возникает невалидный UTF-8. Ошибка также
/// предлагает использование шестнадцатеричных экранирующих последовательностей,
/// которые поддерживаются многими движками регулярных выражений.
pub fn pattern_from_os(pattern: &OsStr) -> Result<&str, InvalidPatternError> {
    pattern.to_str().ok_or_else(|| {
        let valid_up_to = pattern
            .to_string_lossy()
            .find('\u{FFFD}')
            .expect("код замены Unicode для невалидного UTF-8");
        InvalidPatternError { original: escape_os(pattern), valid_up_to }
    })
}

/// Преобразует произвольные байты в шаблон регулярного выражения.
///
/// Это преобразование завершается ошибкой, если данный шаблон не является
/// валидным UTF-8, в этом случае предоставляется целевая ошибка с большей
/// информацией о том, где возникает невалидный UTF-8. Ошибка также
/// предлагает использование шестнадцатеричных экранирующих последовательностей,
/// которые поддерживаются многими движками регулярных выражений.
pub fn pattern_from_bytes(
    pattern: &[u8],
) -> Result<&str, InvalidPatternError> {
    std::str::from_utf8(pattern).map_err(|err| InvalidPatternError {
        original: escape(pattern),
        valid_up_to: err.valid_up_to(),
    })
}

/// Читает шаблоны из пути к файлу, по одному на строку.
///
/// Если возникла проблема при чтении или если какой-либо из шаблонов
/// содержит невалидный UTF-8, то возвращается ошибка. Если возникла
/// проблема с конкретным шаблоном, то сообщение об ошибке будет включать
/// номер строки и путь к файлу.
pub fn patterns_from_path<P: AsRef<Path>>(path: P) -> io::Result<Vec<String>> {
    let path = path.as_ref();
    let file = std::fs::File::open(path).map_err(|err| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("{}: {}", path.display(), err),
        )
    })?;
    patterns_from_reader(file).map_err(|err| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("{}:{}", path.display(), err),
        )
    })
}

/// Читает шаблоны из stdin, по одному на строку.
///
/// Если возникла проблема при чтении или если какой-либо из шаблонов
/// содержит невалидный UTF-8, то возвращается ошибка. Если возникла
/// проблема с конкретным шаблоном, то сообщение об ошибке будет включать
/// номер строки и факт того, что он пришел из stdin.
pub fn patterns_from_stdin() -> io::Result<Vec<String>> {
    let stdin = io::stdin();
    let locked = stdin.lock();
    patterns_from_reader(locked).map_err(|err| {
        io::Error::new(io::ErrorKind::Other, format!("<stdin>:{}", err))
    })
}

/// Читает шаблоны из любого читателя, по одному на строку.
///
/// Если возникла проблема при чтении или если какой-либо из шаблонов
/// содержит невалидный UTF-8, то возвращается ошибка. Если возникла
/// проблема с конкретным шаблоном, то сообщение об ошибке будет включать
/// номер строки.
///
/// Обратите внимание, что эта подпрограмма использует свой собственный
/// внутренний буфер, поэтому вызывающий не должен предоставлять свой
/// собственный буферизированный читатель, если это возможно.
///
/// # Пример
///
/// Это показывает, как разбирать шаблоны, по одному на строку.
///
/// ```
/// use grep_cli::patterns_from_reader;
///
/// let patterns = "\
/// foo
/// bar\\s+foo
/// [a-z]{3}
/// ";
///
/// assert_eq!(patterns_from_reader(patterns.as_bytes())?, vec![
///     r"foo",
///     r"bar\s+foo",
///     r"[a-z]{3}",
/// ]);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn patterns_from_reader<R: io::Read>(rdr: R) -> io::Result<Vec<String>> {
    let mut patterns = vec![];
    let mut line_number = 0;
    io::BufReader::new(rdr).for_byte_line(|line| {
        line_number += 1;
        match pattern_from_bytes(line) {
            Ok(pattern) => {
                patterns.push(pattern.to_string());
                Ok(true)
            }
            Err(err) => Err(io::Error::new(
                io::ErrorKind::Other,
                format!("{}: {}", line_number, err),
            )),
        }
    })?;
    Ok(patterns)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes() {
        let pat = b"abc\xFFxyz";
        let err = pattern_from_bytes(pat).unwrap_err();
        assert_eq!(3, err.valid_up_to());
    }

    #[test]
    #[cfg(unix)]
    fn os() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let pat = OsStr::from_bytes(b"abc\xFFxyz");
        let err = pattern_from_os(pat).unwrap_err();
        assert_eq!(3, err.valid_up_to());
    }
}
