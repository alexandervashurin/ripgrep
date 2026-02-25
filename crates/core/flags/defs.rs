/*!
Определяет все флаги, доступные в ripgrep.

Каждый флаг соответствует unit-структуре с соответствующей реализацией
`Flag`. Обратите внимание, что каждая реализация `Flag` может фактически иметь
много возможных проявлений одного и того же «флага». То есть каждая реализация
`Flag` может иметь следующие флаги, доступные конечному пользователю ripgrep:

* Длинное имя флага.
* Необязательное короткое имя флага.
* Необязательное отрицательное длинное имя флага.
* Произвольно длинный список псевдонимов.

Идея в том, что хотя есть несколько флагов, которые пользователь может
ввести, одна реализация `Flag` соответствует одному _логическому_ флагу внутри
ripgrep. Например, `-E`, `--encoding` и `--no-encoding` все манипулируют одним
и тем же состоянием кодировки в ripgrep.
*/

use std::{path::PathBuf, sync::LazyLock};

use {anyhow::Context as AnyhowContext, bstr::ByteVec};

use crate::flags::{
    Category, Flag, FlagValue,
    lowargs::{
        BinaryMode, BoundaryMode, BufferMode, CaseMode, ColorChoice,
        ContextMode, EncodingMode, EngineChoice, GenerateMode, LoggingMode,
        LowArgs, MmapMode, Mode, PatternSource, SearchMode, SortMode,
        SortModeKind, SpecialMode, TypeChange,
    },
};

#[cfg(test)]
use crate::flags::parse::parse_low_raw;

use super::CompletionType;

/// A list of all flags in ripgrep via implementations of `Flag`.
///
/// The order of these flags matter. It determines the order of the flags in
/// the generated documentation (`-h`, `--help` and the man page) within each
/// category. (This is why the deprecated flags are last.)
pub(super) const FLAGS: &[&dyn Flag] = &[
    // -e/--regexp and -f/--file should come before anything else in the
    // same category.
    &Regexp,
    &File,
    &AfterContext,
    &BeforeContext,
    &Binary,
    &BlockBuffered,
    &ByteOffset,
    &CaseSensitive,
    &Color,
    &Colors,
    &Column,
    &Context,
    &ContextSeparator,
    &Count,
    &CountMatches,
    &Crlf,
    &Debug,
    &DfaSizeLimit,
    &Encoding,
    &Engine,
    &FieldContextSeparator,
    &FieldMatchSeparator,
    &Files,
    &FilesWithMatches,
    &FilesWithoutMatch,
    &FixedStrings,
    &Follow,
    &Generate,
    &Glob,
    &GlobCaseInsensitive,
    &Heading,
    &Help,
    &Hidden,
    &HostnameBin,
    &HyperlinkFormat,
    &IGlob,
    &IgnoreCase,
    &IgnoreFile,
    &IgnoreFileCaseInsensitive,
    &IncludeZero,
    &InvertMatch,
    &JSON,
    &LineBuffered,
    &LineNumber,
    &LineNumberNo,
    &LineRegexp,
    &MaxColumns,
    &MaxColumnsPreview,
    &MaxCount,
    &MaxDepth,
    &MaxFilesize,
    &Mmap,
    &Multiline,
    &MultilineDotall,
    &NoConfig,
    &NoIgnore,
    &NoIgnoreDot,
    &NoIgnoreExclude,
    &NoIgnoreFiles,
    &NoIgnoreGlobal,
    &NoIgnoreMessages,
    &NoIgnoreParent,
    &NoIgnoreVcs,
    &NoMessages,
    &NoRequireGit,
    &NoUnicode,
    &Null,
    &NullData,
    &OneFileSystem,
    &OnlyMatching,
    &PathSeparator,
    &Passthru,
    &PCRE2,
    &PCRE2Version,
    &Pre,
    &PreGlob,
    &Pretty,
    &Quiet,
    &RegexSizeLimit,
    &Replace,
    &SearchZip,
    &SmartCase,
    &Sort,
    &Sortr,
    &Stats,
    &StopOnNonmatch,
    &Text,
    &Threads,
    &Trace,
    &Trim,
    &Type,
    &TypeNot,
    &TypeAdd,
    &TypeClear,
    &TypeList,
    &Unrestricted,
    &Version,
    &Vimgrep,
    &WithFilename,
    &WithFilenameNo,
    &WordRegexp,
    // DEPRECATED (make them show up last in their respective categories)
    &AutoHybridRegex,
    &NoPcre2Unicode,
    &SortFiles,
];

/// -A/--after-context
#[derive(Debug)]
struct AfterContext;

impl Flag for AfterContext {
    fn is_switch(&self) -> bool {
        false
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'A')
    }
    fn name_long(&self) -> &'static str {
        "after-context"
    }
    fn doc_variable(&self) -> Option<&'static str> {
        Some("NUM")
    }
    fn doc_category(&self) -> Category {
        Category::Output
    }
    fn doc_short(&self) -> &'static str {
        "Показать NUM строк после каждого совпадения."
    }
    fn doc_long(&self) -> &'static str {
        r"
Показать \fINUM\fP строк после каждого совпадения.
.sp
Это переопределяет флаг \flag{passthru} и частично переопределяет флаг
\flag{context}.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.context.set_after(convert::usize(&v.unwrap_value())?);
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_after_context() {
    let mkctx = |lines| {
        let mut mode = ContextMode::default();
        mode.set_after(lines);
        mode
    };

    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(ContextMode::default(), args.context);

    let args = parse_low_raw(["--after-context", "5"]).unwrap();
    assert_eq!(mkctx(5), args.context);

    let args = parse_low_raw(["--after-context=5"]).unwrap();
    assert_eq!(mkctx(5), args.context);

    let args = parse_low_raw(["-A", "5"]).unwrap();
    assert_eq!(mkctx(5), args.context);

    let args = parse_low_raw(["-A5"]).unwrap();
    assert_eq!(mkctx(5), args.context);

    let args = parse_low_raw(["-A5", "-A10"]).unwrap();
    assert_eq!(mkctx(10), args.context);

    let args = parse_low_raw(["-A5", "-A0"]).unwrap();
    assert_eq!(mkctx(0), args.context);

    let args = parse_low_raw(["-A5", "--passthru"]).unwrap();
    assert_eq!(ContextMode::Passthru, args.context);

    let args = parse_low_raw(["--passthru", "-A5"]).unwrap();
    assert_eq!(mkctx(5), args.context);

    let n = usize::MAX.to_string();
    let args = parse_low_raw(["--after-context", n.as_str()]).unwrap();
    assert_eq!(mkctx(usize::MAX), args.context);

    #[cfg(target_pointer_width = "64")]
    {
        let n = (u128::from(u64::MAX) + 1).to_string();
        let result = parse_low_raw(["--after-context", n.as_str()]);
        assert!(result.is_err(), "{result:?}");
    }
}

/// --auto-hybrid-regex
#[derive(Debug)]
struct AutoHybridRegex;

impl Flag for AutoHybridRegex {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "auto-hybrid-regex"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("no-auto-hybrid-regex")
    }
    fn doc_category(&self) -> Category {
        Category::Search
    }
    fn doc_short(&self) -> &'static str {
        "(УСТАРЕЛО) Использовать PCRE2, если уместно."
    }
    fn doc_long(&self) -> &'static str {
        r"
УСТАРЕЛО. Используйте вместо этого \flag{engine}.
.sp
Когда этот флаг используется, ripgrep будет динамически выбирать между
поддерживаемыми движками регулярных выражений в зависимости от функций,
используемых в шаблоне. Когда ripgrep выбирает движок регулярных выражений,
он применяет этот выбор для каждого регулярного выражения, предоставленного
ripgrep (например, через несколько флагов \flag{regexp} или \flag{file}).
.sp
В качестве примера того, как может вести себя этот флаг, ripgrep попытается
использовать свой движок регулярных выражений на основе конечных автоматов
по умолчанию, когда шаблон может быть успешно скомпилирован с этим движком
регулярных выражений. Если PCRE2 включен и шаблон не может быть скомпилирован
с движком регулярных выражений по умолчанию, то PCRE2 будет автоматически
использоваться для поиска. Если PCRE2 недоступен, то этот флаг не имеет
эффекта, потому что есть только один движок регулярных выражений для выбора.
.sp
В будущем ripgrep может скорректировать свою эвристику для того, как он
решает, какой движок регулярных выражений использовать. В целом, эвристика
будет ограничена статическим анализом шаблонов, а не каким-либо конкретным
поведением во время выполнения, наблюдаемым при поиске файлов.
.sp
Основным недостатком использования этого флага является то, что может быть
не всегда очевидно, какой движок регулярных выражений использует ripgrep, и,
таким образом, семантика совпадения или профиль производительности ripgrep
могут незаметно и неожиданно измениться. Однако во многих случаях все движки
регулярных выражений согласятся с тем, что constitutes совпадение, и может
быть приятно прозрачно поддерживать более продвинутые функции регулярных
выражений, такие как просмотр окружения и обратные ссылки, без явной
необходимости их включать.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        let mode = if v.unwrap_switch() {
            EngineChoice::Auto
        } else {
            EngineChoice::Default
        };
        args.engine = mode;
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_auto_hybrid_regex() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(EngineChoice::Default, args.engine);

    let args = parse_low_raw(["--auto-hybrid-regex"]).unwrap();
    assert_eq!(EngineChoice::Auto, args.engine);

    let args =
        parse_low_raw(["--auto-hybrid-regex", "--no-auto-hybrid-regex"])
            .unwrap();
    assert_eq!(EngineChoice::Default, args.engine);

    let args =
        parse_low_raw(["--no-auto-hybrid-regex", "--auto-hybrid-regex"])
            .unwrap();
    assert_eq!(EngineChoice::Auto, args.engine);

    let args = parse_low_raw(["--auto-hybrid-regex", "-P"]).unwrap();
    assert_eq!(EngineChoice::PCRE2, args.engine);

    let args = parse_low_raw(["-P", "--auto-hybrid-regex"]).unwrap();
    assert_eq!(EngineChoice::Auto, args.engine);

    let args =
        parse_low_raw(["--engine=auto", "--auto-hybrid-regex"]).unwrap();
    assert_eq!(EngineChoice::Auto, args.engine);

    let args =
        parse_low_raw(["--engine=default", "--auto-hybrid-regex"]).unwrap();
    assert_eq!(EngineChoice::Auto, args.engine);

    let args =
        parse_low_raw(["--auto-hybrid-regex", "--engine=default"]).unwrap();
    assert_eq!(EngineChoice::Default, args.engine);
}

/// -B/--before-context
#[derive(Debug)]
struct BeforeContext;

impl Flag for BeforeContext {
    fn is_switch(&self) -> bool {
        false
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'B')
    }
    fn name_long(&self) -> &'static str {
        "before-context"
    }
    fn doc_variable(&self) -> Option<&'static str> {
        Some("NUM")
    }
    fn doc_category(&self) -> Category {
        Category::Output
    }
    fn doc_short(&self) -> &'static str {
        "Показать NUM строк перед каждым совпадением."
    }
    fn doc_long(&self) -> &'static str {
        r"
Показать \fINUM\fP строк перед каждым совпадением.
.sp
Это переопределяет флаг \flag{passthru} и частично переопределяет флаг
\flag{context}.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.context.set_before(convert::usize(&v.unwrap_value())?);
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_before_context() {
    let mkctx = |lines| {
        let mut mode = ContextMode::default();
        mode.set_before(lines);
        mode
    };

    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(ContextMode::default(), args.context);

    let args = parse_low_raw(["--before-context", "5"]).unwrap();
    assert_eq!(mkctx(5), args.context);

    let args = parse_low_raw(["--before-context=5"]).unwrap();
    assert_eq!(mkctx(5), args.context);

    let args = parse_low_raw(["-B", "5"]).unwrap();
    assert_eq!(mkctx(5), args.context);

    let args = parse_low_raw(["-B5"]).unwrap();
    assert_eq!(mkctx(5), args.context);

    let args = parse_low_raw(["-B5", "-B10"]).unwrap();
    assert_eq!(mkctx(10), args.context);

    let args = parse_low_raw(["-B5", "-B0"]).unwrap();
    assert_eq!(mkctx(0), args.context);

    let args = parse_low_raw(["-B5", "--passthru"]).unwrap();
    assert_eq!(ContextMode::Passthru, args.context);

    let args = parse_low_raw(["--passthru", "-B5"]).unwrap();
    assert_eq!(mkctx(5), args.context);

    let n = usize::MAX.to_string();
    let args = parse_low_raw(["--before-context", n.as_str()]).unwrap();
    assert_eq!(mkctx(usize::MAX), args.context);

    #[cfg(target_pointer_width = "64")]
    {
        let n = (u128::from(u64::MAX) + 1).to_string();
        let result = parse_low_raw(["--before-context", n.as_str()]);
        assert!(result.is_err(), "{result:?}");
    }
}

/// --binary
#[derive(Debug)]
struct Binary;

impl Flag for Binary {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "binary"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("no-binary")
    }
    fn doc_category(&self) -> Category {
        Category::Filter
    }
    fn doc_short(&self) -> &'static str {
        "Искать в бинарных файлах."
    }
    fn doc_long(&self) -> &'static str {
        r"
Включение этого флага заставит ripgrep искать в бинарных файлах. По умолчанию
ripgrep пытается автоматически пропускать бинарные файлы, чтобы улучшить
релевантность результатов и сделать поиск быстрее.
.sp
Бинарные файлы эвристически обнаруживаются на основе того, содержат ли они
байт \fBNUL\fP или нет. По умолчанию (без установки этого флага), как только
байт \fBNUL\fP обнаружен, ripgrep прекращает поиск файла. Обычно байты
\fBNUL\fP встречаются в начале большинства бинарных файлов. Если байт
\fBNUL\fP встречается после совпадения, то ripgrep не напечатает совпадение,
остановит поиск этого файла и выдаст предупреждение, что некоторые совпадения
подавляются.
.sp
Напротив, когда этот флаг предоставлен, ripgrep продолжит поиск файла, даже
если байт \fBNUL\fP найден. В частности, если байт \fBNUL\fP найден, то
ripgrep продолжит поиск, пока не будет найдено совпадение или не будет достигнут
конец файла, в зависимости от того, что произойдет раньше. Если совпадение
найдено, то ripgrep остановится и напечатает предупреждение, говорящее, что
поиск остановился преждевременно.
.sp
Если вы хотите, чтобы ripgrep искал файл без какой-либо специальной обработки
байта \fBNUL\fP (и потенциально выводил бинарные данные в stdout), то вы
должны использовать флаг \flag{text}.
.sp
Флаг \flag{binary} — это флаг для управления механизмом автоматической
фильтрации ripgrep. Таким образом, его не нужно использовать при явном поиске
файла или при поиске в stdin. То есть, он применим только при рекурсивном
поиске в каталоге.
.sp
Когда флаг \flag{unrestricted} предоставлен в третий раз, то этот флаг
автоматически включается.
.sp
Этот флаг переопределяет флаг \flag{text}.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.binary = if v.unwrap_switch() {
            BinaryMode::SearchAndSuppress
        } else {
            BinaryMode::Auto
        };
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_binary() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(BinaryMode::Auto, args.binary);

    let args = parse_low_raw(["--binary"]).unwrap();
    assert_eq!(BinaryMode::SearchAndSuppress, args.binary);

    let args = parse_low_raw(["--binary", "--no-binary"]).unwrap();
    assert_eq!(BinaryMode::Auto, args.binary);

    let args = parse_low_raw(["--no-binary", "--binary"]).unwrap();
    assert_eq!(BinaryMode::SearchAndSuppress, args.binary);

    let args = parse_low_raw(["--binary", "-a"]).unwrap();
    assert_eq!(BinaryMode::AsText, args.binary);

    let args = parse_low_raw(["-a", "--binary"]).unwrap();
    assert_eq!(BinaryMode::SearchAndSuppress, args.binary);

    let args = parse_low_raw(["-a", "--no-binary"]).unwrap();
    assert_eq!(BinaryMode::Auto, args.binary);
}

/// --block-buffered
#[derive(Debug)]
struct BlockBuffered;

impl Flag for BlockBuffered {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "block-buffered"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("no-block-buffered")
    }
    fn doc_category(&self) -> Category {
        Category::Output
    }
    fn doc_short(&self) -> &'static str {
        "Принудительно использовать блочную буферизацию."
    }
    fn doc_long(&self) -> &'static str {
        r"
При включении ripgrep будет использовать блочную буферизацию. То есть, когда
найдена совпадающая строка, она будет записана в буфер в памяти и не будет
записана в stdout, пока буфер не достигнет определенного размера. Это значение
по умолчанию, когда stdout ripgrep перенаправлен в конвейер или файл. Когда
stdout ripgrep подключен к tty, по умолчанию будет использоваться построчная
буферизация. Принудительная блочная буферизация может быть полезна при выводе
большого объема содержимого в tty.
.sp
Это переопределяет флаг \flag{line-buffered}.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.buffer = if v.unwrap_switch() {
            BufferMode::Block
        } else {
            BufferMode::Auto
        };
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_block_buffered() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(BufferMode::Auto, args.buffer);

    let args = parse_low_raw(["--block-buffered"]).unwrap();
    assert_eq!(BufferMode::Block, args.buffer);

    let args =
        parse_low_raw(["--block-buffered", "--no-block-buffered"]).unwrap();
    assert_eq!(BufferMode::Auto, args.buffer);

    let args = parse_low_raw(["--block-buffered", "--line-buffered"]).unwrap();
    assert_eq!(BufferMode::Line, args.buffer);
}

/// --byte-offset
#[derive(Debug)]
struct ByteOffset;

impl Flag for ByteOffset {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'b')
    }
    fn name_long(&self) -> &'static str {
        "byte-offset"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("no-byte-offset")
    }
    fn doc_category(&self) -> Category {
        Category::Output
    }
    fn doc_short(&self) -> &'static str {
        "Вывести байтовое смещение для каждой совпадающей строки."
    }
    fn doc_long(&self) -> &'static str {
        r"
Вывести 0-основанное байтовое смещение во входном файле перед каждой строкой
вывода. Если указан \flag{only-matching}, вывести смещение самого найденного
текста.
.sp
Если ripgrep выполняет перекодирование, то байтовое смещение указано в терминах
результата перекодирования, а не исходных данных. Это применяется аналогично
к другим преобразованиям данных, таким как декомпрессия или фильтр \flag{pre}.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.byte_offset = v.unwrap_switch();
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_byte_offset() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(false, args.byte_offset);

    let args = parse_low_raw(["--byte-offset"]).unwrap();
    assert_eq!(true, args.byte_offset);

    let args = parse_low_raw(["-b"]).unwrap();
    assert_eq!(true, args.byte_offset);

    let args = parse_low_raw(["--byte-offset", "--no-byte-offset"]).unwrap();
    assert_eq!(false, args.byte_offset);

    let args = parse_low_raw(["--no-byte-offset", "-b"]).unwrap();
    assert_eq!(true, args.byte_offset);
}

/// -s/--case-sensitive
#[derive(Debug)]
struct CaseSensitive;

impl Flag for CaseSensitive {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_short(&self) -> Option<u8> {
        Some(b's')
    }
    fn name_long(&self) -> &'static str {
        "case-sensitive"
    }
    fn doc_category(&self) -> Category {
        Category::Search
    }
    fn doc_short(&self) -> &'static str {
        r"Искать с учётом регистра (по умолчанию)."
    }
    fn doc_long(&self) -> &'static str {
        r"
Выполнять поиск с учётом регистра. Это режим по умолчанию.
.sp
Это глобальная опция, которая применяется ко всем шаблонам, переданным в ripgrep.
Отдельные шаблоны всё ещё могут быть сопоставлены без учёта регистра с помощью
встроенных флагов регулярных выражений. Например, \fB(?i)abc\fP будет сопоставлять
\fBabc\fP без учёта регистра, даже когда используется этот флаг.
.sp
Этот флаг переопределяет флаги \flag{ignore-case} и \flag{smart-case}.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        assert!(v.unwrap_switch(), "flag has no negation");
        args.case = CaseMode::Sensitive;
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_case_sensitive() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(CaseMode::Sensitive, args.case);

    let args = parse_low_raw(["--case-sensitive"]).unwrap();
    assert_eq!(CaseMode::Sensitive, args.case);

    let args = parse_low_raw(["-s"]).unwrap();
    assert_eq!(CaseMode::Sensitive, args.case);
}

/// --color
#[derive(Debug)]
struct Color;

impl Flag for Color {
    fn is_switch(&self) -> bool {
        false
    }
    fn name_long(&self) -> &'static str {
        "color"
    }
    fn doc_variable(&self) -> Option<&'static str> {
        Some("WHEN")
    }
    fn doc_category(&self) -> Category {
        Category::Output
    }
    fn doc_short(&self) -> &'static str {
        "Когда использовать цвет."
    }
    fn doc_long(&self) -> &'static str {
        r"
Этот флаг управляет тем, когда использовать цвета. Настройка по умолчанию —
\fBauto\fP, что означает, что ripgrep будет пытаться угадать, когда использовать
цвета. Например, если ripgrep выводит в tty, то он будет использовать цвета,
но если он перенаправлен в файл или конвейер, то он подавит цветной вывод.
.sp
ripgrep также подавляет цветной вывод по умолчанию в некоторых других случаях.
Они включают, но не ограничиваются:
.sp
.IP \(bu 3n
Когда переменная окружения \fBTERM\fP не установлена или установлена в \fBdumb\fP.
.sp
.IP \(bu 3n
Когда переменная окружения \fBNO_COLOR\fP установлена (независимо от значения).
.sp
.IP \(bu 3n
Когда предоставлены флаги, которые подразумевают отсутствие необходимости в цветах.
Например, \flag{vimgrep} и \flag{json}.
.
.PP
Возможные значения для этого флага:
.sp
.IP \fBnever\fP 10n
Цвета никогда не будут использоваться.
.sp
.IP \fBauto\fP 10n
По умолчанию. ripgrep пытается быть умным.
.sp
.IP \fBalways\fP 10n
Цвета всегда будут использоваться независимо от того, куда отправляется вывод.
.sp
.IP \fBansi\fP 10n
Как 'always', но испускает ANSI-последовательности (даже в консоли Windows).
.
.PP
Этот флаг также управляет тем, испускаются ли гиперссылки. Например, когда
указан формат гиперссылок, гиперссылки не будут использоваться, когда цвет
подавлен. Если вы хотите испускать гиперссылки, но без цветов, то вы должны
использовать флаг \flag{colors}, чтобы вручную установить все стили цвета в
\fBnone\fP:
.sp
.EX
    \-\-colors 'path:none' \\
    \-\-colors 'line:none' \\
    \-\-colors 'column:none' \\
    \-\-colors 'match:none' \\
    \-\-colors 'highlight:none'
.EE
.sp
"
    }
    fn doc_choices(&self) -> &'static [&'static str] {
        &["never", "auto", "always", "ansi"]
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.color = match convert::str(&v.unwrap_value())? {
            "never" => ColorChoice::Never,
            "auto" => ColorChoice::Auto,
            "always" => ColorChoice::Always,
            "ansi" => ColorChoice::Ansi,
            unk => anyhow::bail!("choice '{unk}' is unrecognized"),
        };
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_color() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(ColorChoice::Auto, args.color);

    let args = parse_low_raw(["--color", "never"]).unwrap();
    assert_eq!(ColorChoice::Never, args.color);

    let args = parse_low_raw(["--color", "auto"]).unwrap();
    assert_eq!(ColorChoice::Auto, args.color);

    let args = parse_low_raw(["--color", "always"]).unwrap();
    assert_eq!(ColorChoice::Always, args.color);

    let args = parse_low_raw(["--color", "ansi"]).unwrap();
    assert_eq!(ColorChoice::Ansi, args.color);

    let args = parse_low_raw(["--color=never"]).unwrap();
    assert_eq!(ColorChoice::Never, args.color);

    let args =
        parse_low_raw(["--color", "always", "--color", "never"]).unwrap();
    assert_eq!(ColorChoice::Never, args.color);

    let args =
        parse_low_raw(["--color", "never", "--color", "always"]).unwrap();
    assert_eq!(ColorChoice::Always, args.color);

    let result = parse_low_raw(["--color", "foofoo"]);
    assert!(result.is_err(), "{result:?}");

    let result = parse_low_raw(["--color", "Always"]);
    assert!(result.is_err(), "{result:?}");
}

/// --colors
#[derive(Debug)]
struct Colors;

impl Flag for Colors {
    fn is_switch(&self) -> bool {
        false
    }
    fn name_long(&self) -> &'static str {
        "colors"
    }
    fn doc_variable(&self) -> Option<&'static str> {
        Some("COLOR_SPEC")
    }
    fn doc_category(&self) -> Category {
        Category::Output
    }
    fn doc_short(&self) -> &'static str {
        "Настроить цветовые настройки и стили."
    }
    fn doc_long(&self) -> &'static str {
        r#"
Этот флаг задаёт цветовые настройки для использования в выводе. Этот флаг может
быть предоставлен несколько раз. Настройки применяются итеративно. Существующие
цветовые метки ограничены одним из восьми вариантов: \fBred\fP, \fBblue\fP,
\fBgreen\fP, \fBcyan\fP, \fBmagenta\fP, \fByellow\fP, \fBwhite\fP и \fBblack\fP.
Стили ограничены \fBnobold\fP, \fBbold\fP, \fBnointense\fP, \fBintense\fP,
\fBnounderline\fP, \fBunderline\fP, \fBnoitalic\fP или \fBitalic\fP.
.sp
Формат флага:
\fB{\fP\fItype\fP\fB}:{\fP\fIattribute\fP\fB}:{\fP\fIvalue\fP\fB}\fP.
\fItype\fP должен быть одним из \fBpath\fP, \fBline\fP, \fBcolumn\fP,
\fBhighlight\fP или \fBmatch\fP. \fIattribute\fP может быть \fBfg\fP, \fBbg\fP
или \fBstyle\fP. \fIvalue\fP — это либо цвет (для \fBfg\fP и \fBbg\fP), либо
текстовый стиль. Специальный формат, \fB{\fP\fItype\fP\fB}:none\fP, очистит все
настройки цвета для \fItype\fP.
.sp
Например, следующая команда изменит цвет совпадения на пурпурный, а цвет фона
для номеров строк на жёлтый:
.sp
.EX
    rg \-\-colors 'match:fg:magenta' \-\-colors 'line:bg:yellow'
.EE
.sp
Другой пример, следующая команда «подсветит» несовпадающий текст в совпадающих
строках:
.sp
.EX
    rg \-\-colors 'highlight:bg:yellow' \-\-colors 'highlight:fg:black'
.EE
.sp
Тип цвета «highlight» особенно полезен для контрастирования совпадающих строк с
окружающим контекстом, напечатанным флагами \flag{before-context},
\flag{after-context}, \flag{context} или \flag{passthru}.
.sp
Расширенные цвета могут использоваться для \fIvalue\fP, когда tty поддерживает
ANSI-цветовые последовательности. Они указываются как \fIx\fP (256 цветов) или
.IB x , x , x
(24-битный truecolor), где \fIx\fP — число от \fB0\fP до \fB255\fP включительно.
\fIx\fP может быть задано как обычное десятичное число или шестнадцатеричное
число, которое имеет префикс \fB0x\fP.
.sp
Например, следующая команда изменит цвет фона совпадения на представленный
rgb-значением (0,128,255):
.sp
.EX
    rg \-\-colors 'match:bg:0,128,255'
.EE
.sp
или, эквивалентно,
.sp
.EX
    rg \-\-colors 'match:bg:0x0,0x80,0xFF'
.EE
.sp
Обратите внимание, что стили \fBintense\fP и \fBnointense\fP не будут иметь
эффекта при использовании вместе с этими расширенными цветовыми кодами.
"#
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        let v = v.unwrap_value();
        let v = convert::str(&v)?;
        args.colors.push(v.parse()?);
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_colors() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert!(args.colors.is_empty());

    let args = parse_low_raw(["--colors", "match:fg:magenta"]).unwrap();
    assert_eq!(args.colors, vec!["match:fg:magenta".parse().unwrap()]);

    let args = parse_low_raw([
        "--colors",
        "match:fg:magenta",
        "--colors",
        "line:bg:yellow",
    ])
    .unwrap();
    assert_eq!(
        args.colors,
        vec![
            "match:fg:magenta".parse().unwrap(),
            "line:bg:yellow".parse().unwrap()
        ]
    );

    let args = parse_low_raw(["--colors", "highlight:bg:240"]).unwrap();
    assert_eq!(args.colors, vec!["highlight:bg:240".parse().unwrap()]);

    let args = parse_low_raw([
        "--colors",
        "match:fg:magenta",
        "--colors",
        "highlight:bg:blue",
    ])
    .unwrap();
    assert_eq!(
        args.colors,
        vec![
            "match:fg:magenta".parse().unwrap(),
            "highlight:bg:blue".parse().unwrap()
        ]
    );
}

