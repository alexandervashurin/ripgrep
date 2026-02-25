use std::io::{self, Write};

use {
    bstr::ByteSlice,
    grep_matcher::{
        LineMatchKind, LineTerminator, Match, Matcher, NoCaptures, NoError,
    },
    regex::bytes::{Regex, RegexBuilder},
};

use crate::{
    searcher::{BinaryDetection, Searcher, SearcherBuilder},
    sink::{Sink, SinkContext, SinkFinish, SinkMatch},
};

/// Простой regex-матчер.
///
/// Это поддерживает установку конфигурации завершителя строк матчера
/// напрямую, что мы используем для целей тестирования. То есть вызывающая
/// сторона явно определяет, включена ли оптимизация завершителя строк.
/// (На самом деле эта оптимизация обнаруживается автоматически путём
/// проверки и возможного изменения самого regex.)
#[derive(Clone, Debug)]
pub(crate) struct RegexMatcher {
    regex: Regex,
    line_term: Option<LineTerminator>,
    every_line_is_candidate: bool,
}

impl RegexMatcher {
    /// Создать новый regex-матчер.
    pub(crate) fn new(pattern: &str) -> RegexMatcher {
        let regex = RegexBuilder::new(pattern)
            .multi_line(true) // разрешает ^ и $ совпадать на границах \n
            .build()
            .unwrap();
        RegexMatcher { regex, line_term: None, every_line_is_candidate: false }
    }

    /// Принудительно установить завершитель строк этого матчера.
    ///
    /// По умолчанию у этого матчера не установлен завершитель строк.
    pub(crate) fn set_line_term(
        &mut self,
        line_term: Option<LineTerminator>,
    ) -> &mut RegexMatcher {
        self.line_term = line_term;
        self
    }

    /// Возвращать ли каждую строку как кандидата или нет.
    ///
    /// Это заставляет поисковики обрабатывать случай сообщения о
    /// ложноположительном результате.
    pub(crate) fn every_line_is_candidate(
        &mut self,
        yes: bool,
    ) -> &mut RegexMatcher {
        self.every_line_is_candidate = yes;
        self
    }
}

impl Matcher for RegexMatcher {
    type Captures = NoCaptures;
    type Error = NoError;

    fn find_at(
        &self,
        haystack: &[u8],
        at: usize,
    ) -> Result<Option<Match>, NoError> {
        Ok(self
            .regex
            .find_at(haystack, at)
            .map(|m| Match::new(m.start(), m.end())))
    }

    fn new_captures(&self) -> Result<NoCaptures, NoError> {
        Ok(NoCaptures::new())
    }

    fn line_terminator(&self) -> Option<LineTerminator> {
        self.line_term
    }

    fn find_candidate_line(
        &self,
        haystack: &[u8],
    ) -> Result<Option<LineMatchKind>, NoError> {
        if self.every_line_is_candidate {
            assert!(self.line_term.is_some());
            if haystack.is_empty() {
                return Ok(None);
            }
            // Сделать это интересным и вернуть последний байт в текущей
            // строке.
            let i = haystack
                .find_byte(self.line_term.unwrap().as_byte())
                .map(|i| i)
                .unwrap_or(haystack.len() - 1);
            Ok(Some(LineMatchKind::Candidate(i)))
        } else {
            Ok(self.shortest_match(haystack)?.map(LineMatchKind::Confirmed))
        }
    }
}

/// Реализация Sink, которая печатает всю доступную информацию.
///
/// Это полезно для тестов, потому что позволяет нам легко подтвердить,
/// передаются ли данные в Sink корректно.
#[derive(Clone, Debug)]
pub(crate) struct KitchenSink(Vec<u8>);

impl KitchenSink {
    /// Создать новую реализацию Sink, которая включает всё на кухне.
    pub(crate) fn new() -> KitchenSink {
        KitchenSink(vec![])
    }

