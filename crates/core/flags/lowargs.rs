/*!
Предоставляет определение низкоуровневых аргументов из флагов CLI.
*/

use std::{
    ffi::{OsStr, OsString},
    path::PathBuf,
};

use {
    bstr::{BString, ByteVec},
    grep::printer::{HyperlinkFormat, UserColorSpec},
};

/// Коллекция «низкоуровневых» аргументов.
///
/// «Низкоуровневый» здесь предназначен для ограничения этого типа как можно
/// ближе к фактическим флагам CLI и аргументам. А именно, помимо некоторых
/// удобных типов, помогающих проверять значения флагов и dealing with overrides
/// между флагами, эти низкоуровневые аргументы не содержат никаких
/// высокоуровневых абстракций.
///
/// Другим навязанным самим собой ограничением является то, что заполнение
/// низкоуровневых аргументов не должно требовать ничего, кроме проверки того,
/// что предоставил пользователь. Например, низкоуровневые аргументы не содержат
/// `HyperlinkConfig`, поскольку для получения полной конфигурации нужно
/// обнаружить имя хоста текущей системы (что может потребовать запуска
/// бинарного файла или системного вызова).
///
/// Низкоуровневые аргументы заполняются парсером напрямую через метод `update`
/// соответствующей реализации трейта `Flag`.
#[derive(Debug, Default)]
pub(crate) struct LowArgs {
    // Essential arguments.
    pub(crate) special: Option<SpecialMode>,
    pub(crate) mode: Mode,
    pub(crate) positional: Vec<OsString>,
    pub(crate) patterns: Vec<PatternSource>,
    // Everything else, sorted lexicographically.
    pub(crate) binary: BinaryMode,
    pub(crate) boundary: Option<BoundaryMode>,
    pub(crate) buffer: BufferMode,
    pub(crate) byte_offset: bool,
    pub(crate) case: CaseMode,
    pub(crate) color: ColorChoice,
    pub(crate) colors: Vec<UserColorSpec>,
    pub(crate) column: Option<bool>,
    pub(crate) context: ContextMode,
    pub(crate) context_separator: ContextSeparator,
    pub(crate) crlf: bool,
    pub(crate) dfa_size_limit: Option<usize>,
    pub(crate) encoding: EncodingMode,
    pub(crate) engine: EngineChoice,
    pub(crate) field_context_separator: FieldContextSeparator,
    pub(crate) field_match_separator: FieldMatchSeparator,
    pub(crate) fixed_strings: bool,
    pub(crate) follow: bool,
    pub(crate) glob_case_insensitive: bool,
    pub(crate) globs: Vec<String>,
    pub(crate) heading: Option<bool>,
    pub(crate) hidden: bool,
    pub(crate) hostname_bin: Option<PathBuf>,
    pub(crate) hyperlink_format: HyperlinkFormat,
    pub(crate) iglobs: Vec<String>,
    pub(crate) ignore_file: Vec<PathBuf>,
    pub(crate) ignore_file_case_insensitive: bool,
    pub(crate) include_zero: bool,
    pub(crate) invert_match: bool,
    pub(crate) line_number: Option<bool>,
    pub(crate) logging: Option<LoggingMode>,
    pub(crate) max_columns: Option<u64>,
    pub(crate) max_columns_preview: bool,
    pub(crate) max_count: Option<u64>,
    pub(crate) max_depth: Option<usize>,
    pub(crate) max_filesize: Option<u64>,
    pub(crate) mmap: MmapMode,
    pub(crate) multiline: bool,
    pub(crate) multiline_dotall: bool,
    pub(crate) no_config: bool,
    pub(crate) no_ignore_dot: bool,
    pub(crate) no_ignore_exclude: bool,
    pub(crate) no_ignore_files: bool,
    pub(crate) no_ignore_global: bool,
    pub(crate) no_ignore_messages: bool,
    pub(crate) no_ignore_parent: bool,
    pub(crate) no_ignore_vcs: bool,
    pub(crate) no_messages: bool,
    pub(crate) no_require_git: bool,
    pub(crate) no_unicode: bool,
    pub(crate) null: bool,
    pub(crate) null_data: bool,
    pub(crate) one_file_system: bool,
    pub(crate) only_matching: bool,
    pub(crate) path_separator: Option<u8>,
    pub(crate) pre: Option<PathBuf>,
    pub(crate) pre_glob: Vec<String>,
    pub(crate) quiet: bool,
    pub(crate) regex_size_limit: Option<usize>,
    pub(crate) replace: Option<BString>,
    pub(crate) search_zip: bool,
    pub(crate) sort: Option<SortMode>,
    pub(crate) stats: bool,
    pub(crate) stop_on_nonmatch: bool,
    pub(crate) threads: Option<usize>,
    pub(crate) trim: bool,
    pub(crate) type_changes: Vec<TypeChange>,
    pub(crate) unrestricted: usize,
    pub(crate) vimgrep: bool,
    pub(crate) with_filename: Option<bool>,
}

