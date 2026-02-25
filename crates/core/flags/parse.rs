/*!
Разбирает аргументы командной строки в структурированное и типизированное представление.
*/

use std::{borrow::Cow, collections::BTreeSet, ffi::OsString};

use anyhow::Context;

use crate::flags::{
    Flag, FlagValue,
    defs::FLAGS,
    hiargs::HiArgs,
    lowargs::{LoggingMode, LowArgs, SpecialMode},
};

/// Результат разбора аргументов CLI.
///
/// Это в основном `anyhow::Result<T>`, но с одним дополнительным вариантом,
/// который inhabitated всякий раз, когда ripgrep должен выполнить «специальный»
/// режим. То есть, когда пользователь предоставляет флаги `-h/--help` или
/// `-V/--version`.
///
/// Этот специальный вариант существует, чтобы позволить разбору CLI коротко
/// замыкать как можно быстрее и разумнее. Например, он позволяет разбору CLI
/// избегать чтения конфигурации ripgrep и преобразования низкоуровневых
/// аргументов в представление более высокого уровня.
#[derive(Debug)]
pub(crate) enum ParseResult<T> {
    Special(SpecialMode),
    Ok(T),
    Err(anyhow::Error),
}

impl<T> ParseResult<T> {
    /// Если этот результат — `Ok`, то применяет `then` к нему. В противном
    /// случае возвращает этот результат без изменений.
    fn and_then<U>(
        self,
        mut then: impl FnMut(T) -> ParseResult<U>,
    ) -> ParseResult<U> {
        match self {
            ParseResult::Special(mode) => ParseResult::Special(mode),
            ParseResult::Ok(t) => then(t),
            ParseResult::Err(err) => ParseResult::Err(err),
        }
    }
}

/// Разбирает аргументы CLI и преобразует их в их высокоуровневое представление.
pub(crate) fn parse() -> ParseResult<HiArgs> {
    parse_low().and_then(|low| match HiArgs::from_low_args(low) {
        Ok(hi) => ParseResult::Ok(hi),
        Err(err) => ParseResult::Err(err),
    })
}

/// Разбирает аргументы CLI только в их низкоуровневое представление.
///
/// Это учитывает конфигурацию. То есть, оно попытается прочитать
/// `RIPGREP_CONFIG_PATH` и добавить любые аргументы, найденные там, в начало
/// аргументов, переданных этому процессу.
///
/// Это также установит однопроходные глобальные флаги состояния, такие как
/// уровень журнала и должны ли печататься сообщения.
fn parse_low() -> ParseResult<LowArgs> {
    if let Err(err) = crate::logger::Logger::init() {
        let err = anyhow::anyhow!("не удалось инициализировать логгер: {err}");
        return ParseResult::Err(err);
    }

    let parser = Parser::new();
    let mut low = LowArgs::default();
    if let Err(err) = parser.parse(std::env::args_os().skip(1), &mut low) {
        return ParseResult::Err(err);
    }
    // Хотя мы еще не разобрали файл конфигурации (предполагая, что он
    // существует), мы все еще можем использовать аргументы, данные в CLI,
    // для настройки предпочтений ведения журнала ripgrep. Даже если файл
    // конфигурации изменяет их каким-либо образом, это действительно лучшее,
    // что мы можем сделать. Таким образом, например, люди могут передать
    // `--trace` и видеть любые сообщения, записанные во время разбора
    // файла конфигурации.
    set_log_levels(&low);
    // Прежде чем мы попытаемся учесть конфигурацию, мы можем завершиться
    // досрочно, если включен специальный режим. Это в основном только для
    // вывода версии и помощи, на которые не должна влиять дополнительная
    // конфигурация.
    if let Some(special) = low.special.take() {
        return ParseResult::Special(special);
    }
    // Если конечный пользователь говорит нет конфигурации, то уважаем это.
    if low.no_config {
        log::debug!("не читаем файлы конфигурации, потому что присутствует --no-config");
        return ParseResult::Ok(low);
    }
    // Ищем аргументы из файла конфигурации. Если мы ничего не получили
    // (будь то файл пуст или RIPGREP_CONFIG_PATH не был установлен), то
    // нам не нужно разбирать заново.
    let config_args = crate::flags::config::args();
    if config_args.is_empty() {
        log::debug!("никаких дополнительных аргументов не найдено из файла конфигурации");
        return ParseResult::Ok(low);
    }
    // Конечные аргументы — это просто аргументы из CLI, добавленные в
    // конец аргументов конфигурации.
    let mut final_args = config_args;
    final_args.extend(std::env::args_os().skip(1));

    // Теперь выполняем танец разбора CLI снова.
    let mut low = LowArgs::default();
    if let Err(err) = parser.parse(final_args.into_iter(), &mut low) {
        return ParseResult::Err(err);
    }
    // Сбрасываем уровни сообщений и ведения журнала, поскольку они могли
    // измениться.
    set_log_levels(&low);
    ParseResult::Ok(low)
}

