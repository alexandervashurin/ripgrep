/*!
Модуль gitignore предоставляет возможность сопоставления glob из файла
gitignore с путями к файлам.

Обратите внимание, что этот модуль реализует спецификацию, как описано в
странице руководства `gitignore` с нуля. То есть этот модуль *не* вызывает
командную строку `git`.
*/

use std::{
    fs::File,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    sync::Arc,
};

use {
    globset::{Candidate, GlobBuilder, GlobSet, GlobSetBuilder},
    regex_automata::util::pool::Pool,
};

use crate::{
    Error, Match, PartialErrorBuilder,
    pathutil::{is_file_name, strip_prefix},
};

/// Glob представляет собой одиночный glob в файле gitignore.
///
/// Это используется для сообщения информации о glob с наивысшим приоритетом,
/// который совпал в одном или нескольких файлах gitignore.
#[derive(Clone, Debug)]
pub struct Glob {
    /// The file path that this glob was extracted from.
    from: Option<PathBuf>,
    /// The original glob string.
    original: String,
    /// The actual glob string used to convert to a regex.
    actual: String,
    /// Whether this is a whitelisted glob or not.
    is_whitelist: bool,
    /// Whether this glob should only match directories or not.
    is_only_dir: bool,
}

impl Glob {
    /// Возвращает путь к файлу, из которого был извлечён этот glob.
    pub fn from(&self) -> Option<&Path> {
        self.from.as_ref().map(|p| &**p)
    }

    /// Исходный glob, как он был определён в файле gitignore.
    pub fn original(&self) -> &str {
        &self.original
    }

    /// Фактический glob, который был скомпилирован с учётом семантики gitignore.
    pub fn actual(&self) -> &str {
        &self.actual
    }

    /// Был ли этот glob в белом списке или нет.
    pub fn is_whitelist(&self) -> bool {
        self.is_whitelist
    }

    /// Должен ли этот glob соответствовать директории или нет.
    pub fn is_only_dir(&self) -> bool {
        self.is_only_dir
    }

    /// Возвращает true тогда и только тогда, когда этот glob имеет префикс `**/`.
    fn has_doublestar_prefix(&self) -> bool {
        self.actual.starts_with("**/") || self.actual == "**"
    }
}

/// Gitignore — это matcher для glob в одном или нескольких файлах gitignore
/// в одной директории.
#[derive(Clone, Debug)]
pub struct Gitignore {
    set: GlobSet,
    root: PathBuf,
    globs: Vec<Glob>,
    num_ignores: u64,
    num_whitelists: u64,
    matches: Option<Arc<Pool<Vec<usize>>>>,
}

impl Gitignore {
    /// Создаёт новый matcher gitignore из данного пути к файлу gitignore.
    ///
    /// Если желательно включить несколько файлов gitignore в один matcher
    /// или читать glob gitignore из другого источника, то используйте
    /// `GitignoreBuilder`.
    ///
    /// Это всегда возвращает валидный matcher, даже если он пуст. В частности,
    /// файл Gitignore может быть частично валидным, например, когда один glob
    /// некорректен, а остальные нет.
    ///
    /// Обратите внимание, что ошибки I/O игнорируются. Для более гранулярного
    /// контроля над ошибками используйте `GitignoreBuilder`.
    pub fn new<P: AsRef<Path>>(
        gitignore_path: P,
    ) -> (Gitignore, Option<Error>) {
        let path = gitignore_path.as_ref();
        let parent = path.parent().unwrap_or(Path::new("/"));
        let mut builder = GitignoreBuilder::new(parent);
        let mut errs = PartialErrorBuilder::default();
        errs.maybe_push_ignore_io(builder.add(path));
        match builder.build() {
            Ok(gi) => (gi, errs.into_error_option()),
            Err(err) => {
                errs.push(err);
                (Gitignore::empty(), errs.into_error_option())
            }
        }
    }

