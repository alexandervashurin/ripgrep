/*!
Этот модуль предоставляет процедуры для чтения файлов конфигурации «rc» ripgrep.

Основным выходом этих процедур является последовательность аргументов, где
каждый аргумент точно соответствует одному аргументу оболочки.
*/

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

use bstr::{ByteSlice, io::BufReadExt};

/// Возвращает последовательность аргументов, полученных из файлов конфигурации
/// rc ripgrep.
pub fn args() -> Vec<OsString> {
    let config_path = match std::env::var_os("RIPGREP_CONFIG_PATH") {
        None => return vec![],
        Some(config_path) => {
            if config_path.is_empty() {
                return vec![];
            }
            PathBuf::from(config_path)
        }
    };
    let (args, errs) = match parse(&config_path) {
        Ok((args, errs)) => (args, errs),
        Err(err) => {
            message!(
                "failed to read the file specified in RIPGREP_CONFIG_PATH: {}",
                err
            );
            return vec![];
        }
    };
    if !errs.is_empty() {
        for err in errs {
            message!("{}:{}", config_path.display(), err);
        }
    }
    log::debug!(
        "{}: arguments loaded from config file: {:?}",
        config_path.display(),
        args
    );
    args
}

/// Разбирает единственный файл rc ripgrep из данного пути.
///
/// При успехе эта функция возвращает набор аргументов оболочки, по порядку,
/// которые должны быть добавлены в начало аргументов, данных ripgrep в
/// командной строке.
///
/// Если файл не может быть прочитан, то возвращается ошибка. Если возникла
/// проблема с разбором одной или нескольких строк в файле, то возвращаются
/// ошибки для каждой строки в дополнение к успешно разобранным аргументам.
fn parse<P: AsRef<Path>>(
    path: P,
) -> anyhow::Result<(Vec<OsString>, Vec<anyhow::Error>)> {
    let path = path.as_ref();
    match std::fs::File::open(&path) {
        Ok(file) => parse_reader(file),
        Err(err) => anyhow::bail!("{}: {}", path.display(), err),
    }
}

/// Разбирает единственный файл rc ripgrep из данного читателя.
///
/// Вызывающие не должны предоставлять буферизованный читатель, так как эта
/// процедура будет использовать свой собственный буфер внутри.
///
/// При успехе эта функция возвращает набор аргументов оболочки, по порядку,
/// которые должны быть добавлены в начало аргументов, данных ripgrep в
/// командной строке.
///
/// Если читатель не может быть прочитан, то возвращается ошибка. Если возникла
/// проблема с разбором одной или нескольких строк, то возвращаются ошибки
/// для каждой строки в дополнение к успешно разобранным аргументам.
fn parse_reader<R: std::io::Read>(
    rdr: R,
) -> anyhow::Result<(Vec<OsString>, Vec<anyhow::Error>)> {
    let mut bufrdr = std::io::BufReader::new(rdr);
    let (mut args, mut errs) = (vec![], vec![]);
    let mut line_number = 0;
    bufrdr.for_byte_line_with_terminator(|line| {
        line_number += 1;

        let line = line.trim();
        if line.is_empty() || line[0] == b'#' {
            return Ok(true);
        }
        match line.to_os_str() {
            Ok(osstr) => {
                args.push(osstr.to_os_string());
            }
            Err(err) => {
                errs.push(anyhow::anyhow!("{line_number}: {err}"));
            }
        }
        Ok(true)
    })?;
    Ok((args, errs))
}

#[cfg(test)]
mod tests {
    use super::parse_reader;
    use std::ffi::OsString;

    #[test]
    fn basic() {
        let (args, errs) = parse_reader(
            &b"\
# Test
--context=0
   --smart-case
-u


   # --bar
--foo
"[..],
        )
        .unwrap();
        assert!(errs.is_empty());
        let args: Vec<String> =
            args.into_iter().map(|s| s.into_string().unwrap()).collect();
        assert_eq!(args, vec!["--context=0", "--smart-case", "-u", "--foo",]);
    }

    // Мы проверяем, что мы можем обработать невалидный UTF-8 в Unix-подобных системах.
    #[test]
    #[cfg(unix)]
    fn error() {
        use std::os::unix::ffi::OsStringExt;

        let (args, errs) = parse_reader(
            &b"\
quux
foo\xFFbar
baz
"[..],
        )
        .unwrap();
        assert!(errs.is_empty());
        assert_eq!(
            args,
            vec![
                OsString::from("quux"),
                OsString::from_vec(b"foo\xFFbar".to_vec()),
                OsString::from("baz"),
            ]
        );
    }

    // ... но проверяем, что невалидный UTF-8 приводит к ошибке в Windows.
    #[test]
    #[cfg(not(unix))]
    fn error() {
        let (args, errs) = parse_reader(
            &b"\
quux
foo\xFFbar
baz
"[..],
        )
        .unwrap();
        assert_eq!(errs.len(), 1);
        assert_eq!(args, vec![OsString::from("quux"), OsString::from("baz"),]);
    }
}