/// «Специальный» режим, который превалирует над всем остальным.
///
/// Когда присутствует один из этих режимов, он переопределяет все остальное
/// и заставляет ripgrep коротко замыкать. В частности, мы избегаем
/// преобразования типов низкоуровневых аргументов в типы высокоуровневых
/// аргументов, которые могут завершиться ошибкой по разным причинам,
/// связанным с окружением. (Разбор низкоуровневых аргументов также может
/// завершиться ошибкой, но обычно не таким образом, с которым нельзя
/// справиться, удалив соответствующие аргументы из команды CLI.) Это в
/// целом является страховкой, чтобы гарантировать, что информация о версии
/// и помощи в основном всегда доступна.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SpecialMode {
    /// Показывает сжатую версию вывода «помощи». Вообще говоря, это показывает
    /// каждый флаг и чрезвычайно краткое описание этого флага в одной строке.
    /// Это соответствует флагу `-h`.
    HelpShort,
    /// Показывает очень подробную версию вывода «помощи». Документация для
    /// некоторых флагов будет длиной в абзацы. Это соответствует флагу `--help`.
    HelpLong,
    /// Показывает сжатую информацию о версии. Например, `ripgrep x.y.z`.
    VersionShort,
    /// Показывает подробную информацию о версии. Включает «краткую» информацию,
    /// а также функции, включенные в сборку.
    VersionLong,
    /// Показывает информацию о версии PCRE2 или ошибку, если эта сборка ripgrep
    /// не поддерживает PCRE2.
    VersionPCRE2,
}

/// Общий режим, в котором должен работать ripgrep.
///
/// Если бы ripgrep был разработан без наследия grep, это были бы, вероятно,
/// подкоманды? Возможно, нет, поскольку они не так часто используются.
///
/// Суть помещения их в один enum заключается в том, что они все взаимно
/// исключают друг друга и переопределяют друг друга.
///
/// Обратите внимание, что -h/--help и -V/--version не включены в это, потому
/// что они всегда переопределяют все остальное, независимо от того, где они
/// появляются в командной строке. Они рассматриваются как «специальные» режимы,
/// которые коротко замыкают обычный поток ripgrep.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Mode {
    /// ripgrep будет выполнять поиск некоторого рода.
    Search(SearchMode),
    /// Показывает файлы, которые *будут* искаться, но не ищет их фактически.
    Files,
    /// Выводит все определения типов файлов, включая типы файлов по умолчанию
    /// и любые дополнительные типы файлов, добавленные в командной строке.
    Types,
    /// Генерирует различные вещи, такие как страница руководства и файлы
    /// автодополнения.
    Generate(GenerateMode),
}

impl Default for Mode {
    fn default() -> Mode {
        Mode::Search(SearchMode::Standard)
    }
}