    /// Вернуть данные, записанные в этот sink.
    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl Sink for KitchenSink {
    type Error = io::Error;

    fn matched(
        &mut self,
        _searcher: &Searcher,
        mat: &SinkMatch<'_>,
    ) -> Result<bool, io::Error> {
        assert!(!mat.bytes().is_empty());
        assert!(mat.lines().count() >= 1);

        let mut line_number = mat.line_number();
        let mut byte_offset = mat.absolute_byte_offset();
        for line in mat.lines() {
            if let Some(ref mut n) = line_number {
                write!(self.0, "{}:", n)?;
                *n += 1;
            }

            write!(self.0, "{}:", byte_offset)?;
            byte_offset += line.len() as u64;
            self.0.write_all(line)?;
        }
        Ok(true)
    }

    fn context(
        &mut self,
        _searcher: &Searcher,
        context: &SinkContext<'_>,
    ) -> Result<bool, io::Error> {
        assert!(!context.bytes().is_empty());
        assert!(context.lines().count() == 1);

        if let Some(line_number) = context.line_number() {
            write!(self.0, "{}-", line_number)?;
        }
        write!(self.0, "{}-", context.absolute_byte_offset)?;
        self.0.write_all(context.bytes())?;
        Ok(true)
    }

    fn context_break(
        &mut self,
        _searcher: &Searcher,
    ) -> Result<bool, io::Error> {
        self.0.write_all(b"--\n")?;
        Ok(true)
    }

    fn finish(
        &mut self,
        _searcher: &Searcher,
        sink_finish: &SinkFinish,
    ) -> Result<(), io::Error> {
        writeln!(self.0, "")?;
        writeln!(self.0, "byte count:{}", sink_finish.byte_count())?;
        if let Some(offset) = sink_finish.binary_byte_offset() {
            writeln!(self.0, "binary offset:{}", offset)?;
        }
        Ok(())
    }
}

/// Тип для выражения тестов на поисковике.
///
/// Код поисковика имеет много различных путей выполнения, в основном
/// для целей оптимизации множества различных вариантов использования.
/// Намерение поисковика — выбрать лучший путь выполнения на основе
/// конфигурации, что означает, что нет очевидного прямого способа
/// попросить выполнить определённый путь. Таким образом, цель этого
/// тестировщика — явно проверить как можно больше осмысленных путей.
///
/// Тестировщик работает, предполагая, что вы хотите протестировать
/// все соответствующие пути выполнения. Их можно сократить по мере
/// необходимости с помощью различных методов конструктора.
#[derive(Debug)]
pub(crate) struct SearcherTester {
    haystack: String,
    pattern: String,
    filter: Option<::regex::Regex>,
    print_labels: bool,
    expected_no_line_number: Option<String>,
    expected_with_line_number: Option<String>,
    expected_slice_no_line_number: Option<String>,
    expected_slice_with_line_number: Option<String>,
    by_line: bool,
    multi_line: bool,
    invert_match: bool,
    line_number: bool,
    binary: BinaryDetection,
    auto_heap_limit: bool,
    after_context: usize,
    before_context: usize,
    passthru: bool,
}

impl SearcherTester {
    /// Создать новый тестировщик для тестирования поисковиков.
    pub(crate) fn new(haystack: &str, pattern: &str) -> SearcherTester {
        SearcherTester {
            haystack: haystack.to_string(),
            pattern: pattern.to_string(),
            filter: None,
            print_labels: false,
            expected_no_line_number: None,
            expected_with_line_number: None,
            expected_slice_no_line_number: None,
            expected_slice_with_line_number: None,
            by_line: true,
            multi_line: true,
            invert_match: false,
            line_number: true,
            binary: BinaryDetection::none(),
            auto_heap_limit: true,
            after_context: 0,
            before_context: 0,
            passthru: false,
        }
    }

    /// Выполнить тест. Если тест успешен, то он возвращается успешно.
    /// Если тест не удаётся, то происходит паника с информативным
    /// сообщением.
    pub(crate) fn test(&self) {
        // Проверить ошибки конфигурации.
        if self.expected_no_line_number.is_none() {
            panic!("должна быть предоставлена строка 'expected' БЕЗ номеров строк");
        }
        if self.line_number && self.expected_with_line_number.is_none() {
            panic!(
                "должна быть предоставлена строка 'expected' с номерами строк, \
                    или отключите тестирование с номерами строк"
            );
        }

        let configs = self.configs();
        if configs.is_empty() {
            panic!("конфигурация теста привела к тому, что ничего не тестируется");
        }
        if self.print_labels {
            for config in &configs {
                let labels = vec![
                    format!("reader-{}", config.label),
                    format!("slice-{}", config.label),
                ];
                for label in &labels {
                    if self.include(label) {
                        println!("{}", label);
                    } else {
                        println!("{} (ignored)", label);
                    }
                }
            }
        }
        for config in &configs {
            let label = format!("reader-{}", config.label);
            if self.include(&label) {
                let got = config.search_reader(&self.haystack);
                assert_eq_printed!(config.expected_reader, got, "{}", label);
            }

            let label = format!("slice-{}", config.label);
            if self.include(&label) {
                let got = config.search_slice(&self.haystack);
                assert_eq_printed!(config.expected_slice, got, "{}", label);
            }
        }
    }

