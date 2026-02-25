use std::{cell::RefCell, io, path::Path, sync::Arc};

use {
    bstr::ByteSlice,
    termcolor::{HyperlinkSpec, WriteColor},
};

use crate::util::DecimalFormatter;

use self::aliases::HYPERLINK_PATTERN_ALIASES;

mod aliases;

/// Конфигурация гиперссылок.
///
/// Эта конфигурация указывает как [формат гиперссылки](HyperlinkFormat),
/// так и [окружение](HyperlinkConfig) для интерполяции подмножества
/// переменных. Конкретное подмножество включает переменные, которые
/// предназначены быть неизменными в течение времени жизни процесса,
/// такие как имя хоста машины.
///
/// Конфигурация гиперссылки может быть предоставлена построителям принтеров,
/// таким как [`StandardBuilder::hyperlink`](crate::StandardBuilder::hyperlink).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HyperlinkConfig(Arc<HyperlinkConfigInner>);

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct HyperlinkConfigInner {
    env: HyperlinkEnvironment,
    format: HyperlinkFormat,
}

impl HyperlinkConfig {
    /// Создаёт новую конфигурацию из окружения и формата.
    pub fn new(
        env: HyperlinkEnvironment,
        format: HyperlinkFormat,
    ) -> HyperlinkConfig {
        HyperlinkConfig(Arc::new(HyperlinkConfigInner { env, format }))
    }

    /// Возвращает окружение гиперссылок в этой конфигурации.
    pub(crate) fn environment(&self) -> &HyperlinkEnvironment {
        &self.0.env
    }

    /// Возвращает формат гиперссылок в этой конфигурации.
    pub(crate) fn format(&self) -> &HyperlinkFormat {
        &self.0.format
    }
}

/// Формат гиперссылки с переменными.
///
/// Это может быть создано путём парсинга строки с помощью `HyperlinkFormat::from_str`.
///
/// Формат по умолчанию пуст. Пустой формат действителен и эффективно
/// отключает гиперссылки.
///
/// # Пример
///
/// ```
/// use grep_printer::HyperlinkFormat;
///
/// let fmt = "vscode".parse::<HyperlinkFormat>()?;
/// assert_eq!(fmt.to_string(), "vscode://file{path}:{line}:{column}");
///
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HyperlinkFormat {
    parts: Vec<Part>,
    is_line_dependent: bool,
}

impl HyperlinkFormat {
    /// Создаёт пустой формат гиперссылки.
    pub fn empty() -> HyperlinkFormat {
        HyperlinkFormat::default()
    }

    /// Возвращает true, если этот формат пуст.
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    /// Создаёт [`HyperlinkConfig`] из этого формата и данного окружения.
    pub fn into_config(self, env: HyperlinkEnvironment) -> HyperlinkConfig {
        HyperlinkConfig::new(env, self)
    }

    /// Возвращает true, если формат может создавать зависимые от строки гиперссылки.
    pub(crate) fn is_line_dependent(&self) -> bool {
        self.is_line_dependent
    }
}

impl std::str::FromStr for HyperlinkFormat {
    type Err = HyperlinkFormatError;

    fn from_str(s: &str) -> Result<HyperlinkFormat, HyperlinkFormatError> {
        use self::HyperlinkFormatErrorKind::*;

        #[derive(Debug)]
        enum State {
            Verbatim,
            VerbatimCloseVariable,
            OpenVariable,
            InVariable,
        }

        let mut builder = FormatBuilder::new();
        let input = match HyperlinkAlias::find(s) {
            Some(alias) => alias.format(),
            None => s,
        };
        let mut name = String::new();
        let mut state = State::Verbatim;
        let err = |kind| HyperlinkFormatError { kind };
        for ch in input.chars() {
            state = match state {
                State::Verbatim => {
                    if ch == '{' {
                        State::OpenVariable
                    } else if ch == '}' {
                        State::VerbatimCloseVariable
                    } else {
                        builder.append_char(ch);
                        State::Verbatim
                    }
                }
                State::VerbatimCloseVariable => {
                    if ch == '}' {
                        builder.append_char('}');
                        State::Verbatim
                    } else {
                        return Err(err(InvalidCloseVariable));
                    }
                }
                State::OpenVariable => {
                    if ch == '{' {
                        builder.append_char('{');
                        State::Verbatim
                    } else {
                        name.clear();
                        if ch == '}' {
                            builder.append_var(&name)?;
                            State::Verbatim
                        } else {
                            name.push(ch);
                            State::InVariable
                        }
                    }
                }
                State::InVariable => {
                    if ch == '}' {
                        builder.append_var(&name)?;
                        State::Verbatim
                    } else {
                        name.push(ch);
                        State::InVariable
                    }
                }
            };
        }
        match state {
            State::Verbatim => builder.build(),
            State::VerbatimCloseVariable => Err(err(InvalidCloseVariable)),
            State::OpenVariable | State::InVariable => {
                Err(err(UnclosedVariable))
            }
        }
    }
}

impl std::fmt::Display for HyperlinkFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for part in self.parts.iter() {
            part.fmt(f)?;
        }
        Ok(())
    }
}

