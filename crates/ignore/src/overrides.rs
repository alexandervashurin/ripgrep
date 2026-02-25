/*!
Модуль overrides предоставляет возможность указания набора override glob.

Это предоставляет функциональность, аналогичную `--include` или `--exclude`
в инструментах командной строки.
*/

use std::path::Path;

use crate::{
    Error, Match,
    gitignore::{self, Gitignore, GitignoreBuilder},
};

/// Glob представляет собой одиночный glob в matcher override.
///
/// Это используется для сообщения информации о glob с наивысшим приоритетом,
/// который совпал.
///
/// Обратите внимание, что не все совпадения обязательно соответствуют
/// конкретному glob. Например, если есть один или несколько glob белого
/// списка и путь к файлу не соответствует ни одному glob в наборе, то путь
/// к файлу считается проигнорированным.
///
/// Время жизни `'a` относится к времени жизни matcher, который создал
/// этот glob.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct Glob<'a>(GlobInner<'a>);

#[derive(Clone, Debug)]
#[allow(dead_code)]
enum GlobInner<'a> {
    /// No glob matched, but the file path should still be ignored.
    UnmatchedIgnore,
    /// A glob matched.
    Matched(&'a gitignore::Glob),
}

impl<'a> Glob<'a> {
    fn unmatched() -> Glob<'a> {
        Glob(GlobInner::UnmatchedIgnore)
    }
}

/// Управляет набором override, предоставленных явно конечным пользователем.
#[derive(Clone, Debug)]
pub struct Override(Gitignore);

impl Override {
    /// Возвращает пустой matcher, который никогда не соответствует ни одному пути.
    pub fn empty() -> Override {
        Override(Gitignore::empty())
    }

    /// Возвращает директорию этого набора override.
    ///
    /// Все совпадения выполняются относительно этого пути.
    pub fn path(&self) -> &Path {
        self.0.path()
    }

    /// Возвращает true тогда и только тогда, когда этот matcher пуст.
    ///
    /// Когда matcher пуст, он никогда не будет соответствовать ни одному пути.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Возвращает общее количество ignore glob.
    pub fn num_ignores(&self) -> u64 {
        self.0.num_whitelists()
    }

    /// Возвращает общее количество glob в белом списке.
    pub fn num_whitelists(&self) -> u64 {
        self.0.num_ignores()
    }

    /// Возвращает, соответствует ли данный путь к файлу шаблону в этом
    /// matcher override.
    ///
    /// `is_dir` должно быть true, если путь относится к директории, и false
    /// в противном случае.
    ///
    /// Если нет override, то это всегда возвращает `Match::None`.
    ///
    /// Если есть хотя бы один override белого списка и `is_dir` — false,
    /// то это никогда не возвращает `Match::None`, так как несовпадения
    /// интерпретируются как игнорируемые.
    ///
    /// Данный путь сопоставляется с glob относительно пути, данного при
    /// построении matcher override. В частности, перед сопоставлением
    /// `path` его префикс (как определяется общим суффиксом данной
    /// директории) удаляется. Если нет общего перекрытия суффикса/префикса,
    /// то `path` предполагается находящимся в той же директории, что и
    /// корневой путь для этого набора override.
    pub fn matched<'a, P: AsRef<Path>>(
        &'a self,
        path: P,
        is_dir: bool,
    ) -> Match<Glob<'a>> {
        if self.is_empty() {
            return Match::None;
        }
        let mat = self.0.matched(path, is_dir).invert();
        if mat.is_none() && self.num_whitelists() > 0 && !is_dir {
            return Match::Ignore(Glob::unmatched());
        }
        mat.map(move |giglob| Glob(GlobInner::Matched(giglob)))
    }
}

/// Строит matcher для набора glob override.
#[derive(Clone, Debug)]
pub struct OverrideBuilder {
    builder: GitignoreBuilder,
}

impl OverrideBuilder {
    /// Создаёт новый построитель override.
    ///
    /// Сопоставление выполняется относительно данного пути к директории.
    pub fn new<P: AsRef<Path>>(path: P) -> OverrideBuilder {
        let mut builder = GitignoreBuilder::new(path);
        builder.allow_unclosed_class(false);
        OverrideBuilder { builder }
    }

    /// Строит новый matcher override из glob, добавленных на данный момент.
    ///
    /// Как только matcher построен, в него нельзя добавить новые glob.
    pub fn build(&self) -> Result<Override, Error> {
        Ok(Override(self.builder.build()?))
    }