    /// Set a regex pattern to filter the tests that are run.
    ///
    /// By default, no filter is present. When a filter is set, only test
    /// configurations with a label matching the given pattern will be run.
    ///
    /// This is often useful when debugging tests, e.g., when you want to do
    /// printf debugging and only want one particular test configuration to
    /// execute.
    #[allow(dead_code)]
    pub(crate) fn filter(&mut self, pattern: &str) -> &mut SearcherTester {
        self.filter = Some(::regex::Regex::new(pattern).unwrap());
        self
    }

    /// When set, the labels for all test configurations are printed before
    /// executing any test.
    ///
    /// Note that in order to see these in tests that aren't failing, you'll
    /// want to use `cargo test -- --nocapture`.
    #[allow(dead_code)]
    pub(crate) fn print_labels(&mut self, yes: bool) -> &mut SearcherTester {
        self.print_labels = yes;
        self
    }

    /// Установить ожидаемые результаты поиска без номеров строк.
    pub(crate) fn expected_no_line_number(
        &mut self,
        exp: &str,
    ) -> &mut SearcherTester {
        self.expected_no_line_number = Some(exp.to_string());
        self
    }

    /// Установить ожидаемые результаты поиска с номерами строк.
    pub(crate) fn expected_with_line_number(
        &mut self,
        exp: &str,
    ) -> &mut SearcherTester {
        self.expected_with_line_number = Some(exp.to_string());
        self
    }

    /// Установить ожидаемые результаты поиска без номеров строк при
    /// выполнении поиска по срезу. Если не указано, используется
    /// `expected_no_line_number`.
    pub(crate) fn expected_slice_no_line_number(
        &mut self,
        exp: &str,
    ) -> &mut SearcherTester {
        self.expected_slice_no_line_number = Some(exp.to_string());
        self
    }

    /// Установить ожидаемые результаты поиска с номерами строк при
    /// выполнении поиска по срезу. Если не указано, используется
    /// `expected_with_line_number`.
    #[allow(dead_code)]
    pub(crate) fn expected_slice_with_line_number(
        &mut self,
        exp: &str,
    ) -> &mut SearcherTester {
        self.expected_slice_with_line_number = Some(exp.to_string());
        self
    }

    /// Тестировать ли поиск с номерами строк или нет.
    ///
    /// Это включено по умолчанию. Когда включено, должна быть предоставлена
    /// строка, которая ожидается при наличии номеров строк. В противном
    /// случае ожидаемая строка не требуется.
    pub(crate) fn line_number(&mut self, yes: bool) -> &mut SearcherTester {
        self.line_number = yes;
        self
    }

    /// Тестировать ли поиск с использованием построчного поисковика или нет.
    ///
    /// По умолчанию это включено.
    pub(crate) fn by_line(&mut self, yes: bool) -> &mut SearcherTester {
        self.by_line = yes;
        self
    }

    /// Тестировать ли поиск с использованием поисковика по нескольким
    /// строкам или нет.
    ///
    /// По умолчанию это включено.
    #[allow(dead_code)]
    pub(crate) fn multi_line(&mut self, yes: bool) -> &mut SearcherTester {
        self.multi_line = yes;
        self
    }

    /// Выполнять ли инвертированный поиск или нет.
    ///
    /// По умолчанию это отключено.
    pub(crate) fn invert_match(&mut self, yes: bool) -> &mut SearcherTester {
        self.invert_match = yes;
        self
    }

    /// Включать ли обнаружение двоичных данных во всех поисках.
    ///
    /// По умолчанию это отключено.
    pub(crate) fn binary_detection(
        &mut self,
        detection: BinaryDetection,
    ) -> &mut SearcherTester {
        self.binary = detection;
        self
    }

    /// Автоматически ли пытаться тестировать настройку ограничения кучи
    /// или нет.
    ///
    /// По умолчанию одна из конфигураций теста включает установку
    /// ограничения кучи в минимальное значение для нормальной работы,
    /// что проверяет, что всё работает даже в крайних случаях. Однако
    /// в некоторых случаях ограничение кучи может (ожидаемо) немного
    /// изменить вывод. Например, это может повлиять на количество
    /// байтов, searched при выполнении обнаружения двоичных данных.
    /// Для удобства может быть полезно отключить автоматический тест
    /// ограничения кучи.
    pub(crate) fn auto_heap_limit(
        &mut self,
        yes: bool,
    ) -> &mut SearcherTester {
        self.auto_heap_limit = yes;
        self
    }