/// --column
#[derive(Debug)]
struct Column;

impl Flag for Column {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "column"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("no-column")
    }
    fn doc_category(&self) -> Category {
        Category::Output
    }
    fn doc_short(&self) -> &'static str {
        "Показать номера столбцов."
    }
    fn doc_long(&self) -> &'static str {
        r"
Показать номера столбцов (1-основанные). Это показывает номера столбцов только
для первого совпадения в каждой строке. Это не пытается учитывать Unicode.
Один байт равен одному столбцу. Это подразумевает \flag{line-number}.
.sp
Когда используется \flag{only-matching}, записанные номера столбцов соответствуют
началу каждого совпадения.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.column = Some(v.unwrap_switch());
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_column() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(None, args.column);

    let args = parse_low_raw(["--column"]).unwrap();
    assert_eq!(Some(true), args.column);

    let args = parse_low_raw(["--column", "--no-column"]).unwrap();
    assert_eq!(Some(false), args.column);

    let args = parse_low_raw(["--no-column", "--column"]).unwrap();
    assert_eq!(Some(true), args.column);
}

/// -C/--context
#[derive(Debug)]
struct Context;

impl Flag for Context {
    fn is_switch(&self) -> bool {
        false
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'C')
    }
    fn name_long(&self) -> &'static str {
        "context"
    }
    fn doc_variable(&self) -> Option<&'static str> {
        Some("NUM")
    }
    fn doc_category(&self) -> Category {
        Category::Output
    }
    fn doc_short(&self) -> &'static str {
        r"Показать NUM строк до и после каждого совпадения."
    }
    fn doc_long(&self) -> &'static str {
        r"
Показать \fINUM\fP строк до и после каждого совпадения. Это эквивалентно
предоставлению обоих флагов \flag{before-context} и \flag{after-context} с
одинаковым значением.
.sp
Это переопределяет флаг \flag{passthru}. Флаги \flag{after-context} и
\flag{before-context} оба частично переопределяют этот флаг, независимо от
порядка. Например, \fB\-A2 \-C1\fP эквивалентно \fB\-A2 \-B1\fP.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.context.set_both(convert::usize(&v.unwrap_value())?);
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_context() {
    let mkctx = |lines| {
        let mut mode = ContextMode::default();
        mode.set_both(lines);
        mode
    };

    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(ContextMode::default(), args.context);

    let args = parse_low_raw(["--context", "5"]).unwrap();
    assert_eq!(mkctx(5), args.context);

    let args = parse_low_raw(["--context=5"]).unwrap();
    assert_eq!(mkctx(5), args.context);

    let args = parse_low_raw(["-C", "5"]).unwrap();
    assert_eq!(mkctx(5), args.context);

    let args = parse_low_raw(["-C5"]).unwrap();
    assert_eq!(mkctx(5), args.context);

    let args = parse_low_raw(["-C5", "-C10"]).unwrap();
    assert_eq!(mkctx(10), args.context);

    let args = parse_low_raw(["-C5", "-C0"]).unwrap();
    assert_eq!(mkctx(0), args.context);

    let args = parse_low_raw(["-C5", "--passthru"]).unwrap();
    assert_eq!(ContextMode::Passthru, args.context);

    let args = parse_low_raw(["--passthru", "-C5"]).unwrap();
    assert_eq!(mkctx(5), args.context);

    let n = usize::MAX.to_string();
    let args = parse_low_raw(["--context", n.as_str()]).unwrap();
    assert_eq!(mkctx(usize::MAX), args.context);

    #[cfg(target_pointer_width = "64")]
    {
        let n = (u128::from(u64::MAX) + 1).to_string();
        let result = parse_low_raw(["--context", n.as_str()]);
        assert!(result.is_err(), "{result:?}");
    }

    // Test the interaction between -A/-B and -C. Basically, -A/-B always
    // partially overrides -C, regardless of where they appear relative to
    // each other. This behavior is also how GNU grep works, and it also makes
    // logical sense to me: -A/-B are the more specific flags.
    let args = parse_low_raw(["-A1", "-C5"]).unwrap();
    let mut mode = ContextMode::default();
    mode.set_after(1);
    mode.set_both(5);
    assert_eq!(mode, args.context);
    assert_eq!((5, 1), args.context.get_limited());

    let args = parse_low_raw(["-B1", "-C5"]).unwrap();
    let mut mode = ContextMode::default();
    mode.set_before(1);
    mode.set_both(5);
    assert_eq!(mode, args.context);
    assert_eq!((1, 5), args.context.get_limited());

    let args = parse_low_raw(["-A1", "-B2", "-C5"]).unwrap();
    let mut mode = ContextMode::default();
    mode.set_before(2);
    mode.set_after(1);
    mode.set_both(5);
    assert_eq!(mode, args.context);
    assert_eq!((2, 1), args.context.get_limited());

    // These next three are like the ones above, but with -C before -A/-B. This
    // tests that -A and -B only partially override -C. That is, -C1 -A2 is
    // equivalent to -B1 -A2.
    let args = parse_low_raw(["-C5", "-A1"]).unwrap();
    let mut mode = ContextMode::default();
    mode.set_after(1);
    mode.set_both(5);
    assert_eq!(mode, args.context);
    assert_eq!((5, 1), args.context.get_limited());

    let args = parse_low_raw(["-C5", "-B1"]).unwrap();
    let mut mode = ContextMode::default();
    mode.set_before(1);
    mode.set_both(5);
    assert_eq!(mode, args.context);
    assert_eq!((1, 5), args.context.get_limited());

    let args = parse_low_raw(["-C5", "-A1", "-B2"]).unwrap();
    let mut mode = ContextMode::default();
    mode.set_before(2);
    mode.set_after(1);
    mode.set_both(5);
    assert_eq!(mode, args.context);
    assert_eq!((2, 1), args.context.get_limited());
}

/// --context-separator
#[derive(Debug)]
struct ContextSeparator;

impl Flag for ContextSeparator {
    fn is_switch(&self) -> bool {
        false
    }
    fn name_long(&self) -> &'static str {
        "context-separator"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("no-context-separator")
    }
    fn doc_variable(&self) -> Option<&'static str> {
        Some("SEPARATOR")
    }
    fn doc_category(&self) -> Category {
        Category::Output
    }
    fn doc_short(&self) -> &'static str {
        r"Установить разделитель для контекстных блоков."
    }
    fn doc_long(&self) -> &'static str {
        r"
Строка, используемая для разделения неслияных контекстных строк в выводе. Это
используется только когда один из контекстных флагов используется (то есть,
\flag{after-context}, \flag{before-context} или \flag{context}). Последовательности
экранирования, такие как \fB\\x7F\fP или \fB\\t\fP, могут быть использованы.
Значение по умолчанию — \fB\-\-\fP.
.sp
Когда разделитель контекста установлен в пустую строку, разрыв строки всё ещё
вставляется. Чтобы полностью отключить разделители контекста, используйте флаг
\flag-negate{context-separator}.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        use crate::flags::lowargs::ContextSeparator as Separator;

        args.context_separator = match v {
            FlagValue::Switch(true) => {
                unreachable!("flag can only be disabled")
            }
            FlagValue::Switch(false) => Separator::disabled(),
            FlagValue::Value(v) => Separator::new(&v)?,
        };
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_context_separator() {
    use bstr::BString;

    use crate::flags::lowargs::ContextSeparator as Separator;

    let getbytes = |ctxsep: Separator| ctxsep.into_bytes().map(BString::from);

    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(Some(BString::from("--")), getbytes(args.context_separator));

    let args = parse_low_raw(["--context-separator", "XYZ"]).unwrap();
    assert_eq!(Some(BString::from("XYZ")), getbytes(args.context_separator));

    let args = parse_low_raw(["--no-context-separator"]).unwrap();
    assert_eq!(None, getbytes(args.context_separator));

    let args = parse_low_raw([
        "--context-separator",
        "XYZ",
        "--no-context-separator",
    ])
    .unwrap();
    assert_eq!(None, getbytes(args.context_separator));

    let args = parse_low_raw([
        "--no-context-separator",
        "--context-separator",
        "XYZ",
    ])
    .unwrap();
    assert_eq!(Some(BString::from("XYZ")), getbytes(args.context_separator));

    // This checks that invalid UTF-8 can be used. This case isn't too tricky
    // to handle, because it passes the invalid UTF-8 as an escape sequence
    // that is itself valid UTF-8. It doesn't become invalid UTF-8 until after
    // the argument is parsed and then unescaped.
    let args = parse_low_raw(["--context-separator", r"\xFF"]).unwrap();
    assert_eq!(Some(BString::from(b"\xFF")), getbytes(args.context_separator));

    // In this case, we specifically try to pass an invalid UTF-8 argument to
    // the flag. In theory we might be able to support this, but because we do
    // unescaping and because unescaping wants valid UTF-8, we do a UTF-8 check
    // on the value. Since we pass invalid UTF-8, it fails. This demonstrates
    // that the only way to use an invalid UTF-8 separator is by specifying an
    // escape sequence that is itself valid UTF-8.
    #[cfg(unix)]
    {
        use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

        let result = parse_low_raw([
            OsStr::from_bytes(b"--context-separator"),
            OsStr::from_bytes(&[0xFF]),
        ]);
        assert!(result.is_err(), "{result:?}");
    }
}

/// -c/--count
#[derive(Debug)]
struct Count;

impl Flag for Count {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'c')
    }
    fn name_long(&self) -> &'static str {
        "count"
    }
    fn doc_category(&self) -> Category {
        Category::OutputModes
    }
    fn doc_short(&self) -> &'static str {
        r"Показать количество совпадающих строк для каждого файла."
    }
    fn doc_long(&self) -> &'static str {
        r"
Этот флаг подавляет обычный вывод и показывает количество строк, которые
соответствуют заданным шаблонам для каждого искомого файла. Каждый файл,
содержащий совпадение, выводит свой путь и количество в каждой строке.
Обратите внимание, что если не включён \flag{multiline} и заданные шаблоны
не могут соответствовать нескольким строкам, это сообщает количество строк,
которые соответствуют, а не общее количество совпадений. Когда режим
многострочного поиска включён и заданные шаблоны могут соответствовать
нескольким строкам, \flag{count} эквивалентен \flag{count-matches}.
.sp
Если в ripgrep передан только один файл, то выводится только количество при
наличии совпадения. Флаг \flag{with-filename} может быть использован для
принудительного вывода пути к файлу в этом случае. Если вам нужно, чтобы
количество выводилось независимо от наличия совпадения, используйте
\flag{include-zero}.
.sp
Обратите внимание, что возможно, что этот флаг будет иметь результаты,
несогласованные с выводом \flag{files-with-matches}. В частности, по умолчанию
ripgrep пытается избежать поиска файлов с бинарными данными. С этим флагом
ripgrep должен искать всё содержимое файлов, которое может включать бинарные
данные. Но с \flag{files-with-matches} ripgrep может остановиться, как только
совпадение обнаружено, что может произойти задолго до любых бинарных данных.
Чтобы избежать этой несогласованности без отключения бинарного обнаружения,
используйте флаг \flag{binary}.
.sp
Это переопределяет флаг \flag{count-matches}. Обратите внимание, что когда
\flag{count} используется вместе с \flag{only-matching}, ripgrep ведёт себя
так, как будто был предоставлен \flag{count-matches}.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        assert!(v.unwrap_switch(), "--count can only be enabled");
        args.mode.update(Mode::Search(SearchMode::Count));
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_count() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(Mode::Search(SearchMode::Standard), args.mode);

    let args = parse_low_raw(["--count"]).unwrap();
    assert_eq!(Mode::Search(SearchMode::Count), args.mode);

    let args = parse_low_raw(["-c"]).unwrap();
    assert_eq!(Mode::Search(SearchMode::Count), args.mode);

    let args = parse_low_raw(["--count-matches", "--count"]).unwrap();
    assert_eq!(Mode::Search(SearchMode::Count), args.mode);

    let args = parse_low_raw(["--count-matches", "-c"]).unwrap();
    assert_eq!(Mode::Search(SearchMode::Count), args.mode);
}

/// --count-matches
#[derive(Debug)]
struct CountMatches;

impl Flag for CountMatches {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "count-matches"
    }
    fn doc_variable(&self) -> Option<&'static str> {
        None
    }
    fn doc_category(&self) -> Category {
        Category::OutputModes
    }
    fn doc_short(&self) -> &'static str {
        r"Показать количество каждого совпадения для каждого файла."
    }
    fn doc_long(&self) -> &'static str {
        r"
Этот флаг подавляет обычный вывод и показывает количество отдельных совпадений
заданных шаблонов для каждого искомого файла. Каждый файл, содержащий совпадения,
выводит свой путь и количество совпадений в каждой строке. Обратите внимание,
что это сообщает общее количество отдельных совпадений, а не количество строк,
которые соответствуют.
.sp
Если в ripgrep передан только один файл, то выводится только количество при
наличии совпадения. Флаг \flag{with-filename} может быть использован для
принудительного вывода пути к файлу в этом случае.
.sp
Это переопределяет флаг \flag{count}. Обратите внимание, что когда \flag{count}
используется вместе с \flag{only-matching}, ripgrep ведёт себя так, как будто
был предоставлен \flag{count-matches}.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        assert!(v.unwrap_switch(), "--count-matches can only be enabled");
        args.mode.update(Mode::Search(SearchMode::CountMatches));
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_count_matches() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(Mode::Search(SearchMode::Standard), args.mode);

    let args = parse_low_raw(["--count-matches"]).unwrap();
    assert_eq!(Mode::Search(SearchMode::CountMatches), args.mode);

    let args = parse_low_raw(["--count", "--count-matches"]).unwrap();
    assert_eq!(Mode::Search(SearchMode::CountMatches), args.mode);

    let args = parse_low_raw(["-c", "--count-matches"]).unwrap();
    assert_eq!(Mode::Search(SearchMode::CountMatches), args.mode);
}

/// --crlf
#[derive(Debug)]
struct Crlf;

impl Flag for Crlf {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "crlf"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("no-crlf")
    }
    fn doc_category(&self) -> Category {
        Category::Search
    }
    fn doc_short(&self) -> &'static str {
        r"Использовать CRLF-терминаторы строк (удобно для Windows)."
    }
    fn doc_long(&self) -> &'static str {
        r"
Когда включено, ripgrep будет рассматривать CRLF (\fB\\r\\n\fP) как терминатор
строки вместо только \fB\\n\fP.
.sp
В основном, это позволяет якорным утверждениям строк \fB^\fP и \fB$\fP в
шаблонах регулярных выражений рассматривать CRLF, CR или LF как терминаторы
строк вместо только LF. Обратите внимание, что они никогда не будут
соответствовать между CR и LF. CRLF рассматривается как один терминатор строки.
.sp
При использовании движка регулярных выражений по умолчанию поддержка CRLF
также может быть включена внутри шаблона с помощью флага \fBR\fP. Например,
\fB(?R:$)\fP будет соответствовать только перед CR или LF, но никогда между
CR и LF.
.sp
Этот флаг переопределяет \flag{null-data}.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.crlf = v.unwrap_switch();
        if args.crlf {
            args.null_data = false;
        }
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_crlf() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(false, args.crlf);

    let args = parse_low_raw(["--crlf"]).unwrap();
    assert_eq!(true, args.crlf);
    assert_eq!(false, args.null_data);

    let args = parse_low_raw(["--crlf", "--null-data"]).unwrap();
    assert_eq!(false, args.crlf);
    assert_eq!(true, args.null_data);

    let args = parse_low_raw(["--null-data", "--crlf"]).unwrap();
    assert_eq!(true, args.crlf);
    assert_eq!(false, args.null_data);

    let args = parse_low_raw(["--null-data", "--no-crlf"]).unwrap();
    assert_eq!(false, args.crlf);
    assert_eq!(true, args.null_data);

    let args = parse_low_raw(["--null-data", "--crlf", "--no-crlf"]).unwrap();
    assert_eq!(false, args.crlf);
    assert_eq!(false, args.null_data);
}

/// --debug
#[derive(Debug)]
struct Debug;

impl Flag for Debug {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "debug"
    }
    fn doc_category(&self) -> Category {
        Category::Logging
    }
    fn doc_short(&self) -> &'static str {
        r"Показать отладочные сообщения."
    }
    fn doc_long(&self) -> &'static str {
        r"
Показать отладочные сообщения. Пожалуйста, используйте это при отправке
отчёта об ошибке.
.sp
Флаг \flag{debug} обычно полезен для выяснения того, почему ripgrep пропустил
поиск определённого файла. Отладочные сообщения должны упоминать все пропущенные
файлы и причину их пропуска.
.sp
Чтобы получить ещё больше отладочного вывода, используйте флаг \flag{trace},
который подразумевает \flag{debug} вместе с дополнительными трассировочными
данными.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        assert!(v.unwrap_switch(), "--debug can only be enabled");
        args.logging = Some(LoggingMode::Debug);
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_debug() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(None, args.logging);

    let args = parse_low_raw(["--debug"]).unwrap();
    assert_eq!(Some(LoggingMode::Debug), args.logging);

    let args = parse_low_raw(["--trace", "--debug"]).unwrap();
    assert_eq!(Some(LoggingMode::Debug), args.logging);
}

/// --dfa-size-limit
#[derive(Debug)]
struct DfaSizeLimit;

impl Flag for DfaSizeLimit {
    fn is_switch(&self) -> bool {
        false
    }
    fn name_long(&self) -> &'static str {
        "dfa-size-limit"
    }
    fn doc_variable(&self) -> Option<&'static str> {
        Some("NUM+SUFFIX?")
    }
    fn doc_category(&self) -> Category {
        Category::Search
    }
    fn doc_short(&self) -> &'static str {
        r"Верхний предел размера DFA регулярного выражения."
    }
    fn doc_long(&self) -> &'static str {
        r"
Верхний предел размера DFA регулярного выражения. Предел по умолчанию довольно
щедрый для любого отдельного шаблона или для многих небольших шаблонов. Это
следует изменять только при очень больших вводах регулярных выражений, где
(более медленный) резервный движок регулярных выражений может иначе
использоваться, если предел достигнут.
.sp
Формат ввода принимает суффиксы \fBK\fP, \fBM\fP или \fBG\fP, которые
соответствуют килобайтам, мегабайтам и гигабайтам соответственно. Если суффикс
не предоставлен, ввод рассматривается как байты.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        let v = v.unwrap_value();
        args.dfa_size_limit = Some(convert::human_readable_usize(&v)?);
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_dfa_size_limit() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(None, args.dfa_size_limit);

    #[cfg(target_pointer_width = "64")]
    {
        let args = parse_low_raw(["--dfa-size-limit", "9G"]).unwrap();
        assert_eq!(Some(9 * (1 << 30)), args.dfa_size_limit);

        let args = parse_low_raw(["--dfa-size-limit=9G"]).unwrap();
        assert_eq!(Some(9 * (1 << 30)), args.dfa_size_limit);

        let args =
            parse_low_raw(["--dfa-size-limit=9G", "--dfa-size-limit=0"])
                .unwrap();
        assert_eq!(Some(0), args.dfa_size_limit);
    }

    let args = parse_low_raw(["--dfa-size-limit=0K"]).unwrap();
    assert_eq!(Some(0), args.dfa_size_limit);

    let args = parse_low_raw(["--dfa-size-limit=0M"]).unwrap();
    assert_eq!(Some(0), args.dfa_size_limit);

    let args = parse_low_raw(["--dfa-size-limit=0G"]).unwrap();
    assert_eq!(Some(0), args.dfa_size_limit);

    let result = parse_low_raw(["--dfa-size-limit", "9999999999999999999999"]);
    assert!(result.is_err(), "{result:?}");

    let result = parse_low_raw(["--dfa-size-limit", "9999999999999999G"]);
    assert!(result.is_err(), "{result:?}");
}

/// -E/--encoding
#[derive(Debug)]
struct Encoding;

impl Flag for Encoding {
    fn is_switch(&self) -> bool {
        false
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'E')
    }
    fn name_long(&self) -> &'static str {
        "encoding"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("no-encoding")
    }
    fn doc_variable(&self) -> Option<&'static str> {
        Some("ENCODING")
    }
    fn doc_category(&self) -> Category {
        Category::Search
    }
    fn doc_short(&self) -> &'static str {
        r"Указать кодировку текста файлов для поиска."
    }
    fn doc_long(&self) -> &'static str {
        r"
Указать кодировку текста, которую ripgrep будет использовать для всех
искомых файлов. Значение по умолчанию — \fBauto\fP, что заставит ripgrep
предпринять наилучшую попытку автоматического обнаружения кодировки для
каждого файла. Автоматическое обнаружение в этом случае применяется только
к файлам, которые начинаются с UTF-8 или UTF-16 метки порядка байтов (BOM).
Никакое другое автоматическое обнаружение не выполняется. Можно также указать
\fBnone\fP, что полностью отключит проверку BOM и всегда приведёт к поиску
сырых байтов, включая BOM, если он присутствует, независимо от его кодировки.
.sp
Другие поддерживаемые значения можно найти в списке меток здесь:
\fIhttps://encoding.spec.whatwg.org/#concept-encoding-get\fP.
.sp
Дополнительные сведения о кодировке и о том, как ripgrep работает с ней, см.
в \fBGUIDE.md\fP.
.sp
Обнаружение кодировки, которое использует ripgrep, может быть возвращено в
автоматический режим с помощью флага \flag-negate{encoding}.
"
    }
    fn completion_type(&self) -> CompletionType {
        CompletionType::Encoding
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        let value = match v {
            FlagValue::Value(v) => v,
            FlagValue::Switch(true) => {
                unreachable!("--encoding must accept a value")
            }
            FlagValue::Switch(false) => {
                args.encoding = EncodingMode::Auto;
                return Ok(());
            }
        };
        let label = convert::str(&value)?;
        args.encoding = match label {
            "auto" => EncodingMode::Auto,
            "none" => EncodingMode::Disabled,
            _ => EncodingMode::Some(grep::searcher::Encoding::new(label)?),
        };
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_encoding() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(EncodingMode::Auto, args.encoding);

    let args = parse_low_raw(["--encoding", "auto"]).unwrap();
    assert_eq!(EncodingMode::Auto, args.encoding);

    let args = parse_low_raw(["--encoding", "none"]).unwrap();
    assert_eq!(EncodingMode::Disabled, args.encoding);

    let args = parse_low_raw(["--encoding=none"]).unwrap();
    assert_eq!(EncodingMode::Disabled, args.encoding);

    let args = parse_low_raw(["-E", "none"]).unwrap();
    assert_eq!(EncodingMode::Disabled, args.encoding);

    let args = parse_low_raw(["-Enone"]).unwrap();
    assert_eq!(EncodingMode::Disabled, args.encoding);

    let args = parse_low_raw(["-E", "none", "--no-encoding"]).unwrap();
    assert_eq!(EncodingMode::Auto, args.encoding);

    let args = parse_low_raw(["--no-encoding", "-E", "none"]).unwrap();
    assert_eq!(EncodingMode::Disabled, args.encoding);

    let args = parse_low_raw(["-E", "utf-16"]).unwrap();
    let enc = grep::searcher::Encoding::new("utf-16").unwrap();
    assert_eq!(EncodingMode::Some(enc), args.encoding);

    let args = parse_low_raw(["-E", "utf-16", "--no-encoding"]).unwrap();
    assert_eq!(EncodingMode::Auto, args.encoding);

    let result = parse_low_raw(["-E", "foo"]);
    assert!(result.is_err(), "{result:?}");
}

/// --engine
#[derive(Debug)]
struct Engine;

impl Flag for Engine {
    fn is_switch(&self) -> bool {
        false
    }
    fn name_long(&self) -> &'static str {
        "engine"
    }
    fn doc_variable(&self) -> Option<&'static str> {
        Some("ENGINE")
    }
    fn doc_category(&self) -> Category {
        Category::Search
    }
    fn doc_short(&self) -> &'static str {
        r"Указать, какой движок регулярных выражений использовать."
    }
    fn doc_long(&self) -> &'static str {
        r"
Указать, какой движок регулярных выражений использовать. Когда вы выбираете
движок регулярных выражений, это применяется к каждому регулярному выражению,
предоставленному ripgrep (например, через несколько флагов \flag{regexp} или
\flag{file}).
.sp
Принимаемые значения: \fBdefault\fP, \fBpcre2\fP или \fBauto\fP.
.sp
Значение по умолчанию — \fBdefault\fP, что обычно быстрее всего и должно
подходить для большинства случаев использования. Движок \fBpcre2\fP обычно
полезен, когда вы хотите использовать такие функции, как просмотр окружения
или обратные ссылки. \fBauto\fP будет динамически выбирать между поддерживаемыми
движками регулярных выражений в зависимости от функций, используемых в шаблоне,
на наилучшей основе.
.sp
Обратите внимание, что движок \fBpcre2\fP — это дополнительная функция ripgrep.
Если PCRE2 не был включён в вашу сборку ripgrep, то использование этого флага
приведёт к тому, что ripgrep напечатает сообщение об ошибке и выйдет.
.sp
Это переопределяет предыдущие использования флагов \flag{pcre2} и
\flag{auto-hybrid-regex}.
"
    }
    fn doc_choices(&self) -> &'static [&'static str] {
        &["default", "pcre2", "auto"]
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        let v = v.unwrap_value();
        let string = convert::str(&v)?;
        args.engine = match string {
            "default" => EngineChoice::Default,
            "pcre2" => EngineChoice::PCRE2,
            "auto" => EngineChoice::Auto,
            _ => anyhow::bail!("unrecognized regex engine '{string}'"),
        };
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_engine() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(EngineChoice::Default, args.engine);

    let args = parse_low_raw(["--engine", "pcre2"]).unwrap();
    assert_eq!(EngineChoice::PCRE2, args.engine);

    let args = parse_low_raw(["--engine=pcre2"]).unwrap();
    assert_eq!(EngineChoice::PCRE2, args.engine);

    let args =
        parse_low_raw(["--auto-hybrid-regex", "--engine=pcre2"]).unwrap();
    assert_eq!(EngineChoice::PCRE2, args.engine);

    let args =
        parse_low_raw(["--engine=pcre2", "--auto-hybrid-regex"]).unwrap();
    assert_eq!(EngineChoice::Auto, args.engine);

    let args =
        parse_low_raw(["--auto-hybrid-regex", "--engine=auto"]).unwrap();
    assert_eq!(EngineChoice::Auto, args.engine);

    let args =
        parse_low_raw(["--auto-hybrid-regex", "--engine=default"]).unwrap();
    assert_eq!(EngineChoice::Default, args.engine);

    let args =
        parse_low_raw(["--engine=pcre2", "--no-auto-hybrid-regex"]).unwrap();
    assert_eq!(EngineChoice::Default, args.engine);
}

/// --field-context-separator
#[derive(Debug)]
struct FieldContextSeparator;

impl Flag for FieldContextSeparator {
    fn is_switch(&self) -> bool {
        false
    }
    fn name_long(&self) -> &'static str {
        "field-context-separator"
    }
    fn doc_variable(&self) -> Option<&'static str> {
        Some("SEPARATOR")
    }
    fn doc_category(&self) -> Category {
        Category::Output
    }
    fn doc_short(&self) -> &'static str {
        r"Установить разделитель контекста поля."
    }
    fn doc_long(&self) -> &'static str {
        r"
