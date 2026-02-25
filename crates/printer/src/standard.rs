use std::{
    cell::{Cell, RefCell},
    cmp,
    io::{self, Write},
    path::Path,
    sync::Arc,
    time::Instant,
};

use {
    bstr::ByteSlice,
    grep_matcher::{Match, Matcher},
    grep_searcher::{
        LineStep, Searcher, Sink, SinkContext, SinkFinish, SinkMatch,
    },
    termcolor::{ColorSpec, NoColor, WriteColor},
};

use crate::{
    color::ColorSpecs,
    counter::CounterWriter,
    hyperlink::{self, HyperlinkConfig},
    stats::Stats,
    util::{
        DecimalFormatter, PrinterPath, Replacer, Sunk,
        find_iter_at_in_context, trim_ascii_prefix, trim_line_terminator,
    },
};

/// Конфигурация для стандартного принтера.
///
/// Управляется через StandardBuilder и затем используется реальной
/// реализацией. После создания принтера конфигурация замораживается
/// и не может быть изменена.
#[derive(Debug, Clone)]
struct Config {
    colors: ColorSpecs,
    hyperlink: HyperlinkConfig,
    stats: bool,
    heading: bool,
    path: bool,
    only_matching: bool,
    per_match: bool,
    per_match_one_line: bool,
    replacement: Arc<Option<Vec<u8>>>,
    max_columns: Option<u64>,
    max_columns_preview: bool,
    column: bool,
    byte_offset: bool,
    trim_ascii: bool,
    separator_search: Arc<Option<Vec<u8>>>,
    separator_context: Arc<Option<Vec<u8>>>,
    separator_field_match: Arc<Vec<u8>>,
    separator_field_context: Arc<Vec<u8>>,
    separator_path: Option<u8>,
    path_terminator: Option<u8>,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            colors: ColorSpecs::default(),
            hyperlink: HyperlinkConfig::default(),
            stats: false,
            heading: false,
            path: true,
            only_matching: false,
            per_match: false,
            per_match_one_line: false,
            replacement: Arc::new(None),
            max_columns: None,
            max_columns_preview: false,
            column: false,
            byte_offset: false,
            trim_ascii: false,
            separator_search: Arc::new(None),
            separator_context: Arc::new(Some(b"--".to_vec())),
            separator_field_match: Arc::new(b":".to_vec()),
            separator_field_context: Arc::new(b"-".to_vec()),
            separator_path: None,
            path_terminator: None,
        }
    }
}

/// Билдер для «стандартного» grep-подобного принтера.
///
/// Билдер позволяет настроить поведение принтера. Настраиваемое
/// поведение включает, но не ограничивается, ограничением количества совпадений,
/// настройкой разделителей, выполнением замены шаблонов, записью статистики
/// и установкой цветов.
///
/// Некоторые параметры конфигурации, такие как отображение номеров строк или
/// контекстных строк, берутся непосредственно из конфигурации
/// `grep_searcher::Searcher`.
///
/// После создания `Standard` принтера его конфигурация не может быть изменена.
#[derive(Clone, Debug)]
pub struct StandardBuilder {
    config: Config,
}

