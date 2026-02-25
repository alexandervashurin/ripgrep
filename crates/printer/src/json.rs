use std::{
    io::{self, Write},
    path::Path,
    sync::Arc,
    time::Instant,
};

use {
    grep_matcher::{Match, Matcher},
    grep_searcher::{Searcher, Sink, SinkContext, SinkFinish, SinkMatch},
    serde_json as json,
};

use crate::{
    counter::CounterWriter, jsont, stats::Stats, util::Replacer,
    util::find_iter_at_in_context,
};

/// Конфигурация для JSON принтера.
///
/// Это управляется JSONBuilder, а затем используется фактической
/// реализацией. Как только принтер построен, конфигурация заморожена и
/// не может быть изменена.
#[derive(Debug, Clone)]
struct Config {
    pretty: bool,
    always_begin_end: bool,
    replacement: Arc<Option<Vec<u8>>>,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            pretty: false,
            always_begin_end: false,
            replacement: Arc::new(None),
        }
    }
}

/// Построитель для JSON lines принтера.
///
/// Построитель позволяет настраивать поведение принтера. JSON принтер
/// имеет меньше опций конфигурации, чем стандартный принтер, потому что он
/// является структурированным форматом, и принтер всегда пытается найти
/// как можно больше информации.
///
/// Некоторые опции конфигурации, такие как включение номеров строк или
/// отображение контекстных строк, берутся непосредственно из
/// конфигурации `grep_searcher::Searcher`.
///
/// Как только `JSON` принтер построен, его конфигурация не может быть изменена.
#[derive(Clone, Debug)]
pub struct JSONBuilder {
    config: Config,
}

impl JSONBuilder {
    /// Возвращает новый построитель для конфигурирования JSON принтера.
    pub fn new() -> JSONBuilder {
        JSONBuilder { config: Config::default() }
    }

    /// Создаёт JSON принтер, который записывает результаты в данный writer.
    pub fn build<W: io::Write>(&self, wtr: W) -> JSON<W> {
        JSON {
            config: self.config.clone(),
            wtr: CounterWriter::new(wtr),
            matches: vec![],
        }
    }

    /// Печатает JSON в красиво отформатированном виде.
    ///
    /// Включение этого режима больше не производит формат "JSON lines", в том смысле, что
    /// каждый печатаемый JSON объект может занимать несколько строк.
    ///
    /// Это отключено по умолчанию.
    pub fn pretty(&mut self, yes: bool) -> &mut JSONBuilder {
        self.config.pretty = yes;
        self
    }

    /// Когда включено, сообщения `begin` и `end` всегда выводятся, даже
    /// когда совпадение не найдено.
    ///
    /// Когда отключено, сообщения `begin` и `end` показываются только если есть
    /// хотя бы одно сообщение `match` или `context`.
    ///
    /// Это отключено по умолчанию.
    pub fn always_begin_end(&mut self, yes: bool) -> &mut JSONBuilder {
        self.config.always_begin_end = yes;
        self
    }

    /// Устанавливает байты, которые будут использоваться для замены каждого вхождения найденного совпадения.
    ///
    /// Байты замены могут включать ссылки на группы захвата,
    /// которые могут быть либо в индексной форме (например, `$2`), либо могут ссылаться на именованные
    /// группы захвата, если они присутствуют в исходном паттерне (например, `$foo`).
    ///
    /// Для документации о полном формате, пожалуйста, смотрите метод `interpolate` трейта `Capture` в
    /// крейте [grep-printer](https://docs.rs/grep-printer).
    pub fn replacement(
        &mut self,
        replacement: Option<Vec<u8>>,
    ) -> &mut JSONBuilder {
        self.config.replacement = Arc::new(replacement);
        self
    }
}

