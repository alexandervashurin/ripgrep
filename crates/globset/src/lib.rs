/*!
Крейт globset предоставляет кроссплатформенное сопоставление одиночных glob
и наборов glob.

Сопоставление набора glob — это процесс одновременного сопоставления одного
или нескольких шаблонов glob с одним кандидатом пути и возврата всех glob,
которые совпали. Например, дан следующий набор glob:

* `*.rs`
* `src/lib.rs`
* `src/**/foo.rs`

и путь `src/bar/baz/foo.rs`, тогда набор сообщит, что первый и третий
glob совпали.

# Пример: один glob

Этот пример показывает, как сопоставить один glob с одним путём к файлу.

```
use globset::Glob;

let glob = Glob::new("*.rs")?.compile_matcher();

assert!(glob.is_match("foo.rs"));
assert!(glob.is_match("foo/bar.rs"));
assert!(!glob.is_match("Cargo.toml"));
# Ok::<(), Box<dyn std::error::Error>>(())
```

# Пример: настройка matcher glob

Этот пример показывает, как использовать `GlobBuilder` для настройки
аспектов семантики сопоставления. В этом примере мы предотвращаем
сопоставление подстановочных знаков с разделителями путей.

```
use globset::GlobBuilder;

let glob = GlobBuilder::new("*.rs")
    .literal_separator(true).build()?.compile_matcher();

assert!(glob.is_match("foo.rs"));
assert!(!glob.is_match("foo/bar.rs")); // больше не соответствует
assert!(!glob.is_match("Cargo.toml"));
# Ok::<(), Box<dyn std::error::Error>>(())
```

# Пример: сопоставление нескольких glob одновременно

Этот пример показывает, как сопоставить несколько шаблонов glob одновременно.

```
use globset::{Glob, GlobSetBuilder};

let mut builder = GlobSetBuilder::new();
// GlobBuilder можно использовать для настройки семантики сопоставления
// каждого glob независимо.
builder.add(Glob::new("*.rs")?);
builder.add(Glob::new("src/lib.rs")?);
builder.add(Glob::new("src/**/foo.rs")?);
let set = builder.build()?;