Установить разделитель контекста поля. Этот разделитель используется только
при печати контекстных строк. Он используется для разделения путей к файлам,
номеров строк, столбцов и самой контекстной строки. Разделитель может быть
любым количеством байтов, включая ноль. Могут быть использованы последовательности
экранирования, такие как \fB\\x7F\fP или \fB\\t\fP.
.sp
Символ \fB-\fP является значением по умолчанию.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        use crate::flags::lowargs::FieldContextSeparator as Separator;

        args.field_context_separator = Separator::new(&v.unwrap_value())?;
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_field_context_separator() {
    use bstr::BString;

    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(BString::from("-"), args.field_context_separator.into_bytes());

    let args = parse_low_raw(["--field-context-separator", "XYZ"]).unwrap();
    assert_eq!(
        BString::from("XYZ"),
        args.field_context_separator.into_bytes()
    );

    let args = parse_low_raw(["--field-context-separator=XYZ"]).unwrap();
    assert_eq!(
        BString::from("XYZ"),
        args.field_context_separator.into_bytes()
    );

    let args = parse_low_raw([
        "--field-context-separator",
        "XYZ",
        "--field-context-separator",
        "ABC",
    ])
    .unwrap();
    assert_eq!(
        BString::from("ABC"),
        args.field_context_separator.into_bytes()
    );

    let args = parse_low_raw(["--field-context-separator", r"\t"]).unwrap();
    assert_eq!(BString::from("\t"), args.field_context_separator.into_bytes());

    let args = parse_low_raw(["--field-context-separator", r"\x00"]).unwrap();
    assert_eq!(
        BString::from("\x00"),
        args.field_context_separator.into_bytes()
    );

    // This checks that invalid UTF-8 can be used. This case isn't too tricky
    // to handle, because it passes the invalid UTF-8 as an escape sequence
    // that is itself valid UTF-8. It doesn't become invalid UTF-8 until after
    // the argument is parsed and then unescaped.
    let args = parse_low_raw(["--field-context-separator", r"\xFF"]).unwrap();
    assert_eq!(
        BString::from(b"\xFF"),
        args.field_context_separator.into_bytes()
    );

    // In this case, we specifically try to pass an invalid UTF-8 argument to
    // the flag. In theory we might be able to support this, but because we do
    // unescaping and because unescaping wants valid UTF-8, we do a UTF-8 check
    // on the value. Since we pass invalid UTF-8, it fails. This demonstrates
    // that the only way to use an invalid UTF-8 separator is by specifying an
    // escape sequence that is itself valid UTF-8.
    #[cfg(unix)]
    {
        use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

        let result = parse_low_raw([
            OsStr::from_bytes(b"--field-context-separator"),
            OsStr::from_bytes(&[0xFF]),
        ]);
        assert!(result.is_err(), "{result:?}");
    }
}

/// --field-match-separator
#[derive(Debug)]
struct FieldMatchSeparator;

impl Flag for FieldMatchSeparator {
    fn is_switch(&self) -> bool {
        false
    }
    fn name_long(&self) -> &'static str {
        "field-match-separator"
    }
    fn doc_variable(&self) -> Option<&'static str> {
        Some("SEPARATOR")
    }
    fn doc_category(&self) -> Category {
        Category::Output
    }
    fn doc_short(&self) -> &'static str {
        r"Установить разделитель совпадения поля."
    }
    fn doc_long(&self) -> &'static str {
        r"
Установить разделитель совпадения поля. Этот разделитель используется только
при печати совпадающих строк. Он используется для разделения путей к файлам,
номеров строк, столбцов и самой совпадающей строки. Разделитель может быть
любым количеством байтов, включая ноль. Могут быть использованы последовательности
экранирования, такие как \fB\\x7F\fP или \fB\\t\fP.
.sp
Символ \fB:\fP является значением по умолчанию.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        use crate::flags::lowargs::FieldMatchSeparator as Separator;

        args.field_match_separator = Separator::new(&v.unwrap_value())?;
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_field_match_separator() {
    use bstr::BString;

    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(BString::from(":"), args.field_match_separator.into_bytes());

    let args = parse_low_raw(["--field-match-separator", "XYZ"]).unwrap();
    assert_eq!(BString::from("XYZ"), args.field_match_separator.into_bytes());

    let args = parse_low_raw(["--field-match-separator=XYZ"]).unwrap();
    assert_eq!(BString::from("XYZ"), args.field_match_separator.into_bytes());

    let args = parse_low_raw([
        "--field-match-separator",
        "XYZ",
        "--field-match-separator",
        "ABC",
    ])
    .unwrap();
    assert_eq!(BString::from("ABC"), args.field_match_separator.into_bytes());

    let args = parse_low_raw(["--field-match-separator", r"\t"]).unwrap();
    assert_eq!(BString::from("\t"), args.field_match_separator.into_bytes());

    let args = parse_low_raw(["--field-match-separator", r"\x00"]).unwrap();
    assert_eq!(BString::from("\x00"), args.field_match_separator.into_bytes());

    // This checks that invalid UTF-8 can be used. This case isn't too tricky
    // to handle, because it passes the invalid UTF-8 as an escape sequence
    // that is itself valid UTF-8. It doesn't become invalid UTF-8 until after
    // the argument is parsed and then unescaped.
    let args = parse_low_raw(["--field-match-separator", r"\xFF"]).unwrap();
    assert_eq!(
        BString::from(b"\xFF"),
        args.field_match_separator.into_bytes()
    );

    // In this case, we specifically try to pass an invalid UTF-8 argument to
    // the flag. In theory we might be able to support this, but because we do
    // unescaping and because unescaping wants valid UTF-8, we do a UTF-8 check
    // on the value. Since we pass invalid UTF-8, it fails. This demonstrates
    // that the only way to use an invalid UTF-8 separator is by specifying an
    // escape sequence that is itself valid UTF-8.
    #[cfg(unix)]
    {
        use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

        let result = parse_low_raw([
            OsStr::from_bytes(b"--field-match-separator"),
            OsStr::from_bytes(&[0xFF]),
        ]);
        assert!(result.is_err(), "{result:?}");
    }
}

/// -f/--file
#[derive(Debug)]
struct File;

impl Flag for File {
    fn is_switch(&self) -> bool {
        false
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'f')
    }
    fn name_long(&self) -> &'static str {
        "file"
    }
    fn doc_variable(&self) -> Option<&'static str> {
        Some("PATTERNFILE")
    }
    fn doc_category(&self) -> Category {
        Category::Input
    }
    fn doc_short(&self) -> &'static str {
        r"Search for patterns from the given file."
    }
    fn doc_long(&self) -> &'static str {
        r"
Search for patterns from the given file, with one pattern per line. When this
flag is used multiple times or in combination with the \flag{regexp} flag, then
all patterns provided are searched. Empty pattern lines will match all input
lines, and the newline is not counted as part of the pattern.
.sp
A line is printed if and only if it matches at least one of the patterns.
.sp
When \fIPATTERNFILE\fP is \fB-\fP, then \fBstdin\fP will be read for the
patterns.
.sp
When \flag{file} or \flag{regexp} is used, then ripgrep treats all positional
arguments as files or directories to search.
"
    }
    fn completion_type(&self) -> CompletionType {
        CompletionType::Filename
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        let path = PathBuf::from(v.unwrap_value());
        args.patterns.push(PatternSource::File(path));
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_file() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(Vec::<PatternSource>::new(), args.patterns);

    let args = parse_low_raw(["--file", "foo"]).unwrap();
    assert_eq!(vec![PatternSource::File(PathBuf::from("foo"))], args.patterns);

    let args = parse_low_raw(["--file=foo"]).unwrap();
    assert_eq!(vec![PatternSource::File(PathBuf::from("foo"))], args.patterns);

    let args = parse_low_raw(["-f", "foo"]).unwrap();
    assert_eq!(vec![PatternSource::File(PathBuf::from("foo"))], args.patterns);

    let args = parse_low_raw(["-ffoo"]).unwrap();
    assert_eq!(vec![PatternSource::File(PathBuf::from("foo"))], args.patterns);

    let args = parse_low_raw(["--file", "-foo"]).unwrap();
    assert_eq!(
        vec![PatternSource::File(PathBuf::from("-foo"))],
        args.patterns
    );

    let args = parse_low_raw(["--file=-foo"]).unwrap();
    assert_eq!(
        vec![PatternSource::File(PathBuf::from("-foo"))],
        args.patterns
    );

    let args = parse_low_raw(["-f", "-foo"]).unwrap();
    assert_eq!(
        vec![PatternSource::File(PathBuf::from("-foo"))],
        args.patterns
    );

    let args = parse_low_raw(["-f-foo"]).unwrap();
    assert_eq!(
        vec![PatternSource::File(PathBuf::from("-foo"))],
        args.patterns
    );

    let args = parse_low_raw(["--file=foo", "--file", "bar"]).unwrap();
    assert_eq!(
        vec![
            PatternSource::File(PathBuf::from("foo")),
            PatternSource::File(PathBuf::from("bar"))
        ],
        args.patterns
    );

    // We permit path arguments to be invalid UTF-8. So test that. Some of
    // these cases are tricky and depend on lexopt doing the right thing.
    //
    // We probably should add tests for this handling on Windows too, but paths
    // that are invalid UTF-16 appear incredibly rare in the Windows world.
    #[cfg(unix)]
    {
        use std::{
            ffi::{OsStr, OsString},
            os::unix::ffi::{OsStrExt, OsStringExt},
        };

        let bytes = &[b'A', 0xFF, b'Z'][..];
        let path = PathBuf::from(OsString::from_vec(bytes.to_vec()));

        let args = parse_low_raw([
            OsStr::from_bytes(b"--file"),
            OsStr::from_bytes(bytes),
        ])
        .unwrap();
        assert_eq!(vec![PatternSource::File(path.clone())], args.patterns);

        let args = parse_low_raw([
            OsStr::from_bytes(b"-f"),
            OsStr::from_bytes(bytes),
        ])
        .unwrap();
        assert_eq!(vec![PatternSource::File(path.clone())], args.patterns);

        let mut bytes = b"--file=A".to_vec();
        bytes.push(0xFF);
        bytes.push(b'Z');
        let args = parse_low_raw([OsStr::from_bytes(&bytes)]).unwrap();
        assert_eq!(vec![PatternSource::File(path.clone())], args.patterns);

        let mut bytes = b"-fA".to_vec();
        bytes.push(0xFF);
        bytes.push(b'Z');
        let args = parse_low_raw([OsStr::from_bytes(&bytes)]).unwrap();
        assert_eq!(vec![PatternSource::File(path.clone())], args.patterns);
    }
}

/// --files
#[derive(Debug)]
struct Files;

impl Flag for Files {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "files"
    }
    fn doc_category(&self) -> Category {
        Category::OtherBehaviors
    }
    fn doc_short(&self) -> &'static str {
        r"Вывести каждый файл, который будет искаться."
    }
    fn doc_long(&self) -> &'static str {
        r"
Вывести каждый файл, который будет искаться, без фактического выполнения поиска.
Это полезно для определения того, ищется ли определённый файл или нет.
.sp
Это переопределяет \flag{type-list}.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        assert!(v.unwrap_switch());
        args.mode.update(Mode::Files);
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_files() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(Mode::Search(SearchMode::Standard), args.mode);

    let args = parse_low_raw(["--files"]).unwrap();
    assert_eq!(Mode::Files, args.mode);
}

/// -l/--files-with-matches
#[derive(Debug)]
struct FilesWithMatches;

impl Flag for FilesWithMatches {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'l')
    }
    fn name_long(&self) -> &'static str {
        "files-with-matches"
    }
    fn doc_category(&self) -> Category {
        Category::OutputModes
    }
    fn doc_short(&self) -> &'static str {
        r"Вывести пути как минимум с одним совпадением."
    }
    fn doc_long(&self) -> &'static str {
        r"
Вывести только пути как минимум с одним совпадением и подавить содержимое
совпадений.
.sp
Обратите внимание, что возможно, что этот флаг будет иметь результаты,
несогласованные с выводом \flag{count}. В частности, по умолчанию ripgrep
пытается избежать поиска файлов с бинарными данными. С этим флагом ripgrep
может прекратить поиск до того, как бинарные данные будут обнаружены. Но с
\flag{count} ripgrep должен искать всё содержимое для определения количества
совпадений, что означает, что он может увидеть бинарные данные, которые
заставят его пропустить поиск этого файла. Чтобы избежать этой несогласованности
без отключения бинарного обнаружения, используйте флаг \flag{binary}.
.sp
Это переопределяет \flag{files-without-match}.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        assert!(v.unwrap_switch(), "--files-with-matches can only be enabled");
        args.mode.update(Mode::Search(SearchMode::FilesWithMatches));
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_files_with_matches() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(Mode::Search(SearchMode::Standard), args.mode);

    let args = parse_low_raw(["--files-with-matches"]).unwrap();
    assert_eq!(Mode::Search(SearchMode::FilesWithMatches), args.mode);

    let args = parse_low_raw(["-l"]).unwrap();
    assert_eq!(Mode::Search(SearchMode::FilesWithMatches), args.mode);
}

/// -l/--files-without-match
#[derive(Debug)]
struct FilesWithoutMatch;

impl Flag for FilesWithoutMatch {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "files-without-match"
    }
    fn doc_category(&self) -> Category {
        Category::OutputModes
    }
    fn doc_short(&self) -> &'static str {
        r"Вывести пути, которые содержат ноль совпадений."
    }
    fn doc_long(&self) -> &'static str {
        r"
Вывести пути, которые содержат ноль совпадений, и подавить содержимое совпадений.
.sp
Это переопределяет \flag{files-with-matches}.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        assert!(
            v.unwrap_switch(),
            "--files-without-match can only be enabled"
        );
        args.mode.update(Mode::Search(SearchMode::FilesWithoutMatch));
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_files_without_match() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(Mode::Search(SearchMode::Standard), args.mode);

    let args = parse_low_raw(["--files-without-match"]).unwrap();
    assert_eq!(Mode::Search(SearchMode::FilesWithoutMatch), args.mode);

    let args =
        parse_low_raw(["--files-with-matches", "--files-without-match"])
            .unwrap();
    assert_eq!(Mode::Search(SearchMode::FilesWithoutMatch), args.mode);

    let args =
        parse_low_raw(["--files-without-match", "--files-with-matches"])
            .unwrap();
    assert_eq!(Mode::Search(SearchMode::FilesWithMatches), args.mode);
}

/// -F/--fixed-strings
#[derive(Debug)]
struct FixedStrings;

impl Flag for FixedStrings {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'F')
    }
    fn name_long(&self) -> &'static str {
        "fixed-strings"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("no-fixed-strings")
    }
    fn doc_category(&self) -> Category {
        Category::Search
    }
    fn doc_short(&self) -> &'static str {
        r"Рассматривать все шаблоны как литералы."
    }
    fn doc_long(&self) -> &'static str {
        r"
Рассматривать все шаблоны как литералы вместо регулярных выражений. Когда этот
флаг используется, специальные мета-символы регулярных выражений, такие как
\fB.(){}*+\fP, не должны экранироваться.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.fixed_strings = v.unwrap_switch();
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_fixed_strings() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(false, args.fixed_strings);

    let args = parse_low_raw(["--fixed-strings"]).unwrap();
    assert_eq!(true, args.fixed_strings);

    let args = parse_low_raw(["-F"]).unwrap();
    assert_eq!(true, args.fixed_strings);

    let args = parse_low_raw(["-F", "--no-fixed-strings"]).unwrap();
    assert_eq!(false, args.fixed_strings);

    let args = parse_low_raw(["--no-fixed-strings", "-F"]).unwrap();
    assert_eq!(true, args.fixed_strings);
}

/// -L/--follow
#[derive(Debug)]
struct Follow;

impl Flag for Follow {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'L')
    }
    fn name_long(&self) -> &'static str {
        "follow"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("no-follow")
    }
    fn doc_category(&self) -> Category {
        Category::Filter
    }
    fn doc_short(&self) -> &'static str {
        r"Следовать по символическим ссылкам."
    }
    fn doc_long(&self) -> &'static str {
        r"
Этот флаг предписывает ripgrep следовать по символическим ссылкам при обходе
каталогов. Это поведение отключено по умолчанию. Обратите внимание, что ripgrep
будет проверять циклы символических ссылок и сообщать об ошибках, если найдёт
их. ripgrep также будет сообщать об ошибках для битых ссылок. Чтобы подавить
сообщения об ошибках, используйте флаг \flag{no-messages}.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.follow = v.unwrap_switch();
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_follow() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(false, args.follow);

    let args = parse_low_raw(["--follow"]).unwrap();
    assert_eq!(true, args.follow);

    let args = parse_low_raw(["-L"]).unwrap();
    assert_eq!(true, args.follow);

    let args = parse_low_raw(["-L", "--no-follow"]).unwrap();
    assert_eq!(false, args.follow);

    let args = parse_low_raw(["--no-follow", "-L"]).unwrap();
    assert_eq!(true, args.follow);
}

/// --generate
#[derive(Debug)]
struct Generate;

impl Flag for Generate {
    fn is_switch(&self) -> bool {
        false
    }
    fn name_long(&self) -> &'static str {
        "generate"
    }
    fn doc_variable(&self) -> Option<&'static str> {
        Some("KIND")
    }
    fn doc_category(&self) -> Category {
        Category::OtherBehaviors
    }
    fn doc_short(&self) -> &'static str {
        r"Сгенерировать man-страницы и скрипты автодополнения."
    }
    fn doc_long(&self) -> &'static str {
        r"
Этот флаг предписывает ripgrep сгенерировать некоторый специальный вид вывода,
определённый \fIKIND\fP, а затем выйти без поиска. \fIKIND\fP может быть одним
из следующих значений:
.sp
.TP 15
\fBman\fP
Генерирует страницу руководства для ripgrep в формате \fBroff\fP.
.TP 15
\fBcomplete\-bash\fP
Генерирует скрипт автодополнения для оболочки \fBbash\fP.
.TP 15
\fBcomplete\-zsh\fP
Генерирует скрипт автодополнения для оболочки \fBzsh\fP.
.TP 15
\fBcomplete\-fish\fP
Генерирует скрипт автодополнения для оболочки \fBfish\fP.
.TP 15
\fBcomplete\-powershell\fP
Генерирует скрипт автодополнения для PowerShell.
.PP
Вывод записывается в \fBstdout\fP. Список выше может расширяться со временем.
"
    }
    fn doc_choices(&self) -> &'static [&'static str] {
        &[
            "man",
            "complete-bash",
            "complete-zsh",
            "complete-fish",
            "complete-powershell",
        ]
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        let genmode = match convert::str(&v.unwrap_value())? {
            "man" => GenerateMode::Man,
            "complete-bash" => GenerateMode::CompleteBash,
            "complete-zsh" => GenerateMode::CompleteZsh,
            "complete-fish" => GenerateMode::CompleteFish,
            "complete-powershell" => GenerateMode::CompletePowerShell,
            unk => anyhow::bail!("choice '{unk}' is unrecognized"),
        };
        args.mode.update(Mode::Generate(genmode));
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_generate() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(Mode::Search(SearchMode::Standard), args.mode);

    let args = parse_low_raw(["--generate", "man"]).unwrap();
    assert_eq!(Mode::Generate(GenerateMode::Man), args.mode);

    let args = parse_low_raw(["--generate", "complete-bash"]).unwrap();
    assert_eq!(Mode::Generate(GenerateMode::CompleteBash), args.mode);

    let args = parse_low_raw(["--generate", "complete-zsh"]).unwrap();
    assert_eq!(Mode::Generate(GenerateMode::CompleteZsh), args.mode);

    let args = parse_low_raw(["--generate", "complete-fish"]).unwrap();
    assert_eq!(Mode::Generate(GenerateMode::CompleteFish), args.mode);

    let args = parse_low_raw(["--generate", "complete-powershell"]).unwrap();
    assert_eq!(Mode::Generate(GenerateMode::CompletePowerShell), args.mode);

    let args =
        parse_low_raw(["--generate", "complete-bash", "--generate=man"])
            .unwrap();
    assert_eq!(Mode::Generate(GenerateMode::Man), args.mode);

    let args = parse_low_raw(["--generate", "man", "-l"]).unwrap();
    assert_eq!(Mode::Search(SearchMode::FilesWithMatches), args.mode);

    // An interesting quirk of how the modes override each other that lets
    // you get back to the "default" mode of searching.
    let args =
        parse_low_raw(["--generate", "man", "--json", "--no-json"]).unwrap();
    assert_eq!(Mode::Search(SearchMode::Standard), args.mode);
}

/// -g/--glob
#[derive(Debug)]
struct Glob;

impl Flag for Glob {
    fn is_switch(&self) -> bool {
        false
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'g')
    }
    fn name_long(&self) -> &'static str {
        "glob"
    }
    fn doc_variable(&self) -> Option<&'static str> {
        Some("GLOB")
    }
    fn doc_category(&self) -> Category {
        Category::Filter
    }
    fn doc_short(&self) -> &'static str {
        r"Включить или исключить пути к файлам."
    }
    fn doc_long(&self) -> &'static str {
        r#"