/// JSON принтер, который выводит результаты в формате JSON lines.
///
/// Этот тип параметризован над `W`, который представляет любую реализацию
/// трейта стандартной библиотеки `io::Write`.
///
/// # Формат
///
/// Этот раздел описывает формат JSON, используемый этим принтером.
///
/// Чтобы не ходить вокруг да около, посмотрите на
/// [пример](#example)
/// в конце.
///
/// ## Обзор
///
/// Формат этого принтера — [JSON Lines](https://jsonlines.org/).
/// В частности, этот принтер выводит последовательность сообщений, где
/// каждое сообщение закодировано как одно JSON значение в одной строке. Есть
/// четыре различных типа сообщений (и это число может расшириться со временем):
///
/// * **begin** - Сообщение, указывающее, что файл ищется.
/// * **end** - Сообщение, указывающее, что файл закончен поиском. Это
///   сообщение также включает сводную статистику о поиске.
/// * **match** - Сообщение, указывающее, что совпадение найдено. Это включает
///   текст и смещения совпадения.
/// * **context** - Сообщение, указывающее, что найдена контекстная строка.
///   Это включает текст строки, а также любую информацию о совпадении, если
///   поиск был инвертирован.
///
/// Каждое сообщение закодировано в одном формате конверта, который включает тег,
/// указывающий тип сообщения, и объект для полезной нагрузки:
///
/// ```json
/// {
///     "type": "{begin|end|match|context}",
///     "data": { ... }
/// }
/// ```
///
/// Само сообщение закодировано в ключе `data` конверта.
///
/// ## Кодирование текста
///
/// Перед описанием формата каждого сообщения мы сначала должны кратко обсудить
/// кодирование текста, поскольку оно влияет на каждый тип сообщения. В частности, JSON
/// может быть закодирован только в UTF-8, UTF-16 или UTF-32. Для целей этого
/// принтера нам нужно беспокоиться только о UTF-8. Проблема здесь в том, что поиск
/// не ограничен только UTF-8, что подразумевает, что могут быть сообщены совпадения,
/// содержащие невалидный UTF-8. Более того, этот принтер может
/// также выводить пути к файлам, и кодирование путей к файлам само по себе не
/// гарантированно является валидным UTF-8. Поэтому этот принтер должен как-то
/// обрабатывать наличие невалидного UTF-8. Принтер может молча игнорировать такие
/// вещи полностью или даже потерянно транскодировать невалидный UTF-8 в валидный UTF-8,
/// заменяя все невалидные последовательности символом замены Unicode.
/// Однако это помешало бы потребителям этого формата получать доступ к
/// оригинальным данным без потерь.
///
/// Поэтому этот принтер будет выводить валидные байты, закодированные в UTF-8, как обычные
/// JSON строки и в противном случае кодировать данные в base64, которые не являются валидным UTF-8. Чтобы
/// сообщить, происходит ли этот процесс или нет, строки ключуются по
/// имени `text`, тогда как произвольные байты ключуются по `bytes`.
///
/// Например, когда путь включён в сообщение, он отформатирован следующим образом,
/// если и только если путь является валидным UTF-8:
///
/// ```json
/// {
///     "path": {
///         "text": "/home/ubuntu/lib.rs"
///     }
/// }
/// ```
///
/// Если вместо этого наш путь был `/home/ubuntu/lib\xFF.rs`, где байт `\xFF`
/// делает его невалидным UTF-8, путь был бы вместо этого закодирован следующим образом:
///
/// ```json
/// {
///     "path": {
///         "bytes": "L2hvbWUvdWJ1bnR1L2xpYv8ucnM="
///     }
/// }
/// ```
///
/// Это же представление используется для сообщения о совпадениях.
///
/// Принтер гарантирует, что поле `text` используется, когда базовые
/// байты являются валидным UTF-8.
///
/// ## Сетевой формат
///
/// Этот раздел документирует сетевой формат, испускаемый этим принтером,
/// начиная с четырёх типов сообщений.
///
/// Каждое сообщение имеет свой собственный формат и содержится внутри
/// конверта, который указывает тип сообщения. Конверт имеет эти поля:
///
/// * **type** - Строка, указывающая тип этого сообщения. Это может быть
///   одна из четырёх возможных строк: `begin`, `end`, `match` или `context`.
///   Этот список может расшириться со временем.
/// * **data** - Фактические данные сообщения. Формат этого поля зависит от
///   значения `type`. Возможные форматы сообщений:
///   [`begin`](#message-begin),
///   [`end`](#message-end),
///   [`match`](#message-match),
///   [`context`](#message-context).
///
/// #### Сообщение: **begin**
///
/// Это сообщение указывает, что поиск начался. Оно имеет эти поля:
///
/// * **path** - Объект
///   [произвольных данных](#object-arbitrary-data),
///   представляющий путь к файлу, соответствующий поиску, если он
///   присутствует. Если путь к файлу недоступен, то это поле равно `null`.
///
/// #### Сообщение: **end**
///
/// Это сообщение указывает, что поиск завершился. Оно имеет эти поля:
///
/// * **path** - Объект
///   [произвольных данных](#object-arbitrary-data),
///   представляющий путь к файлу, соответствующий поиску, если он
///   присутствует. Если путь к файлу недоступен, то это поле равно `null`.
/// * **binary_offset** - Абсолютное смещение в исканных данных,
///   соответствующее месту, где были обнаружены бинарные данные. Если
///   бинарные данные не были обнаружены (или если обнаружение бинарных
///   данных было отключено), то это поле равно `null`.
/// * **stats** - Объект [`stats`](#object-stats), который содержит
///   сводную статистику для предыдущего поиска.
///
/// #### Сообщение: **match**
///
/// Это сообщение указывает, что совпадение найдено. Совпадение обычно
/// соответствует одной строке текста, хотя оно может соответствовать
/// нескольким строкам, если поиск может испускать совпадения по нескольким
/// строкам. Оно имеет эти поля:
///
/// * **path** - Объект
///   [произвольных данных](#object-arbitrary-data),
///   представляющий путь к файлу, соответствующий поиску, если он
///   присутствует. Если путь к файлу недоступен, то это поле равно `null`.
/// * **lines** - Объект
///   [произвольных данных](#object-arbitrary-data),
///   представляющий одну или несколько строк, содержащихся в этом совпадении.
/// * **line_number** - Если searcher настроен на сообщение номеров строк,
///   то это соответствует номеру строки первой строки в `lines`. Если
///   номера строк недоступны, то это `null`.
/// * **absolute_offset** - Абсолютное байтовое смещение, соответствующее
///   началу `lines` в исканных данных.
/// * **submatches** - Массив объектов [`submatch`](#object-submatch),
///   соответствующих совпадениям в `lines`. Смещения, включённые в каждый
///   `submatch`, соответствуют байтовым смещениям в `lines`. (Если `lines`
///   кодирован в base64, то байтовые смещения соответствуют данным после
///   декодирования base64.) Объекты `submatch` гарантированно отсортированы
///   по их начальным смещениям. Заметьте, что возможно, что этот массив
///   будет пуст, например, когда поиск сообщает инвертированные совпадения.
///   Если конфигурация указывает замену, то результирующий текст замены
///   также присутствует.
///
/// #### Сообщение: **context**
///
/// Это сообщение указывает, что контекстная строка найдена. Контекстная
/// строка — это строка, которая не содержит совпадения, но обычно смежна
/// со строкой, которая содержит совпадение. Точный способ, которым
/// контекстные строки сообщаются, определяется searcher. Оно имеет эти
/// поля, которые являются точно такими же полями, найденными в
/// [`match`](#message-match):
///
/// * **path** - Объект
///   [произвольных данных](#object-arbitrary-data),
///   представляющий путь к файлу, соответствующий поиску, если он
///   присутствует. Если путь к файлу недоступен, то это поле равно `null`.
/// * **lines** - Объект
///   [произвольных данных](#object-arbitrary-data),
///   представляющий одну или несколько строк, содержащихся в этом контексте.
///   Это включает терминаторы строк, если они присутствуют.
/// * **line_number** - Если searcher настроен на сообщение номеров строк,
///   то это соответствует номеру строки первой строки в `lines`. Если
///   номера строк недоступны, то это `null`.
/// * **absolute_offset** - Абсолютное байтовое смещение, соответствующее
///   началу `lines` в исканных данных.
/// * **submatches** - Массив объектов [`submatch`](#object-submatch),
///   соответствующих совпадениям в `lines`. Смещения, включённые в каждый
///   `submatch`, соответствуют байтовым смещениям в `lines`. (Если `lines`
///   кодирован в base64, то байтовые смещения соответствуют данным после
///   декодирования base64.) Объекты `submatch` гарантированно отсортированы
///   по их начальным смещениям. Заметьте, что возможно, что этот массив
///   будет непуст, например, когда поиск сообщает инвертированные совпадения
///   так, что оригинальный matcher мог сопоставить вещи в контекстных
///   строках. Если конфигурация указывает замену, то результирующий текст
///   замены также присутствует.
///
/// #### Объект: **submatch**
///
/// Этот объект описывает подсовпадения, найденные в сообщениях `match` или
/// `context`. Поля `start` и `end` указывают полуоткрытый интервал, на
/// котором происходит совпадение (`start` включён, но `end` — нет).
/// Гарантируется, что `start <= end`. Он имеет эти поля:
///
/// * **match** - Объект
///   [произвольных данных](#object-arbitrary-data),
///   соответствующий тексту в этом подсовпадении.
/// * **start** - Байтовое смещение, указывающее начало этого совпадения.
///   Это смещение обычно сообщается в терминах данных родительского
///   объекта. Например, поле `lines` в сообщениях
///   [`match`](#message-match) или [`context`](#message-context).
/// * **end** - Байтовое смещение, указывающее конец этого совпадения.
///   Это смещение обычно сообщается в терминах данных родительского
///   объекта. Например, поле `lines` в сообщениях
///   [`match`](#message-match) или [`context`](#message-context).
/// * **replacement** (опционально) - Объект
///   [произвольных данных](#object-arbitrary-data), соответствующий
///   тексту замены для этого подсовпадения, если конфигурация указывает
///   замену.
///
/// #### Объект: **stats**
///
/// Этот объект включён в сообщения и содержит сводную статистику о
/// поиске. Он имеет эти поля:
///
/// * **elapsed** - Объект [`duration`](#object-duration), описывающий
///   длину времени, прошедшего во время выполнения поиска.
/// * **searches** - Количество поисков, которые были выполнены. Для этого
///   принтера это значение всегда `1`. (Реализации могут испускать
///   дополнительные типы сообщений, которые используют этот же объект
///   `stats`, который представляет сводную статистику по нескольким
///   поискам.)
/// * **searches_with_match** - Количество поисков, которые были выполнены
///   и нашли хотя бы одно совпадение. Это никогда не больше `searches`.
/// * **bytes_searched** - Общее количество байтов, которые были исканы.
/// * **bytes_printed** - Общее количество байтов, которые были напечатаны.
///   Это включает всё, испущенное этим принтером.
/// * **matched_lines** - Общее количество строк, которые участвовали в
///   совпадении. Когда совпадения могут содержать несколько строк, то это
///   включает каждую строку, которая является частью каждого совпадения.
/// * **matches** - Общее количество совпадений. Может быть несколько
///   совпадений на строку. Когда совпадения могут содержать несколько
///   строк, каждое совпадение считается только один раз, независимо от
///   того, сколько строк оно охватывает.
///
/// #### Объект: **duration**
///
/// Этот объект включает несколько полей для описания длительности. Два
/// его поля, `secs` и `nanos`, могут быть объединены для получения
/// наносекундной точности на системах, которые поддерживают это. Он имеет
/// эти поля:
///
/// * **secs** - Целое число секунд, указывающее длину этой длительности.
/// * **nanos** - Дробная часть этой длительности, представленная
///   наносекундами. Если наносекундная точность не поддерживается, то
///   это обычно округляется до ближайшего количества наносекунд.
/// * **human** - Читаемая человеком строка, описывающая длину
///   длительности. Формат строки сам по себе не указан.
///
/// #### Объект: **произвольные данные**
///
/// Этот объект используется, когда произвольные данные должны быть
/// представлены как значение JSON. Этот объект содержит два поля, где
/// обычно присутствует только одно из полей:
///
/// * **text** - Обычная JSON строка, которая кодирована в UTF-8. Это поле
///   заполняется тогда и только тогда, когда базовые данные являются
///   валидным UTF-8.
/// * **bytes** - Обычная JSON строка, которая является кодировкой base64
///   базовых байтов.
///
/// Больше информации о мотивации для этого представления можно увидеть в
/// разделе [кодирование текста](#text-encoding) выше.
///
/// ## Пример
///
/// Этот раздел показывает небольшой пример, который включает все типы
/// сообщений.
///
/// Вот файл, который мы хотим искать, расположенный в `/home/andrew/sherlock`:
///
/// ```text
/// For the Doctor Watsons of this world, as opposed to the Sherlock
/// Holmeses, success in the province of detective work must always
/// be, to a very large extent, the result of luck. Sherlock Holmes
/// can extract a clew from a wisp of straw or a flake of cigar ash;
/// but Doctor Watson has to have it taken out for him and dusted,
/// and exhibited clearly, with a label attached.
/// ```
///
/// Поиск `Watson` с `before_context`, равным `1`, с включёнными номерами
/// строк показывает что-то вроде этого с использованием стандартного
/// принтера:
///
/// ```text
/// sherlock:1:For the Doctor Watsons of this world, as opposed to the Sherlock
/// --
/// sherlock-4-can extract a clew from a wisp of straw or a flake of cigar ash;
/// sherlock:5:but Doctor Watson has to have it taken out for him and dusted,
/// ```
///
/// Вот как выглядит тот же поиск с использованием описанного выше сетевого
/// формата JSON, где мы показываем полу-красиво оформленный JSON (вместо
/// строгого формата JSON Lines), в иллюстративных целях:
///
/// ```json
/// {
///   "type": "begin",
///   "data": {
///     "path": {"text": "/home/andrew/sherlock"}}
///   }
/// }
/// {
///   "type": "match",
///   "data": {
///     "path": {"text": "/home/andrew/sherlock"},
///     "lines": {"text": "For the Doctor Watsons of this world, as opposed to the Sherlock\n"},
///     "line_number": 1,
///     "absolute_offset": 0,
///     "submatches": [
///       {"match": {"text": "Watson"}, "start": 15, "end": 21}
///     ]
///   }
/// }
/// {
///   "type": "context",
///   "data": {
///     "path": {"text": "/home/andrew/sherlock"},
///     "lines": {"text": "can extract a clew from a wisp of straw or a flake of cigar ash;\n"},
///     "line_number": 4,
///     "absolute_offset": 193,
///     "submatches": []
///   }
/// }
/// {
///   "type": "match",
///   "data": {
///     "path": {"text": "/home/andrew/sherlock"},
///     "lines": {"text": "but Doctor Watson has to have it taken out for him and dusted,\n"},
///     "line_number": 5,
///     "absolute_offset": 258,
///     "submatches": [
///       {"match": {"text": "Watson"}, "start": 11, "end": 17}
///     ]
///   }
/// }
/// {
///   "type": "end",
///   "data": {
///     "path": {"text": "/home/andrew/sherlock"},
///     "binary_offset": null,
///     "stats": {
///       "elapsed": {"secs": 0, "nanos": 36296, "human": "0.0000s"},
///       "searches": 1,
///       "searches_with_match": 1,
///       "bytes_searched": 367,
///       "bytes_printed": 1151,
///       "matched_lines": 2,
///       "matches": 2
///     }
///   }
/// }
/// ```
/// и вот как элемент типа match выглядел бы, если бы текст замены
/// 'Moriarity' был дан как параметр:
/// ```json
/// {
///   "type": "match",
///   "data": {
///     "path": {"text": "/home/andrew/sherlock"},
///     "lines": {"text": "For the Doctor Watsons of this world, as opposed to the Sherlock\n"},
///     "line_number": 1,
///     "absolute_offset": 0,
///     "submatches": [
///       {"match": {"text": "Watson"}, "replacement": {"text": "Moriarity"}, "start": 15, "end": 21}
///     ]
///   }
/// }
/// ```

