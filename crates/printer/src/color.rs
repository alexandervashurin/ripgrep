use termcolor::{Color, ColorSpec, ParseColorError};

/// Возвращает набор спецификаций цвета по умолчанию.
///
/// Это может со временем измениться, но выбор цвета предназначен для
/// достаточно консервативной работы across терминальных тем.
///
/// Дополнительные спецификации цвета могут быть добавлены в возвращаемый
/// список. Более недавно добавленные спецификации переопределяют ранее
/// добавленные спецификации.
pub fn default_color_specs() -> Vec<UserColorSpec> {
    vec![
        #[cfg(unix)]
        "path:fg:magenta".parse().unwrap(),
        #[cfg(windows)]
        "path:fg:cyan".parse().unwrap(),
        "line:fg:green".parse().unwrap(),
        "match:fg:red".parse().unwrap(),
        "match:style:bold".parse().unwrap(),
    ]
}

/// Ошибка, которая может возникнуть при разборе спецификаций цвета.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ColorError {
    /// Это происходит, когда используется нераспознанный тип вывода.
    UnrecognizedOutType(String),
    /// Это происходит, когда используется нераспознанный тип спецификации.
    UnrecognizedSpecType(String),
    /// Это происходит, когда используется нераспознанное имя цвета.
    UnrecognizedColor(String, String),
    /// Это происходит, когда используется нераспознанный атрибут стиля.
    UnrecognizedStyle(String),
    /// Это происходит, когда формат спецификации цвета недействителен.
    InvalidFormat(String),
}

impl std::error::Error for ColorError {}

impl ColorError {
    fn from_parse_error(err: ParseColorError) -> ColorError {
        ColorError::UnrecognizedColor(
            err.invalid().to_string(),
            err.to_string(),
        )
    }
}

impl std::fmt::Display for ColorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            ColorError::UnrecognizedOutType(ref name) => write!(
                f,
                "нераспознанный тип вывода '{}'. Выберите из: \
                 path, line, column, match, highlight.",
                name,
            ),
            ColorError::UnrecognizedSpecType(ref name) => write!(
                f,
                "нераспознанный тип спецификации '{}'. Выберите из: \
                 fg, bg, style, none.",
                name,
            ),
            ColorError::UnrecognizedColor(_, ref msg) => write!(f, "{}", msg),
            ColorError::UnrecognizedStyle(ref name) => write!(
                f,
                "нераспознанный атрибут стиля '{}'. Выберите из: \
                 nobold, bold, nointense, intense, nounderline, \
                 underline, noitalic, italic.",
                name,
            ),
            ColorError::InvalidFormat(ref original) => write!(
                f,
                "недействительный формат спецификации цвета: '{}'. \
                 Допустимый формат — \
                 '(path|line|column|match|highlight):(fg|bg|style):(value)'.",
                original,
            ),
        }
    }
}

/// Объединённый набор спецификаций цвета.
///
/// Этот набор спецификаций цвета представляет различные типы цветов,
/// которые поддерживаются принтерами в этом крейте. Набор спецификаций
/// цвета может быть создан из последовательности
/// [`UserColorSpec`](crate::UserColorSpec).
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ColorSpecs {
    path: ColorSpec,
    line: ColorSpec,
    column: ColorSpec,
    matched: ColorSpec,
    highlight: ColorSpec,
}

