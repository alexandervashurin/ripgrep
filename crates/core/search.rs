/*!
Определяет очень высокий уровень абстракции "поискового рабочего".

Поисковый рабочий управляет точками взаимодействия высокого уровня между
матчером (т.е. какой движок регулярных выражений используется), поисковиком
(т.е. как данные фактически читаются и сопоставляются с использованием
движка регулярных выражений) и принтером. Например, поисковый рабочий —
это место, где происходят такие вещи, как препроцессоры или распаковка.
*/

use std::{io, path::Path};

use {grep::matcher::Matcher, termcolor::WriteColor};

/// Конфигурация для поискового рабочего.
///
/// Среди некоторых других вещей, конфигурация в основном управляет тем,
/// как мы показываем результаты поиска пользователям на очень высоком уровне.
#[derive(Clone, Debug)]
struct Config {
    preprocessor: Option<std::path::PathBuf>,
    preprocessor_globs: ignore::overrides::Override,
    search_zip: bool,
    binary_implicit: grep::searcher::BinaryDetection,
    binary_explicit: grep::searcher::BinaryDetection,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            preprocessor: None,
            preprocessor_globs: ignore::overrides::Override::empty(),
            search_zip: false,
            binary_implicit: grep::searcher::BinaryDetection::none(),
            binary_explicit: grep::searcher::BinaryDetection::none(),
        }
    }
}

/// Построитель для настройки и создания поискового рабочего.
#[derive(Clone, Debug)]
pub(crate) struct SearchWorkerBuilder {
    config: Config,
    command_builder: grep::cli::CommandReaderBuilder,
}

impl Default for SearchWorkerBuilder {
    fn default() -> SearchWorkerBuilder {
        SearchWorkerBuilder::new()
    }
}

impl SearchWorkerBuilder {
    /// Создать новый построитель для настройки и создания поискового рабочего.
    pub(crate) fn new() -> SearchWorkerBuilder {
        let mut command_builder = grep::cli::CommandReaderBuilder::new();
        command_builder.async_stderr(true);

        SearchWorkerBuilder { config: Config::default(), command_builder }
    }

    /// Создать новый поисковый рабочий, используя данные поисковик, матчер
    /// и принтер.
    pub(crate) fn build<W: WriteColor>(
        &self,
        matcher: PatternMatcher,
        searcher: grep::searcher::Searcher,
        printer: Printer<W>,
    ) -> SearchWorker<W> {
        let config = self.config.clone();
        let command_builder = self.command_builder.clone();
        let decomp_builder = config.search_zip.then(|| {
            let mut decomp_builder =
                grep::cli::DecompressionReaderBuilder::new();
            decomp_builder.async_stderr(true);
            decomp_builder
        });
        SearchWorker {
            config,
            command_builder,
            decomp_builder,
            matcher,
            searcher,
            printer,
        }
    }

    /// Установить путь к команде препроцессора.
    ///
    /// Когда это установлено, вместо прямого поиска файлов данная команда
    /// будет запущена с путем к файлу в качестве первого аргумента, и вывод
    /// этой команды будет искаться вместо этого.
    pub(crate) fn preprocessor(
        &mut self,
        cmd: Option<std::path::PathBuf>,
    ) -> anyhow::Result<&mut SearchWorkerBuilder> {
        if let Some(ref prog) = cmd {
            let bin = grep::cli::resolve_binary(prog)?;
            self.config.preprocessor = Some(bin);
        } else {
            self.config.preprocessor = None;
        }
        Ok(self)
    }

    /// Установить glob-шаблоны для определения, какие файлы должны быть
    /// пропущены через препроцессор. По умолчанию, без glob-шаблонов и
    /// указанного препроцессора, каждый файл пропускается через препроцессор.
    pub(crate) fn preprocessor_globs(
        &mut self,
        globs: ignore::overrides::Override,
    ) -> &mut SearchWorkerBuilder {
        self.config.preprocessor_globs = globs;
        self
    }