    /// Добавляет glob к набору override.
    ///
    /// Glob, предоставленные здесь, имеют точно такую же семантику, что и
    /// одиночная строка в файле `gitignore`, где значение `!` инвертировано:
    /// а именно, `!` в начале glob будет игнорировать файл. Без `!` все
    /// совпадения предоставленного glob трактуются как совпадения белого списка.
    pub fn add(&mut self, glob: &str) -> Result<&mut OverrideBuilder, Error> {
        self.builder.add_line(None, glob)?;
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
    ) -> Result<&mut OverrideBuilder, Error> {
        // TODO: This should not return a `Result`. Fix this in the next semver
        // release.
        self.builder.case_insensitive(yes)?;
        Ok(self)
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
    ///
    /// Это значение по умолчанию отличается от значения по умолчанию для
    /// [`Gitignore`]. А именно, [`Gitignore`] предназначен для соответствия
    /// поведению git как есть. Но эта абстракция для override glob не
    /// обязательно соответствует какой-либо другой известной спецификации
    /// и вместо этого приоритизирует лучшие сообщения об ошибках.
    pub fn allow_unclosed_class(&mut self, yes: bool) -> &mut OverrideBuilder {
        self.builder.allow_unclosed_class(yes);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::{Override, OverrideBuilder};

    const ROOT: &'static str = "/home/andrew/foo";

    fn ov(globs: &[&str]) -> Override {
        let mut builder = OverrideBuilder::new(ROOT);
        for glob in globs {
            builder.add(glob).unwrap();
        }
        builder.build().unwrap()
    }

    #[test]
    fn empty() {
        let ov = ov(&[]);
        assert!(ov.matched("a.foo", false).is_none());
        assert!(ov.matched("a", false).is_none());
        assert!(ov.matched("", false).is_none());
    }

    #[test]
    fn simple() {
        let ov = ov(&["*.foo", "!*.bar"]);
        assert!(ov.matched("a.foo", false).is_whitelist());
        assert!(ov.matched("a.foo", true).is_whitelist());
        assert!(ov.matched("a.rs", false).is_ignore());
        assert!(ov.matched("a.rs", true).is_none());
        assert!(ov.matched("a.bar", false).is_ignore());
        assert!(ov.matched("a.bar", true).is_ignore());
    }

    #[test]
    fn only_ignores() {
        let ov = ov(&["!*.bar"]);
        assert!(ov.matched("a.rs", false).is_none());
        assert!(ov.matched("a.rs", true).is_none());
        assert!(ov.matched("a.bar", false).is_ignore());
        assert!(ov.matched("a.bar", true).is_ignore());
    }

    #[test]
    fn precedence() {
        let ov = ov(&["*.foo", "!*.bar.foo"]);
        assert!(ov.matched("a.foo", false).is_whitelist());
        assert!(ov.matched("a.baz", false).is_ignore());
        assert!(ov.matched("a.bar.foo", false).is_ignore());
    }

    #[test]
    fn gitignore() {
        let ov = ov(&["/foo", "bar/*.rs", "baz/**"]);
        assert!(ov.matched("bar/lib.rs", false).is_whitelist());
        assert!(ov.matched("bar/wat/lib.rs", false).is_ignore());
        assert!(ov.matched("wat/bar/lib.rs", false).is_ignore());
        assert!(ov.matched("foo", false).is_whitelist());
        assert!(ov.matched("wat/foo", false).is_ignore());
        assert!(ov.matched("baz", false).is_ignore());
        assert!(ov.matched("baz/a", false).is_whitelist());
        assert!(ov.matched("baz/a/b", false).is_whitelist());
    }

    #[test]
    fn allow_directories() {
        // This tests that directories are NOT ignored when they are unmatched.
        let ov = ov(&["*.rs"]);
        assert!(ov.matched("foo.rs", false).is_whitelist());
        assert!(ov.matched("foo.c", false).is_ignore());
        assert!(ov.matched("foo", false).is_ignore());
        assert!(ov.matched("foo", true).is_none());
        assert!(ov.matched("src/foo.rs", false).is_whitelist());
        assert!(ov.matched("src/foo.c", false).is_ignore());
        assert!(ov.matched("src/foo", false).is_ignore());
        assert!(ov.matched("src/foo", true).is_none());
    }

    #[test]
    fn absolute_path() {
        let ov = ov(&["!/bar"]);
        assert!(ov.matched("./foo/bar", false).is_none());
    }

    #[test]
    fn case_insensitive() {
        let ov = OverrideBuilder::new(ROOT)
            .case_insensitive(true)
            .unwrap()
            .add("*.html")
            .unwrap()
            .build()
            .unwrap();
        assert!(ov.matched("foo.html", false).is_whitelist());
        assert!(ov.matched("foo.HTML", false).is_whitelist());
        assert!(ov.matched("foo.htm", false).is_ignore());
        assert!(ov.matched("foo.HTM", false).is_ignore());
    }

    #[test]
    fn default_case_sensitive() {
        let ov =
            OverrideBuilder::new(ROOT).add("*.html").unwrap().build().unwrap();
        assert!(ov.matched("foo.html", false).is_whitelist());
        assert!(ov.matched("foo.HTML", false).is_ignore());
        assert!(ov.matched("foo.htm", false).is_ignore());
        assert!(ov.matched("foo.HTM", false).is_ignore());
    }
}