impl Mode {
    /// Обновляет этот режим до нового режима, реализуя различные семантики
    /// переопределения. Например, режим поиска не может переопределить режим,
    /// не связанный с поиском.
    pub(crate) fn update(&mut self, new: Mode) {
        match *self {
            // Если мы в режиме поиска, то что угодно может переопределить его.
            Mode::Search(_) => *self = new,
            _ => {
                // Как только мы в режиме, не связанном с поиском, другие режимы,
                // не связанные с поиском, могут переопределить его. Но режимы
                // поиска не могут. Так, например, `--files -l` все еще будет
                // Mode::Files.
                if !matches!(*self, Mode::Search(_)) {
                    *self = new;
                }
            }
        }
    }
}

/// Вид поиска, который будет выполнять ripgrep.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SearchMode {
    /// Режим работы по умолчанию. ripgrep ищет совпадения и печатает их,
    /// когда находит.
    ///
    /// Для этого режима нет специального флага, поскольку он является
    /// режимом по умолчанию. Но некоторые из режимов ниже, такие как JSON,
    /// имеют флаги отрицания, такие как --no-json, которые позволяют
    /// вернуться к этому режиму по умолчанию.
    Standard,
    /// Показывает файлы, содержащие хотя бы одно совпадение.
    FilesWithMatches,
    /// Показывает файлы, которые не содержат никаких совпадений.
    FilesWithoutMatch,
    /// Показывает файлы, содержащие хотя бы одно совпадение, и количество
    /// совпадающих строк.
    Count,
    /// Показывает файлы, содержащие хотя бы одно совпадение, и общее
    /// количество совпадений.
    CountMatches,
    /// Печатает совпадения в формате строк JSON.
    JSON,
}

/// То, что генерировать через флаг --generate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GenerateMode {
    /// Генерирует сырой roff, используемый для страницы руководства man.
    Man,
    /// Автодополнения для bash.
    CompleteBash,
    /// Автодополнения для zsh.
    CompleteZsh,
    /// Автодополнения для fish.
    CompleteFish,
    /// Автодополнения для PowerShell.
    CompletePowerShell,
}

/// Указывает, как ripgrep должен обрабатывать двоичные данные.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum BinaryMode {
    /// Автоматически определяет, какой режим двоичных файлов использовать.
    /// По существу, когда файл ищется явно, то он будет искаться с использованием
    /// стратегии `SearchAndSuppress`. В противном случае он будет искаться
    /// способом, который пытается пропускать двоичные файлы как можно больше.
    /// То есть, как только файл классифицирован как двоичный, поиск немедленно
    /// остановится.
    Auto,
    /// Искать файлы, даже когда они содержат двоичные данные, но если совпадение
    /// найдено, подавить его и выдать предупреждение.
    ///
    /// В этом режиме байты `NUL` заменяются терминаторами строк. Это эвристика,
    /// предназначенная для уменьшения использования памяти кучи, поскольку
    /// настоящие двоичные данные не ориентированы на строки. Если кто-то пытается
    /// обрабатывать такие данные как ориентированные на строки, то может
    /// получиться непрактично большие строки. Например, многие двоичные файлы
    /// содержат очень длинные последовательности байтов NUL.
    SearchAndSuppress,
    /// Обрабатывать все файлы, как если бы они были простым текстом. Нет
    /// пропуска и нет замены байтов `NUL` терминаторами строк.
    AsText,
}

impl Default for BinaryMode {
    fn default() -> BinaryMode {
        BinaryMode::Auto
    }
}

/// Указывает, какой вид граничного режима использовать (строка или слово).
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum BoundaryMode {
    /// Разрешать только совпадения, окруженные границами строк.
    Line,
    /// Разрешать только совпадения, окруженные границами слов.
    Word,
}

/// Указывает режим буферизации, который ripgrep должен использовать при выводе.
///
/// По умолчанию — `Auto`.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum BufferMode {
    /// Выбирать режим буферизации, 'line' или 'block', автоматически на основе
    /// того, подключен ли stdout к tty.
    Auto,
    /// Очищать выходной буфер всякий раз, когда виден терминатор строки.
    ///
    /// Это полезно, когда хочет видеть результаты поиска более немедленно,
    /// например, с `tail -f`.
    Line,
    /// Очищать выходной буфер всякий раз, когда он достигает некоторого
    /// фиксированного размера. Размер обычно достаточно велик, чтобы
    /// содержать много строк.
    ///
    /// Это полезно для максимальной производительности, особенно при печати
    /// большого количества результатов.
    Block,
}