    /// Включить распаковку и поиск распространенных сжатых файлов.
    ///
    /// Когда включено, если определенный путь к файлу распознан как сжатый
    /// файл, то он распаковывается перед поиском.
    ///
    /// Обратите внимание, что если установлена команда препроцессора, то
    /// она переопределяет эту настройку.
    pub(crate) fn search_zip(
        &mut self,
        yes: bool,
    ) -> &mut SearchWorkerBuilder {
        self.config.search_zip = yes;
        self
    }

    /// Установить обнаружение двоичных файлов, которое должно использоваться
    /// при поиске файлов, найденных через рекурсивный поиск по каталогу.
    ///
    /// Обычно это обнаружение двоичных файлов может быть
    /// `grep::searcher::BinaryDetection::quit`, если мы хотим полностью
    /// пропустить двоичные файлы.
    ///
    /// По умолчанию обнаружение двоичных файлов не выполняется.
    pub(crate) fn binary_detection_implicit(
        &mut self,
        detection: grep::searcher::BinaryDetection,
    ) -> &mut SearchWorkerBuilder {
        self.config.binary_implicit = detection;
        self
    }

    /// Установить обнаружение двоичных файлов, которое должно использоваться
    /// при поиске файлов, явно предоставленных конечным пользователем.
    ///
    /// Обычно это обнаружение двоичных файлов НЕ должно быть
    /// `grep::searcher::BinaryDetection::quit`, поскольку мы никогда не
    /// хотим автоматически фильтровать файлы, предоставленные конечным
    /// пользователем.
    ///
    /// По умолчанию обнаружение двоичных файлов не выполняется.
    pub(crate) fn binary_detection_explicit(
        &mut self,
        detection: grep::searcher::BinaryDetection,
    ) -> &mut SearchWorkerBuilder {
        self.config.binary_explicit = detection;
        self
    }
}

/// Результат выполнения поиска.
///
/// Вообще говоря, "результат" поиска отправляется в принтер, который записывает
/// результаты в базовый писатель, такой как stdout или файл. Однако каждый
/// поиск также имеет некоторую агрегированную статистику или метаданные,
/// которые могут быть полезны подпрограммам высокого уровня.
#[derive(Clone, Debug, Default)]
pub(crate) struct SearchResult {
    has_match: bool,
    stats: Option<grep::printer::Stats>,
}

impl SearchResult {
    /// Нашел ли поиск совпадение или нет.
    pub(crate) fn has_match(&self) -> bool {
        self.has_match
    }

    /// Вернуть агрегированную статистику поиска для одного поиска, если
    /// доступна.
    ///
    /// Вычисление статистики может быть дорогим, поэтому они присутствуют
    /// только если явно включены в принтере, предоставленном вызывающим.
    pub(crate) fn stats(&self) -> Option<&grep::printer::Stats> {
        self.stats.as_ref()
    }
}

/// Матчер шаблонов, используемый поисковым рабочим.
#[derive(Clone, Debug)]
pub(crate) enum PatternMatcher {
    RustRegex(grep::regex::RegexMatcher),
    #[cfg(feature = "pcre2")]
    PCRE2(grep::pcre2::RegexMatcher),
}

/// Принтер, используемый поисковым рабочим.
///
/// Параметр типа `W` относится к типу базового писателя.
#[derive(Clone, Debug)]
pub(crate) enum Printer<W> {
    /// Использовать стандартный принтер, который поддерживает классический
    /// формат, подобный grep.
    Standard(grep::printer::Standard<W>),
    /// Использовать принтер сводки, который поддерживает агрегированные
    /// отображения результатов поиска.
    Summary(grep::printer::Summary<W>),
    /// JSON принтер, который выводит результаты в формате JSON Lines.
    JSON(grep::printer::JSON<W>),
}

impl<W: WriteColor> Printer<W> {
    /// Вернуть изменяемую ссылку на базовый писатель принтера.
    pub(crate) fn get_mut(&mut self) -> &mut W {
        match *self {
            Printer::Standard(ref mut p) => p.get_mut(),
            Printer::Summary(ref mut p) => p.get_mut(),
            Printer::JSON(ref mut p) => p.get_mut(),
        }
    }
}