impl StandardBuilder {
    /// Создать новый билдер для настройки стандартного принтера.
    pub fn new() -> StandardBuilder {
        StandardBuilder { config: Config::default() }
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
    pub fn build<W: WriteColor>(&self, wtr: W) -> Standard<W> {
        Standard {
            config: self.config.clone(),
            wtr: RefCell::new(CounterWriter::new(wtr)),
            matches: vec![],
        }
    }

    /// Создать принтер из любой реализации `io::Write` и никогда не выводить
    /// какие-либо цвета, независимо от настроек спецификаций пользовательских
    /// цветов.
    ///
    /// Это вспомогательная функция для
    /// `StandardBuilder::build(termcolor::NoColor::new(wtr))`.
    pub fn build_no_color<W: io::Write>(
        &self,
        wtr: W,
    ) -> Standard<NoColor<W>> {
        self.build(NoColor::new(wtr))
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
    pub fn color_specs(&mut self, specs: ColorSpecs) -> &mut StandardBuilder {
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
    ) -> &mut StandardBuilder {
        self.config.hyperlink = config;
        self
    }

    /// Включить сбор различной агрегированной статистики.
    ///
    /// Когда эта опция включена (по умолчанию она отключена), статистика
    /// будет собираться для всех использований принтера `Standard`,
    /// возвращённого методом `build`, включая, но не ограничиваясь,
    /// общим количеством совпадений, общим количеством байт, подвергшихся
    /// поиску, и общим количеством напечатанных байт.
    ///
    /// Агрегированную статистику можно получить через метод
    /// [`StandardSink::stats`] sink'а.
    ///
    /// Когда эта опция включена, этому принтеру может потребоваться
    /// выполнить дополнительную работу для вычисления определённой
    /// статистики, что может привести к увеличению времени поиска.
    ///
    /// Полное описание доступной статистики см. в [`Stats`].
    pub fn stats(&mut self, yes: bool) -> &mut StandardBuilder {
        self.config.stats = yes;
        self
    }

    /// Включить использование «заголовков» в принтере.
    ///
    /// Когда эта опция включена и если путь к файлу был передан принтеру,
    /// то путь к файлу будет напечатан один раз на отдельной строке перед
    /// отображением любых совпадений. Если заголовок не является первым,
    /// что выводится принтером, то перед заголовком печатается символ
    /// перевода строки.
    ///
    /// По умолчанию эта опция отключена. Когда она отключена, принтер не
    /// показывает никаких заголовков и вместо этого печатает путь к файлу
    /// (если он был передан) на той же строке, что и каждое совпадение
    /// (или контекстная строка).
    pub fn heading(&mut self, yes: bool) -> &mut StandardBuilder {
        self.config.heading = yes;
        self
    }

    /// Когда включено, если путь был передан принтеру, то он отображается
    /// в выводе (либо как заголовок, либо как префикс к каждой строке
    /// совпадения). Когда отключено, то никакие пути никогда не включаются
    /// в вывод, даже если путь предоставлен принтеру.
    ///
    /// По умолчанию включено.
    pub fn path(&mut self, yes: bool) -> &mut StandardBuilder {
        self.config.path = yes;
        self
    }

    /// Печатать только конкретные совпадения вместо всей строки, содержащей
    /// каждое совпадение. Каждое совпадение печатается на отдельной строке.
    /// Когда включён многострочный поиск, то совпадения, охватывающие
    /// несколько строк, печатаются так, что отображаются только
    /// соответствующие части каждой строки.
    pub fn only_matching(&mut self, yes: bool) -> &mut StandardBuilder {
        self.config.only_matching = yes;
        self
    }

    /// Печатать как минимум одну строку для каждого совпадения.
    ///
    /// Это похоже на опцию `only_matching`, за исключением того, что для
    /// каждого совпадения печатается вся строка. Это обычно полезно в
    /// сочетании с опцией `column`, которая покажет начальный номер столбца
    /// для каждого совпадения в каждой строке.
    ///
    /// Когда включён многострочный режим, каждое совпадение печатается,
    /// включая каждую строку в совпадении. Как и с однострочными
    /// совпадениями, если строка содержит несколько совпадений (даже если
    /// только частично), то эта строка печатается один раз для каждого
    /// совпадения, в котором она участвует, при условии, что это первая
    /// строка в этом совпадении. В многострочном режиме номера столбцов
    /// указывают только начало совпадения. Последующие строки в
    /// многострочном совпадении всегда имеют номер столбца `1`.
    ///
    /// Когда совпадение содержит несколько строк, включение
    /// `per_match_one_line` приведёт к тому, что будет напечатана только
    /// первая строка каждого совпадения.
    pub fn per_match(&mut self, yes: bool) -> &mut StandardBuilder {
        self.config.per_match = yes;
        self
    }

    /// Печатать не более одной строки на совпадение, когда включено
    /// `per_match`.
    ///
    /// По умолчанию каждая строка в каждом найденном совпадении печатается,
    /// когда включено `per_match`. Однако это иногда нежелательно, например,
    /// когда вы всегда хотите только одну строку на совпадение.
    ///
    /// Это применимо только когда включено многострочное сопоставление,
    /// поскольку в противном случае совпадения гарантированно охватывают
    /// одну строку.
    ///
    /// По умолчанию отключено.
    pub fn per_match_one_line(&mut self, yes: bool) -> &mut StandardBuilder {
        self.config.per_match_one_line = yes;
        self
    }

    /// Установить байты, которые будут использоваться для замены каждого
    /// найденного совпадения.
    ///
    /// Байты замены могут включать ссылки на группы захвата, которые могут
    /// быть либо в индексной форме (например, `$2`), либо могут ссылаться
    /// на именованные группы захвата, если они присутствуют в исходном
    /// шаблоне (например, `$foo`).
    ///
    /// Полную документацию по формату см. в методе `interpolate` трейта
    /// `Capture` в крейте
    /// [grep-printer](https://docs.rs/grep-printer).
    pub fn replacement(
        &mut self,
        replacement: Option<Vec<u8>>,
    ) -> &mut StandardBuilder {
        self.config.replacement = Arc::new(replacement);
        self
    }

    /// Установить максимальное количество столбцов, разрешённых для каждой
    /// напечатанной строки. Один столбец эвристически определяется как
    /// один байт.
    ///
    /// Если найдена строка, превышающая этот максимум, то она заменяется
    /// сообщением о том, что строка была пропущена.
    ///
    /// По умолчанию ограничение не указывается, в этом случае каждая
    /// строка совпадения или контекстная строка печатается независимо
    /// от её длины.
    pub fn max_columns(&mut self, limit: Option<u64>) -> &mut StandardBuilder {
        self.config.max_columns = limit;
        self
    }

    /// Когда включено, если обнаружено, что строка превышает установленное
    /// ограничение максимального количества столбцов (измеряемое в байтах),
    /// то вместо неё печатается превью длинной строки.
    ///
    /// Превью будет соответствовать первым `N` *графемным кластерам* строки,
    /// где `N` — ограничение, установленное через `max_columns`.
    ///
    /// Если ограничение не установлено, то включение этой опции не имеет
    /// эффекта.
    ///
    /// По умолчанию отключено.
    pub fn max_columns_preview(&mut self, yes: bool) -> &mut StandardBuilder {
        self.config.max_columns_preview = yes;
        self
    }

    /// Печатать номер столбца первого совпадения в строке.
    ///
    /// Эта опция удобна для использования с `per_match`, который печатает
    /// строку для каждого совпадения вместе с начальным смещением для
    /// этого совпадения.
    ///
    /// Номера столбцов вычисляются в байтах от начала печатаемой строки.
    ///
    /// По умолчанию отключено.
    pub fn column(&mut self, yes: bool) -> &mut StandardBuilder {
        self.config.column = yes;
        self
    }

    /// Печатать абсолютное смещение в байтах начала каждой напечатанной
    /// строки.
    ///
    /// Абсолютное смещение в байтах начинается от начала каждого поиска и
    /// является нулевым.
    ///
    /// Если установлена опция `only_matching`, то это будет печатать
    /// абсолютное смещение в байтах начала каждого совпадения.
    pub fn byte_offset(&mut self, yes: bool) -> &mut StandardBuilder {
        self.config.byte_offset = yes;
        self
    }

    /// Когда включено, все строки будут иметь префиксные пробельные символы
    /// ASCII, обрезанные перед записью.
    ///
    /// По умолчанию отключено.
    pub fn trim_ascii(&mut self, yes: bool) -> &mut StandardBuilder {
        self.config.trim_ascii = yes;
        self
    }

    /// Установить разделитель, используемый между наборами результатов
    /// поиска.
    ///
    /// Когда это установлено, то он будет напечатан на отдельной строке
    /// непосредственно перед результатами для одного поиска, если и только
    /// если предыдущий поиск уже вывел результаты. По сути, это позволяет
    /// показать разделитель между наборами результатов поиска, который не
    /// появляется в начале или в конце всех результатов поиска.
    ///
    /// Для воспроизведения классического формата grep это обычно
    /// устанавливается в `--` (то же самое, что и разделитель контекста),
    /// если и только если запрошены контекстные строки, но в противном
    /// случае отключается.
    ///
    /// По умолчанию отключено.
    pub fn separator_search(
        &mut self,
        sep: Option<Vec<u8>>,
    ) -> &mut StandardBuilder {
        self.config.separator_search = Arc::new(sep);
        self
    }

    /// Установить разделитель, используемый между несмежными прогонами
    /// контекста поиска, но только когда searcher настроен на сообщение
    /// о контекстных строках.
    ///
    /// Разделитель всегда печатается на отдельной строке, даже если он
    /// пустой.
    ///
    /// Если разделитель не установлен, то при разрыве контекста ничего
    /// не печатается.
    ///
    /// По умолчанию установлено `--`.
    pub fn separator_context(
        &mut self,
        sep: Option<Vec<u8>>,
    ) -> &mut StandardBuilder {
        self.config.separator_context = Arc::new(sep);
        self
    }

    /// Установить разделитель, используемый между полями, выводимыми для
    /// строк совпадений.
    ///
    /// Например, когда в searcher включены номера строк, этот принтер будет
    /// печатать номер строки перед каждой строкой совпадения. Байты,
    /// переданные здесь, будут записаны после номера строки, но перед
    /// строкой совпадения.
    ///
    /// По умолчанию установлено `:`.
    pub fn separator_field_match(
        &mut self,
        sep: Vec<u8>,
    ) -> &mut StandardBuilder {
        self.config.separator_field_match = Arc::new(sep);
        self
    }

    /// Установить разделитель, используемый между полями, выводимыми для
    /// контекстных строк.
    ///
    /// Например, когда в searcher включены номера строк, этот принтер будет
    /// печатать номер строки перед каждой контекстной строкой. Байты,
    /// переданные здесь, будут записаны после номера строки, но перед
    /// контекстной строкой.
    ///
    /// По умолчанию установлено `-`.
    pub fn separator_field_context(
        &mut self,
        sep: Vec<u8>,
    ) -> &mut StandardBuilder {
        self.config.separator_field_context = Arc::new(sep);
        self
    }

    /// Установить разделитель путей, используемый при печати путей к
    /// файлам.
    ///
    /// Когда принтер настроен с путём к файлу и когда найдено совпадение,
    /// этот путь к файлу будет напечатан (либо как заголовок, либо как
    /// префикс к каждой строке совпадения или контекстной строке, в
    /// зависимости от других параметров конфигурации). Обычно печать
    /// выполняется путём вывода пути к файлу как есть. Однако эта настройка
    /// предоставляет возможность использовать другой разделитель путей от
    /// того, который настроен в текущей среде.
    ///
    /// Типичное использование этой опции — позволить пользователям cygwin
    /// в Windows установить разделитель путей в `/` вместо использования
    /// системного `\` по умолчанию.
    pub fn separator_path(&mut self, sep: Option<u8>) -> &mut StandardBuilder {
        self.config.separator_path = sep;
        self
    }

    /// Установить терминатор путей.
    ///
    /// Терминатор путей — это байт, который печатается после каждого пути
    /// к файлу, выводимого этим принтером.
    ///
    /// Если терминатор путей не установлен (по умолчанию), то пути
    /// завершаются либо символами новой строки (когда включено `heading`),
    /// либо разделителями полей совпадения или контекста (например, `:`
    /// или `-`).
    pub fn path_terminator(
        &mut self,
        terminator: Option<u8>,
    ) -> &mut StandardBuilder {
        self.config.path_terminator = terminator;
        self
    }
}

/// Стандартный принтер, реализующий grep-подобное форматирование, включая
/// поддержку цвета.
///
/// Принтер по умолчанию можно создать с помощью одного из конструкторов
/// `Standard::new` или `Standard::new_no_color`. Однако существует
/// значительное количество опций, настраивающих вывод этого принтера.
/// Эти опции могут быть настроены с помощью [`StandardBuilder`].
///
/// Этот тип параметризован над `W`, который представляет любую реализацию
/// трейта `termcolor::WriteColor`. Если цвета не желательны, то можно
/// использовать конструктор `new_no_color`, или, альтернативно, можно
/// использовать адаптер `termcolor::NoColor` для обёртки любой реализации
/// `io::Write` без включения каких-либо цветов.
#[derive(Clone, Debug)]
pub struct Standard<W> {
    config: Config,
    wtr: RefCell<CounterWriter<W>>,
    matches: Vec<Match>,
}

impl<W: WriteColor> Standard<W> {
    /// Создать стандартный принтер с конфигурацией по умолчанию, который
    /// записывает совпадения в указанный writer.
    ///
    /// Writer должен быть реализацией `termcolor::WriteColor`, а не просто
    /// реализацией `io::Write`. Для использования обычной реализации
    /// `io::Write` (одновременно жертвуя цветами) используйте конструктор
    /// `new_no_color`.
    pub fn new(wtr: W) -> Standard<W> {
        StandardBuilder::new().build(wtr)
    }
}

impl<W: io::Write> Standard<NoColor<W>> {
    /// Создать стандартный принтер с конфигурацией по умолчанию, который
    /// записывает совпадения в указанный writer.
    ///
    /// Writer может быть любой реализацией `io::Write`. С этим конструктором
    /// принтер никогда не будет выводить цвета.
    pub fn new_no_color(wtr: W) -> Standard<NoColor<W>> {
        StandardBuilder::new().build_no_color(wtr)
    }
}

impl<W: WriteColor> Standard<W> {
    /// Создать реализацию `Sink` для стандартного принтера.
    ///
    /// Это не связывает принтер с путём к файлу, что означает, что эта
    /// реализация никогда не будет печатать путь к файлу вместе с
    /// совпадениями.
    pub fn sink<'s, M: Matcher>(
        &'s mut self,
        matcher: M,
    ) -> StandardSink<'static, 's, M, W> {
        let interpolator =
            hyperlink::Interpolator::new(&self.config.hyperlink);
        let stats = if self.config.stats { Some(Stats::new()) } else { None };
        let needs_match_granularity = self.needs_match_granularity();
        StandardSink {
            matcher,
            standard: self,
            replacer: Replacer::new(),
            interpolator,
            path: None,
            start_time: Instant::now(),
            match_count: 0,
            binary_byte_offset: None,
            stats,
            needs_match_granularity,
        }
    }

    /// Создать реализацию `Sink`, связанную с путём к файлу.
    ///
    /// Когда принтер связан с путём, то он может, в зависимости от своей
    /// конфигурации, печатать путь вместе с найденными совпадениями.
    pub fn sink_with_path<'p, 's, M, P>(
        &'s mut self,
        matcher: M,
        path: &'p P,
    ) -> StandardSink<'p, 's, M, W>
    where
        M: Matcher,
        P: ?Sized + AsRef<Path>,
    {
        if !self.config.path {
            return self.sink(matcher);
        }
        let interpolator =
            hyperlink::Interpolator::new(&self.config.hyperlink);
        let stats = if self.config.stats { Some(Stats::new()) } else { None };
        let ppath = PrinterPath::new(path.as_ref())
            .with_separator(self.config.separator_path);
        let needs_match_granularity = self.needs_match_granularity();
        StandardSink {
            matcher,
            standard: self,
            replacer: Replacer::new(),
            interpolator,
            path: Some(ppath),
            start_time: Instant::now(),
            match_count: 0,
            binary_byte_offset: None,
            stats,
            needs_match_granularity,
        }
    }

    /// Возвращает true тогда и только тогда, когда конфигурация принтера
    /// требует от нас находить каждое отдельное совпадение в строках,
    /// сообщаемых searcher.
    ///
    /// Мы заботимся об этом различии, потому что нахождение каждого
    /// отдельного совпадения стоит дороже, поэтому мы делаем это только
    /// тогда, когда это необходимо.
    fn needs_match_granularity(&self) -> bool {
        let supports_color = self.wtr.borrow().supports_color();
        let match_colored = !self.config.colors.matched().is_none();

        // Раскраска требует определения каждого отдельного совпадения.
        (supports_color && match_colored)
        // Функция column требует нахождения позиции первого совпадения.
        || self.config.column
        // Требуется нахождение каждого совпадения для выполнения замены.
        || self.config.replacement.is_some()
        // Вывод строки для каждого совпадения требует нахождения каждого совпадения.
        || self.config.per_match
        // Вывод только совпадения требует нахождения каждого совпадения.
        || self.config.only_matching
        // Вычисление определённой статистики требует нахождения каждого совпадения.
        || self.config.stats
    }
}

impl<W> Standard<W> {
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
/// для стандартного принтера.
///
/// `Sink` может быть создан через методы [`Standard::sink`] или
/// [`Standard::sink_with_path`], в зависимости от того, хотите ли вы
/// включить путь к файлу в выводе принтера.
///
/// Создание `StandardSink` дёшево, и вызывающие стороны должны создавать
/// новый для каждой вещи, которая подвергается поиску. После завершения
/// поиска вызывающие стороны могут запросить у этого sink информацию,
/// такую как произошло ли совпадение или были ли найдены бинарные данные
/// (и если да, то смещение, в котором они произошли).
///
/// Этот тип параметризован несколькими параметрами типа:
///
/// * `'p` относится к времени жизни пути к файлу, если он предоставлен.
/// Когда путь к файлу не предоставлен, то это `'static`.
/// * `'s` относится к времени жизни принтера [`Standard`], который этот
/// тип заимствует.
/// * `M` относится к типу matcher, используемого
/// `grep_searcher::Searcher`, который сообщает результаты этому sink.
/// * `W` относится к нижележащему writer, в который этот принтер записывает
/// свой вывод.
#[derive(Debug)]
pub struct StandardSink<'p, 's, M: Matcher, W> {
    matcher: M,
    standard: &'s mut Standard<W>,
    replacer: Replacer<M>,
    interpolator: hyperlink::Interpolator,
    path: Option<PrinterPath<'p>>,
    start_time: Instant,
    match_count: u64,
    binary_byte_offset: Option<u64>,
    stats: Option<Stats>,
    needs_match_granularity: bool,
}

impl<'p, 's, M: Matcher, W: WriteColor> StandardSink<'p, 's, M, W> {
    /// Возвращает true тогда и только тогда, когда этот принтер получил
    /// совпадение в предыдущем поиске.
    ///
    /// Это не зависит от результата поисков до предыдущего поиска в этом
    /// sink.
    pub fn has_match(&self) -> bool {
        self.match_count > 0
    }

    /// Вернуть общее количество совпадений, сообщённых этому sink.
    ///
    /// Это соответствует количеству вызовов `Sink::matched` во время
    /// предыдущего поиска.
    ///
    /// Это не зависит от результата поисков до предыдущего поиска в этом
    /// sink.
    pub fn match_count(&self) -> u64 {
        self.match_count
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
    /// конфигурацию [`StandardBuilder`].
    pub fn stats(&self) -> Option<&Stats> {
        self.stats.as_ref()
    }

    /// Выполнить matcher на данных байтах и записать расположения
    /// совпадений, если текущая конфигурация требует гранулярности
    /// совпадений.
    fn record_matches(
        &mut self,
        searcher: &Searcher,
        bytes: &[u8],
        range: std::ops::Range<usize>,
    ) -> io::Result<()> {
        self.standard.matches.clear();
        if !self.needs_match_granularity {
            return Ok(());
        }
        // Если печать требует определения расположения каждого отдельного
        // совпадения, то вычислим и сохраним их прямо сейчас для
        // использования позже. Хотя это добавляет дополнительное
        // копирование для хранения совпадений, мы амортизируем выделение
        // памяти для этого, и это значительно упрощает логику печати до
        // такой степени, что легко убедиться, что мы никогда не делаем
        // более одного поиска для нахождения совпадений (ну, для замен
        // мы делаем один дополнительный поиск для выполнения фактической
        // замены).
        let matches = &mut self.standard.matches;
        find_iter_at_in_context(
            searcher,
            &self.matcher,
            bytes,
            range.clone(),
            |m| {
                let (s, e) = (m.start() - range.start, m.end() - range.start);
                matches.push(Match::new(s, e));
                true
            },
        )?;
        // Не сообщать о пустых совпадениях, появляющихся в конце байтов.
        if !matches.is_empty()
            && matches.last().unwrap().is_empty()
            && matches.last().unwrap().start() >= range.end
        {
            matches.pop().unwrap();
        }
        Ok(())
    }

    /// Если конфигурация указывает замену, то это выполняет замену,
    /// лениво выделяя память при необходимости.
    ///
    /// Для доступа к результату замены используйте `replacer.replacement()`.
    fn replace(
        &mut self,
        searcher: &Searcher,
        bytes: &[u8],
        range: std::ops::Range<usize>,
    ) -> io::Result<()> {
        self.replacer.clear();
        if self.standard.config.replacement.is_some() {
            let replacement =
                (*self.standard.config.replacement).as_ref().unwrap();
            self.replacer.replace_all(
                searcher,
                &self.matcher,
                bytes,
                range,
                replacement,
            )?;
        }
        Ok(())
    }
}

impl<'p, 's, M: Matcher, W: WriteColor> Sink for StandardSink<'p, 's, M, W> {
    type Error = io::Error;

