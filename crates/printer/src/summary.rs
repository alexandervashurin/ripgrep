use std::{
    cell::RefCell,
    io::{self, Write},
    path::Path,
    sync::Arc,
    time::Instant,
};

use {
    grep_matcher::Matcher,
    grep_searcher::{Searcher, Sink, SinkError, SinkFinish, SinkMatch},
    termcolor::{ColorSpec, NoColor, WriteColor},
};

use crate::{
    color::ColorSpecs,
    counter::CounterWriter,
    hyperlink::{self, HyperlinkConfig},
    stats::Stats,
    util::{PrinterPath, find_iter_at_in_context},
};

/// Конфигурация для принтера сводки.
///
/// Управляется через SummaryBuilder и затем используется реальной
/// реализацией. После создания принтера конфигурация замораживается
/// и не может быть изменена.
#[derive(Debug, Clone)]
struct Config {
    kind: SummaryKind,
    colors: ColorSpecs,
    hyperlink: HyperlinkConfig,
    stats: bool,
    path: bool,
    exclude_zero: bool,
    separator_field: Arc<Vec<u8>>,
    separator_path: Option<u8>,
    path_terminator: Option<u8>,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            kind: SummaryKind::Count,
            colors: ColorSpecs::default(),
            hyperlink: HyperlinkConfig::default(),
            stats: false,
            path: true,
            exclude_zero: true,
            separator_field: Arc::new(b":".to_vec()),
            separator_path: None,
            path_terminator: None,
        }
    }
}

/// Тип вывода сводки (если есть) для печати.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SummaryKind {
    /// Показать только количество общего числа совпадений (считая каждую
    /// строку максимум один раз), которые были найдены.
    ///
    /// Если настройка `path` включена, то количество предваряется
    /// соответствующим путём к файлу.
    Count,
    /// Показать только количество общего числа совпадений (считая
    /// возможно много совпадений в каждой строке), которые были найдены.
    ///
    /// Если настройка `path` включена, то количество предваряется
    /// соответствующим путём к файлу.
    CountMatches,
    /// Показать только путь к файлу, если и только если было найдено
    /// совпадение.
    ///
    /// Это игнорирует настройку `path` и всегда показывает путь к файлу.
    /// Если путь к файлу не предоставлен, то поиск немедленно остановится
    /// и вернёт ошибку.
    PathWithMatch,
    /// Показать только путь к файлу, если и только если было найдено
    /// совпадение.
    ///
    /// Это игнорирует настройку `path` и всегда показывает путь к файлу.
    /// Если путь к файлу не предоставлен, то поиск немедленно остановится
    /// и вернёт ошибку.
    PathWithoutMatch,
    /// Не показывать никакого вывода и остановить поиск, как только
    /// найдено совпадение.
    ///
    /// Обратите внимание, что если `stats` включён, то поиск продолжается
    /// для вычисления статистики.
    QuietWithMatch,
    /// Не показывать никакого вывода и остановить поиск, как только
    /// найден файл без совпадений.
    ///
    /// Обратите внимание, что если `stats` включён, то поиск продолжается
    /// для вычисления статистики.
    QuietWithoutMatch,
}

impl SummaryKind {
    /// Возвращает true тогда и только тогда, когда этот режим вывода
    /// требует путь к файлу.
    ///
    /// Когда режим вывода требует путь к файлу, то принтер сводки будет
    /// сообщать об ошибке в начале каждого поиска, в котором отсутствует
    /// путь к файлу.
    fn requires_path(&self) -> bool {
        use self::SummaryKind::*;

        match *self {
            PathWithMatch | PathWithoutMatch => true,
            Count | CountMatches | QuietWithMatch | QuietWithoutMatch => false,
        }
    }

    /// Возвращает true тогда и только тогда, когда этот режим вывода
    /// требует вычисления статистики, независимо от того, включена она
    /// или нет.
    fn requires_stats(&self) -> bool {
        use self::SummaryKind::*;

        match *self {
            CountMatches => true,
            Count | PathWithMatch | PathWithoutMatch | QuietWithMatch
            | QuietWithoutMatch => false,
        }
    }

    /// Возвращает true тогда и только тогда, когда принтер, использующий
    /// этот режим вывода, может завершить после первого увиденного
    /// совпадения.
    fn quit_early(&self) -> bool {
        use self::SummaryKind::*;

        match *self {
            PathWithMatch | QuietWithMatch => true,
            Count | CountMatches | PathWithoutMatch | QuietWithoutMatch => {
                false
            }
        }
    }
}

/// Билдер для принтера сводки.
///
/// Билдер позволяет настроить поведение принтера. Принтер сводки имеет
/// меньше параметров конфигурации, чем стандартный принтер, потому что
/// он предназначен для создания агрегированного вывода об одном поиске
/// (обычно всего одна строка) вместо вывода для каждого совпадения.
///
/// После создания принтера `Summary` его конфигурация не может быть
/// изменена.
#[derive(Clone, Debug)]
pub struct SummaryBuilder {
    config: Config,
}