/// Рабочий для выполнения поисков.
///
/// Предполагается, что один рабочий выполняет много поисков, и обычно
/// предполагается, что он будет использоваться из одного потока. При поиске
/// с использованием нескольких потоков лучше создавать нового рабочего для
/// каждого потока.
#[derive(Clone, Debug)]
pub(crate) struct SearchWorker<W> {
    config: Config,
    command_builder: grep::cli::CommandReaderBuilder,
    /// Это `None`, когда `search_zip` не включен, так как в этом случае он
    /// никогда не может быть использован. Мы делаем это, потому что построение
    /// читателя иногда может выполнять нетривиальную работу (например,
    /// разрешение путей к бинарным файлам распаковки в Windows).
    decomp_builder: Option<grep::cli::DecompressionReaderBuilder>,
    matcher: PatternMatcher,
    searcher: grep::searcher::Searcher,
    printer: Printer<W>,
}

impl<W: WriteColor> SearchWorker<W> {
    /// Выполнить поиск по данному стогу сена.
    pub(crate) fn search(
        &mut self,
        haystack: &crate::haystack::Haystack,
    ) -> io::Result<SearchResult> {
        let bin = if haystack.is_explicit() {
            self.config.binary_explicit.clone()
        } else {
            self.config.binary_implicit.clone()
        };
        let path = haystack.path();
        log::trace!("{}: обнаружение двоичных файлов: {:?}", path.display(), bin);

        self.searcher.set_binary_detection(bin);
        if haystack.is_stdin() {
            self.search_reader(path, &mut io::stdin().lock())
        } else if self.should_preprocess(path) {
            self.search_preprocessor(path)
        } else if self.should_decompress(path) {
            self.search_decompress(path)
        } else {
            self.search_path(path)
        }
    }

    /// Вернуть изменяемую ссылку на базовый принтер.
    pub(crate) fn printer(&mut self) -> &mut Printer<W> {
        &mut self.printer
    }

    /// Возвращает true тогда и только тогда, когда данный путь к файлу
    /// должен быть распакован перед поиском.
    fn should_decompress(&self, path: &Path) -> bool {
        self.decomp_builder.as_ref().is_some_and(|decomp_builder| {
            decomp_builder.get_matcher().has_command(path)
        })
    }

    /// Возвращает true тогда и только тогда, когда данный путь к файлу
    /// должен быть пропущен через препроцессор.
    fn should_preprocess(&self, path: &Path) -> bool {
        if !self.config.preprocessor.is_some() {
            return false;
        }
        if self.config.preprocessor_globs.is_empty() {
            return true;
        }
        !self.config.preprocessor_globs.matched(path, false).is_ignore()
    }

    /// Искать данный путь к файлу, сначала запрашивая у препроцессора данные
    /// для поиска вместо прямого открытия пути.
    fn search_preprocessor(
        &mut self,
        path: &Path,
    ) -> io::Result<SearchResult> {
        use std::{fs::File, process::Stdio};

        let bin = self.config.preprocessor.as_ref().unwrap();
        let mut cmd = std::process::Command::new(bin);
        cmd.arg(path).stdin(Stdio::from(File::open(path)?));

        let mut rdr = self.command_builder.build(&mut cmd).map_err(|err| {
            io::Error::new(
                io::ErrorKind::Other,
                format!(
                    "команда препроцессора не смогла запуститься: '{cmd:?}': {err}",
                ),
            )
        })?;
        let result = self.search_reader(path, &mut rdr).map_err(|err| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("команда препроцессора не удалась: '{cmd:?}': {err}"),
            )
        });
        let close_result = rdr.close();
        let search_result = result?;
        close_result?;
        Ok(search_result)
    }

    /// Попытаться распаковать данные по данному пути к файлу и искать
    /// результат. Если данный путь к файлу не распознан как сжатый файл,
    /// то искать его без выполнения какой-либо распаковки.
    fn search_decompress(&mut self, path: &Path) -> io::Result<SearchResult> {
        let Some(ref decomp_builder) = self.decomp_builder else {
            return self.search_path(path);
        };
        let mut rdr = decomp_builder.build(path)?;
        let result = self.search_reader(path, &mut rdr);
        let close_result = rdr.close();
        let search_result = result?;
        close_result?;
        Ok(search_result)
    }

    /// Искать содержимое данного пути к файлу.
    fn search_path(&mut self, path: &Path) -> io::Result<SearchResult> {
        use self::PatternMatcher::*;

        let (searcher, printer) = (&mut self.searcher, &mut self.printer);
        match self.matcher {
            RustRegex(ref m) => search_path(m, searcher, printer, path),
            #[cfg(feature = "pcre2")]
            PCRE2(ref m) => search_path(m, searcher, printer, path),
        }
    }

    /// Выполняет поиск по данному читателю, который может или не может
    /// соответствовать напрямую содержимому данного пути к файлу. Вместо
    /// этого читатель может фактически заставить искать что-то другое
    /// (например, когда установлен препроцессор или когда включена
    /// распаковка). В этих случаях путь к файлу используется только для
    /// визуальных целей.
    ///
    /// Вообще говоря, этот метод следует использовать только тогда, когда
    /// нет другого выбора. Поиск через `search_path` предоставляет больше
    /// возможностей для оптимизаций (например, отображения памяти).
    fn search_reader<R: io::Read>(
        &mut self,
        path: &Path,
        rdr: &mut R,
    ) -> io::Result<SearchResult> {
        use self::PatternMatcher::*;

        let (searcher, printer) = (&mut self.searcher, &mut self.printer);
        match self.matcher {
            RustRegex(ref m) => search_reader(m, searcher, printer, path, rdr),
            #[cfg(feature = "pcre2")]
            PCRE2(ref m) => search_reader(m, searcher, printer, path, rdr),
        }
    }
}

