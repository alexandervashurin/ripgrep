use std::fmt::Write;
use std::path::{Path, is_separator};

use regex_automata::meta::Regex;

use crate::{Candidate, Error, ErrorKind, new_regex};

/// Описывает стратегию сопоставления для конкретного шаблона.
///
/// Это предоставляет способ более быстрого определения того, соответствует ли
/// шаблон конкретному пути к файлу таким образом, который масштабируется
/// с большим количеством шаблонов. Например, если многие шаблоны имеют вид
/// `*.ext`, то можно проверить, соответствует ли какой-либо из этих шаблонов,
/// выполнив поиск расширения пути к файлу в хеш-таблице.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MatchStrategy {
    /// Шаблон соответствует тогда и только тогда, когда весь путь к файлу
    /// соответствует этой буквенной строке.
    Literal(String),
    /// Шаблон соответствует тогда и только тогда, когда базовое имя пути
    /// к файлу соответствует этой буквенной строке.
    BasenameLiteral(String),
    /// Шаблон соответствует тогда и только тогда, когда расширение пути
    /// к файлу соответствует этой буквенной строке.
    Extension(String),
    /// Шаблон соответствует тогда и только тогда, когда этот префикс является
    /// префиксом пути кандидата.
    Prefix(String),
    /// Шаблон соответствует тогда и только тогда, когда этот префикс является
    /// префиксом пути кандидата.
    ///
    /// Исключение: если `component` истинно, то `suffix` должно появляться
    /// в начале пути к файлу или сразу после `/`.
    Suffix {
        /// Фактический суффикс.
        suffix: String,
        /// Должно ли это начинаться в начале компонента пути.
        component: bool,
    },
    /// Шаблон соответствует только если данное расширение соответствует
    /// расширению пути к файлу. Обратите внимание, что это необходимый,
    /// но НЕ достаточный критерий. А именно, если расширение соответствует,
    /// то всё равно требуется полный поиск по регулярному выражению.
    RequiredExtension(String),
    /// Для сопоставления требуется регулярное выражение.
    Regex,
}

impl MatchStrategy {
    /// Возвращает стратегию сопоставления для данного шаблона.
    pub(crate) fn new(pat: &Glob) -> MatchStrategy {
        if let Some(lit) = pat.basename_literal() {
            MatchStrategy::BasenameLiteral(lit)
        } else if let Some(lit) = pat.literal() {
            MatchStrategy::Literal(lit)
        } else if let Some(ext) = pat.ext() {
            MatchStrategy::Extension(ext)
        } else if let Some(prefix) = pat.prefix() {
            MatchStrategy::Prefix(prefix)
        } else if let Some((suffix, component)) = pat.suffix() {
            MatchStrategy::Suffix { suffix, component }
        } else if let Some(ext) = pat.required_ext() {
            MatchStrategy::RequiredExtension(ext)
        } else {
            MatchStrategy::Regex
        }
    }
}

/// Glob представляет собой успешно разобранный шаблон glob для оболочки.
///
/// Он не может быть использован напрямую для сопоставления путей к файлам,
/// но может быть преобразован в строку регулярного выражения или в matcher.
#[derive(Clone, Eq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct Glob {
    glob: String,
    re: String,
    opts: GlobOptions,
    tokens: Tokens,
}

impl AsRef<Glob> for Glob {
    fn as_ref(&self) -> &Glob {
        self
    }
}

impl PartialEq for Glob {
    fn eq(&self, other: &Glob) -> bool {
        self.glob == other.glob && self.opts == other.opts
    }
}

impl std::hash::Hash for Glob {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.glob.hash(state);
        self.opts.hash(state);
    }
}

impl std::fmt::Debug for Glob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if f.alternate() {
            f.debug_struct("Glob")
                .field("glob", &self.glob)
                .field("re", &self.re)
                .field("opts", &self.opts)
                .field("tokens", &self.tokens)
                .finish()
        } else {
            f.debug_tuple("Glob").field(&self.glob).finish()
        }
    }
}

impl std::fmt::Display for Glob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.glob.fmt(f)
    }
}

impl std::str::FromStr for Glob {
    type Err = Error;

    fn from_str(glob: &str) -> Result<Self, Self::Err> {
        Self::new(glob)
    }
}

/// Matcher для одного шаблона.
#[derive(Clone, Debug)]
pub struct GlobMatcher {
    /// Базовый шаблон.
    pat: Glob,
    /// Шаблон в виде скомпилированного регулярного выражения.
    re: Regex,
}

impl GlobMatcher {
    /// Проверяет, соответствует ли данный путь этому шаблону или нет.
    pub fn is_match<P: AsRef<Path>>(&self, path: P) -> bool {
        self.is_match_candidate(&Candidate::new(path.as_ref()))
    }

    /// Проверяет, соответствует ли данный путь этому шаблону или нет.
    pub fn is_match_candidate(&self, path: &Candidate<'_>) -> bool {
        self.re.is_match(&path.path)
    }

    /// Возвращает `Glob`, использованный для компиляции этого matcher.
    pub fn glob(&self) -> &Glob {
        &self.pat
    }
}

/// Стратегический matcher для одного шаблона.
#[cfg(test)]
#[derive(Clone, Debug)]
struct GlobStrategic {
    /// Стратегия сопоставления для использования.
    strategy: MatchStrategy,
    /// Шаблон в виде скомпилированного регулярного выражения.
    re: Regex,
}

#[cfg(test)]
impl GlobStrategic {
    /// Проверяет, соответствует ли данный путь этому шаблону или нет.
    fn is_match<P: AsRef<Path>>(&self, path: P) -> bool {
        self.is_match_candidate(&Candidate::new(path.as_ref()))
    }

    /// Проверяет, соответствует ли данный путь этому шаблону или нет.
    fn is_match_candidate(&self, candidate: &Candidate<'_>) -> bool {
        let byte_path = &*candidate.path;

        match self.strategy {
            MatchStrategy::Literal(ref lit) => lit.as_bytes() == byte_path,
            MatchStrategy::BasenameLiteral(ref lit) => {
                lit.as_bytes() == &*candidate.basename
            }
            MatchStrategy::Extension(ref ext) => {
                ext.as_bytes() == &*candidate.ext
            }
            MatchStrategy::Prefix(ref pre) => {
                starts_with(pre.as_bytes(), byte_path)
            }
            MatchStrategy::Suffix { ref suffix, component } => {
                if component && byte_path == &suffix.as_bytes()[1..] {
                    return true;
                }
                ends_with(suffix.as_bytes(), byte_path)
            }
            MatchStrategy::RequiredExtension(ref ext) => {
                let ext = ext.as_bytes();
                &*candidate.ext == ext && self.re.is_match(byte_path)
            }
            MatchStrategy::Regex => self.re.is_match(byte_path),
        }
    }
}

/// Построитель для шаблона.
///
/// Этот построитель позволяет настраивать семантику сопоставления шаблона.
/// Например, можно сделать сопоставление регистронезависимым.
///
/// Время жизни `'a` относится к времени жизни строки шаблона.
#[derive(Clone, Debug)]
pub struct GlobBuilder<'a> {
    /// Шаблон glob для компиляции.
    glob: &'a str,
    /// Параметры для шаблона.
    opts: GlobOptions,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
struct GlobOptions {
    /// Сопоставлять ли регистронезависимо.
    case_insensitive: bool,
    /// Требовать ли буквенный разделитель для сопоставления разделителя в пути
    /// к файлу. Например, когда включено, `*` не будет сопоставляться с `/`.
    literal_separator: bool,
    /// Использовать ли `\` для экранирования специальных символов.
    /// Например, когда включено, `\*` будет сопоставляться с буквальным `*`.
    backslash_escape: bool,
    /// Следует ли удалять пустой случай в альтернативе.
    /// Например, когда включено, `{,a}` будет сопоставляться с "" и "a".
    empty_alternates: bool,
    /// Разрешён ли незакрытый класс символов. Когда найден незакрытый класс
    /// символов, открывающий `[` трактуется как буквальный `[`.
    /// Когда это не включено, открывающий `[` без соответствующего `]`
    /// трактуется как ошибка.
    allow_unclosed_class: bool,
}