#[derive(Clone, Debug)]
pub struct JSON<W> {
    config: Config,
    wtr: CounterWriter<W>,
    matches: Vec<Match>,
}

impl<W: io::Write> JSON<W> {
    /// Возвращает JSON lines принтер с конфигурацией по умолчанию, который
    /// записывает совпадения в данный writer.
    pub fn new(wtr: W) -> JSON<W> {
        JSONBuilder::new().build(wtr)
    }

    /// Возвращает реализацию `Sink` для JSON принтера.
    ///
    /// Это не связывает принтер с путём к файлу, что означает, что эта
    /// реализация никогда не будет печатать путь к файлу вместе с
    /// совпадениями.
    pub fn sink<'s, M: Matcher>(
        &'s mut self,
        matcher: M,
    ) -> JSONSink<'static, 's, M, W> {
        JSONSink {
            matcher,
            replacer: Replacer::new(),
            json: self,
            path: None,
            start_time: Instant::now(),
            match_count: 0,
            binary_byte_offset: None,
            begin_printed: false,
            stats: Stats::new(),
        }
    }

    /// Возвращает реализацию `Sink`, связанную с путём к файлу.
    ///
    /// Когда принтер связан с путём, то он может, в зависимости от
    /// своей конфигурации, печатать путь вместе с найденными совпадениями.
    pub fn sink_with_path<'p, 's, M, P>(
        &'s mut self,
        matcher: M,
        path: &'p P,
    ) -> JSONSink<'p, 's, M, W>
    where
        M: Matcher,
        P: ?Sized + AsRef<Path>,
    {
        JSONSink {
            matcher,
            replacer: Replacer::new(),
            json: self,
            path: Some(path.as_ref()),
            start_time: Instant::now(),
            match_count: 0,
            binary_byte_offset: None,
            begin_printed: false,
            stats: Stats::new(),
        }
    }

    /// Записывает данное сообщение, за которым следует новая строка. Новая
    /// строка определяется из конфигурации данного searcher.
    fn write_message(
        &mut self,
        message: &jsont::Message<'_>,
    ) -> io::Result<()> {
        if self.config.pretty {
            json::to_writer_pretty(&mut self.wtr, message)?;
        } else {
            json::to_writer(&mut self.wtr, message)?;
        }
        let _ = self.wtr.write(b"\n")?; // Это всегда будет Ok(1) при успехе.
        Ok(())
    }
}