Включить или исключить файлы и каталоги для поиска, которые соответствуют
заданному glob. Это всегда переопределяет любую другую логику игнорирования.
Может быть использовано несколько флагов glob. Правила glob сопоставляются с
глобами \fB.gitignore\fP. Предшествуйте glob символом \fB!\fP, чтобы исключить
его. Если несколько glob соответствуют файлу или каталогу, glob, указанный
позже в командной строке, имеет приоритет.
.sp
Как расширение, glob поддерживают указание альтернатив:
.BI "\-g '" ab{c,d}* '
эквивалентно
.BI "\-g " "abc " "\-g " abd.
Пустые альтернативы, такие как
.BI "\-g '" ab{,c} '
в настоящее время не поддерживаются. Обратите внимание, что это расширение
синтаксиса в настоящее время также включено в файлах \fBgitignore\fP, хотя
этот синтаксис не поддерживается самим git. ripgrep может отключить это
расширение синтаксиса в файлах gitignore, но оно всегда будет доступно через
флаг \flag{glob}.
.sp
Когда этот флаг установлен, каждый файл и каталог применяется к нему для
проверки соответствия. Например, если вы хотите искать только в определённом
каталоге \fIfoo\fP, то
.BI "\-g " foo
неверно, потому что \fIfoo/bar\fP не соответствует
glob \fIfoo\fP. Вместо этого вы должны использовать
.BI "\-g '" foo/** '.
"#
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        let glob = convert::string(v.unwrap_value())?;
        args.globs.push(glob);
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_glob() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(Vec::<String>::new(), args.globs);

    let args = parse_low_raw(["--glob", "foo"]).unwrap();
    assert_eq!(vec!["foo".to_string()], args.globs);

    let args = parse_low_raw(["--glob=foo"]).unwrap();
    assert_eq!(vec!["foo".to_string()], args.globs);

    let args = parse_low_raw(["-g", "foo"]).unwrap();
    assert_eq!(vec!["foo".to_string()], args.globs);

    let args = parse_low_raw(["-gfoo"]).unwrap();
    assert_eq!(vec!["foo".to_string()], args.globs);

    let args = parse_low_raw(["--glob", "-foo"]).unwrap();
    assert_eq!(vec!["-foo".to_string()], args.globs);

    let args = parse_low_raw(["--glob=-foo"]).unwrap();
    assert_eq!(vec!["-foo".to_string()], args.globs);

    let args = parse_low_raw(["-g", "-foo"]).unwrap();
    assert_eq!(vec!["-foo".to_string()], args.globs);

    let args = parse_low_raw(["-g-foo"]).unwrap();
    assert_eq!(vec!["-foo".to_string()], args.globs);
}

/// --glob-case-insensitive
#[derive(Debug)]
struct GlobCaseInsensitive;

impl Flag for GlobCaseInsensitive {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "glob-case-insensitive"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("no-glob-case-insensitive")
    }
    fn doc_category(&self) -> Category {
        Category::Filter
    }
    fn doc_short(&self) -> &'static str {
        r"Обрабатывать все шаблоны glob без учёта регистра."
    }
    fn doc_long(&self) -> &'static str {
        r"
Обрабатывать все шаблоны glob, предоставленные с флагом \flag{glob}, без учёта
регистра. Это фактически рассматривает \flag{glob} как \flag{iglob}.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.glob_case_insensitive = v.unwrap_switch();
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_glob_case_insensitive() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(false, args.glob_case_insensitive);

    let args = parse_low_raw(["--glob-case-insensitive"]).unwrap();
    assert_eq!(true, args.glob_case_insensitive);

    let args = parse_low_raw([
        "--glob-case-insensitive",
        "--no-glob-case-insensitive",
    ])
    .unwrap();
    assert_eq!(false, args.glob_case_insensitive);

    let args = parse_low_raw([
        "--no-glob-case-insensitive",
        "--glob-case-insensitive",
    ])
    .unwrap();
    assert_eq!(true, args.glob_case_insensitive);
}

/// --heading
#[derive(Debug)]
struct Heading;

impl Flag for Heading {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "heading"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("no-heading")
    }
    fn doc_category(&self) -> Category {
        Category::Output
    }
    fn doc_short(&self) -> &'static str {
        r"Вывести совпадения, сгруппированные по каждому файлу."
    }
    fn doc_long(&self) -> &'static str {
        r"
Этот флаг печатает путь к файлу над группами совпадений из каждого файла вместо
печати пути к файлу как префикса для каждой совпадающей строки.
.sp
Это режим по умолчанию при выводе в tty.
.sp
Когда \fBstdout\fP не является tty, ripgrep по умолчанию будет использовать
стандартный grep-подобный формат. Можно принудительно использовать этот формат
в Unix-подобных средах, передав вывод ripgrep в \fBcat\fP. Например,
\fBrg\fP \fIfoo\fP \fB| cat\fP.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.heading = Some(v.unwrap_switch());
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_heading() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(None, args.heading);

    let args = parse_low_raw(["--heading"]).unwrap();
    assert_eq!(Some(true), args.heading);

    let args = parse_low_raw(["--no-heading"]).unwrap();
    assert_eq!(Some(false), args.heading);

    let args = parse_low_raw(["--heading", "--no-heading"]).unwrap();
    assert_eq!(Some(false), args.heading);

    let args = parse_low_raw(["--no-heading", "--heading"]).unwrap();
    assert_eq!(Some(true), args.heading);
}

/// -h/--help
#[derive(Debug)]
struct Help;

impl Flag for Help {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "help"
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'h')
    }
    fn doc_category(&self) -> Category {
        Category::Output
    }
    fn doc_short(&self) -> &'static str {
        r"Показать справку."
    }
    fn doc_long(&self) -> &'static str {
        r"
Этот флаг печатает вывод справки для ripgrep.
.sp
В отличие от большинства других флагов, поведение короткого флага, \fB\-h\fP, и
длинного флага, \fB\-\-help\fP, различается. Короткий флаг покажет сжатый вывод
справки, в то время как длинный флаг покажет подробный вывод справки. Подробный
вывод справки имеет полную документацию, тогда как сжатый вывод справки покажет
только одну строку для каждого флага.
"
    }

    fn update(&self, v: FlagValue, _: &mut LowArgs) -> anyhow::Result<()> {
        assert!(v.unwrap_switch(), "--help has no negation");
        // Since this flag has different semantics for -h and --help and the
        // Flag trait doesn't support encoding this sort of thing, we handle it
        // as a special case in the parser.
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_help() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(None, args.special);

    let args = parse_low_raw(["-h"]).unwrap();
    assert_eq!(Some(SpecialMode::HelpShort), args.special);

    let args = parse_low_raw(["--help"]).unwrap();
    assert_eq!(Some(SpecialMode::HelpLong), args.special);

    let args = parse_low_raw(["-h", "--help"]).unwrap();
    assert_eq!(Some(SpecialMode::HelpLong), args.special);

    let args = parse_low_raw(["--help", "-h"]).unwrap();
    assert_eq!(Some(SpecialMode::HelpShort), args.special);
}

/// -./--hidden
#[derive(Debug)]
struct Hidden;

impl Flag for Hidden {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'.')
    }
    fn name_long(&self) -> &'static str {
        "hidden"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("no-hidden")
    }
    fn doc_category(&self) -> Category {
        Category::Filter
    }
    fn doc_short(&self) -> &'static str {
        r"Искать скрытые файлы и каталоги."
    }
    fn doc_long(&self) -> &'static str {
        r#"
Искать скрытые файлы и каталоги. По умолчанию скрытые файлы и каталоги
пропускаются. Обратите внимание, что если скрытый файл или каталог внесён в
белый список в файле игнорирования, то он будет искаться, даже если этот флаг
не предоставлен. Аналогично, если скрытый файл или каталог предоставлен явно
как аргумент для ripgrep.
.sp
Файл или каталог считается скрытым, если его базовое имя начинается с символа
точки (\fB.\fP). В операционных системах, которые поддерживают атрибут файла
«скрытый», таких как Windows, файлы с этим атрибутом также считаются скрытыми.
.sp
Обратите внимание, что \flag{hidden} будет включать файлы и папки, такие как
\fB.git\fP, независимо от \flag{no-ignore-vcs}. Чтобы исключить такие пути при
использовании \flag{hidden}, вы должны явно игнорировать их, используя другой
флаг или файл игнорирования.
"#
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.hidden = v.unwrap_switch();
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_hidden() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(false, args.hidden);

    let args = parse_low_raw(["--hidden"]).unwrap();
    assert_eq!(true, args.hidden);

    let args = parse_low_raw(["-."]).unwrap();
    assert_eq!(true, args.hidden);

    let args = parse_low_raw(["-.", "--no-hidden"]).unwrap();
    assert_eq!(false, args.hidden);

    let args = parse_low_raw(["--no-hidden", "-."]).unwrap();
    assert_eq!(true, args.hidden);
}

/// --hostname-bin
#[derive(Debug)]
struct HostnameBin;

impl Flag for HostnameBin {
    fn is_switch(&self) -> bool {
        false
    }
    fn name_long(&self) -> &'static str {
        "hostname-bin"
    }
    fn doc_variable(&self) -> Option<&'static str> {
        Some("COMMAND")
    }
    fn doc_category(&self) -> Category {
        Category::Output
    }
    fn doc_short(&self) -> &'static str {
        r"Запустить программу для получения имени хоста этой системы."
    }
    fn doc_long(&self) -> &'static str {
        r#"
Этот флаг управляет тем, как ripgrep определяет имя хоста этой системы.
Значение флага должно соответствовать исполняемому файлу (либо путь, либо то,
что может быть найдено через переменную окружения \fBPATH\fP вашей системы).
Когда установлен, ripgrep запустит этот исполняемый файл без аргументов и
рассмотрит его вывод (с удалёнными ведущими и замыкающими пробелами) как имя
хоста вашей системы.
.sp
Когда не установлен (по умолчанию или пустая строка), ripgrep попытается
автоматически определить имя хоста вашей системы. В Unix это соответствует
вызову \fBgethostname\fP. В Windows это соответствует вызову \fBGetComputerNameExW\fP
для получения «физического DNS-имени хоста» системы.
.sp
ripgrep использует имя хоста вашей системы для создания гиперссылок.
"#
    }
    fn completion_type(&self) -> CompletionType {
        CompletionType::Executable
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        let path = PathBuf::from(v.unwrap_value());
        args.hostname_bin =
            if path.as_os_str().is_empty() { None } else { Some(path) };
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_hostname_bin() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(None, args.hostname_bin);

    let args = parse_low_raw(["--hostname-bin", "foo"]).unwrap();
    assert_eq!(Some(PathBuf::from("foo")), args.hostname_bin);

    let args = parse_low_raw(["--hostname-bin=foo"]).unwrap();
    assert_eq!(Some(PathBuf::from("foo")), args.hostname_bin);
}

/// --hyperlink-format
#[derive(Debug)]
struct HyperlinkFormat;

impl Flag for HyperlinkFormat {
    fn is_switch(&self) -> bool {
        false
    }
    fn name_long(&self) -> &'static str {
        "hyperlink-format"
    }
    fn doc_variable(&self) -> Option<&'static str> {
        Some("FORMAT")
    }
    fn doc_category(&self) -> Category {
        Category::Output
    }
    fn doc_short(&self) -> &'static str {
        r"Установить формат гиперссылок."
    }
    fn doc_long(&self) -> &'static str {
        static DOC: LazyLock<String> = LazyLock::new(|| {
            let mut doc = String::new();
            doc.push_str(
                r#"
Установить формат гиперссылок для использования при выводе результатов. Гиперссылки
делают определённые элементы вывода ripgrep, такие как пути к файлам, кликабельными.
Это обычно работает только в эмуляторах терминала, которые поддерживают OSC-8
гиперссылки. Например, формат \fBfile://{host}{path}\fP будет испускать RFC 8089
гиперссылку. Чтобы увидеть формат, который использует ripgrep, передайте флаг
\flag{debug}.
.sp
Альтернативно, строка формата может соответствовать одному из следующих псевдонимов:
"#,
            );

            let mut aliases = grep::printer::hyperlink_aliases();
            aliases.sort_by_key(|alias| {
                alias.display_priority().unwrap_or(i16::MAX)
            });
            for (i, alias) in aliases.iter().enumerate() {
                doc.push_str(r"\fB");
                doc.push_str(alias.name());
                doc.push_str(r"\fP");
                doc.push_str(if i < aliases.len() - 1 { ", " } else { "." });
            }
            doc.push_str(
                r#"
Псевдоним будет заменён строкой формата, которая предназначена для работы с
соответствующим приложением.
.sp
Следующие переменные доступны в строке формата:
.sp
.TP 12
\fB{path}\fP
Обязательно. Заменяется путём к совпадающему файлу. Путь гарантированно будет
абсолютным и процентно закодированным таким образом, что его допустимо поместить
в URI. Обратите внимание, что путь гарантированно начинается с /.
.TP 12
\fB{host}\fP
Необязательно. Заменяется именем хоста вашей системы. В Unix это соответствует
вызову \fBgethostname\fP. В Windows это соответствует вызову \fBGetComputerNameExW\fP
для получения «физического DNS-имени хоста» системы. Альтернативно, если был
предоставлен \flag{hostname-bin}, то будет возвращено имя хоста, полученное из
вывода этой программы. Если имя хоста не может быть найдено, то эта переменная
заменяется пустой строкой.
.TP 12
\fB{line}\fP
Необязательно. Если уместно, заменяется номером строки совпадения. Если номер
строки недоступен (например, если был предоставлен \fB\-\-no\-line\-number\fP),
то он автоматически заменяется значением 1.
.TP 12
\fB{column}\fP
Необязательно, но требует наличия \fB{line}\fP. Если уместно, заменяется номером
столбца совпадения. Если номер столбца недоступен (например, если был предоставлен
\fB\-\-no\-column\fP), то он автоматически заменяется значением 1.
.TP 12
\fB{wslprefix}\fP
Необязательно. Это специальное значение, которое устанавливается в
\fBwsl$/\fP\fIWSL_DISTRO_NAME\fP, где \fIWSL_DISTRO_NAME\fP соответствует
значению эквивалентной переменной окружения. Если система не Unix или переменная
окружения \fIWSL_DISTRO_NAME\fP не установлена, то это заменяется пустой строкой.
.PP
Строка формата может быть пустой. Пустая строка формата эквивалентна псевдониму
\fBnone\fP. В этом случае гиперссылки будут отключены.
.sp
В настоящее время ripgrep не включает гиперссылки по умолчанию. Пользователи должны
явно включить их. Если вы не уверены, какой формат использовать, попробуйте
\fBdefault\fP.
.sp
Как и цвета, когда ripgrep обнаруживает, что stdout не подключён к tty, гиперссылки
автоматически отключаются, независимо от значения этого флага. Пользователи могут
передать \fB\-\-color=always\fP для принудительного испускания гиперссылок.
.sp
Обратите внимание, что гиперссылки записываются только когда путь также присутствует
в выводе и цвета включены. Чтобы записать гиперссылки без цветов, вам нужно настроить
ripgrep не раскрашивать ничего, не отключая полностью все ANSI-последовательности:
.sp
.EX
    \-\-colors 'path:none' \\
    \-\-colors 'line:none' \\
    \-\-colors 'column:none' \\
    \-\-colors 'match:none'
.EE
.sp
ripgrep работает таким образом, потому что рассматривает флаг \flag{color} как
прокси для того, должны ли вообще использоваться ANSI-последовательности. Это
означает, что переменные окружения, такие как \fBNO_COLOR=1\fP и \fBTERM=dumb\fP,
не только отключают цвета, но и гиперссылки. Аналогично, цвета и гиперссылки
отключаются, когда ripgrep не записывает в tty. (Если только не принудить это,
установив \fB\-\-color=always\fP.)
.sp
Если вы ищете файл напрямую, например:
.sp
.EX
    rg foo path/to/file
.EE
.sp
то гиперссылки не будут испускаться, поскольку предоставленный путь не появляется
в выводе. Чтобы заставить путь появиться и, таким образом, также гиперссылку,
используйте флаг \flag{with-filename}.
.sp
Дополнительную информацию о гиперссылках в эмуляторах терминала см.:
https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda
"#,
            );
            doc
        });
        &DOC
    }

    fn doc_choices(&self) -> &'static [&'static str] {
        static CHOICES: LazyLock<Vec<String>> = LazyLock::new(|| {
            let mut aliases = grep::printer::hyperlink_aliases();
            aliases.sort_by_key(|alias| {
                alias.display_priority().unwrap_or(i16::MAX)
            });
            aliases.iter().map(|alias| alias.name().to_string()).collect()
        });
        static BORROWED: LazyLock<Vec<&'static str>> =
            LazyLock::new(|| CHOICES.iter().map(|name| &**name).collect());
        &*BORROWED
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        let v = v.unwrap_value();
        let string = convert::str(&v)?;
        let format = string.parse().context("invalid hyperlink format")?;
        args.hyperlink_format = format;
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_hyperlink_format() {
    let parseformat = |format: &str| {
        format.parse::<grep::printer::HyperlinkFormat>().unwrap()
    };

    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(parseformat("none"), args.hyperlink_format);

    let args = parse_low_raw(["--hyperlink-format", "default"]).unwrap();
    #[cfg(windows)]
    assert_eq!(parseformat("file://{path}"), args.hyperlink_format);
    #[cfg(not(windows))]
    assert_eq!(parseformat("file://{host}{path}"), args.hyperlink_format);

    let args = parse_low_raw(["--hyperlink-format", "file"]).unwrap();
    assert_eq!(parseformat("file://{host}{path}"), args.hyperlink_format);

    let args = parse_low_raw([
        "--hyperlink-format",
        "file",
        "--hyperlink-format=grep+",
    ])
    .unwrap();
    assert_eq!(parseformat("grep+://{path}:{line}"), args.hyperlink_format);

    let args =
        parse_low_raw(["--hyperlink-format", "file://{host}{path}#{line}"])
            .unwrap();
    assert_eq!(
        parseformat("file://{host}{path}#{line}"),
        args.hyperlink_format
    );

    let result = parse_low_raw(["--hyperlink-format", "file://heythere"]);
    assert!(result.is_err(), "{result:?}");
}

/// --iglob
#[derive(Debug)]
struct IGlob;

impl Flag for IGlob {
    fn is_switch(&self) -> bool {
        false
    }
    fn name_long(&self) -> &'static str {
        "iglob"
    }
    fn doc_variable(&self) -> Option<&'static str> {
        Some("GLOB")
    }
    fn doc_category(&self) -> Category {
        Category::Filter
    }
    fn doc_short(&self) -> &'static str {
        r"Включить/исключить пути без учёта регистра."
    }
    fn doc_long(&self) -> &'static str {
        r"
Включить или исключить файлы и каталоги для поиска, которые соответствуют
заданному glob. Это всегда переопределяет любую другую логику игнорирования.
Может быть использовано несколько флагов glob. Правила glob сопоставляются с
глобами \fB.gitignore\fP. Предшествуйте glob символом \fB!\fP, чтобы исключить
его. Если несколько glob соответствуют файлу или каталогу, glob, указанный
позже в командной строке, имеет приоритет. Glob, используемые через этот флаг,
сопоставляются без учёта регистра.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        let glob = convert::string(v.unwrap_value())?;
        args.iglobs.push(glob);
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_iglob() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(Vec::<String>::new(), args.iglobs);

    let args = parse_low_raw(["--iglob", "foo"]).unwrap();
    assert_eq!(vec!["foo".to_string()], args.iglobs);

    let args = parse_low_raw(["--iglob=foo"]).unwrap();
    assert_eq!(vec!["foo".to_string()], args.iglobs);

    let args = parse_low_raw(["--iglob", "-foo"]).unwrap();
    assert_eq!(vec!["-foo".to_string()], args.iglobs);

    let args = parse_low_raw(["--iglob=-foo"]).unwrap();
    assert_eq!(vec!["-foo".to_string()], args.iglobs);
}

/// -i/--ignore-case
#[derive(Debug)]
struct IgnoreCase;

impl Flag for IgnoreCase {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'i')
    }
    fn name_long(&self) -> &'static str {
        "ignore-case"
    }
    fn doc_category(&self) -> Category {
        Category::Search
    }
    fn doc_short(&self) -> &'static str {
        r"Поиск без учёта регистра."
    }
    fn doc_long(&self) -> &'static str {
        r#"
Когда этот флаг предоставлен, все шаблоны будут искаться без учёта регистра.
Правила игнорирования регистра, используемые движком регулярных выражений по
умолчанию ripgrep, соответствуют «простым» правилам приведения регистра Unicode.
.sp
Это глобальная опция, которая применяется ко всем шаблонам, переданным в ripgrep.
Отдельные шаблоны всё ещё могут быть сопоставлены с учётом регистра с помощью
встроенных флагов регулярных выражений. Например, \fB(?\-i)abc\fP будет
сопоставлять \fBabc\fP с учётом регистра, даже когда используется этот флаг.
.sp
Этот флаг переопределяет \flag{case-sensitive} и \flag{smart-case}.
"#
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        assert!(v.unwrap_switch(), "flag has no negation");
        args.case = CaseMode::Insensitive;
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_ignore_case() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(CaseMode::Sensitive, args.case);

    let args = parse_low_raw(["--ignore-case"]).unwrap();
    assert_eq!(CaseMode::Insensitive, args.case);

    let args = parse_low_raw(["-i"]).unwrap();
    assert_eq!(CaseMode::Insensitive, args.case);

    let args = parse_low_raw(["-i", "-s"]).unwrap();
    assert_eq!(CaseMode::Sensitive, args.case);

    let args = parse_low_raw(["-s", "-i"]).unwrap();
    assert_eq!(CaseMode::Insensitive, args.case);
}

/// --ignore-file
#[derive(Debug)]
struct IgnoreFile;

impl Flag for IgnoreFile {
    fn is_switch(&self) -> bool {
        false
    }
    fn name_long(&self) -> &'static str {
        "ignore-file"
    }
    fn doc_variable(&self) -> Option<&'static str> {
        Some("PATH")
    }
    fn doc_category(&self) -> Category {
        Category::Filter
    }
    fn doc_short(&self) -> &'static str {
        r"Указать дополнительные файлы игнорирования."
    }
    fn doc_long(&self) -> &'static str {
        r"
Указывает путь к одному или нескольким файлам правил в формате \fBgitignore\fP.
Эти шаблоны применяются после применения шаблонов, найденных в \fB.gitignore\fP,
\fB.rgignore\fP и \fB.ignore\fP, и сопоставляются относительно текущего рабочего
каталога. То есть файлы, указанные через этот флаг, имеют меньший приоритет, чем
файлы, автоматически найденные в дереве каталогов. Несколько дополнительных файлов
игнорирования могут быть указаны путём многократного использования этого флага.
При указании нескольких файлов игнорирования более ранние файлы имеют меньший
приоритет, чем более поздние файлы.
.sp
Если вы ищете способ включить или исключить файлы и каталоги напрямую из командной
строки, используйте вместо этого \flag{glob}.
"
    }
    fn completion_type(&self) -> CompletionType {
        CompletionType::Filename
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        let path = PathBuf::from(v.unwrap_value());
        args.ignore_file.push(path);
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_ignore_file() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(Vec::<PathBuf>::new(), args.ignore_file);

    let args = parse_low_raw(["--ignore-file", "foo"]).unwrap();
    assert_eq!(vec![PathBuf::from("foo")], args.ignore_file);

    let args = parse_low_raw(["--ignore-file", "foo", "--ignore-file", "bar"])
        .unwrap();
    assert_eq!(
        vec![PathBuf::from("foo"), PathBuf::from("bar")],
        args.ignore_file
    );
}

/// --ignore-file-case-insensitive
#[derive(Debug)]
struct IgnoreFileCaseInsensitive;

impl Flag for IgnoreFileCaseInsensitive {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "ignore-file-case-insensitive"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("no-ignore-file-case-insensitive")
    }
    fn doc_category(&self) -> Category {
        Category::Filter
    }
    fn doc_short(&self) -> &'static str {
        r"Обрабатыват�������������������������������������������������������������������������������������������������������������������������� файлы игнорирования без учёта регистра."
    }
    fn doc_long(&self) -> &'static str {
        r"
Обрабатывать файлы игнорирования (\fB.gitignore\fP, \fB.ignore\fP и т.д.) без
учёта регистра. Обратите внимание, что это имеет штраф производительности и
наиболее полезно в файловых системах без учёта регистра (таких как Windows).
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.ignore_file_case_insensitive = v.unwrap_switch();
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_ignore_file_case_insensitive() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(false, args.ignore_file_case_insensitive);

    let args = parse_low_raw(["--ignore-file-case-insensitive"]).unwrap();
    assert_eq!(true, args.ignore_file_case_insensitive);

    let args = parse_low_raw([
        "--ignore-file-case-insensitive",
        "--no-ignore-file-case-insensitive",
    ])
    .unwrap();
    assert_eq!(false, args.ignore_file_case_insensitive);

    let args = parse_low_raw([
        "--no-ignore-file-case-insensitive",
        "--ignore-file-case-insensitive",
    ])
    .unwrap();
    assert_eq!(true, args.ignore_file_case_insensitive);
}

/// --include-zero
#[derive(Debug)]
struct IncludeZero;

impl Flag for IncludeZero {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "include-zero"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("no-include-zero")
    }
    fn doc_category(&self) -> Category {
        Category::Output
    }
    fn doc_short(&self) -> &'static str {
        r"Включить ноль совпадений в сводный вывод."
    }
    fn doc_long(&self) -> &'static str {
        r"
При использовании с \flag{count} или \flag{count-matches} это заставляет ripgrep
выводить количество совпадений для каждого файла, даже если было ноль совпадений.
Это отключено по умолчанию, но может быть включено, чтобы заставить ripgrep
вести себя больше как grep.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.include_zero = v.unwrap_switch();
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_include_zero() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(false, args.include_zero);

    let args = parse_low_raw(["--include-zero"]).unwrap();
    assert_eq!(true, args.include_zero);

    let args = parse_low_raw(["--include-zero", "--no-include-zero"]).unwrap();
    assert_eq!(false, args.include_zero);
}

/// -v/--invert-match
#[derive(Debug)]
struct InvertMatch;

impl Flag for InvertMatch {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'v')
    }
    fn name_long(&self) -> &'static str {
        "invert-match"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("no-invert-match")
    }
    fn doc_category(&self) -> Category {
        Category::Search
    }
    fn doc_short(&self) -> &'static str {
        r"Инвертировать совпадение."
    }
    fn doc_long(&self) -> &'static str {
        r"
Этот флаг инвертирует совпадение. То есть, вместо печати строк, которые
соответствуют, ripgrep будет печатать строки, которые не соответствуют.
.sp
Обратите внимание, что это инвертирует только построчное сопоставление. Например,
комбинирование этого флага с \flag{files-with-matches} будет выводить файлы,
которые содержат любые строки, не соответствующие заданным шаблонам. Это не то
же самое, что, например, \flag{files-without-match}, который будет выводить
файлы, которые не содержат никаких совпадающих строк.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.invert_match = v.unwrap_switch();
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_invert_match() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(false, args.invert_match);

    let args = parse_low_raw(["--invert-match"]).unwrap();
    assert_eq!(true, args.invert_match);

    let args = parse_low_raw(["-v"]).unwrap();
    assert_eq!(true, args.invert_match);

    let args = parse_low_raw(["-v", "--no-invert-match"]).unwrap();
    assert_eq!(false, args.invert_match);
}

/// --json
#[derive(Debug)]
struct JSON;

impl Flag for JSON {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "json"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("no-json")
    }
    fn doc_category(&self) -> Category {
        Category::OutputModes
    }
    fn doc_short(&self) -> &'static str {
        r"Показать результаты поиска в формате JSON Lines."
    }
    fn doc_long(&self) -> &'static str {
        r"
Включить вывод результатов в формате JSON Lines.
.sp
Когда этот флаг предоставлен, ripgrep будет испускать последовательность
сообщений, каждое закодировано как JSON-объект, где есть пять различных типов
сообщений:
.sp
.TP 12
\fBbegin\fP
Сообщение, которое указывает, что файл ищется и содержит как минимум одно
совпадение.
.TP 12
\fBend\fP
Сообщение, которое указывает, что файл закончен поиском. Это сообщение также
включает сводную статистику о поиске для определённого файла.
.TP 12
\fBmatch\fP
Сообщение, которое указывает, что совпадение найдено. Это включает текст и
смещения совпадения.
.TP 12
\fBcontext\fP
Сообщение, которое указывает, что контекстная строка найдена. Это включает
текст строки, а также любую информацию о совпадении, если поиск был инвертирован.
.TP 12
\fBsummary\fP
Финальное сообщение, испускаемое ripgrep, которое содержит сводную статистику
о поиске по всем файлам.
.PP
Поскольку пути к файлам или содержимое файлов не гарантированно являются
валидным UTF-8, а сам JSON должен быть представим кодировкой Unicode, ripgrep
будет испускать все элементы данных как объекты с одним из двух ключей:
\fBtext\fP или \fBbytes\fP. \fBtext\fP — это нормальная JSON-строка, когда
данные являются валидным UTF-8, в то время как \fBbytes\fP — это base64-кодированное
содержимое данных.
.sp
Формат JSON Lines поддерживается только для отображения результатов поиска.
Он не может быть использован с другими флагами, которые испускают другие типы
вывода, такими как \flag{files}, \flag{files-with-matches}, \flag{files-without-match},
\flag{count} или \flag{count-matches}. ripgrep сообщит об ошибке, если любой
из вышеупомянутых флагов используется вместе с \flag{json}.
.sp
Другие флаги, которые управляют аспектами стандартного вывода, такие как
\flag{only-matching}, \flag{heading}, \flag{replace}, \flag{max-columns} и т.д.,
не имеют эффекта, когда \flag{json} установлен. Однако включение вывода JSON
всегда неявно и безусловно включает \flag{stats}.
.sp
Более полное описание используемого формата JSON можно найти здесь:
\fIhttps://docs.rs/grep-printer/*/grep_printer/struct.JSON.html\fP.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        if v.unwrap_switch() {
            args.mode.update(Mode::Search(SearchMode::JSON));
        } else if matches!(args.mode, Mode::Search(SearchMode::JSON)) {
            // --no-json only reverts to the default mode if the mode is
            // JSON, otherwise it's a no-op.
            args.mode.update(Mode::Search(SearchMode::Standard));
        }
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_json() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(Mode::Search(SearchMode::Standard), args.mode);

    let args = parse_low_raw(["--json"]).unwrap();
    assert_eq!(Mode::Search(SearchMode::JSON), args.mode);

    let args = parse_low_raw(["--json", "--no-json"]).unwrap();
    assert_eq!(Mode::Search(SearchMode::Standard), args.mode);

    let args = parse_low_raw(["--json", "--files", "--no-json"]).unwrap();
    assert_eq!(Mode::Files, args.mode);

    let args = parse_low_raw(["--json", "-l", "--no-json"]).unwrap();
    assert_eq!(Mode::Search(SearchMode::FilesWithMatches), args.mode);
}

/// --line-buffered
#[derive(Debug)]
struct LineBuffered;

impl Flag for LineBuffered {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "line-buffered"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("no-line-buffered")
    }
    fn doc_category(&self) -> Category {
        Category::Output
    }
    fn doc_short(&self) -> &'static str {
        r"Принудительно использовать построчную буферизацию."
    }
    fn doc_long(&self) -> &'static str {
        r"
Когда включено, ripgrep всегда будет использовать построчную буферизацию. То есть,
всякий раз, когда найдена совпадающая строка, она будет немедленно сброшена в
stdout. Это по умолчанию, когда stdout ripgrep подключён к tty, но в противном
случае ripgrep будет использовать блочную буферизацию, которая обычно быстрее.
Этот флаг заставляет ripgrep использовать построчную буферизацию, даже если он
иначе использовал бы блочную буферизацию. Это обычно полезно в конвейерах
оболочки, например:
.sp
.EX
    tail -f something.log | rg foo --line-buffered | rg bar
.EE
.sp
Это переопределяет флаг \flag{block-buffered}.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.buffer = if v.unwrap_switch() {
            BufferMode::Line
        } else {
            BufferMode::Auto
        };
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_line_buffered() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(BufferMode::Auto, args.buffer);

    let args = parse_low_raw(["--line-buffered"]).unwrap();
    assert_eq!(BufferMode::Line, args.buffer);

    let args =
        parse_low_raw(["--line-buffered", "--no-line-buffered"]).unwrap();
    assert_eq!(BufferMode::Auto, args.buffer);

    let args = parse_low_raw(["--line-buffered", "--block-buffered"]).unwrap();
    assert_eq!(BufferMode::Block, args.buffer);
}

/// -n/--line-number
#[derive(Debug)]
struct LineNumber;

impl Flag for LineNumber {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'n')
    }
    fn name_long(&self) -> &'static str {
        "line-number"
    }
    fn doc_category(&self) -> Category {
        Category::Output
    }
    fn doc_short(&self) -> &'static str {
        r"Показать номера строк."
    }
    fn doc_long(&self) -> &'static str {
        r"
Показать номера строк (1-основанные).
.sp
Это включено по умолчанию, когда stdout подключён к tty.
.sp
Этот флаг может быть отключен с помощью \flag{no-line-number}.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        assert!(v.unwrap_switch(), "--line-number has no automatic negation");
        args.line_number = Some(true);
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_line_number() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(None, args.line_number);

    let args = parse_low_raw(["--line-number"]).unwrap();
    assert_eq!(Some(true), args.line_number);

    let args = parse_low_raw(["-n"]).unwrap();
    assert_eq!(Some(true), args.line_number);

    let args = parse_low_raw(["-n", "--no-line-number"]).unwrap();
    assert_eq!(Some(false), args.line_number);
}

/// -N/--no-line-number
#[derive(Debug)]
struct LineNumberNo;

impl Flag for LineNumberNo {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'N')
    }
    fn name_long(&self) -> &'static str {
        "no-line-number"
    }
    fn doc_category(&self) -> Category {
        Category::Output
    }
    fn doc_short(&self) -> &'static str {
        r"Подавить н��ме��а строк."
    }
    fn doc_long(&self) -> &'static str {
        r"
П��да��ит�� номера с��ро��.
.sp
Н��ме��а строк о��кл��че��ы по у��ол��ан��ю, ��ог��а stdout ��е подключён к tty.
.sp
Номера строк могут быть принудительно включены с помощью \flag{line-number}.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        assert!(
            v.unwrap_switch(),
            "--no-line-number has no automatic negation"
        );
        args.line_number = Some(false);
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_no_line_number() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(None, args.line_number);

    let args = parse_low_raw(["--no-line-number"]).unwrap();
    assert_eq!(Some(false), args.line_number);

    let args = parse_low_raw(["-N"]).unwrap();
    assert_eq!(Some(false), args.line_number);

    let args = parse_low_raw(["-N", "--line-number"]).unwrap();
    assert_eq!(Some(true), args.line_number);
}

/// -x/--line-regexp
#[derive(Debug)]
struct LineRegexp;

impl Flag for LineRegexp {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'x')
    }
    fn name_long(&self) -> &'static str {
        "line-regexp"
    }
    fn doc_category(&self) -> Category {
        Category::Search
    }
    fn doc_short(&self) -> &'static str {
        r"Показать совпадения, окружённые границами строк."
    }
    fn doc_long(&self) -> &'static str {
        r"
Когда включено, ripgrep будет показывать только совпадения, окружённые границами
строк. Это эквивалентно окружению каждого шаблона символами \fB^\fP и \fB$\fP.
Другими словами, это печатает только строки, где вся строка участвует в совпадении.
.sp
Это переопределяет флаг \flag{word-regexp}.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        assert!(v.unwrap_switch(), "--line-regexp has no negation");
        args.boundary = Some(BoundaryMode::Line);
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_line_regexp() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(None, args.boundary);

    let args = parse_low_raw(["--line-regexp"]).unwrap();
    assert_eq!(Some(BoundaryMode::Line), args.boundary);

    let args = parse_low_raw(["-x"]).unwrap();
    assert_eq!(Some(BoundaryMode::Line), args.boundary);
}