assert_eq!(set.matches("src/bar/baz/foo.rs"), vec![0, 2]);
# Ok::<(), Box<dyn std::error::Error>>(())
```

# Синтаксис

Поддерживается стандартный синтаксис glob в стиле Unix:

* `?` соответствует любому одиночному символу. (Если включена опция
  `literal_separator`, то `?` никогда не может соответствовать разделителю
  пути.)
* `*` соответствует нулю или более символам. (Если включена опция
  `literal_separator`, то `*` никогда не может соответствовать разделителю
  пути.)
* `**` рекурсивно соответствует директориям, но разрешён только в трёх
  ситуациях. Во-первых, если glob начинается с <code>\*\*&#x2F;</code>,
  то он соответствует всем директориям. Например,
  <code>\*\*&#x2F;foo</code> соответствует `foo` и `bar/foo`, но не
  `foo/bar`. Во-вторых, если glob заканчивается на
  <code>&#x2F;\*\*</code>, то он соответствует всем вложенным элементам.
  Например, <code>foo&#x2F;\*\*</code> соответствует `foo/a` и `foo/a/b`,
  но не `foo`. В-третьих, если glob содержит
  <code>&#x2F;\*\*&#x2F;</code> где-либо внутри шаблона, то он соответствует
  нулю или более директориям. Использование `**` в любом другом месте
  незаконно (N.B. glob `**` разрешён и означает «соответствовать всему»).
* `{a,b}` соответствует `a` или `b`, где `a` и `b` — произвольные шаблоны
  glob. (N.B. Вложение `{...}` в настоящее время не разрешено.)
* `[ab]` соответствует `a` или `b`, где `a` и `b` — символы. Используйте
  `[!ab]` для сопоставления любого символа, кроме `a` и `b`.
* Мета-символы, такие как `*` и `?`, могут быть экранированы с помощью
  обозначения класса символов. Например, `[*]` соответствует `*`.
* Когда включены экранирования обратной косой чертой, обратная косая черта
  (`\`) будет экранировать все мета-символы в glob. Если она предшествует
  не мета-символу, то косая черта игнорируется. `\\` будет соответствовать
  буквальному `\\`. Обратите внимание, что этот режим включён по умолчанию
  только на платформах Unix, но может быть включён на любой платформе
  через настройку `backslash_escape` в `Glob`.

`GlobBuilder` можно использовать для предотвращения сопоставления
подстановочными знаками разделителей путей или для включения
регистронезависимого сопоставления.

# Возможности крейта

Этот крейт включает необязательные функции, которые могут быть включены
при необходимости. Эти функции не требуются, но могут быть полезны в
зависимости от варианта использования.

Доступны следующие функции:

* **arbitrary** —
  Включение этой функции создаёт публичную зависимость от крейта
  [`arbitrary`](https://crates.io/crates/arbitrary).
  А именно, он реализует трейт `Arbitrary` из этого крейта для типа
  [`Glob`]. Эта функция отключена по умолчанию.
*/

#![deny(missing_docs)]

use std::{
    borrow::Cow,
    panic::{RefUnwindSafe, UnwindSafe},
    path::Path,
    sync::Arc,
};

use {
    aho_corasick::AhoCorasick,
    bstr::{B, ByteSlice, ByteVec},
    regex_automata::{
        PatternSet,
        meta::Regex,
        util::pool::{Pool, PoolGuard},
    },
};

use crate::{
    glob::MatchStrategy,
    pathutil::{file_name, file_name_ext, normalize_path},
};

pub use crate::glob::{Glob, GlobBuilder, GlobMatcher};

mod fnv;
mod glob;
mod pathutil;

#[cfg(feature = "serde1")]
mod serde_impl;

#[cfg(feature = "log")]
macro_rules! debug {
    ($($token:tt)*) => (::log::debug!($($token)*);)
}

#[cfg(not(feature = "log"))]
macro_rules! debug {
    ($($token:tt)*) => {};
}

/// Представляет ошибку, которая может возникнуть при разборе шаблона glob.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Error {
    /// Исходный glob, предоставленный вызывающей стороной.
    glob: Option<String>,
    /// Вид ошибки.
    kind: ErrorKind,
}

/// Вид ошибки, которая может возникнуть при разборе шаблона glob.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ErrorKind {
    /// **УСТАРЕЛО**.
    ///
    /// Эта ошибка раньше возникала для согласованности со спецификацией
    /// git, но теперь спецификация принимает все использования `**`.
    /// Когда `**` не появляется рядом с разделителем пути или в начале/
    /// конце glob, оно теперь трактуется как два последовательных
    /// шаблона `*`. Таким образом, эта ошибка больше не используется.
    InvalidRecursive,
    /// Возникает, когда класс символов (например, `[abc]`) не закрыт.
    UnclosedClass,
    /// Возникает, когда диапазон в символе (например, `[a-z]`) некорректен.
    /// Например, если диапазон начинается с символа, который лексикографически
    /// больше, чем его конец.
    InvalidRange(char, char),
    /// Возникает, когда найдена `}` без соответствующей `{`.
    UnopenedAlternates,
    /// Возникает, когда найдена `{` без соответствующей `}`.
    UnclosedAlternates,
    /// **УСТАРЕЛО**.
    ///
    /// Эта ошибка раньше возникала, когда альтернирующая группа была вложена
    /// внутри другой альтернирующей группы, например, `{{a,b},{c,d}}`.
    /// Однако теперь это поддерживается, и такая ошибка не может возникнуть.
    NestedAlternates,
    /// Возникает, когда неэкранированный '\' найден в конце glob.
    DanglingEscape,
    /// Ошибка, связанная с разбором или компиляцией регулярного выражения.
    Regex(String),
}

impl std::error::Error for Error {
    fn description(&self) -> &str {
        self.kind.description()
    }
}

impl Error {
    /// Возвращает glob, вызвавший эту ошибку, если он существует.
    pub fn glob(&self) -> Option<&str> {
        self.glob.as_ref().map(|s| &**s)
    }

    /// Возвращает вид этой ошибки.
    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }
}

impl ErrorKind {
    fn description(&self) -> &str {
        match *self {
            ErrorKind::InvalidRecursive => {
                "некорректное использование **; должен быть один компонент пути"
            }
            ErrorKind::UnclosedClass => {
                "незакрытый класс символов; отсутствует ']'"
            }
            ErrorKind::InvalidRange(_, _) => "некорректный диапазон символов",
            ErrorKind::UnopenedAlternates => {
                "неоткрытая группа альтернатив; отсутствует '{' \
                (может, экранировать '}' с помощью '[}]'?)"
            }
            ErrorKind::UnclosedAlternates => {
                "незакрытая группа альтернатив; отсутствует '}' \
                (может, экранировать '{' с помощью '[{]'?)"
            }
            ErrorKind::NestedAlternates => {
                "вложенные группы альтернатив не разрешены"
            }
            ErrorKind::DanglingEscape => "висящий '\\'",
            ErrorKind::Regex(ref err) => err,
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.glob {
            None => self.kind.fmt(f),
            Some(ref glob) => {
                write!(f, "error parsing glob '{}': {}", glob, self.kind)
            }
        }
    }
}

impl std::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            ErrorKind::InvalidRecursive
            | ErrorKind::UnclosedClass
            | ErrorKind::UnopenedAlternates
            | ErrorKind::UnclosedAlternates
            | ErrorKind::NestedAlternates
            | ErrorKind::DanglingEscape
            | ErrorKind::Regex(_) => write!(f, "{}", self.description()),
            ErrorKind::InvalidRange(s, e) => {
                write!(f, "invalid range; '{}' > '{}'", s, e)
            }
        }
    }
}

fn new_regex(pat: &str) -> Result<Regex, Error> {
    let syntax = regex_automata::util::syntax::Config::new()
        .utf8(false)
        .dot_matches_new_line(true);
    let config = Regex::config()
        .utf8_empty(false)
        .nfa_size_limit(Some(10 * (1 << 20)))
        .hybrid_cache_capacity(10 * (1 << 20));
    Regex::builder().syntax(syntax).configure(config).build(pat).map_err(
        |err| Error {
            glob: Some(pat.to_string()),
            kind: ErrorKind::Regex(err.to_string()),
        },
    )
}

fn new_regex_set(pats: Vec<String>) -> Result<Regex, Error> {
    let syntax = regex_automata::util::syntax::Config::new()
        .utf8(false)
        .dot_matches_new_line(true);
    let config = Regex::config()
        .match_kind(regex_automata::MatchKind::All)
        .utf8_empty(false)
        .nfa_size_limit(Some(10 * (1 << 20)))
        .hybrid_cache_capacity(10 * (1 << 20));
    Regex::builder()
        .syntax(syntax)
        .configure(config)
        .build_many(&pats)
        .map_err(|err| Error {
            glob: None,
            kind: ErrorKind::Regex(err.to_string()),
        })
}

/// GlobSet представляет группу glob, которые могут быть сопоставлены
/// вместе за один проход.
#[derive(Clone, Debug)]
pub struct GlobSet {
    len: usize,
    strats: Vec<GlobSetMatchStrategy>,
}

impl GlobSet {
    /// Создаёт новый [`GlobSetBuilder`]. `GlobSetBuilder` можно использовать
    /// для добавления новых шаблонов. Как только все шаблоны добавлены,
    /// следует вызвать `build` для создания `GlobSet`, который затем можно
    /// использовать для сопоставления.
    #[inline]
    pub fn builder() -> GlobSetBuilder {
        GlobSetBuilder::new()
    }

    /// Создаёт пустой `GlobSet`. Пустой набор ничего не соответствует.
    #[inline]
    pub const fn empty() -> GlobSet {
        GlobSet { len: 0, strats: vec![] }
    }

    /// Возвращает true, если этот набор пуст и, следовательно, ничего не соответствует.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Возвращает количество glob в этом наборе.
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Возвращает true, если какой-либо glob в этом наборе соответствует данному пути.
    pub fn is_match<P: AsRef<Path>>(&self, path: P) -> bool {
        self.is_match_candidate(&Candidate::new(path.as_ref()))
    }

    /// Возвращает true, если какой-либо glob в этом наборе соответствует данному пути.
    ///
    /// Это принимает Candidate в качестве входных данных, что можно использовать
    /// для амортизации стоимости подготовки пути к сопоставлению.
    pub fn is_match_candidate(&self, path: &Candidate<'_>) -> bool {
        if self.is_empty() {
            return false;
        }
        for strat in &self.strats {
            if strat.is_match(path) {
                return true;
            }
        }
        false
    }

    /// Возвращает true, если все glob в этом наборе соответствуют данному пути.
    ///
    /// Это вернёт true, если набор glob пуст, так как в этом случае все
    /// `0` glob совпадут.
    ///
    /// ```
    /// use globset::{Glob, GlobSetBuilder};
    ///
    /// let mut builder = GlobSetBuilder::new();
    /// builder.add(Glob::new("src/*").unwrap());
    /// builder.add(Glob::new("**/*.rs").unwrap());
    /// let set = builder.build().unwrap();
    ///
    /// assert!(set.matches_all("src/foo.rs"));
    /// assert!(!set.matches_all("src/bar.c"));
    /// assert!(!set.matches_all("test.rs"));
    /// ```
    pub fn matches_all<P: AsRef<Path>>(&self, path: P) -> bool {
        self.matches_all_candidate(&Candidate::new(path.as_ref()))
    }

    /// Возвращает true, если все glob в этом наборе соответствуют данному пути.
    ///
    /// Это принимает Candidate в качестве входных данных, что можно использовать
    /// для амортизации стоимости подготовки пути к сопоставлению.
    ///
    /// Это вернёт true, если набор glob пуст, так как в этом случае все
    /// `0` glob совпадут.
    pub fn matches_all_candidate(&self, path: &Candidate<'_>) -> bool {
        for strat in &self.strats {
            if !strat.is_match(path) {
                return false;
            }
        }
        true
    }

    /// Возвращает порядковый номер каждого шаблона glob, который соответствует
    /// данному пути.
    pub fn matches<P: AsRef<Path>>(&self, path: P) -> Vec<usize> {
        self.matches_candidate(&Candidate::new(path.as_ref()))
    }

    /// Возвращает порядковый номер каждого шаблона glob, который соответствует
    /// данному пути.
    ///
    /// Это принимает Candidate в качестве входных данных, что можно использовать
    /// для амортизации стоимости подготовки пути к сопоставлению.
    pub fn matches_candidate(&self, path: &Candidate<'_>) -> Vec<usize> {
        let mut into = vec![];
        if self.is_empty() {
            return into;
        }
        self.matches_candidate_into(path, &mut into);
        into
    }

    /// Добавляет порядковый номер каждого шаблона glob, который соответствует
    /// данному пути, в указанный вектор.
    ///
    /// `into` очищается перед началом сопоставления и содержит набор
    /// порядковых номеров (в порядке возрастания) после завершения
    /// сопоставления. Если ни один glob не совпал, то `into` будет пуст.
    pub fn matches_into<P: AsRef<Path>>(
        &self,
        path: P,
        into: &mut Vec<usize>,
    ) {
        self.matches_candidate_into(&Candidate::new(path.as_ref()), into);
    }

    /// Добавляет порядковый номер каждого шаблона glob, который соответствует
    /// данному пути, в указанный вектор.
    ///
    /// `into` очищается перед началом сопоставления и содержит набор
    /// порядковых номеров (в порядке возрастания) после завершения
    /// сопоставления. Если ни один glob не совпал, то `into` будет пуст.
    ///
    /// Это принимает Candidate в качестве входных данных, что можно использовать
    /// для амортизации стоимости подготовки пути к сопоставлению.
    pub fn matches_candidate_into(
        &self,
        path: &Candidate<'_>,
        into: &mut Vec<usize>,
    ) {
        into.clear();
        if self.is_empty() {
            return;
        }
        for strat in &self.strats {
            strat.matches_into(path, into);
        }
        into.sort();
        into.dedup();
    }

    /// Строит новый matcher из коллекции шаблонов Glob.
    ///
    /// Как только matcher построен, в него нельзя добавить новые шаблоны.
    pub fn new<I, G>(globs: I) -> Result<GlobSet, Error>
    where
        I: IntoIterator<Item = G>,
        G: AsRef<Glob>,
    {
        let mut it = globs.into_iter().peekable();
        if it.peek().is_none() {
            return Ok(GlobSet::empty());
        }

        let mut len = 0;
        let mut lits = LiteralStrategy::new();
        let mut base_lits = BasenameLiteralStrategy::new();
        let mut exts = ExtensionStrategy::new();
        let mut prefixes = MultiStrategyBuilder::new();
        let mut suffixes = MultiStrategyBuilder::new();
        let mut required_exts = RequiredExtensionStrategyBuilder::new();
        let mut regexes = MultiStrategyBuilder::new();
        for (i, p) in it.enumerate() {
            len += 1;

            let p = p.as_ref();
            match MatchStrategy::new(p) {
                MatchStrategy::Literal(lit) => {
                    lits.add(i, lit);
                }
                MatchStrategy::BasenameLiteral(lit) => {
                    base_lits.add(i, lit);
                }
                MatchStrategy::Extension(ext) => {
                    exts.add(i, ext);
                }
                MatchStrategy::Prefix(prefix) => {
                    prefixes.add(i, prefix);
                }
                MatchStrategy::Suffix { suffix, component } => {
                    if component {
                        lits.add(i, suffix[1..].to_string());
                    }
                    suffixes.add(i, suffix);
                }
                MatchStrategy::RequiredExtension(ext) => {
                    required_exts.add(i, ext, p.regex().to_owned());
                }
                MatchStrategy::Regex => {
                    debug!(
                        "glob `{:?}` converted to regex: `{:?}`",
                        p,
                        p.regex()
                    );
                    regexes.add(i, p.regex().to_owned());
                }
            }
        }
        debug!(
            "built glob set; {} literals, {} basenames, {} extensions, \
                {} prefixes, {} suffixes, {} required extensions, {} regexes",
            lits.0.len(),
            base_lits.0.len(),
            exts.0.len(),
            prefixes.literals.len(),
            suffixes.literals.len(),
            required_exts.0.len(),
            regexes.literals.len()
        );
        let mut strats = Vec::with_capacity(7);
        // Добавляем только те стратегии, которые заполнены
        if !exts.0.is_empty() {
            strats.push(GlobSetMatchStrategy::Extension(exts));
        }
        if !base_lits.0.is_empty() {
            strats.push(GlobSetMatchStrategy::BasenameLiteral(base_lits));
        }
        if !lits.0.is_empty() {
            strats.push(GlobSetMatchStrategy::Literal(lits));
        }
        if !suffixes.is_empty() {
            strats.push(GlobSetMatchStrategy::Suffix(suffixes.suffix()));
        }
        if !prefixes.is_empty() {
            strats.push(GlobSetMatchStrategy::Prefix(prefixes.prefix()));
        }
        if !required_exts.0.is_empty() {
            strats.push(GlobSetMatchStrategy::RequiredExtension(
                required_exts.build()?,
            ));
        }
        if !regexes.is_empty() {
            strats.push(GlobSetMatchStrategy::Regex(regexes.regex_set()?));
        }

        Ok(GlobSet { len, strats })
    }
}