impl Default for BufferMode {
    fn default() -> BufferMode {
        BufferMode::Auto
    }
}

/// Указывает режим регистра для того, как интерпретировать все шаблоны,
/// данные ripgrep.
///
/// По умолчанию — `Sensitive`.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum CaseMode {
    /// Шаблоны сопоставляются с учетом регистра. Т.е., `a` не сопоставляется с `A`.
    Sensitive,
    /// Шаблоны сопоставляются без учета регистра. Т.е., `a` сопоставляется с `A`.
    Insensitive,
    /// Шаблоны автоматически сопоставляются без учета регистра только тогда,
    /// когда они состоят из всех строчных буквенных символов. Например, шаблон
    /// `a` будет сопоставляться с `A`, но `A` не будет сопоставляться с `a`.
    Smart,
}

impl Default for CaseMode {
    fn default() -> CaseMode {
        CaseMode::Sensitive
    }
}

/// Указывает, должен ли ripgrep включать цвет/гиперссылки в свой вывод.
///
/// По умолчанию — `Auto`.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum ColorChoice {
    /// Цвет и гиперссылки никогда не будут использоваться.
    Never,
    /// Цвет и гиперссылки будут использоваться только когда stdout подключен к tty.
    Auto,
    /// Цвет всегда будет использоваться.
    Always,
    /// Цвет всегда будет использоваться и будут использоваться только ANSI-последовательности.
    ///
    /// Это имеет смысл только в контексте устаревших API консоли Windows. На момент
    /// написания ripgrep будет пытаться использовать устаревшие API консоли, если
    /// не считается, что ANSI-раскраска возможна. Этот параметр заставит ripgrep
    /// использовать ANSI-раскраску.
    Ansi,
}

impl Default for ColorChoice {
    fn default() -> ColorChoice {
        ColorChoice::Auto
    }
}

impl ColorChoice {
    /// Преобразует этот выбор цвета в соответствующий тип termcolor.
    pub(crate) fn to_termcolor(&self) -> termcolor::ColorChoice {
        match *self {
            ColorChoice::Never => termcolor::ColorChoice::Never,
            ColorChoice::Auto => termcolor::ColorChoice::Auto,
            ColorChoice::Always => termcolor::ColorChoice::Always,
            ColorChoice::Ansi => termcolor::ColorChoice::AlwaysAnsi,
        }
    }
}

/// Указывает опции контекста строк, которые ripgrep должен использовать для вывода.
///
/// По умолчанию — отсутствие контекста вообще.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum ContextMode {
    /// Все строки будут напечатаны. То есть, контекст неограничен.
    Passthru,
    /// Показывать только определенное количество строк до и после каждого совпадения.
    Limited(ContextModeLimited),
}

impl Default for ContextMode {
    fn default() -> ContextMode {
        ContextMode::Limited(ContextModeLimited::default())
    }
}

impl ContextMode {
    /// Устанавливает контекст «до».
    ///
    /// Если это было установлено в контекст «passthru», то оно переопределяется
    /// в пользу ограниченного контекста с данным значением для «до» и `0` для
    /// «после».
    pub(crate) fn set_before(&mut self, lines: usize) {
        match *self {
            ContextMode::Passthru => {
                *self = ContextMode::Limited(ContextModeLimited {
                    before: Some(lines),
                    after: None,
                    both: None,
                })
            }
            ContextMode::Limited(ContextModeLimited {
                ref mut before,
                ..
            }) => *before = Some(lines),
        }
    }