impl SummaryBuilder {
    /// Создать новый билдер для настройки принтера сводки.
    pub fn new() -> SummaryBuilder {
        SummaryBuilder { config: Config::default() }
    }

    /// Создать принтер, используя любую реализацию `termcolor::WriteColor`.
    ///
    /// Реализация `WriteColor`, используемая здесь, управляет тем, используются
    /// ли цвета или нет, когда цвета были настроены с помощью метода
    /// `color_specs`.
    ///
    /// Для максимальной переносимости вызывающие стороны должны обычно
    /// использовать либо `termcolor::StandardStream`, либо
    /// `termcolor::BufferedStandardStream` где это уместно, что автоматически
    /// включит цвета в Windows, когда это возможно.
    ///
    /// Однако вызывающие стороны также могут предоставить произвольный
    /// writer, используя обёртки `termcolor::Ansi` или `termcolor::NoColor`,
    /// которые всегда включают цвета через ANSI-escape последовательности или
    /// всегда отключают цвета соответственно.
    ///
    /// Для удобства вызывающие стороны могут использовать `build_no_color` для
    /// автоматического выбора обёртки `termcolor::NoColor`, чтобы избежать
    /// необходимости явного импорта из `termcolor`.
    pub fn build<W: WriteColor>(&self, wtr: W) -> Summary<W> {
        Summary {
            config: self.config.clone(),
            wtr: RefCell::new(CounterWriter::new(wtr)),
        }
    }

    /// Создать принтер из любой реализации `io::Write` и никогда не выводить
    /// какие-либо цвета, независимо от настроек спецификаций пользовательских
    /// цветов.
    ///
    /// Это вспомогательная функция для
    /// `SummaryBuilder::build(termcolor::NoColor::new(wtr))`.
    pub fn build_no_color<W: io::Write>(&self, wtr: W) -> Summary<NoColor<W>> {
        self.build(NoColor::new(wtr))
    }

    /// Установить режим вывода для этого принтера.
    ///
    /// Режим вывода управляет тем, как печатаются агрегированные результаты
    /// поиска.
    ///
    /// По умолчанию этот принтер использует режим `Count`.
    pub fn kind(&mut self, kind: SummaryKind) -> &mut SummaryBuilder {
        self.config.kind = kind;
        self
    }

    /// Установить спецификации пользовательских цветов для использования
    /// при раскраске в этом принтере.
    ///
    /// [`UserColorSpec`](crate::UserColorSpec) может быть создан из строки
    /// в соответствии с форматом спецификации цвета. См. документацию типа
    /// `UserColorSpec` для получения более подробной информации о формате.
    /// [`ColorSpecs`] может быть затем сгенерирован из нуля или более
    /// `UserColorSpec`.
    ///
    /// Независимо от предоставленных здесь спецификаций цвета, фактически
    /// используется ли цвет или нет, определяется реализацией `WriteColor`,
    /// переданной в `build`. Например, если в `build` передан
    /// `termcolor::NoColor`, то никакой цвет никогда не будет напечатан,
    /// независимо от предоставленных здесь спецификаций цвета.
    ///
    /// Это полностью переопределяет любые предыдущие спецификации цвета.
    /// Это не добавляет к каким-либо ранее предоставленным спецификациям
    /// цвета в этом билдере.
    ///
    /// Спецификации цвета по умолчанию не предоставляют никакой стилизации.
    pub fn color_specs(&mut self, specs: ColorSpecs) -> &mut SummaryBuilder {
        self.config.colors = specs;
        self
    }

    /// Установить конфигурацию для использования с гиперссылками,
    /// выводимыми этим принтером.
    ///
    /// Независимо от предоставленного здесь формата гиперссылок, фактически
    /// используются ли гиперссылки или нет, определяется реализацией
    /// `WriteColor`, переданной в `build`. Например, если в `build` передан
    /// `termcolor::NoColor`, то никакие гиперссылки никогда не будут
    /// напечатаны, независимо от предоставленного здесь формата.
    ///
    /// Это полностью переопределяет любой предыдущий формат гиперссылок.
    ///
    /// Конфигурация по умолчанию приводит к тому, что никакие гиперссылки
    /// не выводятся.
    pub fn hyperlink(
        &mut self,
        config: HyperlinkConfig,
    ) -> &mut SummaryBuilder {
        self.config.hyperlink = config;
        self
    }