impl Default for GlobSet {
    /// Создаёт пустой GlobSet по умолчанию.
    fn default() -> Self {
        GlobSet::empty()
    }
}

/// GlobSetBuilder строит группу шаблонов, которые можно использовать
/// для одновременного сопоставления пути к файлу.
#[derive(Clone, Debug)]
pub struct GlobSetBuilder {
    pats: Vec<Glob>,
}

impl GlobSetBuilder {
    /// Создаёт новый `GlobSetBuilder`. `GlobSetBuilder` можно использовать
    /// для добавления новых шаблонов. Как только все шаблоны добавлены,
    /// следует вызвать `build` для создания [`GlobSet`], который затем
    /// можно использовать для сопоставления.
    pub fn new() -> GlobSetBuilder {
        GlobSetBuilder { pats: vec![] }
    }

    /// Строит новый matcher из всех шаблонов glob, добавленных на данный момент.
    ///
    /// Как только matcher построен, в него нельзя добавить новые шаблоны.
    pub fn build(&self) -> Result<GlobSet, Error> {
        GlobSet::new(self.pats.iter())
    }

    /// Добавляет новый шаблон в этот набор.
    pub fn add(&mut self, pat: Glob) -> &mut GlobSetBuilder {
        self.pats.push(pat);
        self
    }
}