    /// Создаёт новый matcher gitignore из глобального файла ignore, если он
    /// существует.
    ///
    /// Путь к файлу глобальной конфигурации указывается опцией конфигурации
    /// git `core.excludesFile`.
    ///
    /// Путь к файлу конфигурации Git — `$HOME/.gitconfig`. Если
    /// `$HOME/.gitconfig` не существует или не указывает `core.excludesFile`,
    /// то читается `$XDG_CONFIG_HOME/git/ignore`. Если `$XDG_CONFIG_HOME` не
    /// установлен или пуст, то вместо него используется `$HOME/.config/git/ignore`.
    pub fn global() -> (Gitignore, Option<Error>) {
        match std::env::current_dir() {
            Ok(cwd) => GitignoreBuilder::new(cwd).build_global(),
            Err(err) => (Gitignore::empty(), Some(err.into())),
        }
    }

    /// Создаёт новый пустой matcher gitignore, который никогда ничего не соответствует.
    ///
    /// Его путь пуст.
    pub fn empty() -> Gitignore {
        Gitignore {
            set: GlobSet::empty(),
            root: PathBuf::from(""),
            globs: vec![],
            num_ignores: 0,
            num_whitelists: 0,
            matches: None,
        }
    }

    /// Возвращает директорию, содержащую этот matcher gitignore.
    ///
    /// Все совпадения выполняются относительно этого пути.
    pub fn path(&self) -> &Path {
        &*self.root
    }

    /// Возвращает true тогда и только тогда, когда этот gitignore не имеет
    /// ни одного glob и, следовательно, никогда не соответствует ни одному
    /// пути к файлу.
    pub fn is_empty(&self) -> bool {
        self.set.is_empty()
    }

    /// Возвращает общее количество glob, что должно быть эквивалентно
    /// `num_ignores + num_whitelists`.
    pub fn len(&self) -> usize {
        self.set.len()
    }

    /// Возвращает общее количество ignore glob.
    pub fn num_ignores(&self) -> u64 {
        self.num_ignores
    }

    /// Возвращает общее количество glob в белом списке.
    pub fn num_whitelists(&self) -> u64 {
        self.num_whitelists
    }

    /// Возвращает, соответствует ли данный путь (файл или директория)
    /// шаблону в этом matcher gitignore.
    ///
    /// `is_dir` должно быть true, если путь относится к директории, и false
    /// в противном случае.
    ///
    /// Данный путь сопоставляется относительно пути, данного при построении
    /// matcher. В частности, перед сопоставлением `path` его префикс (как
    /// определяется общим суффиксом директории, содержащей этот gitignore)
    /// удаляется. Если нет общего перекрытия суффикса/префикса, то `path`
    /// предполагается относительным к этому matcher.
    pub fn matched<P: AsRef<Path>>(
        &self,
        path: P,
        is_dir: bool,
    ) -> Match<&Glob> {
        if self.is_empty() {
            return Match::None;
        }
        self.matched_stripped(self.strip(path.as_ref()), is_dir)
    }

    /// Возвращает, соответствует ли данный путь (файл или директория, и,
    /// как ожидается, находится под корневым путём) или любая из его
    /// родительских директорий (вплоть до корня) соответствует шаблону в
    /// этом matcher gitignore.
    ///
    /// ПРИМЕЧАНИЕ: Этот метод более затратен, чем обход иерархии директорий
    /// сверху вниз и сопоставление записей. Но его проще использовать в
    /// случаях, когда доступен список путей без иерархии.
    ///
    /// `is_dir` должно быть true, если путь относится к директории, и false
    /// в противном случае.
    ///
    /// Данный путь сопоставляется относительно пути, данного при построении
    /// matcher. В частности, перед сопоставлением `path` его префикс (как
    /// определяется общим суффиксом директории, содержащей этот gitignore)
    /// удаляется. Если нет общего перекрытия суффикса/префикса, то `path`
    /// предполагается относительным к этому matcher.
    ///
    /// # Паника
    ///
    /// Этот метод паникует, если данный путь к файлу не находится под
    /// корневым путём этого matcher.
    pub fn matched_path_or_any_parents<P: AsRef<Path>>(
        &self,
        path: P,
        is_dir: bool,
    ) -> Match<&Glob> {
        if self.is_empty() {
            return Match::None;
        }
        let mut path = self.strip(path.as_ref());
        assert!(!path.has_root(), "path is expected to be under the root");

        match self.matched_stripped(path, is_dir) {
            Match::None => (), // walk up
            a_match => return a_match,
        }
        while let Some(parent) = path.parent() {
            match self.matched_stripped(parent, /* is_dir */ true) {
                Match::None => path = parent, // walk up
                a_match => return a_match,
            }
        }
        Match::None
    }