/// Одна спецификация цвета, предоставленная пользователем.
///
/// ## Формат
///
/// Формат `Spec` — это тройка: `{type}:{attribute}:{value}`. Каждый
/// компонент определяется следующим образом:
///
/// * `{type}` может быть одним из `path`, `line`, `column`, `match` или
///   `highlight`.
/// * `{attribute}` может быть одним из `fg`, `bg` или `style`.
///   `{attribute}` также может быть специальным значением `none`, в этом
///   случае `{value}` может быть опущено.
/// * `{value}` — это либо имя цвета (для `fg`/`bg`), либо инструкция стиля.
///
/// `{type}` управляет тем, какая часть вывода должна быть стилизована.
///
/// Когда `{attribute}` равен `none`, то это должно привести к очистке любых
/// существующих настроек стиля для указанного `type`.
///
/// `{value}` должно быть цветом, когда `{attribute}` равен `fg` или `bg`,
/// или это должно быть инструкцией стиля, когда `{attribute}` равен `style`.
/// Когда `{attribute}` равен `none`, `{value}` должно быть опущено.
///
/// Допустимые цвета: `black`, `blue`, `green`, `red`, `cyan`, `magenta`,
/// `yellow`, `white`. Расширенные цвета также могут быть указаны и
/// форматируются как `x` (для 256-битных цветов) или `x,x,x` (для
/// 24-битного true color), где `x` — число от 0 до 255 включительно.
/// `x` может быть дано как нормальное десятичное число или шестнадцатеричное
/// число, где последнее имеет префикс `0x`.
///
/// Допустимые инструкции стиля: `nobold`, `bold`, `intense`, `nointense`,
/// `underline`, `nounderline`, `italic`, `noitalic`.
///
/// ## Пример
///
/// Стандартный способ создания `UserColorSpec` — разобрать его из строки.
/// После создания нескольких `UserColorSpec` они могут быть предоставлены
/// стандартному принтеру, где они будут автоматически применены к выводу.
///
/// `UserColorSpec` также может быть преобразован в `termcolor::ColorSpec`:
///
/// ```rust
/// # fn main() {
/// use termcolor::{Color, ColorSpec};
/// use grep_printer::UserColorSpec;
///
/// let user_spec1: UserColorSpec = "path:fg:blue".parse().unwrap();
/// let user_spec2: UserColorSpec = "match:bg:0xff,0x7f,0x00".parse().unwrap();
///
/// let spec1 = user_spec1.to_color_spec();
/// let spec2 = user_spec2.to_color_spec();
///
/// assert_eq!(spec1.fg(), Some(&Color::Blue));
/// assert_eq!(spec2.bg(), Some(&Color::Rgb(0xFF, 0x7F, 0x00)));
/// # }
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserColorSpec {
    ty: OutType,
    value: SpecValue,
}

impl UserColorSpec {
    /// Преобразовать эту предоставленную пользователем спецификацию цвета
    /// в спецификацию, которая может быть использована с `termcolor`. Это
    /// отбрасывает тип этой спецификации (где тип указывает, где цвет
    /// применяется в стандартном принтере, например, к пути к файлу или
    /// номерам строк и т.д.).
    pub fn to_color_spec(&self) -> ColorSpec {
        let mut spec = ColorSpec::default();
        self.value.merge_into(&mut spec);
        spec
    }
}

/// Фактическое значение, данное спецификацией.
#[derive(Clone, Debug, Eq, PartialEq)]
enum SpecValue {
    None,
    Fg(Color),
    Bg(Color),
    Style(Style),
}

/// Набор настраиваемых частей вывода ripgrep.
#[derive(Clone, Debug, Eq, PartialEq)]
enum OutType {
    Path,
    Line,
    Column,
    Match,
    Highlight,
}

/// Тип спецификации.
#[derive(Clone, Debug, Eq, PartialEq)]
enum SpecType {
    Fg,
    Bg,
    Style,
    None,
}

/// Набор доступных стилей для использования в терминале.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Style {
    Bold,
    NoBold,
    Intense,
    NoIntense,
    Underline,
    NoUnderline,
    Italic,
    NoItalic,
}

impl ColorSpecs {
    /// Создать спецификации цвета из списка предоставленных пользователем
    /// спецификаций.
    pub fn new(specs: &[UserColorSpec]) -> ColorSpecs {
        let mut merged = ColorSpecs::default();
        for spec in specs {
            match spec.ty {
                OutType::Path => spec.merge_into(&mut merged.path),
                OutType::Line => spec.merge_into(&mut merged.line),
                OutType::Column => spec.merge_into(&mut merged.column),
                OutType::Match => spec.merge_into(&mut merged.matched),
                OutType::Highlight => spec.merge_into(&mut merged.highlight),
            }
        }
        merged
    }

    /// Создать набор спецификаций по умолчанию с цветом.
    ///
    /// Это отличается от реализации `Default` для `ColorSpecs` тем, что
    /// это предоставляет набор вариантов цвета по умолчанию, тогда как
    /// реализация `Default` не предоставляет вариантов цвета.
    pub fn default_with_color() -> ColorSpecs {
        ColorSpecs::new(&default_color_specs())
    }

    /// Вернуть спецификацию цвета для раскраски путей к файлам.
    pub fn path(&self) -> &ColorSpec {
        &self.path
    }

    /// Вернуть спецификацию цвета для раскраски номеров строк.
    pub fn line(&self) -> &ColorSpec {
        &self.line
    }

    /// Вернуть спецификацию цвета для раскраски номеров столбцов.
    pub fn column(&self) -> &ColorSpec {
        &self.column
    }

    /// Вернуть спецификацию цвета для раскраски совпавшего текста.
    pub fn matched(&self) -> &ColorSpec {
        &self.matched
    }

