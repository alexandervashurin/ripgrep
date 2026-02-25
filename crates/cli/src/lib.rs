/*!
Этот крейт предоставляет общие подпрограммы, используемые в приложениях
командной строки, с фокусом на подпрограммы, полезные для приложений,
ориентированных на поиск. Как утилитарная библиотека, здесь нет
центрального типа или функции. Однако ключевой фокус этого крейта —
улучшение режимов отказа и предоставление удобных для пользователя
сообщений об ошибках, когда что-то идет не так.

Насколько это возможно, все в этом крейте работает в Windows, macOS и Linux.


# Стандартный ввод/вывод

[`is_readable_stdin`] определяет, можно ли полезно читать из stdin. Это
полезно при написании приложения, которое изменяет поведение в зависимости
от того, было ли приложение вызвано с данными в stdin. Например, `rg foo`
может рекурсивно искать в текущем рабочем каталоге вхождения `foo`, но
`rg foo < file` может искать только в содержимом `file`.


# Раскраска и буферизация

Подпрограммы [`stdout`], [`stdout_buffered_block`] и [`stdout_buffered_line`]
являются альтернативными конструкторами для [`StandardStream`].
`StandardStream` реализует `termcolor::WriteColor`, который предоставляет
способ вывода цветов в терминалы. Его ключевое использование — инкапсуляция
стиля буферизации. А именно, `stdout` вернет построчно буферизированный
`StandardStream` тогда и только тогда, когда stdout подключен к tty, и в
противном случае вернет блочно буферизированный `StandardStream`.
Построчная буферизация важна для использования с tty, потому что она обычно
уменьшает задержку, с которой конечный пользователь видит вывод. Блочная
буферизация используется в противном случае, потому что она быстрее, и
перенаправление stdout в файл обычно не получает выгоды от уменьшенной
задержки, которую обеспечивает построчная буферизация.

`stdout_buffered_block` и `stdout_buffered_line` могут использоваться для
явной установки стратегии буферизации независимо от того, подключен ли
stdout к tty или нет.


# Экранирование

Подпрограммы [`escape`](crate::escape()), [`escape_os`], [`unescape`] и
[`unescape_os`] предоставляют удобный для пользователя способ работы с
кодированными UTF-8 строками, которые могут выражать произвольные байты.
Например, вы можете захотеть принять строку, содержащую произвольные байты,
в качестве аргумента командной строки, но большинство интерактивных оболочек
затрудняют ввод таких строк. Вместо этого мы можем попросить пользователей
использовать экранирующие последовательности.

Например, `a\xFFz` сама по себе является валидной UTF-8 строкой,
соответствующей следующим байтам:

```ignore
[b'a', b'\\', b'x', b'F', b'F', b'z']
```

Однако мы можем
интерпретировать `\xFF` как экранирующую последовательность с помощью
подпрограмм `unescape`/`unescape_os`, которые выдадут

```ignore
[b'a', b'\xFF', b'z']
```

вместо этого. Например:

```
use grep_cli::unescape;

// Обратите внимание на использование сырой строки!
assert_eq!(vec![b'a', b'\xFF', b'z'], unescape(r"a\xFFz"));
```

Подпрограммы `escape`/`escape_os` предоставляют обратное преобразование,
что упрощает отображение удобных для пользователя сообщений об ошибках,
включающих произвольные байты.


# Построение шаблонов

Обычно шаблоны регулярных выражений должны быть валидным UTF-8. Однако
аргументы командной строки не гарантированно являются валидным UTF-8.
К сожалению, функции преобразования UTF-8 стандартной библиотеки из
`OsStr` не предоставляют хороших сообщений об ошибках. Однако
[`pattern_from_bytes`] и [`pattern_from_os`] предоставляют, включая
сообщение точно о том, где был замечен первый невалидный байт UTF-8.

Дополнительно может быть полезно читать шаблоны из файла, сообщая хорошие
сообщения об ошибках, которые включают номера строк. Подпрограммы
[`patterns_from_path`], [`patterns_from_reader`] и [`patterns_from_stdin`]
делают именно это. Если найден какой-либо шаблон, который является
невалидным UTF-8, то ошибка включает путь к файлу (если доступен) вместе
с номером строки и смещением байта, в котором был наблюден первый
невалидный байт UTF-8.


# Чтение вывода процесса

Иногда приложению командной строки нужно выполнять другие процессы и читать
их stdout потоковым способом. [`CommandReader`] предоставляет эту
функциональность с явной целью улучшения режимов отказа. В частности, если
процесс завершается с кодом ошибки, то stderr читается и преобразуется в
обычную ошибку Rust для показа конечным пользователям. Это делает основные
режимы отказа явными и дает больше информации конечным пользователям для
отладки проблемы.

Как частный случай, [`DecompressionReader`] предоставляет способ распаковки
произвольных файлов путем сопоставления их расширений файлов с соответствующими
программами распаковки (такими как `gzip` и `xz`). Это полезно как средство
выполнения упрощенной распаковки переносимым образом без привязки к конкретным
библиотекам сжатия. Однако это накладывает некоторые накладные расходы, поэтому,
если вам нужно распаковать много маленьких файлов, это может не подходить.

Каждый читатель имеет соответствующий построитель для дополнительной настройки,
например, читать ли stderr асинхронно, чтобы избежать взаимной блокировки (что
включено по умолчанию).


# Разный разбор

Подпрограмма [`parse_human_readable_size`] разбирает строки типа `2M` и
преобразует их в соответствующее количество байт (`2 * 1<<20` в этом случае).
Если найден невалидный размер, то создается хорошее сообщение об ошибке,
которое обычно говорит пользователю, как исправить проблему.
*/