impl<W> JSON<W> {
    /// Возвращает true тогда и только тогда, когда этот принтер записал
    /// хотя бы один байт в базовый writer во время любого из предыдущих
    /// поисков.
    pub fn has_written(&self) -> bool {
        self.wtr.total_count() > 0
    }

    /// Возвращает изменяемую ссылку на базовый writer.
    pub fn get_mut(&mut self) -> &mut W {
        self.wtr.get_mut()
    }

    /// Поглощает этот принтер и возвращает обратно владение базовым
    /// writer.
    pub fn into_inner(self) -> W {
        self.wtr.into_inner()
    }
}

/// Реализация `Sink`, связанная с matcher и опциональным путём к файлу
/// для JSON принтера.
///
/// Этот тип параметризован несколькими параметрами типа:
///
/// * `'p` относится к времени жизни пути к файлу, если он предоставлен.
///   Когда путь к файлу не дан, то это `'static`.
/// * `'s` относится к времени жизни принтера [`JSON`], который этот тип
///   заимствует.
/// * `M` относится к типу matcher, используемого `grep_searcher::Searcher`,
///   который сообщает результаты в этот sink.
/// * `W` относится к базовому writer, в который этот принтер записывает
///   свой вывод.
#[derive(Debug)]
pub struct JSONSink<'p, 's, M: Matcher, W> {
    matcher: M,
    replacer: Replacer<M>,
    json: &'s mut JSON<W>,
    path: Option<&'p Path>,
    start_time: Instant,
    match_count: u64,
    binary_byte_offset: Option<u64>,
    begin_printed: bool,
    stats: Stats,
}