    /// Вернуть спецификацию цвета для раскраски всей строки, если есть
    /// совпавший текст.
    pub fn highlight(&self) -> &ColorSpec {
        &self.highlight
    }
}

impl UserColorSpec {
    /// Объединить эту спецификацию в данную спецификацию цвета.
    fn merge_into(&self, cspec: &mut ColorSpec) {
        self.value.merge_into(cspec);
    }
}

impl SpecValue {
    /// Объединить это значение спецификации в данную спецификацию цвета.
    fn merge_into(&self, cspec: &mut ColorSpec) {
        match *self {
            SpecValue::None => cspec.clear(),
            SpecValue::Fg(ref color) => {
                cspec.set_fg(Some(color.clone()));
            }
            SpecValue::Bg(ref color) => {
                cspec.set_bg(Some(color.clone()));
            }
            SpecValue::Style(ref style) => match *style {
                Style::Bold => {
                    cspec.set_bold(true);
                }
                Style::NoBold => {
                    cspec.set_bold(false);
                }
                Style::Intense => {
                    cspec.set_intense(true);
                }
                Style::NoIntense => {
                    cspec.set_intense(false);
                }
                Style::Underline => {
                    cspec.set_underline(true);
                }
                Style::NoUnderline => {
                    cspec.set_underline(false);
                }
                Style::Italic => {
                    cspec.set_italic(true);
                }
                Style::NoItalic => {
                    cspec.set_italic(false);
                }
            },
        }
    }
}

impl std::str::FromStr for UserColorSpec {
    type Err = ColorError;

    fn from_str(s: &str) -> Result<UserColorSpec, ColorError> {
        let pieces: Vec<&str> = s.split(':').collect();
        if pieces.len() <= 1 || pieces.len() > 3 {
            return Err(ColorError::InvalidFormat(s.to_string()));
        }
        let otype: OutType = pieces[0].parse()?;
        match pieces[1].parse()? {
            SpecType::None => {
                Ok(UserColorSpec { ty: otype, value: SpecValue::None })
            }
            SpecType::Style => {
                if pieces.len() < 3 {
                    return Err(ColorError::InvalidFormat(s.to_string()));
                }
                let style: Style = pieces[2].parse()?;
                Ok(UserColorSpec { ty: otype, value: SpecValue::Style(style) })
            }
            SpecType::Fg => {
                if pieces.len() < 3 {
                    return Err(ColorError::InvalidFormat(s.to_string()));
                }
                let color: Color =
                    pieces[2].parse().map_err(ColorError::from_parse_error)?;
                Ok(UserColorSpec { ty: otype, value: SpecValue::Fg(color) })
            }
            SpecType::Bg => {
                if pieces.len() < 3 {
                    return Err(ColorError::InvalidFormat(s.to_string()));
                }
                let color: Color =
                    pieces[2].parse().map_err(ColorError::from_parse_error)?;
                Ok(UserColorSpec { ty: otype, value: SpecValue::Bg(color) })
            }
        }
    }
}

impl std::str::FromStr for OutType {
    type Err = ColorError;

    fn from_str(s: &str) -> Result<OutType, ColorError> {
        match &*s.to_lowercase() {
            "path" => Ok(OutType::Path),
            "line" => Ok(OutType::Line),
            "column" => Ok(OutType::Column),
            "match" => Ok(OutType::Match),
            "highlight" => Ok(OutType::Highlight),
            _ => Err(ColorError::UnrecognizedOutType(s.to_string())),
        }
    }
}

impl std::str::FromStr for SpecType {
    type Err = ColorError;

    fn from_str(s: &str) -> Result<SpecType, ColorError> {
        match &*s.to_lowercase() {
            "fg" => Ok(SpecType::Fg),
            "bg" => Ok(SpecType::Bg),
            "style" => Ok(SpecType::Style),
            "none" => Ok(SpecType::None),
            _ => Err(ColorError::UnrecognizedSpecType(s.to_string())),
        }
    }
}

impl std::str::FromStr for Style {
    type Err = ColorError;

    fn from_str(s: &str) -> Result<Style, ColorError> {
        match &*s.to_lowercase() {
            "bold" => Ok(Style::Bold),
            "nobold" => Ok(Style::NoBold),
            "intense" => Ok(Style::Intense),
            "nointense" => Ok(Style::NoIntense),
            "underline" => Ok(Style::Underline),
            "nounderline" => Ok(Style::NoUnderline),
            "italic" => Ok(Style::Italic),
            "noitalic" => Ok(Style::NoItalic),
            _ => Err(ColorError::UnrecognizedStyle(s.to_string())),
        }
    }
}
