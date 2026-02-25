/*!
Крейт ignore предоставляет быстрый рекурсивный итератор по директориям,
который уважает различные фильтры, такие как glob, типы файлов и файлы
`.gitignore`. Точное соответствие правилам и приоритетам объясняется в
документации к `WalkBuilder`.

Во вторую очередь, этот крейт экспонирует матчеры gitignore и типов файлов
для вариантов использования, требующих более тонкого контроля.

# Пример

Этот пример показывает наиболее базовое использование этого крейта. Этот
код будет рекурсивно обходить текущую директорию, автоматически фильтруя
файлы и директории согласно ignore glob, найденным в файлах вроде
`.ignore` и `.gitignore`:


```rust,no_run
use ignore::Walk;

for result in Walk::new("./") {
    // Каждый элемент, выдаваемый итератором, является либо записью о
    // директории, либо ошибкой, поэтому либо печатаем путь, либо ошибку.
    match result {
        Ok(entry) => println!("{}", entry.path().display()),
        Err(err) => println!("ERROR: {}", err),
    }
}
```

# Пример: расширенный

По умолчанию итератор рекурсивного обхода директорий будет игнорировать
скрытые файлы и директории. Это можно отключить, построив итератор с
помощью `WalkBuilder`:

```rust,no_run
use ignore::WalkBuilder;

for result in WalkBuilder::new("./").hidden(false).build() {
    println!("{:?}", result);
}
```

Смотрите документацию для `WalkBuilder` для получения многих других опций.
*/

#![deny(missing_docs)]

use std::path::{Path, PathBuf};

pub use crate::walk::{
    DirEntry, ParallelVisitor, ParallelVisitorBuilder, Walk, WalkBuilder,
    WalkParallel, WalkState,
};

mod default_types;
mod dir;
pub mod gitignore;
pub mod overrides;
mod pathutil;
pub mod types;
mod walk;

/// Представляет ошибку, которая может возникнуть при разборе файла gitignore.
#[derive(Debug)]
pub enum Error {
    /// Коллекция «мягких» ошибок. Они возникают, когда добавление файла
    /// ignore частично удалось.
    Partial(Vec<Error>),
    /// Ошибка, связанная с конкретным номером строки.
    WithLineNumber {
        /// Номер строки.
        line: u64,
        /// Основная ошибка.
        err: Box<Error>,
    },
    /// Ошибка, связанная с конкретным путём к файлу.
    WithPath {
        /// Путь к файлу.
        path: PathBuf,
        /// Основная ошибка.
        err: Box<Error>,
    },
    /// Ошибка, связанная с конкретной глубиной директории при рекурсивном
    /// обходе директории.
    WithDepth {
        /// Глубина директории.
        depth: usize,
        /// Основная ошибка.
        err: Box<Error>,
    },
    /// Ошибка, возникающая при обнаружении цикла файлов при обходе
    /// символических ссылок.
    Loop {
        /// Путь к файлу предка в цикле.
        ancestor: PathBuf,
        /// Путь к дочернему файлу в цикле.
        child: PathBuf,
    },
    /// Ошибка, возникающая при выполнении I/O, например, при чтении файла ignore.
    Io(std::io::Error),
    /// Ошибка, возникающая при попытке разбора glob.
    Glob {
        /// Исходный glob, вызвавший эту ошибку. Этот glob, когда доступен,
        /// всегда соответствует glob, предоставленному конечным пользователем.
        /// Например, это glob, как написано в файле `.gitignore`.
        ///
        /// (Этот glob может отличаться от glob, который фактически
        /// компилируется, после учёта семантики `gitignore`.)
        glob: Option<String>,
        /// Основная ошибка glob в виде строки.
        err: String,
    },
    /// Выбор типа для типа файла, который не определён.
    UnrecognizedFileType(String),
    /// Определённый пользователем тип файла не может быть разобран.
    InvalidDefinition,
}

impl Clone for Error {
    fn clone(&self) -> Error {
        match *self {
            Error::Partial(ref errs) => Error::Partial(errs.clone()),
            Error::WithLineNumber { line, ref err } => {
                Error::WithLineNumber { line, err: err.clone() }
            }
            Error::WithPath { ref path, ref err } => {
                Error::WithPath { path: path.clone(), err: err.clone() }
            }
            Error::WithDepth { depth, ref err } => {
                Error::WithDepth { depth, err: err.clone() }
            }
            Error::Loop { ref ancestor, ref child } => Error::Loop {
                ancestor: ancestor.clone(),
                child: child.clone(),
            },
            Error::Io(ref err) => match err.raw_os_error() {
                Some(e) => Error::Io(std::io::Error::from_raw_os_error(e)),
                None => {
                    Error::Io(std::io::Error::new(err.kind(), err.to_string()))
                }
            },
            Error::Glob { ref glob, ref err } => {
                Error::Glob { glob: glob.clone(), err: err.clone() }
            }
            Error::UnrecognizedFileType(ref err) => {
                Error::UnrecognizedFileType(err.clone())
            }
            Error::InvalidDefinition => Error::InvalidDefinition,
        }
    }
}