impl<'p, 's, M: Matcher, W: io::Write> JSONSink<'p, 's, M, W> {
    /// Возвращает true тогда и только тогда, когда этот принтер получил
    /// совпадение в предыдущем поиске.
    ///
    /// Это не зависит от результата поисков до предыдущего поиска.
    pub fn has_match(&self) -> bool {
        self.match_count > 0
    }

    /// Возвращает общее количество совпадений, сообщённых в этот sink.
    ///
    /// Это соответствует количеству вызовов `Sink::matched`.
    pub fn match_count(&self) -> u64 {
        self.match_count
    }

    /// Если бинарные данные были найдены в предыдущем поиске, это
    /// возвращает смещение, на котором бинарные данные были впервые
    /// обнаружены.
    ///
    /// Возвращаемое смещение — это абсолютное смещение относительно
    /// всего набора исканных байтов.
    ///
    /// Это не зависит от результата поисков до предыдущего поиска.
    /// Например, если поиск до предыдущего поиска нашёл бинарные
    /// данные, но предыдущий поиск не нашёл бинарных данных, то это
    /// вернёт `None`.
    pub fn binary_byte_offset(&self) -> Option<u64> {
        self.binary_byte_offset
    }

    /// Возвращает ссылку на статистику, созданную принтером для всех
    /// поисков, выполненных на этом sink.
    pub fn stats(&self) -> &Stats {
        &self.stats
    }

