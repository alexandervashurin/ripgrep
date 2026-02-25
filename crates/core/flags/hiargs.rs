/*!
Предоставляет определение высокоуровневых аргументов из флагов CLI.
*/

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use {
    bstr::BString,
    grep::printer::{ColorSpecs, SummaryKind},
};

use crate::{
    flags::lowargs::{
        BinaryMode, BoundaryMode, BufferMode, CaseMode, ColorChoice,
        ContextMode, ContextSeparator, EncodingMode, EngineChoice,
        FieldContextSeparator, FieldMatchSeparator, LowArgs, MmapMode, Mode,
        PatternSource, SearchMode, SortMode, SortModeKind, TypeChange,
    },
    haystack::{Haystack, HaystackBuilder},
    search::{PatternMatcher, Printer, SearchWorker, SearchWorkerBuilder},
};

/// Высокоуровневое представление аргументов CLI.
///
/// Различие между низкоуровневыми и высокоуровневыми аргументами несколько
/// произвольно и расплывчато. Основная идея здесь заключается в том, что
/// высокоуровневые аргументы обычно требуют, чтобы весь разбор CLI был
/// завершен. Например, нельзя создать глоб-матчер, пока не известны все
/// глоб-шаблоны.
///
/// Поэтому, пока низкоуровневые аргументы собираются во время самого разбора,
/// высокоуровневые аргументы не создаются до тех пор, пока разбор полностью
/// не завершится.
#[derive(Debug)]
pub(crate) struct HiArgs {
    binary: BinaryDetection,
    boundary: Option<BoundaryMode>,
    buffer: BufferMode,
    byte_offset: bool,
    case: CaseMode,
    color: ColorChoice,
    colors: grep::printer::ColorSpecs,
    column: bool,
    context: ContextMode,
    context_separator: ContextSeparator,
    crlf: bool,
    cwd: PathBuf,
    dfa_size_limit: Option<usize>,
    encoding: EncodingMode,
    engine: EngineChoice,
    field_context_separator: FieldContextSeparator,
    field_match_separator: FieldMatchSeparator,
    file_separator: Option<Vec<u8>>,
    fixed_strings: bool,
    follow: bool,
    globs: ignore::overrides::Override,
    heading: bool,
    hidden: bool,
    hyperlink_config: grep::printer::HyperlinkConfig,
    ignore_file_case_insensitive: bool,
    ignore_file: Vec<PathBuf>,
    include_zero: bool,
    invert_match: bool,
    is_terminal_stdout: bool,
    line_number: bool,
    max_columns: Option<u64>,
    max_columns_preview: bool,
    max_count: Option<u64>,
    max_depth: Option<usize>,
    max_filesize: Option<u64>,
    mmap_choice: grep::searcher::MmapChoice,
    mode: Mode,
    multiline: bool,
    multiline_dotall: bool,
    no_ignore_dot: bool,
    no_ignore_exclude: bool,
    no_ignore_files: bool,
    no_ignore_global: bool,
    no_ignore_parent: bool,
    no_ignore_vcs: bool,
    no_require_git: bool,
    no_unicode: bool,
    null_data: bool,
    one_file_system: bool,
    only_matching: bool,
    path_separator: Option<u8>,
    paths: Paths,
    path_terminator: Option<u8>,
    patterns: Patterns,
    pre: Option<PathBuf>,
    pre_globs: ignore::overrides::Override,
    quiet: bool,
    quit_after_match: bool,
    regex_size_limit: Option<usize>,
    replace: Option<BString>,
    search_zip: bool,
    sort: Option<SortMode>,
    stats: Option<grep::printer::Stats>,
    stop_on_nonmatch: bool,
    threads: usize,
    trim: bool,
    types: ignore::types::Types,
    vimgrep: bool,
    with_filename: bool,
}