    /// Установить количество строк для включения в контекст "after".
    ///
    /// По умолчанию `0`, что эквивалентно отсутствию печати какого-либо
    /// контекста.
    pub(crate) fn after_context(
        &mut self,
        lines: usize,
    ) -> &mut SearcherTester {
        self.after_context = lines;
        self
    }

    /// Установить количество строк для включения в контекст "before".
    ///
    /// По умолчанию `0`, что эквивалентно отсутствию печати какого-либо
    /// контекста.
    pub(crate) fn before_context(
        &mut self,
        lines: usize,
    ) -> &mut SearcherTester {
        self.before_context = lines;
        self
    }

    /// Включать ли функцию "passthru" или нет.
    ///
    /// Когда passthru включён, он фактически обрабатывает все несовпадающие
    /// строки как контекстные. Другими словами, включение этого аналогично
    /// запросу неограниченного количества контекстных строк до и после.
    ///
    /// По умолчанию это отключено.
    pub(crate) fn passthru(&mut self, yes: bool) -> &mut SearcherTester {
        self.passthru = yes;
        self
    }

    /// Вернуть минимальный размер буфера, необходимый для успешного поиска.
    ///
    /// Обычно это соответствует максимальной длине строки (включая её
    /// завершитель), но если включены настройки контекста, то это должно
    /// включать сумму N самых длинных строк.
    ///
    /// Обратите внимание, что это должно учитывать, использует ли тест
    /// поиск по нескольким строкам или нет, поскольку поиск по нескольким
    /// строкам требует возможности поместить весь haystack в память.
    fn minimal_heap_limit(&self, multi_line: bool) -> usize {
        if multi_line {
            1 + self.haystack.len()
        } else if self.before_context == 0 && self.after_context == 0 {
            1 + self.haystack.lines().map(|s| s.len()).max().unwrap_or(0)
        } else {
            let mut lens: Vec<usize> =
                self.haystack.lines().map(|s| s.len()).collect();
            lens.sort();
            lens.reverse();

            let context_count = if self.passthru {
                self.haystack.lines().count()
            } else {
                // Почему мы добавляем 2 здесь? Ну, нам нужно добавить 1,
                // чтобы иметь место для поиска хотя бы одной строки. Мы
                // добавляем ещё одну, потому что реализация иногда будет
                // включать дополнительную строку при обработке контекста.
                // Нет особой хорошей причины, кроме как сохранить
                /// реализацию простой.
                2 + self.before_context + self.after_context
            };

            // Мы добавляем 1 к каждой строке, поскольку `str::lines` не
            // включает завершитель строки.
            lens.into_iter()
                .take(context_count)
                .map(|len| len + 1)
                .sum::<usize>()
        }
    }

    /// Возвращает true тогда и только тогда, когда данная метка должна
    /// быть включена как часть выполнения `test`.
    ///
    /// Включение определяется указанным фильтром. Если фильтр не был
    /// задан, то это всегда возвращает `true`.
    fn include(&self, label: &str) -> bool {
        let re = match self.filter {
            None => return true,
            Some(ref re) => re,
        };
        re.is_match(label)
    }