impl GlobOptions {
    fn default() -> GlobOptions {
        GlobOptions {
            case_insensitive: false,
            literal_separator: false,
            backslash_escape: !is_separator('\\'),
            empty_alternates: false,
            allow_unclosed_class: false,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
struct Tokens(Vec<Token>);

impl std::ops::Deref for Tokens {
    type Target = Vec<Token>;
    fn deref(&self) -> &Vec<Token> {
        &self.0
    }
}

impl std::ops::DerefMut for Tokens {
    fn deref_mut(&mut self) -> &mut Vec<Token> {
        &mut self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
enum Token {
    Literal(char),
    Any,
    ZeroOrMore,
    RecursivePrefix,
    RecursiveSuffix,
    RecursiveZeroOrMore,
    Class { negated: bool, ranges: Vec<(char, char)> },
    Alternates(Vec<Tokens>),
}

impl Glob {
    /// Строит новый шаблон с параметрами по умолчанию.
    pub fn new(glob: &str) -> Result<Glob, Error> {
        GlobBuilder::new(glob).build()
    }

    /// Возвращает matcher для этого шаблона.
    pub fn compile_matcher(&self) -> GlobMatcher {
        let re =
            new_regex(&self.re).expect("regex compilation shouldn't fail");
        GlobMatcher { pat: self.clone(), re }
    }

    /// Возвращает стратегический matcher.
    ///
    /// Это не экспонируется, потому что неясно, действительно ли это
    /// быстрее, чем просто запуск регулярного выражения для *одного*
    /// шаблона. Если это быстрее, то GlobMatcher должен делать это
    /// автоматически.
    #[cfg(test)]
    fn compile_strategic_matcher(&self) -> GlobStrategic {
        let strategy = MatchStrategy::new(self);
        let re =
            new_regex(&self.re).expect("regex compilation shouldn't fail");
        GlobStrategic { strategy, re }
    }

    /// Возвращает исходный шаблон glob, использованный для построения этого шаблона.
    pub fn glob(&self) -> &str {
        &self.glob
    }

    /// Возвращает строку регулярного выражения для этого glob.
    ///
    /// Обратите внимание, что регулярные выражения для glob предназначены
    /// для сопоставления с произвольными байтами (`&[u8]`), а не со
    /// строками Unicode (`&str`). В частности, glob часто используются
    /// для путей к файлам, где нет общей гарантии, что пути к файлам
    /// сами по себе являются допустимым UTF-8. В результате вызывающим
    /// сторонам потребуется убедиться, что они используют API регулярных
    /// выражений, которое может сопоставлять произвольные байты. Например,
    /// API
    /// [`Regex`](https://docs.rs/regex/*/regex/struct.Regex.html)
    /// из крейта
    /// [`regex`](https://crates.io/regex)
    /// не подходит для этого, поскольку он сопоставляет `&str`, но его
    /// API
    /// [`bytes::Regex`](https://docs.rs/regex/*/regex/bytes/struct.Regex.html)
    /// подходит для этого.
    pub fn regex(&self) -> &str {
        &self.re
    }

    /// Возвращает шаблон как буквальную строку тогда и только тогда, когда
    /// шаблон должен соответствовать всему пути точно.
    ///
    /// Базовый формат этих шаблонов: `{literal}`.
    fn literal(&self) -> Option<String> {
        if self.opts.case_insensitive {
            return None;
        }
        let mut lit = String::new();
        for t in &*self.tokens {
            let Token::Literal(c) = *t else { return None };
            lit.push(c);
        }
        if lit.is_empty() { None } else { Some(lit) }
    }

    /// Возвращает расширение, если этот шаблон соответствует пути к файлу
    /// тогда и только тогда, когда путь к файлу имеет возвращаемое расширение.
    ///
    /// Обратите внимание, что возвращаемое расширение отличается от расширения,
    /// которое возвращает std::path::Path::extension. А именно, это расширение
    /// включает точку '.'. Также пути вида `.rs` считаются имеющими расширение
    /// `.rs`.
    fn ext(&self) -> Option<String> {
        if self.opts.case_insensitive {
            return None;
        }
        let start = match *self.tokens.get(0)? {
            Token::RecursivePrefix => 1,
            _ => 0,
        };
        match *self.tokens.get(start)? {
            Token::ZeroOrMore => {
                // Если не было рекурсивного префикса, то мы разрешаем
                // `*` только если `*` может соответствовать `/`. Например, если `*` не может
                // соответствовать `/`, то `*.c` не соответствует `foo/bar.c`.
                if start == 0 && self.opts.literal_separator {
                    return None;
                }
            }
            _ => return None,
        }
        match *self.tokens.get(start + 1)? {
            Token::Literal('.') => {}
            _ => return None,
        }
        let mut lit = ".".to_string();
        for t in self.tokens[start + 2..].iter() {
            match *t {
                Token::Literal('.') | Token::Literal('/') => return None,
                Token::Literal(c) => lit.push(c),
                _ => return None,
            }
        }
        if lit.is_empty() { None } else { Some(lit) }
    }

    /// Это похоже на `ext`, но возвращает расширение, даже если его
    /// недостаточно для подтверждения совпадения. А именно, если расширение
    /// возвращено, то оно необходимо, но недостаточно для совпадения.
    fn required_ext(&self) -> Option<String> {
        if self.opts.case_insensitive {
            return None;
        }
        // Нам всё равно на начало этого шаблона. Всё, что нам
        // нужно проверить, это заканчивается ли он буквальным видом `.ext`.
        let mut ext: Vec<char> = vec![]; // построено в обратном порядке
        for t in self.tokens.iter().rev() {
            match *t {
                Token::Literal('/') => return None,
                Token::Literal(c) => {
                    ext.push(c);
                    if c == '.' {
                        break;
                    }
                }
                _ => return None,
            }
        }
        if ext.last() != Some(&'.') {
            None
        } else {
            ext.reverse();
            Some(ext.into_iter().collect())
        }
    }

    /// Возвращает буквенный префикс этого шаблона, если весь шаблон
    /// соответствует, если буквенный префикс соответствует.
    fn prefix(&self) -> Option<String> {
        if self.opts.case_insensitive {
            return None;
        }
        let (end, need_sep) = match *self.tokens.last()? {
            Token::ZeroOrMore => {
                if self.opts.literal_separator {
                    // Если завершающий `*` не может соответствовать `/`, то мы не можем
                    // предполагать, что совпадение префикса соответствует совпадению
                    // общего шаблона. Например, `foo/*` с
                    // включённым `literal_separator` соответствует `foo/bar`, но не
                    // `foo/bar/baz`, хотя `foo/bar/baz` имеет буквенный префикс `foo/`.
                    return None;
                }
                (self.tokens.len() - 1, false)
            }
            Token::RecursiveSuffix => (self.tokens.len() - 1, true),
            _ => (self.tokens.len(), false),
        };
        let mut lit = String::new();
        for t in &self.tokens[0..end] {
            let Token::Literal(c) = *t else { return None };
            lit.push(c);
        }
        if need_sep {
            lit.push('/');
        }
        if lit.is_empty() { None } else { Some(lit) }
    }

    /// Возвращает буквенный суффикс этого шаблона, если весь шаблон
    /// соответствует, если буквенный суффикс соответствует.
    ///
    /// Если возвращён буквенный суффикс и он должен соответствовать либо
    /// всему пути к файлу, либо ему должен предшествовать `/`, то также
    /// возвращается true. Это происходит с шаблоном вида `**/foo/bar`.
    /// А именно, этот шаблон соответствует `foo/bar` и `baz/foo/bar`,
    /// но не соответствует `foofoo/bar`. В этом случае возвращаемый
    /// суффикс — `/foo/bar` (но должен соответствовать всему пути `foo/bar`).
    ///
    /// Когда это возвращает true, буквенный суффикс гарантированно начинается с `/`.
    fn suffix(&self) -> Option<(String, bool)> {
        if self.opts.case_insensitive {
            return None;
        }
        let mut lit = String::new();
        let (start, entire) = match *self.tokens.get(0)? {
            Token::RecursivePrefix => {
                // Нам важно, следует ли это за компонентом пути, только если следующий
                // токен является буквальным.
                if let Some(&Token::Literal(_)) = self.tokens.get(1) {
                    lit.push('/');
                    (1, true)
                } else {
                    (1, false)
                }
            }
            _ => (0, false),
        };
        let start = match *self.tokens.get(start)? {
            Token::ZeroOrMore => {
                // Если literal_separator включён, то `*` не может
                // обязательно соответствовать всему, поэтому сообщение о совпадении суффикса
                // как о совпадении шаблона было бы ложноположительным.
                if self.opts.literal_separator {
                    return None;
                }
                start + 1
            }
            _ => start,
        };
        for t in &self.tokens[start..] {
            let Token::Literal(c) = *t else { return None };
            lit.push(c);
        }
        if lit.is_empty() || lit == "/" { None } else { Some((lit, entire)) }
    }

    /// Если этому шаблону нужно проверять только базовое имя пути к файлу,
    /// то возвращаются токены, соответствующие только совпадению базового имени.
    ///
    /// Например, для шаблона `**/*.foo` возвращаются только токены,
    /// соответствующие `*.foo`.
    ///
    /// Обратите внимание, что это вернёт None, если любое совпадение токенов
    /// базового имени не соответствует совпадению всего шаблона. Например,
    /// glob `foo` соответствует только когда путь к файлу имеет базовое имя
    /// `foo`, но не *всегда* соответствует, когда путь к файлу имеет базовое
    /// имя `foo`. Например, `foo` не соответствует `abc/foo`.
    fn basename_tokens(&self) -> Option<&[Token]> {
        if self.opts.case_insensitive {
            return None;
        }
        let start = match *self.tokens.get(0)? {
            Token::RecursivePrefix => 1,
            _ => {
                // Без ничего, чтобы поглотить родительскую часть пути,
                // мы не можем предположить, что сопоставление только по базовому имени
                // корректно.
                return None;
            }
        };
        if self.tokens[start..].is_empty() {
            return None;
        }
        for t in self.tokens[start..].iter() {
            match *t {
                Token::Literal('/') => return None,
                Token::Literal(_) => {} // OK
                Token::Any | Token::ZeroOrMore => {
                    if !self.opts.literal_separator {
                        // В этом случае `*` и `?` могут соответствовать разделителю
                        // пути, что означает, что это может выйти за пределы
                        // базового имени.
                        return None;
                    }
                }
                Token::RecursivePrefix
                | Token::RecursiveSuffix
                | Token::RecursiveZeroOrMore => {
                    return None;
                }
                Token::Class { .. } | Token::Alternates(..) => {
                    // Мы *могли* быть немного умнее здесь, но любой из
                    // них всё равно предотвратит наши буквенные оптимизации,
                    // так что сдаёмся.
                    return None;
                }
            }
        }
        Some(&self.tokens[start..])
    }

    /// Возвращает шаблон как буквальную строку тогда и только тогда, когда
    /// шаблон исключительно соответствует базовому имени пути к файлу
    /// *и* является буквальным.
    ///
    /// Базовый формат этих шаблонов: `**/{literal}`, где `{literal}`
    /// не содержит разделителя пути.
    fn basename_literal(&self) -> Option<String> {
        let tokens = self.basename_tokens()?;
        let mut lit = String::new();
        for t in tokens {
            let Token::Literal(c) = *t else { return None };
            lit.push(c);
        }
        Some(lit)
    }
}

impl<'a> GlobBuilder<'a> {
    /// Создаёт новый построитель для данного шаблона.
    ///
    /// Шаблон не компилируется, пока не будет вызван `build`.
    pub fn new(glob: &'a str) -> GlobBuilder<'a> {
        GlobBuilder { glob, opts: GlobOptions::default() }
    }

    /// Разбирает и строит шаблон.
    pub fn build(&self) -> Result<Glob, Error> {
        let mut p = Parser {
            glob: &self.glob,
            alternates_stack: Vec::new(),
            branches: vec![Tokens::default()],
            chars: self.glob.chars().peekable(),
            prev: None,
            cur: None,
            found_unclosed_class: false,
            opts: &self.opts,
        };
        p.parse()?;
        if p.branches.is_empty() {
            // OK из-за того, как управляются branches/alternate_stack.
            // Если мы оказались здесь, то *должна* быть ошибка в парсере
            // где-то.
            unreachable!()
        } else if p.branches.len() > 1 {
            Err(Error {
                glob: Some(self.glob.to_string()),
                kind: ErrorKind::UnclosedAlternates,
            })
        } else {
            let tokens = p.branches.pop().unwrap();
            Ok(Glob {
                glob: self.glob.to_string(),
                re: tokens.to_regex_with(&self.opts),
                opts: self.opts,
                tokens,
            })
        }
    }

    /// Переключает, соответствует ли шаблон регистронезависимо или нет.
    ///
    /// По умолчанию это отключено.
    pub fn case_insensitive(&mut self, yes: bool) -> &mut GlobBuilder<'a> {
        self.opts.case_insensitive = yes;
        self
    }

    /// Переключает, требуется ли буквенный `/` для сопоставления
    /// разделителя пути.
    ///
    /// По умолчанию это false: `*` и `?` будут сопоставляться с `/`.
    pub fn literal_separator(&mut self, yes: bool) -> &mut GlobBuilder<'a> {
        self.opts.literal_separator = yes;
        self
    }

    /// Когда включено, обратная косая черта (`\`) может использоваться для
    /// экранирования специальных символов в шаблоне glob. Дополнительно это
    /// предотвратит интерпретацию `\` как разделителя пути на всех платформах.
    ///
    /// Это включено по умолчанию на платформах, где `\` не является
    /// разделителем пути, и отключено по умолчанию на платформах, где
    /// `\` является разделителем пути.
    pub fn backslash_escape(&mut self, yes: bool) -> &mut GlobBuilder<'a> {
        self.opts.backslash_escape = yes;
        self
    }

    /// Переключает, принимается ли пустой шаблон в списке альтернатив.
    ///
    /// Например, если это установлено, то glob `foo{,.txt}` будет
    /// соответствовать как `foo`, так и `foo.txt`.
    ///
    /// По умолчанию это false.
    pub fn empty_alternates(&mut self, yes: bool) -> &mut GlobBuilder<'a> {
        self.opts.empty_alternates = yes;
        self
    }

    /// Переключает, разрешены ли незакрытые классы символов. Когда разрешено,
    /// `[` без соответствующего `]` трактуется буквально вместо того,
    /// чтобы приводить к ошибке разбора.
    ///
    /// Например, если это установлено, то glob `[abc` будет трактоваться
    /// как буквальная строка `[abc` вместо возврата ошибки.
    ///
    /// По умолчанию это false. Вообще говоря, включение этого приводит к
    /// худшим режимам отказа, поскольку парсер glob становится более
    /// разрешительным. Вы можете захотеть включить это, когда совместимость
    /// (например, с реализациями POSIX glob) важнее хороших сообщений об ошибках.
    pub fn allow_unclosed_class(&mut self, yes: bool) -> &mut GlobBuilder<'a> {
        self.opts.allow_unclosed_class = yes;
        self
    }
}

impl Tokens {
    /// Преобразует этот шаблон в строку, которая гарантированно будет
    /// допустимым регулярным выражением и будет представлять семантику
    /// сопоставления этого шаблона glob и данных параметров.
    fn to_regex_with(&self, options: &GlobOptions) -> String {
        let mut re = String::new();
        re.push_str("(?-u)");
        if options.case_insensitive {
            re.push_str("(?i)");
        }
        re.push('^');
        // Особый случай. Если весь glob — это просто `**`, то он должен соответствовать
        // всему.
        if self.len() == 1 && self[0] == Token::RecursivePrefix {
            re.push_str(".*");
            re.push('$');
            return re;
        }
        self.tokens_to_regex(options, &self, &mut re);
        re.push('$');
        re
    }