    /// Как matched, но принимает путь, который уже был очищен.
    fn matched_stripped<P: AsRef<Path>>(
        &self,
        path: P,
        is_dir: bool,
    ) -> Match<&Glob> {
        if self.is_empty() {
            return Match::None;
        }
        let path = path.as_ref();
        let mut matches = self.matches.as_ref().unwrap().get();
        let candidate = Candidate::new(path);
        self.set.matches_candidate_into(&candidate, &mut *matches);
        for &i in matches.iter().rev() {
            let glob = &self.globs[i];
            if !glob.is_only_dir() || is_dir {
                return if glob.is_whitelist() {
                    Match::Whitelist(glob)
                } else {
                    Match::Ignore(glob)
                };
            }
        }
        Match::None
    }

    /// Очищает данный путь так, чтобы он подходил для сопоставления с этим
    /// matcher gitignore.
    fn strip<'a, P: 'a + AsRef<Path> + ?Sized>(
        &'a self,
        path: &'a P,
    ) -> &'a Path {
        let mut path = path.as_ref();
        // A leading ./ is completely superfluous. We also strip it from
        // our gitignore root path, so we need to strip it from our candidate
        // path too.
        if let Some(p) = strip_prefix("./", path) {
            path = p;
        }
        // Strip any common prefix between the candidate path and the root
        // of the gitignore, to make sure we get relative matching right.
        // BUT, a file name might not have any directory components to it,
        // in which case, we don't want to accidentally strip any part of the
        // file name.
        //
        // As an additional special case, if the root is just `.`, then we
        // shouldn't try to strip anything, e.g., when path begins with a `.`.
        if self.root != Path::new(".") && !is_file_name(path) {
            if let Some(p) = strip_prefix(&self.root, path) {
                path = p;
                // If we're left with a leading slash, get rid of it.
                if let Some(p) = strip_prefix("/", path) {
                    path = p;
                }
            }
        }
        path
    }
}

/// Строит matcher для одного набора glob из файла .gitignore.
#[derive(Clone, Debug)]
pub struct GitignoreBuilder {
    builder: GlobSetBuilder,
    root: PathBuf,
    globs: Vec<Glob>,
    case_insensitive: bool,
    allow_unclosed_class: bool,
}

impl GitignoreBuilder {
    /// Создаёт новый построитель для файла gitignore.
    ///
    /// Данный путь должен быть путём, относительно которого должны
    /// сопоставляться glob для этого файла gitignore. Обратите внимание,
    /// что пути всегда сопоставляются относительно данного корневого пути.
    /// Обычно корневой путь должен соответствовать *директории*, содержащей
    /// файл `.gitignore`.
    pub fn new<P: AsRef<Path>>(root: P) -> GitignoreBuilder {
        let root = root.as_ref();
        GitignoreBuilder {
            builder: GlobSetBuilder::new(),
            root: strip_prefix("./", root).unwrap_or(root).to_path_buf(),
            globs: vec![],
            case_insensitive: false,
            allow_unclosed_class: true,
        }
    }

    /// Строит новый matcher из glob, добавленных на данный момент.
    ///
    /// Как только matcher построен, в него нельзя добавить новые glob.
    pub fn build(&self) -> Result<Gitignore, Error> {
        let nignore = self.globs.iter().filter(|g| !g.is_whitelist()).count();
        let nwhite = self.globs.iter().filter(|g| g.is_whitelist()).count();
        let set = self
            .builder
            .build()
            .map_err(|err| Error::Glob { glob: None, err: err.to_string() })?;
        Ok(Gitignore {
            set,
            root: self.root.clone(),
            globs: self.globs.clone(),
            num_ignores: nignore as u64,
            num_whitelists: nwhite as u64,
            matches: Some(Arc::new(Pool::new(|| vec![]))),
        })
    }