/// Устанавливает глобальные флаги состояния, которые управляют ведением
/// журнала на основе низкоуровневых аргументов.
fn set_log_levels(low: &LowArgs) {
    crate::messages::set_messages(!low.no_messages);
    crate::messages::set_ignore_messages(!low.no_ignore_messages);
    match low.logging {
        Some(LoggingMode::Trace) => {
            log::set_max_level(log::LevelFilter::Trace)
        }
        Some(LoggingMode::Debug) => {
            log::set_max_level(log::LevelFilter::Debug)
        }
        None => log::set_max_level(log::LevelFilter::Warn),
    }
}

/// Разбирает последовательность аргументов CLI в низкоуровневое типизированное
/// представление аргументов.
///
/// Это открыто для тестирования того, что правильные низкоуровневые аргументы
/// разобраны из CLI. Оно просто запускает парсер один раз над аргументами CLI.
/// Оно не настраивает ведение журнала и не читает из файла конфигурации.
///
/// Это предполагает, что данный итератор *не* начинается с имени бинарного файла.
#[cfg(test)]
pub(crate) fn parse_low_raw(
    rawargs: impl IntoIterator<Item = impl Into<OsString>>,
) -> anyhow::Result<LowArgs> {
    let mut args = LowArgs::default();
    Parser::new().parse(rawargs, &mut args)?;
    Ok(args)
}

/// Возвращает метаданные для флага с данным именем.
pub(super) fn lookup(name: &str) -> Option<&'static dyn Flag> {
    // N.B. Создание нового парсера может выглядеть дорогим, но оно только
    // строит trie поиска ровно один раз. То есть, мы получаем `&'static Parser`
    // от `Parser::new()`.
    match Parser::new().find_long(name) {
        FlagLookup::Match(&FlagInfo { flag, .. }) => Some(flag),
        _ => None,
    }
}

/// Парсер для превращения последовательности аргументов командной строки в
/// более строго типизированный набор аргументов.
#[derive(Debug)]
struct Parser {
    /// Единая карта, которая содержит все возможные имена флагов. Это включает
    /// короткие и длинные имена, псевдонимы и отрицания. Это отображает эти
    /// имена в индексы в `info`.
    map: FlagMap,
    /// Карта от ID, возвращаемых `map`, к соответствующей информации о флаге.
    info: Vec<FlagInfo>,
}

impl Parser {
    /// Создает новый парсер.
    ///
    /// Это всегда создает один и тот же парсер и только один раз. Вызывающие
    /// могут вызывать это неоднократно, и парсер будет построен только один раз.
    fn new() -> &'static Parser {
        use std::sync::OnceLock;

