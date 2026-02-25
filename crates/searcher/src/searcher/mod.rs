use std::{
    cell::RefCell,
    cmp,
    fs::File,
    io::{self, Read},
    path::Path,
};

use {
    encoding_rs_io::DecodeReaderBytesBuilder,
    grep_matcher::{LineTerminator, Match, Matcher},
};

use crate::{
    line_buffer::{
        self, BufferAllocation, DEFAULT_BUFFER_CAPACITY, LineBuffer,
        LineBufferBuilder, LineBufferReader, alloc_error,
    },
    searcher::glue::{MultiLine, ReadByLine, SliceByLine},
    sink::{Sink, SinkError},
};

pub use self::mmap::MmapChoice;

mod core;
mod glue;
mod mmap;

/// Мы используем этот псевдоним типа, поскольку нам нужна эргономика типа
/// `Match` матчера, но на практике мы используем его для произвольных
/// диапазонов, поэтому дадим ему более точное имя. Это используется только
/// во внутренних механизмах поисковика.
type Range = Match;

/// Поведение обнаружения двоичных данных при поиске.
///
/// Обнаружение двоичных данных — это процесс _эвристического_ определения
/// того, является ли данный фрагмент данных двоичным или нет, а затем
/// выполнение действия на основе результата этой эвристики. Мотивация
/// обнаружения двоичных данных заключается в том, что двоичные данные часто
/// указывают на данные, которые нежелательно искать с помощью текстовых
/// шаблонов. Конечно, есть много случаев, когда это не так, поэтому
/// обнаружение двоичных данных по умолчанию отключено.
///
/// К сожалению, обнаружение двоичных данных работает по-разному в
/// зависимости от типа выполняемого поиска:
///
/// 1. При выполнении поиска с использованием буфера фиксированного размера
///    обнаружение двоичных данных применяется к содержимому буфера по мере
///    его заполнения. Обнаружение двоичных данных должно применяться
///    непосредственно к буферу, потому что двоичные файлы могут не содержать
///    завершителей строк, что может привести к чрезмерному использованию
///    памяти.
/// 2. При выполнении поиска с использованием отображений памяти или чтении
///    данных из кучи обнаружение двоичных данных более консервативно. А
///    именно, только область фиксированного размера в начале содержимого
///    проверяется на наличие двоичных данных. Когда включён режим `Quit`,
///    первые несколько КБ данных проверяются на наличие двоичных данных.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BinaryDetection(line_buffer::BinaryDetection);

impl BinaryDetection {
    /// Обнаружение двоичных данных не выполняется. Данные, сообщаемые
    /// поисковиком, могут содержать произвольные байты.
    ///
    /// Это значение по умолчанию.
    pub fn none() -> BinaryDetection {
        BinaryDetection(line_buffer::BinaryDetection::None)
    }

    /// Обнаружение двоичных данных выполняется путём поиска заданного байта.
    ///
    /// Когда поиск выполняется с использованием буфера фиксированного размера,
    /// содержимое этого буфера всегда проверяется на наличие этого байта.
    /// Если он найден, то основные данные считаются двоичными, и поиск
    /// останавливается, как если бы был достигнут EOF.
    ///
    /// Когда поиск выполняется с отображением всего содержимого в память,
    /// обнаружение двоичных данных более консервативно. А именно, только
    /// область фиксированного размера в начале содержимого проверяется на
    /// наличие двоичных данных. В качестве компромисса любые последующие
    /// совпадающие (или контекстные) строки также проверяются на наличие
    /// двоичных данных. Если в любой момент обнаруживаются двоичные данные,
    /// поиск останавливается, как если бы был достигнут EOF.
    pub fn quit(binary_byte: u8) -> BinaryDetection {
        BinaryDetection(line_buffer::BinaryDetection::Quit(binary_byte))
    }

    /// Обнаружение двоичных данных выполняется путём поиска заданного байта
    /// и замены его на завершитель строк, настроенный в поисковике.
    /// (Если поисковик настроен на использование `CRLF` в качестве
    /// завершителя строк, то этот байт заменяется только на `LF`.)
    ///
    /// Когда поиск выполняется с использованием буфера фиксированного размера,
    /// содержимое этого буфера всегда проверяется на наличие этого байта и
    /// заменяется завершителем строк. По сути, вызывающей стороне
    /// гарантируется, что она никогда не увидит этот байт во время поиска.
    ///
    /// Когда поиск выполняется с отображением всего содержимого в память,
    /// эта настройка не имеет эффекта и игнорируется.
    pub fn convert(binary_byte: u8) -> BinaryDetection {
        BinaryDetection(line_buffer::BinaryDetection::Convert(binary_byte))
    }

    /// Если это обнаружение двоичных данных использует стратегию "quit",
    /// то возвращается байт, который приведёт к завершению поиска.
    /// В любом другом случае возвращается `None`.
    pub fn quit_byte(&self) -> Option<u8> {
        match self.0 {
            line_buffer::BinaryDetection::Quit(b) => Some(b),
            _ => None,
        }
    }

    /// Если это обнаружение двоичных данных использует стратегию "convert",
    /// то возвращается байт, который будет заменён завершителем строк.
    /// В любом другом случае возвращается `None`.
    pub fn convert_byte(&self) -> Option<u8> {
        match self.0 {
            line_buffer::BinaryDetection::Convert(b) => Some(b),
            _ => None,
        }
    }
}