    fn tokens_to_regex(
        &self,
        options: &GlobOptions,
        tokens: &[Token],
        re: &mut String,
    ) {
        for tok in tokens.iter() {
            match *tok {
                Token::Literal(c) => {
                    re.push_str(&char_to_escaped_literal(c));
                }
                Token::Any => {
                    if options.literal_separator {
                        re.push_str("[^/]");
                    } else {
                        re.push_str(".");
                    }
                }
                Token::ZeroOrMore => {
                    if options.literal_separator {
                        re.push_str("[^/]*");
                    } else {
                        re.push_str(".*");
                    }
                }
                Token::RecursivePrefix => {
                    re.push_str("(?:/?|.*/)");
                }
                Token::RecursiveSuffix => {
                    re.push_str("/.*");
                }
                Token::RecursiveZeroOrMore => {
                    re.push_str("(?:/|/.*/)");
                }
                Token::Class { negated, ref ranges } => {
                    re.push('[');
                    if negated {
                        re.push('^');
                    }
                    for r in ranges {
                        if r.0 == r.1 {
                            // Не строго необходимо, но приятнее для просмотра.
                            re.push_str(&char_to_escaped_literal(r.0));
                        } else {
                            re.push_str(&char_to_escaped_literal(r.0));
                            re.push('-');
                            re.push_str(&char_to_escaped_literal(r.1));
                        }
                    }
                    re.push(']');
                }
                Token::Alternates(ref patterns) => {
                    let mut parts = vec![];
                    for pat in patterns {
                        let mut altre = String::new();
                        self.tokens_to_regex(options, &pat, &mut altre);
                        if !altre.is_empty() || options.empty_alternates {
                            parts.push(altre);
                        }
                    }

                    // Возможно иметь пустое множество, в этом случае
                    // результирующая альтернация '()' была бы ошибкой.
                    if !parts.is_empty() {
                        re.push_str("(?:");
                        re.push_str(&parts.join("|"));
                        re.push(')');
                    }
                }
            }
        }
    }
}

/// Преобразует скалярное значение Unicode в экранированную строку, подходящую для использования
/// в качестве литерала в регулярном выражении, не поддерживающем Unicode.
fn char_to_escaped_literal(c: char) -> String {
    let mut buf = [0; 4];
    let bytes = c.encode_utf8(&mut buf).as_bytes();
    bytes_to_escaped_literal(bytes)
}

/// Преобразует произвольную последовательность байтов в строку UTF-8. Все не-ASCII
/// кодовые единицы преобразуются в их экранированную форму.
fn bytes_to_escaped_literal(bs: &[u8]) -> String {
    let mut s = String::with_capacity(bs.len());
    for &b in bs {
        if b <= 0x7F {
            regex_syntax::escape_into(
                char::from(b).encode_utf8(&mut [0; 4]),
                &mut s,
            );
        } else {
            write!(&mut s, "\\x{:02x}", b).unwrap();
        }
    }
    s
}

struct Parser<'a> {
    /// Glob для разбора.
    glob: &'a str,
    /// Отмечает индекс в `stack`, где началась альтернация.
    alternates_stack: Vec<usize>,
    /// Набор активных ветвей альтернации, находящихся в процессе разбора.
    /// Токены добавляются в конец последней.
    branches: Vec<Tokens>,
    /// Итератор символов по шаблону glob для разбора.
    chars: std::iter::Peekable<std::str::Chars<'a>>,
    /// Предыдущий увиденный символ.
    prev: Option<char>,
    /// Текущий символ.
    cur: Option<char>,
    /// Не удалось ли найти закрывающий `]` для класса
    /// символов. Это может быть true только когда `GlobOptions::allow_unclosed_class`
    /// включён. Когда включено, невозможно когда-либо разобрать другой
    /// класс символов с этим glob. Это потому, что классы не могут быть
    /// вложенными *и* единственный способ, когда это происходит, — это когда никогда не бывает `]`.
    ///
    /// Мы отслеживаем это состояние, чтобы не тратить квадратичное время
    /// на попытку разбора чего-то вроде `[[[[[[[[[[[[[[[[[[[[[[[...`.
    found_unclosed_class: bool,
    /// Параметры Glob, которые могут влиять на разбор.
    opts: &'a GlobOptions,
}