/// Псевдоним для формата гиперссылки.
///
/// Псевдонимы гиперссылок встроены, поэтому они содержат статические значения.
/// Некоторые их функции доступны в const блоках.
#[derive(Clone, Debug)]
pub struct HyperlinkAlias {
    name: &'static str,
    description: &'static str,
    format: &'static str,
    display_priority: Option<i16>,
}

impl HyperlinkAlias {
    /// Возвращает имя псевдонима.
    pub const fn name(&self) -> &str {
        self.name
    }

    /// Возвращает очень краткое описание этого псевдонима гиперссылки.
    pub const fn description(&self) -> &str {
        self.description
    }

    /// Возвращает приоритет отображения этого псевдонима.
    ///
    /// Если приоритет не установлен, возвращается `None`.
    ///
    /// Приоритет отображения должен отражать некоторый специальный статус,
    /// связанный с псевдонимом. Например, псевдонимы `default` и `none` имеют
    /// приоритет отображения. Это предназначено для поощрения их перечисления
    /// первыми в документации.
    ///
    /// Более низкий приоритет отображения означает, что псевдоним должен
    /// показываться перед псевдонимами с более высоким (или отсутствующим)
    /// приоритетом отображения.
    ///
    /// Вызывающие не могут полагаться на какое-либо конкретное значение
    /// приоритета отображения, остающееся стабильным между совместимыми
    /// с semver выпусками этого крейта.
    pub const fn display_priority(&self) -> Option<i16> {
        self.display_priority
    }

    /// Возвращает строку формата псевдонима.
    const fn format(&self) -> &'static str {
        self.format
    }

    /// Ищет псевдоним гиперссылки, определённый данным именем.
    ///
    /// Если он не существует, возвращается `None`.
    fn find(name: &str) -> Option<&HyperlinkAlias> {
        HYPERLINK_PATTERN_ALIASES
            .binary_search_by_key(&name, |alias| alias.name())
            .map(|i| &HYPERLINK_PATTERN_ALIASES[i])
            .ok()
    }
}

/// Статическое окружение для интерполяции гиперссылок.
///
/// Это окружение позволяет устанавливать значения переменных, используемых
/// в интерполяции гиперссылок, которые не ожидаются изменяющимися в течение
/// времени жизни программы. То есть эти значения инвариантны.
///
/// В настоящее время это включает имя хоста и префикс дистрибутива WSL.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HyperlinkEnvironment {
    host: Option<String>,
    wsl_prefix: Option<String>,
}

impl HyperlinkEnvironment {
    /// Создаёт новое пустое окружение гиперссылок.
    pub fn new() -> HyperlinkEnvironment {
        HyperlinkEnvironment::default()
    }

    /// Устанавливает переменную `{host}`, которая заполняет любые компоненты
    /// имени хоста гиперссылки.
    ///
    /// Можно получить имя хоста в текущем окружении через функцию `hostname`
    /// в крейте `grep-cli`.
    pub fn host(&mut self, host: Option<String>) -> &mut HyperlinkEnvironment {
        self.host = host;
        self
    }

    /// Устанавливает переменную `{wslprefix}`, которая содержит префикс
    /// дистрибутива WSL. Пример значения: `wsl$/Ubuntu`. Имя дистрибутива
    /// обычно можно получить из переменной окружения `WSL_DISTRO_NAME`.
    pub fn wsl_prefix(
        &mut self,
        wsl_prefix: Option<String>,
    ) -> &mut HyperlinkEnvironment {
        self.wsl_prefix = wsl_prefix;
        self
    }
}

/// Ошибка, которая может возникнуть при парсинге формата гиперссылки.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HyperlinkFormatError {
    kind: HyperlinkFormatErrorKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum HyperlinkFormatErrorKind {
    /// Это происходит, когда в формате нет переменных.
    NoVariables,
    /// Это происходит, когда переменная {path} отсутствует.
    NoPathVariable,
    /// Это происходит, когда переменная {line} отсутствует, при этом
    /// переменная {column} присутствует.
    NoLineVariable,
    /// Это происходит, когда используется неизвестная переменная.
    InvalidVariable(String),
    /// Формат не начинается с допустимой схемы.
    InvalidScheme,
    /// Это происходит, когда найден неэкранированный `}` без соответствующего
    /// `{` перед ним.
    InvalidCloseVariable,
    /// Это происходит, когда найден `{` без соответствующего `}` после него.
    UnclosedVariable,
}

impl std::error::Error for HyperlinkFormatError {}