/// -M/--max-columns
#[derive(Debug)]
struct MaxColumns;

impl Flag for MaxColumns {
    fn is_switch(&self) -> bool {
        false
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'M')
    }
    fn name_long(&self) -> &'static str {
        "max-columns"
    }
    fn doc_variable(&self) -> Option<&'static str> {
        Some("NUM")
    }
    fn doc_category(&self) -> Category {
        Category::Output
    }
    fn doc_short(&self) -> &'static str {
        r"Пропустить строки длиннее этого предела."
    }
    fn doc_long(&self) -> &'static str {
        r"
Когда предоставлен, ripgrep будет пропускать строки длиннее этого предела в байтах.
Вместо печати длинных строк печатается только количество совпадений в этой строке.
.sp
Когда этот флаг опущен или установлен в \fB0\fP, то он не имеет эффекта.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        let max = convert::u64(&v.unwrap_value())?;
        args.max_columns = if max == 0 { None } else { Some(max) };
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_max_columns() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(None, args.max_columns);

    let args = parse_low_raw(["--max-columns", "5"]).unwrap();
    assert_eq!(Some(5), args.max_columns);

    let args = parse_low_raw(["-M", "5"]).unwrap();
    assert_eq!(Some(5), args.max_columns);

    let args = parse_low_raw(["-M5"]).unwrap();
    assert_eq!(Some(5), args.max_columns);

    let args = parse_low_raw(["--max-columns", "5", "-M0"]).unwrap();
    assert_eq!(None, args.max_columns);
}

/// --max-columns-preview
#[derive(Debug)]
struct MaxColumnsPreview;

impl Flag for MaxColumnsPreview {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "max-columns-preview"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("no-max-columns-preview")
    }
    fn doc_category(&self) -> Category {
        Category::Output
    }
    fn doc_short(&self) -> &'static str {
        r"Показать предпросмотр для строк, превышающих предел."
    }
    fn doc_long(&self) -> &'static str {
        r"
Печатает предпросмотр для строк, превышающих настроенный предел максимального
количества столбцов.
.sp
Когда используется флаг \flag{max-columns}, ripgrep по умолчанию полностью
заменяет любую строку, которая слишком длинная, сообщением, указывающим, что
совпадающая строка была удалена. Когда этот флаг комбинирован с \flag{max-columns},
вместо этого показывается предпросмотр строки (соответствующий размеру предела),
где часть строки, превышающая предел, не показывается.
.sp
Если флаг \flag{max-columns} не установлен, то это не имеет эффекта.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.max_columns_preview = v.unwrap_switch();
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_max_columns_preview() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(false, args.max_columns_preview);

    let args = parse_low_raw(["--max-columns-preview"]).unwrap();
    assert_eq!(true, args.max_columns_preview);

    let args =
        parse_low_raw(["--max-columns-preview", "--no-max-columns-preview"])
            .unwrap();
    assert_eq!(false, args.max_columns_preview);
}

/// -m/--max-count
#[derive(Debug)]
struct MaxCount;

impl Flag for MaxCount {
    fn is_switch(&self) -> bool {
        false
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'm')
    }
    fn name_long(&self) -> &'static str {
        "max-count"
    }
    fn doc_variable(&self) -> Option<&'static str> {
        Some("NUM")
    }
    fn doc_category(&self) -> Category {
        Category::Search
    }
    fn doc_short(&self) -> &'static str {
        r"Ограничить количество совпадающих строк."
    }
    fn doc_long(&self) -> &'static str {
        r"
Ограничить количество совпадающих строк на каждый искомый файл до \fINUM\fP.
.sp
Когда используется \flag{multiline}, одно совпадение, которое охватывает
несколько строк, считается только один раз для целей этого предела. Несколько
совпадений в одной строке считаются только один раз, как они были бы в режиме
без многострочного поиска.
.sp
При комбинировании с \flag{after-context} или \flag{context} возможно, что
будет напечатано больше совпадений, чем максимум, если контекстные строки
содержат совпадение.
.sp
Обратите внимание, что \fB0\fP является допустимым значением, но вряд ли
полезным. Когда используется, ripgrep не будет искать ничего.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.max_count = Some(convert::u64(&v.unwrap_value())?);
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_max_count() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(None, args.max_count);

    let args = parse_low_raw(["--max-count", "5"]).unwrap();
    assert_eq!(Some(5), args.max_count);

    let args = parse_low_raw(["-m", "5"]).unwrap();
    assert_eq!(Some(5), args.max_count);

    let args = parse_low_raw(["-m", "5", "--max-count=10"]).unwrap();
    assert_eq!(Some(10), args.max_count);
    let args = parse_low_raw(["-m0"]).unwrap();
    assert_eq!(Some(0), args.max_count);
}

/// --max-depth
#[derive(Debug)]
struct MaxDepth;

impl Flag for MaxDepth {
    fn is_switch(&self) -> bool {
        false
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'd')
    }
    fn name_long(&self) -> &'static str {
        "max-depth"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["maxdepth"]
    }
    fn doc_variable(&self) -> Option<&'static str> {
        Some("NUM")
    }
    fn doc_category(&self) -> Category {
        Category::Filter
    }
    fn doc_short(&self) -> &'static str {
        r"Спускаться не более чем на NUM каталогов."
    }
    fn doc_long(&self) -> &'static str {
        r"
Этот флаг ограничивает глубину обхода каталогов до \fINUM\fP уровней сверх
предоставленных путей. Значение \fB0\fP ищет только явно предоставленные пути.
.sp
Например, \fBrg --max-depth 0 \fP\fIdir/\fP не выполняет никаких действий,
потому что в \fIdir/\fP не будет спуска. \fBrg --max-depth 1 \fP\fIdir/\fP
будет искать только прямых потомков \fIdir\fP.
.sp
Альтернативное написание этого флага — \fB\-\-maxdepth\fP.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.max_depth = Some(convert::usize(&v.unwrap_value())?);
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_max_depth() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(None, args.max_depth);

    let args = parse_low_raw(["--max-depth", "5"]).unwrap();
    assert_eq!(Some(5), args.max_depth);

    let args = parse_low_raw(["-d", "5"]).unwrap();
    assert_eq!(Some(5), args.max_depth);

    let args = parse_low_raw(["--max-depth", "5", "--max-depth=10"]).unwrap();
    assert_eq!(Some(10), args.max_depth);

    let args = parse_low_raw(["--max-depth", "0"]).unwrap();
    assert_eq!(Some(0), args.max_depth);

    let args = parse_low_raw(["--maxdepth", "5"]).unwrap();
    assert_eq!(Some(5), args.max_depth);
}

/// --max-filesize
#[derive(Debug)]
struct MaxFilesize;

impl Flag for MaxFilesize {
    fn is_switch(&self) -> bool {
        false
    }
    fn name_long(&self) -> &'static str {
        "max-filesize"
    }
    fn doc_variable(&self) -> Option<&'static str> {
        Some("NUM+SUFFIX?")
    }
    fn doc_category(&self) -> Category {
        Category::Filter
    }
    fn doc_short(&self) -> &'static str {
        r"Игнорировать файлы больше NUM по размеру."
    }
    fn doc_long(&self) -> &'static str {
        r"
Игнорировать файлы больше \fINUM\fP по размеру. Это не применяется к каталогам.
.sp
Формат ввода принимает суффиксы \fBK\fP, \fBM\fP или \fBG\fP, которые
соответствуют килобайтам, мегабайтам и гигабайтам соответственно. Если суффикс
не предоставлен, ввод рассматривается как байты.
.sp
Примеры: \fB\-\-max-filesize 50K\fP или \fB\-\-max\-filesize 80M\fP.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        let v = v.unwrap_value();
        args.max_filesize = Some(convert::human_readable_u64(&v)?);
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_max_filesize() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(None, args.max_filesize);

    let args = parse_low_raw(["--max-filesize", "1024"]).unwrap();
    assert_eq!(Some(1024), args.max_filesize);

    let args = parse_low_raw(["--max-filesize", "1K"]).unwrap();
    assert_eq!(Some(1024), args.max_filesize);

    let args =
        parse_low_raw(["--max-filesize", "1K", "--max-filesize=1M"]).unwrap();
    assert_eq!(Some(1024 * 1024), args.max_filesize);
}

/// --mmap
#[derive(Debug)]
struct Mmap;

impl Flag for Mmap {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "mmap"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("no-mmap")
    }
    fn doc_category(&self) -> Category {
        Category::Search
    }
    fn doc_short(&self) -> &'static str {
        r"Искать с использованием отображения в память, когда возможно."
    }
    fn doc_long(&self) -> &'static str {
        r"
Когда включено, ripgrep будет искать с использованием отображения в память, когда
это возможно. Это включено по умолчанию, когда ripgrep считает, что это будет
быстрее.
.sp
Поиск с отображением в память не может использоваться во всех обстоятельствах.
Например, при поиске виртуальных файлов или потоков, таких как \fBstdin\fP. В
таких случаях отображение в память не будет использоваться, даже когда этот флаг
включён.
.sp
Обратите внимание, что ripgrep может аварийно завершиться неожиданно, когда
используется отображение в память, если он ищет файл, который одновременно
усекается. Пользователи могут отказаться от этой возможности, отключив отображение
в память.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.mmap = if v.unwrap_switch() {
            MmapMode::AlwaysTryMmap
        } else {
            MmapMode::Never
        };
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_mmap() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(MmapMode::Auto, args.mmap);

    let args = parse_low_raw(["--mmap"]).unwrap();
    assert_eq!(MmapMode::AlwaysTryMmap, args.mmap);

    let args = parse_low_raw(["--no-mmap"]).unwrap();
    assert_eq!(MmapMode::Never, args.mmap);

    let args = parse_low_raw(["--mmap", "--no-mmap"]).unwrap();
    assert_eq!(MmapMode::Never, args.mmap);

    let args = parse_low_raw(["--no-mmap", "--mmap"]).unwrap();
    assert_eq!(MmapMode::AlwaysTryMmap, args.mmap);
}

/// -U/--multiline
#[derive(Debug)]
struct Multiline;

impl Flag for Multiline {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'U')
    }
    fn name_long(&self) -> &'static str {
        "multiline"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("no-multiline")
    }
    fn doc_category(&self) -> Category {
        Category::Search
    }
    fn doc_short(&self) -> &'static str {
        r"Включить поиск по нескольким строкам."
    }
    fn doc_long(&self) -> &'static str {
        r#"
Этот флаг включает поиск по нескольким строкам.
.sp
Когда режим многострочного поиска включён, ripgrep снимет ограничение, что
совпадение не может включать терминатор строки. Например, когда режим
многострочного поиска не включён (по умолчанию), то регулярное выражение
\fB\\p{any}\fP будет сопоставлять любую Unicode-кодовую точку, кроме \fB\\n\fP.
Аналогично, регулярное выражение \fB\\n\fP явно запрещено, и если вы попытаетесь
использовать его, ripgrep вернёт ошибку. Однако, когда режим многострочного
поиска включён, \fB\\p{any}\fP будет сопоставлять любую Unicode-кодовую точку,
включая \fB\\n\fP, и регулярные выражения, такие как \fB\\n\fP, разрешены.
.sp
Важное замечание: многострочный режим не изменяет семантику сопоставления \fB.\fP.
А именно, в большинстве сопоставителей регулярных выражений \fB.\fP по умолчанию
сопоставляет любой символ, кроме \fB\\n\fP, и это верно в ripgrep. Чтобы заставить
\fB.\fP сопоставлять \fB\\n\fP, вы должны включить флаг «dot all» внутри регулярного
выражения. Например, и \fB(?s).\fP, и \fB(?s:.)\fP имеют одинаковую семантику, где
\fB.\fP будет сопоставлять любой символ, включая \fB\\n\fP. Альтернативно, флаг
\flag{multiline-dotall} может быть передан, чтобы сделать поведение «dot all»
по умолчанию. Этот флаг применяется только когда многострочный поиск включён.
.sp
Нет предела количеству строк, которое может охватывать одно совпадение.
.sp
\fBПРЕДУПРЕЖДЕНИЕ\fP: Из-за того, как работает базовый движок регулярных выражений,
многострочные поиски могут быть медленнее, чем обычные поиски, ориентированные на
строки, и они также могут использовать больше памяти. В частности, когда включён
многострочный режим, ripgrep требует, чтобы каждый искомый файл был расположен
непрерывно в памяти (либо путём чтения его в кучу, либо путём отображения в память).
Вещи, которые не могут быть отображены в память (такие как \fBstdin\fP), будут
потреблены до EOF, прежде чем поиск может начаться. В общем, ripgrep будет делать
эти вещи только когда необходимо. В частности, если предоставлен флаг \flag{multiline},
но регулярное выражение не содержит шаблонов, которые могли бы сопоставить символы
\fB\\n\fP, то ripgrep автоматически избежит чтения каждого файла в память перед его
поиском. Тем не менее, если вас интересуют только совпадения, охватывающие не более
одной строки, то всегда лучше отключить многострочный режим.
.sp
Это переопределяет флаг \flag{stop-on-nonmatch}.
"#
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.multiline = v.unwrap_switch();
        if args.multiline {
            args.stop_on_nonmatch = false;
        }
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_multiline() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(false, args.multiline);

    let args = parse_low_raw(["--multiline"]).unwrap();
    assert_eq!(true, args.multiline);

    let args = parse_low_raw(["-U"]).unwrap();
    assert_eq!(true, args.multiline);

    let args = parse_low_raw(["-U", "--no-multiline"]).unwrap();
    assert_eq!(false, args.multiline);
}

/// --multiline-dotall
#[derive(Debug)]
struct MultilineDotall;

impl Flag for MultilineDotall {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "multiline-dotall"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("no-multiline-dotall")
    }
    fn doc_category(&self) -> Category {
        Category::Search
    }
    fn doc_short(&self) -> &'static str {
        r"Заставить '.' сопоставлять терминаторы строк."
    }
    fn doc_long(&self) -> &'static str {
        r#"
Этот флаг включает режим «dot all» во всех шаблонах регулярных выражений. Это
заставляет \fB.\fP сопоставлять терминаторы строк, когда включён многострочный
поиск. Этот флаг не имеет эффекта, если многострочный поиск не включён с помощью
флага \flag{multiline}.
.sp
Обычно \fB.\fP будет сопоставлять любой символ, кроме терминаторов строк. Хотя
такое поведение обычно не актуально для поиска, ориентированного на строки
(поскольку совпадения могут охватывать не более одной строки), это может быть
полезно при поиске с флагом \flag{multiline}. По умолчанию многострочный режим
работает без включённого режима «dot all».
.sp
Этот флаг обычно предназначен для использования в псевдониме или вашем файле
конфигурации ripgrep, если вы предпочитаете семантику «dot all» по умолчанию.
Обратите внимание, что независимо от того, используется ли этот флаг, семантика
«dot all» всё ещё может управляться с помощью встроенных флагов в самом шаблоне
регулярного выражения, например, \fB(?s:.)\fP всегда включает «dot all», тогда
как \fB(?-s:.)\fP всегда отключает «dot all». Более того, вы можете использовать
классы символов, такие как \fB\\p{any}\fP, для сопоставления любой Unicode-кодовой
точки независимо от того, включён ли режим «dot all» или нет.
"#
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.multiline_dotall = v.unwrap_switch();
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_multiline_dotall() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(false, args.multiline_dotall);

    let args = parse_low_raw(["--multiline-dotall"]).unwrap();
    assert_eq!(true, args.multiline_dotall);

    let args = parse_low_raw(["--multiline-dotall", "--no-multiline-dotall"])
        .unwrap();
    assert_eq!(false, args.multiline_dotall);
}

/// --no-config
#[derive(Debug)]
struct NoConfig;

impl Flag for NoConfig {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "no-config"
    }
    fn doc_category(&self) -> Category {
        Category::OtherBehaviors
    }
    fn doc_short(&self) -> &'static str {
        r"Никогда не читать файлы конфигурации."
    }
    fn doc_long(&self) -> &'static str {
        r"
Когда установлен, ripgrep никогда не будет читать файлы конфигурации. Когда этот
флаг присутствует, ripgrep не будет уважать переменную окружения
\fBRIPGREP_CONFIG_PATH\fP.
.sp
Если ripgrep когда-либо получит функцию автоматического чтения файлов конфигурации
в предопределённых местах, то этот флаг также отключит это поведение.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        assert!(v.unwrap_switch(), "--no-config has no negation");
        args.no_config = true;
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_no_config() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(false, args.no_config);

    let args = parse_low_raw(["--no-config"]).unwrap();
    assert_eq!(true, args.no_config);
}

/// --no-ignore
#[derive(Debug)]
struct NoIgnore;

impl Flag for NoIgnore {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "no-ignore"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("ignore")
    }
    fn doc_category(&self) -> Category {
        Category::Filter
    }
    fn doc_short(&self) -> &'static str {
        r"Не использовать файлы игнорирования."
    }
    fn doc_long(&self) -> &'static str {
        r"
Когда установлен, файлы игнорирования, такие как \fB.gitignore\fP, \fB.ignore\fP и
\fB.rgignore\fP, не будут уважаться. Это подразумевает \flag{no-ignore-dot},
\flag{no-ignore-exclude}, \flag{no-ignore-global}, \flag{no-ignore-parent} и
\flag{no-ignore-vcs}.
.sp
Это не подразумевает \flag{no-ignore-files}, поскольку \flag{ignore-file}
указывается явно как аргумент командной строки.
.sp
При однократном предоставлении флаг \flag{unrestricted} идентичен по поведению
этому флагу и может считаться псевдонимом. Однако последующие флаги
\flag{unrestricted} имеют дополнительные эффекты.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        let yes = v.unwrap_switch();
        args.no_ignore_dot = yes;
        args.no_ignore_exclude = yes;
        args.no_ignore_global = yes;
        args.no_ignore_parent = yes;
        args.no_ignore_vcs = yes;
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_no_ignore() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(false, args.no_ignore_dot);
    assert_eq!(false, args.no_ignore_exclude);
    assert_eq!(false, args.no_ignore_global);
    assert_eq!(false, args.no_ignore_parent);
    assert_eq!(false, args.no_ignore_vcs);

    let args = parse_low_raw(["--no-ignore"]).unwrap();
    assert_eq!(true, args.no_ignore_dot);
    assert_eq!(true, args.no_ignore_exclude);
    assert_eq!(true, args.no_ignore_global);
    assert_eq!(true, args.no_ignore_parent);
    assert_eq!(true, args.no_ignore_vcs);

    let args = parse_low_raw(["--no-ignore", "--ignore"]).unwrap();
    assert_eq!(false, args.no_ignore_dot);
    assert_eq!(false, args.no_ignore_exclude);
    assert_eq!(false, args.no_ignore_global);
    assert_eq!(false, args.no_ignore_parent);
    assert_eq!(false, args.no_ignore_vcs);
}

/// --no-ignore-dot
#[derive(Debug)]
struct NoIgnoreDot;

impl Flag for NoIgnoreDot {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "no-ignore-dot"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("ignore-dot")
    }
    fn doc_category(&self) -> Category {
        Category::Filter
    }
    fn doc_short(&self) -> &'static str {
        r"Не использовать файлы .ignore или .rgignore."
    }
    fn doc_long(&self) -> &'static str {
        r"
Не уважать правила фильтрации из файлов \fB.ignore\fP или \fB.rgignore\fP.
.sp
Это не влияет на то, будет ли ripgrep игнорировать файлы и каталоги, имена
которых начинаются с точки. Для этого см. флаг \flag{hidden}. Этот флаг также
не влияет на то, будут ли уважаться правила фильтрации из файлов \fB.gitignore\fP.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.no_ignore_dot = v.unwrap_switch();
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_no_ignore_dot() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(false, args.no_ignore_dot);

    let args = parse_low_raw(["--no-ignore-dot"]).unwrap();
    assert_eq!(true, args.no_ignore_dot);

    let args = parse_low_raw(["--no-ignore-dot", "--ignore-dot"]).unwrap();
    assert_eq!(false, args.no_ignore_dot);
}

/// --no-ignore-exclude
#[derive(Debug)]
struct NoIgnoreExclude;

impl Flag for NoIgnoreExclude {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "no-ignore-exclude"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("ignore-exclude")
    }
    fn doc_category(&self) -> Category {
        Category::Filter
    }
    fn doc_short(&self) -> &'static str {
        r"Не использовать файлы локального исключения."
    }
    fn doc_long(&self) -> &'static str {
        r"
Не уважать правила фильтрации из файлов, которые настроены вручную для репозитория.
Например, это включает \fBgit\fP's \fB.git/info/exclude\fP.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.no_ignore_exclude = v.unwrap_switch();
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_no_ignore_exclude() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(false, args.no_ignore_exclude);

    let args = parse_low_raw(["--no-ignore-exclude"]).unwrap();
    assert_eq!(true, args.no_ignore_exclude);

    let args =
        parse_low_raw(["--no-ignore-exclude", "--ignore-exclude"]).unwrap();
    assert_eq!(false, args.no_ignore_exclude);
}

/// --no-ignore-files
#[derive(Debug)]
struct NoIgnoreFiles;

impl Flag for NoIgnoreFiles {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "no-ignore-files"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("ignore-files")
    }
    fn doc_category(&self) -> Category {
        Category::Filter
    }
    fn doc_short(&self) -> &'static str {
        r"Не использовать аргументы --ignore-file."
    }
    fn doc_long(&self) -> &'static str {
        r"
Когда установлен, любые флаги \flag{ignore-file}, даже те, которые идут после
этого флага, игнорируются.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.no_ignore_files = v.unwrap_switch();
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_no_ignore_files() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(false, args.no_ignore_files);

    let args = parse_low_raw(["--no-ignore-files"]).unwrap();
    assert_eq!(true, args.no_ignore_files);

    let args = parse_low_raw(["--no-ignore-files", "--ignore-files"]).unwrap();
    assert_eq!(false, args.no_ignore_files);
}

/// --no-ignore-global
#[derive(Debug)]
struct NoIgnoreGlobal;

impl Flag for NoIgnoreGlobal {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "no-ignore-global"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("ignore-global")
    }
    fn doc_category(&self) -> Category {
        Category::Filter
    }
    fn doc_short(&self) -> &'static str {
        r"Не использовать глобальные файлы игнорирования."
    }
    fn doc_long(&self) -> &'static str {
        r#"
Не уважать правила фильтрации из файлов игнорирования, которые поступают из
«глобальных» источников, таких как опция конфигурации \fBgit\fP
\fBcore.excludesFile\fP (которая по умолчанию равна \fB$HOME/.config/git/ignore\fP).
"#
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.no_ignore_global = v.unwrap_switch();
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_no_ignore_global() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(false, args.no_ignore_global);

    let args = parse_low_raw(["--no-ignore-global"]).unwrap();
    assert_eq!(true, args.no_ignore_global);

    let args =
        parse_low_raw(["--no-ignore-global", "--ignore-global"]).unwrap();
    assert_eq!(false, args.no_ignore_global);
}

/// --no-ignore-messages
#[derive(Debug)]
struct NoIgnoreMessages;

impl Flag for NoIgnoreMessages {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "no-ignore-messages"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("ignore-messages")
    }
    fn doc_category(&self) -> Category {
        Category::Logging
    }
    fn doc_short(&self) -> &'static str {
        r"Подавить сообщения об ошибках парсинга gitignore."
    }
    fn doc_long(&self) -> &'static str {
        r"
Когда этот флаг включён, все сообщения об ошибках, связанные с парсингом файлов
игнорирования, подавляются. По умолчанию сообщения об ошибках печатаются в stderr.
В случаях, когда эти ошибки ожидаются, этот флаг может быть использован, чтобы
избежать шума, производимого сообщениями.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.no_ignore_messages = v.unwrap_switch();
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_no_ignore_messages() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(false, args.no_ignore_messages);

    let args = parse_low_raw(["--no-ignore-messages"]).unwrap();
    assert_eq!(true, args.no_ignore_messages);

    let args =
        parse_low_raw(["--no-ignore-messages", "--ignore-messages"]).unwrap();
    assert_eq!(false, args.no_ignore_messages);
}

/// --no-ignore-parent
#[derive(Debug)]
struct NoIgnoreParent;

impl Flag for NoIgnoreParent {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "no-ignore-parent"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("ignore-parent")
    }
    fn doc_category(&self) -> Category {
        Category::Filter
    }
    fn doc_short(&self) -> &'static str {
        r"Не использовать файлы игнорирования в родительских каталогах."
    }
    fn doc_long(&self) -> &'static str {
        r"
Когда этот флаг установлен, правила фильтрации из файлов игнорирования, найденных
в родительских каталогах, не уважаются. По умолчанию ripgrep будет подниматься
по родительским каталогам текущего рабочего каталога, чтобы найти любые
применимые файлы игнорирования, которые должны быть применены. В некоторых
случаях это может быть нежелательно.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.no_ignore_parent = v.unwrap_switch();
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_no_ignore_parent() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(false, args.no_ignore_parent);

    let args = parse_low_raw(["--no-ignore-parent"]).unwrap();
    assert_eq!(true, args.no_ignore_parent);

    let args =
        parse_low_raw(["--no-ignore-parent", "--ignore-parent"]).unwrap();
    assert_eq!(false, args.no_ignore_parent);
}

/// --no-ignore-vcs
#[derive(Debug)]
struct NoIgnoreVcs;

impl Flag for NoIgnoreVcs {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "no-ignore-vcs"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("ignore-vcs")
    }
    fn doc_category(&self) -> Category {
        Category::Filter
    }
    fn doc_short(&self) -> &'static str {
        r"Не использовать файлы игнорирования от системы контроля версий."
    }
    fn doc_long(&self) -> &'static str {
        r"
Когда предоставлен, правила фильтрации из файлов игнорирования системы контроля
версий (например, \fB.gitignore\fP) не уважаются. По умолчанию ripgrep уважает
правила игнорирования \fBgit\fP для автоматической фильтрации. В некоторых случаях
может быть нежелательно уважать правила игнорирования системы контроля версий и
вместо этого уважать только правила в \fB.ignore\fP или \fB.rgignore\fP.
.sp
Обратите внимание, что этот флаг не влияет напрямую на фильтрацию файлов или
папок системы контроля версий, которые начинаются с точки (\fB.\fP), таких как
\fB.git\fP. На них влияют флаг \flag{hidden} и связанные с ним флаги.
.sp
Этот флаг также подразумевает \flag{no-ignore-parent} для файлов игнорирования
системы контроля версий.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.no_ignore_vcs = v.unwrap_switch();
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_no_ignore_vcs() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(false, args.no_ignore_vcs);

    let args = parse_low_raw(["--no-ignore-vcs"]).unwrap();
    assert_eq!(true, args.no_ignore_vcs);

    let args = parse_low_raw(["--no-ignore-vcs", "--ignore-vcs"]).unwrap();
    assert_eq!(false, args.no_ignore_vcs);
}

/// --no-messages
#[derive(Debug)]
struct NoMessages;

impl Flag for NoMessages {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "no-messages"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("messages")
    }
    fn doc_category(&self) -> Category {
        Category::Logging
    }
    fn doc_short(&self) -> &'static str {
        r"Подавить некоторые сообщения об ошибках."
    }
    fn doc_long(&self) -> &'static str {
        r"
Этот флаг подавляет некоторые сообщения об ошибках. В частности, сообщения,
связанные с неудачным открытием и чтением файлов. Сообщения об ошибках, связанные
с синтаксисом шаблона, всё ещё показываются.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.no_messages = v.unwrap_switch();
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_no_messages() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(false, args.no_messages);

    let args = parse_low_raw(["--no-messages"]).unwrap();
    assert_eq!(true, args.no_messages);

    let args = parse_low_raw(["--no-messages", "--messages"]).unwrap();
    assert_eq!(false, args.no_messages);
}

/// --no-pcre2-unicode
#[derive(Debug)]
struct NoPcre2Unicode;

impl Flag for NoPcre2Unicode {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "no-pcre2-unicode"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("pcre2-unicode")
    }
    fn doc_category(&self) -> Category {
        Category::Search
    }
    fn doc_short(&self) -> &'static str {
        r"(УСТАРЕЛО) Отключить режим Unicode для PCRE2."
    }
    fn doc_long(&self) -> &'static str {
        r"