    /// Включить сбор различной агрегированной статистики.
    ///
    /// Когда эта опция включена (по умолчанию она отключена), статистика
    /// будет собираться для всех использований принтера `Summary`,
    /// возвращённого методом `build`, включая, но не ограничиваясь,
    /// общим количеством совпадений, общим количеством байт, подвергшихся
    /// поиску, и общим количеством напечатанных байт.
    ///
    /// Агрегированную статистику можно получить через метод
    /// [`SummarySink::stats`] sink'а.
    ///
    /// Когда эта опция включена, этому принтеру может потребоваться
    /// выполнить дополнительную работу для вычисления определённой
    /// статистики, что может привести к увеличению времени поиска.
    /// Например, в режиме `QuietWithMatch` поиск может завершить после
    /// нахождения первого совпадения, но если включён `stats`, то поиск
    /// продолжится после первого совпадения для вычисления статистики.
    ///
    /// Полное описание доступной статистики см. в [`Stats`].
    ///
    /// Обратите внимание, что некоторые режимы вывода, такие как
    /// `CountMatches`, автоматически включают эту опцию, даже если она
    /// была явно отключена.
    pub fn stats(&mut self, yes: bool) -> &mut SummaryBuilder {
        self.config.stats = yes;
        self
    }

    /// Когда включено, если путь был передан принтеру, то он отображается
    /// в выводе (либо как заголовок, либо как префикс к каждой строке
    /// совпадения). Когда отключено, то никакие пути никогда не включаются
    /// в вывод, даже если путь предоставлен принтеру.
    ///
    /// Эта настройка не имеет эффекта в режимах `PathWithMatch` и
    /// `PathWithoutMatch`.
    ///
    /// По умолчанию включено.
    pub fn path(&mut self, yes: bool) -> &mut SummaryBuilder {
        self.config.path = yes;
        self
    }

    /// Исключить результаты сводки, связанные с количеством, без
    /// совпадений.
    ///
    /// Когда включено и режим либо `Count`, либо `CountMatches`, то
    /// результаты не печатаются, если совпадений не найдено. В противном
    /// случае каждый поиск печатает результат с возможным количеством
    /// совпадений `0`.
    ///
    /// По умолчанию включено.
    pub fn exclude_zero(&mut self, yes: bool) -> &mut SummaryBuilder {
        self.config.exclude_zero = yes;
        self
    }

    /// Установить разделитель, используемый между полями для режимов
    /// `Count` и `CountMatches`.
    ///
    /// По умолчанию установлено `:`.
    pub fn separator_field(&mut self, sep: Vec<u8>) -> &mut SummaryBuilder {
        self.config.separator_field = Arc::new(sep);
        self
    }

    /// Установить разделитель путей, используемый при печати путей к
    /// файлам.
    ///
    /// Обычно печать выполняется путём вывода пути к файлу как есть. Однако
    /// эта настройка предоставляет возможность использовать другой
    /// разделитель путей от того, который настроен в текущей среде.
    ///
    /// Типичное использование этой опции — позволить пользователям cygwin
    /// в Windows установить разделитель путей в `/` вместо использования
    /// системного `\` по умолчанию.
    ///
    /// По умолчанию отключено.
    pub fn separator_path(&mut self, sep: Option<u8>) -> &mut SummaryBuilder {
        self.config.separator_path = sep;
        self
    }

    /// Установить терминатор путей.
    ///
    /// Терминатор путей — это байт, который печатается после каждого пути
    /// к файлу, выводимого этим принтером.
    ///
    /// Если терминатор путей не установлен (по умолчанию), то пути
    /// завершаются либо символами новой строки, либо настроенным
    /// разделителем полей.
    pub fn path_terminator(
        &mut self,
        terminator: Option<u8>,
    ) -> &mut SummaryBuilder {
        self.config.path_terminator = terminator;
        self
    }
}

/// Принтер сводки, который выводит агрегированные результаты поиска.
///
/// Агрегированные результаты обычно соответствуют путям к файлам и/или
/// количеству найденных совпадений.
///
/// Принтер по умолчанию можно создать с помощью одного из конструкторов
/// `Summary::new` или `Summary::new_no_color`. Однако существует
/// несколько опций, настраивающих вывод этого принтера. Эти опции могут
/// быть настроены с помощью [`SummaryBuilder`].
///
/// Этот тип параметризован над `W`, который представляет любую реализацию
/// трейта `termcolor::WriteColor`.
#[derive(Clone, Debug)]
pub struct Summary<W> {
    config: Config,
    wtr: RefCell<CounterWriter<W>>,
}

impl<W: WriteColor> Summary<W> {
    /// Создать принтер сводки с конфигурацией по умолчанию, который
    /// записывает совпадения в указанный writer.
    ///
    /// Writer должен быть реализацией `termcolor::WriteColor`, а не просто
    /// реализацией `io::Write`. Для использования обычной реализации
    /// `io::Write` (одновременно жертвуя цветами) используйте конструктор
    /// `new_no_color`.
    ///
    /// Конфигурация по умолчанию использует режим сводки `Count`.
    pub fn new(wtr: W) -> Summary<W> {
        SummaryBuilder::new().build(wtr)
    }
}

impl<W: io::Write> Summary<NoColor<W>> {
    /// Создать принтер сводки с конфигурацией по умолчанию, который
    /// записывает совпадения в указанный writer.
    ///
    /// Writer может быть любой реализацией `io::Write`. С этим конструктором
    /// принтер никогда не будет выводить цвета.
    ///
    /// Конфигурация по умолчанию использует режим сводки `Count`.
    pub fn new_no_color(wtr: W) -> Summary<NoColor<W>> {
        SummaryBuilder::new().build_no_color(wtr)
    }
}