    fn matched(
        &mut self,
        searcher: &Searcher,
        mat: &SinkMatch<'_>,
    ) -> Result<bool, io::Error> {
        self.match_count += 1;

        self.record_matches(
            searcher,
            mat.buffer(),
            mat.bytes_range_in_buffer(),
        )?;
        self.replace(searcher, mat.buffer(), mat.bytes_range_in_buffer())?;

        if let Some(ref mut stats) = self.stats {
            stats.add_matches(self.standard.matches.len() as u64);
            stats.add_matched_lines(mat.lines().count() as u64);
        }
        if searcher.binary_detection().convert_byte().is_some() {
            if self.binary_byte_offset.is_some() {
                return Ok(false);
            }
        }
        StandardImpl::from_match(searcher, self, mat).sink()?;
        Ok(true)
    }

    fn context(
        &mut self,
        searcher: &Searcher,
        ctx: &SinkContext<'_>,
    ) -> Result<bool, io::Error> {
        self.standard.matches.clear();
        self.replacer.clear();

        if searcher.invert_match() {
            self.record_matches(searcher, ctx.bytes(), 0..ctx.bytes().len())?;
            self.replace(searcher, ctx.bytes(), 0..ctx.bytes().len())?;
        }
        if searcher.binary_detection().convert_byte().is_some() {
            if self.binary_byte_offset.is_some() {
                return Ok(false);
            }
        }

        StandardImpl::from_context(searcher, self, ctx).sink()?;
        Ok(true)
    }

    fn context_break(
        &mut self,
        searcher: &Searcher,
    ) -> Result<bool, io::Error> {
        StandardImpl::new(searcher, self).write_context_separator()?;
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
        self.binary_byte_offset = Some(binary_byte_offset);
        Ok(true)
    }

    fn begin(&mut self, _searcher: &Searcher) -> Result<bool, io::Error> {
        self.standard.wtr.borrow_mut().reset_count();
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
        if let Some(offset) = self.binary_byte_offset {
            StandardImpl::new(searcher, self).write_binary_message(offset)?;
        }
        if let Some(stats) = self.stats.as_mut() {
            stats.add_elapsed(self.start_time.elapsed());
            stats.add_searches(1);
            if self.match_count > 0 {
                stats.add_searches_with_match(1);
            }
            stats.add_bytes_searched(finish.byte_count());
            stats.add_bytes_printed(self.standard.wtr.borrow().count());
        }
        Ok(())
    }
}

/// Фактическая реализация стандартного принтера. Это связывает вместе
/// searcher, реализацию sink и информацию о совпадении.
///
/// StandardImpl инициализируется каждый раз, когда сообщается о совпадении
/// или контекстной строке.
#[derive(Debug)]
struct StandardImpl<'a, M: Matcher, W> {
    searcher: &'a Searcher,
    sink: &'a StandardSink<'a, 'a, M, W>,
    sunk: Sunk<'a>,
    /// Установлено в true тогда и только тогда, когда мы записываем
    /// совпадение с цветом.
    in_color_match: Cell<bool>,
}

impl<'a, M: Matcher, W: WriteColor> StandardImpl<'a, M, W> {
    /// Связать self с searcher и вернуть основную реализацию Sink.
    fn new(
        searcher: &'a Searcher,
        sink: &'a StandardSink<'_, '_, M, W>,
    ) -> StandardImpl<'a, M, W> {
        StandardImpl {
            searcher,
            sink,
            sunk: Sunk::empty(),
            in_color_match: Cell::new(false),
        }
    }

    /// Связать self с searcher и вернуть основную реализацию Sink
    /// для использования при обработке строк совпадений.
    fn from_match(
        searcher: &'a Searcher,
        sink: &'a StandardSink<'_, '_, M, W>,
        mat: &'a SinkMatch<'a>,
    ) -> StandardImpl<'a, M, W> {
        let sunk = Sunk::from_sink_match(
            mat,
            &sink.standard.matches,
            sink.replacer.replacement(),
        );
        StandardImpl { sunk, ..StandardImpl::new(searcher, sink) }
    }