impl<'a> Parser<'a> {
    fn error(&self, kind: ErrorKind) -> Error {
        Error { glob: Some(self.glob.to_string()), kind }
    }

    fn parse(&mut self) -> Result<(), Error> {
        while let Some(c) = self.bump() {
            match c {
                '?' => self.push_token(Token::Any)?,
                '*' => self.parse_star()?,
                '[' if !self.found_unclosed_class => self.parse_class()?,
                '{' => self.push_alternate()?,
                '}' => self.pop_alternate()?,
                ',' => self.parse_comma()?,
                '\\' => self.parse_backslash()?,
                c => self.push_token(Token::Literal(c))?,
            }
        }
        Ok(())
    }

    fn push_alternate(&mut self) -> Result<(), Error> {
        self.alternates_stack.push(self.branches.len());
        self.branches.push(Tokens::default());
        Ok(())
    }

    fn pop_alternate(&mut self) -> Result<(), Error> {
        let Some(start) = self.alternates_stack.pop() else {
            return Err(self.error(ErrorKind::UnopenedAlternates));
        };
        assert!(start <= self.branches.len());
        let alts = Token::Alternates(self.branches.drain(start..).collect());
        self.push_token(alts)?;
        Ok(())
    }

    fn push_token(&mut self, tok: Token) -> Result<(), Error> {
        if let Some(ref mut pat) = self.branches.last_mut() {
            return Ok(pat.push(tok));
        }
        Err(self.error(ErrorKind::UnopenedAlternates))
    }

    fn pop_token(&mut self) -> Result<Token, Error> {
        if let Some(ref mut pat) = self.branches.last_mut() {
            return Ok(pat.pop().unwrap());
        }
        Err(self.error(ErrorKind::UnopenedAlternates))
    }

    fn have_tokens(&self) -> Result<bool, Error> {
        match self.branches.last() {
            None => Err(self.error(ErrorKind::UnopenedAlternates)),
            Some(ref pat) => Ok(!pat.is_empty()),
        }
    }

    fn parse_comma(&mut self) -> Result<(), Error> {
        // Если мы не внутри групповой альтернации, то не
        // обрабатываем запятые специально. В противном случае, нам нужно начать
        // новую ветвь альтернации.
        if self.alternates_stack.is_empty() {
            self.push_token(Token::Literal(','))
        } else {
            Ok(self.branches.push(Tokens::default()))
        }
    }

    fn parse_backslash(&mut self) -> Result<(), Error> {
        if self.opts.backslash_escape {
            match self.bump() {
                None => Err(self.error(ErrorKind::DanglingEscape)),
                Some(c) => self.push_token(Token::Literal(c)),
            }
        } else if is_separator('\\') {
            // Нормализуем все шаблоны для использования / в качестве разделителя.
            self.push_token(Token::Literal('/'))
        } else {
            self.push_token(Token::Literal('\\'))
        }
    }

    fn parse_star(&mut self) -> Result<(), Error> {
        let prev = self.prev;
        if self.peek() != Some('*') {
            self.push_token(Token::ZeroOrMore)?;
            return Ok(());
        }
        assert!(self.bump() == Some('*'));
        if !self.have_tokens()? {
            if !self.peek().map_or(true, is_separator) {
                self.push_token(Token::ZeroOrMore)?;
                self.push_token(Token::ZeroOrMore)?;
            } else {
                self.push_token(Token::RecursivePrefix)?;
                assert!(self.bump().map_or(true, is_separator));
            }
            return Ok(());
        }

        if !prev.map(is_separator).unwrap_or(false) {
            if self.branches.len() <= 1
                || (prev != Some(',') && prev != Some('{'))
            {
                self.push_token(Token::ZeroOrMore)?;
                self.push_token(Token::ZeroOrMore)?;
                return Ok(());
            }
        }
        let is_suffix = match self.peek() {
            None => {
                assert!(self.bump().is_none());
                true
            }
            Some(',') | Some('}') if self.branches.len() >= 2 => true,
            Some(c) if is_separator(c) => {
                assert!(self.bump().map(is_separator).unwrap_or(false));
                false
            }
            _ => {
                self.push_token(Token::ZeroOrMore)?;
                self.push_token(Token::ZeroOrMore)?;
                return Ok(());
            }
        };
        match self.pop_token()? {
            Token::RecursivePrefix => {
                self.push_token(Token::RecursivePrefix)?;
            }
            Token::RecursiveSuffix => {
                self.push_token(Token::RecursiveSuffix)?;
            }
            _ => {
                if is_suffix {
                    self.push_token(Token::RecursiveSuffix)?;
                } else {
                    self.push_token(Token::RecursiveZeroOrMore)?;
                }
            }
        }
        Ok(())
    }