/// Кодировка для использования при поиске.
///
/// Кодировка может использоваться для настройки [`SearcherBuilder`] для
/// транскодирования исходных данных из кодировки в UTF-8 перед поиском.
///
/// Клонирование `Encoding` всегда дёшево.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Encoding(&'static encoding_rs::Encoding);

impl Encoding {
    /// Создать новую кодировку для указанной метки.
    ///
    /// Предоставленная метка кодировки сопоставляется с кодировкой через
    /// набор доступных вариантов, указанных в
    /// [Encoding Standard](https://encoding.spec.whatwg.org/#concept-encoding-get).
    /// Если данная метка не соответствует допустимой кодировке, то
    /// возвращается ошибка.
    pub fn new(label: &str) -> Result<Encoding, ConfigError> {
        let label = label.as_bytes();
        match encoding_rs::Encoding::for_label_no_replacement(label) {
            Some(encoding) => Ok(Encoding(encoding)),
            None => {
                Err(ConfigError::UnknownEncoding { label: label.to_vec() })
            }
        }
    }
}

/// Внутренняя конфигурация поисковика. Она используется несколькими типами,
/// связанными с поиском, но записывается в неё только SearcherBuilder.
#[derive(Clone, Debug)]
pub struct Config {
    /// Завершитель строк для использования.
    line_term: LineTerminator,
    /// Инвертировать ли сопоставление.
    invert_match: bool,
    /// Количество строк после совпадения для включения.
    after_context: usize,
    /// Количество строк перед совпадением для включения.
    before_context: usize,
    /// Включать ли неограниченный контекст или нет.
    passthru: bool,
    /// Подсчитывать ли номера строк.
    line_number: bool,
    /// Максимальный объём памяти кучи для использования.
    ///
    /// Если не указано, явное ограничение не применяется. При установке
    /// в `0` доступна только стратегия поиска с отображением в память.
    heap_limit: Option<usize>,
    /// Стратегия отображения в память.
    mmap: MmapChoice,
    /// Стратегия обнаружения двоичных данных.
    binary: BinaryDetection,
    /// Включать ли сопоставление по нескольким строкам.
    multi_line: bool,
    /// Кодировка, которая при наличии заставляет поисковик транскодировать
    /// все входные данные из кодировки в UTF-8.
    encoding: Option<Encoding>,
    /// Выполнять ли автоматическое транскодирование на основе BOM или нет.
    bom_sniffing: bool,
    /// Останавливать ли поиск, когда найдена несовпадающая строка после
    /// совпадающей строки.
    stop_on_nonmatch: bool,
    /// Максимальное количество совпадений, которое должен выдать этот
    /// поисковик.
    max_matches: Option<u64>,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            line_term: LineTerminator::default(),
            invert_match: false,
            after_context: 0,
            before_context: 0,
            passthru: false,
            line_number: true,
            heap_limit: None,
            mmap: MmapChoice::default(),
            binary: BinaryDetection::default(),
            multi_line: false,
            encoding: None,
            bom_sniffing: true,
            stop_on_nonmatch: false,
            max_matches: None,
        }
    }
}

impl Config {
    /// Return the maximal amount of lines needed to fulfill this
    /// configuration's context.
    ///
    /// If this returns `0`, then no context is ever needed.
    fn max_context(&self) -> usize {
        cmp::max(self.before_context, self.after_context)
    }

    /// Build a line buffer from this configuration.
    fn line_buffer(&self) -> LineBuffer {
        let mut builder = LineBufferBuilder::new();
        builder
            .line_terminator(self.line_term.as_byte())
            .binary_detection(self.binary.0);

        if let Some(limit) = self.heap_limit {
            let (capacity, additional) = if limit <= DEFAULT_BUFFER_CAPACITY {
                (limit, 0)
            } else {
                (DEFAULT_BUFFER_CAPACITY, limit - DEFAULT_BUFFER_CAPACITY)
            };
            builder
                .capacity(capacity)
                .buffer_alloc(BufferAllocation::Error(additional));
        }
        builder.build()
    }
}

/// Ошибка, которая может возникнуть при создании поисковика.
///
/// Эта ошибка возникает, когда присутствует бессмысленная конфигурация
/// при попытке сконструировать `Searcher` из `SearcherBuilder`.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ConfigError {
    /// Указывает, что конфигурация ограничения кучи предотвращает
    /// использование всех возможных стратегий поиска. Например, если
    /// ограничение кучи установлено в 0, а поиск с отображением в память
    /// отключен или недоступен.
    SearchUnavailable,
    /// Возникает, когда матчер сообщает о завершителе строк, отличном от
    /// настроенного в поисковике.
    MismatchedLineTerminators {
        /// Завершитель строк матчера.
        matcher: LineTerminator,
        /// Завершитель строк поисковика.
        searcher: LineTerminator,
    },
    /// Возникает, когда не удалось найти кодировку для определённой метки.
    UnknownEncoding {
        /// Предоставленная метка кодировки, которую не удалось найти.
        label: Vec<u8>,
    },
}