        // Поскольку состояние парсера неизменяемо и полностью определено
        /// FLAGS, и поскольку FLAGS — это константа, мы можем инициализировать
        /// его ровно один раз.
        static P: OnceLock<Parser> = OnceLock::new();
        P.get_or_init(|| {
            let mut infos = vec![];
            for &flag in FLAGS.iter() {
                infos.push(FlagInfo {
                    flag,
                    name: Ok(flag.name_long()),
                    kind: FlagInfoKind::Standard,
                });
                for alias in flag.aliases() {
                    infos.push(FlagInfo {
                        flag,
                        name: Ok(alias),
                        kind: FlagInfoKind::Alias,
                    });
                }
                if let Some(byte) = flag.name_short() {
                    infos.push(FlagInfo {
                        flag,
                        name: Err(byte),
                        kind: FlagInfoKind::Standard,
                    });
                }
                if let Some(name) = flag.name_negated() {
                    infos.push(FlagInfo {
                        flag,
                        name: Ok(name),
                        kind: FlagInfoKind::Negated,
                    });
                }
            }
            let map = FlagMap::new(&infos);
            Parser { map, info: infos }
        })
    }

    /// Разбирает данные аргументы CLI в низкоуровневое представление.
    ///
    /// Данный итератор *не* должен начинаться с имени бинарного файла.
    fn parse<I, O>(&self, rawargs: I, args: &mut LowArgs) -> anyhow::Result<()>
    where
        I: IntoIterator<Item = O>,
        O: Into<OsString>,
    {
        let mut p = lexopt::Parser::from_args(rawargs);
        while let Some(arg) = p.next().context("invalid CLI arguments")? {
            let lookup = match arg {
                lexopt::Arg::Value(value) => {
                    args.positional.push(value);
                    continue;
                }
                lexopt::Arg::Short(ch) if ch == 'h' => {
                    // Особый случай -h/--help, поскольку поведение различается
                    // в зависимости от того, дан ли короткий или длинный флаг.
                    args.special = Some(SpecialMode::HelpShort);
                    continue;
                }
                lexopt::Arg::Short(ch) if ch == 'V' => {
                    // Особый случай -V/--version, поскольку поведение различается
                    // в зависимости от того, дан ли короткий или длинный флаг.
                    args.special = Some(SpecialMode::VersionShort);
                    continue;
                }
                lexopt::Arg::Short(ch) => self.find_short(ch),
                lexopt::Arg::Long(name) if name == "help" => {
                    // Особый случай -h/--help, поскольку поведение различается
                    // в зависимости от того, дан ли короткий или длинный флаг.
                    args.special = Some(SpecialMode::HelpLong);
                    continue;
                }
                lexopt::Arg::Long(name) if name == "version" => {
                    // Особый случай -V/--version, поскольку поведение различается
                    // в зависимости от того, дан ли короткий или длинный флаг.
                    args.special = Some(SpecialMode::VersionLong);
                    continue;
                }
                lexopt::Arg::Long(name) => self.find_long(name),
            };
            let mat = match lookup {
                FlagLookup::Match(mat) => mat,
                FlagLookup::UnrecognizedShort(name) => {
                    anyhow::bail!("нераспознанный флаг -{name}")
                }
                FlagLookup::UnrecognizedLong(name) => {
                    let mut msg = format!("нераспознанный флаг --{name}");
                    if let Some(suggest_msg) = suggest(&name) {
                        msg = format!("{msg}\n\n{suggest_msg}");
                    }
                    anyhow::bail!("{msg}")
                }
            };
            let value = if matches!(mat.kind, FlagInfoKind::Negated) {
                // Отрицательные флаги всегда являются переключателями, даже если
                // не отрицательный флаг не является. Например, --context-separator
                // принимает значение, но --no-context-separator — нет.
                FlagValue::Switch(false)
            } else if mat.flag.is_switch() {
                FlagValue::Switch(true)
            } else {
                FlagValue::Value(p.value().with_context(|| {
                    format!("отсутствует значение для флага {mat}")
                })?)
            };
            mat.flag
                .update(value, args)
                .with_context(|| format!("ошибка разбора флага {mat}"))?;
        }
        Ok(())
    }

    /// Ищет флаг по его короткому имени.
    fn find_short(&self, ch: char) -> FlagLookup<'_> {
        if !ch.is_ascii() {
            return FlagLookup::UnrecognizedShort(ch);
        }
        let byte = u8::try_from(ch).unwrap();
        let Some(index) = self.map.find(&[byte]) else {
            return FlagLookup::UnrecognizedShort(ch);
        };
        FlagLookup::Match(&self.info[index])
    }

    /// Ищет флаг по его длинному имени.
    ///
    /// Это также работает для псевдонимов и отрицательных имен.
    fn find_long(&self, name: &str) -> FlagLookup<'_> {
        let Some(index) = self.map.find(name.as_bytes()) else {
            return FlagLookup::UnrecognizedLong(name.to_string());
        };
        FlagLookup::Match(&self.info[index])
    }
}

/// Результат поиска имени флага.
#[derive(Debug)]
enum FlagLookup<'a> {
    /// Поиск нашел совпадение, и метаданные для флага прикреплены.
    Match(&'a FlagInfo),
    /// Данное короткое имя нераспознано.
    UnrecognizedShort(char),
    /// Данное длинное имя нераспознано.
    UnrecognizedLong(String),
}

/// Информация о флаге, связанная с ID флага в карте флагов.
#[derive(Debug)]
struct FlagInfo {
    /// Объект флага и его связанные метаданные.
    flag: &'static dyn Flag,
    /// Фактическое имя, которое хранится в автомате Ахо-Корасик. Когда это
    /// байт, это соответствует короткому односимвольному флагу ASCII.
    /// Фактический шаблон, который находится в автомате Ахо-Корасик, — это
    /// просто один байт.
    name: Result<&'static str, u8>,
    /// Тип флага, который хранится для соответствующего шаблона Ахо-Корасик.
    kind: FlagInfoKind,
}

/// Тип флага, который сопоставляется.
#[derive(Debug)]
enum FlagInfoKind {
    /// Стандартный флаг, например, --passthru.
    Standard,
    /// Отрицание стандартного флага, например, --no-multiline.
    Negated,
    /// Псевдоним для стандартного флага, например, --passthrough.
    Alias,
}