impl Error {
    /// Возвращает true, если это частичная ошибка.
    ///
    /// Частичная ошибка возникает, когда только некоторые операции не
    /// удались, в то время как другие могли succeed. Например, файл
    /// ignore может содержать недопустимый glob среди в остальном
    /// допустимых glob.
    pub fn is_partial(&self) -> bool {
        match *self {
            Error::Partial(_) => true,
            Error::WithLineNumber { ref err, .. } => err.is_partial(),
            Error::WithPath { ref err, .. } => err.is_partial(),
            Error::WithDepth { ref err, .. } => err.is_partial(),
            _ => false,
        }
    }

    /// Возвращает true, если эта ошибка является исключительно ошибкой I/O.
    pub fn is_io(&self) -> bool {
        match *self {
            Error::Partial(ref errs) => errs.len() == 1 && errs[0].is_io(),
            Error::WithLineNumber { ref err, .. } => err.is_io(),
            Error::WithPath { ref err, .. } => err.is_io(),
            Error::WithDepth { ref err, .. } => err.is_io(),
            Error::Loop { .. } => false,
            Error::Io(_) => true,
            Error::Glob { .. } => false,
            Error::UnrecognizedFileType(_) => false,
            Error::InvalidDefinition => false,
        }
    }

    /// Проверяет исходную [`std::io::Error`], если она существует.
    ///
    /// [`None`] возвращается, если [`Error`] не соответствует
    /// [`std::io::Error`]. Это может произойти, например, когда ошибка
    /// была вызвана тем, что в дереве директорий был найден цикл при
    /// следовании символическим ссылкам.
    ///
    /// Этот метод возвращает заиммованное значение, время жизни которого
    /// ограничено временем жизни [`Error`]. Для получения владеющего
    /// значения можно использовать [`into_io_error`].
    ///
    /// > Это оригинальная [`std::io::Error`] и _не_ то же самое, что
    /// > [`impl From<Error> for std::io::Error`][impl], который содержит
    /// > дополнительную информацию об ошибке.
    ///
    /// [`None`]: https://doc.rust-lang.org/stable/std/option/enum.Option.html#variant.None
    /// [`std::io::Error`]: https://doc.rust-lang.org/stable/std/io/struct.Error.html
    /// [`From`]: https://doc.rust-lang.org/stable/std/convert/trait.From.html
    /// [`Error`]: struct.Error.html
    /// [`into_io_error`]: struct.Error.html#method.into_io_error
    /// [impl]: struct.Error.html#impl-From%3CError%3E
    pub fn io_error(&self) -> Option<&std::io::Error> {
        match *self {
            Error::Partial(ref errs) => {
                if errs.len() == 1 {
                    errs[0].io_error()
                } else {
                    None
                }
            }
            Error::WithLineNumber { ref err, .. } => err.io_error(),
            Error::WithPath { ref err, .. } => err.io_error(),
            Error::WithDepth { ref err, .. } => err.io_error(),
            Error::Loop { .. } => None,
            Error::Io(ref err) => Some(err),
            Error::Glob { .. } => None,
            Error::UnrecognizedFileType(_) => None,
            Error::InvalidDefinition => None,
        }
    }

    /// Аналогично [`io_error`], но потребляет self для преобразования
    /// в исходную [`std::io::Error`], если она существует.
    ///
    /// [`io_error`]: struct.Error.html#method.io_error
    /// [`std::io::Error`]: https://doc.rust-lang.org/stable/std/io/struct.Error.html
    pub fn into_io_error(self) -> Option<std::io::Error> {
        match self {
            Error::Partial(mut errs) => {
                if errs.len() == 1 {
                    errs.remove(0).into_io_error()
                } else {
                    None
                }
            }
            Error::WithLineNumber { err, .. } => err.into_io_error(),
            Error::WithPath { err, .. } => err.into_io_error(),
            Error::WithDepth { err, .. } => err.into_io_error(),
            Error::Loop { .. } => None,
            Error::Io(err) => Some(err),
            Error::Glob { .. } => None,
            Error::UnrecognizedFileType(_) => None,
            Error::InvalidDefinition => None,
        }
    }