impl std::error::Error for ConfigError {}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            ConfigError::SearchUnavailable => {
                write!(f, "grep config error: no available searchers")
            }
            ConfigError::MismatchedLineTerminators { matcher, searcher } => {
                write!(
                    f,
                    "grep config error: mismatched line terminators, \
                     matcher has {:?} but searcher has {:?}",
                    matcher, searcher
                )
            }
            ConfigError::UnknownEncoding { ref label } => write!(
                f,
                "grep config error: unknown encoding: {}",
                String::from_utf8_lossy(label),
            ),
        }
    }
}

/// Конструктор для настройки поисковика.
///
/// Конструктор поиска позволяет указать конфигурацию поисковика,
/// включая такие опции, как инвертирование поиска или включение
/// поиска по нескольким строкам.
///
/// После создания поисковика полезно повторно использовать этот
/// поисковик для нескольких поисков, если это возможно.
#[derive(Clone, Debug)]
pub struct SearcherBuilder {
    config: Config,
}

impl Default for SearcherBuilder {
    fn default() -> SearcherBuilder {
        SearcherBuilder::new()
    }
}

impl SearcherBuilder {
    /// Создать новый конструктор поисковика с конфигурацией по умолчанию.
    pub fn new() -> SearcherBuilder {
        SearcherBuilder { config: Config::default() }
    }

    /// Построить поисковик с данным матчером.
    pub fn build(&self) -> Searcher {
        let mut config = self.config.clone();
        if config.passthru {
            config.before_context = 0;
            config.after_context = 0;
        }

        let mut decode_builder = DecodeReaderBytesBuilder::new();
        decode_builder
            .encoding(self.config.encoding.as_ref().map(|e| e.0))
            .utf8_passthru(true)
            .strip_bom(self.config.bom_sniffing)
            .bom_override(true)
            .bom_sniffing(self.config.bom_sniffing);

        Searcher {
            config,
            decode_builder,
            decode_buffer: RefCell::new(vec![0; 8 * (1 << 10)]),
            line_buffer: RefCell::new(self.config.line_buffer()),
            multi_line_buffer: RefCell::new(vec![]),
        }
    }

    /// Установить завершитель строк, используемый поисковиком.
    ///
    /// При использовании поисковика, если предоставленный матчер имеет
    /// установленный завершитель строк, то он должен совпадать с этим.
    /// Если они не совпадают, создание поисковика вернёт ошибку.
    ///
    /// По умолчанию установлено значение `b'\n'`.
    pub fn line_terminator(
        &mut self,
        line_term: LineTerminator,
    ) -> &mut SearcherBuilder {
        self.config.line_term = line_term;
        self
    }

    /// Инвертировать ли сопоставление, при котором строки, не совпадающие
    /// с шаблоном, сообщаются вместо сообщения о совпадающих строках.
    ///
    /// По умолчанию это отключено.
    pub fn invert_match(&mut self, yes: bool) -> &mut SearcherBuilder {
        self.config.invert_match = yes;
        self
    }

    /// Подсчитывать и включать ли номера строк с совпадающими строками.
    ///
    /// Это включено по умолчанию. Вычисление номеров строк связано с
    /// небольшой потерей производительности, поэтому это можно отключить,
    /// когда это нежелательно.
    pub fn line_number(&mut self, yes: bool) -> &mut SearcherBuilder {
        self.config.line_number = yes;
        self
    }

    /// Включать ли поиск по нескольким строкам или нет.
    ///
    /// Когда поиск по нескольким строкам включён, совпадения *могут*
    /// охватывать несколько строк. И наоборот, когда поиск по нескольким
    /// строкам отключён, невозможно, чтобы какое-либо совпадение охватывало
    /// более одной строки.
    ///
    /// **Предупреждение:** поиск по нескольким строкам требует, чтобы всё
    /// содержимое для поиска было отображено в памяти одновременно. При
    /// поиске файлов будут использоваться отображения памяти, если это
    /// возможно и если они включены, что позволяет избежать использования
    /// кучи вашей программы. Однако, если отображения памяти не могут быть
    /// использованы (например, для поиска потоков, таких как `stdin`, или
    /// если необходимо транскодирование), то всё содержимое потока
    /// считывается в кучу перед началом поиска.
    ///
    /// По умолчанию это отключено.
    pub fn multi_line(&mut self, yes: bool) -> &mut SearcherBuilder {
        self.config.multi_line = yes;
        self
    }

    /// Включать ли фиксированное количество строк после каждого совпадения.
    ///
    /// Когда это установлено в ненулевое число, поисковик будет сообщать
    /// `line_count` контекстных строк после каждого совпадения.
    ///
    /// По умолчанию установлено значение `0`.
    pub fn after_context(
        &mut self,
        line_count: usize,
    ) -> &mut SearcherBuilder {
        self.config.after_context = line_count;
        self
    }

    /// Включать ли фиксированное количество строк перед каждым совпадением.
    ///
    /// Когда это установлено в ненулевое число, поисковик будет сообщать
    /// `line_count` контекстных строк перед каждым совпадением.
    ///
    /// По умолчанию установлено значение `0`.
    pub fn before_context(
        &mut self,
        line_count: usize,
    ) -> &mut SearcherBuilder {
        self.config.before_context = line_count;
        self
    }