impl HiArgs {
    /// Преобразует низкоуровневые аргументы в высокоуровневые аргументы.
    ///
    /// Этот процесс может завершиться ошибкой по разным причинам. Например,
    /// невалидные глобы или какая-либо проблема с окружением.
    pub(crate) fn from_low_args(mut low: LowArgs) -> anyhow::Result<HiArgs> {
        // Вызывающие не должны пытаться преобразовывать низкоуровневые аргументы,
        // когда присутствует специальный режим короткого замыкания.
        assert_eq!(None, low.special, "special mode demands short-circuiting");
        // Если режим сортировки не поддерживается, то мы громко завершаемся с
        // ошибкой. Я не уверен, правильно ли это. Мы могли бы молчаливо «не
        // сортировать». Если бы мы хотели пойти по этому пути, то мы могли бы
        // просто установить `low.sort = None`, если `supported()` возвращает ошибку.
        if let Some(ref sort) = low.sort {
            sort.supported()?;
        }

        // Мы изменяем режим на месте в `low`, чтобы последующие преобразования
        // видели правильный режим.
        match low.mode {
            Mode::Search(ref mut mode) => match *mode {
                // трактовать `-v --count-matches` как `-v --count`
                SearchMode::CountMatches if low.invert_match => {
                    *mode = SearchMode::Count;
                }
                // трактовать `-o --count` как `--count-matches`
                SearchMode::Count if low.only_matching => {
                    *mode = SearchMode::CountMatches;
                }
                _ => {}
            },
            _ => {}
        }

        let mut state = State::new()?;
        let patterns = Patterns::from_low_args(&mut state, &mut low)?;
        let paths = Paths::from_low_args(&mut state, &patterns, &mut low)?;

        let binary = BinaryDetection::from_low_args(&state, &low);
        let colors = take_color_specs(&mut state, &mut low);
        let hyperlink_config = take_hyperlink_config(&mut state, &mut low)?;
        let stats = stats(&low);
        let types = types(&low)?;
        let globs = globs(&state, &low)?;
        let pre_globs = preprocessor_globs(&state, &low)?;

        let color = match low.color {
            ColorChoice::Auto if !state.is_terminal_stdout => {
                ColorChoice::Never
            }
            _ => low.color,
        };
        let column = low.column.unwrap_or(low.vimgrep);
        let heading = match low.heading {
            None => !low.vimgrep && state.is_terminal_stdout,
            Some(false) => false,
            Some(true) => !low.vimgrep,
        };
        let path_terminator = if low.null { Some(b'\x00') } else { None };
        let quit_after_match = stats.is_none() && low.quiet;
        let threads = if low.sort.is_some() || paths.is_one_file {
            1
        } else if let Some(threads) = low.threads {
            threads
        } else {
            std::thread::available_parallelism().map_or(1, |n| n.get()).min(12)
        };
        log::debug!("using {threads} thread(s)");
        let with_filename = low
            .with_filename
            .unwrap_or_else(|| low.vimgrep || !paths.is_one_file);

        let file_separator = match low.mode {
            Mode::Search(SearchMode::Standard) => {
                if heading {
                    Some(b"".to_vec())
                } else if let ContextMode::Limited(ref limited) = low.context {
                    let (before, after) = limited.get();
                    if before > 0 || after > 0 {
                        low.context_separator.clone().into_bytes()
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            _ => None,
        };

        let line_number = low.line_number.unwrap_or_else(|| {
            if low.quiet {
                return false;
            }
            let Mode::Search(ref search_mode) = low.mode else { return false };
            match *search_mode {
                SearchMode::FilesWithMatches
                | SearchMode::FilesWithoutMatch
                | SearchMode::Count
                | SearchMode::CountMatches => return false,
                SearchMode::JSON => return true,
                SearchMode::Standard => {
                    // Несколько вещей могут подразумевать подсчет номеров строк. В
                    // частности, мы обычно хотим показывать номера строк по
                    // умолчанию при выводе в tty для человеческого потребления,
                    // за исключением одного интересного случая: когда мы ищем
                    // только stdin. Это делает конвейеры работающими, как ожидается.
                    (state.is_terminal_stdout && !paths.is_only_stdin())
                        || column
                        || low.vimgrep
                }
            }
        });

        let mmap_choice = {
            // БЕЗОПАСНОСТЬ: Отображения в память трудно или невозможно
            // инкапсулировать безопасным переносимым способом, который
            // одновременно не отрицает некоторые из преимуществ использования
            // отображений в память. Для использования ripgrep мы никогда не
            // мутируем отображение в память и обычно никогда не храним
            // содержимое отображения в памяти в структуре данных, которая
            // зависит от неизменяемости. Вообще говоря, худшее, что может
            // случиться, — это SIGBUS (если основной файл усечен во время
            // чтения), что приведет к прерыванию ripgrep. Это рассуждение
            // должно рассматриваться как подозрительное.
            let maybe = unsafe { grep::searcher::MmapChoice::auto() };
            let never = grep::searcher::MmapChoice::never();
            match low.mmap {
                MmapMode::Auto => {
                    if paths.paths.len() <= 10
                        && paths.paths.iter().all(|p| p.is_file())
                    {
                        // Если мы ищем только несколько путей и все они
                        // являются файлами, то отображения в память, вероятно,
                        // быстрее.
                        maybe
                    } else {
                        never
                    }
                }
                MmapMode::AlwaysTryMmap => maybe,
                MmapMode::Never => never,
            }
        };

        Ok(HiArgs {
            mode: low.mode,
            patterns,
            paths,
            binary,
            boundary: low.boundary,
            buffer: low.buffer,
            byte_offset: low.byte_offset,
            case: low.case,
            color,
            colors,
            column,
            context: low.context,
            context_separator: low.context_separator,
            crlf: low.crlf,
            cwd: state.cwd,
            dfa_size_limit: low.dfa_size_limit,
            encoding: low.encoding,
            engine: low.engine,
            field_context_separator: low.field_context_separator,
            field_match_separator: low.field_match_separator,
            file_separator,
            fixed_strings: low.fixed_strings,
            follow: low.follow,
            heading,
            hidden: low.hidden,
            hyperlink_config,
            ignore_file: low.ignore_file,
            ignore_file_case_insensitive: low.ignore_file_case_insensitive,
            include_zero: low.include_zero,
            invert_match: low.invert_match,
            is_terminal_stdout: state.is_terminal_stdout,
            line_number,
            max_columns: low.max_columns,
            max_columns_preview: low.max_columns_preview,
            max_count: low.max_count,
            max_depth: low.max_depth,
            max_filesize: low.max_filesize,
            mmap_choice,
            multiline: low.multiline,
            multiline_dotall: low.multiline_dotall,
            no_ignore_dot: low.no_ignore_dot,
            no_ignore_exclude: low.no_ignore_exclude,
            no_ignore_files: low.no_ignore_files,
            no_ignore_global: low.no_ignore_global,
            no_ignore_parent: low.no_ignore_parent,
            no_ignore_vcs: low.no_ignore_vcs,
            no_require_git: low.no_require_git,
            no_unicode: low.no_unicode,
            null_data: low.null_data,
            one_file_system: low.one_file_system,
            only_matching: low.only_matching,
            globs,
            path_separator: low.path_separator,
            path_terminator,
            pre: low.pre,
            pre_globs,
            quiet: low.quiet,
            quit_after_match,
            regex_size_limit: low.regex_size_limit,
            replace: low.replace,
            search_zip: low.search_zip,
            sort: low.sort,
            stats,
            stop_on_nonmatch: low.stop_on_nonmatch,
            threads,
            trim: low.trim,
            types,
            vimgrep: low.vimgrep,
            with_filename,
        })
    }

    /// Возвращает писатель для вывода буферов в stdout.
    ///
    /// Это предназначено для использования из нескольких потоков. А именно,
    /// буферный писатель может создавать новые буферы, которые отправляются
    /// в потоки. Потоки могут независимо записывать в буферы. Как только
    /// единица работы завершена, буфер может быть передан буферному писателю
    /// для записи в stdout.
    pub(crate) fn buffer_writer(&self) -> termcolor::BufferWriter {
        let mut wtr =
            termcolor::BufferWriter::stdout(self.color.to_termcolor());
        wtr.separator(self.file_separator.clone());
        wtr
    }

    /// Возвращает true, когда ripgrep должен был угадать поиск в текущем
    /// рабочем каталоге. То есть, это true, когда ripgrep вызван без каких-либо
    /// путей к файлам или каталогов для поиска.
    ///
    /// Помимо изменения того, как печатаются пути к файлам (т.е. без ведущего
    /// `./`), это также полезно по диагностическим причинам. Например, ripgrep
    /// выведет сообщение об ошибке, когда ничего не найдено, поскольку возможно,
    /// что действующие правила игнорирования слишком агрессивны. Но это
    /// предупреждение выводится только тогда, когда ripgrep был вызван без
    /// каких-либо явных путей к файлам, поскольку в противном случае
    /// предупреждение было бы слишком агрессивным.
    pub(crate) fn has_implicit_path(&self) -> bool {
        self.paths.has_implicit_path
    }

    /// Возвращает правильно настроенный построитель для создания стогов сена.
    ///
    /// Построитель может использоваться для превращения записи каталога (из
    /// крейта `ignore`) во что-то, что можно искать.
    pub(crate) fn haystack_builder(&self) -> HaystackBuilder {
        let mut builder = HaystackBuilder::new();
        builder.strip_dot_prefix(self.paths.has_implicit_path);
        builder
    }

    /// Возвращает матчер, который должен использоваться для поиска с использованием
    /// выбора движка, сделанного пользователем.
    ///
    /// Если возникла проблема с созданием матчера (например, ошибка синтаксиса),
    /// то возвращается ошибка.
    pub(crate) fn matcher(&self) -> anyhow::Result<PatternMatcher> {
        match self.engine {
            EngineChoice::Default => match self.matcher_rust() {
                Ok(m) => Ok(m),
                Err(err) => {
                    anyhow::bail!(suggest_other_engine(err.to_string()));
                }
            },
            EngineChoice::PCRE2 => Ok(self.matcher_pcre2()?),
            EngineChoice::Auto => {
                let rust_err = match self.matcher_rust() {
                    Ok(m) => return Ok(m),
                    Err(err) => err,
                };
                log::debug!(
                    "error building Rust regex in hybrid mode:\n{rust_err}",
                );

                let pcre_err = match self.matcher_pcre2() {
                    Ok(m) => return Ok(m),
                    Err(err) => err,
                };
                let divider = "~".repeat(79);
                anyhow::bail!(
                    "regex could not be compiled with either the default \
                     regex engine or with PCRE2.\n\n\
                     default regex engine error:\n\
                     {divider}\n\
                     {rust_err}\n\
                     {divider}\n\n\
                     PCRE2 regex engine error:\n{pcre_err}",
                );
            }
        }
    }

    /// Создает матчер с использованием PCRE2.
    ///
    /// Если возникла проблема с созданием матчера (например, ошибка синтаксиса
    /// регулярного выражения), то возвращается ошибка.
    ///
    /// Если функция `pcre2` не включена, то это всегда возвращает ошибку.
    fn matcher_pcre2(&self) -> anyhow::Result<PatternMatcher> {
        #[cfg(feature = "pcre2")]
        {
            let mut builder = grep::pcre2::RegexMatcherBuilder::new();
            builder.multi_line(true).fixed_strings(self.fixed_strings);
            match self.case {
                CaseMode::Sensitive => builder.caseless(false),
                CaseMode::Insensitive => builder.caseless(true),
                CaseMode::Smart => builder.case_smart(true),
            };
            if let Some(ref boundary) = self.boundary {
                match *boundary {
                    BoundaryMode::Line => builder.whole_line(true),
                    BoundaryMode::Word => builder.word(true),
                };
            }
            // По какой-то причине JIT выдает ошибку «не хватает памяти» во время
            // компиляции регулярного выражения в 32-битных системах. Поэтому
            // не используем его там.
            if cfg!(target_pointer_width = "64") {
                builder
                    .jit_if_available(true)
                    // В документации PCRE2 сказано, что 32 КБ — это значение по
                    // умолчанию, и что 1 МБ должно быть достаточно для чего угодно.
                    // Но давайте увеличим до 10 МБ.
                    .max_jit_stack_size(Some(10 * (1 << 20)));
            }
            if !self.no_unicode {
                builder.utf(true).ucp(true);
            }
            if self.multiline {
                builder.dotall(self.multiline_dotall);
            }
            if self.crlf {
                builder.crlf(true);
            }
            let m = builder.build_many(&self.patterns.patterns)?;
            Ok(PatternMatcher::PCRE2(m))
        }
        #[cfg(not(feature = "pcre2"))]
        {
            Err(anyhow::anyhow!(
                "PCRE2 is not available in this build of ripgrep"
            ))
        }
    }

    /// Создает матчер с использованием движка регулярных выражений Rust.
    ///
    /// Если возникла проблема с созданием матчера (например, ошибка синтаксиса
    /// регулярного выражения), то возвращается ошибка.
    fn matcher_rust(&self) -> anyhow::Result<PatternMatcher> {
        let mut builder = grep::regex::RegexMatcherBuilder::new();
        builder
            .multi_line(true)
            .unicode(!self.no_unicode)
            .octal(false)
            .fixed_strings(self.fixed_strings);
        match self.case {
            CaseMode::Sensitive => builder.case_insensitive(false),
            CaseMode::Insensitive => builder.case_insensitive(true),
            CaseMode::Smart => builder.case_smart(true),
        };
        if let Some(ref boundary) = self.boundary {
            match *boundary {
                BoundaryMode::Line => builder.whole_line(true),
                BoundaryMode::Word => builder.word(true),
            };
        }
        if self.multiline {
            builder.dot_matches_new_line(self.multiline_dotall);
            if self.crlf {
                builder.crlf(true).line_terminator(None);
            }
        } else {
            builder.line_terminator(Some(b'\n')).dot_matches_new_line(false);
            if self.crlf {
                builder.crlf(true);
            }
            // Нам не нужно устанавливать это в многострочном режиме, поскольку
            // многострочные матчеры не используют оптимизации, связанные с
            // терминаторами строк. Более того, многострочное регулярное
            // выражение, используемое с --null-data, должно иметь возможность
            // явно сопоставлять байты NUL, что в противном случае было бы
            // запрещено.
            if self.null_data {
                builder.line_terminator(Some(b'\x00'));
            }
        }
        if let Some(limit) = self.regex_size_limit {
            builder.size_limit(limit);
        }
        if let Some(limit) = self.dfa_size_limit {
            builder.dfa_size_limit(limit);
        }
        if !self.binary.is_none() {
            builder.ban_byte(Some(b'\x00'));
        }
        let m = match builder.build_many(&self.patterns.patterns) {
            Ok(m) => m,
            Err(err) => {
                anyhow::bail!(suggest_text(suggest_multiline(err.to_string())))
            }
        };
        Ok(PatternMatcher::RustRegex(m))
    }

    /// Возвращает true, если считается, что возможно некоторое ненулевое
    /// количество совпадений.
    ///
    /// Когда это возвращает false, для ripgrep невозможно когда-либо сообщить
    /// о совпадении.
    pub(crate) fn matches_possible(&self) -> bool {
        if self.patterns.patterns.is_empty() && !self.invert_match {
            return false;
        }
        if self.max_count == Some(0) {
            return false;
        }
        true
    }

    /// Возвращает «режим», в котором должен работать ripgrep.
    ///
    /// Это обычно полезно для определения того, какое действие должен
    /// выполнять ripgrep. Основным режимом, конечно, является «поиск», но
    /// есть и другие режимы, не связанные с поиском, такие как `--type-list`
    /// и `--files`.
    pub(crate) fn mode(&self) -> Mode {
        self.mode
    }

    /// Возвращает построитель для создания «принтера путей».
    ///
    /// Это полезно для режима `--files` в ripgrep, где принтеру просто нужно
    /// выводить пути и не нужно беспокоиться о функциональности поиска.
    pub(crate) fn path_printer_builder(
        &self,
    ) -> grep::printer::PathPrinterBuilder {
        let mut builder = grep::printer::PathPrinterBuilder::new();
        builder
            .color_specs(self.colors.clone())
            .hyperlink(self.hyperlink_config.clone())
            .separator(self.path_separator.clone())
            .terminator(self.path_terminator.unwrap_or(b'\n'));
        builder
    }

    /// Возвращает принтер для данного режима поиска.
    ///
    /// Это выбирает, какой принтер строить (JSON, сводка или стандартный) на
    /// основе данного режима поиска.
    pub(crate) fn printer<W: termcolor::WriteColor>(
        &self,
        search_mode: SearchMode,
        wtr: W,
    ) -> Printer<W> {
        let summary_kind = if self.quiet {
            match search_mode {
                SearchMode::FilesWithMatches
                | SearchMode::Count
                | SearchMode::CountMatches
                | SearchMode::JSON
                | SearchMode::Standard => SummaryKind::QuietWithMatch,
                SearchMode::FilesWithoutMatch => {
                    SummaryKind::QuietWithoutMatch
                }
            }
        } else {
            match search_mode {
                SearchMode::FilesWithMatches => SummaryKind::PathWithMatch,
                SearchMode::FilesWithoutMatch => SummaryKind::PathWithoutMatch,
                SearchMode::Count => SummaryKind::Count,
                SearchMode::CountMatches => SummaryKind::CountMatches,
                SearchMode::JSON => {
                    return Printer::JSON(self.printer_json(wtr));
                }
                SearchMode::Standard => {
                    return Printer::Standard(self.printer_standard(wtr));
                }
            }
        };
        Printer::Summary(self.printer_summary(wtr, summary_kind))
    }

    /// Создает JSON принтер.
    fn printer_json<W: std::io::Write>(
        &self,
        wtr: W,
    ) -> grep::printer::JSON<W> {
        grep::printer::JSONBuilder::new()
            .pretty(false)
            .always_begin_end(false)
            .replacement(self.replace.clone().map(|r| r.into()))
            .build(wtr)
    }

    /// Создает «стандартный» grep принтер, где совпадения печатаются как
    /// строки простого текста.
    fn printer_standard<W: termcolor::WriteColor>(
        &self,
        wtr: W,
    ) -> grep::printer::Standard<W> {
        let mut builder = grep::printer::StandardBuilder::new();
        builder
            .byte_offset(self.byte_offset)
            .color_specs(self.colors.clone())
            .column(self.column)
            .heading(self.heading)
            .hyperlink(self.hyperlink_config.clone())
            .max_columns_preview(self.max_columns_preview)
            .max_columns(self.max_columns)
            .only_matching(self.only_matching)
            .path(self.with_filename)
            .path_terminator(self.path_terminator.clone())
            .per_match_one_line(true)
            .per_match(self.vimgrep)
            .replacement(self.replace.clone().map(|r| r.into()))
            .separator_context(self.context_separator.clone().into_bytes())
            .separator_field_context(
                self.field_context_separator.clone().into_bytes(),
            )
            .separator_field_match(
                self.field_match_separator.clone().into_bytes(),
            )
            .separator_path(self.path_separator.clone())
            .stats(self.stats.is_some())
            .trim_ascii(self.trim);
        // При выполнении многопоточного поиска буферный писатель отвечает
        // за запись разделителей, поскольку он является единственной вещью,
        // которая знает, было ли что-то напечатано или нет. Но для однопоточного
        // случая мы не используем буферный писатель и, таким образом, можем
        // позволить принтеру владеть этим.
        if self.threads == 1 {
            builder.separator_search(self.file_separator.clone());
        }
        builder.build(wtr)
    }

    /// Создает «сводный» принтер, где результаты поиска агрегируются на
    /// основе каждого файла.
    fn printer_summary<W: termcolor::WriteColor>(
        &self,
        wtr: W,
        kind: SummaryKind,
    ) -> grep::printer::Summary<W> {
        grep::printer::SummaryBuilder::new()
            .color_specs(self.colors.clone())
            .exclude_zero(!self.include_zero)
            .hyperlink(self.hyperlink_config.clone())
            .kind(kind)
            .path(self.with_filename)
            .path_terminator(self.path_terminator.clone())
            .separator_field(b":".to_vec())
            .separator_path(self.path_separator.clone())
            .stats(self.stats.is_some())
            .build(wtr)
    }

    /// Возвращает true, если ripgrep должен работать в «тихом» режиме.
    ///
    /// Вообще говоря, тихий режим означает, что ripgrep не должен печатать
    /// ничего в stdout. Есть некоторые исключения. Например, когда пользователь
    /// предоставил `--stats`, то ripgrep выведет статистику в stdout.
    pub(crate) fn quiet(&self) -> bool {
        self.quiet
    }

    /// Возвращает true, когда ripgrep должен прекратить поиск после нахождения
    /// единственного совпадения.
    ///
    /// Это полезно, например, когда включен тихий режим. В этом случае
    /// пользователи обычно не могут заметить разницу в поведении между поиском,
    /// который находит все совпадения, и поиском, который находит только одно
    /// из них. (Исключением здесь является, если дан `--stats`, то
    /// `quit_after_match` всегда будет возвращать false, поскольку пользователь
    /// ожидает, что ripgrep найдет все.)
    pub(crate) fn quit_after_match(&self) -> bool {
        self.quit_after_match
    }

    /// Создает рабочего для выполнения поисков.
    ///
    /// Результаты поиска находятся с использованием данного матчера и
    /// записываются в данный принтер.
    pub(crate) fn search_worker<W: termcolor::WriteColor>(
        &self,
        matcher: PatternMatcher,
        searcher: grep::searcher::Searcher,
        printer: Printer<W>,
    ) -> anyhow::Result<SearchWorker<W>> {
        let mut builder = SearchWorkerBuilder::new();
        builder
            .preprocessor(self.pre.clone())?
            .preprocessor_globs(self.pre_globs.clone())
            .search_zip(self.search_zip)
            .binary_detection_explicit(self.binary.explicit.clone())
            .binary_detection_implicit(self.binary.implicit.clone());
        Ok(builder.build(matcher, searcher, printer))
    }

    /// Создает поисковик из параметров командной строки.
    pub(crate) fn searcher(&self) -> anyhow::Result<grep::searcher::Searcher> {
        let line_term = if self.crlf {
            grep::matcher::LineTerminator::crlf()
        } else if self.null_data {
            grep::matcher::LineTerminator::byte(b'\x00')
        } else {
            grep::matcher::LineTerminator::byte(b'\n')
        };
        let mut builder = grep::searcher::SearcherBuilder::new();
        builder
            .max_matches(self.max_count)
            .line_terminator(line_term)
            .invert_match(self.invert_match)
            .line_number(self.line_number)
            .multi_line(self.multiline)
            .memory_map(self.mmap_choice.clone())
            .stop_on_nonmatch(self.stop_on_nonmatch);
        match self.context {
            ContextMode::Passthru => {
                builder.passthru(true);
            }
            ContextMode::Limited(ref limited) => {
                let (before, after) = limited.get();
                builder.before_context(before);
                builder.after_context(after);
            }
        }
        match self.encoding {
            EncodingMode::Auto => {} // default for the searcher
            EncodingMode::Some(ref enc) => {
                builder.encoding(Some(enc.clone()));
            }
            EncodingMode::Disabled => {
                builder.bom_sniffing(false);
            }
        }
        Ok(builder.build())
    }

    /// Учитывая итератор стогов сена, сортирует их, если необходимо.
    ///
    /// Когда сортировка необходима, этот метод соберет весь итератор в
    /// память, отсортирует их, а затем вернет новый итератор. Когда сортировка
    /// не необходима, то данный итератор возвращается как есть без сбора
    /// его в память.
    ///
    /// Особый случай — когда запрошена сортировка по пути в порядке
    /// возрастания. В этом случае данный итератор возвращается как есть без
    /// какой-либо дополнительной сортировки. Это делается потому, что
    /// `walk_builder()` будет сортировать итератор, который он выдает во время
    /// обхода каталога, поэтому дополнительная сортировка не нужна.
    pub(crate) fn sort<'a, I>(
        &self,
        haystacks: I,
    ) -> Box<dyn Iterator<Item = Haystack> + 'a>
    where
        I: Iterator<Item = Haystack> + 'a,
    {
        use std::{cmp::Ordering, fs::Metadata, io, time::SystemTime};

        fn attach_timestamps(
            haystacks: impl Iterator<Item = Haystack>,
            get: impl Fn(&Metadata) -> io::Result<SystemTime>,
        ) -> impl Iterator<Item = (Haystack, Option<SystemTime>)> {
            haystacks.map(move |s| {
                let time = s.path().metadata().and_then(|m| get(&m)).ok();
                (s, time)
            })
        }

        let Some(ref sort) = self.sort else { return Box::new(haystacks) };
        let mut with_timestamps: Vec<_> = match sort.kind {
            SortModeKind::Path if !sort.reverse => return Box::new(haystacks),
            SortModeKind::Path => {
                let mut haystacks = haystacks.collect::<Vec<Haystack>>();
                haystacks.sort_by(|ref h1, ref h2| {
                    h1.path().cmp(h2.path()).reverse()
                });
                return Box::new(haystacks.into_iter());
            }
            SortModeKind::LastModified => {
                attach_timestamps(haystacks, |md| md.modified()).collect()
            }
            SortModeKind::LastAccessed => {
                attach_timestamps(haystacks, |md| md.accessed()).collect()
            }
            SortModeKind::Created => {
                attach_timestamps(haystacks, |md| md.created()).collect()
            }
        };
        with_timestamps.sort_by(|(_, t1), (_, t2)| {
            let ordering = match (*t1, *t2) {
                // Both have metadata, do the obvious thing.
                (Some(t1), Some(t2)) => t1.cmp(&t2),
                // Things that error should appear later (when ascending).
                (Some(_), None) => Ordering::Less,
                // Things that error should appear later (when ascending).
                (None, Some(_)) => Ordering::Greater,
                // When both error, we can't distinguish, so treat as equal.
                (None, None) => Ordering::Equal,
            };
            if sort.reverse { ordering.reverse() } else { ordering }
        });
        Box::new(with_timestamps.into_iter().map(|(s, _)| s))
    }

    /// Возвращает объект статистики, если пользователь запросил, чтобы ripgrep
    /// отслеживал различные метрики во время поиска.
    ///
    /// Когда это возвращает `None`, то вызывающие могут предположить, что
    /// пользователь не запросил статистику.
    pub(crate) fn stats(&self) -> Option<grep::printer::Stats> {
        self.stats.clone()
    }

    /// Возвращает писатель с поддержкой цвета для stdout.
    ///
    /// Возвращаемый писатель также настроен на выполнение либо построчной,
    /// либо поблочной буферизации на основе явной конфигурации от пользователя
    /// через флаги CLI или автоматически на основе того, подключен ли stdout
    /// к tty.
    pub(crate) fn stdout(&self) -> grep::cli::StandardStream {
        let color = self.color.to_termcolor();
        match self.buffer {
            BufferMode::Auto => {
                if self.is_terminal_stdout {
                    grep::cli::stdout_buffered_line(color)
                } else {
                    grep::cli::stdout_buffered_block(color)
                }
            }
            BufferMode::Line => grep::cli::stdout_buffered_line(color),
            BufferMode::Block => grep::cli::stdout_buffered_block(color),
        }
    }

    /// Возвращает общее количество потоков, которые ripgrep должен использовать
    /// для выполнения поиска.
    ///
    /// Это число является результатом размышлений как об эвристиках (например,
    /// доступное количество ядер), так и о том, поддерживает ли режим ripgrep
    /// параллелизм. Предполагается, что это число будет использоваться для
    /// непосредственного определения того, сколько потоков создавать.
    pub(crate) fn threads(&self) -> usize {
        self.threads
    }

    /// Возвращает созданный матчер типов файлов.
    ///
    /// Матчер включает как правила по умолчанию, так и любые правила,
    /// добавленные пользователем для этого конкретного вызова.
    pub(crate) fn types(&self) -> &ignore::types::Types {
        &self.types
    }

    /// Создает новый построитель для рекурсивного обхода каталога.
    ///
    /// Возвращенный построитель может быть использован для запуска
    /// однопоточного или многопоточного обхода каталога. Для многопоточного
    /// обхода количество настроенных потоков эквивалентно `HiArgs::threads`.
    ///
    /// Если `HiArgs::threads` равен `1`, то вызывающие обычно должны явно
    /// использовать однопоточный обход, поскольку он не будет иметь
    /// ненужных накладных расходов на синхронизацию.
    pub(crate) fn walk_builder(&self) -> anyhow::Result<ignore::WalkBuilder> {
        let mut builder = ignore::WalkBuilder::new(&self.paths.paths[0]);
        for path in self.paths.paths.iter().skip(1) {
            builder.add(path);
        }
        if !self.no_ignore_files {
            for path in self.ignore_file.iter() {
                if let Some(err) = builder.add_ignore(path) {
                    ignore_message!("{err}");
                }
            }
        }
        builder
            .max_depth(self.max_depth)
            .follow_links(self.follow)
            .max_filesize(self.max_filesize)
            .threads(self.threads)
            .same_file_system(self.one_file_system)
            .skip_stdout(matches!(self.mode, Mode::Search(_)))
            .overrides(self.globs.clone())
            .types(self.types.clone())
            .hidden(!self.hidden)
            .parents(!self.no_ignore_parent)
            .ignore(!self.no_ignore_dot)
            .git_global(!self.no_ignore_vcs && !self.no_ignore_global)
            .git_ignore(!self.no_ignore_vcs)
            .git_exclude(!self.no_ignore_vcs && !self.no_ignore_exclude)
            .require_git(!self.no_require_git)
            .ignore_case_insensitive(self.ignore_file_case_insensitive)
            .current_dir(&self.cwd);
        if !self.no_ignore_dot {
            builder.add_custom_ignore_filename(".rgignore");
        }
        // When we want to sort paths lexicographically in ascending order,
        // then we can actually do this during directory traversal itself.
        // Otherwise, sorting is done by collecting all paths, sorting them and
        // then searching them.
        if let Some(ref sort) = self.sort {
            assert_eq!(1, self.threads, "sorting implies single threaded");
            if !sort.reverse && matches!(sort.kind, SortModeKind::Path) {
                builder.sort_by_file_name(|a, b| a.cmp(b));
            }
        }
        Ok(builder)
    }
}

/// Состояние, которое нужно вычислить только один раз во время разбора аргументов.
///
/// Это состояние предназначено для того, чтобы быть несколько общим и
/// разделяемым между несколькими преобразованиями низкоуровневых аргументов
/// в высокоуровневые. Состояние может даже мутироваться различными
/// преобразованиями как способ сообщения об изменениях другим преобразованиям.
/// Например, чтение шаблонов может потреблять из stdin. Если мы знаем, что
/// stdin был потреблен и никакие другие пути к файлам не были даны, то мы
/// знаем наверняка, что должны искать в CWD. Таким образом, изменение состояния
/// при чтении шаблонов может повлиять на то, как в конечном итоге создаются
/// пути к файлам.
#[derive(Debug)]
struct State {
    /// Считается ли, что tty подключен к stdout. Обратите внимание, что в
    /// Unix-системах это всегда правильно. В Windows эвристики используются
    /// стандартной библиотекой Rust, особенно для окружений cygwin/MSYS.
    is_terminal_stdout: bool,
    /// Был ли stdin уже потреблен. Это полезно знать и для предоставления
    /// хороших сообщений об ошибках, когда пользователь попытался читать из
    /// stdin в двух разных местах. Например, `rg -f - -`.
    stdin_consumed: bool,
    /// Текущий рабочий каталог.
    cwd: PathBuf,
}

impl State {
    /// Инициализирует состояние некоторыми разумными значениями по умолчанию.
    ///
    /// Обратите внимание, что значения состояния могут изменяться в течение
    /// времени жизни разбора аргументов.
    fn new() -> anyhow::Result<State> {
        use std::io::IsTerminal;

        let cwd = current_dir()?;
        log::debug!("read CWD from environment: {}", cwd.display());
        Ok(State {
            is_terminal_stdout: std::io::stdout().is_terminal(),
            stdin_consumed: false,
            cwd,
        })
    }
}

/// Дизъюнкция шаблонов для поиска.
///
/// Количество шаблонов может быть пустым, например, через `-f /dev/null`.
#[derive(Debug)]
struct Patterns {
    /// Фактические шаблоны для сопоставления.
    patterns: Vec<String>,
}

impl Patterns {
    /// Извлекает шаблоны из низкоуровневых аргументов.
    ///
    /// Это включает сбор шаблонов из -e/--regexp и -f/--file.
    ///
    /// Если вызов подразумевает, что первый позиционный аргумент является
    /// шаблоном (наиболее распространенный случай), то первый позиционный
    /// аргумент также извлекается.
    fn from_low_args(
        state: &mut State,
        low: &mut LowArgs,
    ) -> anyhow::Result<Patterns> {
        // Первый позиционный аргумент является шаблоном только тогда, когда
        // ripgrep инструктирован искать и ни -e/--regexp, ни -f/--file не даны.
        // В основном, первый позиционный аргумент является шаблоном только
        // тогда, когда шаблон не был дан каким-либо другим способом.

        // Отсутствие поиска означает отсутствие шаблонов. Даже если даны
        // -e/--regexp или -f/--file, мы знаем, что не будем их использовать,
        // поэтому не будем их собирать.
        if !matches!(low.mode, Mode::Search(_)) {
            return Ok(Patterns { patterns: vec![] });
        }
        // Если мы ничего не получили от -e/--regexp и -f/--file, то первый
        // позиционный аргумент является шаблоном.
        if low.patterns.is_empty() {
            anyhow::ensure!(
                !low.positional.is_empty(),
                "ripgrep требует как минимум один шаблон для выполнения поиска"
            );
            let ospat = low.positional.remove(0);
            let Ok(pat) = ospat.into_string() else {
                anyhow::bail!("данный шаблон не является валидным UTF-8")
            };
            return Ok(Patterns { patterns: vec![pat] });
        }
        // В противном случае нам нужно прочитать наши шаблоны из -e/--regexp и
        // -f/--file. Мы дедуплицируем по мере продвижения. Если мы не будем
        // дедуплицировать, то это может привести к серьезным замедлениям для
        // неаккуратных входных данных. Это может быть удивительно, и движок
        // регулярных выражений в конечном итоге дедуплицирует дублирующиеся
        // ветви в одном регулярном выражении (может быть), но не до тех пор,
        // пока он не пройдет через парсинг и некоторые другие уровни. Если
        // есть много дубликатов, то это может привести к значительным
        // дополнительным затратам. Прискорбно, что мы платим дополнительные
        // затраты здесь для дедупликации для вероятно uncommon случая, но я
        // видел, что это имеет большое влияние на реальные данные.
        let mut seen = HashSet::new();
        let mut patterns = Vec::with_capacity(low.patterns.len());
        let mut add = |pat: String| {
            if !seen.contains(&pat) {
                seen.insert(pat.clone());
                patterns.push(pat);
            }
        };
        for source in low.patterns.drain(..) {
            match source {
                PatternSource::Regexp(pat) => add(pat),
                PatternSource::File(path) => {
                    if path == Path::new("-") {
                        anyhow::ensure!(
                            !state.stdin_consumed,
                            "ошибка чтения -f/--file из stdin: stdin уже был потреблен"
                        );
                        for pat in grep::cli::patterns_from_stdin()? {
                            add(pat);
                        }
                        state.stdin_consumed = true;
                    } else {
                        for pat in grep::cli::patterns_from_path(&path)? {
                            add(pat);
                        }
                    }
                }
            }
        }
        Ok(Patterns { patterns })
    }
}

/// Коллекция путей, которые мы хотим искать.
///
/// Это гарантирует, что всегда есть как минимум один путь.
#[derive(Debug)]
struct Paths {
    /// Фактические пути.
    paths: Vec<PathBuf>,
    /// Это true, когда ripgrep должен был угадать поиск в текущем рабочем
    /// каталоге. Например, когда пользователь просто запускает `rg foo`.
    /// Странно нуждаться в этом, но это тонко изменяет то, как печатаются пути.
    /// Когда не дано явных путей, ripgrep печатает относительные пути. Но когда
    /// даны явные пути, ripgrep печатает пути так, как они были даны.
    has_implicit_path: bool,
    /// Это true, когда известно, что будет искаться только один файловый дескриптор.
    is_one_file: bool,
}

impl Paths {
    /// Извлекает пути для поиска из данных низкоуровневых аргументов.
    fn from_low_args(
        state: &mut State,
        _: &Patterns,
        low: &mut LowArgs,
    ) -> anyhow::Result<Paths> {
        // Нам требуется `&Patterns`, даже though мы не используем его, чтобы
        // гарантировать, что шаблоны уже были прочитаны из LowArgs. Это
        // позволяет нам безопасно предполагать, что все оставшиеся позиционные
        // аргументы предназначены для путей к файлам.

        let mut paths = Vec::with_capacity(low.positional.len());
        for osarg in low.positional.drain(..) {
            let path = PathBuf::from(osarg);
            if state.stdin_consumed && path == Path::new("-") {
                anyhow::bail!(
                    "ошибка: попытка чтения шаблонов из stdin \
                     во время поиска stdin",
                );
            }
            paths.push(path);
        }
        log::debug!("количество путей для поиска: {}", paths.len());
        if !paths.is_empty() {
            let is_one_file = paths.len() == 1
                // Обратите внимание, что мы используем `!paths[0].is_dir()` здесь
                // вместо `paths[0].is_file()`. А именно, последнее может
                // возвращать `false`, даже когда путь является чем-то, напоминающим
                // файл. Поэтому вместо этого мы просто считаем путь файлом,
                // пока мы знаем, что это не каталог.
                //
                // См.: https://github.com/BurntSushi/ripgrep/issues/2736
                && (paths[0] == Path::new("-") || !paths[0].is_dir());
            log::debug!("is_one_file? {is_one_file:?}");
            return Ok(Paths { paths, has_implicit_path: false, is_one_file });
        }
        // N.B. is_readable_stdin — это эвристика! Часть проблемы заключается в том, что
        // многие API «выполнения процесса» открывают канал stdin, даже though stdin
        // на самом деле не используется. ripgrep затем думает, что он должен искать
        // stdin, и получается видимость того, что он зависает. Это ужасный режим
        // отказа, но на самом деле нет хорошего способа смягчить это. Это просто
        // следствие того, что позволяем пользователю вводить 'rg foo' и «угадываем»,
        // что он имел в виду поиск CWD.
        let is_readable_stdin = grep::cli::is_readable_stdin();
        let use_cwd = !is_readable_stdin
            || state.stdin_consumed
            || !matches!(low.mode, Mode::Search(_));
        log::debug!(
            "использование эвристик для определения, читать ли из \
             stdin или искать ./ (\
             is_readable_stdin={is_readable_stdin}, \
             stdin_consumed={stdin_consumed}, \
             mode={mode:?})",
            stdin_consumed = state.stdin_consumed,
            mode = low.mode,
        );
        let (path, is_one_file) = if use_cwd {
            log::debug!("эвристика выбрала поиск ./");
            (PathBuf::from("./"), false)
        } else {
            log::debug!("эвристика выбрала поиск stdin");
            (PathBuf::from("-"), true)
        };
        Ok(Paths { paths: vec![path], has_implicit_path: true, is_one_file })
    }

    /// Возвращает true, если ripgrep будет искать только stdin и ничего больше.
    fn is_only_stdin(&self) -> bool {
        self.paths.len() == 1 && self.paths[0] == Path::new("-")
    }
}

/// Конфигурация «обнаружения двоичных файлов», которую ripgrep должен использовать.
///
/// ripgrep на самом деле использует две различные эвристики обнаружения двоичных
/// файлов в зависимости от того, ищется ли файл явно (например, через аргумент CLI)
/// или неявно (например, через обход каталога). В общем, первая никогда не может
/// использовать эвристику, которая позволяет ей «прекратить» поиск до получения
/// EOF или нахождения совпадения. (Поскольку в противном случае это считалось бы
/// фильтром, а ripgrep следует правилу, что явно данный файл всегда ищется.)
#[derive(Debug)]
struct BinaryDetection {
    explicit: grep::searcher::BinaryDetection,
    implicit: grep::searcher::BinaryDetection,
}

impl BinaryDetection {
    /// Определяет правильный режим обнаружения двоичных файлов из низкоуровневых аргументов.
    fn from_low_args(_: &State, low: &LowArgs) -> BinaryDetection {
        let none = matches!(low.binary, BinaryMode::AsText) || low.null_data;
        let convert = matches!(low.binary, BinaryMode::SearchAndSuppress);
        let explicit = if none {
            grep::searcher::BinaryDetection::none()
        } else {
            grep::searcher::BinaryDetection::convert(b'\x00')
        };
        let implicit = if none {
            grep::searcher::BinaryDetection::none()
        } else if convert {
            grep::searcher::BinaryDetection::convert(b'\x00')
        } else {
            grep::searcher::BinaryDetection::quit(b'\x00')
        };
        BinaryDetection { explicit, implicit }
    }

    /// Возвращает true, когда и неявное, и явное обнаружение двоичных файлов
    /// отключено.
    pub(crate) fn is_none(&self) -> bool {
        let none = grep::searcher::BinaryDetection::none();
        self.explicit == none && self.implicit == none
    }
}

/// Создает матчер типов файлов из низкоуровневых аргументов.
fn types(low: &LowArgs) -> anyhow::Result<ignore::types::Types> {
    let mut builder = ignore::types::TypesBuilder::new();
    builder.add_defaults();
    for tychange in low.type_changes.iter() {
        match *tychange {
            TypeChange::Clear { ref name } => {
                builder.clear(name);
            }
            TypeChange::Add { ref def } => {
                builder.add_def(def)?;
            }
            TypeChange::Select { ref name } => {
                builder.select(name);
            }
            TypeChange::Negate { ref name } => {
                builder.negate(name);
            }
        }
    }
    Ok(builder.build()?)
}

/// Создает матчер переопределения глобов из флагов CLI `-g/--glob` и `--iglob`.
fn globs(
    state: &State,
    low: &LowArgs,
) -> anyhow::Result<ignore::overrides::Override> {
    if low.globs.is_empty() && low.iglobs.is_empty() {
        return Ok(ignore::overrides::Override::empty());
    }
    let mut builder = ignore::overrides::OverrideBuilder::new(&state.cwd);
    // Make all globs case insensitive with --glob-case-insensitive.
    if low.glob_case_insensitive {
        builder.case_insensitive(true).unwrap();
    }
    for glob in low.globs.iter() {
        builder.add(glob)?;
    }
    // This only enables case insensitivity for subsequent globs.
    builder.case_insensitive(true).unwrap();
    for glob in low.iglobs.iter() {
        builder.add(&glob)?;
    }
    Ok(builder.build()?)
}

/// Создает матчер глобов для всех глобов препроцессора (через `--pre-glob`).
fn preprocessor_globs(
    state: &State,
    low: &LowArgs,
) -> anyhow::Result<ignore::overrides::Override> {
    if low.pre_glob.is_empty() {
        return Ok(ignore::overrides::Override::empty());
    }
    let mut builder = ignore::overrides::OverrideBuilder::new(&state.cwd);
    for glob in low.pre_glob.iter() {
        builder.add(glob)?;
    }
    Ok(builder.build()?)
}

/// Определяет, должна ли отслеживаться статистика для этого поиска. Если да,
/// то возвращается объект статистики.
fn stats(low: &LowArgs) -> Option<grep::printer::Stats> {
    if !matches!(low.mode, Mode::Search(_)) {
        return None;
    }
    if low.stats || matches!(low.mode, Mode::Search(SearchMode::JSON)) {
        return Some(grep::printer::Stats::new());
    }
    None
}

/// Извлекает любые спецификации цвета, предоставленные пользователем, и собирает
/// их в одну конфигурацию.
fn take_color_specs(_: &mut State, low: &mut LowArgs) -> ColorSpecs {
    let mut specs = grep::printer::default_color_specs();
    for spec in low.colors.drain(..) {
        specs.push(spec);
    }
    ColorSpecs::new(&specs)
}

/// Извлекает необходимую информацию из низкоуровневых аргументов для создания
/// полной конфигурации гиперссылок.
fn take_hyperlink_config(
    _: &mut State,
    low: &mut LowArgs,
) -> anyhow::Result<grep::printer::HyperlinkConfig> {
    let mut env = grep::printer::HyperlinkEnvironment::new();
    if let Some(hostname) = hostname(low.hostname_bin.as_deref()) {
        log::debug!("found hostname for hyperlink configuration: {hostname}");
        env.host(Some(hostname));
    }
    if let Some(wsl_prefix) = wsl_prefix() {
        log::debug!(
            "found wsl_prefix for hyperlink configuration: {wsl_prefix}"
        );
        env.wsl_prefix(Some(wsl_prefix));
    }
    let fmt = std::mem::take(&mut low.hyperlink_format);
    log::debug!("hyperlink format: {:?}", fmt.to_string());
    Ok(grep::printer::HyperlinkConfig::new(env, fmt))
}

/// Пытается получить текущий рабочий каталог.
///
/// Это в основном просто передает управление стандартной библиотеке, однако
/// такие вещи завершатся ошибкой, если ripgrep находится в каталоге, который
/// больше не существует. Мы пытаемся использовать некоторые механизмы
/// восстановления, такие как запрос переменной окружения PWD, но в противном
/// случае возвращаем ошибку.
fn current_dir() -> anyhow::Result<PathBuf> {
    let err = match std::env::current_dir() {
        Err(err) => err,
        Ok(cwd) => return Ok(cwd),
    };
    if let Some(cwd) = std::env::var_os("PWD") {
        if !cwd.is_empty() {
            return Ok(PathBuf::from(cwd));
        }
    }
    anyhow::bail!(
        "failed to get current working directory: {err}\n\
         did your CWD get deleted?",
    )
}

/// Получает имя хоста, которое должно использоваться везде, где требуется имя хоста.
///
/// В настоящее время это используется только в формате гиперссылок.
///
/// Это работает путем сначала запуска данной бинарной программы (если присутствует
/// и без аргументов) для получения имени хоста после обрезки ведущих и конечных
/// пробелов. Если это не удается по какой-либо причине, то оно возвращается к
/// получению имени хоста через платформо-специфичные средства (например,
/// `gethostname` в Unix).
///
/// Цель `bin` — сделать возможным для конечных пользователей переопределять,
/// как ripgrep определяет имя хоста.
fn hostname(bin: Option<&Path>) -> Option<String> {
    let Some(bin) = bin else { return platform_hostname() };
    let bin = match grep::cli::resolve_binary(bin) {
        Ok(bin) => bin,
        Err(err) => {
            log::debug!(
                "failed to run command '{bin:?}' to get hostname \
                 (falling back to platform hostname): {err}",
            );
            return platform_hostname();
        }
    };
    let mut cmd = std::process::Command::new(&bin);
    cmd.stdin(std::process::Stdio::null());
    let rdr = match grep::cli::CommandReader::new(&mut cmd) {
        Ok(rdr) => rdr,
        Err(err) => {
            log::debug!(
                "failed to spawn command '{bin:?}' to get \
                 hostname (falling back to platform hostname): {err}",
            );
            return platform_hostname();
        }
    };
    let out = match std::io::read_to_string(rdr) {
        Ok(out) => out,
        Err(err) => {
            log::debug!(
                "failed to read output from command '{bin:?}' to get \
                 hostname (falling back to platform hostname): {err}",
            );
            return platform_hostname();
        }
    };
    let hostname = out.trim();
    if hostname.is_empty() {
        log::debug!(
            "output from command '{bin:?}' is empty after trimming \
             leading and trailing whitespace (falling back to \
             platform hostname)",
        );
        return platform_hostname();
    }
    Some(hostname.to_string())
}

/// Пытается получить имя хоста, используя платформо-специфичные процедуры.
///
/// Например, это выполнит `gethostname` в Unix и `GetComputerNameExW` в Windows.
fn platform_hostname() -> Option<String> {
    let hostname_os = match grep::cli::hostname() {
        Ok(x) => x,
        Err(err) => {
            log::debug!("could not get hostname: {}", err);
            return None;
        }
    };
    let Some(hostname) = hostname_os.to_str() else {
        log::debug!(
            "got hostname {:?}, but it's not valid UTF-8",
            hostname_os
        );
        return None;
    };
    Some(hostname.to_string())
}

/// Возвращает значение для переменной `{wslprefix}` в формате гиперссылки.
///
/// Префикс WSL — это что-то вроде общего ресурса/сети, что предназначено для
/// разрешения приложениям Windows открывать файлы, хранящиеся на диске WSL.
///
/// Если имя дистрибутива WSL недоступно, не является валидным UTF-8 или это
/// не выполняется в окружении Unix, то это возвращает None.
///
/// См.: <https://learn.microsoft.com/en-us/windows/wsl/filesystems>
fn wsl_prefix() -> Option<String> {
    if !cfg!(unix) {
        return None;
    }
    let distro_os = std::env::var_os("WSL_DISTRO_NAME")?;
    let Some(distro) = distro_os.to_str() else {
        log::debug!(
            "found WSL_DISTRO_NAME={:?}, but value is not UTF-8",
            distro_os
        );
        return None;
    };
    Some(format!("wsl$/{distro}"))
}

/// Возможно предлагает другой движок регулярных выражений на основе данного
/// сообщения об ошибке.
///
/// Это проверяет ошибку, полученную в результате создания матчера регулярных
/// выражений Rust, и если считается, что она соответствует ошибке синтаксиса,
/// которую может обработать другой движок, то добавляет сообщение с предложением
/// использовать флаг engine.
fn suggest_other_engine(msg: String) -> String {
    if let Some(pcre_msg) = suggest_pcre2(&msg) {
        return pcre_msg;
    }
    msg
}

/// Возможно предлагает PCRE2 на основе данного сообщения об ошибке.
///
/// Проверяет ошибку, полученную в результате создания матчера регулярных
/// выражений Rust, и если считается, что она соответствует ошибке синтаксиса,
/// которую может обработать PCRE2, то добавляет сообщение с предложением
/// использовать -P/--pcre2.
fn suggest_pcre2(msg: &str) -> Option<String> {
    if !cfg!(feature = "pcre2") {
        return None;
    }
    if !msg.contains("backreferences") && !msg.contains("look-around") {
        None
    } else {
        Some(format!(
            "{msg}

Consider enabling PCRE2 with the --pcre2 flag, which can handle backreferences
and look-around.",
        ))
    }
}

/// Возможно предлагает многострочный режим на основе данного сообщения об ошибке.
///
/// Делает немного хакерскую проверку данного сообщения об ошибке, и если
/// похоже, что пользователь попытался ввести буквальный терминатор строки,
/// то он вернет новое сообщение об ошибке с предложением использовать
/// -U/--multiline.
fn suggest_multiline(msg: String) -> String {
    if msg.contains("the literal") && msg.contains("not allowed") {
        format!(
            "{msg}

Consider enabling multiline mode with the --multiline flag (or -U for short).
When multiline mode is enabled, new line characters can be matched.",
        )
    } else {
        msg
    }
}

/// Возможно предлагает флаг `-a/--text`.
fn suggest_text(msg: String) -> String {
    if msg.contains("pattern contains \"\\0\"") {
        format!(
            "{msg}

Consider enabling text mode with the --text flag (or -a for short). Otherwise,
binary detection is enabled and matching a NUL byte is impossible.",
        )
    } else {
        msg
    }
}