/// Кандидат пути для сопоставления.
///
/// Всё сопоставление glob в этом крейте работает со значениями `Candidate`.
/// Построение кандидатов имеет очень небольшую стоимость, поэтому вызывающие
/// стороны могут счесть полезным амортизировать эту стоимость при
/// сопоставлении одного пути с несколькими glob или наборами glob.
#[derive(Clone)]
pub struct Candidate<'a> {
    path: Cow<'a, [u8]>,
    basename: Cow<'a, [u8]>,
    ext: Cow<'a, [u8]>,
}

impl<'a> std::fmt::Debug for Candidate<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("Candidate")
            .field("path", &self.path.as_bstr())
            .field("basename", &self.basename.as_bstr())
            .field("ext", &self.ext.as_bstr())
            .finish()
    }
}

impl<'a> Candidate<'a> {
    /// Создаёт нового кандидата для сопоставления из данного пути.
    pub fn new<P: AsRef<Path> + ?Sized>(path: &'a P) -> Candidate<'a> {
        Self::from_cow(Vec::from_path_lossy(path.as_ref()))
    }

    /// Создаёт нового кандидата для сопоставления из данного пути как
    /// последовательности байтов.
    ///
    /// Вообще говоря, ожидается, что байты будут _условно_ UTF-8.
    /// Последовательность байтов может содержать недопустимый UTF-8.
    /// Однако, если байты находятся в какой-либо другой кодировке,
    /// не совместимой с ASCII (например, UTF-16), то результаты
    /// сопоставления не определены.
    pub fn from_bytes<P: AsRef<[u8]> + ?Sized>(path: &'a P) -> Candidate<'a> {
        Self::from_cow(Cow::Borrowed(path.as_ref()))
    }

    fn from_cow(path: Cow<'a, [u8]>) -> Candidate<'a> {
        let path = normalize_path(path);
        let basename = file_name(&path).unwrap_or(Cow::Borrowed(B("")));
        let ext = file_name_ext(&basename).unwrap_or(Cow::Borrowed(B("")));
        Candidate { path, basename, ext }
    }

    fn path_prefix(&self, max: usize) -> &[u8] {
        if self.path.len() <= max { &*self.path } else { &self.path[..max] }
    }

    fn path_suffix(&self, max: usize) -> &[u8] {
        if self.path.len() <= max {
            &*self.path
        } else {
            &self.path[self.path.len() - max..]
        }
    }
}

#[derive(Clone, Debug)]
enum GlobSetMatchStrategy {
    Literal(LiteralStrategy),
    BasenameLiteral(BasenameLiteralStrategy),
    Extension(ExtensionStrategy),
    Prefix(PrefixStrategy),
    Suffix(SuffixStrategy),
    RequiredExtension(RequiredExtensionStrategy),
    Regex(RegexSetStrategy),
}

impl GlobSetMatchStrategy {
    fn is_match(&self, candidate: &Candidate<'_>) -> bool {
        use self::GlobSetMatchStrategy::*;
        match *self {
            Literal(ref s) => s.is_match(candidate),
            BasenameLiteral(ref s) => s.is_match(candidate),
            Extension(ref s) => s.is_match(candidate),
            Prefix(ref s) => s.is_match(candidate),
            Suffix(ref s) => s.is_match(candidate),
            RequiredExtension(ref s) => s.is_match(candidate),
            Regex(ref s) => s.is_match(candidate),
        }
    }

    fn matches_into(
        &self,
        candidate: &Candidate<'_>,
        matches: &mut Vec<usize>,
    ) {
        use self::GlobSetMatchStrategy::*;
        match *self {
            Literal(ref s) => s.matches_into(candidate, matches),
            BasenameLiteral(ref s) => s.matches_into(candidate, matches),
            Extension(ref s) => s.matches_into(candidate, matches),
            Prefix(ref s) => s.matches_into(candidate, matches),
            Suffix(ref s) => s.matches_into(candidate, matches),
            RequiredExtension(ref s) => s.matches_into(candidate, matches),
            Regex(ref s) => s.matches_into(candidate, matches),
        }
    }
}

#[derive(Clone, Debug)]
struct LiteralStrategy(fnv::HashMap<Vec<u8>, Vec<usize>>);

impl LiteralStrategy {
    fn new() -> LiteralStrategy {
        LiteralStrategy(fnv::HashMap::default())
    }

    fn add(&mut self, global_index: usize, lit: String) {
        self.0.entry(lit.into_bytes()).or_insert(vec![]).push(global_index);
    }

    fn is_match(&self, candidate: &Candidate<'_>) -> bool {
        self.0.contains_key(candidate.path.as_bytes())
    }

    #[inline(never)]
    fn matches_into(
        &self,
        candidate: &Candidate<'_>,
        matches: &mut Vec<usize>,
    ) {
        if let Some(hits) = self.0.get(candidate.path.as_bytes()) {
            matches.extend(hits);
        }
    }
}

#[derive(Clone, Debug)]
struct BasenameLiteralStrategy(fnv::HashMap<Vec<u8>, Vec<usize>>);

impl BasenameLiteralStrategy {
    fn new() -> BasenameLiteralStrategy {
        BasenameLiteralStrategy(fnv::HashMap::default())
    }

    fn add(&mut self, global_index: usize, lit: String) {
        self.0.entry(lit.into_bytes()).or_insert(vec![]).push(global_index);
    }

    fn is_match(&self, candidate: &Candidate<'_>) -> bool {
        if candidate.basename.is_empty() {
            return false;
        }
        self.0.contains_key(candidate.basename.as_bytes())
    }

    #[inline(never)]
    fn matches_into(
        &self,
        candidate: &Candidate<'_>,
        matches: &mut Vec<usize>,
    ) {
        if candidate.basename.is_empty() {
            return;
        }
        if let Some(hits) = self.0.get(candidate.basename.as_bytes()) {
            matches.extend(hits);
        }
    }
}

#[derive(Clone, Debug)]
struct ExtensionStrategy(fnv::HashMap<Vec<u8>, Vec<usize>>);

impl ExtensionStrategy {
    fn new() -> ExtensionStrategy {
        ExtensionStrategy(fnv::HashMap::default())
    }

    fn add(&mut self, global_index: usize, ext: String) {
        self.0.entry(ext.into_bytes()).or_insert(vec![]).push(global_index);
    }

    fn is_match(&self, candidate: &Candidate<'_>) -> bool {
        if candidate.ext.is_empty() {
            return false;
        }
        self.0.contains_key(candidate.ext.as_bytes())
    }

    #[inline(never)]
    fn matches_into(
        &self,
        candidate: &Candidate<'_>,
        matches: &mut Vec<usize>,
    ) {
        if candidate.ext.is_empty() {
            return;
        }
        if let Some(hits) = self.0.get(candidate.ext.as_bytes()) {
            matches.extend(hits);
        }
    }
}

#[derive(Clone, Debug)]
struct PrefixStrategy {
    matcher: AhoCorasick,
    map: Vec<usize>,
    longest: usize,
}

impl PrefixStrategy {
    fn is_match(&self, candidate: &Candidate<'_>) -> bool {
        let path = candidate.path_prefix(self.longest);
        for m in self.matcher.find_overlapping_iter(path) {
            if m.start() == 0 {
                return true;
            }
        }
        false
    }

    fn matches_into(
        &self,
        candidate: &Candidate<'_>,
        matches: &mut Vec<usize>,
    ) {
        let path = candidate.path_prefix(self.longest);
        for m in self.matcher.find_overlapping_iter(path) {
            if m.start() == 0 {
                matches.push(self.map[m.pattern()]);
            }
        }
    }
}

#[derive(Clone, Debug)]
struct SuffixStrategy {
    matcher: AhoCorasick,
    map: Vec<usize>,
    longest: usize,
}

impl SuffixStrategy {
    fn is_match(&self, candidate: &Candidate<'_>) -> bool {
        let path = candidate.path_suffix(self.longest);
        for m in self.matcher.find_overlapping_iter(path) {
            if m.end() == path.len() {
                return true;
            }
        }
        false
    }

    fn matches_into(
        &self,
        candidate: &Candidate<'_>,
        matches: &mut Vec<usize>,
    ) {
        let path = candidate.path_suffix(self.longest);
        for m in self.matcher.find_overlapping_iter(path) {
            if m.end() == path.len() {
                matches.push(self.map[m.pattern()]);
            }
        }
    }
}

#[derive(Clone, Debug)]
struct RequiredExtensionStrategy(fnv::HashMap<Vec<u8>, Vec<(usize, Regex)>>);

impl RequiredExtensionStrategy {
    fn is_match(&self, candidate: &Candidate<'_>) -> bool {
        if candidate.ext.is_empty() {
            return false;
        }
        match self.0.get(candidate.ext.as_bytes()) {
            None => false,
            Some(regexes) => {
                for &(_, ref re) in regexes {
                    if re.is_match(candidate.path.as_bytes()) {
                        return true;
                    }
                }
                false
            }
        }
    }

    #[inline(never)]
    fn matches_into(
        &self,
        candidate: &Candidate<'_>,
        matches: &mut Vec<usize>,
    ) {
        if candidate.ext.is_empty() {
            return;
        }
        if let Some(regexes) = self.0.get(candidate.ext.as_bytes()) {
            for &(global_index, ref re) in regexes {
                if re.is_match(candidate.path.as_bytes()) {
                    matches.push(global_index);
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
struct RegexSetStrategy {
    matcher: Regex,
    map: Vec<usize>,
    // Мы используем пул PatternSets, чтобы, надеюсь, выделять новый для каждого
    // вызова.
    //
    // TODO: В следующем семантически ломающем релизе мы должны убрать этот пул и
    // предоставить непрозрачный тип, который обёртывает PatternSet. Тогда вызывающие
    // стороны смогут предоставить его в `matches_into` напрямую. Вызывающие стороны всё ещё
    // могут захотеть использовать пул или подобное для амортизации выделения, но это
    // соответствует статус-кво и избавляет нас от необходимости делать это здесь.
    patset: Arc<Pool<PatternSet, PatternSetPoolFn>>,
}

type PatternSetPoolFn =
    Box<dyn Fn() -> PatternSet + Send + Sync + UnwindSafe + RefUnwindSafe>;

impl RegexSetStrategy {
    fn is_match(&self, candidate: &Candidate<'_>) -> bool {
        self.matcher.is_match(candidate.path.as_bytes())
    }

    fn matches_into(
        &self,
        candidate: &Candidate<'_>,
        matches: &mut Vec<usize>,
    ) {
        let input = regex_automata::Input::new(candidate.path.as_bytes());
        let mut patset = self.patset.get();
        patset.clear();
        self.matcher.which_overlapping_matches(&input, &mut patset);
        for i in patset.iter() {
            matches.push(self.map[i]);
        }
        PoolGuard::put(patset);
    }
}

#[derive(Clone, Debug)]
struct MultiStrategyBuilder {
    literals: Vec<String>,
    map: Vec<usize>,
    longest: usize,
}

impl MultiStrategyBuilder {
    fn new() -> MultiStrategyBuilder {
        MultiStrategyBuilder { literals: vec![], map: vec![], longest: 0 }
    }

    fn add(&mut self, global_index: usize, literal: String) {
        if literal.len() > self.longest {
            self.longest = literal.len();
        }
        self.map.push(global_index);
        self.literals.push(literal);
    }

    fn prefix(self) -> PrefixStrategy {
        PrefixStrategy {
            matcher: AhoCorasick::new(&self.literals).unwrap(),
            map: self.map,
            longest: self.longest,
        }
    }

    fn suffix(self) -> SuffixStrategy {
        SuffixStrategy {
            matcher: AhoCorasick::new(&self.literals).unwrap(),
            map: self.map,
            longest: self.longest,
        }
    }

    fn regex_set(self) -> Result<RegexSetStrategy, Error> {
        let matcher = new_regex_set(self.literals)?;
        let pattern_len = matcher.pattern_len();
        let create: PatternSetPoolFn =
            Box::new(move || PatternSet::new(pattern_len));
        Ok(RegexSetStrategy {
            matcher,
            map: self.map,
            patset: Arc::new(Pool::new(create)),
        })
    }

    fn is_empty(&self) -> bool {
        self.literals.is_empty()
    }
}

#[derive(Clone, Debug)]
struct RequiredExtensionStrategyBuilder(
    fnv::HashMap<Vec<u8>, Vec<(usize, String)>>,
);

impl RequiredExtensionStrategyBuilder {
    fn new() -> RequiredExtensionStrategyBuilder {
        RequiredExtensionStrategyBuilder(fnv::HashMap::default())
    }

    fn add(&mut self, global_index: usize, ext: String, regex: String) {
        self.0
            .entry(ext.into_bytes())
            .or_insert(vec![])
            .push((global_index, regex));
    }

    fn build(self) -> Result<RequiredExtensionStrategy, Error> {
        let mut exts = fnv::HashMap::default();
        for (ext, regexes) in self.0.into_iter() {
            exts.insert(ext.clone(), vec![]);
            for (global_index, regex) in regexes {
                let compiled = new_regex(&regex)?;
                exts.get_mut(&ext).unwrap().push((global_index, compiled));
            }
        }
        Ok(RequiredExtensionStrategy(exts))
    }
}

/// Экранирует мета-символы в данном шаблоне glob.
///
/// Экранирование работает путём окружения мета-символов квадратными скобками.
/// Например, `*` становится `[*]`.
///
/// # Пример
///
/// ```
/// use globset::escape;
///
/// assert_eq!(escape("foo*bar"), "foo[*]bar");
/// assert_eq!(escape("foo?bar"), "foo[?]bar");
/// assert_eq!(escape("foo[bar"), "foo[[]bar");
/// assert_eq!(escape("foo]bar"), "foo[]]bar");
/// assert_eq!(escape("foo{bar"), "foo[{]bar");
/// assert_eq!(escape("foo}bar"), "foo[}]bar");
/// ```
pub fn escape(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            // обратите внимание, что ! не нуждается в экранировании, потому что он special
            // только внутри скобок
            '?' | '*' | '[' | ']' | '{' | '}' => {
                escaped.push('[');
                escaped.push(c);
                escaped.push(']');
            }
            c => {
                escaped.push(c);
            }
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use crate::glob::Glob;

    use super::{GlobSet, GlobSetBuilder};

    #[test]
    fn set_works() {
        let mut builder = GlobSetBuilder::new();
        builder.add(Glob::new("src/**/*.rs").unwrap());
        builder.add(Glob::new("*.c").unwrap());
        builder.add(Glob::new("src/lib.rs").unwrap());
        let set = builder.build().unwrap();

        assert!(set.is_match("foo.c"));
        assert!(set.is_match("src/foo.c"));
        assert!(!set.is_match("foo.rs"));
        assert!(!set.is_match("tests/foo.rs"));
        assert!(set.is_match("src/foo.rs"));
        assert!(set.is_match("src/grep/src/main.rs"));

        let matches = set.matches("src/lib.rs");
        assert_eq!(2, matches.len());
        assert_eq!(0, matches[0]);
        assert_eq!(2, matches[1]);
    }

    #[test]
    fn empty_set_works() {
        let set = GlobSetBuilder::new().build().unwrap();
        assert!(!set.is_match(""));
        assert!(!set.is_match("a"));
        assert!(set.matches_all("a"));
    }

    #[test]
    fn default_set_is_empty_works() {
        let set: GlobSet = Default::default();
        assert!(!set.is_match(""));
        assert!(!set.is_match("a"));
    }

    #[test]
    fn escape() {
        use super::escape;
        assert_eq!("foo", escape("foo"));
        assert_eq!("foo[*]", escape("foo*"));
        assert_eq!("[[][]]", escape("[]"));
        assert_eq!("[*][?]", escape("*?"));
        assert_eq!("src/[*][*]/[*].rs", escape("src/**/*.rs"));
        assert_eq!("bar[[]ab[]]baz", escape("bar[ab]baz"));
        assert_eq!("bar[[]!![]]!baz", escape("bar[!!]!baz"));
    }

    // This tests that regex matching doesn't "remember" the results of
    // previous searches. That is, if any memory is reused from a previous
    // search, then it should be cleared first.
    #[test]
    fn set_does_not_remember() {
        let mut builder = GlobSetBuilder::new();
        builder.add(Glob::new("*foo*").unwrap());
        builder.add(Glob::new("*bar*").unwrap());
        builder.add(Glob::new("*quux*").unwrap());
        let set = builder.build().unwrap();

        let matches = set.matches("ZfooZquuxZ");
        assert_eq!(2, matches.len());
        assert_eq!(0, matches[0]);
        assert_eq!(2, matches[1]);

        let matches = set.matches("nada");
        assert_eq!(0, matches.len());
    }

    #[test]
    fn debug() {
        let mut builder = GlobSetBuilder::new();
        builder.add(Glob::new("*foo*").unwrap());
        builder.add(Glob::new("*bar*").unwrap());
        builder.add(Glob::new("*quux*").unwrap());
        assert_eq!(
            format!("{builder:?}"),
            "GlobSetBuilder { pats: [Glob(\"*foo*\"), Glob(\"*bar*\"), Glob(\"*quux*\")] }",
        );
    }
}