    /// Выполняет matcher на данных байтах и записывает расположения
    /// совпадений, если текущая конфигурация требует гранулярности
    /// совпадений.
    fn record_matches(
        &mut self,
        searcher: &Searcher,
        bytes: &[u8],
        range: std::ops::Range<usize>,
    ) -> io::Result<()> {
        self.json.matches.clear();
        // Если печать требует знания расположения каждого отдельного
        // совпадения, то вычисляем и сохраняем их прямо сейчас для
        // использования позже. Хотя это добавляет дополнительную копию
        // для хранения совпадений, мы амортизируем выделение для этого,
        // и это значительно упрощает логику печати до такой степени,
        // что легко убедиться, что мы никогда не делаем более одного
        // поиска для нахождения совпадений.
        let matches = &mut self.json.matches;
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
        // Не сообщаем пустые совпадения, появляющиеся в конце байтов.
        if !matches.is_empty()
            && matches.last().unwrap().is_empty()
            && matches.last().unwrap().start() >= bytes.len()
        {
            matches.pop().unwrap();
        }
        Ok(())
    }

    /// Если конфигурация указывает замену, то это выполняет замену,
    /// лениво выделяя память, если необходимо.
    ///
    /// Для доступа к результату замены используйте `replacer.replacement()`.
    fn replace(
        &mut self,
        searcher: &Searcher,
        bytes: &[u8],
        range: std::ops::Range<usize>,
    ) -> io::Result<()> {
        self.replacer.clear();
        if self.json.config.replacement.is_some() {
            let replacement =
                (*self.json.config.replacement).as_ref().map(|r| &*r).unwrap();
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

    /// Записывает сообщение "begin".
    fn write_begin_message(&mut self) -> io::Result<()> {
        if self.begin_printed {
            return Ok(());
        }
        let msg = jsont::Message::Begin(jsont::Begin { path: self.path });
        self.json.write_message(&msg)?;
        self.begin_printed = true;
        Ok(())
    }
}

impl<'p, 's, M: Matcher, W: io::Write> Sink for JSONSink<'p, 's, M, W> {
    type Error = io::Error;

    fn matched(
        &mut self,
        searcher: &Searcher,
        mat: &SinkMatch<'_>,
    ) -> Result<bool, io::Error> {
        self.match_count += 1;
        self.write_begin_message()?;

        self.record_matches(
            searcher,
            mat.buffer(),
            mat.bytes_range_in_buffer(),
        )?;
        self.replace(searcher, mat.buffer(), mat.bytes_range_in_buffer())?;
        self.stats.add_matches(self.json.matches.len() as u64);
        self.stats.add_matched_lines(mat.lines().count() as u64);

        let submatches = SubMatches::new(
            mat.bytes(),
            &self.json.matches,
            self.replacer.replacement(),
        );
        let msg = jsont::Message::Match(jsont::Match {
            path: self.path,
            lines: mat.bytes(),
            line_number: mat.line_number(),
            absolute_offset: mat.absolute_byte_offset(),
            submatches: submatches.as_slice(),
        });
        self.json.write_message(&msg)?;
        Ok(true)
    }

    fn context(
        &mut self,
        searcher: &Searcher,
        ctx: &SinkContext<'_>,
    ) -> Result<bool, io::Error> {
        self.write_begin_message()?;
        self.json.matches.clear();

        let submatches = if searcher.invert_match() {
            self.record_matches(searcher, ctx.bytes(), 0..ctx.bytes().len())?;
            self.replace(searcher, ctx.bytes(), 0..ctx.bytes().len())?;
            SubMatches::new(
                ctx.bytes(),
                &self.json.matches,
                self.replacer.replacement(),
            )
        } else {
            SubMatches::empty()
        };
        let msg = jsont::Message::Context(jsont::Context {
            path: self.path,
            lines: ctx.bytes(),
            line_number: ctx.line_number(),
            absolute_offset: ctx.absolute_byte_offset(),
            submatches: submatches.as_slice(),
        });
        self.json.write_message(&msg)?;
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
                    path = path.display(),
                );
            }
        }
        Ok(true)
    }

    fn begin(&mut self, _searcher: &Searcher) -> Result<bool, io::Error> {
        self.json.wtr.reset_count();
        self.start_time = Instant::now();
        self.match_count = 0;
        self.binary_byte_offset = None;

        if !self.json.config.always_begin_end {
            return Ok(true);
        }
        self.write_begin_message()?;
        Ok(true)
    }

    fn finish(
        &mut self,
        _searcher: &Searcher,
        finish: &SinkFinish,
    ) -> Result<(), io::Error> {
        self.binary_byte_offset = finish.binary_byte_offset();
        self.stats.add_elapsed(self.start_time.elapsed());
        self.stats.add_searches(1);
        if self.match_count > 0 {
            self.stats.add_searches_with_match(1);
        }
        self.stats.add_bytes_searched(finish.byte_count());
        self.stats.add_bytes_printed(self.json.wtr.count());

        if !self.begin_printed {
            return Ok(());
        }
        let msg = jsont::Message::End(jsont::End {
            path: self.path,
            binary_offset: finish.binary_byte_offset(),
            stats: self.stats.clone(),
        });
        self.json.write_message(&msg)?;
        Ok(())
    }
}