    /// Устанавливает контекст «после».
    ///
    /// Если это было установлено в контекст «passthru», то оно переопределяется
    /// в пользу ограниченного контекста с данным значением для «после» и `0` для
    /// «до».
    pub(crate) fn set_after(&mut self, lines: usize) {
        match *self {
            ContextMode::Passthru => {
                *self = ContextMode::Limited(ContextModeLimited {
                    before: None,
                    after: Some(lines),
                    both: None,
                })
            }
            ContextMode::Limited(ContextModeLimited {
                ref mut after, ..
            }) => *after = Some(lines),
        }
    }

    /// Устанавливает контекст «оба».
    ///
    /// Если это было установлено в контекст «passthru», то оно переопределяется
    /// в пользу ограниченного контекста с данным значением для «оба» и `None` для
    /// «до» и «после».
    pub(crate) fn set_both(&mut self, lines: usize) {
        match *self {
            ContextMode::Passthru => {
                *self = ContextMode::Limited(ContextModeLimited {
                    before: None,
                    after: None,
                    both: Some(lines),
                })
            }
            ContextMode::Limited(ContextModeLimited {
                ref mut both, ..
            }) => *both = Some(lines),
        }
    }

    /// Удобная функция для использования в тестах, которая возвращает
    /// ограниченный контекст. Если этот режим не ограничен, то паникует.
    #[cfg(test)]
    pub(crate) fn get_limited(&self) -> (usize, usize) {
        match *self {
            ContextMode::Passthru => unreachable!("context mode is passthru"),
            ContextMode::Limited(ref limited) => limited.get(),
        }
    }
}

/// Режим контекста для конечного количества строк.
///
/// А именно, это указывает, что определенное количество строк (возможно, ноль)
/// должно быть показано до и/или после каждой совпадающей строки.
///
/// Обратите внимание, что есть тонкая разница между `Some(0)` и `None`. В
/// первом случае это происходит, когда `0` дано явно, тогда как `None` — это
/// значение по умолчанию и возникает, когда значение не указано.
///
/// `both` устанавливается только флагом -C/--context. Причина, по которой мы
/// не просто устанавливаем before = after = --context, заключается в том, что
/// настройки контекста до и после всегда имеют приоритет над настройкой -C/--context,
/// независимо от порядка. Таким образом, нам нужно отслеживать их отдельно.
#[derive(Debug, Default, Eq, PartialEq)]
pub(crate) struct ContextModeLimited {
    before: Option<usize>,
    after: Option<usize>,
    both: Option<usize>,
}

impl ContextModeLimited {
    /// Возвращает определенное количество контекстных строк, которые должны
    /// быть показаны вокруг каждого совпадения. Это учитывает правильный
    /// приоритет, т.е., что `before` и `after` оба частично переопределяют
    /// `both` во всех случаях.
    ///
    /// По умолчанию это возвращает `(0, 0)`.
    pub(crate) fn get(&self) -> (usize, usize) {
        let (mut before, mut after) =
            self.both.map(|lines| (lines, lines)).unwrap_or((0, 0));
        // --before и --after всегда переопределяют --context, независимо
        // от того, где они появляются друг относительно друга.
        if let Some(lines) = self.before {
            before = lines;
        }
        if let Some(lines) = self.after {
            after = lines;
        }
        (before, after)
    }
}

/// Представляет разделитель для использования между несмежными разделами
/// контекстных строк.
///
/// По умолчанию — `--`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ContextSeparator(Option<BString>);

impl Default for ContextSeparator {
    fn default() -> ContextSeparator {
        ContextSeparator(Some(BString::from("--")))
    }
}

impl ContextSeparator {
    /// Создает новый контекстный разделитель из предоставленного пользователем
    /// аргумента. Это обрабатывает экранирование.
    pub(crate) fn new(os: &OsStr) -> anyhow::Result<ContextSeparator> {
        let Some(string) = os.to_str() else {
            anyhow::bail!(
                "separator must be valid UTF-8 (use escape sequences \
                 to provide a separator that is not valid UTF-8)"
            )
        };
        Ok(ContextSeparator(Some(Vec::unescape_bytes(string).into())))
    }