impl<W: WriteColor> Summary<W> {
    /// Создать реализацию `Sink` для принтера сводки.
    ///
    /// Это не связывает принтер с путём к файлу, что означает, что эта
    /// реализация никогда не будет печатать путь к файлу. Если режим
    /// вывода этого принтера сводки не имеет смысла без пути к файлу
    /// (такого как `PathWithMatch` или `PathWithoutMatch`), то любые
    /// поиски, выполненные с использованием этого sink, немедленно
    /// завершатся с ошибкой.
    pub fn sink<'s, M: Matcher>(
        &'s mut self,
        matcher: M,
    ) -> SummarySink<'static, 's, M, W> {
        let interpolator =
            hyperlink::Interpolator::new(&self.config.hyperlink);
        let stats = if self.config.stats || self.config.kind.requires_stats() {
            Some(Stats::new())
        } else {
            None
        };
        SummarySink {
            matcher,
            summary: self,
            interpolator,
            path: None,
            start_time: Instant::now(),
            match_count: 0,
            binary_byte_offset: None,
            stats,
        }
    }

    /// Создать реализацию `Sink`, связанную с путём к файлу.
    ///
    /// Когда принтер связан с путём, то он может, в зависимости от своей
    /// конфигурации, печатать путь.
    pub fn sink_with_path<'p, 's, M, P>(
        &'s mut self,
        matcher: M,
        path: &'p P,
    ) -> SummarySink<'p, 's, M, W>
    where
        M: Matcher,
        P: ?Sized + AsRef<Path>,
    {
        if !self.config.path && !self.config.kind.requires_path() {
            return self.sink(matcher);
        }
        let interpolator =
            hyperlink::Interpolator::new(&self.config.hyperlink);
        let stats = if self.config.stats || self.config.kind.requires_stats() {
            Some(Stats::new())
        } else {
            None
        };
        let ppath = PrinterPath::new(path.as_ref())
            .with_separator(self.config.separator_path);
        SummarySink {
            matcher,
            summary: self,
            interpolator,
            path: Some(ppath),
            start_time: Instant::now(),
            match_count: 0,
            binary_byte_offset: None,
            stats,
        }
    }
}

impl<W> Summary<W> {
    /// Возвращает true тогда и только тогда, когда этот принтер записал
    /// хотя бы один байт в нижележащий writer во время любого из
    /// предыдущих поисков.
    pub fn has_written(&self) -> bool {
        self.wtr.borrow().total_count() > 0
    }

    /// Вернуть изменяемую ссылку на нижележащий writer.
    pub fn get_mut(&mut self) -> &mut W {
        self.wtr.get_mut().get_mut()
    }

    /// Уничтожить этот принтер и вернуть обратно владение нижележащим
    /// writer.
    pub fn into_inner(self) -> W {
        self.wtr.into_inner().into_inner()
    }
}

/// Реализация `Sink`, связанная с matcher и необязательным путём к файлу
/// для принтера сводки.
///
/// Этот тип параметризован несколькими параметрами типа:
///
/// * `'p` относится к времени жизни пути к файлу, если он предоставлен.
/// Когда путь к файлу не предоставлен, то это `'static`.
/// * `'s` относится к времени жизни принтера [`Summary`], который этот
/// тип заимствует.
/// * `M` относится к типу matcher, используемого
/// `grep_searcher::Searcher`, который сообщает результаты этому sink.
/// * `W` относится к нижележащему writer, в который этот принтер записывает
/// свой вывод.
#[derive(Debug)]
pub struct SummarySink<'p, 's, M: Matcher, W> {
    matcher: M,
    summary: &'s mut Summary<W>,
    interpolator: hyperlink::Interpolator,
    path: Option<PrinterPath<'p>>,
    start_time: Instant,
    match_count: u64,
    binary_byte_offset: Option<u64>,
    stats: Option<Stats>,
}

impl<'p, 's, M: Matcher, W: WriteColor> SummarySink<'p, 's, M, W> {
    /// Возвращает true тогда и только тогда, когда этот принтер получил
    /// совпадение в предыдущем поиске.
    ///
    /// Это не зависит от результата поисков до предыдущего поиска.
    pub fn has_match(&self) -> bool {
        match self.summary.config.kind {
            SummaryKind::PathWithoutMatch | SummaryKind::QuietWithoutMatch => {
                self.match_count == 0
            }
            _ => self.match_count > 0,
        }
    }

    /// Если бинарные данные были найдены в предыдущем поиске, это возвращает
    /// смещение, в котором бинарные данные были впервые обнаружены.
    ///
    /// Возвращаемое смещение является абсолютным смещением относительно
    /// всего набора байт, подвергшихся поиску.
    ///
    /// Это не зависит от результата поисков до предыдущего поиска.
    /// Например, если поиск до предыдущего поиска нашёл бинарные данные,
    /// но предыдущий поиск не нашёл бинарных данных, то это вернёт `None`.
    pub fn binary_byte_offset(&self) -> Option<u64> {
        self.binary_byte_offset
    }