/// SubMatches представляет набор совпадений в непрерывном диапазоне
/// байтов.
///
/// Более простым представлением для этого был бы просто `Vec<SubMatch>`,
/// но распространённый случай — это ровно одно совпадение на диапазон
/// байтов, что мы специализируем здесь, используя массив фиксированного
/// размера без какого-либо выделения.
enum SubMatches<'a> {
    Empty,
    Small([jsont::SubMatch<'a>; 1]),
    Big(Vec<jsont::SubMatch<'a>>),
}

impl<'a> SubMatches<'a> {
    /// Создаёт новый набор диапазонов совпадений из набора совпадений и
    /// соответствующих байтов, к которым эти совпадения применяются.
    fn new(
        bytes: &'a [u8],
        matches: &[Match],
        replacement: Option<(&'a [u8], &'a [Match])>,
    ) -> SubMatches<'a> {
        if matches.len() == 1 {
            let mat = matches[0];
            SubMatches::Small([jsont::SubMatch {
                m: &bytes[mat],
                replacement: replacement
                    .map(|(rbuf, rmatches)| &rbuf[rmatches[0]]),
                start: mat.start(),
                end: mat.end(),
            }])
        } else {
            let mut match_ranges = vec![];
            for (i, &mat) in matches.iter().enumerate() {
                match_ranges.push(jsont::SubMatch {
                    m: &bytes[mat],
                    replacement: replacement
                        .map(|(rbuf, rmatches)| &rbuf[rmatches[i]]),
                    start: mat.start(),
                    end: mat.end(),
                });
            }
            SubMatches::Big(match_ranges)
        }
    }

    /// Создаёт пустой набор диапазонов совпадений.
    fn empty() -> SubMatches<'static> {
        SubMatches::Empty
    }

    /// Возвращает этот набор диапазонов совпадений как срез.
    fn as_slice(&self) -> &[jsont::SubMatch<'_>] {
        match *self {
            SubMatches::Empty => &[],
            SubMatches::Small(ref x) => x,
            SubMatches::Big(ref x) => x,
        }
    }
}

#[cfg(test)]
mod tests {
    use grep_matcher::LineTerminator;
    use grep_regex::{RegexMatcher, RegexMatcherBuilder};
    use grep_searcher::SearcherBuilder;

    use super::{JSON, JSONBuilder};