    /// Создает новый разделитель, который инструктирует принтер полностью
    /// отключить контекстные разделители.
    pub(crate) fn disabled() -> ContextSeparator {
        ContextSeparator(None)
    }

    /// Возвращает сырые байты этого разделителя.
    ///
    /// Если контекстные разделители были отключены, то это возвращает `None`.
    ///
    /// Обратите внимание, что это может вернуть вариант `Some` с нулевыми байтами.
    pub(crate) fn into_bytes(self) -> Option<Vec<u8>> {
        self.0.map(|sep| sep.into())
    }
}

/// Режим кодировки, который будет использовать поисковик.
///
/// По умолчанию — `Auto`.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum EncodingMode {
    /// Использовать только BOM sniffing для автоопределения кодировки.
    Auto,
    /// Использовать явную кодировку принудительно, но позволить BOM sniffing
    /// переопределить ее.
    Some(grep::searcher::Encoding),
    /// Не использовать явную кодировку и отключить все BOM sniffing. Это
    /// всегда приведет к поиску сырых байтов, независимо от их истинной
    /// кодировки.
    Disabled,
}

impl Default for EncodingMode {
    fn default() -> EncodingMode {
        EncodingMode::Auto
    }
}

/// Движок регулярных выражений для использования.
///
/// По умолчанию — `Default`.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum EngineChoice {
    /// Использует движок регулярных выражений по умолчанию: крейт Rust `regex`.
    ///
    /// (Ну, технически он использует `regex-automata`, но `regex-automata` —
    /// это реализация крейта `regex`.)
    Default,
    /// Динамически выбирает правильный движок для использования.
    ///
    /// Это работает путем попытки использовать движок по умолчанию, и если
    /// шаблон не компилируется, он переключается на движок PCRE2, если он
    /// доступен.
    Auto,
    /// Использует движок регулярных выражений PCRE2, если он доступен.
    PCRE2,
}

impl Default for EngineChoice {
    fn default() -> EngineChoice {
        EngineChoice::Default
    }
}

/// Разделитель поля контекста для использования между метаданными для каждой
/// контекстной строки.
///
/// По умолчанию — `-`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FieldContextSeparator(BString);

impl Default for FieldContextSeparator {
    fn default() -> FieldContextSeparator {
        FieldContextSeparator(BString::from("-"))
    }
}

impl FieldContextSeparator {
    /// Создает новый разделитель из данного значения аргумента, предоставленного
    /// пользователем. Экранирование обрабатывается автоматически.
    pub(crate) fn new(os: &OsStr) -> anyhow::Result<FieldContextSeparator> {
        let Some(string) = os.to_str() else {
            anyhow::bail!(
                "separator must be valid UTF-8 (use escape sequences \
                 to provide a separator that is not valid UTF-8)"
            )
        };
        Ok(FieldContextSeparator(Vec::unescape_bytes(string).into()))
    }

    /// Возвращает сырые байты этого разделителя.
    ///
    /// Обратите внимание, что это может вернуть пустой `Vec`.
    pub(crate) fn into_bytes(self) -> Vec<u8> {
        self.0.into()
    }
}

/// Разделитель поля совпадения для использования между метаданными для каждой
/// совпадающей строки.
///
/// По умолчанию — `:`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FieldMatchSeparator(BString);

impl Default for FieldMatchSeparator {
    fn default() -> FieldMatchSeparator {
        FieldMatchSeparator(BString::from(":"))
    }
}

impl FieldMatchSeparator {
    /// Создает новый разделитель из данного значения аргумента, предоставленного
    /// пользователем. Экранирование обрабатывается автоматически.
    pub(crate) fn new(os: &OsStr) -> anyhow::Result<FieldMatchSeparator> {
        let Some(string) = os.to_str() else {
            anyhow::bail!(
                "separator must be valid UTF-8 (use escape sequences \
                 to provide a separator that is not valid UTF-8)"
            )
        };
        Ok(FieldMatchSeparator(Vec::unescape_bytes(string).into()))
    }