    /// Строит matcher глобального gitignore, используя конфигурацию в этом
    /// построителе.
    ///
    /// Это потребляет владение построителем, в отличие от `build`, потому что
    /// должно мутировать построитель для добавления glob глобального gitignore.
    ///
    /// Обратите внимание, что это игнорирует путь, данный конструктору этого
    /// построителя, и вместо этого автоматически получает путь из глобальной
    /// конфигурации git.
    pub fn build_global(mut self) -> (Gitignore, Option<Error>) {
        match gitconfig_excludes_path() {
            None => (Gitignore::empty(), None),
            Some(path) => {
                if !path.is_file() {
                    (Gitignore::empty(), None)
                } else {
                    let mut errs = PartialErrorBuilder::default();
                    errs.maybe_push_ignore_io(self.add(path));
                    match self.build() {
                        Ok(gi) => (gi, errs.into_error_option()),
                        Err(err) => {
                            errs.push(err);
                            (Gitignore::empty(), errs.into_error_option())
                        }
                    }
                }
            }
        }
    }

    /// Добавляет каждый glob из данного пути к файлу.
    ///
    /// Данный файл должен быть отформатирован как файл `gitignore`.
    ///
    /// Обратите внимание, что могут быть возвращены частичные ошибки.
    /// Например, если возникла проблема с добавлением одного glob, будет
    /// возвращена ошибка для него, но все остальные валидные glob всё
    /// равно будут добавлены.
    pub fn add<P: AsRef<Path>>(&mut self, path: P) -> Option<Error> {
        let path = path.as_ref();
        let file = match File::open(path) {
            Err(err) => return Some(Error::Io(err).with_path(path)),
            Ok(file) => file,
        };
        log::debug!("opened gitignore file: {}", path.display());
        let rdr = BufReader::new(file);
        let mut errs = PartialErrorBuilder::default();
        for (i, line) in rdr.lines().enumerate() {
            let lineno = (i + 1) as u64;
            let line = match line {
                Ok(line) => line,
                Err(err) => {
                    errs.push(Error::Io(err).tagged(path, lineno));
                    break;
                }
            };

            // Match Git's handling of .gitignore files that begin with the Unicode BOM
            const UTF8_BOM: &str = "\u{feff}";
            let line =
                if i == 0 { line.trim_start_matches(UTF8_BOM) } else { &line };

            if let Err(err) = self.add_line(Some(path.to_path_buf()), &line) {
                errs.push(err.tagged(path, lineno));
            }
        }
        errs.into_error_option()
    }

    /// Добавляет каждую строку glob из данной строки.
    ///
    /// Если эта строка получена из конкретного файла `gitignore`, то его
    /// путь должен быть предоставлен здесь.
    ///
    /// Данная строка должна быть отформатирована как файл `gitignore`.
    #[cfg(test)]
    fn add_str(
        &mut self,
        from: Option<PathBuf>,
        gitignore: &str,
    ) -> Result<&mut GitignoreBuilder, Error> {
        for line in gitignore.lines() {
            self.add_line(from.clone(), line)?;
        }
        Ok(self)
    }