    /// Связать self с searcher и вернуть основную реализацию Sink
    /// для использования при обработке контекстных строк.
    fn from_context(
        searcher: &'a Searcher,
        sink: &'a StandardSink<'_, '_, M, W>,
        ctx: &'a SinkContext<'a>,
    ) -> StandardImpl<'a, M, W> {
        let sunk = Sunk::from_sink_context(
            ctx,
            &sink.standard.matches,
            sink.replacer.replacement(),
        );
        StandardImpl { sunk, ..StandardImpl::new(searcher, sink) }
    }

    fn sink(&self) -> io::Result<()> {
        self.write_search_prelude()?;
        if self.sunk.matches().is_empty() {
            if self.multi_line() && !self.is_context() {
                self.sink_fast_multi_line()
            } else {
                self.sink_fast()
            }
        } else {
            if self.multi_line() && !self.is_context() {
                self.sink_slow_multi_line()
            } else {
                self.sink_slow()
            }
        }
    }

    /// Печатать совпадения (ограниченные одной строкой) быстро, избегая
    /// определения каждого отдельного совпадения в строках, сообщённых в
    /// данном `SinkMatch`.
    ///
    /// Это следует использовать только тогда, когда конфигурация не требует
    /// гранулярности совпадений и searcher не находится в многострочном
    /// режиме.
    fn sink_fast(&self) -> io::Result<()> {
        debug_assert!(self.sunk.matches().is_empty());
        debug_assert!(!self.multi_line() || self.is_context());

        self.write_prelude(
            self.sunk.absolute_byte_offset(),
            self.sunk.line_number(),
            None,
        )?;
        self.write_line(self.sunk.bytes())
    }

    /// Печатать совпадения (возможно, охватывающие более одной строки)
    /// быстро, избегая определения каждого отдельного совпадения в строках,
    /// сообщённых в данном `SinkMatch`.
    ///
    /// Это следует использовать только тогда, когда конфигурация не требует
    /// гранулярности совпадений. Это может использоваться, когда searcher
    /// находится в многострочном режиме.
    fn sink_fast_multi_line(&self) -> io::Result<()> {
        debug_assert!(self.sunk.matches().is_empty());
        // Это на самом деле не является требуемым инвариантом для использования
        // этого метода, но если мы окажемся здесь и многострочный режим
        // отключён, то мы всё равно должны считать это ошибкой, поскольку
        // мы должны использовать matched_fast вместо этого.
        debug_assert!(self.multi_line());

        let line_term = self.searcher.line_terminator().as_byte();
        let mut absolute_byte_offset = self.sunk.absolute_byte_offset();
        for (i, line) in self.sunk.lines(line_term).enumerate() {
            self.write_prelude(
                absolute_byte_offset,
                self.sunk.line_number().map(|n| n + i as u64),
                None,
            )?;
            absolute_byte_offset += line.len() as u64;

            self.write_line(line)?;
        }
        Ok(())
    }

    /// Печатать строку совпадения, где конфигурация принтера требует
    /// нахождения каждого отдельного совпадения (например, для раскраски).
    fn sink_slow(&self) -> io::Result<()> {
        debug_assert!(!self.sunk.matches().is_empty());
        debug_assert!(!self.multi_line() || self.is_context());

        if self.config().only_matching {
            for &m in self.sunk.matches() {
                self.write_prelude(
                    self.sunk.absolute_byte_offset() + m.start() as u64,
                    self.sunk.line_number(),
                    Some(m.start() as u64 + 1),
                )?;

                let buf = &self.sunk.bytes()[m];
                self.write_colored_line(&[Match::new(0, buf.len())], buf)?;
            }
        } else if self.config().per_match {
            for &m in self.sunk.matches() {
                self.write_prelude(
                    self.sunk.absolute_byte_offset() + m.start() as u64,
                    self.sunk.line_number(),
                    Some(m.start() as u64 + 1),
                )?;
                self.write_colored_line(&[m], self.sunk.bytes())?;
            }
        } else {
            self.write_prelude(
                self.sunk.absolute_byte_offset(),
                self.sunk.line_number(),
                Some(self.sunk.matches()[0].start() as u64 + 1),
            )?;
            self.write_colored_line(self.sunk.matches(), self.sunk.bytes())?;
        }
        Ok(())
    }

    fn sink_slow_multi_line(&self) -> io::Result<()> {
        debug_assert!(!self.sunk.matches().is_empty());
        debug_assert!(self.multi_line());

        if self.config().only_matching {
            return self.sink_slow_multi_line_only_matching();
        } else if self.config().per_match {
            return self.sink_slow_multi_per_match();
        }

        let line_term = self.searcher.line_terminator().as_byte();
        let bytes = self.sunk.bytes();
        let matches = self.sunk.matches();
        let mut midx = 0;
        let mut count = 0;
        let mut stepper = LineStep::new(line_term, 0, bytes.len());
        while let Some((start, end)) = stepper.next(bytes) {
            let mut line = Match::new(start, end);
            self.write_prelude(
                self.sunk.absolute_byte_offset() + line.start() as u64,
                self.sunk.line_number().map(|n| n + count),
                Some(matches[0].start() as u64 + 1),
            )?;
            count += 1;
            self.trim_ascii_prefix(bytes, &mut line);
            if self.exceeds_max_columns(&bytes[line]) {
                self.write_exceeded_line(bytes, line, matches, &mut midx)?;
            } else {
                self.write_colored_matches(bytes, line, matches, &mut midx)?;
                self.write_line_term()?;
            }
        }
        Ok(())
    }

    fn sink_slow_multi_line_only_matching(&self) -> io::Result<()> {
        let line_term = self.searcher.line_terminator().as_byte();
        let spec = self.config().colors.matched();
        let bytes = self.sunk.bytes();
        let matches = self.sunk.matches();
        let mut midx = 0;
        let mut count = 0;
        let mut stepper = LineStep::new(line_term, 0, bytes.len());
        while let Some((start, end)) = stepper.next(bytes) {
            let mut line = Match::new(start, end);
            self.trim_line_terminator(bytes, &mut line);
            self.trim_ascii_prefix(bytes, &mut line);
            while !line.is_empty() {
                if matches[midx].end() <= line.start() {
                    if midx + 1 < matches.len() {
                        midx += 1;
                        continue;
                    } else {
                        break;
                    }
                }
                let m = matches[midx];

                if line.start() < m.start() {
                    let upto = cmp::min(line.end(), m.start());
                    line = line.with_start(upto);
                } else {
                    let upto = cmp::min(line.end(), m.end());
                    self.write_prelude(
                        self.sunk.absolute_byte_offset() + m.start() as u64,
                        self.sunk.line_number().map(|n| n + count),
                        Some(m.start() as u64 + 1),
                    )?;

                    let this_line = line.with_end(upto);
                    line = line.with_start(upto);
                    if self.exceeds_max_columns(&bytes[this_line]) {
                        self.write_exceeded_line(
                            bytes, this_line, matches, &mut midx,
                        )?;
                    } else {
                        self.write_spec(spec, &bytes[this_line])?;
                        self.write_line_term()?;
                    }
                }
            }
            count += 1;
        }
        Ok(())
    }

    fn sink_slow_multi_per_match(&self) -> io::Result<()> {
        let line_term = self.searcher.line_terminator().as_byte();
        let spec = self.config().colors.matched();
        let bytes = self.sunk.bytes();
        for &m in self.sunk.matches() {
            let mut count = 0;
            let mut stepper = LineStep::new(line_term, 0, bytes.len());
            while let Some((start, end)) = stepper.next(bytes) {
                let mut line = Match::new(start, end);
                if line.start() >= m.end() {
                    break;
                } else if line.end() <= m.start() {
                    count += 1;
                    continue;
                }
                self.write_prelude(
                    self.sunk.absolute_byte_offset() + line.start() as u64,
                    self.sunk.line_number().map(|n| n + count),
                    Some(m.start().saturating_sub(line.start()) as u64 + 1),
                )?;
                count += 1;
                self.trim_line_terminator(bytes, &mut line);
                self.trim_ascii_prefix(bytes, &mut line);
                if self.exceeds_max_columns(&bytes[line]) {
                    self.write_exceeded_line(bytes, line, &[m], &mut 0)?;
                    continue;
                }

                while !line.is_empty() {
                    if m.end() <= line.start() {
                        self.write(&bytes[line])?;
                        line = line.with_start(line.end());
                    } else if line.start() < m.start() {
                        let upto = cmp::min(line.end(), m.start());
                        self.write(&bytes[line.with_end(upto)])?;
                        line = line.with_start(upto);
                    } else {
                        let upto = cmp::min(line.end(), m.end());
                        self.write_spec(spec, &bytes[line.with_end(upto)])?;
                        line = line.with_start(upto);
                    }
                }
                self.write_line_term()?;
                // Оказывается, vimgrep действительно хочет только одну
                // строку на совпадение, даже когда совпадение охватывает
                // несколько строк. Поэтому когда эта опция включена, мы
                // просто завершаем после печати первой строки.
                //
                // См.: https://github.com/BurntSushi/ripgrep/issues/1866
                if self.config().per_match_one_line {
                    break;
                }
            }
        }
        Ok(())
    }

    /// Записать начальную часть строки совпадения. Это (может) включать
    /// такие вещи, как путь к файлу, номер строки и другие, в зависимости
    /// от конфигурации и переданных параметров.
    #[inline(always)]
    fn write_prelude(
        &self,
        absolute_byte_offset: u64,
        line_number: Option<u64>,
        column: Option<u64>,
    ) -> io::Result<()> {
        let mut prelude = PreludeWriter::new(self);
        prelude.start(line_number, column)?;
        prelude.write_path()?;
        prelude.write_line_number(line_number)?;
        prelude.write_column_number(column)?;
        prelude.write_byte_offset(absolute_byte_offset)?;
        prelude.end()
    }

    #[inline(always)]
    fn write_line(&self, line: &[u8]) -> io::Result<()> {
        let line = if !self.config().trim_ascii {
            line
        } else {
            let lineterm = self.searcher.line_terminator();
            let full_range = Match::new(0, line.len());
            let range = trim_ascii_prefix(lineterm, line, full_range);
            &line[range]
        };
        if self.exceeds_max_columns(line) {
            let range = Match::new(0, line.len());
            self.write_exceeded_line(
                line,
                range,
                self.sunk.matches(),
                &mut 0,
            )?;
        } else {
            // self.write_trim(line)?;
            self.write(line)?;
            if !self.has_line_terminator(line) {
                self.write_line_term()?;
            }
        }
        Ok(())
    }

    fn write_colored_line(
        &self,
        matches: &[Match],
        bytes: &[u8],
    ) -> io::Result<()> {
        // Если мы знаем, что не будем выводить цвет, то можем пойти быстрее.
        let spec = self.config().colors.matched();
        if !self.wtr().borrow().supports_color() || spec.is_none() {
            return self.write_line(bytes);
        }

        let mut line = Match::new(0, bytes.len());
        self.trim_ascii_prefix(bytes, &mut line);
        if self.exceeds_max_columns(bytes) {
            self.write_exceeded_line(bytes, line, matches, &mut 0)
        } else {
            self.write_colored_matches(bytes, line, matches, &mut 0)?;
            self.write_line_term()?;
            Ok(())
        }
    }

    /// Записать часть `line` из `bytes` с соответствующей раскраской для
    /// каждого `match`, начиная с `match_index`.
    ///
    /// Это учитывает обрезку любого префикса пробельных символов и *никогда*
    /// не печатает терминатор строки. Если совпадение превышает диапазон,
    /// указанный в `line`, то печатается только часть совпадения в пределах
    /// `line` (если таковая имеется).
    fn write_colored_matches(
        &self,
        bytes: &[u8],
        mut line: Match,
        matches: &[Match],
        match_index: &mut usize,
    ) -> io::Result<()> {
        self.trim_line_terminator(bytes, &mut line);
        if matches.is_empty() {
            self.write(&bytes[line])?;
            return Ok(());
        }
        self.start_line_highlight()?;
        while !line.is_empty() {
            if matches[*match_index].end() <= line.start() {
                if *match_index + 1 < matches.len() {
                    *match_index += 1;
                    continue;
                } else {
                    self.end_color_match()?;
                    self.write(&bytes[line])?;
                    break;
                }
            }

            let m = matches[*match_index];
            if line.start() < m.start() {
                let upto = cmp::min(line.end(), m.start());
                self.end_color_match()?;
                self.write(&bytes[line.with_end(upto)])?;
                line = line.with_start(upto);
            } else {
                let upto = cmp::min(line.end(), m.end());
                self.start_color_match()?;
                self.write(&bytes[line.with_end(upto)])?;
                line = line.with_start(upto);
            }
        }
        self.end_color_match()?;
        self.end_line_highlight()?;
        Ok(())
    }

    fn write_exceeded_line(
        &self,
        bytes: &[u8],
        mut line: Match,
        matches: &[Match],
        match_index: &mut usize,
    ) -> io::Result<()> {
        if self.config().max_columns_preview {
            let original = line;
            let end = bytes[line]
                .grapheme_indices()
                .map(|(_, end, _)| end)
                .take(self.config().max_columns.unwrap_or(0) as usize)
                .last()
                .unwrap_or(0)
                + line.start();
            line = line.with_end(end);
            self.write_colored_matches(bytes, line, matches, match_index)?;

            if matches.is_empty() {
                self.write(b" [... omitted end of long line]")?;
            } else {
                let remaining = matches
                    .iter()
                    .filter(|m| {
                        m.start() >= line.end() && m.start() < original.end()
                    })
                    .count();
                let tense = if remaining == 1 { "match" } else { "matches" };
                write!(
                    self.wtr().borrow_mut(),
                    " [... {} more {}]",
                    remaining,
                    tense,
                )?;
            }
            self.write_line_term()?;
            return Ok(());
        }
        if self.sunk.original_matches().is_empty() {
            if self.is_context() {
                self.write(b"[Omitted long context line]")?;
            } else {
                self.write(b"[Omitted long matching line]")?;
            }
        } else {
            if self.config().only_matching {
                if self.is_context() {
                    self.write(b"[Omitted long context line]")?;
                } else {
                    self.write(b"[Omitted long matching line]")?;
                }
            } else {
                write!(
                    self.wtr().borrow_mut(),
                    "[Omitted long line with {} matches]",
                    self.sunk.original_matches().len(),
                )?;
            }
        }
        self.write_line_term()?;
        Ok(())
    }

    /// Если у этого принтера связан путь к файлу, то это запишет этот путь
    /// в нижележащий writer, за которым следует терминатор строки.
    /// (Если установлен терминатор путей, то он используется вместо
    /// терминатора строки.)
    fn write_path_line(&self) -> io::Result<()> {
        if let Some(path) = self.path() {
            self.write_path_hyperlink(path)?;
            if let Some(term) = self.config().path_terminator {
                self.write(&[term])?;
            } else {
                self.write_line_term()?;
            }
        }
        Ok(())
    }

    fn write_search_prelude(&self) -> io::Result<()> {
        let this_search_written = self.wtr().borrow().count() > 0;
        if this_search_written {
            return Ok(());
        }
        if let Some(ref sep) = *self.config().separator_search {
            let ever_written = self.wtr().borrow().total_count() > 0;
            if ever_written {
                self.write(sep)?;
                self.write_line_term()?;
            }
        }
        if self.config().heading {
            self.write_path_line()?;
        }
        Ok(())
    }

    fn write_binary_message(&self, offset: u64) -> io::Result<()> {
        if !self.sink.has_match() {
            return Ok(());
        }

        let bin = self.searcher.binary_detection();
        if let Some(byte) = bin.quit_byte() {
            if let Some(path) = self.path() {
                self.write_path_hyperlink(path)?;
                self.write(b": ")?;
            }
            let remainder = format!(
                "WARNING: stopped searching binary file after match \
                 (found {:?} byte around offset {})\n",
                [byte].as_bstr(),
                offset,
            );
            self.write(remainder.as_bytes())?;
        } else if let Some(byte) = bin.convert_byte() {
            if let Some(path) = self.path() {
                self.write_path_hyperlink(path)?;
                self.write(b": ")?;
            }
            let remainder = format!(
                "binary file matches (found {:?} byte around offset {})\n",
                [byte].as_bstr(),
                offset,
            );
            self.write(remainder.as_bytes())?;
        }
        Ok(())
    }

    fn write_context_separator(&self) -> io::Result<()> {
        if let Some(ref sep) = *self.config().separator_context {
            self.write(sep)?;
            self.write_line_term()?;
        }
        Ok(())
    }

    fn write_line_term(&self) -> io::Result<()> {
        self.write(self.searcher.line_terminator().as_bytes())
    }

    fn write_spec(&self, spec: &ColorSpec, buf: &[u8]) -> io::Result<()> {
        let mut wtr = self.wtr().borrow_mut();
        wtr.set_color(spec)?;
        wtr.write_all(buf)?;
        wtr.reset()?;
        Ok(())
    }

    fn write_path(&self, path: &PrinterPath) -> io::Result<()> {
        let mut wtr = self.wtr().borrow_mut();
        wtr.set_color(self.config().colors.path())?;
        wtr.write_all(path.as_bytes())?;
        wtr.reset()
    }

    fn write_path_hyperlink(&self, path: &PrinterPath) -> io::Result<()> {
        let status = self.start_hyperlink(path, None, None)?;
        self.write_path(path)?;
        self.end_hyperlink(status)
    }

    fn start_hyperlink(
        &self,
        path: &PrinterPath,
        line_number: Option<u64>,
        column: Option<u64>,
    ) -> io::Result<hyperlink::InterpolatorStatus> {
        let Some(hyperpath) = path.as_hyperlink() else {
            return Ok(hyperlink::InterpolatorStatus::inactive());
        };
        let values =
            hyperlink::Values::new(hyperpath).line(line_number).column(column);
        self.sink.interpolator.begin(&values, &mut *self.wtr().borrow_mut())
    }

    fn end_hyperlink(
        &self,
        status: hyperlink::InterpolatorStatus,
    ) -> io::Result<()> {
        self.sink.interpolator.finish(status, &mut *self.wtr().borrow_mut())
    }

    fn start_color_match(&self) -> io::Result<()> {
        if self.in_color_match.get() {
            return Ok(());
        }
        self.wtr().borrow_mut().set_color(self.config().colors.matched())?;
        self.in_color_match.set(true);
        Ok(())
    }

    fn end_color_match(&self) -> io::Result<()> {
        if !self.in_color_match.get() {
            return Ok(());
        }
        if self.highlight_on() {
            self.wtr()
                .borrow_mut()
                .set_color(self.config().colors.highlight())?;
        } else {
            self.wtr().borrow_mut().reset()?;
        }
        self.in_color_match.set(false);
        Ok(())
    }

    fn highlight_on(&self) -> bool {
        !self.config().colors.highlight().is_none() && !self.is_context()
    }

    fn start_line_highlight(&self) -> io::Result<()> {
        if self.highlight_on() {
            self.wtr()
                .borrow_mut()
                .set_color(self.config().colors.highlight())?;
        }
        Ok(())
    }

    fn end_line_highlight(&self) -> io::Result<()> {
        if self.highlight_on() {
            self.wtr().borrow_mut().reset()?;
        }
        Ok(())
    }

    fn write(&self, buf: &[u8]) -> io::Result<()> {
        self.wtr().borrow_mut().write_all(buf)
    }

    fn trim_line_terminator(&self, buf: &[u8], line: &mut Match) {
        trim_line_terminator(&self.searcher, buf, line);
    }

    fn has_line_terminator(&self, buf: &[u8]) -> bool {
        self.searcher.line_terminator().is_suffix(buf)
    }

    fn is_context(&self) -> bool {
        self.sunk.context_kind().is_some()
    }

    /// Вернуть нижележащую конфигурацию для этого принтера.
    fn config(&self) -> &'a Config {
        &self.sink.standard.config
    }

    /// Вернуть нижележащий writer, в который мы печатаем.
    fn wtr(&self) -> &'a RefCell<CounterWriter<W>> {
        &self.sink.standard.wtr
    }

    /// Вернуть путь, связанный с этим принтером, если он существует.
    fn path(&self) -> Option<&'a PrinterPath<'a>> {
        self.sink.path.as_ref()
    }

    /// Вернуть соответствующий разделитель полей в зависимости от того,
    /// выводим ли мы строки совпадений или контекстные строки.
    fn separator_field(&self) -> &[u8] {
        if self.is_context() {
            &self.config().separator_field_context
        } else {
            &self.config().separator_field_match
        }
    }

    /// Возвращает true тогда и только тогда, когда данная строка превышает
    /// установленное максимальное количество столбцов. Если максимум не
    /// установлен, то это всегда возвращает false.
    fn exceeds_max_columns(&self, line: &[u8]) -> bool {
        self.config().max_columns.map_or(false, |m| line.len() as u64 > m)
    }

    /// Возвращает true тогда и только тогда, когда searcher может сообщать
    /// о совпадениях на нескольких строках.
    ///
    /// Обратите внимание, что это не просто возвращает, находится ли
    /// searcher в многострочном режиме, но также проверяет, может ли
    /// matcher сопоставлять несколько строк. Если нет, то нам не нужна
    /// многострочная обработка, даже если в searcher включён многострочный
    /// режим.
    fn multi_line(&self) -> bool {
        self.searcher.multi_line_with_matcher(&self.sink.matcher)
    }

    /// Обрезать префиксные пробелы ASCII из данного слайса и вернуть
    /// соответствующий диапазон.
    ///
    /// Это прекращает обрезку префикса, как только видит непробельный
    /// символ или терминатор строки.
    fn trim_ascii_prefix(&self, slice: &[u8], range: &mut Match) {
        if !self.config().trim_ascii {
            return;
        }
        let lineterm = self.searcher.line_terminator();
        *range = trim_ascii_prefix(lineterm, slice, *range)
    }
}