    /// Возвращает сырые байты этого разделителя.
    ///
    /// Обратите внимание, что это может вернуть пустой `Vec`.
    pub(crate) fn into_bytes(self) -> Vec<u8> {
        self.0.into()
    }
}

/// Тип ведения журнала, который выполнять. `Debug` выводит некоторые детали,
/// а `Trace` выводит гораздо больше.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum LoggingMode {
    Debug,
    Trace,
}

/// Указывает, когда использовать отображения в память.
///
/// По умолчанию — `Auto`.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum MmapMode {
    /// Это инструктирует ripgrep использовать эвристики для выбора, когда
    /// использовать и не использовать отображения в память для поиска.
    Auto,
    /// Это инструктирует ripgrep всегда пытаться использовать отображения
    /// в память, когда это возможно. (Отображения в память невозможны для
    /// использования во всех обстоятельствах, например, для виртуальных файлов.)
    AlwaysTryMmap,
    /// Никогда не использовать отображения в память ни при каких обстоятельствах.
    /// Это включает даже когда многострочный поиск включен, где ripgrep читает
    /// все содержимое файла в кучу перед его поиском.
    Never,
}

impl Default for MmapMode {
    fn default() -> MmapMode {
        MmapMode::Auto
    }
}

/// Представляет источник шаблонов, которые ripgrep должен искать.
///
/// Причина унификации их заключается в том, чтобы мы могли сохранить порядок
/// флагов `-f/--flag` и `-e/--regexp` относительно друг друга.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum PatternSource {
    /// Происходит из флага `-e/--regexp`.
    Regexp(String),
    /// Происходит из флага `-f/--file`.
    File(PathBuf),
}

/// Критерии сортировки, если присутствуют.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct SortMode {
    /// Следует ли инвертировать критерии сортировки (т.е., порядок по убыванию).
    pub(crate) reverse: bool,
    /// Фактические критерии сортировки.
    pub(crate) kind: SortModeKind,
}

/// Критерии для использования для сортировки.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum SortModeKind {
    /// Сортировать по пути.
    Path,
    /// Сортировать по времени последнего изменения.
    LastModified,
    /// Сортировать по времени последнего доступа.
    LastAccessed,
    /// Сортировать по времени создания.
    Created,
}

impl SortMode {
    /// Проверяет, поддерживается ли выбранный режим сортировки. Если нет,
    /// то возвращается ошибка (надеюсь, объясняющая почему).
    pub(crate) fn supported(&self) -> anyhow::Result<()> {
        match self.kind {
            SortModeKind::Path => Ok(()),
            SortModeKind::LastModified => {
                let md = std::env::current_exe()
                    .and_then(|p| p.metadata())
                    .and_then(|md| md.modified());
                let Err(err) = md else { return Ok(()) };
                anyhow::bail!(
                    "сортировка по времени последнего изменения не поддерживается: {err}"
                );
            }
            SortModeKind::LastAccessed => {
                let md = std::env::current_exe()
                    .and_then(|p| p.metadata())
                    .and_then(|md| md.accessed());
                let Err(err) = md else { return Ok(()) };
                anyhow::bail!(
                    "сортировка по времени последнего доступа не поддерживается: {err}"
                );
            }
            SortModeKind::Created => {
                let md = std::env::current_exe()
                    .and_then(|p| p.metadata())
                    .and_then(|md| md.created());
                let Err(err) = md else { return Ok(()) };
                anyhow::bail!(
                    "сортировка по времени создания не поддерживается: {err}"
                );
            }
        }
    }
}

/// Единственный экземпляр либо изменения, либо выбора одного из типов файлов
/// ripgrep.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum TypeChange {
    /// Очистить данный тип из ripgrep.
    Clear { name: String },
    /// Добавить данное определение типа (имя и glob) в ripgrep.
    Add { def: String },
    /// Выбрать данный тип для фильтрации.
    Select { name: String },
    /// Выбрать данный тип для фильтрации, но инвертировать его.
    Negate { name: String },
}