    fn parse_class(&mut self) -> Result<(), Error> {
        // Сохраняем состояние парсера для возможного отката к разбору буквального '['.
        let saved_chars = self.chars.clone();
        let saved_prev = self.prev;
        let saved_cur = self.cur;

        fn add_to_last_range(
            glob: &str,
            r: &mut (char, char),
            add: char,
        ) -> Result<(), Error> {
            r.1 = add;
            if r.1 < r.0 {
                Err(Error {
                    glob: Some(glob.to_string()),
                    kind: ErrorKind::InvalidRange(r.0, r.1),
                })
            } else {
                Ok(())
            }
        }
        let mut ranges = vec![];
        let negated = match self.chars.peek() {
            Some(&'!') | Some(&'^') => {
                let bump = self.bump();
                assert!(bump == Some('!') || bump == Some('^'));
                true
            }
            _ => false,
        };
        let mut first = true;
        let mut in_range = false;
        loop {
            let Some(c) = self.bump() else {
                return if self.opts.allow_unclosed_class == true {
                    self.chars = saved_chars;
                    self.cur = saved_cur;
                    self.prev = saved_prev;
                    self.found_unclosed_class = true;

                    self.push_token(Token::Literal('['))
                } else {
                    Err(self.error(ErrorKind::UnclosedClass))
                };
            };
            match c {
                ']' => {
                    if first {
                        ranges.push((']', ']'));
                    } else {
                        break;
                    }
                }
                '-' => {
                    if first {
                        ranges.push(('-', '-'));
                    } else if in_range {
                        // инвариант: in_range устанавливается только когда
                        // уже увидён по крайней мере один символ.
                        let r = ranges.last_mut().unwrap();
                        add_to_last_range(&self.glob, r, '-')?;
                        in_range = false;
                    } else {
                        assert!(!ranges.is_empty());
                        in_range = true;
                    }
                }
                c => {
                    if in_range {
                        // инвариант: in_range устанавливается только когда
                        // уже увидён по крайней мере один символ.
                        add_to_last_range(
                            &self.glob,
                            ranges.last_mut().unwrap(),
                            c,
                        )?;
                    } else {
                        ranges.push((c, c));
                    }
                    in_range = false;
                }
            }
            first = false;
        }
        if in_range {
            // Означает, что последним символом в классе был '-', поэтому добавляем
            // его как буквальный.
            ranges.push(('-', '-'));
        }
        self.push_token(Token::Class { negated, ranges })
    }

    fn bump(&mut self) -> Option<char> {
        self.prev = self.cur;
        self.cur = self.chars.next();
        self.cur
    }

    fn peek(&mut self) -> Option<char> {
        self.chars.peek().map(|&ch| ch)
    }
}

#[cfg(test)]
fn starts_with(needle: &[u8], haystack: &[u8]) -> bool {
    needle.len() <= haystack.len() && needle == &haystack[..needle.len()]
}

#[cfg(test)]
fn ends_with(needle: &[u8], haystack: &[u8]) -> bool {
    if needle.len() > haystack.len() {
        return false;
    }
    needle == &haystack[haystack.len() - needle.len()..]
}

#[cfg(test)]
mod tests {
    use super::Token::*;
    use super::{Glob, GlobBuilder, Token};
    use crate::{ErrorKind, GlobSetBuilder};

    #[derive(Clone, Copy, Debug, Default)]
    struct Options {
        casei: Option<bool>,
        litsep: Option<bool>,
        bsesc: Option<bool>,
        ealtre: Option<bool>,
        unccls: Option<bool>,
    }

    macro_rules! syntax {
        ($name:ident, $pat:expr, $tokens:expr) => {
            #[test]
            fn $name() {
                let pat = Glob::new($pat).unwrap();
                assert_eq!($tokens, pat.tokens.0);
            }
        };
    }

    macro_rules! syntaxerr {
        ($name:ident, $pat:expr, $err:expr) => {
            #[test]
            fn $name() {
                let err = Glob::new($pat).unwrap_err();
                assert_eq!(&$err, err.kind());
            }
        };
    }

    macro_rules! toregex {
        ($name:ident, $pat:expr, $re:expr) => {
            toregex!($name, $pat, $re, Options::default());
        };
        ($name:ident, $pat:expr, $re:expr, $options:expr) => {
            #[test]
            fn $name() {
                let mut builder = GlobBuilder::new($pat);
                if let Some(casei) = $options.casei {
                    builder.case_insensitive(casei);
                }
                if let Some(litsep) = $options.litsep {
                    builder.literal_separator(litsep);
                }
                if let Some(bsesc) = $options.bsesc {
                    builder.backslash_escape(bsesc);
                }
                if let Some(ealtre) = $options.ealtre {
                    builder.empty_alternates(ealtre);
                }
                if let Some(unccls) = $options.unccls {
                    builder.allow_unclosed_class(unccls);
                }

                let pat = builder.build().unwrap();
                assert_eq!(format!("(?-u){}", $re), pat.regex());
            }
        };
    }

    macro_rules! matches {
        ($name:ident, $pat:expr, $path:expr) => {
            matches!($name, $pat, $path, Options::default());
        };
        ($name:ident, $pat:expr, $path:expr, $options:expr) => {
            #[test]
            fn $name() {
                let mut builder = GlobBuilder::new($pat);
                if let Some(casei) = $options.casei {
                    builder.case_insensitive(casei);
                }
                if let Some(litsep) = $options.litsep {
                    builder.literal_separator(litsep);
                }
                if let Some(bsesc) = $options.bsesc {
                    builder.backslash_escape(bsesc);
                }
                if let Some(ealtre) = $options.ealtre {
                    builder.empty_alternates(ealtre);
                }
                let pat = builder.build().unwrap();
                let matcher = pat.compile_matcher();
                let strategic = pat.compile_strategic_matcher();
                let set = GlobSetBuilder::new().add(pat).build().unwrap();
                assert!(matcher.is_match($path));
                assert!(strategic.is_match($path));
                assert!(set.is_match($path));
            }
        };
    }

    macro_rules! nmatches {
        ($name:ident, $pat:expr, $path:expr) => {
            nmatches!($name, $pat, $path, Options::default());
        };
        ($name:ident, $pat:expr, $path:expr, $options:expr) => {
            #[test]
            fn $name() {
                let mut builder = GlobBuilder::new($pat);
                if let Some(casei) = $options.casei {
                    builder.case_insensitive(casei);
                }
                if let Some(litsep) = $options.litsep {
                    builder.literal_separator(litsep);
                }
                if let Some(bsesc) = $options.bsesc {
                    builder.backslash_escape(bsesc);
                }
                if let Some(ealtre) = $options.ealtre {
                    builder.empty_alternates(ealtre);
                }
                let pat = builder.build().unwrap();
                let matcher = pat.compile_matcher();
                let strategic = pat.compile_strategic_matcher();
                let set = GlobSetBuilder::new().add(pat).build().unwrap();
                assert!(!matcher.is_match($path));
                assert!(!strategic.is_match($path));
                assert!(!set.is_match($path));
            }
        };
    }

    fn s(string: &str) -> String {
        string.to_string()
    }

    fn class(s: char, e: char) -> Token {
        Class { negated: false, ranges: vec![(s, e)] }
    }