/// Writer для прелюдии (начальной части строки совпадения).
///
/// Это инкапсулирует состояние, необходимое для печати прелюдии.
struct PreludeWriter<'a, M: Matcher, W> {
    std: &'a StandardImpl<'a, M, W>,
    next_separator: PreludeSeparator,
    field_separator: &'a [u8],
    interp_status: hyperlink::InterpolatorStatus,
}

/// Тип разделителя, используемого в прелюдии
enum PreludeSeparator {
    /// Нет разделителя.
    None,
    /// Разделитель полей, либо для строки совпадения, либо для контекстной
    /// строки.
    FieldSeparator,
    /// Терминатор путей.
    PathTerminator,
}

impl<'a, M: Matcher, W: WriteColor> PreludeWriter<'a, M, W> {
    /// Создать новый prelude printer.
    #[inline(always)]
    fn new(std: &'a StandardImpl<'a, M, W>) -> PreludeWriter<'a, M, W> {
        PreludeWriter {
            std,
            next_separator: PreludeSeparator::None,
            field_separator: std.separator_field(),
            interp_status: hyperlink::InterpolatorStatus::inactive(),
        }
    }

    /// Запустить прелюдию с гиперссылкой, когда это применимо.
    ///
    /// Если был написан заголовок и формат гиперссылки инвариантен к
    /// номеру строки, то это не добавляет гиперссылку на каждую строку
    /// прелюдии, так как она всё равно не указывала бы на строку.
    /// Гиперссылка на заголовке должна быть достаточной и менее
    /// запутывающей.
    #[inline(always)]
    fn start(
        &mut self,
        line_number: Option<u64>,
        column: Option<u64>,
    ) -> io::Result<()> {
        let Some(path) = self.std.path() else { return Ok(()) };
        if self.config().hyperlink.format().is_line_dependent()
            || !self.config().heading
        {
            self.interp_status =
                self.std.start_hyperlink(path, line_number, column)?;
        }
        Ok(())
    }

    /// Завершить прелюдию и записать оставшийся вывод.
    #[inline(always)]
    fn end(&mut self) -> io::Result<()> {
        self.std.end_hyperlink(std::mem::replace(
            &mut self.interp_status,
            hyperlink::InterpolatorStatus::inactive(),
        ))?;
        self.write_separator()
    }

    /// Если у этого принтера связан путь к файлу, то это запишет этот путь
    /// в нижележащий writer, за которым следует данный разделитель полей.
    /// (Если установлен терминатор путей, то он используется вместо
    /// разделителя полей.)
    #[inline(always)]
    fn write_path(&mut self) -> io::Result<()> {
        // Прелюдия не обрабатывает заголовки, только то, что идёт перед
        // совпадением на той же строке. Поэтому если мы выводим пути в
        // заголовках, мы не должны делать это здесь на каждой строке.
        if self.config().heading {
            return Ok(());
        }
        let Some(path) = self.std.path() else { return Ok(()) };
        self.write_separator()?;
        self.std.write_path(path)?;

        self.next_separator = if self.config().path_terminator.is_some() {
            PreludeSeparator::PathTerminator
        } else {
            PreludeSeparator::FieldSeparator
        };
        Ok(())
    }

    /// Записать поле номера строки, если оно присутствует.
    #[inline(always)]
    fn write_line_number(&mut self, line: Option<u64>) -> io::Result<()> {
        let Some(line_number) = line else { return Ok(()) };
        self.write_separator()?;
        let n = DecimalFormatter::new(line_number);
        self.std.write_spec(self.config().colors.line(), n.as_bytes())?;
        self.next_separator = PreludeSeparator::FieldSeparator;
        Ok(())
    }

    /// Записать поле номера столбца, если оно присутствует и настроено
    /// для этого.
    #[inline(always)]
    fn write_column_number(&mut self, column: Option<u64>) -> io::Result<()> {
        if !self.config().column {
            return Ok(());
        }
        let Some(column_number) = column else { return Ok(()) };
        self.write_separator()?;
        let n = DecimalFormatter::new(column_number);
        self.std.write_spec(self.config().colors.column(), n.as_bytes())?;
        self.next_separator = PreludeSeparator::FieldSeparator;
        Ok(())
    }

    /// Записать поле смещения в байтах, если настроено для этого.
    #[inline(always)]
    fn write_byte_offset(&mut self, offset: u64) -> io::Result<()> {
        if !self.config().byte_offset {
            return Ok(());
        }
        self.write_separator()?;
        let n = DecimalFormatter::new(offset);
        self.std.write_spec(self.config().colors.column(), n.as_bytes())?;
        self.next_separator = PreludeSeparator::FieldSeparator;
        Ok(())
    }

    /// Записать разделитель, определённый предыдущим поле.
    ///
    /// Это вызывается перед записью содержимого поля и в конце прелюдии.
    #[inline(always)]
    fn write_separator(&mut self) -> io::Result<()> {
        match self.next_separator {
            PreludeSeparator::None => {}
            PreludeSeparator::FieldSeparator => {
                self.std.write(self.field_separator)?;
            }
            PreludeSeparator::PathTerminator => {
                if let Some(term) = self.config().path_terminator {
                    self.std.write(&[term])?;
                }
            }
        }
        self.next_separator = PreludeSeparator::None;
        Ok(())
    }

    #[inline(always)]
    fn config(&self) -> &Config {
        self.std.config()
    }
}

#[cfg(test)]
mod tests {
    use grep_matcher::LineTerminator;
    use grep_regex::{RegexMatcher, RegexMatcherBuilder};
    use grep_searcher::SearcherBuilder;
    use termcolor::{Ansi, NoColor};

    use super::{ColorSpecs, Standard, StandardBuilder};

    const SHERLOCK: &'static str = "\
For the Doctor Watsons of this world, as opposed to the Sherlock
Holmeses, success in the province of detective work must always
be, to a very large extent, the result of luck. Sherlock Holmes
can extract a clew from a wisp of straw or a flake of cigar ash;
but Doctor Watson has to have it taken out for him and dusted,
and exhibited clearly, with a label attached.\
";

    #[allow(dead_code)]
    const SHERLOCK_CRLF: &'static str = "\
For the Doctor Watsons of this world, as opposed to the Sherlock\r
Holmeses, success in the province of detective work must always\r
be, to a very large extent, the result of luck. Sherlock Holmes\r
can extract a clew from a wisp of straw or a flake of cigar ash;\r
but Doctor Watson has to have it taken out for him and dusted,\r
and exhibited clearly, with a label attached.\
";

    fn printer_contents(printer: &mut Standard<NoColor<Vec<u8>>>) -> String {
        String::from_utf8(printer.get_mut().get_ref().to_owned()).unwrap()
    }

    fn printer_contents_ansi(printer: &mut Standard<Ansi<Vec<u8>>>) -> String {
        String::from_utf8(printer.get_mut().get_ref().to_owned()).unwrap()
    }

    #[test]
    fn reports_match() {
        let matcher = RegexMatcher::new("Sherlock").unwrap();
        let mut printer = StandardBuilder::new().build(NoColor::new(vec![]));
        let mut sink = printer.sink(&matcher);
        SearcherBuilder::new()
            .line_number(false)
            .build()
            .search_reader(&matcher, SHERLOCK.as_bytes(), &mut sink)
            .unwrap();
        assert!(sink.has_match());

        let matcher = RegexMatcher::new("zzzzz").unwrap();
        let mut printer = StandardBuilder::new().build(NoColor::new(vec![]));
        let mut sink = printer.sink(&matcher);
        SearcherBuilder::new()
            .line_number(false)
            .build()
            .search_reader(&matcher, SHERLOCK.as_bytes(), &mut sink)
            .unwrap();
        assert!(!sink.has_match());
    }

    #[test]
    fn reports_binary() {
        use grep_searcher::BinaryDetection;

        let matcher = RegexMatcher::new("Sherlock").unwrap();
        let mut printer = StandardBuilder::new().build(NoColor::new(vec![]));
        let mut sink = printer.sink(&matcher);
        SearcherBuilder::new()
            .line_number(false)
            .build()
            .search_reader(&matcher, SHERLOCK.as_bytes(), &mut sink)
            .unwrap();
        assert!(sink.binary_byte_offset().is_none());

        let matcher = RegexMatcher::new(".+").unwrap();
        let mut printer = StandardBuilder::new().build(NoColor::new(vec![]));
        let mut sink = printer.sink(&matcher);
        SearcherBuilder::new()
            .line_number(false)
            .binary_detection(BinaryDetection::quit(b'\x00'))
            .build()
            .search_reader(&matcher, &b"abc\x00"[..], &mut sink)
            .unwrap();
        assert_eq!(sink.binary_byte_offset(), Some(3));
    }

    #[test]
    fn reports_stats() {
        use std::time::Duration;

        let matcher = RegexMatcher::new("Sherlock|opposed").unwrap();
        let mut printer =
            StandardBuilder::new().stats(true).build(NoColor::new(vec![]));
        let stats = {
            let mut sink = printer.sink(&matcher);
            SearcherBuilder::new()
                .line_number(false)
                .build()
                .search_reader(&matcher, SHERLOCK.as_bytes(), &mut sink)
                .unwrap();
            sink.stats().unwrap().clone()
        };
        let buf = printer_contents(&mut printer);

        assert!(stats.elapsed() > Duration::default());
        assert_eq!(stats.searches(), 1);
        assert_eq!(stats.searches_with_match(), 1);
        assert_eq!(stats.bytes_searched(), SHERLOCK.len() as u64);
        assert_eq!(stats.bytes_printed(), buf.len() as u64);
        assert_eq!(stats.matched_lines(), 2);
        assert_eq!(stats.matches(), 3);
    }

    #[test]
    fn reports_stats_multiple() {
        use std::time::Duration;

        let matcher = RegexMatcher::new("Sherlock|opposed").unwrap();
        let mut printer =
            StandardBuilder::new().stats(true).build(NoColor::new(vec![]));
        let stats = {
            let mut sink = printer.sink(&matcher);
            SearcherBuilder::new()
                .line_number(false)
                .build()
                .search_reader(&matcher, SHERLOCK.as_bytes(), &mut sink)
                .unwrap();
            SearcherBuilder::new()
                .line_number(false)
                .build()
                .search_reader(&matcher, &b"zzzzzzzzzz"[..], &mut sink)
                .unwrap();
            SearcherBuilder::new()
                .line_number(false)
                .build()
                .search_reader(&matcher, SHERLOCK.as_bytes(), &mut sink)
                .unwrap();
            sink.stats().unwrap().clone()
        };
        let buf = printer_contents(&mut printer);

        assert!(stats.elapsed() > Duration::default());
        assert_eq!(stats.searches(), 3);
        assert_eq!(stats.searches_with_match(), 2);
        assert_eq!(stats.bytes_searched(), 10 + 2 * SHERLOCK.len() as u64);
        assert_eq!(stats.bytes_printed(), buf.len() as u64);
        assert_eq!(stats.matched_lines(), 4);
        assert_eq!(stats.matches(), 6);
    }