    /// Включать ли функцию "passthru" или нет.
    ///
    /// Когда passthru включён, он фактически обрабатывает все несовпадающие
    /// строки как контекстные. Другими словами, включение этого аналогично
    /// запросу неограниченного количества контекстных строк до и после.
    ///
    /// Когда режим passthru включён, любые настройки `before_context` или
    /// `after_context` игнорируются путём установки их в `0`.
    ///
    /// По умолчанию это отключено.
    pub fn passthru(&mut self, yes: bool) -> &mut SearcherBuilder {
        self.config.passthru = yes;
        self
    }

    /// Установить приблизительное ограничение на объём памяти кучи,
    /// используемой поисковиком.
    ///
    /// Ограничение кучи применяется в двух сценариях:
    ///
    /// * При поиске с использованием буфера фиксированного размера
    ///   ограничение кучи контролирует, насколько большим может быть
    ///   этот буфер. Предполагая, что контекст отключён, минимальный
    ///   размер этого буфера — это длина (в байтах) самой большой
    ///   отдельной строки в содержимом, которое ищется. Если какая-либо
    ///   строка превышает ограничение кучи, будет возвращена ошибка.
    /// * При выполнении поиска по нескольким строкам буфер фиксированного
    ///   размера не может быть использован. Таким образом, единственный
    ///   выбор — прочитать всё содержимое в кучу или использовать
    ///   отображения памяти. В первом случае применяется установленное
    ///   здесь ограничение кучи.
    ///
    /// Если ограничение кучи установлено в `0`, то пространство кучи не
    /// используется. Если нет доступных альтернативных стратегий для поиска
    /// без пространства кучи (например, отображения памяти отключены), то
    /// поисковик немедленно вернёт ошибку.
    ///
    /// По умолчанию ограничение не установлено.
    pub fn heap_limit(
        &mut self,
        bytes: Option<usize>,
    ) -> &mut SearcherBuilder {
        self.config.heap_limit = bytes;
        self
    }

    /// Установить стратегию использования отображений памяти.
    ///
    /// В настоящее время можно использовать только две стратегии:
    ///
    /// * **Automatic** — поисковик будет использовать эвристики, включая,
    ///   но не ограничиваясь размером файла и платформой, для определения,
    ///   использовать ли отображения памяти или нет.
    /// * **Never** — отображения памяти никогда не будут использоваться.
    ///   Если включён поиск по нескольким строкам, то всё содержимое будет
    ///   прочитано в кучу перед началом поиска.
    ///
    /// Поведение по умолчанию — **never**. Вообще говоря и, возможно,
    /// вопреки общепринятому мнению, отображения памяти не обязательно
    /// обеспечивают более быстрый поиск. Например, в зависимости от
    /// платформы, использование отображений памяти при поиске большого
    /// каталога может быть значительно медленнее, чем использование
    /// обычных вызовов чтения из-за накладных расходов на управление
    /// отображениями памяти.
    ///
    /// Однако в некоторых случаях отображения памяти могут быть быстрее.
    /// На некоторых платформах при поиске очень большого файла, который
    /// *уже находится в памяти*, может быть немного быстрее искать его
    /// как отображение памяти вместо использования обычных вызовов чтения.
    ///
    /// Наконец, отображения памяти имеют несколько сложную историю
    /// безопасности в Rust. Если вы не уверены, стоит ли включать
    /// отображения памяти, то просто не беспокойтесь об этом.
    ///
    /// **ПРЕДУПРЕЖДЕНИЕ**: Если ваш процесс ищет отображение в память
    /// файла, и в то же время этот файл усекается, то возможно, что
    /// процесс завершится с ошибкой bus error.
    pub fn memory_map(
        &mut self,
        strategy: MmapChoice,
    ) -> &mut SearcherBuilder {
        self.config.mmap = strategy;
        self
    }

    /// Установить стратегию обнаружения двоичных данных.
    ///
    /// Стратегия обнаружения двоичных данных определяет не только то,
    /// как поисковик обнаруживает двоичные данные, но и как он реагирует
    /// на наличие двоичных данных. Дополнительную информацию см. в типе
    /// [`BinaryDetection`].
    ///
    /// По умолчанию обнаружение двоичных данных отключено.
    pub fn binary_detection(
        &mut self,
        detection: BinaryDetection,
    ) -> &mut SearcherBuilder {
        self.config.binary = detection;
        self
    }

    /// Установить кодировку, используемую для чтения исходных данных
    /// перед поиском.
    ///
    /// Когда кодировка предоставлена, исходные данные _безусловно_
    /// транскодируются с использованием этой кодировки, если не
    /// присутствует BOM. Если BOM присутствует, то вместо этого
    /// используется кодировка, указанная BOM. Если процесс
    /// транскодирования встречает ошибку, то байты заменяются
    /// символом замены Unicode.
    ///
    /// Когда кодировка не указана (по умолчанию), используется
    /// обнаружение BOM (если оно включено, а оно включено по
    /// умолчанию) для определения, являются ли исходные данные
    /// UTF-8 или UTF-16, и транскодирование будет выполнено
    /// автоматически. Если BOM не найден, то исходные данные
    /// ищутся _как если бы_ они были UTF-8. Однако, пока исходные
    /// данные хотя бы ASCII-совместимы, возможно, что поиск даст
    /// полезные результаты.
    pub fn encoding(
        &mut self,
        encoding: Option<Encoding>,
    ) -> &mut SearcherBuilder {
        self.config.encoding = encoding;
        self
    }