    fn classn(s: char, e: char) -> Token {
        Class { negated: true, ranges: vec![(s, e)] }
    }

    fn rclass(ranges: &[(char, char)]) -> Token {
        Class { negated: false, ranges: ranges.to_vec() }
    }

    fn rclassn(ranges: &[(char, char)]) -> Token {
        Class { negated: true, ranges: ranges.to_vec() }
    }

    syntax!(literal1, "a", vec![Literal('a')]);
    syntax!(literal2, "ab", vec![Literal('a'), Literal('b')]);
    syntax!(any1, "?", vec![Any]);
    syntax!(any2, "a?b", vec![Literal('a'), Any, Literal('b')]);
    syntax!(seq1, "*", vec![ZeroOrMore]);
    syntax!(seq2, "a*b", vec![Literal('a'), ZeroOrMore, Literal('b')]);
    syntax!(
        seq3,
        "*a*b*",
        vec![ZeroOrMore, Literal('a'), ZeroOrMore, Literal('b'), ZeroOrMore,]
    );
    syntax!(rseq1, "**", vec![RecursivePrefix]);
    syntax!(rseq2, "**/", vec![RecursivePrefix]);
    syntax!(rseq3, "/**", vec![RecursiveSuffix]);
    syntax!(rseq4, "/**/", vec![RecursiveZeroOrMore]);
    syntax!(
        rseq5,
        "a/**/b",
        vec![Literal('a'), RecursiveZeroOrMore, Literal('b'),]
    );
    syntax!(cls1, "[a]", vec![class('a', 'a')]);
    syntax!(cls2, "[!a]", vec![classn('a', 'a')]);
    syntax!(cls3, "[a-z]", vec![class('a', 'z')]);
    syntax!(cls4, "[!a-z]", vec![classn('a', 'z')]);
    syntax!(cls5, "[-]", vec![class('-', '-')]);
    syntax!(cls6, "[]]", vec![class(']', ']')]);
    syntax!(cls7, "[*]", vec![class('*', '*')]);
    syntax!(cls8, "[!!]", vec![classn('!', '!')]);
    syntax!(cls9, "[a-]", vec![rclass(&[('a', 'a'), ('-', '-')])]);
    syntax!(cls10, "[-a-z]", vec![rclass(&[('-', '-'), ('a', 'z')])]);
    syntax!(cls11, "[a-z-]", vec![rclass(&[('a', 'z'), ('-', '-')])]);
    syntax!(
        cls12,
        "[-a-z-]",
        vec![rclass(&[('-', '-'), ('a', 'z'), ('-', '-')]),]
    );
    syntax!(cls13, "[]-z]", vec![class(']', 'z')]);
    syntax!(cls14, "[--z]", vec![class('-', 'z')]);
    syntax!(cls15, "[ --]", vec![class(' ', '-')]);
    syntax!(cls16, "[0-9a-z]", vec![rclass(&[('0', '9'), ('a', 'z')])]);
    syntax!(cls17, "[a-z0-9]", vec![rclass(&[('a', 'z'), ('0', '9')])]);
    syntax!(cls18, "[!0-9a-z]", vec![rclassn(&[('0', '9'), ('a', 'z')])]);
    syntax!(cls19, "[!a-z0-9]", vec![rclassn(&[('a', 'z'), ('0', '9')])]);
    syntax!(cls20, "[^a]", vec![classn('a', 'a')]);
    syntax!(cls21, "[^a-z]", vec![classn('a', 'z')]);

    syntaxerr!(err_unclosed1, "[", ErrorKind::UnclosedClass);
    syntaxerr!(err_unclosed2, "[]", ErrorKind::UnclosedClass);
    syntaxerr!(err_unclosed3, "[!", ErrorKind::UnclosedClass);
    syntaxerr!(err_unclosed4, "[!]", ErrorKind::UnclosedClass);
    syntaxerr!(err_range1, "[z-a]", ErrorKind::InvalidRange('z', 'a'));
    syntaxerr!(err_range2, "[z--]", ErrorKind::InvalidRange('z', '-'));
    syntaxerr!(err_alt1, "{a,b", ErrorKind::UnclosedAlternates);
    syntaxerr!(err_alt2, "{a,{b,c}", ErrorKind::UnclosedAlternates);
    syntaxerr!(err_alt3, "a,b}", ErrorKind::UnopenedAlternates);
    syntaxerr!(err_alt4, "{a,b}}", ErrorKind::UnopenedAlternates);

    const CASEI: Options = Options {
        casei: Some(true),
        litsep: None,
        bsesc: None,
        ealtre: None,
        unccls: None,
    };
    const SLASHLIT: Options = Options {
        casei: None,
        litsep: Some(true),
        bsesc: None,
        ealtre: None,
        unccls: None,
    };
    const NOBSESC: Options = Options {
        casei: None,
        litsep: None,
        bsesc: Some(false),
        ealtre: None,
        unccls: None,
    };
    const BSESC: Options = Options {
        casei: None,
        litsep: None,
        bsesc: Some(true),
        ealtre: None,
        unccls: None,
    };
    const EALTRE: Options = Options {
        casei: None,
        litsep: None,
        bsesc: Some(true),
        ealtre: Some(true),
        unccls: None,
    };
    const UNCCLS: Options = Options {
        casei: None,
        litsep: None,
        bsesc: None,
        ealtre: None,
        unccls: Some(true),
    };

    toregex!(allow_unclosed_class_single, r"[", r"^\[$", &UNCCLS);
    toregex!(allow_unclosed_class_many, r"[abc", r"^\[abc$", &UNCCLS);
    toregex!(allow_unclosed_class_empty1, r"[]", r"^\[\]$", &UNCCLS);
    toregex!(allow_unclosed_class_empty2, r"[][", r"^\[\]\[$", &UNCCLS);
    toregex!(allow_unclosed_class_negated_unclosed, r"[!", r"^\[!$", &UNCCLS);
    toregex!(allow_unclosed_class_negated_empty, r"[!]", r"^\[!\]$", &UNCCLS);
    toregex!(
        allow_unclosed_class_brace1,
        r"{[abc,xyz}",
        r"^(?:\[abc|xyz)$",
        &UNCCLS
    );
    toregex!(
        allow_unclosed_class_brace2,
        r"{[abc,[xyz}",
        r"^(?:\[abc|\[xyz)$",
        &UNCCLS
    );
    toregex!(
        allow_unclosed_class_brace3,
        r"{[abc],[xyz}",
        r"^(?:[abc]|\[xyz)$",
        &UNCCLS
    );

    toregex!(re_empty, "", "^$");

    toregex!(re_casei, "a", "(?i)^a$", &CASEI);

    toregex!(re_slash1, "?", r"^[^/]$", SLASHLIT);
    toregex!(re_slash2, "*", r"^[^/]*$", SLASHLIT);

    toregex!(re1, "a", "^a$");
    toregex!(re2, "?", "^.$");
    toregex!(re3, "*", "^.*$");
    toregex!(re4, "a?", "^a.$");
    toregex!(re5, "?a", "^.a$");
    toregex!(re6, "a*", "^a.*$");
    toregex!(re7, "*a", "^.*a$");
    toregex!(re8, "[*]", r"^[\*]$");
    toregex!(re9, "[+]", r"^[\+]$");
    toregex!(re10, "+", r"^\+$");
    toregex!(re11, "☃", r"^\xe2\x98\x83$");
    toregex!(re12, "**", r"^.*$");
    toregex!(re13, "**/", r"^.*$");
    toregex!(re14, "**/*", r"^(?:/?|.*/).*$");
    toregex!(re15, "**/**", r"^.*$");
    toregex!(re16, "**/**/*", r"^(?:/?|.*/).*$");
    toregex!(re17, "**/**/**", r"^.*$");
    toregex!(re18, "**/**/**/*", r"^(?:/?|.*/).*$");
    toregex!(re19, "a/**", r"^a/.*$");
    toregex!(re20, "a/**/**", r"^a/.*$");
    toregex!(re21, "a/**/**/**", r"^a/.*$");
    toregex!(re22, "a/**/b", r"^a(?:/|/.*/)b$");
    toregex!(re23, "a/**/**/b", r"^a(?:/|/.*/)b$");
    toregex!(re24, "a/**/**/**/b", r"^a(?:/|/.*/)b$");
    toregex!(re25, "**/b", r"^(?:/?|.*/)b$");
    toregex!(re26, "**/**/b", r"^(?:/?|.*/)b$");
    toregex!(re27, "**/**/**/b", r"^(?:/?|.*/)b$");
    toregex!(re28, "a**", r"^a.*.*$");
    toregex!(re29, "**a", r"^.*.*a$");
    toregex!(re30, "a**b", r"^a.*.*b$");
    toregex!(re31, "***", r"^.*.*.*$");
    toregex!(re32, "/a**", r"^/a.*.*$");
    toregex!(re33, "/**a", r"^/.*.*a$");
    toregex!(re34, "/a**b", r"^/a.*.*b$");
    toregex!(re35, "{a,b}", r"^(?:a|b)$");
    toregex!(re36, "{a,{b,c}}", r"^(?:a|(?:b|c))$");
    toregex!(re37, "{{a,b},{c,d}}", r"^(?:(?:a|b)|(?:c|d))$");