impl std::fmt::Display for HyperlinkFormatError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use self::HyperlinkFormatErrorKind::*;

        match self.kind {
            NoVariables => {
                let mut aliases = hyperlink_aliases();
                aliases.sort_by_key(|alias| {
                    alias.display_priority().unwrap_or(i16::MAX)
                });
                let names: Vec<&str> =
                    aliases.iter().map(|alias| alias.name()).collect();
                write!(
                    f,
                    "в формате гиперссылки требуется как минимум переменная \
                     {{path}}, или используйте допустимый псевдоним: \
                     {aliases}",
                    aliases = names.join(", "),
                )
            }
            NoPathVariable => {
                write!(
                    f,
                    "в формате гиперссылки требуется переменная {{path}}",
                )
            }
            NoLineVariable => {
                write!(
                    f,
                    "формат гиперссылки содержит переменную {{column}}, \
                     но переменная {{line}} отсутствует",
                )
            }
            InvalidVariable(ref name) => {
                write!(
                    f,
                    "недопустимая переменная формата гиперссылки: '{name}', \
                     выберите из: path, line, column, host, wslprefix",
                )
            }
            InvalidScheme => {
                write!(
                    f,
                    "формат гиперссылки должен начинаться с допустимой схемы URL, \
                     т.е. [0-9A-Za-z+-.]+:",
                )
            }
            InvalidCloseVariable => {
                write!(
                    f,
                    "неоткрытая переменная: найден '}}' без соответствующего \
                     '{{' перед ним",
                )
            }
            UnclosedVariable => {
                write!(
                    f,
                    "незакрытая переменная: найден '{{' без соответствующего \
                     '}}' после него",
                )
            }
        }
    }
}

/// Построитель для `HyperlinkFormat`.
///
/// Как только `HyperlinkFormat` создан, он неизменяем.
#[derive(Debug)]
struct FormatBuilder {
    parts: Vec<Part>,
}

impl FormatBuilder {
    /// Создаёт новый построитель формата гиперссылки.
    fn new() -> FormatBuilder {
        FormatBuilder { parts: vec![] }
    }

    /// Добавляет статический текст.
    fn append_slice(&mut self, text: &[u8]) -> &mut FormatBuilder {
        if let Some(Part::Text(contents)) = self.parts.last_mut() {
            contents.extend_from_slice(text);
        } else if !text.is_empty() {
            self.parts.push(Part::Text(text.to_vec()));
        }
        self
    }

    /// Добавляет один символ.
    fn append_char(&mut self, ch: char) -> &mut FormatBuilder {
        self.append_slice(ch.encode_utf8(&mut [0; 4]).as_bytes())
    }

    /// Добавляет переменную с данным именем. Если имя не распознано,
    /// возвращается ошибка.
    fn append_var(
        &mut self,
        name: &str,
    ) -> Result<&mut FormatBuilder, HyperlinkFormatError> {
        let part = match name {
            "host" => Part::Host,
            "wslprefix" => Part::WSLPrefix,
            "path" => Part::Path,
            "line" => Part::Line,
            "column" => Part::Column,
            unknown => {
                let err = HyperlinkFormatError {
                    kind: HyperlinkFormatErrorKind::InvalidVariable(
                        unknown.to_string(),
                    ),
                };
                return Err(err);
            }
        };
        self.parts.push(part);
        Ok(self)
    }

    /// Строит формат.
    fn build(&self) -> Result<HyperlinkFormat, HyperlinkFormatError> {
        self.validate()?;
        Ok(HyperlinkFormat {
            parts: self.parts.clone(),
            is_line_dependent: self.parts.contains(&Part::Line),
        })
    }

    /// Проверяет, что формат правильно сформирован.
    fn validate(&self) -> Result<(), HyperlinkFormatError> {
        use self::HyperlinkFormatErrorKind::*;

        let err = |kind| HyperlinkFormatError { kind };
        // Пустой формат допустим. Это просто означает, что поддержка
        // гиперссылок отключена.
        if self.parts.is_empty() {
            return Ok(());
        }
        // Если все части — просто текст, то переменных нет. Это
        // вероятно ссылка на недопустимый псевдоним.
        if self.parts.iter().all(|p| matches!(*p, Part::Text(_))) {
            return Err(err(NoVariables));
        }
        // Даже если у нас есть другие переменные, отсутствие переменной path
        // означает, что гиперссылка не может работать так, как задумано.
        if !self.parts.contains(&Part::Path) {
            return Err(err(NoPathVariable));
        }
        // Если используется переменная {column}, то нам также нужна
        // переменная {line}, иначе {column} не может работать.
        if self.parts.contains(&Part::Column)
            && !self.parts.contains(&Part::Line)
        {
            return Err(err(NoLineVariable));
        }
        self.validate_scheme()
    }

    /// Проверяет, что формат начинается с допустимой схемы. Проверка
    /// выполняется согласно тому, как схема определена в разделах 2.1[1] и
    /// 5[2] RFC 1738. Кратко, схема — это:
    ///
    /// scheme = 1*[ lowalpha | digit | "+" | "-" | "." ]
    ///
    /// но регистронезависима.
    ///
    /// [1]: https://datatracker.ietf.org/doc/html/rfc1738#section-2.1
    /// [2]: https://datatracker.ietf.org/doc/html/rfc1738#section-5
    fn validate_scheme(&self) -> Result<(), HyperlinkFormatError> {
        let err_invalid_scheme = HyperlinkFormatError {
            kind: HyperlinkFormatErrorKind::InvalidScheme,
        };
        let Some(Part::Text(part)) = self.parts.first() else {
            return Err(err_invalid_scheme);
        };
        let Some(colon) = part.find_byte(b':') else {
            return Err(err_invalid_scheme);
        };
        let scheme = &part[..colon];
        if scheme.is_empty() {
            return Err(err_invalid_scheme);
        }
        let is_valid_scheme_char = |byte| match byte {
            b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' | b'+' | b'-' | b'.' => {
                true
            }
            _ => false,
        };
        if !scheme.iter().all(|&b| is_valid_scheme_char(b)) {
            return Err(err_invalid_scheme);
        }
        Ok(())
    }
}