УСТАРЕЛО. Используйте вместо этого \flag{no-unicode}.
.sp
Обратите внимание, что режим Unicode включён по умолчанию.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.no_unicode = v.unwrap_switch();
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_no_pcre2_unicode() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(false, args.no_unicode);

    let args = parse_low_raw(["--no-pcre2-unicode"]).unwrap();
    assert_eq!(true, args.no_unicode);

    let args =
        parse_low_raw(["--no-pcre2-unicode", "--pcre2-unicode"]).unwrap();
    assert_eq!(false, args.no_unicode);
}

/// --no-require-git
#[derive(Debug)]
struct NoRequireGit;

impl Flag for NoRequireGit {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "no-require-git"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("require-git")
    }
    fn doc_category(&self) -> Category {
        Category::Filter
    }
    fn doc_short(&self) -> &'static str {
        r"Использовать .gitignore вне репозиториев git."
    }
    fn doc_long(&self) -> &'static str {
        r"
Когда этот флаг предоставлен, файлы игнорирования системы контроля версий, такие
как \fB.gitignore\fP, уважаются, даже если репозиторий \fBgit\fP отсутствует.
.sp
По умолчанию ripgrep будет уважать правила фильтрации из файлов игнорирования
системы контроля версий только когда ripgrep обнаруживает, что поиск выполняется
внутри репозитория системы контроля версий. Например, когда обнаруживается
каталог \fB.git\fP.
.sp
Этот флаг ослабляет ограничение по умолчанию. Например, это может быть полезно,
когда содержимое репозитория \fBgit\fP хранится или скопировано где-то, но где
состояние репозитория отсутствует.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.no_require_git = v.unwrap_switch();
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_no_require_git() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(false, args.no_require_git);

    let args = parse_low_raw(["--no-require-git"]).unwrap();
    assert_eq!(true, args.no_require_git);

    let args = parse_low_raw(["--no-require-git", "--require-git"]).unwrap();
    assert_eq!(false, args.no_require_git);
}

/// --no-unicode
#[derive(Debug)]
struct NoUnicode;

impl Flag for NoUnicode {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "no-unicode"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("unicode")
    }
    fn doc_category(&self) -> Category {
        Category::Search
    }
    fn doc_short(&self) -> &'static str {
        r"Отключить режим Unicode."
    }
    fn doc_long(&self) -> &'static str {
        r#"
Этот флаг отключает режим Unicode для всех шаблонов, переданных в ripgrep.
.sp
По умолчанию ripgrep будет включать «режим Unicode» во всех своих регулярных
выражениях. Это имеет ряд последствий:
.sp
.IP \(bu 3n
\fB.\fP будет сопоставлять только валидные UTF-8-кодированные Unicode-скалярные
значения.
.sp
.IP \(bu 3n
Классы, такие как \fB\\w\fP, \fB\\s\fP, \fB\\d\fP, все осведомлены о Unicode и
гораздо больше, чем их ASCII-версии.
.sp
.IP \(bu 3n
Сопоставление без учёта регистра будет использовать приведение регистра Unicode.
.sp
.IP \(bu 3n
Доступен большой массив классов, таких как \fB\\p{Emoji}\fP. (Хотя конкретный
набор доступных классов варьируется в зависимости от движка регулярных выражений.
В общем, движок регулярных выражений по умолчанию имеет больше доступных классов.)
.sp
.IP \(bu 3n
Границы слов (\fB\\b\fP и \fB\\B\fP) используют Unicode-определение символа слова.
.PP
В некоторых случаях может быть желательно отключить эти вещи. Этот флаг сделает
именно это. Например, режим Unicode может иногда оказывать негативное влияние на
производительность, особенно когда такие вещи, как \fB\\w\fP, используются часто
(включая через ограниченные повторения, такие как \fB\\w{100}\fP), когда требуется
только их ASCII-интерпретация.
"#
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.no_unicode = v.unwrap_switch();
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_no_unicode() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(false, args.no_unicode);

    let args = parse_low_raw(["--no-unicode"]).unwrap();
    assert_eq!(true, args.no_unicode);

    let args = parse_low_raw(["--no-unicode", "--unicode"]).unwrap();
    assert_eq!(false, args.no_unicode);

    let args = parse_low_raw(["--no-unicode", "--pcre2-unicode"]).unwrap();
    assert_eq!(false, args.no_unicode);

    let args = parse_low_raw(["--no-pcre2-unicode", "--unicode"]).unwrap();
    assert_eq!(false, args.no_unicode);
}

/// -0/--null
#[derive(Debug)]
struct Null;

impl Flag for Null {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'0')
    }
    fn name_long(&self) -> &'static str {
        "null"
    }
    fn doc_category(&self) -> Category {
        Category::Output
    }
    fn doc_short(&self) -> &'static str {
        r"Вывести байт NUL после путей к файлам."
    }
    fn doc_long(&self) -> &'static str {
        r"
Всякий раз, когда путь к файлу выводится, следовать за ним байтом \fBNUL\fP.
Это включает вывод путей к файлам перед совпадениями и при выводе списка
совпадающих файлов, таких как с \flag{count}, \flag{files-with-matches} и
\flag{files}. Эта опция полезна для использования с \fBxargs\fP.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        assert!(v.unwrap_switch(), "--null has no negation");
        args.null = true;
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_null() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(false, args.null);

    let args = parse_low_raw(["--null"]).unwrap();
    assert_eq!(true, args.null);

    let args = parse_low_raw(["-0"]).unwrap();
    assert_eq!(true, args.null);
}

/// --null-data
#[derive(Debug)]
struct NullData;

impl Flag for NullData {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "null-data"
    }
    fn doc_category(&self) -> Category {
        Category::Search
    }
    fn doc_short(&self) -> &'static str {
        r"Использовать NUL как терминатор строки."
    }
    fn doc_long(&self) -> &'static str {
        r"
Включение этого флага заставляет ripgrep использовать \fBNUL\fP как терминатор
строки вместо значения по умолчанию \fP\\n\fP.
.sp
Это полезно при поиске больших бинарных файлов, которые в противном случае имели
бы очень длинные строки, если бы \fB\\n\fP использовался как терминатор строки.
В частности, ripgrep требует, чтобы как минимум каждая строка помещалась в память.
Использование \fBNUL\fP вместо этого может быть полезным временным решением для
поддержания низких требований к памяти и избежания условий OOM (нехватка памяти).
.sp
Это также полезно для обработки данных, разделённых NUL, таких как те, которые
испускаются при использовании флага \flag{null} ripgrep или флага \fB\-\-print0\fP
\fBfind\fP.
.sp
Использование этого флага подразумевает \flag{text}. Он также переопределяет
\flag{crlf}.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        assert!(v.unwrap_switch(), "--null-data has no negation");
        args.crlf = false;
        args.null_data = true;
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_null_data() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(false, args.null_data);

    let args = parse_low_raw(["--null-data"]).unwrap();
    assert_eq!(true, args.null_data);

    let args = parse_low_raw(["--null-data", "--crlf"]).unwrap();
    assert_eq!(false, args.null_data);
    assert_eq!(true, args.crlf);

    let args = parse_low_raw(["--crlf", "--null-data"]).unwrap();
    assert_eq!(true, args.null_data);
    assert_eq!(false, args.crlf);

    let args = parse_low_raw(["--null-data", "--no-crlf"]).unwrap();
    assert_eq!(true, args.null_data);
    assert_eq!(false, args.crlf);
}

/// --one-file-system
#[derive(Debug)]
struct OneFileSystem;

impl Flag for OneFileSystem {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "one-file-system"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("no-one-file-system")
    }
    fn doc_category(&self) -> Category {
        Category::Filter
    }
    fn doc_short(&self) -> &'static str {
        r"Пропускать каталоги на других файловых системах."
    }
    fn doc_long(&self) -> &'static str {
        r"
Когда включено, ripgrep не будет пересекать границы файловых систем относительно
того, где начался поиск.
.sp
Обратите внимание, что это применяется к каждому аргументу пути, предоставленному
ripgrep. Например, в команде
.sp
.EX
    rg \-\-one\-file\-system /foo/bar /quux/baz
.EE
.sp
ripgrep будет искать как \fI/foo/bar\fP, так и \fI/quux/baz\fP, даже если они
находятся на разных файловых системах, но не будет пересекать границу файловой
системы при обходе дерева каталогов каждого пути.
.sp
Это похоже на флаг \fB\-xdev\fP или \fB\-mount\fP \fBfind\fP.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.one_file_system = v.unwrap_switch();
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_one_file_system() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(false, args.one_file_system);

    let args = parse_low_raw(["--one-file-system"]).unwrap();
    assert_eq!(true, args.one_file_system);

    let args =
        parse_low_raw(["--one-file-system", "--no-one-file-system"]).unwrap();
    assert_eq!(false, args.one_file_system);
}

/// -o/--only-matching
#[derive(Debug)]
struct OnlyMatching;

impl Flag for OnlyMatching {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'o')
    }
    fn name_long(&self) -> &'static str {
        "only-matching"
    }
    fn doc_category(&self) -> Category {
        Category::Output
    }
    fn doc_short(&self) -> &'static str {
        r"Вывести только совпавшие части строки."
    }
    fn doc_long(&self) -> &'static str {
        r"
Вывести только совпавшие (непустые) части совпадающей строки, причём каждая такая
часть на отдельной строке вывода.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        assert!(v.unwrap_switch(), "--only-matching does not have a negation");
        args.only_matching = true;
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_only_matching() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(false, args.only_matching);

    let args = parse_low_raw(["--only-matching"]).unwrap();
    assert_eq!(true, args.only_matching);

    let args = parse_low_raw(["-o"]).unwrap();
    assert_eq!(true, args.only_matching);
}

/// --path-separator
#[derive(Debug)]
struct PathSeparator;

impl Flag for PathSeparator {
    fn is_switch(&self) -> bool {
        false
    }
    fn name_long(&self) -> &'static str {
        "path-separator"
    }
    fn doc_variable(&self) -> Option<&'static str> {
        Some("SEPARATOR")
    }
    fn doc_category(&self) -> Category {
        Category::Output
    }
    fn doc_short(&self) -> &'static str {
        r"Установить разделитель путей для вывода путей."
    }
    fn doc_long(&self) -> &'static str {
        r"
Установить разделитель путей для использования при выводе путей к файлам. По
умолчанию это разделитель путей вашей платформы, который равен \fB/\fP в Unix и
\fB\\\fP в Windows. Этот флаг предназначен для переопределения значения по
умолчанию, когда среда этого требует (например, cygwin). Разделитель путей
ограничен одним байтом.
.sp
Установка этого флага в пустую строку возвращает его к поведению по умолчанию.
То есть разделитель путей автоматически выбирается на основе среды.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        let s = convert::string(v.unwrap_value())?;
        let raw = Vec::unescape_bytes(&s);
        args.path_separator = if raw.is_empty() {
            None
        } else if raw.len() == 1 {
            Some(raw[0])
        } else {
            anyhow::bail!(
                "A path separator must be exactly one byte, but \
                 the given separator is {len} bytes: {sep}\n\
                 In some shells on Windows '/' is automatically \
                 expanded. Use '//' instead.",
                len = raw.len(),
                sep = s,
            )
        };
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_path_separator() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(None, args.path_separator);

    let args = parse_low_raw(["--path-separator", "/"]).unwrap();
    assert_eq!(Some(b'/'), args.path_separator);

    let args = parse_low_raw(["--path-separator", r"\"]).unwrap();
    assert_eq!(Some(b'\\'), args.path_separator);

    let args = parse_low_raw(["--path-separator", r"\x00"]).unwrap();
    assert_eq!(Some(0), args.path_separator);

    let args = parse_low_raw(["--path-separator", r"\0"]).unwrap();
    assert_eq!(Some(0), args.path_separator);

    let args = parse_low_raw(["--path-separator", "\x00"]).unwrap();
    assert_eq!(Some(0), args.path_separator);

    let args = parse_low_raw(["--path-separator", "\0"]).unwrap();
    assert_eq!(Some(0), args.path_separator);

    let args =
        parse_low_raw(["--path-separator", r"\x00", "--path-separator=/"])
            .unwrap();
    assert_eq!(Some(b'/'), args.path_separator);

    let result = parse_low_raw(["--path-separator", "foo"]);
    assert!(result.is_err(), "{result:?}");

    let result = parse_low_raw(["--path-separator", r"\\x00"]);
    assert!(result.is_err(), "{result:?}");
}

/// --passthru
#[derive(Debug)]
struct Passthru;

impl Flag for Passthru {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "passthru"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &["passthrough"]
    }
    fn doc_category(&self) -> Category {
        Category::Output
    }
    fn doc_short(&self) -> &'static str {
        r"Вывести как совпадающие, так и несовпадающие строки."
    }
    fn doc_long(&self) -> &'static str {
        r#"
Вывести как совпадающие, так и несовпадающие строки.
.sp
Другой способ достичь подобного эффекта — изменить ваш шаблон для сопоставления
пустой строки. Например, если вы ищете с помощью \fBrg\fP \fIfoo\fP, то
использование \fBrg\fP \fB'^|\fP\fIfoo\fP\fB'\fP вместо этого будет выводить
каждую строку в каждом искомом файле, но только вхождения \fIfoo\fP будут
подсвечены. Этот флаг включает то же поведение без необходимости изменять шаблон.
.sp
Альтернативное написание этого флага — \fB\-\-passthrough\fP.
.sp
Это переопределяет флаги \flag{context}, \flag{after-context} и
\flag{before-context}.
"#
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        assert!(v.unwrap_switch(), "--passthru has no negation");
        args.context = ContextMode::Passthru;
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_passthru() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(ContextMode::default(), args.context);

    let args = parse_low_raw(["--passthru"]).unwrap();
    assert_eq!(ContextMode::Passthru, args.context);

    let args = parse_low_raw(["--passthrough"]).unwrap();
    assert_eq!(ContextMode::Passthru, args.context);
}

/// -P/--pcre2
#[derive(Debug)]
struct PCRE2;

impl Flag for PCRE2 {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'P')
    }
    fn name_long(&self) -> &'static str {
        "pcre2"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("no-pcre2")
    }
    fn doc_category(&self) -> Category {
        Category::Search
    }
    fn doc_short(&self) -> &'static str {
        r"Включить сопоставление PCRE2."
    }
    fn doc_long(&self) -> &'static str {
        r"
Когда этот флаг присутствует, ripgrep будет использовать движок регулярных
выражений PCRE2 вместо своего движка регулярных выражений по умолчанию.
.sp
Это обычно полезно, когда вы хот��те использовать такие функции, как просмотр
окружения или обратные ссылки.
.sp
Использование этого флага то же самое, что передача \fB\-\-engine=pcre2\fP.
Пользователи могут вместо этого выбрать передачу \fB\-\-engine=auto\fP, чтобы
попросить ripgrep автоматически выбрать правильный движок регулярных выражений
на основе предоставленных ��аб��он��в. ��то�� флаг �� флаг \flag{engine} переопределяют
друг ��ру��а.
.sp
Обра��ит�� внимание, что PCRE2 — эт�� дополнительная функция ripgrep. Если PCRE2
не был включён в вашу сборку ripgrep, то использование этого флага приведёт к
тому, что ripgrep напечатает сообщение об ошибке и выйдет. PCRE2 также может
иметь худший пользовательский опыт в некоторых случаях, поскольку у него меньше
API интроспекции, чем у движка регулярных выражений по умолчанию ripgrep.
Например, если вы используете \fB\\n\fP в регулярном выражении PCRE2 без флага
\flag{multiline}, то ripgrep молча не сможет ничего сопоставить вместо того,
чтобы немедленно сообщить об ошибке (как он делает с движком регулярных
выражений по умолчанию).
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.engine = if v.unwrap_switch() {
            EngineChoice::PCRE2
        } else {
            EngineChoice::Default
        };
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_pcre2() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(EngineChoice::Default, args.engine);

    let args = parse_low_raw(["--pcre2"]).unwrap();
    assert_eq!(EngineChoice::PCRE2, args.engine);

    let args = parse_low_raw(["-P"]).unwrap();
    assert_eq!(EngineChoice::PCRE2, args.engine);

    let args = parse_low_raw(["-P", "--no-pcre2"]).unwrap();
    assert_eq!(EngineChoice::Default, args.engine);

    let args = parse_low_raw(["--engine=auto", "-P", "--no-pcre2"]).unwrap();
    assert_eq!(EngineChoice::Default, args.engine);

    let args = parse_low_raw(["-P", "--engine=auto"]).unwrap();
    assert_eq!(EngineChoice::Auto, args.engine);
}

/// --pcre2-version
#[derive(Debug)]
struct PCRE2Version;

impl Flag for PCRE2Version {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "pcre2-version"
    }
    fn doc_category(&self) -> Category {
        Category::OtherBehaviors
    }
    fn doc_short(&self) -> &'static str {
        r"Вывести версию PCRE2, которую использует ripgrep."
    }
    fn doc_long(&self) -> &'static str {
        r"
Когда этот флаг присутствует, ripgrep напечатает используемую версию PCRE2
вместе с другой информацией, а затем выйдет. Если PCRE2 недоступен, то ripgrep
напечатает сообщение об ошибке и выйдет с кодом ошибки.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        assert!(v.unwrap_switch(), "--pcre2-version has no negation");
        args.special = Some(SpecialMode::VersionPCRE2);
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_pcre2_version() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(None, args.special);

    let args = parse_low_raw(["--pcre2-version"]).unwrap();
    assert_eq!(Some(SpecialMode::VersionPCRE2), args.special);
}

/// --pre
#[derive(Debug)]
struct Pre;

impl Flag for Pre {
    fn is_switch(&self) -> bool {
        false
    }
    fn name_long(&self) -> &'static str {
        "pre"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("no-pre")
    }
    fn doc_variable(&self) -> Option<&'static str> {
        Some("COMMAND")
    }
    fn doc_category(&self) -> Category {
        Category::Input
    }
    fn doc_short(&self) -> &'static str {
        r"Искать вывод COMMAND для каждого PATH."
    }
    fn doc_long(&self) -> &'static str {
        r#"
Для каждого входного \fIPATH\fP этот флаг заставляет ripgrep искать стандартный
вывод \fICOMMAND\fP \fIPATH\fP вместо содержимого \fIPATH\fP. Эта опция ожидает,
что программа \fICOMMAND\fP будет либо путём, либо доступна в вашем \fBPATH\fP.
Либо пустая строка \fICOMMAND\fP, либо флаг \fB\-\-no\-pre\fP отключат это
поведение.
.sp
.TP 12
\fBПРЕДУПРЕЖДЕНИЕ\fP
Когда этот флаг установлен, ripgrep безусловно запустит процесс для каждого
файла, который ищется. Следовательно, это может повлечь ненужно большой штраф
производительности, если вам не нужна гибкость, предлагаемая этим флагом. Одним
из возможных способов смягчения этого является использование флага \flag{pre-glob}
для ограничения того, с какими файлами запускается препроцессор.
.PP
Препроцессор не запускается, когда ripgrep ищет stdin.
.sp
При поиске по наборам файлов, которые могут требовать один из нескольких
препроцессоров, \fICOMMAND\fP должен быть программой-обёрткой, которая сначала
классифицирует \fIPATH\fP на основе магических чисел/содержимого или на основе
имени \fIPATH\fP, а затем направляет к соответствующему препроцессору. Каждый
\fICOMMAND\fP также имеет свой стандартный ввод, подключённый к \fIPATH\fP для
удобства.
.sp
Например, сценарий оболочки для \fICOMMAND\fP может выглядеть так:
.sp
.EX
    case "$1" in
    *.pdf)
        exec pdftotext "$1" -
        ;;
    *)
        case $(file "$1") in
        *Zstandard*)
            exec pzstd -cdq
            ;;
        *)
            exec cat
            ;;
        esac
        ;;
    esac
.EE
.sp
Приведённый выше сценарий использует \fBpdftotext\fP для преобразования PDF-файла
в обычный текст. Для всех остальных файлов сценарий использует утилиту \fBfile\fP
для определения типа файла на основе его содержимого. Если это сжатый файл в
формате Zstandard, то \fBpzstd\fP используется для распаковки содержимого в stdout.
.sp
Это переопределяет флаг \flag{search-zip}.
"#
    }
    fn completion_type(&self) -> CompletionType {
        CompletionType::Executable
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        let path = match v {
            FlagValue::Value(v) => PathBuf::from(v),
            FlagValue::Switch(yes) => {
                assert!(!yes, "there is no affirmative switch for --pre");
                args.pre = None;
                return Ok(());
            }
        };
        args.pre = if path.as_os_str().is_empty() { None } else { Some(path) };
        if args.pre.is_some() {
            args.search_zip = false;
        }
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_pre() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(None, args.pre);

    let args = parse_low_raw(["--pre", "foo/bar"]).unwrap();
    assert_eq!(Some(PathBuf::from("foo/bar")), args.pre);

    let args = parse_low_raw(["--pre", ""]).unwrap();
    assert_eq!(None, args.pre);

    let args = parse_low_raw(["--pre", "foo/bar", "--pre", ""]).unwrap();
    assert_eq!(None, args.pre);

    let args = parse_low_raw(["--pre", "foo/bar", "--pre="]).unwrap();
    assert_eq!(None, args.pre);

    let args = parse_low_raw(["--pre", "foo/bar", "--no-pre"]).unwrap();
    assert_eq!(None, args.pre);
}

/// --pre-glob
#[derive(Debug)]
struct PreGlob;

impl Flag for PreGlob {
    fn is_switch(&self) -> bool {
        false
    }
    fn name_long(&self) -> &'static str {
        "pre-glob"
    }
    fn doc_variable(&self) -> Option<&'static str> {
        Some("GLOB")
    }
    fn doc_category(&self) -> Category {
        Category::Input
    }
    fn doc_short(&self) -> &'static str {
        r"Включить или исключить файлы из препроцессора."
    }
    fn doc_long(&self) -> &'static str {
        r#"
Этот флаг работает в сочетании с флагом \flag{pre}. А именно, когда один или
несколько флагов \flag{pre-glob} предоставлены, то только файлы, которые
соответствуют заданному набору глобов, будут переданы команде, указанной флагом
\flag{pre}. Любые несовпадающие файлы будут искаться без использования команды
препроцессора.
.sp
Этот флаг полезен при поиске многих файлов с флагом \flag{pre}. А именно, он
предоставляет возможность избежать накладных расходов процесса для файлов,
которые не нуждаются в препроцессинге. Например, учитывая следующий сценарий
оболочки, \fIpre-pdftotext\fP:
.sp
.EX
    #!/bin/sh
    pdftotext "$1" -
.EE
.sp
тогда возможно использовать \fB\-\-pre\fP \fIpre-pdftotext\fP
\fB\-\-pre\-glob\fP '\fI*.pdf\fP', чтобы заставить ripgrep выполнять команду
\fIpre-pdftotext\fP только на файлах с расширением \fI.pdf\fP.
.sp
Может быть использовано несколько флагов \flag{pre-glob}. Правила glob
сопоставляются с глобами \fBgitignore\fP. Предшествуйте glob символом \fB!\fP,
чтобы исключить его.
.sp
Этот флаг не имеет эффекта, если флаг \flag{pre} не используется.
"#
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        let glob = convert::string(v.unwrap_value())?;
        args.pre_glob.push(glob);
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_pre_glob() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(Vec::<String>::new(), args.pre_glob);

    let args = parse_low_raw(["--pre-glob", "*.pdf"]).unwrap();
    assert_eq!(vec!["*.pdf".to_string()], args.pre_glob);

    let args =
        parse_low_raw(["--pre-glob", "*.pdf", "--pre-glob=foo"]).unwrap();
    assert_eq!(vec!["*.pdf".to_string(), "foo".to_string()], args.pre_glob);
}

/// -p/--pretty
#[derive(Debug)]
struct Pretty;

impl Flag for Pretty {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'p')
    }
    fn name_long(&self) -> &'static str {
        "pretty"
    }
    fn doc_category(&self) -> Category {
        Category::Output
    }
    fn doc_short(&self) -> &'static str {
        r"Псевдоним для цветов, заголовков и номеров строк."
    }
    fn doc_long(&self) -> &'static str {
        r"
Это псевдоним для \fB\-\-color=always \-\-heading \-\-line\-number\fP. Этот флаг
полезен, когда вы всё ещё хотите красивый вывод, даже если вы передаёте ripgrep
в другую программу или файл. Например: \fBrg -p \fP\fIfoo\fP \fB| less -R\fP.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        assert!(v.unwrap_switch(), "--pretty has no negation");
        args.color = ColorChoice::Always;
        args.heading = Some(true);
        args.line_number = Some(true);
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_pretty() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(ColorChoice::Auto, args.color);
    assert_eq!(None, args.heading);
    assert_eq!(None, args.line_number);

    let args = parse_low_raw(["--pretty"]).unwrap();
    assert_eq!(ColorChoice::Always, args.color);
    assert_eq!(Some(true), args.heading);
    assert_eq!(Some(true), args.line_number);

    let args = parse_low_raw(["-p"]).unwrap();
    assert_eq!(ColorChoice::Always, args.color);
    assert_eq!(Some(true), args.heading);
    assert_eq!(Some(true), args.line_number);
}

/// -q/--quiet
#[derive(Debug)]
struct Quiet;

impl Flag for Quiet {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'q')
    }
    fn name_long(&self) -> &'static str {
        "quiet"
    }
    fn doc_category(&self) -> Category {
        Category::Output
    }
    fn doc_short(&self) -> &'static str {
        r"Не выводить ничего в stdout."
    }
    fn doc_long(&self) -> &'static str {
        r"
Не выводить ничего в stdout. Если совпадение найдено в файле, то ripgrep
прекратит поиск. Это полезно, когда ripgrep используется только для его кода
выхода (который будет кодом ошибки, если совпадения не найдены).
.sp
Когда используется \flag{files}, ripgrep прекратит поиск файлов после нахождения
первого файла, который не соответствует никаким правилам игнорирования.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        assert!(v.unwrap_switch(), "--quiet has no negation");
        args.quiet = true;
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_quiet() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(false, args.quiet);

    let args = parse_low_raw(["--quiet"]).unwrap();
    assert_eq!(true, args.quiet);

    let args = parse_low_raw(["-q"]).unwrap();
    assert_eq!(true, args.quiet);

    // flags like -l and --json cannot override -q, regardless of order
    let args = parse_low_raw(["-q", "--json"]).unwrap();
    assert_eq!(true, args.quiet);

    let args = parse_low_raw(["-q", "--files-with-matches"]).unwrap();
    assert_eq!(true, args.quiet);

    let args = parse_low_raw(["-q", "--files-without-match"]).unwrap();
    assert_eq!(true, args.quiet);

    let args = parse_low_raw(["-q", "--count"]).unwrap();
    assert_eq!(true, args.quiet);

    let args = parse_low_raw(["-q", "--count-matches"]).unwrap();
    assert_eq!(true, args.quiet);
}

/// --regex-size-limit
#[derive(Debug)]
struct RegexSizeLimit;

impl Flag for RegexSizeLimit {
    fn is_switch(&self) -> bool {
        false
    }
    fn name_long(&self) -> &'static str {
        "regex-size-limit"
    }
    fn doc_variable(&self) -> Option<&'static str> {
        Some("NUM+SUFFIX?")
    }
    fn doc_category(&self) -> Category {
        Category::Search
    }
    fn doc_short(&self) -> &'static str {
        r"Предел размера скомпилированного регулярного выражения."
    }
    fn doc_long(&self) -> &'static str {
        r"