    matches!(match1, "a", "a");
    matches!(match2, "a*b", "a_b");
    matches!(match3, "a*b*c", "abc");
    matches!(match4, "a*b*c", "a_b_c");
    matches!(match5, "a*b*c", "a___b___c");
    matches!(match6, "abc*abc*abc", "abcabcabcabcabcabcabc");
    matches!(match7, "a*a*a*a*a*a*a*a*a", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    matches!(match8, "a*b[xyz]c*d", "abxcdbxcddd");
    matches!(match9, "*.rs", ".rs");
    matches!(match10, "☃", "☃");

    matches!(matchrec1, "some/**/needle.txt", "some/needle.txt");
    matches!(matchrec2, "some/**/needle.txt", "some/one/needle.txt");
    matches!(matchrec3, "some/**/needle.txt", "some/one/two/needle.txt");
    matches!(matchrec4, "some/**/needle.txt", "some/other/needle.txt");
    matches!(matchrec5, "**", "abcde");
    matches!(matchrec6, "**", "");
    matches!(matchrec7, "**", ".asdf");
    matches!(matchrec8, "**", "/x/.asdf");
    matches!(matchrec9, "some/**/**/needle.txt", "some/needle.txt");
    matches!(matchrec10, "some/**/**/needle.txt", "some/one/needle.txt");
    matches!(matchrec11, "some/**/**/needle.txt", "some/one/two/needle.txt");
    matches!(matchrec12, "some/**/**/needle.txt", "some/other/needle.txt");
    matches!(matchrec13, "**/test", "one/two/test");
    matches!(matchrec14, "**/test", "one/test");
    matches!(matchrec15, "**/test", "test");
    matches!(matchrec16, "/**/test", "/one/two/test");
    matches!(matchrec17, "/**/test", "/one/test");
    matches!(matchrec18, "/**/test", "/test");
    matches!(matchrec19, "**/.*", ".abc");
    matches!(matchrec20, "**/.*", "abc/.abc");
    matches!(matchrec21, "**/foo/bar", "foo/bar");
    matches!(matchrec22, ".*/**", ".abc/abc");
    matches!(matchrec23, "test/**", "test/");
    matches!(matchrec24, "test/**", "test/one");
    matches!(matchrec25, "test/**", "test/one/two");
    matches!(matchrec26, "some/*/needle.txt", "some/one/needle.txt");

    matches!(matchrange1, "a[0-9]b", "a0b");
    matches!(matchrange2, "a[0-9]b", "a9b");
    matches!(matchrange3, "a[!0-9]b", "a_b");
    matches!(matchrange4, "[a-z123]", "1");
    matches!(matchrange5, "[1a-z23]", "1");
    matches!(matchrange6, "[123a-z]", "1");
    matches!(matchrange7, "[abc-]", "-");
    matches!(matchrange8, "[-abc]", "-");
    matches!(matchrange9, "[-a-c]", "b");
    matches!(matchrange10, "[a-c-]", "b");
    matches!(matchrange11, "[-]", "-");
    matches!(matchrange12, "a[^0-9]b", "a_b");

    matches!(matchpat1, "*hello.txt", "hello.txt");
    matches!(matchpat2, "*hello.txt", "gareth_says_hello.txt");
    matches!(matchpat3, "*hello.txt", "some/path/to/hello.txt");
    matches!(matchpat4, "*hello.txt", "some\\path\\to\\hello.txt");
    matches!(matchpat5, "*hello.txt", "/an/absolute/path/to/hello.txt");
    matches!(matchpat6, "*some/path/to/hello.txt", "some/path/to/hello.txt");
    matches!(
        matchpat7,
        "*some/path/to/hello.txt",
        "a/bigger/some/path/to/hello.txt"
    );

    matches!(matchescape, "_[[]_[]]_[?]_[*]_!_", "_[_]_?_*_!_");

    matches!(matchcasei1, "aBcDeFg", "aBcDeFg", CASEI);
    matches!(matchcasei2, "aBcDeFg", "abcdefg", CASEI);
    matches!(matchcasei3, "aBcDeFg", "ABCDEFG", CASEI);
    matches!(matchcasei4, "aBcDeFg", "AbCdEfG", CASEI);

    matches!(matchalt1, "a,b", "a,b");
    matches!(matchalt2, ",", ",");
    matches!(matchalt3, "{a,b}", "a");
    matches!(matchalt4, "{a,b}", "b");
    matches!(matchalt5, "{**/src/**,foo}", "abc/src/bar");
    matches!(matchalt6, "{**/src/**,foo}", "foo");
    matches!(matchalt7, "{[}],foo}", "}");
    matches!(matchalt8, "{foo}", "foo");
    matches!(matchalt9, "{}", "");
    matches!(matchalt10, "{,}", "");
    matches!(matchalt11, "{*.foo,*.bar,*.wat}", "test.foo");
    matches!(matchalt12, "{*.foo,*.bar,*.wat}", "test.bar");
    matches!(matchalt13, "{*.foo,*.bar,*.wat}", "test.wat");
    matches!(matchalt14, "foo{,.txt}", "foo.txt");
    nmatches!(matchalt15, "foo{,.txt}", "foo");
    matches!(matchalt16, "foo{,.txt}", "foo", EALTRE);
    matches!(matchalt17, "{a,b{c,d}}", "bc");
    matches!(matchalt18, "{a,b{c,d}}", "bd");
    matches!(matchalt19, "{a,b{c,d}}", "a");

    matches!(matchslash1, "abc/def", "abc/def", SLASHLIT);
    #[cfg(unix)]
    nmatches!(matchslash2, "abc?def", "abc/def", SLASHLIT);
    #[cfg(not(unix))]
    nmatches!(matchslash2, "abc?def", "abc\\def", SLASHLIT);
    nmatches!(matchslash3, "abc*def", "abc/def", SLASHLIT);
    matches!(matchslash4, "abc[/]def", "abc/def", SLASHLIT); // differs
    #[cfg(unix)]
    nmatches!(matchslash5, "abc\\def", "abc/def", SLASHLIT);
    #[cfg(not(unix))]
    matches!(matchslash5, "abc\\def", "abc/def", SLASHLIT);

    matches!(matchbackslash1, "\\[", "[", BSESC);
    matches!(matchbackslash2, "\\?", "?", BSESC);
    matches!(matchbackslash3, "\\*", "*", BSESC);
    matches!(matchbackslash4, "\\[a-z]", "\\a", NOBSESC);
    matches!(matchbackslash5, "\\?", "\\a", NOBSESC);
    matches!(matchbackslash6, "\\*", "\\\\", NOBSESC);
    #[cfg(unix)]
    matches!(matchbackslash7, "\\a", "a");
    #[cfg(not(unix))]
    matches!(matchbackslash8, "\\a", "/a");

    nmatches!(matchnot1, "a*b*c", "abcd");
    nmatches!(matchnot2, "abc*abc*abc", "abcabcabcabcabcabcabca");
    nmatches!(matchnot3, "some/**/needle.txt", "some/other/notthis.txt");
    nmatches!(matchnot4, "some/**/**/needle.txt", "some/other/notthis.txt");
    nmatches!(matchnot5, "/**/test", "test");
    nmatches!(matchnot6, "/**/test", "/one/notthis");
    nmatches!(matchnot7, "/**/test", "/notthis");
    nmatches!(matchnot8, "**/.*", "ab.c");
    nmatches!(matchnot9, "**/.*", "abc/ab.c");
    nmatches!(matchnot10, ".*/**", "a.bc");
    nmatches!(matchnot11, ".*/**", "abc/a.bc");
    nmatches!(matchnot12, "a[0-9]b", "a_b");
    nmatches!(matchnot13, "a[!0-9]b", "a0b");
    nmatches!(matchnot14, "a[!0-9]b", "a9b");
    nmatches!(matchnot15, "[!-]", "-");
    nmatches!(matchnot16, "*hello.txt", "hello.txt-and-then-some");
    nmatches!(matchnot17, "*hello.txt", "goodbye.txt");
    nmatches!(
        matchnot18,
        "*some/path/to/hello.txt",
        "some/path/to/hello.txt-and-then-some"
    );
    nmatches!(
        matchnot19,
        "*some/path/to/hello.txt",
        "some/other/path/to/hello.txt"
    );
    nmatches!(matchnot20, "a", "foo/a");
    nmatches!(matchnot21, "./foo", "foo");
    nmatches!(matchnot22, "**/foo", "foofoo");
    nmatches!(matchnot23, "**/foo/bar", "foofoo/bar");
    nmatches!(matchnot24, "/*.c", "mozilla-sha1/sha1.c");
    nmatches!(matchnot25, "*.c", "mozilla-sha1/sha1.c", SLASHLIT);
    nmatches!(
        matchnot26,
        "**/m4/ltoptions.m4",
        "csharp/src/packages/repositories.config",
        SLASHLIT
    );
    nmatches!(matchnot27, "a[^0-9]b", "a0b");
    nmatches!(matchnot28, "a[^0-9]b", "a9b");
    nmatches!(matchnot29, "[^-]", "-");
    nmatches!(matchnot30, "some/*/needle.txt", "some/needle.txt");
    nmatches!(
        matchrec31,
        "some/*/needle.txt",
        "some/one/two/needle.txt",
        SLASHLIT
    );
    nmatches!(
        matchrec32,
        "some/*/needle.txt",
        "some/one/two/three/needle.txt",
        SLASHLIT
    );
    nmatches!(matchrec33, ".*/**", ".abc");
    nmatches!(matchrec34, "foo/**", "foo");

    macro_rules! extract {
        ($which:ident, $name:ident, $pat:expr, $expect:expr) => {
            extract!($which, $name, $pat, $expect, Options::default());
        };
        ($which:ident, $name:ident, $pat:expr, $expect:expr, $options:expr) => {
            #[test]
            fn $name() {
                let mut builder = GlobBuilder::new($pat);
                if let Some(casei) = $options.casei {
                    builder.case_insensitive(casei);
                }
                if let Some(litsep) = $options.litsep {
                    builder.literal_separator(litsep);
                }
                if let Some(bsesc) = $options.bsesc {
                    builder.backslash_escape(bsesc);
                }
                if let Some(ealtre) = $options.ealtre {
                    builder.empty_alternates(ealtre);
                }
                let pat = builder.build().unwrap();
                assert_eq!($expect, pat.$which());
            }
        };
    }

    macro_rules! literal {
        ($($tt:tt)*) => { extract!(literal, $($tt)*); }
    }

    macro_rules! basetokens {
        ($($tt:tt)*) => { extract!(basename_tokens, $($tt)*); }
    }

    macro_rules! ext {
        ($($tt:tt)*) => { extract!(ext, $($tt)*); }
    }

    macro_rules! required_ext {
        ($($tt:tt)*) => { extract!(required_ext, $($tt)*); }
    }

    macro_rules! prefix {
        ($($tt:tt)*) => { extract!(prefix, $($tt)*); }
    }

    macro_rules! suffix {
        ($($tt:tt)*) => { extract!(suffix, $($tt)*); }
    }

    macro_rules! baseliteral {
        ($($tt:tt)*) => { extract!(basename_literal, $($tt)*); }
    }

    literal!(extract_lit1, "foo", Some(s("foo")));
    literal!(extract_lit2, "foo", None, CASEI);
    literal!(extract_lit3, "/foo", Some(s("/foo")));
    literal!(extract_lit4, "/foo/", Some(s("/foo/")));
    literal!(extract_lit5, "/foo/bar", Some(s("/foo/bar")));
    literal!(extract_lit6, "*.foo", None);
    literal!(extract_lit7, "foo/bar", Some(s("foo/bar")));
    literal!(extract_lit8, "**/foo/bar", None);

    basetokens!(
        extract_basetoks1,
        "**/foo",
        Some(&*vec![Literal('f'), Literal('o'), Literal('o'),])
    );
    basetokens!(extract_basetoks2, "**/foo", None, CASEI);
    basetokens!(
        extract_basetoks3,
        "**/foo",
        Some(&*vec![Literal('f'), Literal('o'), Literal('o'),]),
        SLASHLIT
    );
    basetokens!(extract_basetoks4, "*foo", None, SLASHLIT);
    basetokens!(extract_basetoks5, "*foo", None);
    basetokens!(extract_basetoks6, "**/fo*o", None);
    basetokens!(
        extract_basetoks7,
        "**/fo*o",
        Some(&*vec![Literal('f'), Literal('o'), ZeroOrMore, Literal('o'),]),
        SLASHLIT
    );

    ext!(extract_ext1, "**/*.rs", Some(s(".rs")));
    ext!(extract_ext2, "**/*.rs.bak", None);
    ext!(extract_ext3, "*.rs", Some(s(".rs")));
    ext!(extract_ext4, "a*.rs", None);
    ext!(extract_ext5, "/*.c", None);
    ext!(extract_ext6, "*.c", None, SLASHLIT);
    ext!(extract_ext7, "*.c", Some(s(".c")));

    required_ext!(extract_req_ext1, "*.rs", Some(s(".rs")));
    required_ext!(extract_req_ext2, "/foo/bar/*.rs", Some(s(".rs")));
    required_ext!(extract_req_ext3, "/foo/bar/*.rs", Some(s(".rs")));
    required_ext!(extract_req_ext4, "/foo/bar/.rs", Some(s(".rs")));
    required_ext!(extract_req_ext5, ".rs", Some(s(".rs")));
    required_ext!(extract_req_ext6, "./rs", None);
    required_ext!(extract_req_ext7, "foo", None);
    required_ext!(extract_req_ext8, ".foo/", None);
    required_ext!(extract_req_ext9, "foo/", None);

    prefix!(extract_prefix1, "/foo", Some(s("/foo")));
    prefix!(extract_prefix2, "/foo/*", Some(s("/foo/")));
    prefix!(extract_prefix3, "**/foo", None);
    prefix!(extract_prefix4, "foo/**", Some(s("foo/")));

    suffix!(extract_suffix1, "**/foo/bar", Some((s("/foo/bar"), true)));
    suffix!(extract_suffix2, "*/foo/bar", Some((s("/foo/bar"), false)));
    suffix!(extract_suffix3, "*/foo/bar", None, SLASHLIT);
    suffix!(extract_suffix4, "foo/bar", Some((s("foo/bar"), false)));
    suffix!(extract_suffix5, "*.foo", Some((s(".foo"), false)));
    suffix!(extract_suffix6, "*.foo", None, SLASHLIT);
    suffix!(extract_suffix7, "**/*_test", Some((s("_test"), false)));

    baseliteral!(extract_baselit1, "**/foo", Some(s("foo")));
    baseliteral!(extract_baselit2, "foo", None);
    baseliteral!(extract_baselit3, "*foo", None);
    baseliteral!(extract_baselit4, "*/foo", None);
}