/// Часть формата гиперссылки.
///
/// Последовательность этого соответствует полному формату. (Не все
/// последовательности допустимы.)
#[derive(Clone, Debug, Eq, PartialEq)]
enum Part {
    /// Статический текст.
    ///
    /// Мы используем `Vec<u8>` здесь (и в целом рассматриваем строку формата
    /// как последовательность байтов), потому что пути к файлам могут быть
    /// произвольными байтами. Редкий случай, но нет веской причины
    /// спотыкаться об этом.
    Text(Vec<u8>),
    /// Переменная для имени хоста.
    Host,
    /// Переменная для префикса пути WSL.
    WSLPrefix,
    /// Переменная для пути к файлу.
    Path,
    /// Переменная для номера строки.
    Line,
    /// Переменная для номера столбца.
    Column,
}

impl Part {
    /// Интерполирует эту часть, используя данное `env` и `values`, и записывает
    /// результат интерполяции в предоставленный буфер.
    fn interpolate_to(
        &self,
        env: &HyperlinkEnvironment,
        values: &Values,
        dest: &mut Vec<u8>,
    ) {
        match *self {
            Part::Text(ref text) => dest.extend_from_slice(text),
            Part::Host => dest.extend_from_slice(
                env.host.as_ref().map(|s| s.as_bytes()).unwrap_or(b""),
            ),
            Part::WSLPrefix => dest.extend_from_slice(
                env.wsl_prefix.as_ref().map(|s| s.as_bytes()).unwrap_or(b""),
            ),
            Part::Path => dest.extend_from_slice(&values.path.0),
            Part::Line => {
                let line = DecimalFormatter::new(values.line.unwrap_or(1));
                dest.extend_from_slice(line.as_bytes());
            }
            Part::Column => {
                let column = DecimalFormatter::new(values.column.unwrap_or(1));
                dest.extend_from_slice(column.as_bytes());
            }
        }
    }
}

impl std::fmt::Display for Part {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Part::Text(text) => write!(f, "{}", String::from_utf8_lossy(text)),
            Part::Host => write!(f, "{{host}}"),
            Part::WSLPrefix => write!(f, "{{wslprefix}}"),
            Part::Path => write!(f, "{{path}}"),
            Part::Line => write!(f, "{{line}}"),
            Part::Column => write!(f, "{{column}}"),
        }
    }
}

/// Значения для замены переменных формата.
///
/// Это состоит только из значений, которые зависят от каждого пути или
/// совпадения, которое выводится. Значения, которые инвариантны в течение
/// времени жизни процесса, устанавливаются через [`HyperlinkEnvironment`].
#[derive(Clone, Debug)]
pub(crate) struct Values<'a> {
    path: &'a HyperlinkPath,
    line: Option<u64>,
    column: Option<u64>,
}

impl<'a> Values<'a> {
    /// Создаёт новый набор значений, начиная с данного пути.
    ///
    /// Вызывающие могут также установить номер строки и столбца, используя
    /// методы-мутаторы.
    pub(crate) fn new(path: &'a HyperlinkPath) -> Values<'a> {
        Values { path, line: None, column: None }
    }

    /// Устанавливает номер строки для этих значений.
    ///
    /// Если номер строки не установлен и формат гиперссылки содержит
    /// переменную `{line}`, то она автоматически интерполируется
    /// значением `1`.
    pub(crate) fn line(mut self, line: Option<u64>) -> Values<'a> {
        self.line = line;
        self
    }

    /// Устанавливает номер столбца для этих значений.
    ///
    /// Если номер столбца не установлен и формат гиперссылки содержит
    /// переменную `{column}`, то она автоматически интерполируется
    /// значением `1`.
    pub(crate) fn column(mut self, column: Option<u64>) -> Values<'a> {
        self.column = column;
        self
    }
}

/// Абстракция для интерполяции формата гиперссылки со значениями для
/// каждой переменной.
///
/// Интерполяция переменных происходит через два различных источника.
/// Первый — через `HyperlinkEnvironment` для значений, которые ожидаются
/// инвариантными. Это происходит из `HyperlinkConfig`, использованного
/// для создания этого интерполятора. Второй источник — через `Values`,
/// который предоставляется в `Interpolator::begin`. `Values` содержит
/// такие вещи, как путь к файлу, номер строки и номер столбца.
#[derive(Clone, Debug)]
pub(crate) struct Interpolator {
    config: HyperlinkConfig,
    buf: RefCell<Vec<u8>>,
}

impl Interpolator {
    /// Создаёт новый интерполятор для данной конфигурации формата гиперссылки.
    pub(crate) fn new(config: &HyperlinkConfig) -> Interpolator {
        Interpolator { config: config.clone(), buf: RefCell::new(vec![]) }
    }