    /// Включить автоматическое транскодирование на основе обнаружения BOM.
    ///
    /// Когда это включено и явная кодировка не установлена, этот
    /// поисковик попытается обнаружить кодировку байтов, которые
    /// ищутся, путём обнаружения его байтовой метки порядка (BOM).
    /// В частности, когда это включено, файлы в кодировке UTF-16
    /// будут искаться бесшовно.
    ///
    /// Когда это отключено и если явная кодировка не установлена,
    /// то байты из исходного потока будут переданы без изменений,
    /// включая его BOM, если он присутствует.
    ///
    /// Это включено по умолчанию.
    pub fn bom_sniffing(&mut self, yes: bool) -> &mut SearcherBuilder {
        self.config.bom_sniffing = yes;
        self
    }

    /// Останавливать поиск файла, когда найдена несовпадающая строка
    /// после совпадающей строки.
    ///
    /// Это полезно для поиска отсортированных файлов, где ожидается,
    /// что все совпадения будут на соседних строках.
    pub fn stop_on_nonmatch(
        &mut self,
        stop_on_nonmatch: bool,
    ) -> &mut SearcherBuilder {
        self.config.stop_on_nonmatch = stop_on_nonmatch;
        self
    }

    /// Устанавливает максимальное количество совпадений, которое
    /// должен выдать этот поисковик.
    ///
    /// Если включён поиск по нескольким строкам и совпадение
    /// охватывает несколько строк, то это совпадение считается
    /// ровно один раз для целей соблюдения этого ограничения,
    /// независимо от того, сколько строк оно охватывает.
    ///
    /// Обратите внимание, что `0` является допустимым значением.
    /// Это приведёт к немедленному завершению поисковика без
    /// выполнения какого-либо поиска.
    ///
    /// По умолчанию ограничение не установлено.
    #[inline]
    pub fn max_matches(&mut self, limit: Option<u64>) -> &mut SearcherBuilder {
        self.config.max_matches = limit;
        self
    }
}

/// Поисковик выполняет поиск по haystack и записывает результаты
/// в предоставленный вызывающей стороной sink.
///
/// Совпадения обнаруживаются через реализации трейта `Matcher`,
/// которые должны быть предоставлены вызывающей стороной при
/// выполнении поиска.
///
/// Когда это возможно, поисковик следует использовать повторно.
#[derive(Clone, Debug)]
pub struct Searcher {
    /// Конфигурация для этого поисковика.
    ///
    /// Мы делаем большинство этих настроек доступными для пользователей
    /// `Searcher` через методы публичного API, которые могут быть
    /// запрошены в реализациях `Sink`, если это необходимо.
    config: Config,
    /// Конструктор для создания потокового читателя, который транскодирует
    /// исходные данные согласно либо явно указанной кодировке, либо
    /// автоматически обнаруженной кодировке через обнаружение BOM.
    ///
    /// Когда транскодирование не требуется, построенный транскодер будет
    /// передавать основные байты без дополнительных накладных расходов.
    decode_builder: DecodeReaderBytesBuilder,
    /// Буфер, используемый для рабочего пространства транскодирования.
    decode_buffer: RefCell<Vec<u8>>,
    /// Буфер строк для использования при поиске, ориентированном на строки.
    ///
    /// Мы оборачиваем его в RefCell, чтобы позволить передавать заимствования
    /// `Searcher` в sink. Нам всё ещё требуется изменяемое заимствование
    /// для выполнения поиска, поэтому мы статически предотвращаем вызов
    /// паники RefCell во время выполнения из-за нарушения заимствования.
    line_buffer: RefCell<LineBuffer>,
    /// Буфер, в котором хранится содержимое читателя при выполнении
    /// поиска по нескольким строкам. В частности, поиск по нескольким
    /// строкам не может выполняться инкрементально и требует, чтобы
    /// весь haystack находился в памяти одновременно.
    multi_line_buffer: RefCell<Vec<u8>>,
}

impl Searcher {
    /// Создать новый поисковик с конфигурацией по умолчанию.
    ///
    /// Для настройки поисковика (например, инвертирование сопоставления,
    /// включение отображений памяти, включение контекстов и т.д.)
    /// используйте [`SearcherBuilder`].
    pub fn new() -> Searcher {
        SearcherBuilder::new().build()
    }

    /// Выполнить поиск по файлу с данным путём и записать результаты
    /// в данный sink.
    ///
    /// Если отображения памяти включены и поисковик эвристически полагает,
    /// что отображения памяти помогут поиску работать быстрее, то будут
    /// использованы отображения памяти. По этой причине вызывающим сторонам
    /// следует предпочесть использование этого метода или `search_file`
    /// перед более общим `search_reader`, когда это возможно.
    pub fn search_path<P, M, S>(
        &mut self,
        matcher: M,
        path: P,
        write_to: S,
    ) -> Result<(), S::Error>
    where
        P: AsRef<Path>,
        M: Matcher,
        S: Sink,
    {
        let path = path.as_ref();
        let file = File::open(path).map_err(S::Error::error_io)?;
        self.search_file_maybe_path(matcher, Some(path), &file, write_to)
    }