    /// Возвращает глубину, связанную с рекурсивным обходом директории
    /// (если эта ошибка была сгенерирована рекурсивным итератором директорий).
    pub fn depth(&self) -> Option<usize> {
        match *self {
            Error::WithPath { ref err, .. } => err.depth(),
            Error::WithDepth { depth, .. } => Some(depth),
            _ => None,
        }
    }

    /// Превращает ошибку в помеченную ошибку с данным путём к файлу.
    fn with_path<P: AsRef<Path>>(self, path: P) -> Error {
        Error::WithPath {
            path: path.as_ref().to_path_buf(),
            err: Box::new(self),
        }
    }

    /// Превращает ошибку в помеченную ошибку с данной глубиной.
    fn with_depth(self, depth: usize) -> Error {
        Error::WithDepth { depth, err: Box::new(self) }
    }

    /// Превращает ошибку в помеченную ошибку с данным путём к файлу и
    /// номером строки. Если путь пуст, то он опускается из ошибки.
    fn tagged<P: AsRef<Path>>(self, path: P, lineno: u64) -> Error {
        let errline =
            Error::WithLineNumber { line: lineno, err: Box::new(self) };
        if path.as_ref().as_os_str().is_empty() {
            return errline;
        }
        errline.with_path(path)
    }

    /// Строит ошибку из ошибки walkdir.
    fn from_walkdir(err: walkdir::Error) -> Error {
        let depth = err.depth();
        if let (Some(anc), Some(child)) = (err.loop_ancestor(), err.path()) {
            return Error::WithDepth {
                depth,
                err: Box::new(Error::Loop {
                    ancestor: anc.to_path_buf(),
                    child: child.to_path_buf(),
                }),
            };
        }
        let path = err.path().map(|p| p.to_path_buf());
        let mut ig_err = Error::Io(std::io::Error::from(err));
        if let Some(path) = path {
            ig_err = Error::WithPath { path, err: Box::new(ig_err) };
        }
        ig_err
    }
}