    /// Вернуть ссылку на статистику, созданную принтером для всех поисков,
    /// выполненных на этом sink.
    ///
    /// Это возвращает статистику только если она была запрошена через
    /// конфигурацию [`SummaryBuilder`].
    pub fn stats(&self) -> Option<&Stats> {
        self.stats.as_ref()
    }

    /// Возвращает true тогда и только тогда, когда searcher может сообщать
    /// о совпадениях на нескольких строках.
    ///
    /// Обратите внимание, что это не просто возвращает, находится ли
    /// searcher в многострочном режиме, но также проверяет, может ли
    /// matcher сопоставлять несколько строк. Если нет, то нам не нужна
    /// многострочная обработка, даже если в searcher включён многострочный
    /// режим.
    fn multi_line(&self, searcher: &Searcher) -> bool {
        searcher.multi_line_with_matcher(&self.matcher)
    }

    /// Если у этого принтера связан путь к файлу, то это запишет этот путь
    /// в нижележащий writer, за которым следует терминатор строки.
    /// (Если установлен терминатор путей, то он используется вместо
    /// терминатора строки.)
    fn write_path_line(&mut self, searcher: &Searcher) -> io::Result<()> {
        if self.path.is_some() {
            self.write_path()?;
            if let Some(term) = self.summary.config.path_terminator {
                self.write(&[term])?;
            } else {
                self.write_line_term(searcher)?;
            }
        }
        Ok(())
    }

    /// Если у этого принтера связан путь к файлу, то это запишет этот путь
    /// в нижележащий writer, за которым следует разделитель полей.
    /// (Если установлен терминатор путей, то он используется вместо
    /// разделителя полей.)
    fn write_path_field(&mut self) -> io::Result<()> {
        if self.path.is_some() {
            self.write_path()?;
            if let Some(term) = self.summary.config.path_terminator {
                self.write(&[term])?;
            } else {
                self.write(&self.summary.config.separator_field)?;
            }
        }
        Ok(())
    }

    /// Если у этого принтера связан путь к файлу, то это запишет этот путь
    /// в нижележащий writer в соответствующем стиле (цвет и гиперссылка).
    fn write_path(&mut self) -> io::Result<()> {
        if self.path.is_some() {
            let status = self.start_hyperlink()?;
            self.write_spec(
                self.summary.config.colors.path(),
                self.path.as_ref().unwrap().as_bytes(),
            )?;
            self.end_hyperlink(status)?;
        }
        Ok(())
    }

    /// Запускает span гиперссылки, когда это применимо.
    fn start_hyperlink(
        &mut self,
    ) -> io::Result<hyperlink::InterpolatorStatus> {
        let Some(hyperpath) =
            self.path.as_ref().and_then(|p| p.as_hyperlink())
        else {
            return Ok(hyperlink::InterpolatorStatus::inactive());
        };
        let values = hyperlink::Values::new(hyperpath);
        self.interpolator.begin(&values, &mut *self.summary.wtr.borrow_mut())
    }

    fn end_hyperlink(
        &self,
        status: hyperlink::InterpolatorStatus,
    ) -> io::Result<()> {
        self.interpolator.finish(status, &mut *self.summary.wtr.borrow_mut())
    }

    /// Записать терминатор строки, настроенный в данном searcher.
    fn write_line_term(&self, searcher: &Searcher) -> io::Result<()> {
        self.write(searcher.line_terminator().as_bytes())
    }

    /// Записать данные байты с использованием данного стиля.
    fn write_spec(&self, spec: &ColorSpec, buf: &[u8]) -> io::Result<()> {
        self.summary.wtr.borrow_mut().set_color(spec)?;
        self.write(buf)?;
        self.summary.wtr.borrow_mut().reset()?;
        Ok(())
    }

    /// Записать все данные байты.
    fn write(&self, buf: &[u8]) -> io::Result<()> {
        self.summary.wtr.borrow_mut().write_all(buf)
    }
}

impl<'p, 's, M: Matcher, W: WriteColor> Sink for SummarySink<'p, 's, M, W> {
    type Error = io::Error;

    fn matched(
        &mut self,
        searcher: &Searcher,
        mat: &SinkMatch<'_>,
    ) -> Result<bool, io::Error> {
        let is_multi_line = self.multi_line(searcher);
        let sink_match_count = if self.stats.is_none() && !is_multi_line {
            1
        } else {
            // Это даёт нам столько байт, сколько может предложить searcher.
            // Это не гарантирует, что будет содержаться необходимый контекст
            // для корректного определения совпадений (из-за look-around),
            // но на практике это так.
            let buf = mat.buffer();
            let range = mat.bytes_range_in_buffer();
            let mut count = 0;
            find_iter_at_in_context(
                searcher,
                &self.matcher,
                buf,
                range,
                |_| {
                    count += 1;
                    true
                },
            )?;
            // Из-за того, что `find_iter_at_in_context` внутри является
            // огромным костылём, возможно, что он не найдёт *никаких*
            // совпадений, даже хотя мы явно знаем, что есть хотя бы одно.
            // Поэтому убедимся, что мы запишем хотя бы одно здесь.
            count.max(1)
        };
        if is_multi_line {
            self.match_count += sink_match_count;
        } else {
            self.match_count += 1;
        }
        if let Some(ref mut stats) = self.stats {
            stats.add_matches(sink_match_count);
            stats.add_matched_lines(mat.lines().count() as u64);
        } else if self.summary.config.kind.quit_early() {
            return Ok(false);
        }
        Ok(true)
    }