#![deny(missing_docs)]

mod decompress;
mod escape;
mod hostname;
mod human;
mod pattern;
mod process;
mod wtr;

pub use crate::{
    decompress::{
        DecompressionMatcher, DecompressionMatcherBuilder,
        DecompressionReader, DecompressionReaderBuilder, resolve_binary,
    },
    escape::{escape, escape_os, unescape, unescape_os},
    hostname::hostname,
    human::{ParseSizeError, parse_human_readable_size},
    pattern::{
        InvalidPatternError, pattern_from_bytes, pattern_from_os,
        patterns_from_path, patterns_from_reader, patterns_from_stdin,
    },
    process::{CommandError, CommandReader, CommandReaderBuilder},
    wtr::{
        StandardStream, stdout, stdout_buffered_block, stdout_buffered_line,
    },
};

/// Возвращает true тогда и только тогда, когда stdin считается читаемым.
///
/// Когда stdin читаем, программы командной строки могут выбирать поведение,
/// отличное от того, когда stdin не читаем. Например, `command foo` может
/// искать в текущем каталоге вхождения `foo`, тогда как
/// `command foo < some-file` или `cat some-file | command foo` могут вместо
/// этого искать только в stdin вхождения `foo`.
///
/// Обратите внимание, что это не идеально и по существу соответствует эвристике.
/// Когда вещи неясны (например, если ошибка возникает во время интроспекции
/// для определения, читаем ли stdin), это предпочитает вернуть `false`. Это
/// означает, что возможно, что конечный пользователь передаст что-то в вашу
/// программу, и это вернет `false` и, таким образом, потенциально приведет
/// к игнорированию данных stdin пользователя. Хотя это не идеально, это,
/// возможно, лучше, чем ложное предположение, что stdin читаем, что привело
/// бы к вечной блокировке на чтении stdin. Несмотря на это, команды всегда
/// должны предоставлять явные резервные варианты для переопределения
/// поведения. Например, `rg foo -` будет явно искать в stdin, а `rg foo ./`
/// будет явно искать в текущем рабочем каталоге.
pub fn is_readable_stdin() -> bool {
    use std::io::IsTerminal;

    #[cfg(unix)]
    fn imp() -> bool {
        use std::{
            fs::File,
            os::{fd::AsFd, unix::fs::FileTypeExt},
        };

        let stdin = std::io::stdin();
        let fd = match stdin.as_fd().try_clone_to_owned() {
            Ok(fd) => fd,
            Err(err) => {
                log::debug!(
                    "for heuristic stdin detection on Unix, \
                     could not clone stdin file descriptor \
                     (thus assuming stdin is not readable): {err}",
                );
                return false;
            }
        };
        let file = File::from(fd);
        let md = match file.metadata() {
            Ok(md) => md,
            Err(err) => {
                log::debug!(
                    "for heuristic stdin detection on Unix, \
                     could not get file metadata for stdin \
                     (thus assuming stdin is not readable): {err}",
                );
                return false;
            }
        };
        let ft = md.file_type();
        let is_file = ft.is_file();
        let is_fifo = ft.is_fifo();
        let is_socket = ft.is_socket();
        let is_readable = is_file || is_fifo || is_socket;
        log::debug!(
            "for heuristic stdin detection on Unix, \
             found that \
             is_file={is_file}, is_fifo={is_fifo} and is_socket={is_socket}, \
             and thus concluded that is_stdin_readable={is_readable}",
        );
        is_readable
    }

    #[cfg(windows)]
    fn imp() -> bool {
        let stdin = winapi_util::HandleRef::stdin();
        let typ = match winapi_util::file::typ(stdin) {
            Ok(typ) => typ,
            Err(err) => {
                log::debug!(
                    "for heuristic stdin detection on Windows, \
                     could not get file type of stdin \
                     (thus assuming stdin is not readable): {err}",
                );
                return false;
            }
        };
        let is_disk = typ.is_disk();
        let is_pipe = typ.is_pipe();
        let is_readable = is_disk || is_pipe;
        log::debug!(
            "for heuristic stdin detection on Windows, \
             found that is_disk={is_disk} and is_pipe={is_pipe}, \
             and thus concluded that is_stdin_readable={is_readable}",
        );
        is_readable
    }

    #[cfg(not(any(unix, windows)))]
    fn imp() -> bool {
        log::debug!("on non-{{Unix,Windows}}, assuming stdin is not readable");
        false
    }

    !std::io::stdin().is_terminal() && imp()
}