impl std::error::Error for Error {
    #[allow(deprecated)]
    fn description(&self) -> &str {
        match *self {
            Error::Partial(_) => "partial error",
            Error::WithLineNumber { ref err, .. } => err.description(),
            Error::WithPath { ref err, .. } => err.description(),
            Error::WithDepth { ref err, .. } => err.description(),
            Error::Loop { .. } => "file system loop found",
            Error::Io(ref err) => err.description(),
            Error::Glob { ref err, .. } => err,
            Error::UnrecognizedFileType(_) => "unrecognized file type",
            Error::InvalidDefinition => "invalid definition",
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Error::Partial(ref errs) => {
                let msgs: Vec<String> =
                    errs.iter().map(|err| err.to_string()).collect();
                write!(f, "{}", msgs.join("\n"))
            }
            Error::WithLineNumber { line, ref err } => {
                write!(f, "line {}: {}", line, err)
            }
            Error::WithPath { ref path, ref err } => {
                write!(f, "{}: {}", path.display(), err)
            }
            Error::WithDepth { ref err, .. } => err.fmt(f),
            Error::Loop { ref ancestor, ref child } => write!(
                f,
                "File system loop found: \
                           {} points to an ancestor {}",
                child.display(),
                ancestor.display()
            ),
            Error::Io(ref err) => err.fmt(f),
            Error::Glob { glob: None, ref err } => write!(f, "{}", err),
            Error::Glob { glob: Some(ref glob), ref err } => {
                write!(f, "error parsing glob '{}': {}", glob, err)
            }
            Error::UnrecognizedFileType(ref ty) => {
                write!(f, "unrecognized file type: {}", ty)
            }
            Error::InvalidDefinition => write!(
                f,
                "invalid definition (format is type:glob, e.g., \
                           html:*.html)"
            ),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Error {
        Error::Io(err)
    }
}

#[derive(Debug, Default)]
struct PartialErrorBuilder(Vec<Error>);

impl PartialErrorBuilder {
    fn push(&mut self, err: Error) {
        self.0.push(err);
    }

    fn push_ignore_io(&mut self, err: Error) {
        if !err.is_io() {
            self.push(err);
        }
    }

    fn maybe_push(&mut self, err: Option<Error>) {
        if let Some(err) = err {
            self.push(err);
        }
    }

    fn maybe_push_ignore_io(&mut self, err: Option<Error>) {
        if let Some(err) = err {
            self.push_ignore_io(err);
        }
    }

    fn into_error_option(mut self) -> Option<Error> {
        if self.0.is_empty() {
            None
        } else if self.0.len() == 1 {
            Some(self.0.pop().unwrap())
        } else {
            Some(Error::Partial(self.0))
        }
    }
}

/// Результат сопоставления glob.
///
/// Параметр типа `T` обычно относится к типу, который предоставляет
/// дополнительную информацию о конкретном совпадении. Например, он может
/// идентифицировать конкретный файл gitignore и конкретный шаблон glob,
/// который вызвал совпадение.
#[derive(Clone, Debug)]
pub enum Match<T> {
    /// The path didn't match any glob.
    None,
    /// The highest precedent glob matched indicates the path should be
    /// ignored.
    Ignore(T),
    /// The highest precedent glob matched indicates the path should be
    /// whitelisted.
    Whitelist(T),
}

impl<T> Match<T> {
    /// Возвращает true, если результат сопоставления не соответствует ни одному glob.
    pub fn is_none(&self) -> bool {
        match *self {
            Match::None => true,
            Match::Ignore(_) | Match::Whitelist(_) => false,
        }
    }

    /// Возвращает true, если результат сопоставления подразумевает, что путь
    /// должен быть проигнорирован.
    pub fn is_ignore(&self) -> bool {
        match *self {
            Match::Ignore(_) => true,
            Match::None | Match::Whitelist(_) => false,
        }
    }

    /// Возвращает true, если результат сопоставления подразумевает, что путь
    /// должен быть внесён в белый список.
    pub fn is_whitelist(&self) -> bool {
        match *self {
            Match::Whitelist(_) => true,
            Match::None | Match::Ignore(_) => false,
        }
    }

    /// Инвертирует совпадение так, что `Ignore` становится `Whitelist`, а
    /// `Whitelist` становится `Ignore`. Несовпадение остаётся тем же.
    pub fn invert(self) -> Match<T> {
        match self {
            Match::None => Match::None,
            Match::Ignore(t) => Match::Whitelist(t),
            Match::Whitelist(t) => Match::Ignore(t),
        }
    }

    /// Возвращает значение внутри этого совпадения, если оно существует.
    pub fn inner(&self) -> Option<&T> {
        match *self {
            Match::None => None,
            Match::Ignore(ref t) => Some(t),
            Match::Whitelist(ref t) => Some(t),
        }
    }

    /// Применяет данную функцию к значению внутри этого совпадения.
    ///
    /// Если совпадение не имеет значения, то возвращает совпадение без изменений.
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Match<U> {
        match self {
            Match::None => Match::None,
            Match::Ignore(t) => Match::Ignore(f(t)),
            Match::Whitelist(t) => Match::Whitelist(f(t)),
        }
    }

    /// Возвращает совпадение, если оно не none. В противном случае возвращает other.
    pub fn or(self, other: Self) -> Self {
        if self.is_none() { other } else { self }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        path::{Path, PathBuf},
    };

    /// A convenient result type alias.
    pub(crate) type Result<T> =
        std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

    macro_rules! err {
        ($($tt:tt)*) => {
            Box::<dyn std::error::Error + Send + Sync>::from(format!($($tt)*))
        }
    }

    /// A simple wrapper for creating a temporary directory that is
    /// automatically deleted when it's dropped.
    ///
    /// We use this in lieu of tempfile because tempfile brings in too many
    /// dependencies.
    #[derive(Debug)]
    pub struct TempDir(PathBuf);

    impl Drop for TempDir {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    impl TempDir {
        /// Create a new empty temporary directory under the system's configured
        /// temporary directory.
        pub fn new() -> Result<TempDir> {
            use std::sync::atomic::{AtomicUsize, Ordering};

            static TRIES: usize = 100;
            static COUNTER: AtomicUsize = AtomicUsize::new(0);

            let tmpdir = env::temp_dir();
            for _ in 0..TRIES {
                let count = COUNTER.fetch_add(1, Ordering::Relaxed);
                let path = tmpdir.join("rust-ignore").join(count.to_string());
                if path.is_dir() {
                    continue;
                }
                fs::create_dir_all(&path).map_err(|e| {
                    err!("failed to create {}: {}", path.display(), e)
                })?;
                return Ok(TempDir(path));
            }
            Err(err!("failed to create temp dir after {} tries", TRIES))
        }

        /// Return the underlying path to this temporary directory.
        pub fn path(&self) -> &Path {
            &self.0
        }
    }
}