    /// Добавляет строку из файла gitignore в этот построитель.
    ///
    /// Если эта строка получена из конкретного файла `gitignore`, то его
    /// путь должен быть предоставлен здесь.
    ///
    /// Если строка не может быть разобрана как glob, то возвращается ошибка.
    pub fn add_line(
        &mut self,
        from: Option<PathBuf>,
        mut line: &str,
    ) -> Result<&mut GitignoreBuilder, Error> {
        #![allow(deprecated)]

        if line.starts_with("#") {
            return Ok(self);
        }
        if !line.ends_with("\\ ") {
            line = line.trim_right();
        }
        if line.is_empty() {
            return Ok(self);
        }
        let mut glob = Glob {
            from,
            original: line.to_string(),
            actual: String::new(),
            is_whitelist: false,
            is_only_dir: false,
        };
        let mut is_absolute = false;
        if line.starts_with("\\!") || line.starts_with("\\#") {
            line = &line[1..];
            is_absolute = line.chars().nth(0) == Some('/');
        } else {
            if line.starts_with("!") {
                glob.is_whitelist = true;
                line = &line[1..];
            }
            if line.starts_with("/") {
                // `man gitignore` says that if a glob starts with a slash,
                // then the glob can only match the beginning of a path
                // (relative to the location of gitignore). We achieve this by
                // simply banning wildcards from matching /.
                line = &line[1..];
                is_absolute = true;
            }
        }
        // If it ends with a slash, then this should only match directories,
        // but the slash should otherwise not be used while globbing.
        if line.as_bytes().last() == Some(&b'/') {
            glob.is_only_dir = true;
            line = &line[..line.len() - 1];
            // If the slash was escaped, then remove the escape.
            // See: https://github.com/BurntSushi/ripgrep/issues/2236
            if line.as_bytes().last() == Some(&b'\\') {
                line = &line[..line.len() - 1];
            }
        }
        glob.actual = line.to_string();
        // If there is a literal slash, then this is a glob that must match the
        // entire path name. Otherwise, we should let it match anywhere, so use
        // a **/ prefix.
        if !is_absolute && !line.chars().any(|c| c == '/') {
            // ... but only if we don't already have a **/ prefix.
            if !glob.has_doublestar_prefix() {
                glob.actual = format!("**/{}", glob.actual);
            }
        }
        // If the glob ends with `/**`, then we should only match everything
        // inside a directory, but not the directory itself. Standard globs
        // will match the directory. So we add `/*` to force the issue.
        if glob.actual.ends_with("/**") {
            glob.actual = format!("{}/*", glob.actual);
        }
        let parsed = GlobBuilder::new(&glob.actual)
            .literal_separator(true)
            .case_insensitive(self.case_insensitive)
            .backslash_escape(true)
            .allow_unclosed_class(self.allow_unclosed_class)
            .build()
            .map_err(|err| Error::Glob {
                glob: Some(glob.original.clone()),
                err: err.kind().to_string(),
            })?;
        self.builder.add(parsed);
        self.globs.push(glob);
        Ok(self)
    }

    /// Переключает, должны ли glob сопоставляться регистронезависимо или нет.
    ///
    /// Когда эта опция изменена, затронуты будут только glob, добавленные
    /// после изменения.
    ///
    /// По умолчанию это отключено.
    pub fn case_insensitive(
        &mut self,
        yes: bool,
    ) -> Result<&mut GitignoreBuilder, Error> {
        // TODO: This should not return a `Result`. Fix this in the next semver
        // release.
        self.case_insensitive = yes;
        Ok(self)
    }

    /// Переключает, разрешены ли незакрытые классы символов. Когда разрешено,
    /// `[` без соответствующего `]` трактуется буквально вместо того,
    /// чтобы приводить к ошибке разбора.
    ///
    /// Например, если это установлено, то glob `[abc` будет трактоваться
    /// как буквальная строка `[abc` вместо возврата ошибки.
    ///
    /// По умолчанию это true для соответствия установленной семантике
    /// `gitignore`. Вообще говоря, включение этого приводит к худшим режимам
    /// отказа, поскольку парсер glob становится более разрешительным. Вы
    /// можете захотеть включить это, когда совместимость (например, с
    /// реализациями POSIX glob) важнее хороших сообщений об ошибках.
    pub fn allow_unclosed_class(
        &mut self,
        yes: bool,
    ) -> &mut GitignoreBuilder {
        self.allow_unclosed_class = yes;
        self
    }
}

/// Возвращает путь к файлу глобального файла gitignore текущего окружения.
///
/// Обратите внимание, что возвращённый путь к файлу может не существовать.
pub fn gitconfig_excludes_path() -> Option<PathBuf> {
    // git supports $HOME/.gitconfig and $XDG_CONFIG_HOME/git/config. Notably,
    // both can be active at the same time, where $HOME/.gitconfig takes
    // precedent. So if $HOME/.gitconfig defines a `core.excludesFile`, then
    // we're done.
    match gitconfig_home_contents().and_then(|x| parse_excludes_file(&x)) {
        Some(path) => return Some(path),
        None => {}
    }
    match gitconfig_xdg_contents().and_then(|x| parse_excludes_file(&x)) {
        Some(path) => return Some(path),
        None => {}
    }
    excludes_file_default()
}