/// Искать содержимое данного пути к файлу, используя данные матчер,
/// поисковик и принтер.
fn search_path<M: Matcher, W: WriteColor>(
    matcher: M,
    searcher: &mut grep::searcher::Searcher,
    printer: &mut Printer<W>,
    path: &Path,
) -> io::Result<SearchResult> {
    match *printer {
        Printer::Standard(ref mut p) => {
            let mut sink = p.sink_with_path(&matcher, path);
            searcher.search_path(&matcher, path, &mut sink)?;
            Ok(SearchResult {
                has_match: sink.has_match(),
                stats: sink.stats().map(|s| s.clone()),
            })
        }
        Printer::Summary(ref mut p) => {
            let mut sink = p.sink_with_path(&matcher, path);
            searcher.search_path(&matcher, path, &mut sink)?;
            Ok(SearchResult {
                has_match: sink.has_match(),
                stats: sink.stats().map(|s| s.clone()),
            })
        }
        Printer::JSON(ref mut p) => {
            let mut sink = p.sink_with_path(&matcher, path);
            searcher.search_path(&matcher, path, &mut sink)?;
            Ok(SearchResult {
                has_match: sink.has_match(),
                stats: Some(sink.stats().clone()),
            })
        }
    }
}

/// Искать содержимое данного читателя, используя данные матчер, поисковик
/// и принтер.
fn search_reader<M: Matcher, R: io::Read, W: WriteColor>(
    matcher: M,
    searcher: &mut grep::searcher::Searcher,
    printer: &mut Printer<W>,
    path: &Path,
    mut rdr: R,
) -> io::Result<SearchResult> {
    match *printer {
        Printer::Standard(ref mut p) => {
            let mut sink = p.sink_with_path(&matcher, path);
            searcher.search_reader(&matcher, &mut rdr, &mut sink)?;
            Ok(SearchResult {
                has_match: sink.has_match(),
                stats: sink.stats().map(|s| s.clone()),
            })
        }
        Printer::Summary(ref mut p) => {
            let mut sink = p.sink_with_path(&matcher, path);
            searcher.search_reader(&matcher, &mut rdr, &mut sink)?;
            Ok(SearchResult {
                has_match: sink.has_match(),
                stats: sink.stats().map(|s| s.clone()),
            })
        }
        Printer::JSON(ref mut p) => {
            let mut sink = p.sink_with_path(&matcher, path);
            searcher.search_reader(&matcher, &mut rdr, &mut sink)?;
            Ok(SearchResult {
                has_match: sink.has_match(),
                stats: Some(sink.stats().clone()),
            })
        }
    }
}