    fn binary_data(
        &mut self,
        searcher: &Searcher,
        binary_byte_offset: u64,
    ) -> Result<bool, io::Error> {
        if searcher.binary_detection().quit_byte().is_some() {
            if let Some(ref path) = self.path {
                log::debug!(
                    "ignoring {path}: found binary data at \
                     offset {binary_byte_offset}",
                    path = path.as_path().display(),
                );
            }
        }
        Ok(true)
    }

    fn begin(&mut self, _searcher: &Searcher) -> Result<bool, io::Error> {
        if self.path.is_none() && self.summary.config.kind.requires_path() {
            return Err(io::Error::error_message(format!(
                "output kind {:?} requires a file path",
                self.summary.config.kind,
            )));
        }
        self.summary.wtr.borrow_mut().reset_count();
        self.start_time = Instant::now();
        self.match_count = 0;
        self.binary_byte_offset = None;
        Ok(true)
    }

    fn finish(
        &mut self,
        searcher: &Searcher,
        finish: &SinkFinish,
    ) -> Result<(), io::Error> {
        self.binary_byte_offset = finish.binary_byte_offset();
        if let Some(ref mut stats) = self.stats {
            stats.add_elapsed(self.start_time.elapsed());
            stats.add_searches(1);
            if self.match_count > 0 {
                stats.add_searches_with_match(1);
            }
            stats.add_bytes_searched(finish.byte_count());
            stats.add_bytes_printed(self.summary.wtr.borrow().count());
        }
        // Если наш метод обнаружения бинарных данных говорит завершить
        // после обнаружения бинарных данных, то мы не должны печатать
        // никакие результаты вообще, даже если мы нашли совпадение до
        // обнаружения бинарных данных. Цель здесь — сохранить
        // BinaryDetection::quit как форму фильтра. В противном случае
        // мы можем представить файл с совпадением с меньшим количеством
        // совпадений, чем могло бы быть, что может быть весьма
        // вводящим в заблуждение.
        //
        // Если наш метод обнаружения бинарных данных заключается в
        // преобразовании бинарных данных, то мы не завершаем и поэтому
        // ищем всё содержимое файла.
        //
        // Здесь есть досадное несоответствие. А именно, при использовании
        // QuietWithMatch или PathWithMatch принтер может завершить после
        // первого увиденного совпадения, которое может быть задолго до
        // обнаружения бинарных данных. Это означает, что использование
        // PathWithMatch может напечатать путь, тогда как использование
        // Count может не напечатать его вообще из-за бинарных данных.
        //
        // Это невозможно исправить без потенциально значительного влияния
        // на производительность QuietWithMatch или PathWithMatch, поэтому
        // мы принимаем эту ошибку.
        if self.binary_byte_offset.is_some()
            && searcher.binary_detection().quit_byte().is_some()
        {
            // Обнулить количество совпадений. Сообщаемая статистика всё
            // ещё будет содержать количество совпадений, но «официальное»
            // количество совпадений должно быть нулевым.
            self.match_count = 0;
            return Ok(());
        }

        let show_count =
            !self.summary.config.exclude_zero || self.match_count > 0;
        match self.summary.config.kind {
            SummaryKind::Count => {
                if show_count {
                    self.write_path_field()?;
                    self.write(self.match_count.to_string().as_bytes())?;
                    self.write_line_term(searcher)?;
                }
            }
            SummaryKind::CountMatches => {
                if show_count {
                    self.write_path_field()?;
                    let stats = self
                        .stats
                        .as_ref()
                        .expect("CountMatches should enable stats tracking");
                    self.write(stats.matches().to_string().as_bytes())?;
                    self.write_line_term(searcher)?;
                }
            }
            SummaryKind::PathWithMatch => {
                if self.match_count > 0 {
                    self.write_path_line(searcher)?;
                }
            }
            SummaryKind::PathWithoutMatch => {
                if self.match_count == 0 {
                    self.write_path_line(searcher)?;
                }
            }
            SummaryKind::QuietWithMatch | SummaryKind::QuietWithoutMatch => {}
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use grep_regex::RegexMatcher;
    use grep_searcher::SearcherBuilder;
    use termcolor::NoColor;

    use super::{Summary, SummaryBuilder, SummaryKind};

    const SHERLOCK: &'static [u8] = b"\
For the Doctor Watsons of this world, as opposed to the Sherlock
Holmeses, success in the province of detective work must always
be, to a very large extent, the result of luck. Sherlock Holmes
can extract a clew from a wisp of straw or a flake of cigar ash;
but Doctor Watson has to have it taken out for him and dusted,
and exhibited clearly, with a label attached.
";

    fn printer_contents(printer: &mut Summary<NoColor<Vec<u8>>>) -> String {
        String::from_utf8(printer.get_mut().get_ref().to_owned()).unwrap()
    }

    #[test]
    fn path_with_match_error() {
        let matcher = RegexMatcher::new(r"Watson").unwrap();
        let mut printer = SummaryBuilder::new()
            .kind(SummaryKind::PathWithMatch)
            .build_no_color(vec![]);
        let res = SearcherBuilder::new().build().search_reader(
            &matcher,
            SHERLOCK,
            printer.sink(&matcher),
        );
        assert!(res.is_err());
    }

    #[test]
    fn path_without_match_error() {
        let matcher = RegexMatcher::new(r"Watson").unwrap();
        let mut printer = SummaryBuilder::new()
            .kind(SummaryKind::PathWithoutMatch)
            .build_no_color(vec![]);
        let res = SearcherBuilder::new().build().search_reader(
            &matcher,
            SHERLOCK,
            printer.sink(&matcher),
        );
        assert!(res.is_err());
    }

    #[test]
    fn count_no_path() {
        let matcher = RegexMatcher::new(r"Watson").unwrap();
        let mut printer = SummaryBuilder::new()
            .kind(SummaryKind::Count)
            .build_no_color(vec![]);
        SearcherBuilder::new()
            .build()
            .search_reader(&matcher, SHERLOCK, printer.sink(&matcher))
            .unwrap();

        let got = printer_contents(&mut printer);
        assert_eq_printed!("2\n", got);
    }

    #[test]
    fn count_no_path_even_with_path() {
        let matcher = RegexMatcher::new(r"Watson").unwrap();
        let mut printer = SummaryBuilder::new()
            .kind(SummaryKind::Count)
            .path(false)
            .build_no_color(vec![]);
        SearcherBuilder::new()
            .build()
            .search_reader(
                &matcher,
                SHERLOCK,
                printer.sink_with_path(&matcher, "sherlock"),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        assert_eq_printed!("2\n", got);
    }

    #[test]
    fn count_path() {
        let matcher = RegexMatcher::new(r"Watson").unwrap();
        let mut printer = SummaryBuilder::new()
            .kind(SummaryKind::Count)
            .build_no_color(vec![]);
        SearcherBuilder::new()
            .build()
            .search_reader(
                &matcher,
                SHERLOCK,
                printer.sink_with_path(&matcher, "sherlock"),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        assert_eq_printed!("sherlock:2\n", got);
    }

    #[test]
    fn count_path_with_zero() {
        let matcher = RegexMatcher::new(r"NO MATCH").unwrap();
        let mut printer = SummaryBuilder::new()
            .kind(SummaryKind::Count)
            .exclude_zero(false)
            .build_no_color(vec![]);
        SearcherBuilder::new()
            .build()
            .search_reader(
                &matcher,
                SHERLOCK,
                printer.sink_with_path(&matcher, "sherlock"),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        assert_eq_printed!("sherlock:0\n", got);
    }

    #[test]
    fn count_path_without_zero() {
        let matcher = RegexMatcher::new(r"NO MATCH").unwrap();
        let mut printer = SummaryBuilder::new()
            .kind(SummaryKind::Count)
            .exclude_zero(true)
            .build_no_color(vec![]);
        SearcherBuilder::new()
            .build()
            .search_reader(
                &matcher,
                SHERLOCK,
                printer.sink_with_path(&matcher, "sherlock"),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        assert_eq_printed!("", got);
    }

    #[test]
    fn count_path_field_separator() {
        let matcher = RegexMatcher::new(r"Watson").unwrap();
        let mut printer = SummaryBuilder::new()
            .kind(SummaryKind::Count)
            .separator_field(b"ZZ".to_vec())
            .build_no_color(vec![]);
        SearcherBuilder::new()
            .build()
            .search_reader(
                &matcher,
                SHERLOCK,
                printer.sink_with_path(&matcher, "sherlock"),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        assert_eq_printed!("sherlockZZ2\n", got);
    }

    #[test]
    fn count_path_terminator() {
        let matcher = RegexMatcher::new(r"Watson").unwrap();
        let mut printer = SummaryBuilder::new()
            .kind(SummaryKind::Count)
            .path_terminator(Some(b'\x00'))
            .build_no_color(vec![]);
        SearcherBuilder::new()
            .build()
            .search_reader(
                &matcher,
                SHERLOCK,
                printer.sink_with_path(&matcher, "sherlock"),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        assert_eq_printed!("sherlock\x002\n", got);
    }

    #[test]
    fn count_path_separator() {
        let matcher = RegexMatcher::new(r"Watson").unwrap();
        let mut printer = SummaryBuilder::new()
            .kind(SummaryKind::Count)
            .separator_path(Some(b'\\'))
            .build_no_color(vec![]);
        SearcherBuilder::new()
            .build()
            .search_reader(
                &matcher,
                SHERLOCK,
                printer.sink_with_path(&matcher, "/home/andrew/sherlock"),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        assert_eq_printed!("\\home\\andrew\\sherlock:2\n", got);
    }

    #[test]
    fn count_max_matches() {
        let matcher = RegexMatcher::new(r"Watson").unwrap();
        let mut printer = SummaryBuilder::new()
            .kind(SummaryKind::Count)
            .build_no_color(vec![]);
        SearcherBuilder::new()
            .max_matches(Some(1))
            .build()
            .search_reader(&matcher, SHERLOCK, printer.sink(&matcher))
            .unwrap();

        let got = printer_contents(&mut printer);
        assert_eq_printed!("1\n", got);
    }

    #[test]
    fn count_matches() {
        let matcher = RegexMatcher::new(r"Watson|Sherlock").unwrap();
        let mut printer = SummaryBuilder::new()
            .kind(SummaryKind::CountMatches)
            .build_no_color(vec![]);
        SearcherBuilder::new()
            .build()
            .search_reader(
                &matcher,
                SHERLOCK,
                printer.sink_with_path(&matcher, "sherlock"),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        assert_eq_printed!("sherlock:4\n", got);
    }

    #[test]
    fn path_with_match_found() {
        let matcher = RegexMatcher::new(r"Watson").unwrap();
        let mut printer = SummaryBuilder::new()
            .kind(SummaryKind::PathWithMatch)
            .build_no_color(vec![]);
        SearcherBuilder::new()
            .build()
            .search_reader(
                &matcher,
                SHERLOCK,
                printer.sink_with_path(&matcher, "sherlock"),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        assert_eq_printed!("sherlock\n", got);
    }

    #[test]
    fn path_with_match_not_found() {
        let matcher = RegexMatcher::new(r"ZZZZZZZZ").unwrap();
        let mut printer = SummaryBuilder::new()
            .kind(SummaryKind::PathWithMatch)
            .build_no_color(vec![]);
        SearcherBuilder::new()
            .build()
            .search_reader(
                &matcher,
                SHERLOCK,
                printer.sink_with_path(&matcher, "sherlock"),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        assert_eq_printed!("", got);
    }

    #[test]
    fn path_without_match_found() {
        let matcher = RegexMatcher::new(r"ZZZZZZZZZ").unwrap();
        let mut printer = SummaryBuilder::new()
            .kind(SummaryKind::PathWithoutMatch)
            .build_no_color(vec![]);
        SearcherBuilder::new()
            .build()
            .search_reader(
                &matcher,
                SHERLOCK,
                printer.sink_with_path(&matcher, "sherlock"),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        assert_eq_printed!("sherlock\n", got);
    }

    #[test]
    fn path_without_match_not_found() {
        let matcher = RegexMatcher::new(r"Watson").unwrap();
        let mut printer = SummaryBuilder::new()
            .kind(SummaryKind::PathWithoutMatch)
            .build_no_color(vec![]);
        SearcherBuilder::new()
            .build()
            .search_reader(
                &matcher,
                SHERLOCK,
                printer.sink_with_path(&matcher, "sherlock"),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        assert_eq_printed!("", got);
    }

    #[test]
    fn quiet() {
        let matcher = RegexMatcher::new(r"Watson|Sherlock").unwrap();
        let mut printer = SummaryBuilder::new()
            .kind(SummaryKind::QuietWithMatch)
            .build_no_color(vec![]);
        let match_count = {
            let mut sink = printer.sink_with_path(&matcher, "sherlock");
            SearcherBuilder::new()
                .build()
                .search_reader(&matcher, SHERLOCK, &mut sink)
                .unwrap();
            sink.match_count
        };

        let got = printer_contents(&mut printer);
        assert_eq_printed!("", got);
        // На самом деле совпадений больше одного, но Quiet должен
        // завершить после нахождения первого.
        assert_eq!(1, match_count);
    }

    #[test]
    fn quiet_with_stats() {
        let matcher = RegexMatcher::new(r"Watson|Sherlock").unwrap();
        let mut printer = SummaryBuilder::new()
            .kind(SummaryKind::QuietWithMatch)
            .stats(true)
            .build_no_color(vec![]);
        let match_count = {
            let mut sink = printer.sink_with_path(&matcher, "sherlock");
            SearcherBuilder::new()
                .build()
                .search_reader(&matcher, SHERLOCK, &mut sink)
                .unwrap();
            sink.match_count
        };

        let got = printer_contents(&mut printer);
        assert_eq_printed!("", got);
        // На самом деле совпадений больше одного, и Quiet обычно
        // завершает после нахождения первого, но так как мы запросили
        // статистику, он продолжит поиск всех совпадений.
        assert_eq!(3, match_count);
    }
}
