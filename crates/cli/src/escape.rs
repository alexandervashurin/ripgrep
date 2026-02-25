use std::ffi::OsStr;

use bstr::{ByteSlice, ByteVec};

/// Экранирует произвольные байты в удобочитаемую строку.
///
/// Это преобразует `\t`, `\r` и `\n` в их экранированные формы. Также
/// преобразует непечатаемое подмножество ASCII в дополнение к невалидным
/// байтам UTF-8 в шестнадцатеричные экранирующие последовательности.
/// Все остальное остается без изменений.
///
/// Двойственная функция к этой — [`unescape`].
///
/// # Пример
///
/// Этот пример показывает, как преобразовать байтовую строку, содержащую `\n` и
/// невалидные байты UTF-8, в `String`.
///
/// Обратите особое внимание на использование сырых строк. То есть `r"\n"`
/// эквивалентно `"\\n"`.
///
/// ```
/// use grep_cli::escape;
///
/// assert_eq!(r"foo\nbar\xFFbaz", escape(b"foo\nbar\xFFbaz"));
/// ```
pub fn escape(bytes: &[u8]) -> String {
    bytes.escape_bytes().to_string()
}

/// Экранирует OS-строку в удобочитаемую строку.
///
/// Это как [`escape`], но принимает OS-строку.
pub fn escape_os(string: &OsStr) -> String {
    escape(Vec::from_os_str_lossy(string).as_bytes())
}

/// Деэкранирует строку.
///
/// Поддерживает ограниченный набор экранирующих последовательностей:
///
/// * `\t`, `\r` и `\n` отображаются в соответствующие байты ASCII.
/// * `\xZZ` шестнадцатеричные экранирования отображаются в их байт.
///
/// Все остальное остается без изменений, включая не-шестнадцатеричные
/// экранирования типа `\xGG`.
///
/// Это полезно, когда желательно, чтобы аргумент командной строки мог
/// указывать произвольные байты или иным образом упрощал указание
/// непечатаемых символов.
///
/// Двойственная функция к этой — [`escape`].
///
/// # Пример
///
/// Этот пример показывает, как преобразовать экранированную строку
/// (которая является валидным UTF-8) в соответствующую последовательность
/// байтов. Каждая экранирующая последовательность отображается в
/// свои байты, которые могут включать невалидный UTF-8.
///
/// Обратите особое внимание на использование сырых строк. То есть `r"\n"`
/// эквивалентно `"\\n"`.
///
/// ```
/// use grep_cli::unescape;
///
/// assert_eq!(&b"foo\nbar\xFFbaz"[..], &*unescape(r"foo\nbar\xFFbaz"));
/// ```
pub fn unescape(s: &str) -> Vec<u8> {
    Vec::unescape_bytes(s)
}

/// Деэкранирует OS-строку.
///
/// Это как [`unescape`], но принимает OS-строку.
///
/// Обратите внимание, что это сначала с потерями декодирует данную
/// OS-строку как UTF-8. То есть экранированная строку (то, что дается)
/// должна быть валидным UTF-8.
pub fn unescape_os(string: &OsStr) -> Vec<u8> {
    unescape(&string.to_string_lossy())
}

#[cfg(test)]
mod tests {
    use super::{escape, unescape};

    fn b(bytes: &'static [u8]) -> Vec<u8> {
        bytes.to_vec()
    }

    #[test]
    fn empty() {
        assert_eq!(b(b""), unescape(r""));
        assert_eq!(r"", escape(b""));
    }

    #[test]
    fn backslash() {
        assert_eq!(b(b"\\"), unescape(r"\\"));
        assert_eq!(r"\\", escape(b"\\"));
    }

    #[test]
    fn nul() {
        assert_eq!(b(b"\x00"), unescape(r"\x00"));
        assert_eq!(b(b"\x00"), unescape(r"\0"));
        assert_eq!(r"\0", escape(b"\x00"));
    }

    #[test]
    fn nl() {
        assert_eq!(b(b"\n"), unescape(r"\n"));
        assert_eq!(r"\n", escape(b"\n"));
    }

    #[test]
    fn tab() {
        assert_eq!(b(b"\t"), unescape(r"\t"));
        assert_eq!(r"\t", escape(b"\t"));
    }

    #[test]
    fn carriage() {
        assert_eq!(b(b"\r"), unescape(r"\r"));
        assert_eq!(r"\r", escape(b"\r"));
    }

    #[test]
    fn nothing_simple() {
        assert_eq!(b(b"\\a"), unescape(r"\a"));
        assert_eq!(b(b"\\a"), unescape(r"\\a"));
        assert_eq!(r"\\a", escape(b"\\a"));
    }

    #[test]
    fn nothing_hex0() {
        assert_eq!(b(b"\\x"), unescape(r"\x"));
        assert_eq!(b(b"\\x"), unescape(r"\\x"));
        assert_eq!(r"\\x", escape(b"\\x"));
    }

    #[test]
    fn nothing_hex1() {
        assert_eq!(b(b"\\xz"), unescape(r"\xz"));
        assert_eq!(b(b"\\xz"), unescape(r"\\xz"));
        assert_eq!(r"\\xz", escape(b"\\xz"));
    }

    #[test]
    fn nothing_hex2() {
        assert_eq!(b(b"\\xzz"), unescape(r"\xzz"));
        assert_eq!(b(b"\\xzz"), unescape(r"\\xzz"));
        assert_eq!(r"\\xzz", escape(b"\\xzz"));
    }

    #[test]
    fn invalid_utf8() {
        assert_eq!(r"\xFF", escape(b"\xFF"));
        assert_eq!(r"a\xFFb", escape(b"a\xFFb"));
    }
}