    /// Выполнить поиск по файлу и записать результаты в данный sink.
    ///
    /// Если отображения памяти включены и поисковик эвристически полагает,
    /// что отображения памяти помогут поиску работать быстрее, то будут
    /// использованы отображения памяти. По этой причине вызывающим сторонам
    /// следует предпочесть использование этого метода или `search_path`
    /// перед более общим `search_reader`, когда это возможно.
    pub fn search_file<M, S>(
        &mut self,
        matcher: M,
        file: &File,
        write_to: S,
    ) -> Result<(), S::Error>
    where
        M: Matcher,
        S: Sink,
    {
        self.search_file_maybe_path(matcher, None, file, write_to)
    }

    fn search_file_maybe_path<M, S>(
        &mut self,
        matcher: M,
        path: Option<&Path>,
        file: &File,
        write_to: S,
    ) -> Result<(), S::Error>
    where
        M: Matcher,
        S: Sink,
    {
        if let Some(mmap) = self.config.mmap.open(file, path) {
            log::trace!("{:?}: поиск через отображение в память", path);
            return self.search_slice(matcher, &mmap, write_to);
        }
        // Быстрый путь для поиска по нескольким строкам файлов, когда
        // отображения памяти не включены. Это предварительно выделяет
        // буфер примерно размером с файл, что невозможно при поиске
        // произвольного std::io::Read.
        if self.multi_line_with_matcher(&matcher) {
            log::trace!(
                "{:?}: чтение всего файла в кучу для multiline",
                path
            );
            self.fill_multi_line_buffer_from_file::<S>(file)?;
            log::trace!("{:?}: поиск через стратегию multiline", path);
            MultiLine::new(
                self,
                matcher,
                &*self.multi_line_buffer.borrow(),
                write_to,
            )
            .run()
        } else {
            log::trace!("{:?}: поиск с использованием универсального reader", path);
            self.search_reader(matcher, file, write_to)
        }
    }

    /// Выполнить поиск по любой реализации `std::io::Read` и записать
    /// результаты в данный sink.
    ///
    /// Когда это возможно, эта реализация будет искать читателя
    /// инкрементально, не читая его в память. В некоторых случаях —
    /// например, если включён поиск по нескольким строкам —
    /// инкрементальный поиск невозможен, и данный читатель полностью
    /// потребляется и помещается в кучу перед началом поиска. По этой
    /// причине, когда включён поиск по нескольким строкам, следует
    /// попытаться использовать API более высокого уровня (например,
    /// поиск по файлу или пути к файлу), чтобы отображения памяти
    /// могли быть использованы, если они доступны и включены.
    pub fn search_reader<M, R, S>(
        &mut self,
        matcher: M,
        read_from: R,
        write_to: S,
    ) -> Result<(), S::Error>
    where
        M: Matcher,
        R: io::Read,
        S: Sink,
    {
        self.check_config(&matcher).map_err(S::Error::error_config)?;

        let mut decode_buffer = self.decode_buffer.borrow_mut();
        let decoder = self
            .decode_builder
            .build_with_buffer(read_from, &mut *decode_buffer)
            .map_err(S::Error::error_io)?;

        if self.multi_line_with_matcher(&matcher) {
            log::trace!(
                "generic reader: чтение всего в кучу для multiline"
            );
            self.fill_multi_line_buffer_from_reader::<_, S>(decoder)?;
            log::trace!("generic reader: поиск через стратегию multiline");
            MultiLine::new(
                self,
                matcher,
                &*self.multi_line_buffer.borrow(),
                write_to,
            )
            .run()
        } else {
            let mut line_buffer = self.line_buffer.borrow_mut();
            let rdr = LineBufferReader::new(decoder, &mut *line_buffer);
            log::trace!("generic reader: поиск через стратегию roll buffer");
            ReadByLine::new(self, matcher, rdr, write_to).run()
        }
    }

    /// Выполнить поиск по данному срезу и записать результаты в данный sink.
    pub fn search_slice<M, S>(
        &mut self,
        matcher: M,
        slice: &[u8],
        write_to: S,
    ) -> Result<(), S::Error>
    where
        M: Matcher,
        S: Sink,
    {
        self.check_config(&matcher).map_err(S::Error::error_config)?;

        // Мы можем искать срез напрямую, если нам не нужно выполнять
        // транскодирование.
        if self.slice_needs_transcoding(slice) {
            log::trace!(
                "slice reader: требуется транскодирование, используем generic reader"
            );
            return self.search_reader(matcher, slice, write_to);
        }
        if self.multi_line_with_matcher(&matcher) {
            log::trace!("slice reader: поиск через стратегию multiline");
            MultiLine::new(self, matcher, slice, write_to).run()
        } else {
            log::trace!("slice reader: поиск через стратегию slice-by-line");
            SliceByLine::new(self, matcher, slice, write_to).run()
        }
    }

    /// Установить метод обнаружения двоичных данных, используемый этим
    /// поисковиком.
    pub fn set_binary_detection(&mut self, detection: BinaryDetection) {
        self.config.binary = detection.clone();
        self.line_buffer.borrow_mut().set_binary_detection(detection.0);
    }