    /// Начинает интерполяцию с данными значениями, записывая гиперссылку
    /// в `wtr`. Последующие записи в `wtr`, до вызова `Interpolator::end`,
    /// являются меткой для гиперссылки.
    ///
    /// Это возвращает статус интерполятора, который указывает, была ли
    /// записана гиперссылка. Она может не быть записана, например, если
    /// базовый writer не поддерживает гиперссылки или если формат
    /// гиперссылки пуст. Статус должен быть предоставлен `Interpolator::end`
    /// как инструкция для закрытия гиперссылки или нет.
    pub(crate) fn begin<W: WriteColor>(
        &self,
        values: &Values,
        mut wtr: W,
    ) -> io::Result<InterpolatorStatus> {
        if self.config.format().is_empty()
            || !wtr.supports_hyperlinks()
            || !wtr.supports_color()
        {
            return Ok(InterpolatorStatus::inactive());
        }
        let mut buf = self.buf.borrow_mut();
        buf.clear();
        for part in self.config.format().parts.iter() {
            part.interpolate_to(self.config.environment(), values, &mut buf);
        }
        let spec = HyperlinkSpec::open(&buf);
        wtr.set_hyperlink(&spec)?;
        Ok(InterpolatorStatus { active: true })
    }

    /// Записывает правильные escape-последовательности в `wtr` для закрытия
    /// любой существующей гиперссылки, отмечая конец метки гиперссылки.
    ///
    /// Статус, который передаётся, должен быть возвращён из соответствующего
    /// вызова `Interpolator::begin`. Поскольку `begin` может не записать
    /// гиперссылку (например, если базовый writer не поддерживает гиперссылки),
    /// следует, что `finish` не должен закрывать гиперссылку, которая никогда
    /// не была открыта. Статус указывает, была ли открыта гиперссылка или нет.
    pub(crate) fn finish<W: WriteColor>(
        &self,
        status: InterpolatorStatus,
        mut wtr: W,
    ) -> io::Result<()> {
        if !status.active {
            return Ok(());
        }
        wtr.set_hyperlink(&HyperlinkSpec::close())
    }
}

/// Статус, указывающий, была ли записана гиперссылка или нет.
///
/// Это создаётся `Interpolator::begin` и используется `Interpolator::finish`
/// для определения, была ли фактически открыта гиперссылка или нет. Если
/// она не была открыта, то завершение интерполяции является операцией
/// без действия.
#[derive(Debug)]
pub(crate) struct InterpolatorStatus {
    active: bool,
}

impl InterpolatorStatus {
    /// Создаёт неактивный статус интерполятора.
    #[inline]
    pub(crate) fn inactive() -> InterpolatorStatus {
        InterpolatorStatus { active: false }
    }
}

/// Представляет часть `{path}` гиперссылки.
///
/// Это значение для использования как есть в гиперссылке, преобразованное
/// из пути к файлу ОС.
#[derive(Clone, Debug)]
pub(crate) struct HyperlinkPath(Vec<u8>);

impl HyperlinkPath {
    /// Возвращает путь гиперссылки из пути ОС.
    #[cfg(unix)]
    pub(crate) fn from_path(original_path: &Path) -> Option<HyperlinkPath> {
        use std::os::unix::ffi::OsStrExt;

        // Мы канонизируем путь, чтобы получить его абсолютную версию
        // без каких-либо `.`, `..` или лишних разделителей. К сожалению,
        // это также удаляет симлинки, и в теории было бы неплохо их
        // сохранить. Возможно, ещё проще, мы могли бы просто объединить
        // текущий рабочий каталог с путём и покончить с этим. Было
        // некоторое обсуждение этого в PR#2483, и в целом кажется,
        // есть некоторая неопределённость о том, в какой степени гиперссылки
        // с такими вещами, как `..`, на самом деле работают. Поэтому пока
        // мы делаем самое безопасное возможное, даже хотя я думаю, что
        // это может привести к худшему пользовательскому опыту. (Потому
        // что это означает, что путь, на который вы нажимаете, и фактический
        // путь, который будет пройден, различаются, даже хотя они по сути
        // ссылаются на один и тот же файл.)
        //
        // Также есть потенциальная проблема, что канонизация пути
        // дорога, поскольку она может касаться файловой системы. Это,
        // вероятно, менее проблематично, поскольку гиперссылки создаются
        // только когда они поддерживаются, т.е. при записи в tty.
        //
        // [1]: https://github.com/BurntSushi/ripgrep/pull/2483
        let path = match original_path.canonicalize() {
            Ok(path) => path,
            Err(err) => {
                log::debug!(
                    "hyperlink creation for {:?} failed, error occurred \
                     during path canonicalization: {}",
                    original_path,
                    err,
                );
                return None;
            }
        };
        let bytes = path.as_os_str().as_bytes();
        // Это не должно быть возможным, поскольку можно представить,
        // что канонизация всегда должна возвращать абсолютный путь.
        // Но на самом деле это не гарантировано POSIX, поэтому мы
        // проверяем, верно ли это, и отказываемся создавать гиперссылку
        // из относительного пути, если это не так.
        if !bytes.starts_with(b"/") {
            log::debug!(
                "hyperlink creation for {:?} failed, canonicalization \
                 returned {:?}, which does not start with a slash",
                original_path,
                path,
            );
            return None;
        }
        Some(HyperlinkPath::encode(bytes))
    }