    /// Configs генерирует набор всех конфигураций поиска, которые должны
    /// быть протестированы. Генерируемые конфигурации основаны на
    /// конфигурации в этом конструкторе.
    fn configs(&self) -> Vec<TesterConfig> {
        let mut configs = vec![];

        let matcher = RegexMatcher::new(&self.pattern);
        let mut builder = SearcherBuilder::new();
        builder
            .line_number(false)
            .invert_match(self.invert_match)
            .binary_detection(self.binary.clone())
            .after_context(self.after_context)
            .before_context(self.before_context)
            .passthru(self.passthru);

        if self.by_line {
            let mut matcher = matcher.clone();
            let mut builder = builder.clone();

            let expected_reader =
                self.expected_no_line_number.as_ref().unwrap().to_string();
            let expected_slice = match self.expected_slice_no_line_number {
                None => expected_reader.clone(),
                Some(ref e) => e.to_string(),
            };
            configs.push(TesterConfig {
                label: "byline-noterm-nonumber".to_string(),
                expected_reader: expected_reader.clone(),
                expected_slice: expected_slice.clone(),
                builder: builder.clone(),
                matcher: matcher.clone(),
            });

            if self.auto_heap_limit {
                builder.heap_limit(Some(self.minimal_heap_limit(false)));
                configs.push(TesterConfig {
                    label: "byline-noterm-nonumber-heaplimit".to_string(),
                    expected_reader: expected_reader.clone(),
                    expected_slice: expected_slice.clone(),
                    builder: builder.clone(),
                    matcher: matcher.clone(),
                });
                builder.heap_limit(None);
            }

            matcher.set_line_term(Some(LineTerminator::byte(b'\n')));
            configs.push(TesterConfig {
                label: "byline-term-nonumber".to_string(),
                expected_reader: expected_reader.clone(),
                expected_slice: expected_slice.clone(),
                builder: builder.clone(),
                matcher: matcher.clone(),
            });

            matcher.every_line_is_candidate(true);
            configs.push(TesterConfig {
                label: "byline-term-nonumber-candidates".to_string(),
                expected_reader: expected_reader.clone(),
                expected_slice: expected_slice.clone(),
                builder: builder.clone(),
                matcher: matcher.clone(),
            });
        }
        if self.by_line && self.line_number {
            let mut matcher = matcher.clone();
            let mut builder = builder.clone();

            let expected_reader =
                self.expected_with_line_number.as_ref().unwrap().to_string();
            let expected_slice = match self.expected_slice_with_line_number {
                None => expected_reader.clone(),
                Some(ref e) => e.to_string(),
            };

            builder.line_number(true);
            configs.push(TesterConfig {
                label: "byline-noterm-number".to_string(),
                expected_reader: expected_reader.clone(),
                expected_slice: expected_slice.clone(),
                builder: builder.clone(),
                matcher: matcher.clone(),
            });

            matcher.set_line_term(Some(LineTerminator::byte(b'\n')));
            configs.push(TesterConfig {
                label: "byline-term-number".to_string(),
                expected_reader: expected_reader.clone(),
                expected_slice: expected_slice.clone(),
                builder: builder.clone(),
                matcher: matcher.clone(),
            });

            matcher.every_line_is_candidate(true);
            configs.push(TesterConfig {
                label: "byline-term-number-candidates".to_string(),
                expected_reader: expected_reader.clone(),
                expected_slice: expected_slice.clone(),
                builder: builder.clone(),
                matcher: matcher.clone(),
            });
        }
        if self.multi_line {
            let mut builder = builder.clone();
            let expected_slice = match self.expected_slice_no_line_number {
                None => {
                    self.expected_no_line_number.as_ref().unwrap().to_string()
                }
                Some(ref e) => e.to_string(),
            };

            builder.multi_line(true);
            configs.push(TesterConfig {
                label: "multiline-nonumber".to_string(),
                expected_reader: expected_slice.clone(),
                expected_slice: expected_slice.clone(),
                builder: builder.clone(),
                matcher: matcher.clone(),
            });

            if self.auto_heap_limit {
                builder.heap_limit(Some(self.minimal_heap_limit(true)));
                configs.push(TesterConfig {
                    label: "multiline-nonumber-heaplimit".to_string(),
                    expected_reader: expected_slice.clone(),
                    expected_slice: expected_slice.clone(),
                    builder: builder.clone(),
                    matcher: matcher.clone(),
                });
                builder.heap_limit(None);
            }
        }
        if self.multi_line && self.line_number {
            let mut builder = builder.clone();
            let expected_slice = match self.expected_slice_with_line_number {
                None => self
                    .expected_with_line_number
                    .as_ref()
                    .unwrap()
                    .to_string(),
                Some(ref e) => e.to_string(),
            };

            builder.multi_line(true);
            builder.line_number(true);
            configs.push(TesterConfig {
                label: "multiline-number".to_string(),
                expected_reader: expected_slice.clone(),
                expected_slice: expected_slice.clone(),
                builder: builder.clone(),
                matcher: matcher.clone(),
            });

            builder.heap_limit(Some(self.minimal_heap_limit(true)));
            configs.push(TesterConfig {
                label: "multiline-number-heaplimit".to_string(),
                expected_reader: expected_slice.clone(),
                expected_slice: expected_slice.clone(),
                builder: builder.clone(),
                matcher: matcher.clone(),
            });
            builder.heap_limit(None);
        }
        configs
    }
}

#[derive(Debug)]
struct TesterConfig {
    label: String,
    expected_reader: String,
    expected_slice: String,
    builder: SearcherBuilder,
    matcher: RegexMatcher,
}

impl TesterConfig {
    /// Выполнить поиск с использованием reader. Это упражняет стратегию
    /// инкрементального поиска, где всё содержимое корпуса не обязательно
    /// находится в памяти одновременно.
    fn search_reader(&self, haystack: &str) -> String {
        let mut sink = KitchenSink::new();
        let mut searcher = self.builder.build();
        let result = searcher.search_reader(
            &self.matcher,
            haystack.as_bytes(),
            &mut sink,
        );
        if let Err(err) = result {
            let label = format!("reader-{}", self.label);
            panic!("error running '{}': {}", label, err);
        }
        String::from_utf8(sink.as_bytes().to_vec()).unwrap()
    }