/// Возвращает содержимое файла глобального конфигурационного файла git
/// пользователя, если он существует, в домашней директории пользователя.
fn gitconfig_home_contents() -> Option<Vec<u8>> {
    let home = match home_dir() {
        None => return None,
        Some(home) => home,
    };
    let mut file = match File::open(home.join(".gitconfig")) {
        Err(_) => return None,
        Ok(file) => BufReader::new(file),
    };
    let mut contents = vec![];
    file.read_to_end(&mut contents).ok().map(|_| contents)
}

/// Возвращает содержимое файла глобального конфигурационного файла git
/// пользователя, если он существует, в директории XDG_CONFIG_HOME пользователя.
fn gitconfig_xdg_contents() -> Option<Vec<u8>> {
    let path = std::env::var_os("XDG_CONFIG_HOME")
        .and_then(|x| if x.is_empty() { None } else { Some(PathBuf::from(x)) })
        .or_else(|| home_dir().map(|p| p.join(".config")))
        .map(|x| x.join("git/config"));
    let mut file = match path.and_then(|p| File::open(p).ok()) {
        None => return None,
        Some(file) => BufReader::new(file),
    };
    let mut contents = vec![];
    file.read_to_end(&mut contents).ok().map(|_| contents)
}

/// Возвращает путь к файлу по умолчанию для глобального файла .gitignore.
///
/// В частности, это уважает XDG_CONFIG_HOME.
fn excludes_file_default() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .and_then(|x| if x.is_empty() { None } else { Some(PathBuf::from(x)) })
        .or_else(|| home_dir().map(|p| p.join(".config")))
        .map(|x| x.join("git/ignore"))
}

/// Извлекает настройку `core.excludesfile` из git из данных содержимого
/// сырого файла.
fn parse_excludes_file(data: &[u8]) -> Option<PathBuf> {
    use std::sync::OnceLock;

    use regex_automata::{meta::Regex, util::syntax};

    // N.B. This is the lazy approach, and isn't technically correct, but
    // probably works in more circumstances. I guess we would ideally have
    // a full INI parser. Yuck.
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::builder()
            .configure(Regex::config().utf8_empty(false))
            .syntax(syntax::Config::new().utf8(false))
            .build(r#"(?im-u)^\s*excludesfile\s*=\s*"?\s*(\S+?)\s*"?\s*$"#)
            .unwrap()
    });
    // We don't care about amortizing allocs here I think. This should only
    // be called ~once per traversal or so? (Although it's not guaranteed...)
    let mut caps = re.create_captures();
    re.captures(data, &mut caps);
    let span = caps.get_group(1)?;
    let candidate = &data[span];
    std::str::from_utf8(candidate).ok().map(|s| PathBuf::from(expand_tilde(s)))
}

/// Разворачивает ~ в путях к файлам в значение $HOME.
fn expand_tilde(path: &str) -> String {
    let home = match home_dir() {
        None => return path.to_string(),
        Some(home) => home.to_string_lossy().into_owned(),
    };
    path.replace("~", &home)
}