Предел размера скомпилированного регулярного выражения, где скомпилированное
регулярное выражение обычно соответствует одному объекту в памяти, который может
сопоставить все шаблоны, предоставленные ripgrep. Предел по умолчанию достаточно
щедрый, чтобы большинство разумных шаблонов (или даже небольшое их количество)
поместились.
.sp
Это полезно изменить, когда вы явно хотите позволить ripgrep потратить потенциально
гораздо больше времени и/или памяти на построение сопоставителя регулярных выражений.
.sp
Формат ввода принимает суффиксы \fBK\fP, \fBM\fP или \fBG\fP, которые соответствуют
килобайтам, мегабайтам и гигабайтам соответственно. Если суффикс не предоставлен,
ввод рассматривается как байты.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        let v = v.unwrap_value();
        args.regex_size_limit = Some(convert::human_readable_usize(&v)?);
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_regex_size_limit() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(None, args.regex_size_limit);

    #[cfg(target_pointer_width = "64")]
    {
        let args = parse_low_raw(["--regex-size-limit", "9G"]).unwrap();
        assert_eq!(Some(9 * (1 << 30)), args.regex_size_limit);

        let args = parse_low_raw(["--regex-size-limit=9G"]).unwrap();
        assert_eq!(Some(9 * (1 << 30)), args.regex_size_limit);

        let args =
            parse_low_raw(["--regex-size-limit=9G", "--regex-size-limit=0"])
                .unwrap();
        assert_eq!(Some(0), args.regex_size_limit);
    }

    let args = parse_low_raw(["--regex-size-limit=0K"]).unwrap();
    assert_eq!(Some(0), args.regex_size_limit);

    let args = parse_low_raw(["--regex-size-limit=0M"]).unwrap();
    assert_eq!(Some(0), args.regex_size_limit);

    let args = parse_low_raw(["--regex-size-limit=0G"]).unwrap();
    assert_eq!(Some(0), args.regex_size_limit);

    let result =
        parse_low_raw(["--regex-size-limit", "9999999999999999999999"]);
    assert!(result.is_err(), "{result:?}");

    let result = parse_low_raw(["--regex-size-limit", "9999999999999999G"]);
    assert!(result.is_err(), "{result:?}");
}

/// -e/--regexp
#[derive(Debug)]
struct Regexp;

impl Flag for Regexp {
    fn is_switch(&self) -> bool {
        false
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'e')
    }
    fn name_long(&self) -> &'static str {
        "regexp"
    }
    fn doc_variable(&self) -> Option<&'static str> {
        Some("PATTERN")
    }
    fn doc_category(&self) -> Category {
        Category::Input
    }
    fn doc_short(&self) -> &'static str {
        r"A pattern to search for."
    }
    fn doc_long(&self) -> &'static str {
        r"
A pattern to search for. This option can be provided multiple times, where
all patterns given are searched, in addition to any patterns provided by
\flag{file}. Lines matching at least one of the provided patterns are printed.
This flag can also be used when searching for patterns that start with a dash.
.sp
For example, to search for the literal \fB\-foo\fP:
.sp
.EX
    rg \-e \-foo
.EE
.sp
You can also use the special \fB\-\-\fP delimiter to indicate that no more
flags will be provided. Namely, the following is equivalent to the above:
.sp
.EX
    rg \-\- \-foo
.EE
.sp
When \flag{file} or \flag{regexp} is used, then ripgrep treats all positional
arguments as files or directories to search.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        let regexp = convert::string(v.unwrap_value())?;
        args.patterns.push(PatternSource::Regexp(regexp));
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_regexp() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(Vec::<PatternSource>::new(), args.patterns);

    let args = parse_low_raw(["--regexp", "foo"]).unwrap();
    assert_eq!(vec![PatternSource::Regexp("foo".to_string())], args.patterns);

    let args = parse_low_raw(["--regexp=foo"]).unwrap();
    assert_eq!(vec![PatternSource::Regexp("foo".to_string())], args.patterns);

    let args = parse_low_raw(["-e", "foo"]).unwrap();
    assert_eq!(vec![PatternSource::Regexp("foo".to_string())], args.patterns);

    let args = parse_low_raw(["-efoo"]).unwrap();
    assert_eq!(vec![PatternSource::Regexp("foo".to_string())], args.patterns);

    let args = parse_low_raw(["--regexp", "-foo"]).unwrap();
    assert_eq!(vec![PatternSource::Regexp("-foo".to_string())], args.patterns);

    let args = parse_low_raw(["--regexp=-foo"]).unwrap();
    assert_eq!(vec![PatternSource::Regexp("-foo".to_string())], args.patterns);

    let args = parse_low_raw(["-e", "-foo"]).unwrap();
    assert_eq!(vec![PatternSource::Regexp("-foo".to_string())], args.patterns);

    let args = parse_low_raw(["-e-foo"]).unwrap();
    assert_eq!(vec![PatternSource::Regexp("-foo".to_string())], args.patterns);

    let args = parse_low_raw(["--regexp=foo", "--regexp", "bar"]).unwrap();
    assert_eq!(
        vec![
            PatternSource::Regexp("foo".to_string()),
            PatternSource::Regexp("bar".to_string())
        ],
        args.patterns
    );

    // While we support invalid UTF-8 arguments in general, patterns must be
    // valid UTF-8.
    #[cfg(unix)]
    {
        use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

        let bytes = &[b'A', 0xFF, b'Z'][..];
        let result = parse_low_raw([
            OsStr::from_bytes(b"-e"),
            OsStr::from_bytes(bytes),
        ]);
        assert!(result.is_err(), "{result:?}");
    }

    // Check that combining -e/--regexp and -f/--file works as expected.
    let args = parse_low_raw(["-efoo", "-fbar"]).unwrap();
    assert_eq!(
        vec![
            PatternSource::Regexp("foo".to_string()),
            PatternSource::File(PathBuf::from("bar"))
        ],
        args.patterns
    );

    let args = parse_low_raw(["-efoo", "-fbar", "-equux"]).unwrap();
    assert_eq!(
        vec![
            PatternSource::Regexp("foo".to_string()),
            PatternSource::File(PathBuf::from("bar")),
            PatternSource::Regexp("quux".to_string()),
        ],
        args.patterns
    );
}

/// -r/--replace
#[derive(Debug)]
struct Replace;

impl Flag for Replace {
    fn is_switch(&self) -> bool {
        false
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'r')
    }
    fn name_long(&self) -> &'static str {
        "replace"
    }
    fn doc_variable(&self) -> Option<&'static str> {
        Some("REPLACEMENT")
    }
    fn doc_category(&self) -> Category {
        Category::Output
    }
    fn doc_short(&self) -> &'static str {
        r"Заменить совпадения заданным текстом."
    }
    fn doc_long(&self) -> &'static str {
        r#"
Заменяет каждое совпадение заданным текстом при выводе результатов. Ни этот флаг,
ни любой другой флаг ripgrep не изменит ваши файлы.
.sp
Индексы групп захвата (например, \fB$\fP\fI5\fP) и имена (например, \fB$\fP\fIfoo\fP)
поддерживаются в строке замены. Индексы групп захвата нумеруются на основе позиции
открывающей скобки группы, где самая левая такая группа — \fB$\fP\fI1\fP. Специальная
группа \fB$\fP\fI0\fP соответствует всему совпадению.
.sp
Имя группы формируется путём взятия самой длинной строки из букв, цифр и подчёркиваний
(т.е. \fB[_0-9A-Za-z]\fP) после \fB$\fP. Например, \fB$\fP\fI1a\fP будет заменено
группой с именем \fI1a\fP, а не группой с индексом \fI1\fP. Если имя группы содержит
символы, которые не являются буквами, цифрами или подчёркиваниями, или вы хотите
немедленно следовать за группой другой строкой, имя должно быть помещено в фигурные
скобки. Например, \fB${\fP\fI1\fP\fB}\fP\fIa\fP возьмёт содержимое группы с индексом
\fI1\fP и добавит \fIa\fP в конец.
.sp
Если индекс или имя не ссылаются на допустимую группу захвата, они будут заменены
пустой строкой.
.sp
В оболочках, таких как Bash и zsh, вы должны обернуть шаблон в одинарные кавычки
вместо двойных кавычек. В противном случае индексы групп захвата будут заменены
развёрнутыми переменными оболочки, которые, скорее всего, будут пустыми.
.sp
Чтобы записать литеральный \fB$\fP, используйте \fB$$\fP.
.sp
Обратите внимание, что замена по умолчанию заменяет каждое совпадение, а не всю
строку. Чтобы заменить всю строку, вы должны сопоставить всю строку.
.sp
Этот флаг может быть использован с флагом \flag{only-matching}.
"#
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.replace = Some(convert::string(v.unwrap_value())?.into());
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_replace() {
    use bstr::BString;

    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(None, args.replace);

    let args = parse_low_raw(["--replace", "foo"]).unwrap();
    assert_eq!(Some(BString::from("foo")), args.replace);

    let args = parse_low_raw(["--replace", "-foo"]).unwrap();
    assert_eq!(Some(BString::from("-foo")), args.replace);

    let args = parse_low_raw(["-r", "foo"]).unwrap();
    assert_eq!(Some(BString::from("foo")), args.replace);

    let args = parse_low_raw(["-r", "foo", "-rbar"]).unwrap();
    assert_eq!(Some(BString::from("bar")), args.replace);

    let args = parse_low_raw(["-r", "foo", "-r", ""]).unwrap();
    assert_eq!(Some(BString::from("")), args.replace);
}

/// -z/--search-zip
#[derive(Debug)]
struct SearchZip;

impl Flag for SearchZip {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'z')
    }
    fn name_long(&self) -> &'static str {
        "search-zip"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("no-search-zip")
    }
    fn doc_category(&self) -> Category {
        Category::Input
    }
    fn doc_short(&self) -> &'static str {
        r"Искать в сжатых файлах."
    }
    fn doc_long(&self) -> &'static str {
        r"
Этот флаг предписывает ripgrep искать в сжатых файлах. В настоящее время
поддерживаются файлы gzip, bzip2, xz, LZ4, LZMA, Brotli и Zstd. Эта опция
ожидает, что бинарные файлы распаковки (такие как \fBgzip\fP) будут доступны
в вашем \fBPATH\fP. Если требуемые бинарные файлы не найдены, то ripgrep по
умолчанию не будет выдавать сообщения об ошибках. Используйте флаг \flag{debug},
чтобы увидеть больше информации.
.sp
Обратите внимание, что этот флаг не заставляет ripgrep искать форматы архивов
как деревья каталогов. Он только заставляет ripgrep обнаруживать сжатые файлы
и затем распаковывать их перед поиском их содержимого, как и любого другого файла.
.sp
Это переопределяет флаг \flag{pre}.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.search_zip = if v.unwrap_switch() {
            args.pre = None;
            true
        } else {
            false
        };
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_search_zip() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(false, args.search_zip);

    let args = parse_low_raw(["--search-zip"]).unwrap();
    assert_eq!(true, args.search_zip);

    let args = parse_low_raw(["-z"]).unwrap();
    assert_eq!(true, args.search_zip);

    let args = parse_low_raw(["-z", "--no-search-zip"]).unwrap();
    assert_eq!(false, args.search_zip);

    let args = parse_low_raw(["--pre=foo", "--no-search-zip"]).unwrap();
    assert_eq!(Some(PathBuf::from("foo")), args.pre);
    assert_eq!(false, args.search_zip);

    let args = parse_low_raw(["--pre=foo", "--search-zip"]).unwrap();
    assert_eq!(None, args.pre);
    assert_eq!(true, args.search_zip);

    let args = parse_low_raw(["--pre=foo", "-z", "--no-search-zip"]).unwrap();
    assert_eq!(None, args.pre);
    assert_eq!(false, args.search_zip);
}

/// -S/--smart-case
#[derive(Debug)]
struct SmartCase;

impl Flag for SmartCase {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'S')
    }
    fn name_long(&self) -> &'static str {
        "smart-case"
    }
    fn doc_category(&self) -> Category {
        Category::Search
    }
    fn doc_short(&self) -> &'static str {
        r"Умный поиск с учётом регистра."
    }
    fn doc_long(&self) -> &'static str {
        r"
Этот флаг предписывает ripgrep искать без учёта регистра, если шаблон полностью
в нижнем регистре. В противном случае ripgrep будет искать с учётом регистра.
.sp
Шаблон считается полностью в нижнем регистре, если выполняются оба следующих
правила:
.sp
.IP \(bu 3n
Во-первых, шаблон содержит как минимум один литеральный символ. Например,
\fBa\\w\fP содержит литерал (\fBa\fP), но просто \fB\\w\fP — нет.
.sp
.IP \(bu 3n
Во-вторых, из литералов в шаблоне ни один не считается заглавным согласно
Unicode. Например, \fBfoo\\pL\fP не имеет заглавных литералов, но \fBFoo\\pL\fP
имеет.
.PP
Это переопределяет флаги \flag{case-sensitive} и \flag{ignore-case}.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        assert!(v.unwrap_switch(), "--smart-case flag has no negation");
        args.case = CaseMode::Smart;
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_smart_case() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(CaseMode::Sensitive, args.case);

    let args = parse_low_raw(["--smart-case"]).unwrap();
    assert_eq!(CaseMode::Smart, args.case);

    let args = parse_low_raw(["-S"]).unwrap();
    assert_eq!(CaseMode::Smart, args.case);

    let args = parse_low_raw(["-S", "-s"]).unwrap();
    assert_eq!(CaseMode::Sensitive, args.case);

    let args = parse_low_raw(["-S", "-i"]).unwrap();
    assert_eq!(CaseMode::Insensitive, args.case);

    let args = parse_low_raw(["-s", "-S"]).unwrap();
    assert_eq!(CaseMode::Smart, args.case);

    let args = parse_low_raw(["-i", "-S"]).unwrap();
    assert_eq!(CaseMode::Smart, args.case);
}

/// --sort-files
#[derive(Debug)]
struct SortFiles;

impl Flag for SortFiles {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "sort-files"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("no-sort-files")
    }
    fn doc_category(&self) -> Category {
        Category::Output
    }
    fn doc_short(&self) -> &'static str {
        r"(УСТАРЕЛО) Сортировать результаты по пути к файлу."
    }
    fn doc_long(&self) -> &'static str {
        r"
УСТАРЕЛО. Используйте вместо этого \fB\-\-sort=path\fP.
.sp
Этот флаг предписывает ripgrep сортировать результаты поиска по пути к файлу
лексикографически в порядке возрастания. Обратите внимание, что это в настоящее
время отключает весь параллелизм и запускает поиск в одном потоке.
.sp
Этот флаг переопределяет \flag{sort} и \flag{sortr}.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.sort = if v.unwrap_switch() {
            Some(SortMode { reverse: false, kind: SortModeKind::Path })
        } else {
            None
        };
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_sort_files() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(None, args.sort);

    let args = parse_low_raw(["--sort-files"]).unwrap();
    assert_eq!(
        Some(SortMode { reverse: false, kind: SortModeKind::Path }),
        args.sort
    );

    let args = parse_low_raw(["--sort-files", "--no-sort-files"]).unwrap();
    assert_eq!(None, args.sort);

    let args = parse_low_raw(["--sort", "created", "--sort-files"]).unwrap();
    assert_eq!(
        Some(SortMode { reverse: false, kind: SortModeKind::Path }),
        args.sort
    );

    let args = parse_low_raw(["--sort-files", "--sort", "created"]).unwrap();
    assert_eq!(
        Some(SortMode { reverse: false, kind: SortModeKind::Created }),
        args.sort
    );

    let args = parse_low_raw(["--sortr", "created", "--sort-files"]).unwrap();
    assert_eq!(
        Some(SortMode { reverse: false, kind: SortModeKind::Path }),
        args.sort
    );

    let args = parse_low_raw(["--sort-files", "--sortr", "created"]).unwrap();
    assert_eq!(
        Some(SortMode { reverse: true, kind: SortModeKind::Created }),
        args.sort
    );

    let args = parse_low_raw(["--sort=path", "--no-sort-files"]).unwrap();
    assert_eq!(None, args.sort);

    let args = parse_low_raw(["--sortr=path", "--no-sort-files"]).unwrap();
    assert_eq!(None, args.sort);
}

/// --sort
#[derive(Debug)]
struct Sort;

impl Flag for Sort {
    fn is_switch(&self) -> bool {
        false
    }
    fn name_long(&self) -> &'static str {
        "sort"
    }
    fn doc_variable(&self) -> Option<&'static str> {
        Some("SORTBY")
    }
    fn doc_category(&self) -> Category {
        Category::Output
    }
    fn doc_short(&self) -> &'static str {
        r"Сортировать результаты в порядке возрастания."
    }
    fn doc_long(&self) -> &'static str {
        r"
Этот флаг включает сортировку результатов в порядке возрастания. Возможные
значения для этого флага:
.sp
.TP 12
\fBnone\fP
(По умолчанию) Не сортировать результаты. Быстрее всего. Может быть многопоточным.
.TP 12
\fBpath\fP
Сортировать по пути к файлу. Всегда однопоточный. Порядок определяется сортировкой
файлов в каждой записи каталога во время обхода. Это означает, что для файлов
\fBa/b\fP и \fBa+\fP последний будет отсортирован после первого, даже если \fB+\fP
обычно сортировался бы перед \fB/\fP.
.TP 12
\fBmodified\fP
Сортировать по времени последнего изменения файла. Всегда однопоточный.
.TP 12
\fBaccessed\fP
Сортировать по времени последнего доступа к файлу. Всегда однопоточный.
.TP 12
\fBcreated\fP
Сортировать по времени создания файла. Всегда однопоточный.
.PP
Если выбранный (вручную или по умолчанию) критерий сортировки недоступен в вашей
системе (например, время создания недоступно в файловых системах ext4), то ripgrep
попытается обнаружить это, напечатать ошибку и выйти без поиска.
.sp
Чтобы отсортировать результаты в обратном или убывающем порядке, используйте флаг
\flag{sortr}. Также этот флаг переопределяет \flag{sortr}.
.sp
Обратите внимание, что сортировка результатов всегда заставляет ripgrep отказаться
от параллелизма и работать в одном потоке.
"
    }
    fn doc_choices(&self) -> &'static [&'static str] {
        &["none", "path", "modified", "accessed", "created"]
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        let kind = match convert::str(&v.unwrap_value())? {
            "none" => {
                args.sort = None;
                return Ok(());
            }
            "path" => SortModeKind::Path,
            "modified" => SortModeKind::LastModified,
            "accessed" => SortModeKind::LastAccessed,
            "created" => SortModeKind::Created,
            unk => anyhow::bail!("choice '{unk}' is unrecognized"),
        };
        args.sort = Some(SortMode { reverse: false, kind });
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_sort() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(None, args.sort);

    let args = parse_low_raw(["--sort", "path"]).unwrap();
    assert_eq!(
        Some(SortMode { reverse: false, kind: SortModeKind::Path }),
        args.sort
    );

    let args = parse_low_raw(["--sort", "path", "--sort=created"]).unwrap();
    assert_eq!(
        Some(SortMode { reverse: false, kind: SortModeKind::Created }),
        args.sort
    );

    let args = parse_low_raw(["--sort=none"]).unwrap();
    assert_eq!(None, args.sort);

    let args = parse_low_raw(["--sort", "path", "--sort=none"]).unwrap();
    assert_eq!(None, args.sort);
}

/// --sortr
#[derive(Debug)]
struct Sortr;

impl Flag for Sortr {
    fn is_switch(&self) -> bool {
        false
    }
    fn name_long(&self) -> &'static str {
        "sortr"
    }
    fn doc_variable(&self) -> Option<&'static str> {
        Some("SORTBY")
    }
    fn doc_category(&self) -> Category {
        Category::Output
    }
    fn doc_short(&self) -> &'static str {
        r"Сортировать результаты в порядке убывания."
    }
    fn doc_long(&self) -> &'static str {
        r"
Этот флаг включает сортировку результатов в порядке убывания. Возможные значения
для этого флага:
.sp
.TP 12
\fBnone\fP
(По умолчанию) Не сортировать результаты. Быстрее всего. Может быть многопоточным.
.TP 12
\fBpath\fP
Сортировать по пути к файлу. Всегда однопоточный. Порядок определяется сортировкой
файлов в каждой записи каталога во время обхода. Это означает, что для файлов
\fBa/b\fP и \fBa+\fP последний будет отсортирован перед первым, даже если \fB+\fP
обычно сортировался бы после \fB/\fP при обратной лексикографической сортировке.
.TP 12
\fBmodified\fP
Сортировать по времени последнего изменения файла. Всегда однопоточный.
.TP 12
\fBaccessed\fP
Сортировать по времени последнего доступа к файлу. Всегда однопоточный.
.TP 12
\fBcreated\fP
Сортировать по времени создания файла. Всегда однопоточный.
.PP
Если выбранный (вручную или по умолчанию) критерий сортировки недоступен в вашей
системе (например, время создания недоступно в файловых системах ext4), то ripgrep
попытается обнаружить это, напечатать ошибку и выйти без поиска.
.sp
Чтобы отсортировать результаты в порядке возрастания, используйте флаг \flag{sort}.
Также этот флаг переопределяет \flag{sort}.
.sp
Обратите внимание, что сортировка результатов всегда заставляет ripgrep отказаться
от параллелизма и работать в одном потоке.
"
    }
    fn doc_choices(&self) -> &'static [&'static str] {
        &["none", "path", "modified", "accessed", "created"]
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        let kind = match convert::str(&v.unwrap_value())? {
            "none" => {
                args.sort = None;
                return Ok(());
            }
            "path" => SortModeKind::Path,
            "modified" => SortModeKind::LastModified,
            "accessed" => SortModeKind::LastAccessed,
            "created" => SortModeKind::Created,
            unk => anyhow::bail!("choice '{unk}' is unrecognized"),
        };
        args.sort = Some(SortMode { reverse: true, kind });
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_sortr() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(None, args.sort);

    let args = parse_low_raw(["--sortr", "path"]).unwrap();
    assert_eq!(
        Some(SortMode { reverse: true, kind: SortModeKind::Path }),
        args.sort
    );

    let args = parse_low_raw(["--sortr", "path", "--sortr=created"]).unwrap();
    assert_eq!(
        Some(SortMode { reverse: true, kind: SortModeKind::Created }),
        args.sort
    );

    let args = parse_low_raw(["--sortr=none"]).unwrap();
    assert_eq!(None, args.sort);

    let args = parse_low_raw(["--sortr", "path", "--sortr=none"]).unwrap();
    assert_eq!(None, args.sort);

    let args = parse_low_raw(["--sort=path", "--sortr=path"]).unwrap();
    assert_eq!(
        Some(SortMode { reverse: true, kind: SortModeKind::Path }),
        args.sort
    );

    let args = parse_low_raw(["--sortr=path", "--sort=path"]).unwrap();
    assert_eq!(
        Some(SortMode { reverse: false, kind: SortModeKind::Path }),
        args.sort
    );
}

/// --stats
#[derive(Debug)]
struct Stats;

impl Flag for Stats {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "stats"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("no-stats")
    }
    fn doc_category(&self) -> Category {
        Category::Logging
    }
    fn doc_short(&self) -> &'static str {
        r"Вывести статистику о поиске."
    }
    fn doc_long(&self) -> &'static str {
        r"
Когда включено, ripgrep напечатает сводную статистику о поиске. Когда этот флаг
присутствует, ripgrep напечатает как минимум следующую статистику в stdout в
конце поиска: количество совпадающих строк, количество файлов с совпадениями,
количество искомых файлов и время, затраченное на завершение всего поиска.
.sp
Этот набор сводной статистики может расширяться со временем.
.sp
Этот флаг всегда и неявно включается, когда используется \flag{json}.
.sp
Обратите внимание, что этот флаг не имеет эффекта, если передан \flag{files},
\flag{files-with-matches} или \flag{files-without-match}.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.stats = v.unwrap_switch();
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_stats() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(false, args.stats);

    let args = parse_low_raw(["--stats"]).unwrap();
    assert_eq!(true, args.stats);

    let args = parse_low_raw(["--stats", "--no-stats"]).unwrap();
    assert_eq!(false, args.stats);
}

/// --stop-on-nonmatch
#[derive(Debug)]
struct StopOnNonmatch;

impl Flag for StopOnNonmatch {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "stop-on-nonmatch"
    }
    fn doc_category(&self) -> Category {
        Category::Search
    }
    fn doc_short(&self) -> &'static str {
        r"Остановить поиск после несовпадения."
    }
    fn doc_long(&self) -> &'static str {
        r"
Включение этой опции заставит ripgrep прекратить чтение файла, как только он
встретит несовпадающую строку после того, как он встретил совпадающую строку.
Это полезно, если ожидается, что все совпадения в данном файле будут на
последовательных строках, например, из-за того, что строки отсортированы.
.sp
Это переопределяет флаг \flag{multiline}.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        assert!(v.unwrap_switch(), "--stop-on-nonmatch has no negation");
        args.stop_on_nonmatch = true;
        args.multiline = false;
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_stop_on_nonmatch() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(false, args.stop_on_nonmatch);

    let args = parse_low_raw(["--stop-on-nonmatch"]).unwrap();
    assert_eq!(true, args.stop_on_nonmatch);

    let args = parse_low_raw(["--stop-on-nonmatch", "-U"]).unwrap();
    assert_eq!(true, args.multiline);
    assert_eq!(false, args.stop_on_nonmatch);

    let args = parse_low_raw(["-U", "--stop-on-nonmatch"]).unwrap();
    assert_eq!(false, args.multiline);
    assert_eq!(true, args.stop_on_nonmatch);

    let args =
        parse_low_raw(["--stop-on-nonmatch", "--no-multiline"]).unwrap();
    assert_eq!(false, args.multiline);
    assert_eq!(true, args.stop_on_nonmatch);
}

/// -a/--text
#[derive(Debug)]
struct Text;

impl Flag for Text {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'a')
    }
    fn name_long(&self) -> &'static str {
        "text"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("no-text")
    }
    fn doc_category(&self) -> Category {
        Category::Search
    }
    fn doc_short(&self) -> &'static str {
        r"Искать бинарные файлы, как если бы они были текстом."
    }
    fn doc_long(&self) -> &'static str {
        r"
Этот флаг предписывает ripgrep искать бинарные файлы, как если бы они были
текстом. Когда этот флаг присутствует, обнаружение бинарных файлов ripgrep
отключается. Это означает, что при поиске бинарного файла его содержимое может
быть напечатано, если есть совпадение. Это может привести к печати escape-кодов,
которые изменяют поведение вашего терминала.
.sp
Когда обнаружение бинарных файлов включено, оно несовершенно. В общем, оно
использует простую эвристику. Если во время поиска замечен байт \fBNUL\fP, то
файл считается бинарным, и поиск прекращается (если этот флаг не присутствует).
Альтернативно, если используется флаг \flag{binary}, то ripgrep выйдет только
когда увидит байт \fBNUL\fP после того, как увидит совпадение (или ищет весь
файл).
.sp
Этот флаг переопределяет флаг \flag{binary}.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.binary = if v.unwrap_switch() {
            BinaryMode::AsText
        } else {
            BinaryMode::Auto
        };
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_text() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(BinaryMode::Auto, args.binary);

    let args = parse_low_raw(["--text"]).unwrap();
    assert_eq!(BinaryMode::AsText, args.binary);

    let args = parse_low_raw(["-a"]).unwrap();
    assert_eq!(BinaryMode::AsText, args.binary);

    let args = parse_low_raw(["-a", "--no-text"]).unwrap();
    assert_eq!(BinaryMode::Auto, args.binary);

    let args = parse_low_raw(["-a", "--binary"]).unwrap();
    assert_eq!(BinaryMode::SearchAndSuppress, args.binary);

    let args = parse_low_raw(["--binary", "-a"]).unwrap();
    assert_eq!(BinaryMode::AsText, args.binary);

    let args = parse_low_raw(["-a", "--no-binary"]).unwrap();
    assert_eq!(BinaryMode::Auto, args.binary);

    let args = parse_low_raw(["--binary", "--no-text"]).unwrap();
    assert_eq!(BinaryMode::Auto, args.binary);
}

/// -j/--threads
#[derive(Debug)]
struct Threads;

impl Flag for Threads {
    fn is_switch(&self) -> bool {
        false
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'j')
    }
    fn name_long(&self) -> &'static str {
        "threads"
    }
    fn doc_variable(&self) -> Option<&'static str> {
        Some("NUM")
    }
    fn doc_category(&self) -> Category {
        Category::Search
    }
    fn doc_short(&self) -> &'static str {
        r"Установить приблизительное количество потоков для использования."
    }
    fn doc_long(&self) -> &'static str {
        r"