    /// Возвращает путь гиперссылки из пути ОС.
    #[cfg(windows)]
    pub(crate) fn from_path(original_path: &Path) -> Option<HyperlinkPath> {
        // В Windows мы используем `std::path::absolute` вместо `Path::canonicalize`,
        // так как это может быть намного быстрее, поскольку не касается
        // файловой системы. Это обёртывает API [`GetFullPathNameW`][1],
        // кроме буквенных путей (тех, которые начинаются с `\\?\`,
        // см. [документацию][2] для деталей).
        //
        // Здесь мы удаляем любые префиксы буквенных путей, поскольку мы не
        // можем использовать их в гиперссылках anyway. Это может произойти
        // только если пользователь явно предоставляет буквенный путь как
        // ввод, который уже должен быть абсолютным:
        //
        //   \\?\C:\dir\file.txt           (локальный путь)
        //   \\?\UNC\server\dir\file.txt   (сетевая папка)
        //
        // Префикс `\\?\` постоянен для буквенных путей и может быть за которым
        // следует `UNC\` (универсальное соглашение об именах), которое
        // обозначает сетевую папку.
        //
        // Учитывая, что формат URL по умолчанию в Windows — file://{path},
        // мы должны вернуть следующее из этой функции:
        //
        //   /C:/dir/file.txt        (локальный путь)
        //   //server/dir/file.txt   (сетевая папка)
        //
        // Что производит следующие ссылки:
        //
        //   file:///C:/dir/file.txt        (локальный путь)
        //   file:////server/dir/file.txt   (сетевая папка)
        //
        // Это подставляет переменную {path} ожидаемым значением для
        // наиболее распространённых путей DOS, но с другой стороны,
        // сетевые пути начинаются с одинарного слэша, что может быть
        // неожиданно. Хотя, кажется, работает?
        //
        // Заметьте, что следующий синтаксис URL также кажется допустимым?
        //
        //   file://server/dir/file.txt
        //
        // Но начальная реализация этой процедуры выбрала формат выше.
        //
        // Также заметьте, что синтаксис file://C:/dir/file.txt не корректен,
        // даже хотя он часто работает на практике.
        //
        // В конце концов, этот выбор был подтверждён VSCode, чей формат:
        //
        //   vscode://file{path}:{line}:{column}
        //
        // и который правильно понимает следующий формат URL для сетевых
        // дисков:
        //
        //   vscode://file//server/dir/file.txt:1:1
        //
        // Он не парсит никакое другое количество слэшей в "file//server"
        // как сетевой путь.
        //
        // [1]: https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfullpathnamew
        // [2]: https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file

        const WIN32_NAMESPACE_PREFIX: &str = r"\\?\";
        const UNC_PREFIX: &str = r"UNC\";

        let path = match std::path::absolute(original_path) {
            Ok(path) => path,
            Err(err) => {
                log::debug!(
                    "hyperlink creation for {:?} failed, error occurred \
                     during conversion to absolute path: {}",
                    original_path,
                    err,
                );
                return None;
            }
        };
        // Мы преобразуем путь в строку для более лёгкого управления. Если
        // он не был валидным UTF-16 (и таким образом не мог быть без потерь
        // транскодирован в UTF-8), то мы просто сдаёмся. Неясно, могли бы
        // мы сделать из него осмысленную гиперссылку anyway. И это должно
        // быть исключительно редким случаем.
        let mut string = match path.to_str() {
            Some(string) => string,
            None => {
                log::debug!(
                    "hyperlink creation for {:?} failed, path is not \
                     valid UTF-8",
                    original_path,
                );
                return None;
            }
        };

        // Удаляем префиксы буквенных путей (см. комментарий выше для деталей).
        if string.starts_with(WIN32_NAMESPACE_PREFIX) {
            string = &string[WIN32_NAMESPACE_PREFIX.len()..];

            // Удаляем префикс UNC, если он есть, но сохраняем ведущий слэш.
            if string.starts_with(UNC_PREFIX) {
                string = &string[(UNC_PREFIX.len() - 1)..];
            }
        } else if string.starts_with(r"\\") || string.starts_with(r"//") {
            // Удаляем один из двух ведущих слэшей сетевых путей, он будет добавлен обратно.
            string = &string[1..];
        }

        // Наконец, добавляем ведущий слэш. В случае локального файла это
        // превращает C:\foo\bar в /C:\foo\bar (и затем процентное
        // кодирование превращает это в /C:/foo/bar). В случае сетевой
        // папки это превращает \share\foo\bar в /\share/foo/bar (и затем
        // процентное кодирование превращает это в //share/foo/bar).
        let with_slash = format!("/{string}");
        Some(HyperlinkPath::encode(with_slash.as_bytes()))
    }