    /// Проверить, что конфигурация поисковика и матчер согласованы
    /// друг с другом.
    fn check_config<M: Matcher>(&self, matcher: M) -> Result<(), ConfigError> {
        if self.config.heap_limit == Some(0) && !self.config.mmap.is_enabled()
        {
            return Err(ConfigError::SearchUnavailable);
        }
        let matcher_line_term = match matcher.line_terminator() {
            None => return Ok(()),
            Some(line_term) => line_term,
        };
        if matcher_line_term != self.config.line_term {
            return Err(ConfigError::MismatchedLineTerminators {
                matcher: matcher_line_term,
                searcher: self.config.line_term,
            });
        }
        Ok(())
    }

    /// Возвращает true тогда и только тогда, когда данный срез нуждается
    /// в транскодировании.
    fn slice_needs_transcoding(&self, slice: &[u8]) -> bool {
        self.config.encoding.is_some()
            || (self.config.bom_sniffing && slice_has_bom(slice))
    }
}

/// Следующие методы позволяют запрашивать конфигурацию поисковика.
/// Они могут быть полезны в универсальных реализациях [`Sink`], где
/// вывод может быть настроен в зависимости от того, как настроен
/// поисковик.
impl Searcher {
    /// Возвращает завершитель строк, используемый этим поисковиком.
    #[inline]
    pub fn line_terminator(&self) -> LineTerminator {
        self.config.line_term
    }

    /// Возвращает тип обнаружения двоичных данных, настроенный в этом
    /// поисковике.
    #[inline]
    pub fn binary_detection(&self) -> &BinaryDetection {
        &self.config.binary
    }

    /// Возвращает true тогда и только тогда, когда этот поисковик
    /// настроен на инвертирование результатов поиска. То есть
    /// совпадающие строки — это строки, которые **не** совпадают
    /// с матчером поисковика.
    #[inline]
    pub fn invert_match(&self) -> bool {
        self.config.invert_match
    }

    /// Возвращает true тогда и только тогда, когда этот поисковик
    /// настроен на подсчёт номеров строк.
    #[inline]
    pub fn line_number(&self) -> bool {
        self.config.line_number
    }

    /// Возвращает true тогда и только тогда, когда этот поисковик
    /// настроен на выполнение поиска по нескольким строкам.
    #[inline]
    pub fn multi_line(&self) -> bool {
        self.config.multi_line
    }

    /// Возвращает true тогда и только тогда, когда этот поисковик
    /// настроен на остановку, когда находит несовпадающую строку
    /// после совпадающей.
    #[inline]
    pub fn stop_on_nonmatch(&self) -> bool {
        self.config.stop_on_nonmatch
    }

    /// Возвращает максимальное количество совпадений, выдаваемых
    /// этим поисковиком, если такое ограничение было установлено.
    ///
    /// Если включён поиск по нескольким строкам и совпадение
    /// охватывает несколько строк, то это совпадение считается
    /// ровно один раз для целей соблюдения этого ограничения,
    /// независимо от того, сколько строк оно охватывает.
    ///
    /// Обратите внимание, что `0` является допустимым значением.
    /// Это приведёт к немедленному завершению поисковика без
    /// выполнения какого-либо поиска.
    #[inline]
    pub fn max_matches(&self) -> Option<u64> {
        self.config.max_matches
    }

    /// Возвращает true тогда и только тогда, когда этот поисковик
    /// выберет стратегию для нескольких строк с данным матчером.
    ///
    /// Это может отличаться от результата `multi_line` в случаях,
    /// когда поисковик настроен на выполнение поиска, который может
    /// сообщать о совпадениях по нескольким строкам, но где матчер
    /// гарантирует, что он никогда не создаст совпадение по нескольким
    /// строкам.
    pub fn multi_line_with_matcher<M: Matcher>(&self, matcher: M) -> bool {
        if !self.multi_line() {
            return false;
        }
        if let Some(line_term) = matcher.line_terminator() {
            if line_term == self.line_terminator() {
                return false;
            }
        }
        if let Some(non_matching) = matcher.non_matching_bytes() {
            // Если завершитель строк — CRLF, нам на самом деле не нужно
            // заботиться о том, может ли regex сопоставить `\r` или нет.
            // А именно, `\r` не является ни необходимым, ни достаточным
            // для завершения строки. Всегда требуется `\n`.
            if non_matching.contains(self.line_terminator().as_byte()) {
                return false;
            }
        }
        true
    }

    /// Возвращает количество контекстных строк "after" для сообщения.
    /// Когда сообщение о контексте не включено, это возвращает `0`.
    #[inline]
    pub fn after_context(&self) -> usize {
        self.config.after_context
    }

    /// Возвращает количество контекстных строк "before" для сообщения.
    /// Когда сообщение о контексте не включено, это возвращает `0`.
    #[inline]
    pub fn before_context(&self) -> usize {
        self.config.before_context
    }

    /// Возвращает true тогда и только тогда, когда у поисковика
    /// включён режим "passthru".
    #[inline]
    pub fn passthru(&self) -> bool {
        self.config.passthru
    }