    #[test]
    fn context_break() {
        let matcher = RegexMatcher::new("Watson").unwrap();
        let mut printer = StandardBuilder::new()
            .separator_context(Some(b"--abc--".to_vec()))
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(false)
            .before_context(1)
            .after_context(1)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
For the Doctor Watsons of this world, as opposed to the Sherlock
Holmeses, success in the province of detective work must always
--abc--
can extract a clew from a wisp of straw or a flake of cigar ash;
but Doctor Watson has to have it taken out for him and dusted,
and exhibited clearly, with a label attached.
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn context_break_multiple_no_heading() {
        let matcher = RegexMatcher::new("Watson").unwrap();
        let mut printer = StandardBuilder::new()
            .separator_search(Some(b"--xyz--".to_vec()))
            .separator_context(Some(b"--abc--".to_vec()))
            .build(NoColor::new(vec![]));

        SearcherBuilder::new()
            .line_number(false)
            .before_context(1)
            .after_context(1)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();
        SearcherBuilder::new()
            .line_number(false)
            .before_context(1)
            .after_context(1)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
For the Doctor Watsons of this world, as opposed to the Sherlock
Holmeses, success in the province of detective work must always
--abc--
can extract a clew from a wisp of straw or a flake of cigar ash;
but Doctor Watson has to have it taken out for him and dusted,
and exhibited clearly, with a label attached.
--xyz--
For the Doctor Watsons of this world, as opposed to the Sherlock
Holmeses, success in the province of detective work must always
--abc--
can extract a clew from a wisp of straw or a flake of cigar ash;
but Doctor Watson has to have it taken out for him and dusted,
and exhibited clearly, with a label attached.
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn context_break_multiple_heading() {
        let matcher = RegexMatcher::new("Watson").unwrap();
        let mut printer = StandardBuilder::new()
            .heading(true)
            .separator_search(Some(b"--xyz--".to_vec()))
            .separator_context(Some(b"--abc--".to_vec()))
            .build(NoColor::new(vec![]));

        SearcherBuilder::new()
            .line_number(false)
            .before_context(1)
            .after_context(1)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();
        SearcherBuilder::new()
            .line_number(false)
            .before_context(1)
            .after_context(1)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
For the Doctor Watsons of this world, as opposed to the Sherlock
Holmeses, success in the province of detective work must always
--abc--
can extract a clew from a wisp of straw or a flake of cigar ash;
but Doctor Watson has to have it taken out for him and dusted,
and exhibited clearly, with a label attached.
--xyz--
For the Doctor Watsons of this world, as opposed to the Sherlock
Holmeses, success in the province of detective work must always
--abc--
can extract a clew from a wisp of straw or a flake of cigar ash;
but Doctor Watson has to have it taken out for him and dusted,
and exhibited clearly, with a label attached.
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn path() {
        let matcher = RegexMatcher::new("Watson").unwrap();
        let mut printer =
            StandardBuilder::new().path(false).build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(true)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink_with_path(&matcher, "sherlock"),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
1:For the Doctor Watsons of this world, as opposed to the Sherlock
5:but Doctor Watson has to have it taken out for him and dusted,
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn separator_field() {
        let matcher = RegexMatcher::new("Watson").unwrap();
        let mut printer = StandardBuilder::new()
            .separator_field_match(b"!!".to_vec())
            .separator_field_context(b"^^".to_vec())
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(false)
            .before_context(1)
            .after_context(1)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink_with_path(&matcher, "sherlock"),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
sherlock!!For the Doctor Watsons of this world, as opposed to the Sherlock
sherlock^^Holmeses, success in the province of detective work must always
--
sherlock^^can extract a clew from a wisp of straw or a flake of cigar ash;
sherlock!!but Doctor Watson has to have it taken out for him and dusted,
sherlock^^and exhibited clearly, with a label attached.
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn separator_path() {
        let matcher = RegexMatcher::new("Watson").unwrap();
        let mut printer = StandardBuilder::new()
            .separator_path(Some(b'Z'))
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(false)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink_with_path(&matcher, "books/sherlock"),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
booksZsherlock:For the Doctor Watsons of this world, as opposed to the Sherlock
booksZsherlock:but Doctor Watson has to have it taken out for him and dusted,
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn path_terminator() {
        let matcher = RegexMatcher::new("Watson").unwrap();
        let mut printer = StandardBuilder::new()
            .path_terminator(Some(b'Z'))
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(false)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink_with_path(&matcher, "books/sherlock"),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
books/sherlockZFor the Doctor Watsons of this world, as opposed to the Sherlock
books/sherlockZbut Doctor Watson has to have it taken out for him and dusted,
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn heading() {
        let matcher = RegexMatcher::new("Watson").unwrap();
        let mut printer =
            StandardBuilder::new().heading(true).build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(false)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink_with_path(&matcher, "sherlock"),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
sherlock
For the Doctor Watsons of this world, as opposed to the Sherlock
but Doctor Watson has to have it taken out for him and dusted,
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn no_heading() {
        let matcher = RegexMatcher::new("Watson").unwrap();
        let mut printer =
            StandardBuilder::new().heading(false).build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(false)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink_with_path(&matcher, "sherlock"),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
sherlock:For the Doctor Watsons of this world, as opposed to the Sherlock
sherlock:but Doctor Watson has to have it taken out for him and dusted,
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn no_heading_multiple() {
        let matcher = RegexMatcher::new("Watson").unwrap();
        let mut printer =
            StandardBuilder::new().heading(false).build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(false)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink_with_path(&matcher, "sherlock"),
            )
            .unwrap();

        let matcher = RegexMatcher::new("Sherlock").unwrap();
        SearcherBuilder::new()
            .line_number(false)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink_with_path(&matcher, "sherlock"),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
sherlock:For the Doctor Watsons of this world, as opposed to the Sherlock
sherlock:but Doctor Watson has to have it taken out for him and dusted,
sherlock:For the Doctor Watsons of this world, as opposed to the Sherlock
sherlock:be, to a very large extent, the result of luck. Sherlock Holmes
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn heading_multiple() {
        let matcher = RegexMatcher::new("Watson").unwrap();
        let mut printer =
            StandardBuilder::new().heading(true).build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(false)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink_with_path(&matcher, "sherlock"),
            )
            .unwrap();

        let matcher = RegexMatcher::new("Sherlock").unwrap();
        SearcherBuilder::new()
            .line_number(false)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink_with_path(&matcher, "sherlock"),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
sherlock
For the Doctor Watsons of this world, as opposed to the Sherlock
but Doctor Watson has to have it taken out for him and dusted,
sherlock
For the Doctor Watsons of this world, as opposed to the Sherlock
be, to a very large extent, the result of luck. Sherlock Holmes
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn trim_ascii() {
        let matcher = RegexMatcher::new("Watson").unwrap();
        let mut printer = StandardBuilder::new()
            .trim_ascii(true)
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(false)
            .build()
            .search_reader(
                &matcher,
                "   Watson".as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
Watson
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn trim_ascii_multi_line() {
        let matcher = RegexMatcher::new("(?s:.{0})Watson").unwrap();
        let mut printer = StandardBuilder::new()
            .trim_ascii(true)
            .stats(true)
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(false)
            .multi_line(true)
            .build()
            .search_reader(
                &matcher,
                "   Watson".as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
Watson
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn trim_ascii_with_line_term() {
        let matcher = RegexMatcher::new("Watson").unwrap();
        let mut printer = StandardBuilder::new()
            .trim_ascii(true)
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(true)
            .before_context(1)
            .build()
            .search_reader(
                &matcher,
                "\n   Watson".as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
1-
2:Watson
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn line_number() {
        let matcher = RegexMatcher::new("Watson").unwrap();
        let mut printer = StandardBuilder::new().build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(true)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
1:For the Doctor Watsons of this world, as opposed to the Sherlock
5:but Doctor Watson has to have it taken out for him and dusted,
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn line_number_multi_line() {
        let matcher = RegexMatcher::new("(?s)Watson.+Watson").unwrap();
        let mut printer = StandardBuilder::new().build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(true)
            .multi_line(true)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
1:For the Doctor Watsons of this world, as opposed to the Sherlock
2:Holmeses, success in the province of detective work must always
3:be, to a very large extent, the result of luck. Sherlock Holmes
4:can extract a clew from a wisp of straw or a flake of cigar ash;
5:but Doctor Watson has to have it taken out for him and dusted,
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn column_number() {
        let matcher = RegexMatcher::new("Watson").unwrap();
        let mut printer =
            StandardBuilder::new().column(true).build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(false)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
16:For the Doctor Watsons of this world, as opposed to the Sherlock
12:but Doctor Watson has to have it taken out for him and dusted,
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn column_number_multi_line() {
        let matcher = RegexMatcher::new("(?s)Watson.+Watson").unwrap();
        let mut printer =
            StandardBuilder::new().column(true).build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(false)
            .multi_line(true)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
16:For the Doctor Watsons of this world, as opposed to the Sherlock
16:Holmeses, success in the province of detective work must always
16:be, to a very large extent, the result of luck. Sherlock Holmes
16:can extract a clew from a wisp of straw or a flake of cigar ash;
16:but Doctor Watson has to have it taken out for him and dusted,
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn byte_offset() {
        let matcher = RegexMatcher::new("Watson").unwrap();
        let mut printer = StandardBuilder::new()
            .byte_offset(true)
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(false)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
0:For the Doctor Watsons of this world, as opposed to the Sherlock
258:but Doctor Watson has to have it taken out for him and dusted,
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn byte_offset_multi_line() {
        let matcher = RegexMatcher::new("(?s)Watson.+Watson").unwrap();
        let mut printer = StandardBuilder::new()
            .byte_offset(true)
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(false)
            .multi_line(true)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
0:For the Doctor Watsons of this world, as opposed to the Sherlock
65:Holmeses, success in the province of detective work must always
129:be, to a very large extent, the result of luck. Sherlock Holmes
193:can extract a clew from a wisp of straw or a flake of cigar ash;
258:but Doctor Watson has to have it taken out for him and dusted,
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn max_columns() {
        let matcher = RegexMatcher::new("ash|dusted").unwrap();
        let mut printer = StandardBuilder::new()
            .max_columns(Some(63))
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(false)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
[Omitted long matching line]
but Doctor Watson has to have it taken out for him and dusted,
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn max_columns_preview() {
        let matcher = RegexMatcher::new("exhibited|dusted").unwrap();
        let mut printer = StandardBuilder::new()
            .max_columns(Some(46))
            .max_columns_preview(true)
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(false)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
but Doctor Watson has to have it taken out for [... omitted end of long line]
and exhibited clearly, with a label attached.
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn max_columns_with_count() {
        let matcher = RegexMatcher::new("cigar|ash|dusted").unwrap();
        let mut printer = StandardBuilder::new()
            .stats(true)
            .max_columns(Some(63))
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(false)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
[Omitted long line with 2 matches]
but Doctor Watson has to have it taken out for him and dusted,
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn max_columns_with_count_preview_no_match() {
        let matcher = RegexMatcher::new("exhibited|has to have it").unwrap();
        let mut printer = StandardBuilder::new()
            .stats(true)
            .max_columns(Some(46))
            .max_columns_preview(true)
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(false)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
but Doctor Watson has to have it taken out for [... 0 more matches]
and exhibited clearly, with a label attached.
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn max_columns_with_count_preview_one_match() {
        let matcher = RegexMatcher::new("exhibited|dusted").unwrap();
        let mut printer = StandardBuilder::new()
            .stats(true)
            .max_columns(Some(46))
            .max_columns_preview(true)
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(false)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
but Doctor Watson has to have it taken out for [... 1 more match]
and exhibited clearly, with a label attached.
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn max_columns_with_count_preview_two_matches() {
        let matcher =
            RegexMatcher::new("exhibited|dusted|has to have it").unwrap();
        let mut printer = StandardBuilder::new()
            .stats(true)
            .max_columns(Some(46))
            .max_columns_preview(true)
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(false)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
but Doctor Watson has to have it taken out for [... 1 more match]
and exhibited clearly, with a label attached.
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn max_columns_multi_line() {
        let matcher = RegexMatcher::new("(?s)ash.+dusted").unwrap();
        let mut printer = StandardBuilder::new()
            .max_columns(Some(63))
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(false)
            .multi_line(true)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
[Omitted long matching line]
but Doctor Watson has to have it taken out for him and dusted,
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn max_columns_multi_line_preview() {
        let matcher =
            RegexMatcher::new("(?s)clew|cigar ash.+have it|exhibited")
                .unwrap();
        let mut printer = StandardBuilder::new()
            .stats(true)
            .max_columns(Some(46))
            .max_columns_preview(true)
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(false)
            .multi_line(true)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
can extract a clew from a wisp of straw or a f [... 1 more match]
but Doctor Watson has to have it taken out for [... 0 more matches]
and exhibited clearly, with a label attached.
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn max_matches() {
        let matcher = RegexMatcher::new("Sherlock").unwrap();
        let mut printer = StandardBuilder::new().build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(false)
            .max_matches(Some(1))
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
For the Doctor Watsons of this world, as opposed to the Sherlock
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn max_matches_context() {
        // после контекста: 1
        let matcher = RegexMatcher::new("Doctor Watsons").unwrap();
        let mut printer = StandardBuilder::new().build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .max_matches(Some(1))
            .line_number(false)
            .after_context(1)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
For the Doctor Watsons of this world, as opposed to the Sherlock
Holmeses, success in the province of detective work must always
";
        assert_eq_printed!(expected, got);

        // после контекста: 4
        let mut printer = StandardBuilder::new().build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .max_matches(Some(1))
            .line_number(false)
            .after_context(4)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
For the Doctor Watsons of this world, as opposed to the Sherlock
Holmeses, success in the province of detective work must always
be, to a very large extent, the result of luck. Sherlock Holmes
can extract a clew from a wisp of straw or a flake of cigar ash;
but Doctor Watson has to have it taken out for him and dusted,
";
        assert_eq_printed!(expected, got);

        // после контекста: 1, макс. совпадений: 2
        let matcher = RegexMatcher::new("Doctor Watsons|but Doctor").unwrap();
        let mut printer = StandardBuilder::new().build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .max_matches(Some(2))
            .line_number(false)
            .after_context(1)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
For the Doctor Watsons of this world, as opposed to the Sherlock
Holmeses, success in the province of detective work must always
--
but Doctor Watson has to have it taken out for him and dusted,
and exhibited clearly, with a label attached.
";
        assert_eq_printed!(expected, got);

        // после контекста: 4, макс. совпадений: 2
        let mut printer = StandardBuilder::new().build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .max_matches(Some(2))
            .line_number(false)
            .after_context(4)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
For the Doctor Watsons of this world, as opposed to the Sherlock
Holmeses, success in the province of detective work must always
be, to a very large extent, the result of luck. Sherlock Holmes
can extract a clew from a wisp of straw or a flake of cigar ash;
but Doctor Watson has to have it taken out for him and dusted,
and exhibited clearly, with a label attached.
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn max_matches_context_invert() {
        // после контекста: 1
        let matcher =
            RegexMatcher::new("success|extent|clew|dusted|exhibited").unwrap();
        let mut printer = StandardBuilder::new().build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .invert_match(true)
            .max_matches(Some(1))
            .line_number(false)
            .after_context(1)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
For the Doctor Watsons of this world, as opposed to the Sherlock
Holmeses, success in the province of detective work must always
";
        assert_eq_printed!(expected, got);

        // после контекста: 4
        let mut printer = StandardBuilder::new().build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .invert_match(true)
            .max_matches(Some(1))
            .line_number(false)
            .after_context(4)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
For the Doctor Watsons of this world, as opposed to the Sherlock
Holmeses, success in the province of detective work must always
be, to a very large extent, the result of luck. Sherlock Holmes
can extract a clew from a wisp of straw or a flake of cigar ash;
but Doctor Watson has to have it taken out for him and dusted,
";
        assert_eq_printed!(expected, got);

        // после контекста: 1, макс. совпадений: 2
        let matcher =
            RegexMatcher::new("success|extent|clew|exhibited").unwrap();
        let mut printer = StandardBuilder::new().build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .invert_match(true)
            .max_matches(Some(2))
            .line_number(false)
            .after_context(1)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
For the Doctor Watsons of this world, as opposed to the Sherlock
Holmeses, success in the province of detective work must always
--
but Doctor Watson has to have it taken out for him and dusted,
and exhibited clearly, with a label attached.
";
        assert_eq_printed!(expected, got);

        // после контекста: 4, макс. совпадений: 2
        let mut printer = StandardBuilder::new().build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .invert_match(true)
            .max_matches(Some(2))
            .line_number(false)
            .after_context(4)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
For the Doctor Watsons of this world, as opposed to the Sherlock
Holmeses, success in the province of detective work must always
be, to a very large extent, the result of luck. Sherlock Holmes
can extract a clew from a wisp of straw or a flake of cigar ash;
but Doctor Watson has to have it taken out for him and dusted,
and exhibited clearly, with a label attached.
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn max_matches_multi_line1() {
        let matcher = RegexMatcher::new("(?s:.{0})Sherlock").unwrap();
        let mut printer = StandardBuilder::new().build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(false)
            .multi_line(true)
            .max_matches(Some(1))
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
For the Doctor Watsons of this world, as opposed to the Sherlock
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn max_matches_multi_line2() {
        let matcher =
            RegexMatcher::new(r"(?s)Watson.+?(Holmeses|clearly)").unwrap();
        let mut printer = StandardBuilder::new().build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(false)
            .multi_line(true)
            .max_matches(Some(1))
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
For the Doctor Watsons of this world, as opposed to the Sherlock
Holmeses, success in the province of detective work must always
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn max_matches_multi_line3() {
        let matcher = RegexMatcher::new(r"line 2\nline 3").unwrap();
        let mut printer = StandardBuilder::new().build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(false)
            .multi_line(true)
            .max_matches(Some(1))
            .build()
            .search_reader(
                &matcher,
                "line 2\nline 3 x\nline 2\nline 3\n".as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
line 2
line 3 x
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn max_matches_multi_line4() {
        let matcher =
            RegexMatcher::new(r"line 2\nline 3|x\nline 2\n").unwrap();
        let mut printer = StandardBuilder::new().build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(false)
            .multi_line(true)
            .max_matches(Some(1))
            .build()
            .search_reader(
                &matcher,
                "line 2\nline 3 x\nline 2\nline 3 x\n".as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
line 2
line 3 x
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn only_matching() {
        let matcher = RegexMatcher::new("Doctor Watsons|Sherlock").unwrap();
        let mut printer = StandardBuilder::new()
            .only_matching(true)
            .column(true)
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(true)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
1:9:Doctor Watsons
1:57:Sherlock
3:49:Sherlock
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn only_matching_multi_line1() {
        let matcher =
            RegexMatcher::new(r"(?s:.{0})(Doctor Watsons|Sherlock)").unwrap();
        let mut printer = StandardBuilder::new()
            .only_matching(true)
            .column(true)
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .multi_line(true)
            .line_number(true)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
1:9:Doctor Watsons
1:57:Sherlock
3:49:Sherlock
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn only_matching_multi_line2() {
        let matcher =
            RegexMatcher::new(r"(?s)Watson.+?(Holmeses|clearly)").unwrap();
        let mut printer = StandardBuilder::new()
            .only_matching(true)
            .column(true)
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .multi_line(true)
            .line_number(true)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
1:16:Watsons of this world, as opposed to the Sherlock
2:16:Holmeses
5:12:Watson has to have it taken out for him and dusted,
6:12:and exhibited clearly
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn only_matching_max_columns() {
        let matcher = RegexMatcher::new("Doctor Watsons|Sherlock").unwrap();
        let mut printer = StandardBuilder::new()
            .only_matching(true)
            .max_columns(Some(10))
            .column(true)
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(true)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
1:9:[Omitted long matching line]
1:57:Sherlock
3:49:Sherlock
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn only_matching_max_columns_preview() {
        let matcher = RegexMatcher::new("Doctor Watsons|Sherlock").unwrap();
        let mut printer = StandardBuilder::new()
            .only_matching(true)
            .max_columns(Some(10))
            .max_columns_preview(true)
            .column(true)
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(true)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
1:9:Doctor Wat [... 0 more matches]
1:57:Sherlock
3:49:Sherlock
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn only_matching_max_columns_multi_line1() {
        // Трюк `(?s:.{0})` обманывает matcher, заставляя его думать, что
        // он может сопоставлять несколько строк, фактически не делая этого.
        // Это нужно, чтобы мы могли протестировать многострочн��ю обработку
        // в случае совпадения только на одной строке.
        let matcher =
            RegexMatcher::new(r"(?s:.{0})(Doctor Watsons|Sherlock)").unwrap();
        let mut printer = StandardBuilder::new()
            .only_matching(true)
            .max_columns(Some(10))
            .column(true)
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .multi_line(true)
            .line_number(true)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
1:9:[Omitted long matching line]
1:57:Sherlock
3:49:Sherlock
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn only_matching_max_columns_preview_multi_line1() {
        // Трюк `(?s:.{0})` обманывает matcher, заставляя его думать, что
        // он может сопоставлять несколько строк, фактически не делая этого.
        // Это нужно, чтобы мы могли протестировать многострочную обработку
        // в случае совпадения только на одной строке.
        let matcher =
            RegexMatcher::new(r"(?s:.{0})(Doctor Watsons|Sherlock)").unwrap();
        let mut printer = StandardBuilder::new()
            .only_matching(true)
            .max_columns(Some(10))
            .max_columns_preview(true)
            .column(true)
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .multi_line(true)
            .line_number(true)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
1:9:Doctor Wat [... 0 more matches]
1:57:Sherlock
3:49:Sherlock
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn only_matching_max_columns_multi_line2() {
        let matcher =
            RegexMatcher::new(r"(?s)Watson.+?(Holmeses|clearly)").unwrap();
        let mut printer = StandardBuilder::new()
            .only_matching(true)
            .max_columns(Some(50))
            .column(true)
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .multi_line(true)
            .line_number(true)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
1:16:Watsons of this world, as opposed to the Sherlock
2:16:Holmeses
5:12:[Omitted long matching line]
6:12:and exhibited clearly
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn only_matching_max_columns_preview_multi_line2() {
        let matcher =
            RegexMatcher::new(r"(?s)Watson.+?(Holmeses|clearly)").unwrap();
        let mut printer = StandardBuilder::new()
            .only_matching(true)
            .max_columns(Some(50))
            .max_columns_preview(true)
            .column(true)
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .multi_line(true)
            .line_number(true)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
1:16:Watsons of this world, as opposed to the Sherlock
2:16:Holmeses
5:12:Watson has to have it taken out for him and dusted [... 0 more matches]
6:12:and exhibited clearly
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn per_match() {
        let matcher = RegexMatcher::new("Doctor Watsons|Sherlock").unwrap();
        let mut printer = StandardBuilder::new()
            .per_match(true)
            .column(true)
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(true)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
1:9:For the Doctor Watsons of this world, as opposed to the Sherlock
1:57:For the Doctor Watsons of this world, as opposed to the Sherlock
3:49:be, to a very large extent, the result of luck. Sherlock Holmes
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn per_match_multi_line1() {
        let matcher =
            RegexMatcher::new(r"(?s:.{0})(Doctor Watsons|Sherlock)").unwrap();
        let mut printer = StandardBuilder::new()
            .per_match(true)
            .column(true)
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .multi_line(true)
            .line_number(true)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
1:9:For the Doctor Watsons of this world, as opposed to the Sherlock
1:57:For the Doctor Watsons of this world, as opposed to the Sherlock
3:49:be, to a very large extent, the result of luck. Sherlock Holmes
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn per_match_multi_line2() {
        let matcher =
            RegexMatcher::new(r"(?s)Watson.+?(Holmeses|clearly)").unwrap();
        let mut printer = StandardBuilder::new()
            .per_match(true)
            .column(true)
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .multi_line(true)
            .line_number(true)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
1:16:For the Doctor Watsons of this world, as opposed to the Sherlock
2:1:Holmeses, success in the province of detective work must always
5:12:but Doctor Watson has to have it taken out for him and dusted,
6:1:and exhibited clearly, with a label attached.
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn per_match_multi_line3() {
        let matcher =
            RegexMatcher::new(r"(?s)Watson.+?Holmeses|always.+?be").unwrap();
        let mut printer = StandardBuilder::new()
            .per_match(true)
            .column(true)
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .multi_line(true)
            .line_number(true)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
1:16:For the Doctor Watsons of this world, as opposed to the Sherlock
2:1:Holmeses, success in the province of detective work must always
2:58:Holmeses, success in the province of detective work must always
3:1:be, to a very large extent, the result of luck. Sherlock Holmes
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn per_match_multi_line1_only_first_line() {
        let matcher =
            RegexMatcher::new(r"(?s:.{0})(Doctor Watsons|Sherlock)").unwrap();
        let mut printer = StandardBuilder::new()
            .per_match(true)
            .per_match_one_line(true)
            .column(true)
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .multi_line(true)
            .line_number(true)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
1:9:For the Doctor Watsons of this world, as opposed to the Sherlock
1:57:For the Doctor Watsons of this world, as opposed to the Sherlock
3:49:be, to a very large extent, the result of luck. Sherlock Holmes
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn per_match_multi_line2_only_first_line() {
        let matcher =
            RegexMatcher::new(r"(?s)Watson.+?(Holmeses|clearly)").unwrap();
        let mut printer = StandardBuilder::new()
            .per_match(true)
            .per_match_one_line(true)
            .column(true)
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .multi_line(true)
            .line_number(true)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
1:16:For the Doctor Watsons of this world, as opposed to the Sherlock
5:12:but Doctor Watson has to have it taken out for him and dusted,
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn per_match_multi_line3_only_first_line() {
        let matcher =
            RegexMatcher::new(r"(?s)Watson.+?Holmeses|always.+?be").unwrap();
        let mut printer = StandardBuilder::new()
            .per_match(true)
            .per_match_one_line(true)
            .column(true)
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .multi_line(true)
            .line_number(true)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
1:16:For the Doctor Watsons of this world, as opposed to the Sherlock
2:58:Holmeses, success in the province of detective work must always
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn replacement_passthru() {
        let matcher = RegexMatcher::new(r"Sherlock|Doctor (\w+)").unwrap();
        let mut printer = StandardBuilder::new()
            .replacement(Some(b"doctah $1 MD".to_vec()))
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(true)
            .passthru(true)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
1:For the doctah Watsons MD of this world, as opposed to the doctah  MD
2-Holmeses, success in the province of detective work must always
3:be, to a very large extent, the result of luck. doctah  MD Holmes
4-can extract a clew from a wisp of straw or a flake of cigar ash;
5:but doctah Watson MD has to have it taken out for him and dusted,
6-and exhibited clearly, with a label attached.
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn replacement() {
        let matcher = RegexMatcher::new(r"Sherlock|Doctor (\w+)").unwrap();
        let mut printer = StandardBuilder::new()
            .replacement(Some(b"doctah $1 MD".to_vec()))
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(true)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
1:For the doctah Watsons MD of this world, as opposed to the doctah  MD
3:be, to a very large extent, the result of luck. doctah  MD Holmes
5:but doctah Watson MD has to have it taken out for him and dusted,
";
        assert_eq_printed!(expected, got);
    }

    // Это несколько странный тест, который проверяет поведение попытки
    // замены терминатора строки на что-то другое.
    //
    // См.: https://github.com/BurntSushi/ripgrep/issues/1311
    #[test]
    fn replacement_multi_line() {
        let matcher = RegexMatcher::new(r"\n").unwrap();
        let mut printer = StandardBuilder::new()
            .replacement(Some(b"?".to_vec()))
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(true)
            .multi_line(true)
            .build()
            .search_reader(
                &matcher,
                "hello\nworld\n".as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "1:hello?world?\n";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn replacement_multi_line_diff_line_term() {
        let matcher = RegexMatcherBuilder::new()
            .line_terminator(Some(b'\x00'))
            .build(r"\n")
            .unwrap();
        let mut printer = StandardBuilder::new()
            .replacement(Some(b"?".to_vec()))
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_terminator(LineTerminator::byte(b'\x00'))
            .line_number(true)
            .multi_line(true)
            .build()
            .search_reader(
                &matcher,
                "hello\nworld\n".as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "1:hello?world?\x00";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn replacement_multi_line_combine_lines() {
        let matcher = RegexMatcher::new(r"\n(.)?").unwrap();
        let mut printer = StandardBuilder::new()
            .replacement(Some(b"?$1".to_vec()))
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(true)
            .multi_line(true)
            .build()
            .search_reader(
                &matcher,
                "hello\nworld\n".as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "1:hello?world?\n";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn replacement_max_columns() {
        let matcher = RegexMatcher::new(r"Sherlock|Doctor (\w+)").unwrap();
        let mut printer = StandardBuilder::new()
            .max_columns(Some(67))
            .replacement(Some(b"doctah $1 MD".to_vec()))
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(true)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
1:[Omitted long line with 2 matches]
3:be, to a very large extent, the result of luck. doctah  MD Holmes
5:but doctah Watson MD has to have it taken out for him and dusted,
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn replacement_max_columns_preview1() {
        let matcher = RegexMatcher::new(r"Sherlock|Doctor (\w+)").unwrap();
        let mut printer = StandardBuilder::new()
            .max_columns(Some(67))
            .max_columns_preview(true)
            .replacement(Some(b"doctah $1 MD".to_vec()))
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(true)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
1:For the doctah Watsons MD of this world, as opposed to the doctah   [... 0 more matches]
3:be, to a very large extent, the result of luck. doctah  MD Holmes
5:but doctah Watson MD has to have it taken out for him and dusted,
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn replacement_max_columns_preview2() {
        let matcher =
            RegexMatcher::new("exhibited|dusted|has to have it").unwrap();
        let mut printer = StandardBuilder::new()
            .max_columns(Some(43))
            .max_columns_preview(true)
            .replacement(Some(b"xxx".to_vec()))
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(false)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
but Doctor Watson xxx taken out for him and [... 1 more match]
and xxx clearly, with a label attached.
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn replacement_only_matching() {
        let matcher = RegexMatcher::new(r"Sherlock|Doctor (\w+)").unwrap();
        let mut printer = StandardBuilder::new()
            .only_matching(true)
            .replacement(Some(b"doctah $1 MD".to_vec()))
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(true)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
1:doctah Watsons MD
1:doctah  MD
3:doctah  MD
5:doctah Watson MD
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn replacement_per_match() {
        let matcher = RegexMatcher::new(r"Sherlock|Doctor (\w+)").unwrap();
        let mut printer = StandardBuilder::new()
            .per_match(true)
            .replacement(Some(b"doctah $1 MD".to_vec()))
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(true)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
1:For the doctah Watsons MD of this world, as opposed to the doctah  MD
1:For the doctah Watsons MD of this world, as opposed to the doctah  MD
3:be, to a very large extent, the result of luck. doctah  MD Holmes
5:but doctah Watson MD has to have it taken out for him and dusted,
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn invert() {
        let matcher = RegexMatcher::new(r"Sherlock").unwrap();
        let mut printer = StandardBuilder::new().build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(true)
            .invert_match(true)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
2:Holmeses, success in the province of detective work must always
4:can extract a clew from a wisp of straw or a flake of cigar ash;
5:but Doctor Watson has to have it taken out for him and dusted,
6:and exhibited clearly, with a label attached.
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn invert_multi_line() {
        let matcher = RegexMatcher::new(r"(?s:.{0})Sherlock").unwrap();
        let mut printer = StandardBuilder::new().build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .multi_line(true)
            .line_number(true)
            .invert_match(true)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
2:Holmeses, success in the province of detective work must always
4:can extract a clew from a wisp of straw or a flake of cigar ash;
5:but Doctor Watson has to have it taken out for him and dusted,
6:and exhibited clearly, with a label attached.
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn invert_context() {
        let matcher = RegexMatcher::new(r"Sherlock").unwrap();
        let mut printer = StandardBuilder::new().build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(true)
            .invert_match(true)
            .before_context(1)
            .after_context(1)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
1-For the Doctor Watsons of this world, as opposed to the Sherlock
2:Holmeses, success in the province of detective work must always
3-be, to a very large extent, the result of luck. Sherlock Holmes
4:can extract a clew from a wisp of straw or a flake of cigar ash;
5:but Doctor Watson has to have it taken out for him and dusted,
6:and exhibited clearly, with a label attached.
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn invert_context_multi_line() {
        let matcher = RegexMatcher::new(r"(?s:.{0})Sherlock").unwrap();
        let mut printer = StandardBuilder::new().build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .multi_line(true)
            .line_number(true)
            .invert_match(true)
            .before_context(1)
            .after_context(1)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
1-For the Doctor Watsons of this world, as opposed to the Sherlock
2:Holmeses, success in the province of detective work must always
3-be, to a very large extent, the result of luck. Sherlock Holmes
4:can extract a clew from a wisp of straw or a flake of cigar ash;
5:but Doctor Watson has to have it taken out for him and dusted,
6:and exhibited clearly, with a label attached.
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn invert_context_only_matching() {
        let matcher = RegexMatcher::new(r"Sherlock").unwrap();
        let mut printer = StandardBuilder::new()
            .only_matching(true)
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .line_number(true)
            .invert_match(true)
            .before_context(1)
            .after_context(1)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
1-Sherlock
2:Holmeses, success in the province of detective work must always
3-Sherlock
4:can extract a clew from a wisp of straw or a flake of cigar ash;
5:but Doctor Watson has to have it taken out for him and dusted,
6:and exhibited clearly, with a label attached.
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn invert_context_only_matching_multi_line() {
        let matcher = RegexMatcher::new(r"(?s:.{0})Sherlock").unwrap();
        let mut printer = StandardBuilder::new()
            .only_matching(true)
            .build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .multi_line(true)
            .line_number(true)
            .invert_match(true)
            .before_context(1)
            .after_context(1)
            .build()
            .search_reader(
                &matcher,
                SHERLOCK.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "\
1-Sherlock
2:Holmeses, success in the province of detective work must always
3-Sherlock
4:can extract a clew from a wisp of straw or a flake of cigar ash;
5:but Doctor Watson has to have it taken out for him and dusted,
6:and exhibited clearly, with a label attached.
";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn regression_search_empty_with_crlf() {
        let matcher =
            RegexMatcherBuilder::new().crlf(true).build(r"x?").unwrap();
        let mut printer = StandardBuilder::new()
            .color_specs(ColorSpecs::default_with_color())
            .build(Ansi::new(vec![]));
        SearcherBuilder::new()
            .line_terminator(LineTerminator::crlf())
            .build()
            .search_reader(&matcher, &b"\n"[..], printer.sink(&matcher))
            .unwrap();

        let got = printer_contents_ansi(&mut printer);
        assert!(!got.is_empty());
    }

    #[test]
    fn regression_after_context_with_match() {
        let haystack = "\
a
b
c
d
e
d
e
d
e
d
e
";

        let matcher = RegexMatcherBuilder::new().build(r"d").unwrap();
        let mut printer = StandardBuilder::new().build(NoColor::new(vec![]));
        SearcherBuilder::new()
            .max_matches(Some(1))
            .line_number(true)
            .after_context(2)
            .build()
            .search_reader(
                &matcher,
                haystack.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();

        let got = printer_contents(&mut printer);
        let expected = "4:d\n5-e\n6:d\n";
        assert_eq_printed!(expected, got);
    }

    #[test]
    fn regression_crlf_preserve() {
        let haystack = "hello\nworld\r\n";
        let matcher =
            RegexMatcherBuilder::new().crlf(true).build(r".").unwrap();
        let mut printer = StandardBuilder::new().build(NoColor::new(vec![]));
        let mut searcher = SearcherBuilder::new()
            .line_number(false)
            .line_terminator(LineTerminator::crlf())
            .build();

        searcher
            .search_reader(
                &matcher,
                haystack.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();
        let got = printer_contents(&mut printer);
        let expected = "hello\nworld\r\n";
        assert_eq_printed!(expected, got);

        let mut printer = StandardBuilder::new()
            .replacement(Some(b"$0".to_vec()))
            .build(NoColor::new(vec![]));
        searcher
            .search_reader(
                &matcher,
                haystack.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();
        let got = printer_contents(&mut printer);
        let expected = "hello\nworld\r\n";
        assert_eq_printed!(expected, got);
    }
}