    /// Для других платформ (не windows, не unix), возвращает None и логирует отладочное сообщение.
    #[cfg(not(any(windows, unix)))]
    pub(crate) fn from_path(original_path: &Path) -> Option<HyperlinkPath> {
        log::debug!("гиперссылки не поддерживаются на этой платформе");
        None
    }

    /// Кодирует путь в процентах.
    ///
    /// Буквенно-цифровые символы ASCII и "-", ".", "_", "~" не зарезервированы
    /// согласно разделу 2.3 RFC 3986 (Uniform Resource Identifier (URI):
    /// Generic Syntax) и не кодируются. Другие символы ASCII, кроме
    /// "/" и ":", кодируются в процентах, и "\" заменяется на "/" в Windows.
    ///
    /// Раздел 4 RFC 8089 (The "file" URI Scheme) не требует точных
    /// требований кодирования для символов не ASCII, и эта реализация
    /// оставляет их не закодированными. В Windows функция UrlCreateFromPathW
    /// не кодирует символы не ASCII. Выполнение этого с путями,
    /// кодированными в UTF-8, создаёт недействительные URL file:// на
    /// этой платформе.
    fn encode(input: &[u8]) -> HyperlinkPath {
        let mut result = Vec::with_capacity(input.len());
        for &byte in input.iter() {
            match byte {
                b'0'..=b'9'
                | b'A'..=b'Z'
                | b'a'..=b'z'
                | b'/'
                | b':'
                | b'-'
                | b'.'
                | b'_'
                | b'~'
                | 128.. => {
                    result.push(byte);
                }
                #[cfg(windows)]
                b'\\' => {
                    result.push(b'/');
                }
                _ => {
                    const HEX: &[u8] = b"0123456789ABCDEF";
                    result.push(b'%');
                    result.push(HEX[(byte >> 4) as usize]);
                    result.push(HEX[(byte & 0xF) as usize]);
                }
            }
        }
        HyperlinkPath(result)
    }
}