Этот флаг устанавливает приблизительное количество потоков для использования.
Значение \fB0\fP (которое является значением по умолчанию) заставляет ripgrep
выбирать количество потоков с помощью эвристик.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        let threads = convert::usize(&v.unwrap_value())?;
        args.threads = if threads == 0 { None } else { Some(threads) };
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_threads() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(None, args.threads);

    let args = parse_low_raw(["--threads", "5"]).unwrap();
    assert_eq!(Some(5), args.threads);

    let args = parse_low_raw(["-j", "5"]).unwrap();
    assert_eq!(Some(5), args.threads);

    let args = parse_low_raw(["-j5"]).unwrap();
    assert_eq!(Some(5), args.threads);

    let args = parse_low_raw(["-j5", "-j10"]).unwrap();
    assert_eq!(Some(10), args.threads);

    let args = parse_low_raw(["-j5", "-j0"]).unwrap();
    assert_eq!(None, args.threads);
}

/// --trace
#[derive(Debug)]
struct Trace;

impl Flag for Trace {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "trace"
    }
    fn doc_category(&self) -> Category {
        Category::Logging
    }
    fn doc_short(&self) -> &'static str {
        r"Показать трассировочные сообщения."
    }
    fn doc_long(&self) -> &'static str {
        r"
Показать трассировочные сообщения. Это показывает ещё больше деталей, чем флаг
\flag{debug}. Обычно этот флаг следует использовать только если \flag{debug} не
выдаёт нужную вам информацию.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        assert!(v.unwrap_switch(), "--trace can only be enabled");
        args.logging = Some(LoggingMode::Trace);
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_trace() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(None, args.logging);

    let args = parse_low_raw(["--trace"]).unwrap();
    assert_eq!(Some(LoggingMode::Trace), args.logging);

    let args = parse_low_raw(["--debug", "--trace"]).unwrap();
    assert_eq!(Some(LoggingMode::Trace), args.logging);
}

/// --trim
#[derive(Debug)]
struct Trim;

impl Flag for Trim {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "trim"
    }
    fn name_negated(&self) -> Option<&'static str> {
        Some("no-trim")
    }
    fn doc_category(&self) -> Category {
        Category::Output
    }
    fn doc_short(&self) -> &'static str {
        r"Обрезать начальные пробелы из совпадений."
    }
    fn doc_long(&self) -> &'static str {
        r"
Когда установлен, все ASCII-пробельные символы в начале каждой печатаемой
строки будут удалены.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.trim = v.unwrap_switch();
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_trim() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(false, args.trim);

    let args = parse_low_raw(["--trim"]).unwrap();
    assert_eq!(true, args.trim);

    let args = parse_low_raw(["--trim", "--no-trim"]).unwrap();
    assert_eq!(false, args.trim);
}

/// -t/--type
#[derive(Debug)]
struct Type;

impl Flag for Type {
    fn is_switch(&self) -> bool {
        false
    }
    fn name_short(&self) -> Option<u8> {
        Some(b't')
    }
    fn name_long(&self) -> &'static str {
        "type"
    }
    fn doc_variable(&self) -> Option<&'static str> {
        Some("TYPE")
    }
    fn doc_category(&self) -> Category {
        Category::Filter
    }
    fn doc_short(&self) -> &'static str {
        r"Искать только файлы, соответствующие TYPE."
    }
    fn doc_long(&self) -> &'static str {
        r#"
Этот флаг ограничивает ripgrep поиском файлов, соответствующих \fITYPE\fP.
Может быть предоставлено несколько флагов \flag{type}.
.sp
Этот флаг поддерживает специальное значение \fBall\fP, которое будет вести
себя так, как если бы флаг \flag{type} был предоставлен для каждого типа
файлов, поддерживаемого ripgrep (включая любые пользовательские типы файлов).
Конечный результат — \fB\-\-type=all\fP заставляет ripgrep искать в режиме
«белого списка», где он будет искать только файлы, которые он распознаёт через
свои определения типов.
.sp
Обратите внимание, что этот флаг имеет меньший приоритет, чем флаг \flag{glob}
и любые правила, найденные в файлах игнорирования.
.sp
Чтобы увидеть список доступных типов файлов, используйте флаг \flag{type-list}.
"#
    }
    fn completion_type(&self) -> CompletionType {
        CompletionType::Filetype
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.type_changes.push(TypeChange::Select {
            name: convert::string(v.unwrap_value())?,
        });
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_type() {
    let select = |name: &str| TypeChange::Select { name: name.to_string() };

    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(Vec::<TypeChange>::new(), args.type_changes);

    let args = parse_low_raw(["--type", "rust"]).unwrap();
    assert_eq!(vec![select("rust")], args.type_changes);

    let args = parse_low_raw(["-t", "rust"]).unwrap();
    assert_eq!(vec![select("rust")], args.type_changes);

    let args = parse_low_raw(["-trust"]).unwrap();
    assert_eq!(vec![select("rust")], args.type_changes);

    let args = parse_low_raw(["-trust", "-tpython"]).unwrap();
    assert_eq!(vec![select("rust"), select("python")], args.type_changes);

    let args = parse_low_raw(["-tabcdefxyz"]).unwrap();
    assert_eq!(vec![select("abcdefxyz")], args.type_changes);
}

/// --type-add
#[derive(Debug)]
struct TypeAdd;

impl Flag for TypeAdd {
    fn is_switch(&self) -> bool {
        false
    }
    fn name_long(&self) -> &'static str {
        "type-add"
    }
    fn doc_variable(&self) -> Option<&'static str> {
        Some("TYPESPEC")
    }
    fn doc_category(&self) -> Category {
        Category::Filter
    }
    fn doc_short(&self) -> &'static str {
        r"Добавить новый glob для типа файлов."
    }
    fn doc_long(&self) -> &'static str {
        r"
Этот флаг добавляет новый glob для определённого типа файлов. Только один glob
может быть добавлен за раз. Может быть предоставлено несколько флагов
\flag{type-add}. Если не используется \flag{type-clear}, glob'ы добавляются к
любым существующим glob'ам, определённым внутри ripgrep.
.sp
Обратите внимание, что это должно быть передано каждому вызову ripgrep. Настройки
типов не сохраняются. См. \fBCONFIGURATION FILES\fP для обходного пути.
.sp
Пример:
.sp
.EX
    rg \-\-type\-add 'foo:*.foo' -tfoo \fIPATTERN\fP
.EE
.sp
Этот флаг также может быть использован для включения правил из других типов с
помощью специальной директивы include. Директива include позволяет указать один
или несколько других имён типов (разделённых запятой), которые были определены,
и их правила будут автоматически импортированы в указанный тип. Например, чтобы
создать тип с именем src, который соответствует файлам C++, Python и Markdown,
можно использовать:
.sp
.EX
    \-\-type\-add 'src:include:cpp,py,md'
.EE
.sp
Дополнительные правила glob всё ещё могут быть добавлены к типу src путём
повторного использования этого флага:
.sp
.EX
    \-\-type\-add 'src:include:cpp,py,md' \-\-type\-add 'src:*.foo'
.EE
.sp
Обратите внимание, что имена типов должны состоять только из Unicode-букв или
цифр. Символы пунктуации не допускаются.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.type_changes
            .push(TypeChange::Add { def: convert::string(v.unwrap_value())? });
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_type_add() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(Vec::<TypeChange>::new(), args.type_changes);

    let args = parse_low_raw(["--type-add", "foo"]).unwrap();
    assert_eq!(
        vec![TypeChange::Add { def: "foo".to_string() }],
        args.type_changes
    );

    let args = parse_low_raw(["--type-add", "foo", "--type-add=bar"]).unwrap();
    assert_eq!(
        vec![
            TypeChange::Add { def: "foo".to_string() },
            TypeChange::Add { def: "bar".to_string() }
        ],
        args.type_changes
    );
}

/// --type-clear
#[derive(Debug)]
struct TypeClear;

impl Flag for TypeClear {
    fn is_switch(&self) -> bool {
        false
    }
    fn name_long(&self) -> &'static str {
        "type-clear"
    }
    fn doc_variable(&self) -> Option<&'static str> {
        Some("TYPE")
    }
    fn doc_category(&self) -> Category {
        Category::Filter
    }
    fn doc_short(&self) -> &'static str {
        r"Очистить glob'ы для типа файлов."
    }
    fn doc_long(&self) -> &'static str {
        r"
Очистить glob'ы файлов, ранее определённые для \fITYPE\fP. Это очищает любые
ранее определённые glob'ы для \fITYPE\fP, но glob'ы могут быть добавлены после
этого флага.
.sp
Обратите внимание, что это должно быть передано каждому вызову ripgrep. Настройки
типов н�� сохраняются. См. \fBCONFIGURATION FILES\fP для обходного пути.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.type_changes.push(TypeChange::Clear {
            name: convert::string(v.unwrap_value())?,
        });
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_type_clear() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(Vec::<TypeChange>::new(), args.type_changes);

    let args = parse_low_raw(["--type-clear", "foo"]).unwrap();
    assert_eq!(
        vec![TypeChange::Clear { name: "foo".to_string() }],
        args.type_changes
    );

    let args =
        parse_low_raw(["--type-clear", "foo", "--type-clear=bar"]).unwrap();
    assert_eq!(
        vec![
            TypeChange::Clear { name: "foo".to_string() },
            TypeChange::Clear { name: "bar".to_string() }
        ],
        args.type_changes
    );
}

/// --type-not
#[derive(Debug)]
struct TypeNot;

impl Flag for TypeNot {
    fn is_switch(&self) -> bool {
        false
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'T')
    }
    fn name_long(&self) -> &'static str {
        "type-not"
    }
    fn doc_variable(&self) -> Option<&'static str> {
        Some("TYPE")
    }
    fn doc_category(&self) -> Category {
        Category::Filter
    }
    fn doc_short(&self) -> &'static str {
        r"Не искать файлы, соответствующие TYPE."
    }
    fn doc_long(&self) -> &'static str {
        r#"
Не искать файлы, соответствующие \fITYPE\fP. Может быть предоставлено несколько
флагов \flag{type-not}. Используйте флаг \flag{type-list} для просмотра всех
доступных типов.
.sp
Этот флаг поддерживает специальное значение \fBall\fP, которое будет вести себя
так, как если бы флаг \flag{type-not} был предоставлен для каждого типа файлов,
поддерживаемого ripgrep (включая любые пользовательские типы файлов). Конечный
результат — \fB\-\-type\-not=all\fP заставляет ripgrep искать в режиме «чёрного
списка», где он будет искать только файлы, которые не распознаны его определениями
типов.
.sp
Чтобы увидеть список доступных типов файлов, используйте флаг \flag{type-list}.
"#
    }
    fn completion_type(&self) -> CompletionType {
        CompletionType::Filetype
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        args.type_changes.push(TypeChange::Negate {
            name: convert::string(v.unwrap_value())?,
        });
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_type_not() {
    let select = |name: &str| TypeChange::Select { name: name.to_string() };
    let negate = |name: &str| TypeChange::Negate { name: name.to_string() };

    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(Vec::<TypeChange>::new(), args.type_changes);

    let args = parse_low_raw(["--type-not", "rust"]).unwrap();
    assert_eq!(vec![negate("rust")], args.type_changes);

    let args = parse_low_raw(["-T", "rust"]).unwrap();
    assert_eq!(vec![negate("rust")], args.type_changes);

    let args = parse_low_raw(["-Trust"]).unwrap();
    assert_eq!(vec![negate("rust")], args.type_changes);

    let args = parse_low_raw(["-Trust", "-Tpython"]).unwrap();
    assert_eq!(vec![negate("rust"), negate("python")], args.type_changes);

    let args = parse_low_raw(["-Tabcdefxyz"]).unwrap();
    assert_eq!(vec![negate("abcdefxyz")], args.type_changes);

    let args = parse_low_raw(["-Trust", "-ttoml", "-Tjson"]).unwrap();
    assert_eq!(
        vec![negate("rust"), select("toml"), negate("json")],
        args.type_changes
    );
}

/// --type-list
#[derive(Debug)]
struct TypeList;

impl Flag for TypeList {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "type-list"
    }
    fn doc_category(&self) -> Category {
        Category::OtherBehaviors
    }
    fn doc_short(&self) -> &'static str {
        r"Показать все поддерживаемые типы файлов."
    }
    fn doc_long(&self) -> &'static str {
        r"
Показать все поддерживаемые типы файлов и их соответствующие glob'ы. Это учитывает
любые предоставленные флаги \flag{type-add} и \flag{type-clear}. Каждый тип
печатается на собственной строке, за которым следует \fB:\fP, а затем
разделённый запятыми список glob'ов для этого типа на той же строке.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        assert!(v.unwrap_switch(), "--type-list has no negation");
        args.mode.update(Mode::Types);
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_type_list() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(Mode::Search(SearchMode::Standard), args.mode);

    let args = parse_low_raw(["--type-list"]).unwrap();
    assert_eq!(Mode::Types, args.mode);
}

/// -u/--unrestricted
#[derive(Debug)]
struct Unrestricted;

impl Flag for Unrestricted {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'u')
    }
    fn name_long(&self) -> &'static str {
        "unrestricted"
    }
    fn doc_category(&self) -> Category {
        Category::Filter
    }
    fn doc_short(&self) -> &'static str {
        r#"Уменьшить уровень «умной» фильтрации."#
    }
    fn doc_long(&self) -> &'static str {
        r#"
Этот флаг уменьшает уровень «умной» фильтрации. Повторное использование (до 3 раз)
уменьшает фильтрацию ещё больше. При повторении три раза ripgrep будет искать
каждый файл в дереве каталогов.
.sp
Один флаг \flag{unrestricted} эквивалентен \flag{no-ignore}. Два флага
\flag{unrestricted} эквивалентны \flag{no-ignore} \flag{hidden}. Три флага
\flag{unrestricted} эквивалентны \flag{no-ignore} \flag{hidden} \flag{binary}.
.sp
Единственная фильтрация, которую ripgrep всё ещё выполняет, когда предоставлено
\fB-uuu\fP, — это пропуск символических ссылок и избежание печати совпадений из
бинарных файлов. Символические ссылки могут быть пройдены с помощью флага
\flag{follow}, а бинарные файлы могут быть обработаны как текстовые файлы с
помощью флага \flag{text}.
"#
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        assert!(v.unwrap_switch(), "--unrestricted has no negation");
        args.unrestricted = args.unrestricted.saturating_add(1);
        anyhow::ensure!(
            args.unrestricted <= 3,
            "flag can only be repeated up to 3 times"
        );
        if args.unrestricted == 1 {
            NoIgnore.update(FlagValue::Switch(true), args)?;
        } else if args.unrestricted == 2 {
            Hidden.update(FlagValue::Switch(true), args)?;
        } else {
            assert_eq!(args.unrestricted, 3);
            Binary.update(FlagValue::Switch(true), args)?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_unrestricted() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(false, args.no_ignore_vcs);
    assert_eq!(false, args.hidden);
    assert_eq!(BinaryMode::Auto, args.binary);

    let args = parse_low_raw(["--unrestricted"]).unwrap();
    assert_eq!(true, args.no_ignore_vcs);
    assert_eq!(false, args.hidden);
    assert_eq!(BinaryMode::Auto, args.binary);

    let args = parse_low_raw(["--unrestricted", "-u"]).unwrap();
    assert_eq!(true, args.no_ignore_vcs);
    assert_eq!(true, args.hidden);
    assert_eq!(BinaryMode::Auto, args.binary);

    let args = parse_low_raw(["-uuu"]).unwrap();
    assert_eq!(true, args.no_ignore_vcs);
    assert_eq!(true, args.hidden);
    assert_eq!(BinaryMode::SearchAndSuppress, args.binary);

    let result = parse_low_raw(["-uuuu"]);
    assert!(result.is_err(), "{result:?}");
}

/// --version
#[derive(Debug)]
struct Version;

impl Flag for Version {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'V')
    }
    fn name_long(&self) -> &'static str {
        "version"
    }
    fn doc_category(&self) -> Category {
        Category::OtherBehaviors
    }
    fn doc_short(&self) -> &'static str {
        r"Вывести версию ripgrep."
    }
    fn doc_long(&self) -> &'static str {
        r"
Этот флаг печатает версию ripgrep. Это также может печатать другую релевантную
информацию, такую как наличие специфичных для цели оптимизаций и ревизию \fBgit\fP,
из которой была скомпилирована эта сборка ripgrep.
"
    }

    fn update(&self, v: FlagValue, _: &mut LowArgs) -> anyhow::Result<()> {
        assert!(v.unwrap_switch(), "--version has no negation");
        // Since this flag has different semantics for -V and --version and the
        // Flag trait doesn't support encoding this sort of thing, we handle it
        // as a special case in the parser.
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_version() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(None, args.special);

    let args = parse_low_raw(["-V"]).unwrap();
    assert_eq!(Some(SpecialMode::VersionShort), args.special);

    let args = parse_low_raw(["--version"]).unwrap();
    assert_eq!(Some(SpecialMode::VersionLong), args.special);

    let args = parse_low_raw(["-V", "--version"]).unwrap();
    assert_eq!(Some(SpecialMode::VersionLong), args.special);

    let args = parse_low_raw(["--version", "-V"]).unwrap();
    assert_eq!(Some(SpecialMode::VersionShort), args.special);
}

/// --vimgrep
#[derive(Debug)]
struct Vimgrep;

impl Flag for Vimgrep {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_long(&self) -> &'static str {
        "vimgrep"
    }
    fn doc_category(&self) -> Category {
        Category::Output
    }
    fn doc_short(&self) -> &'static str {
        r"Вывести результаты в формате, совместимом с vim."
    }
    fn doc_long(&self) -> &'static str {
        r"
Этот флаг предписывает ripgrep выводить результаты с каждым совпадением на
отдельной строке, включая номера строк и номера столбцов.
.sp
С этой опцией строка с более чем одним совпадением будет напечатана целиком
более одного раза. По этой причине общий объём вывода в результате этого флага
может быть квадратичным по размеру ввода. Например, если шаблон соответствует
каждому байту во входном файле, то каждая строка будет повторяться для каждого
сопоставленного байта. По этой причине пользователи должны использовать этот
флаг только когда нет другого выбора. Интеграции с редакторами следует
предпочесть другой способ чтения результатов из ripgrep, например, через флаг
\flag{json}. Одной альтернативой для избежания чрезмерного использования памяти
является принудительный перевод ripgrep в однопоточный режим с помощью флага
\flag{threads}. Обратите внимание, что это не повлияет на общий размер вывода,
только на объём кучи, которую ripgrep будет использовать.
"
    }
    fn doc_choices(&self) -> &'static [&'static str] {
        &[]
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        assert!(v.unwrap_switch(), "--vimgrep has no negation");
        args.vimgrep = true;
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_vimgrep() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(false, args.vimgrep);

    let args = parse_low_raw(["--vimgrep"]).unwrap();
    assert_eq!(true, args.vimgrep);
}

/// --with-filename
#[derive(Debug)]
struct WithFilename;

impl Flag for WithFilename {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'H')
    }
    fn name_long(&self) -> &'static str {
        "with-filename"
    }
    fn doc_category(&self) -> Category {
        Category::Output
    }
    fn doc_short(&self) -> &'static str {
        r"Вывести путь к файлу с каждой совпадающей строкой."
    }
    fn doc_long(&self) -> &'static str {
        r"
Этот флаг предписывает ripgrep выводить путь к файлу для каждой совпадающей
строки. Это по умолчанию, когда ищется более одного файла. Если \flag{heading}
включён (по умолчанию при выводе в tty), путь к файлу будет показан над
группами совпадений из каждого файла; в противном случае имя файла будет
показано как префикс ��ля каждой совпадающей строки.
.sp
Этот флаг переопределяет \flag{no-filename}.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        assert!(v.unwrap_switch(), "--with-filename has no defined negation");
        args.with_filename = Some(true);
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_with_filename() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(None, args.with_filename);

    let args = parse_low_raw(["--with-filename"]).unwrap();
    assert_eq!(Some(true), args.with_filename);

    let args = parse_low_raw(["-H"]).unwrap();
    assert_eq!(Some(true), args.with_filename);
}

/// --no-filename
#[derive(Debug)]
struct WithFilenameNo;

impl Flag for WithFilenameNo {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'I')
    }
    fn name_long(&self) -> &'static str {
        "no-filename"
    }
    fn doc_category(&self) -> Category {
        Category::Output
    }
    fn doc_short(&self) -> &'static str {
        r"Никогда не выводить путь с каждой совпадающей строкой."
    }
    fn doc_long(&self) -> &'static str {
        r"
Этот флаг предписывает ripgrep никогда не выводить путь к файлу с каждой
совпадающей строкой. Это по умолчанию, когда ripgrep явно инструктирован искать
один файл или stdin.
.sp
Этот флаг переопределяет \flag{with-filename}.
"
    }
    fn doc_choices(&self) -> &'static [&'static str] {
        &[]
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        assert!(v.unwrap_switch(), "--no-filename has no defined negation");
        args.with_filename = Some(false);
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_with_filename_no() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(None, args.with_filename);

    let args = parse_low_raw(["--no-filename"]).unwrap();
    assert_eq!(Some(false), args.with_filename);

    let args = parse_low_raw(["-I"]).unwrap();
    assert_eq!(Some(false), args.with_filename);

    let args = parse_low_raw(["-I", "-H"]).unwrap();
    assert_eq!(Some(true), args.with_filename);

    let args = parse_low_raw(["-H", "-I"]).unwrap();
    assert_eq!(Some(false), args.with_filename);
}

/// -w/--word-regexp
#[derive(Debug)]
struct WordRegexp;

impl Flag for WordRegexp {
    fn is_switch(&self) -> bool {
        true
    }
    fn name_short(&self) -> Option<u8> {
        Some(b'w')
    }
    fn name_long(&self) -> &'static str {
        "word-regexp"
    }
    fn doc_category(&self) -> Category {
        Category::Search
    }
    fn doc_short(&self) -> &'static str {
        r"Показать совпадения, окружённые границами слов."
    }
    fn doc_long(&self) -> &'static str {
        r"
Когда включено, ripgrep будет показывать только совпадения, окружённые границами
слов. Это эквивалентно окружению каждого шаблона символами \fB\\b{start-half}\fP
и \fB\\b{end-half}\fP.
.sp
Это переопределяет флаг \flag{line-regexp}.
"
    }

    fn update(&self, v: FlagValue, args: &mut LowArgs) -> anyhow::Result<()> {
        assert!(v.unwrap_switch(), "--word-regexp has no negation");
        args.boundary = Some(BoundaryMode::Word);
        Ok(())
    }
}

#[cfg(test)]
#[test]
fn test_word_regexp() {
    let args = parse_low_raw(None::<&str>).unwrap();
    assert_eq!(None, args.boundary);

    let args = parse_low_raw(["--word-regexp"]).unwrap();
    assert_eq!(Some(BoundaryMode::Word), args.boundary);

    let args = parse_low_raw(["-w"]).unwrap();
    assert_eq!(Some(BoundaryMode::Word), args.boundary);

    let args = parse_low_raw(["-x", "-w"]).unwrap();
    assert_eq!(Some(BoundaryMode::Word), args.boundary);

    let args = parse_low_raw(["-w", "-x"]).unwrap();
    assert_eq!(Some(BoundaryMode::Line), args.boundary);
}

mod convert {
    use std::ffi::{OsStr, OsString};

    use anyhow::Context;

    pub(super) fn str(v: &OsStr) -> anyhow::Result<&str> {
        let Some(s) = v.to_str() else {
            anyhow::bail!("value is not valid UTF-8")
        };
        Ok(s)
    }

    pub(super) fn string(v: OsString) -> anyhow::Result<String> {
        let Ok(s) = v.into_string() else {
            anyhow::bail!("value is not valid UTF-8")
        };
        Ok(s)
    }

    pub(super) fn usize(v: &OsStr) -> anyhow::Result<usize> {
        str(v)?.parse().context("value is not a valid number")
    }

    pub(super) fn u64(v: &OsStr) -> anyhow::Result<u64> {
        str(v)?.parse().context("value is not a valid number")
    }

    pub(super) fn human_readable_u64(v: &OsStr) -> anyhow::Result<u64> {
        grep::cli::parse_human_readable_size(str(v)?).context("invalid size")
    }

    pub(super) fn human_readable_usize(v: &OsStr) -> anyhow::Result<usize> {
        let size = human_readable_u64(v)?;
        let Ok(size) = usize::try_from(size) else {
            anyhow::bail!("size is too big")
        };
        Ok(size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn available_shorts() {
        let mut total = vec![false; 128];
        for byte in 0..=0x7F {
            match byte {
                b'.' | b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' => {
                    total[usize::from(byte)] = true
                }
                _ => continue,
            }
        }

        let mut taken = vec![false; 128];
        for flag in FLAGS.iter() {
            let Some(short) = flag.name_short() else { continue };
            taken[usize::from(short)] = true;
        }

        for byte in 0..=0x7F {
            if total[usize::from(byte)] && !taken[usize::from(byte)] {
                eprintln!("{}", char::from(byte));
            }
        }
    }

    #[test]
    fn shorts_all_ascii_alphanumeric() {
        for flag in FLAGS.iter() {
            let Some(byte) = flag.name_short() else { continue };
            let long = flag.name_long();
            assert!(
                byte.is_ascii_alphanumeric() || byte == b'.',
                "\\x{byte:0X} is not a valid short flag for {long}",
            )
        }
    }

    #[test]
    fn longs_all_ascii_alphanumeric() {
        for flag in FLAGS.iter() {
            let long = flag.name_long();
            let count = long.chars().count();
            assert!(count >= 2, "flag '{long}' is less than 2 characters");
            assert!(
                long.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'),
                "flag '{long}' does not match ^[-0-9A-Za-z]+$",
            );
            for alias in flag.aliases() {
                let count = alias.chars().count();
                assert!(
                    count >= 2,
                    "flag '{long}' has alias '{alias}' that is \
                     less than 2 characters",
                );
                assert!(
                    alias
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-'),
                    "flag '{long}' has alias '{alias}' that does not \
                     match ^[-0-9A-Za-z]+$",
                );
            }
            let Some(negated) = flag.name_negated() else { continue };
            let count = negated.chars().count();
            assert!(
                count >= 2,
                "flag '{long}' has negation '{negated}' that is \
                 less than 2 characters",
            );
            assert!(
                negated.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'),
                "flag '{long}' has negation '{negated}' that \
                 does not match ^[-0-9A-Za-z]+$",
            );
        }
    }

    #[test]
    fn shorts_no_duplicates() {
        let mut taken = vec![false; 128];
        for flag in FLAGS.iter() {
            let Some(short) = flag.name_short() else { continue };
            let long = flag.name_long();
            assert!(
                !taken[usize::from(short)],
                "flag {long} has duplicate short flag {}",
                char::from(short)
            );
            taken[usize::from(short)] = true;
        }
    }

    #[test]
    fn longs_no_duplicates() {
        use std::collections::BTreeSet;

        let mut taken = BTreeSet::new();
        for flag in FLAGS.iter() {
            let long = flag.name_long();
            assert!(taken.insert(long), "flag {long} has a duplicate name");
            for alias in flag.aliases() {
                assert!(
                    taken.insert(alias),
                    "flag {long} has an alias {alias} that is duplicative"
                );
            }
            let Some(negated) = flag.name_negated() else { continue };
            assert!(
                taken.insert(negated),
                "negated flag {negated} has a duplicate name"
            );
        }
    }

    #[test]
    fn non_switches_have_variable_names() {
        for flag in FLAGS.iter() {
            if flag.is_switch() {
                continue;
            }
            let long = flag.name_long();
            assert!(
                flag.doc_variable().is_some(),
                "flag '{long}' should have a variable name"
            );
        }
    }

    #[test]
    fn switches_have_no_choices() {
        for flag in FLAGS.iter() {
            if !flag.is_switch() {
                continue;
            }
            let long = flag.name_long();
            let choices = flag.doc_choices();
            assert!(
                choices.is_empty(),
                "switch flag '{long}' \
                 should not have any choices but has some: {choices:?}",
            );
        }
    }

    #[test]
    fn choices_ascii_alphanumeric() {
        for flag in FLAGS.iter() {
            let long = flag.name_long();
            for choice in flag.doc_choices() {
                assert!(
                    choice.chars().all(|c| c.is_ascii_alphanumeric()
                        || c == '-'
                        || c == ':'
                        || c == '+'),
                    "choice '{choice}' for flag '{long}' does not match \
                     ^[-+:0-9A-Za-z]+$",
                )
            }
        }
    }
}