    /// Заполнить буфер для использования с поиском по нескольким строкам
    /// из данного файла. Это читает из файла до EOF или до возникновения
    /// ошибки. Если содержимое превышает настроенное ограничение кучи,
    /// то возвращается ошибка.
    fn fill_multi_line_buffer_from_file<S: Sink>(
        &self,
        file: &File,
    ) -> Result<(), S::Error> {
        assert!(self.config.multi_line);

        let mut decode_buffer = self.decode_buffer.borrow_mut();
        let mut read_from = self
            .decode_builder
            .build_with_buffer(file, &mut *decode_buffer)
            .map_err(S::Error::error_io)?;

        // Если у нас нет ограничения кучи, то мы можем передать управление
        // реализации std read_to_end. fill_multi_line_buffer_from_reader
        // сделает то же самое, но поскольку у нас есть File, мы можем
        // быть немного умнее при предварительном выделении здесь.
        //
        // Если мы транскодируем, то наше предварительное выделение
        // может быть не точным, но, вероятно, всё же лучше, чем ничего.
        if self.config.heap_limit.is_none() {
            let mut buf = self.multi_line_buffer.borrow_mut();
            buf.clear();
            let cap =
                file.metadata().map(|m| m.len() as usize + 1).unwrap_or(0);
            buf.reserve(cap);
            read_from.read_to_end(&mut *buf).map_err(S::Error::error_io)?;
            return Ok(());
        }
        self.fill_multi_line_buffer_from_reader::<_, S>(read_from)
    }

    /// Заполнить буфер для использования с поиском по нескольким строкам
    /// из данного читателя. Это читает из читателя до EOF или до
    /// возникновения ошибки. Если содержимое превышает настроенное
    /// ограничение кучи, то возвращается ошибка.
    fn fill_multi_line_buffer_from_reader<R: io::Read, S: Sink>(
        &self,
        mut read_from: R,
    ) -> Result<(), S::Error> {
        assert!(self.config.multi_line);

        let mut buf = self.multi_line_buffer.borrow_mut();
        buf.clear();

        // Если у нас нет ограничения кучи, то мы можем передать управление
        // реализации std read_to_end...
        let heap_limit = match self.config.heap_limit {
            Some(heap_limit) => heap_limit,
            None => {
                read_from
                    .read_to_end(&mut *buf)
                    .map_err(S::Error::error_io)?;
                return Ok(());
            }
        };
        if heap_limit == 0 {
            return Err(S::Error::error_io(alloc_error(heap_limit)));
        }

        // ... в противном случае нам нужно сделать это самим. Это,
        // вероятно, значительно медленнее, чем оптимально, но мы
        // избегаем беспокойства о безопасности памяти, пока не
        // появится веская причина ускорить это.
        buf.resize(cmp::min(DEFAULT_BUFFER_CAPACITY, heap_limit), 0);
        let mut pos = 0;
        loop {
            let nread = match read_from.read(&mut buf[pos..]) {
                Ok(nread) => nread,
                Err(ref err) if err.kind() == io::ErrorKind::Interrupted => {
                    continue;
                }
                Err(err) => return Err(S::Error::error_io(err)),
            };
            if nread == 0 {
                buf.resize(pos, 0);
                return Ok(());
            }

            pos += nread;
            if buf[pos..].is_empty() {
                let additional = heap_limit - buf.len();
                if additional == 0 {
                    return Err(S::Error::error_io(alloc_error(heap_limit)));
                }
                let limit = buf.len() + additional;
                let doubled = 2 * buf.len();
                buf.resize(cmp::min(doubled, limit), 0);
            }
        }
    }
}

/// Возвращает true тогда и только тогда, когда данный срез начинается
/// с UTF-8 или UTF-16 BOM.
///
/// Это используется поисковиком для определения, необходим ли
/// транскодер. В противном случае выгодно искать срез напрямую.
fn slice_has_bom(slice: &[u8]) -> bool {
    let enc = match encoding_rs::Encoding::for_bom(slice) {
        None => return false,
        Some((enc, _)) => enc,
    };
    log::trace!("обнаружена байтовая метка порядка (BOM) для кодировки {enc:?}");
    [encoding_rs::UTF_16LE, encoding_rs::UTF_16BE, encoding_rs::UTF_8]
        .contains(&enc)
}

#[cfg(test)]
mod tests {
    use crate::testutil::{KitchenSink, RegexMatcher};

    use super::*;

    #[test]
    fn config_error_heap_limit() {
        let matcher = RegexMatcher::new("");
        let sink = KitchenSink::new();
        let mut searcher = SearcherBuilder::new().heap_limit(Some(0)).build();
        let res = searcher.search_slice(matcher, &[], sink);
        assert!(res.is_err());
    }

    #[test]
    fn config_error_line_terminator() {
        let mut matcher = RegexMatcher::new("");
        matcher.set_line_term(Some(LineTerminator::byte(b'z')));

        let sink = KitchenSink::new();
        let mut searcher = Searcher::new();
        let res = searcher.search_slice(matcher, &[], sink);
        assert!(res.is_err());
    }

    #[test]
    fn uft8_bom_sniffing() {
        // См.: https://github.com/BurntSushi/ripgrep/issues/1638
        // ripgrep должен обнаруживать utf-8 BOM, как и utf-16
        let matcher = RegexMatcher::new("foo");
        let haystack: &[u8] = &[0xef, 0xbb, 0xbf, 0x66, 0x6f, 0x6f];

        let mut sink = KitchenSink::new();
        let mut searcher = SearcherBuilder::new().build();

        let res = searcher.search_slice(matcher, haystack, &mut sink);
        assert!(res.is_ok());

        let sink_output = String::from_utf8(sink.as_bytes().to_vec()).unwrap();
        assert_eq!(sink_output, "1:0:foo\nbyte count:3\n");
    }
}