/// Возвращает набор псевдонимов гиперссылок, поддерживаемых этим крейтом.
///
/// Псевдонимы поддерживаются реализацией трейта `FromStr` для
/// [`HyperlinkFormat`]. То есть, если псевдоним увиден, то он автоматически
/// заменяется соответствующим форматом. Например, псевдоним `vscode`
/// отображается в `vscode://file{path}:{line}:{column}`.
///
/// Это предоставлено, чтобы позволить вызывающим включать псевдонимы
/// гиперссылок в документацию способом, который гарантированно соответствует
/// тому, что фактически поддерживается.
///
/// Возвращаемый список гарантированно отсортирован лексикографически
/// по имени псевдонима. Вызывающие могут захотеть переотсортировать
/// список, используя [`HyperlinkAlias::display_priority`] через стабильную
/// сортировку при показе списка пользователям. Это заставит специальные
/// псевдонимы, такие как `none` и `default`, появиться первыми.
pub fn hyperlink_aliases() -> Vec<HyperlinkAlias> {
    HYPERLINK_PATTERN_ALIASES.iter().cloned().collect()
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    #[test]
    fn build_format() {
        let format = FormatBuilder::new()
            .append_slice(b"foo://")
            .append_slice(b"bar-")
            .append_slice(b"baz")
            .append_var("path")
            .unwrap()
            .build()
            .unwrap();

        assert_eq!(format.to_string(), "foo://bar-baz{path}");
        assert_eq!(format.parts[0], Part::Text(b"foo://bar-baz".to_vec()));
        assert!(!format.is_empty());
    }

    #[test]
    fn build_empty_format() {
        let format = FormatBuilder::new().build().unwrap();

        assert!(format.is_empty());
        assert_eq!(format, HyperlinkFormat::empty());
        assert_eq!(format, HyperlinkFormat::default());
    }

    #[test]
    fn handle_alias() {
        assert!(HyperlinkFormat::from_str("file").is_ok());
        assert!(HyperlinkFormat::from_str("none").is_ok());
        assert!(HyperlinkFormat::from_str("none").unwrap().is_empty());
    }

    #[test]
    fn parse_format() {
        let format = HyperlinkFormat::from_str(
            "foo://{host}/bar/{path}:{line}:{column}",
        )
        .unwrap();

        assert_eq!(
            format.to_string(),
            "foo://{host}/bar/{path}:{line}:{column}"
        );
        assert_eq!(format.parts.len(), 8);
        assert!(format.parts.contains(&Part::Path));
        assert!(format.parts.contains(&Part::Line));
        assert!(format.parts.contains(&Part::Column));
    }

    #[test]
    fn parse_valid() {
        assert!(HyperlinkFormat::from_str("").unwrap().is_empty());
        assert_eq!(
            HyperlinkFormat::from_str("foo://{path}").unwrap().to_string(),
            "foo://{path}"
        );
        assert_eq!(
            HyperlinkFormat::from_str("foo://{path}/bar").unwrap().to_string(),
            "foo://{path}/bar"
        );

        HyperlinkFormat::from_str("f://{path}").unwrap();
        HyperlinkFormat::from_str("f:{path}").unwrap();
        HyperlinkFormat::from_str("f-+.:{path}").unwrap();
        HyperlinkFormat::from_str("f42:{path}").unwrap();
        HyperlinkFormat::from_str("42:{path}").unwrap();
        HyperlinkFormat::from_str("+:{path}").unwrap();
        HyperlinkFormat::from_str("F42:{path}").unwrap();
        HyperlinkFormat::from_str("F42://foo{{bar}}{path}").unwrap();
    }

    #[test]
    fn parse_invalid() {
        use super::HyperlinkFormatErrorKind::*;

        let err = |kind| HyperlinkFormatError { kind };
        assert_eq!(
            HyperlinkFormat::from_str("foo://bar").unwrap_err(),
            err(NoVariables),
        );
        assert_eq!(
            HyperlinkFormat::from_str("foo://{line}").unwrap_err(),
            err(NoPathVariable),
        );
        assert_eq!(
            HyperlinkFormat::from_str("foo://{path").unwrap_err(),
            err(UnclosedVariable),
        );
        assert_eq!(
            HyperlinkFormat::from_str("foo://{path}:{column}").unwrap_err(),
            err(NoLineVariable),
        );
        assert_eq!(
            HyperlinkFormat::from_str("{path}").unwrap_err(),
            err(InvalidScheme),
        );
        assert_eq!(
            HyperlinkFormat::from_str(":{path}").unwrap_err(),
            err(InvalidScheme),
        );
        assert_eq!(
            HyperlinkFormat::from_str("f*:{path}").unwrap_err(),
            err(InvalidScheme),
        );

        assert_eq!(
            HyperlinkFormat::from_str("foo://{bar}").unwrap_err(),
            err(InvalidVariable("bar".to_string())),
        );
        assert_eq!(
            HyperlinkFormat::from_str("foo://{}}bar}").unwrap_err(),
            err(InvalidVariable("".to_string())),
        );
        assert_eq!(
            HyperlinkFormat::from_str("foo://{b}}ar}").unwrap_err(),
            err(InvalidVariable("b".to_string())),
        );
        assert_eq!(
            HyperlinkFormat::from_str("foo://{bar}}}").unwrap_err(),
            err(InvalidVariable("bar".to_string())),
        );
        assert_eq!(
            HyperlinkFormat::from_str("foo://{{bar}").unwrap_err(),
            err(InvalidCloseVariable),
        );
        assert_eq!(
            HyperlinkFormat::from_str("foo://{{{bar}").unwrap_err(),
            err(InvalidVariable("bar".to_string())),
        );
        assert_eq!(
            HyperlinkFormat::from_str("foo://{b{{ar}").unwrap_err(),
            err(InvalidVariable("b{{ar".to_string())),
        );
        assert_eq!(
            HyperlinkFormat::from_str("foo://{bar{{}").unwrap_err(),
            err(InvalidVariable("bar{{".to_string())),
        );
    }

    #[test]
    #[cfg(windows)]
    fn convert_to_hyperlink_path() {
        let convert = |path| {
            String::from_utf8(
                HyperlinkPath::from_path(Path::new(path)).unwrap().0,
            )
            .unwrap()
        };

        assert_eq!(convert(r"C:\dir\file.txt"), "/C:/dir/file.txt");
        assert_eq!(
            convert(r"C:\foo\bar\..\other\baz.txt"),
            "/C:/foo/other/baz.txt"
        );

        assert_eq!(convert(r"\\server\dir\file.txt"), "//server/dir/file.txt");
        assert_eq!(
            convert(r"\\server\dir\foo\..\other\file.txt"),
            "//server/dir/other/file.txt"
        );

        assert_eq!(convert(r"\\?\C:\dir\file.txt"), "/C:/dir/file.txt");
        assert_eq!(
            convert(r"\\?\UNC\server\dir\file.txt"),
            "//server/dir/file.txt"
        );
    }

    #[test]
    fn aliases_are_sorted() {
        let aliases = hyperlink_aliases();
        let mut prev =
            aliases.first().expect("aliases should be non-empty").name();
        for alias in aliases.iter().skip(1) {
            let name = alias.name();
            assert!(
                name > prev,
                "'{prev}' should come before '{name}' in \
                 HYPERLINK_PATTERN_ALIASES",
            );
            prev = name;
        }
    }

    #[test]
    fn alias_names_are_reasonable() {
        for alias in hyperlink_aliases() {
            // Здесь нет строгого правила, но если мы хотим определить псевдоним
            // с именем, которое не проходит этот assert, то мы должны
            // вероятно пометить его как достойный рассмотрения. Например, мы
            // действительно не хотим определять псевдоним, который содержит `{` или `}`,
            // что может спутать его с переменной.
            assert!(alias.name().chars().all(|c| c.is_alphanumeric()
                || c == '+'
                || c == '-'
                || c == '.'));
        }
    }

    #[test]
    fn aliases_are_valid_formats() {
        for alias in hyperlink_aliases() {
            let (name, format) = (alias.name(), alias.format());
            assert!(
                format.parse::<HyperlinkFormat>().is_ok(),
                "invalid hyperlink alias '{name}': {format}",
            );
        }
    }
}