    /// Выполнить поиск с использованием среза. Это упражняет процедуры
    /// поиска, которые имеют всё содержимое корпуса в памяти одновременно.
    fn search_slice(&self, haystack: &str) -> String {
        let mut sink = KitchenSink::new();
        let mut searcher = self.builder.build();
        let result = searcher.search_slice(
            &self.matcher,
            haystack.as_bytes(),
            &mut sink,
        );
        if let Err(err) = result {
            let label = format!("slice-{}", self.label);
            panic!("error running '{}': {}", label, err);
        }
        String::from_utf8(sink.as_bytes().to_vec()).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(start: usize, end: usize) -> Match {
        Match::new(start, end)
    }

    #[test]
    fn empty_line1() {
        let haystack = b"";
        let matcher = RegexMatcher::new(r"^$");

        assert_eq!(matcher.find_at(haystack, 0), Ok(Some(m(0, 0))));
    }

    #[test]
    fn empty_line2() {
        let haystack = b"\n";
        let matcher = RegexMatcher::new(r"^$");

        assert_eq!(matcher.find_at(haystack, 0), Ok(Some(m(0, 0))));
        assert_eq!(matcher.find_at(haystack, 1), Ok(Some(m(1, 1))));
    }

    #[test]
    fn empty_line3() {
        let haystack = b"\n\n";
        let matcher = RegexMatcher::new(r"^$");

        assert_eq!(matcher.find_at(haystack, 0), Ok(Some(m(0, 0))));
        assert_eq!(matcher.find_at(haystack, 1), Ok(Some(m(1, 1))));
        assert_eq!(matcher.find_at(haystack, 2), Ok(Some(m(2, 2))));
    }

    #[test]
    fn empty_line4() {
        let haystack = b"a\n\nb\n";
        let matcher = RegexMatcher::new(r"^$");

        assert_eq!(matcher.find_at(haystack, 0), Ok(Some(m(2, 2))));
        assert_eq!(matcher.find_at(haystack, 1), Ok(Some(m(2, 2))));
        assert_eq!(matcher.find_at(haystack, 2), Ok(Some(m(2, 2))));
        assert_eq!(matcher.find_at(haystack, 3), Ok(Some(m(5, 5))));
        assert_eq!(matcher.find_at(haystack, 4), Ok(Some(m(5, 5))));
        assert_eq!(matcher.find_at(haystack, 5), Ok(Some(m(5, 5))));
    }

    #[test]
    fn empty_line5() {
        let haystack = b"a\n\nb\nc";
        let matcher = RegexMatcher::new(r"^$");

        assert_eq!(matcher.find_at(haystack, 0), Ok(Some(m(2, 2))));
        assert_eq!(matcher.find_at(haystack, 1), Ok(Some(m(2, 2))));
        assert_eq!(matcher.find_at(haystack, 2), Ok(Some(m(2, 2))));
        assert_eq!(matcher.find_at(haystack, 3), Ok(None));
        assert_eq!(matcher.find_at(haystack, 4), Ok(None));
        assert_eq!(matcher.find_at(haystack, 5), Ok(None));
        assert_eq!(matcher.find_at(haystack, 6), Ok(None));
    }

    #[test]
    fn empty_line6() {
        let haystack = b"a\n";
        let matcher = RegexMatcher::new(r"^$");

        assert_eq!(matcher.find_at(haystack, 0), Ok(Some(m(2, 2))));
        assert_eq!(matcher.find_at(haystack, 1), Ok(Some(m(2, 2))));
        assert_eq!(matcher.find_at(haystack, 2), Ok(Some(m(2, 2))));
    }
}