    const SHERLOCK: &'static [u8] = b"\
For the Doctor Watsons of this world, as opposed to the Sherlock
Holmeses, success in the province of detective work must always
be, to a very large extent, the result of luck. Sherlock Holmes
can extract a clew from a wisp of straw or a flake of cigar ash;
but Doctor Watson has to have it taken out for him and dusted,
and exhibited clearly, with a label attached.
";

    fn printer_contents(printer: &mut JSON<Vec<u8>>) -> String {
        String::from_utf8(printer.get_mut().to_owned()).unwrap()
    }

    #[test]
    fn binary_detection() {
        use grep_searcher::BinaryDetection;

        const BINARY: &'static [u8] = b"\
For the Doctor Watsons of this world, as opposed to the Sherlock
Holmeses, success in the province of detective work must always
be, to a very large extent, the result of luck. Sherlock Holmes
can extract a clew \x00 from a wisp of straw or a flake of cigar ash;
but Doctor Watson has to have it taken out for him and dusted,
and exhibited clearly, with a label attached.\
";

        let matcher = RegexMatcher::new(r"Watson").unwrap();
        let mut printer = JSONBuilder::new().build(vec![]);
        SearcherBuilder::new()
            .binary_detection(BinaryDetection::quit(b'\x00'))
            .heap_limit(Some(80))
            .build()
            .search_reader(&matcher, BINARY, printer.sink(&matcher))
            .unwrap();
        let got = printer_contents(&mut printer);

        assert_eq!(got.lines().count(), 3);
        let last = got.lines().last().unwrap();
        assert!(last.contains(r#""binary_offset":212,"#));
    }

    #[test]
    fn max_matches() {
        let matcher = RegexMatcher::new(r"Watson").unwrap();
        let mut printer = JSONBuilder::new().build(vec![]);
        SearcherBuilder::new()
            .max_matches(Some(1))
            .build()
            .search_reader(&matcher, SHERLOCK, printer.sink(&matcher))
            .unwrap();
        let got = printer_contents(&mut printer);

        assert_eq!(got.lines().count(), 3);
    }

    #[test]
    fn max_matches_after_context() {
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
        let matcher = RegexMatcher::new(r"d").unwrap();
        let mut printer = JSONBuilder::new().build(vec![]);
        SearcherBuilder::new()
            .after_context(2)
            .max_matches(Some(1))
            .build()
            .search_reader(
                &matcher,
                haystack.as_bytes(),
                printer.sink(&matcher),
            )
            .unwrap();
        let got = printer_contents(&mut printer);

        assert_eq!(got.lines().count(), 5);
    }

    #[test]
    fn no_match() {
        let matcher = RegexMatcher::new(r"DOES NOT MATCH").unwrap();
        let mut printer = JSONBuilder::new().build(vec![]);
        SearcherBuilder::new()
            .build()
            .search_reader(&matcher, SHERLOCK, printer.sink(&matcher))
            .unwrap();
        let got = printer_contents(&mut printer);

        assert!(got.is_empty());
    }

    #[test]
    fn always_begin_end_no_match() {
        let matcher = RegexMatcher::new(r"DOES NOT MATCH").unwrap();
        let mut printer =
            JSONBuilder::new().always_begin_end(true).build(vec![]);
        SearcherBuilder::new()
            .build()
            .search_reader(&matcher, SHERLOCK, printer.sink(&matcher))
            .unwrap();
        let got = printer_contents(&mut printer);

        assert_eq!(got.lines().count(), 2);
        assert!(got.contains("begin") && got.contains("end"));
    }

    #[test]
    fn missing_crlf() {
        let haystack = "test\r\n".as_bytes();

        let matcher = RegexMatcherBuilder::new().build("test").unwrap();
        let mut printer = JSONBuilder::new().build(vec![]);
        SearcherBuilder::new()
            .build()
            .search_reader(&matcher, haystack, printer.sink(&matcher))
            .unwrap();
        let got = printer_contents(&mut printer);
        assert_eq!(got.lines().count(), 3);
        assert!(
            got.lines().nth(1).unwrap().contains(r"test\r\n"),
            r"missing 'test\r\n' in '{}'",
            got.lines().nth(1).unwrap(),
        );

        let matcher =
            RegexMatcherBuilder::new().crlf(true).build("test").unwrap();
        let mut printer = JSONBuilder::new().build(vec![]);
        SearcherBuilder::new()
            .line_terminator(LineTerminator::crlf())
            .build()
            .search_reader(&matcher, haystack, printer.sink(&matcher))
            .unwrap();
        let got = printer_contents(&mut printer);
        assert_eq!(got.lines().count(), 3);
        assert!(
            got.lines().nth(1).unwrap().contains(r"test\r\n"),
            r"missing 'test\r\n' in '{}'",
            got.lines().nth(1).unwrap(),
        );
    }
}