impl std::fmt::Display for FlagInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self.name {
            Ok(long) => write!(f, "--{long}"),
            Err(short) => write!(f, "-{short}", short = char::from(short)),
        }
    }
}

/// Карта от имен флагов (короткие, длинные, отрицательные и псевдонимы) к их ID.
///
/// Как только ID известен, он может быть использован для поиска метаданных
/// флага во внутреннем состоянии парсера.
#[derive(Debug)]
struct FlagMap {
    map: std::collections::HashMap<Vec<u8>, usize>,
}

impl FlagMap {
    /// Создает новую карту флагов для данной информации о флаге.
    ///
    /// Индекс каждой информации о флаге соответствует ее ID.
    fn new(infos: &[FlagInfo]) -> FlagMap {
        let mut map = std::collections::HashMap::with_capacity(infos.len());
        for (i, info) in infos.iter().enumerate() {
            match info.name {
                Ok(name) => {
                    assert_eq!(None, map.insert(name.as_bytes().to_vec(), i));
                }
                Err(byte) => {
                    assert_eq!(None, map.insert(vec![byte], i));
                }
            }
        }
        FlagMap { map }
    }

    /// Ищет совпадение `name` в данном автомате Ахо-Корасик.
    ///
    /// Это возвращает совпадение только если найденное имеет длину,
    /// эквивалентную длине данного имени.
    fn find(&self, name: &[u8]) -> Option<usize> {
        self.map.get(name).copied()
    }
}

/// Возможно возвращает сообщение с предложением флагов, похожих на данное имя.
///
/// Данное имя должно быть флагом, данным пользователем (без ведущих тире),
/// который был нераспознан. Это пытается найти существующие флаги, которые
/// похожи на данное.
fn suggest(unrecognized: &str) -> Option<String> {
    let similars = find_similar_names(unrecognized);
    if similars.is_empty() {
        return None;
    }
    let list = similars
        .into_iter()
        .map(|name| format!("--{name}"))
        .collect::<Vec<String>>()
        .join(", ");
    Some(format!("similar flags that are available: {list}"))
}

/// Возвращает последовательность имен, похожих на данное нераспознанное имя.
fn find_similar_names(unrecognized: &str) -> Vec<&'static str> {
    // Порог сходства Джаккарда, при котором мы считаем два имени флагов
    /// достаточно похожими, чтобы предложить их конечному пользователю.
    ///
    /// Это значение было определено некоторыми специальными экспериментами.
    /// Может потребоваться дальнейшая корректировка.
    const THRESHOLD: f64 = 0.4;

    let mut similar = vec![];
    let bow_given = ngrams(unrecognized);
    for &flag in FLAGS.iter() {
        let name = flag.name_long();
        let bow = ngrams(name);
        if jaccard_index(&bow_given, &bow) >= THRESHOLD {
            similar.push(name);
        }
        if let Some(name) = flag.name_negated() {
            let bow = ngrams(name);
            if jaccard_index(&bow_given, &bow) >= THRESHOLD {
                similar.push(name);
            }
        }
        for name in flag.aliases() {
            let bow = ngrams(name);
            if jaccard_index(&bow_given, &bow) >= THRESHOLD {
                similar.push(name);
            }
        }
    }
    similar
}

/// «Мешок слов» — это набор n-грамм.
type BagOfWords<'a> = BTreeSet<Cow<'a, [u8]>>;

/// Возвращает индекс Джаккарда (мера сходства) между наборами n-грамм.
fn jaccard_index(ngrams1: &BagOfWords<'_>, ngrams2: &BagOfWords<'_>) -> f64 {
    let union = u32::try_from(ngrams1.union(ngrams2).count())
        .expect("fewer than u32::MAX flags");
    let intersection = u32::try_from(ngrams1.intersection(ngrams2).count())
        .expect("fewer than u32::MAX flags");
    f64::from(intersection) / f64::from(union)
}

/// Возвращает все 3-граммы в данном срезе.
///
/// Если срез не содержит 3-грамму, то она искусственно создается путем
/// дополнения его символом, который никогда не появится в имени флага.
fn ngrams(flag_name: &str) -> BagOfWords<'_> {
    // Мы разрешаем только имена флагов ASCII, поэтому мы можем просто
    // использовать байты.
    let slice = flag_name.as_bytes();
    let seq: Vec<Cow<[u8]>> = match slice.len() {
        0 => vec![Cow::Owned(b"!!!".to_vec())],
        1 => vec![Cow::Owned(vec![slice[0], b'!', b'!'])],
        2 => vec![Cow::Owned(vec![slice[0], slice[1], b'!'])],
        _ => slice.windows(3).map(Cow::Borrowed).collect(),
    };
    BTreeSet::from_iter(seq)
}