/// Возвращает true тогда и только тогда, когда stdin считается подключенным
/// к tty или консоли.
///
/// Обратите внимание, что это теперь просто обертка вокруг
/// [`std::io::IsTerminal`](https://doc.rust-lang.org/std/io/trait.IsTerminal.html).
/// Вызывающие должны предпочитать использовать трейт `IsTerminal` напрямую.
/// Эта подпрограмма устарела и будет удалена в следующем несовместимом
/// релизе semver.
#[deprecated(since = "0.1.10", note = "use std::io::IsTerminal instead")]
pub fn is_tty_stdin() -> bool {
    use std::io::IsTerminal;
    std::io::stdin().is_terminal()
}

/// Возвращает true тогда и только тогда, когда stdout считается подключенным
/// к tty или консоли.
///
/// Это полезно, когда вы хотите, чтобы ваша программа командной строки
/// выдавала разный вывод в зависимости от того, печатает ли она напрямую
/// в терминал пользователя или перенаправляется куда-то еще. Например,
/// реализации `ls` часто показывают один элемент на строку, когда stdout
/// перенаправлен, но сжимают вывод при печати в tty.
///
/// Обратите внимание, что это теперь просто обертка вокруг
/// [`std::io::IsTerminal`](https://doc.rust-lang.org/std/io/trait.IsTerminal.html).
/// Вызывающие должны предпочитать использовать трейт `IsTerminal` напрямую.
/// Эта подпрограмма устарела и будет удалена в следующем несовместимом
/// релизе semver.
#[deprecated(since = "0.1.10", note = "use std::io::IsTerminal instead")]
pub fn is_tty_stdout() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal()
}

/// Возвращает true тогда и только тогда, когда stderr считается подключенным
/// к tty или консоли.
///
/// Обратите внимание, что это теперь просто обертка вокруг
/// [`std::io::IsTerminal`](https://doc.rust-lang.org/std/io/trait.IsTerminal.html).
/// Вызывающие должны предпочитать использовать трейт `IsTerminal` напрямую.
/// Эта подпрограмма устарела и будет удалена в следующем несовместимом
/// релизе semver.
#[deprecated(since = "0.1.10", note = "use std::io::IsTerminal instead")]
pub fn is_tty_stderr() -> bool {
    use std::io::IsTerminal;
    std::io::stderr().is_terminal()
}