/// Возвращает расположение домашней директории пользователя.
fn home_dir() -> Option<PathBuf> {
    // We're fine with using std::env::home_dir for now. Its bugs are, IMO,
    // pretty minor corner cases.
    #![allow(deprecated)]
    std::env::home_dir()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{Gitignore, GitignoreBuilder};

    fn gi_from_str<P: AsRef<Path>>(root: P, s: &str) -> Gitignore {
        let mut builder = GitignoreBuilder::new(root);
        builder.add_str(None, s).unwrap();
        builder.build().unwrap()
    }

    macro_rules! ignored {
        ($name:ident, $root:expr, $gi:expr, $path:expr) => {
            ignored!($name, $root, $gi, $path, false);
        };
        ($name:ident, $root:expr, $gi:expr, $path:expr, $is_dir:expr) => {
            #[test]
            fn $name() {
                let gi = gi_from_str($root, $gi);
                assert!(gi.matched($path, $is_dir).is_ignore());
            }
        };
    }

    macro_rules! not_ignored {
        ($name:ident, $root:expr, $gi:expr, $path:expr) => {
            not_ignored!($name, $root, $gi, $path, false);
        };
        ($name:ident, $root:expr, $gi:expr, $path:expr, $is_dir:expr) => {
            #[test]
            fn $name() {
                let gi = gi_from_str($root, $gi);
                assert!(!gi.matched($path, $is_dir).is_ignore());
            }
        };
    }

    const ROOT: &'static str = "/home/foobar/rust/rg";

    ignored!(ig1, ROOT, "months", "months");
    ignored!(ig2, ROOT, "*.lock", "Cargo.lock");
    ignored!(ig3, ROOT, "*.rs", "src/main.rs");
    ignored!(ig4, ROOT, "src/*.rs", "src/main.rs");
    ignored!(ig5, ROOT, "/*.c", "cat-file.c");
    ignored!(ig6, ROOT, "/src/*.rs", "src/main.rs");
    ignored!(ig7, ROOT, "!src/main.rs\n*.rs", "src/main.rs");
    ignored!(ig8, ROOT, "foo/", "foo", true);
    ignored!(ig9, ROOT, "**/foo", "foo");
    ignored!(ig10, ROOT, "**/foo", "src/foo");
    ignored!(ig11, ROOT, "**/foo/**", "src/foo/bar");
    ignored!(ig12, ROOT, "**/foo/**", "wat/src/foo/bar/baz");
    ignored!(ig13, ROOT, "**/foo/bar", "foo/bar");
    ignored!(ig14, ROOT, "**/foo/bar", "src/foo/bar");
    ignored!(ig15, ROOT, "abc/**", "abc/x");
    ignored!(ig16, ROOT, "abc/**", "abc/x/y");
    ignored!(ig17, ROOT, "abc/**", "abc/x/y/z");
    ignored!(ig18, ROOT, "a/**/b", "a/b");
    ignored!(ig19, ROOT, "a/**/b", "a/x/b");
    ignored!(ig20, ROOT, "a/**/b", "a/x/y/b");
    ignored!(ig21, ROOT, r"\!xy", "!xy");
    ignored!(ig22, ROOT, r"\#foo", "#foo");
    ignored!(ig23, ROOT, "foo", "./foo");
    ignored!(ig24, ROOT, "target", "grep/target");
    ignored!(ig25, ROOT, "Cargo.lock", "./tabwriter-bin/Cargo.lock");
    ignored!(ig26, ROOT, "/foo/bar/baz", "./foo/bar/baz");
    ignored!(ig27, ROOT, "foo/", "xyz/foo", true);
    ignored!(ig28, "./src", "/llvm/", "./src/llvm", true);
    ignored!(ig29, ROOT, "node_modules/ ", "node_modules", true);
    ignored!(ig30, ROOT, "**/", "foo/bar", true);
    ignored!(ig31, ROOT, "path1/*", "path1/foo");
    ignored!(ig32, ROOT, ".a/b", ".a/b");
    ignored!(ig33, "./", ".a/b", ".a/b");
    ignored!(ig34, ".", ".a/b", ".a/b");
    ignored!(ig35, "./.", ".a/b", ".a/b");
    ignored!(ig36, "././", ".a/b", ".a/b");
    ignored!(ig37, "././.", ".a/b", ".a/b");
    ignored!(ig38, ROOT, "\\[", "[");
    ignored!(ig39, ROOT, "\\?", "?");
    ignored!(ig40, ROOT, "\\*", "*");
    ignored!(ig41, ROOT, "\\a", "a");
    ignored!(ig42, ROOT, "s*.rs", "sfoo.rs");
    ignored!(ig43, ROOT, "**", "foo.rs");
    ignored!(ig44, ROOT, "**/**/*", "a/foo.rs");

    not_ignored!(ignot1, ROOT, "amonths", "months");
    not_ignored!(ignot2, ROOT, "monthsa", "months");
    not_ignored!(ignot3, ROOT, "/src/*.rs", "src/grep/src/main.rs");
    not_ignored!(ignot4, ROOT, "/*.c", "mozilla-sha1/sha1.c");
    not_ignored!(ignot5, ROOT, "/src/*.rs", "src/grep/src/main.rs");
    not_ignored!(ignot6, ROOT, "*.rs\n!src/main.rs", "src/main.rs");
    not_ignored!(ignot7, ROOT, "foo/", "foo", false);
    not_ignored!(ignot8, ROOT, "**/foo/**", "wat/src/afoo/bar/baz");
    not_ignored!(ignot9, ROOT, "**/foo/**", "wat/src/fooa/bar/baz");
    not_ignored!(ignot10, ROOT, "**/foo/bar", "foo/src/bar");
    not_ignored!(ignot11, ROOT, "#foo", "#foo");
    not_ignored!(ignot12, ROOT, "\n\n\n", "foo");
    not_ignored!(ignot13, ROOT, "foo/**", "foo", true);
    not_ignored!(
        ignot14,
        "./third_party/protobuf",
        "m4/ltoptions.m4",
        "./third_party/protobuf/csharp/src/packages/repositories.config"
    );
    not_ignored!(ignot15, ROOT, "!/bar", "foo/bar");
    not_ignored!(ignot16, ROOT, "*\n!**/", "foo", true);
    not_ignored!(ignot17, ROOT, "src/*.rs", "src/grep/src/main.rs");
    not_ignored!(ignot18, ROOT, "path1/*", "path2/path1/foo");
    not_ignored!(ignot19, ROOT, "s*.rs", "src/foo.rs");

    fn bytes(s: &str) -> Vec<u8> {
        s.to_string().into_bytes()
    }

    fn path_string<P: AsRef<Path>>(path: P) -> String {
        path.as_ref().to_str().unwrap().to_string()
    }

    #[test]
    fn parse_excludes_file1() {
        let data = bytes("[core]\nexcludesFile = /foo/bar");
        let got = super::parse_excludes_file(&data).unwrap();
        assert_eq!(path_string(got), "/foo/bar");
    }

    #[test]
    fn parse_excludes_file2() {
        let data = bytes("[core]\nexcludesFile = ~/foo/bar");
        let got = super::parse_excludes_file(&data).unwrap();
        assert_eq!(path_string(got), super::expand_tilde("~/foo/bar"));
    }

    #[test]
    fn parse_excludes_file3() {
        let data = bytes("[core]\nexcludeFile = /foo/bar");
        assert!(super::parse_excludes_file(&data).is_none());
    }

    #[test]
    fn parse_excludes_file4() {
        let data = bytes("[core]\nexcludesFile = \"~/foo/bar\"");
        let got = super::parse_excludes_file(&data);
        assert_eq!(
            path_string(got.unwrap()),
            super::expand_tilde("~/foo/bar")
        );
    }

    #[test]
    fn parse_excludes_file5() {
        let data = bytes("[core]\nexcludesFile = \" \"~/foo/bar \" \"");
        assert!(super::parse_excludes_file(&data).is_none());
    }

    // See: https://github.com/BurntSushi/ripgrep/issues/106
    #[test]
    fn regression_106() {
        gi_from_str("/", " ");
    }

    #[test]
    fn case_insensitive() {
        let gi = GitignoreBuilder::new(ROOT)
            .case_insensitive(true)
            .unwrap()
            .add_str(None, "*.html")
            .unwrap()
            .build()
            .unwrap();
        assert!(gi.matched("foo.html", false).is_ignore());
        assert!(gi.matched("foo.HTML", false).is_ignore());
        assert!(!gi.matched("foo.htm", false).is_ignore());
        assert!(!gi.matched("foo.HTM", false).is_ignore());
    }

    ignored!(cs1, ROOT, "*.html", "foo.html");
    not_ignored!(cs2, ROOT, "*.html", "foo.HTML");
    not_ignored!(cs3, ROOT, "*.html", "foo.htm");
    not_ignored!(cs4, ROOT, "*.html", "foo.HTM");
}
