use std::{
    ffi::{OsStr, OsString},
    fs::File,
    io,
    path::{Path, PathBuf},
    process::Command,
};

use globset::{Glob, GlobSet, GlobSetBuilder};

use crate::process::{CommandError, CommandReader, CommandReaderBuilder};

/// Построитель для матчера, который определяет, какие файлы будут распакованы.
#[derive(Clone, Debug)]
pub struct DecompressionMatcherBuilder {
    /// Команды для каждого подходящего glob-шаблона.
    commands: Vec<DecompressionCommand>,
    /// Следует ли включать правила сопоставления по умолчанию.
    defaults: bool,
}

/// Представление отдельной команды для распаковки данных
/// во внешнем процессе.
#[derive(Clone, Debug)]
struct DecompressionCommand {
    /// Glob-шаблон, который соответствует этой команде.
    glob: String,
    /// Имя команды или бинарного файла.
    bin: PathBuf,
    /// Аргументы для вызова команды.
    args: Vec<OsString>,
}

impl Default for DecompressionMatcherBuilder {
    fn default() -> DecompressionMatcherBuilder {
        DecompressionMatcherBuilder::new()
    }
}

impl DecompressionMatcherBuilder {
    /// Создает новый построитель для настройки матчера распаковки.
    pub fn new() -> DecompressionMatcherBuilder {
        DecompressionMatcherBuilder { commands: vec![], defaults: true }
    }

    /// Построить матчер для определения способа распаковки файлов.
    ///
    /// Если возникла проблема при компиляции матчера, то возвращается
    /// ошибка.
    pub fn build(&self) -> Result<DecompressionMatcher, CommandError> {
        let defaults = if !self.defaults {
            vec![]
        } else {
            default_decompression_commands()
        };
        let mut glob_builder = GlobSetBuilder::new();
        let mut commands = vec![];
        for decomp_cmd in defaults.iter().chain(&self.commands) {
            let glob = Glob::new(&decomp_cmd.glob).map_err(|err| {
                CommandError::io(io::Error::new(io::ErrorKind::Other, err))
            })?;
            glob_builder.add(glob);
            commands.push(decomp_cmd.clone());
        }
        let globs = glob_builder.build().map_err(|err| {
            CommandError::io(io::Error::new(io::ErrorKind::Other, err))
        })?;
        Ok(DecompressionMatcher { globs, commands })
    }

    /// Когда включено, правила сопоставления по умолчанию будут скомпилированы
    /// в этот матчер перед любыми другими ассоциациями. Когда выключено,
    /// используются только правила, явно указанные этому построителю.
    ///
    /// По умолчанию включено.
    pub fn defaults(&mut self, yes: bool) -> &mut DecompressionMatcherBuilder {
        self.defaults = yes;
        self
    }

    /// Связывает glob-шаблон с командой для распаковки файлов,
    /// соответствующих glob-шаблону.
    ///
    /// Если несколько glob-шаблонов соответствуют одному файлу, то
    /// последний добавленный glob-шаблон имеет приоритет.
    ///
    /// Синтаксис glob задокументирован в
    /// [`globset` crate](https://docs.rs/globset/#syntax).
    ///
    /// `program` разрешается относительно `PATH` и превращается
    /// в абсолютный путь внутри перед выполнением текущей
    /// платформой. В частности, в Windows это избегает проблемы
    /// безопасности, где передача относительного пути в `CreateProcess`
    /// автоматически ищет программу в текущем каталоге. Если программа
    /// не может быть разрешена, то она молча игнорируется и ассоциация
    /// отбрасывается. По этой причине вызывающие должны предпочесть
    /// `try_associate`.
    pub fn associate<P, I, A>(
        &mut self,
        glob: &str,
        program: P,
        args: I,
    ) -> &mut DecompressionMatcherBuilder
    where
        P: AsRef<OsStr>,
        I: IntoIterator<Item = A>,
        A: AsRef<OsStr>,
    {
        let _ = self.try_associate(glob, program, args);
        self
    }

    /// Связывает glob-шаблон с командой для распаковки файлов,
    /// соответствующих glob-шаблону.
    ///
    /// Если несколько glob-шаблонов соответствуют одному файлу, то
    /// последний добавленный glob-шаблон имеет приоритет.
    ///
    /// Синтаксис glob задокументирован в
    /// [`globset` crate](https://docs.rs/globset/#syntax).
    ///
    /// `program` разрешается относительно `PATH` и превращается
    /// в абсолютный путь внутри перед выполнением текущей
    /// платформой. В частности, в Windows это избегает проблемы
    /// безопасности, где передача относительного пути в `CreateProcess`
    /// автоматически ищет программу в текущем каталоге. Если программа
    /// не может быть разрешена, то возвращается ошибка.
    pub fn try_associate<P, I, A>(
        &mut self,
        glob: &str,
        program: P,
        args: I,
    ) -> Result<&mut DecompressionMatcherBuilder, CommandError>
    where
        P: AsRef<OsStr>,
        I: IntoIterator<Item = A>,
        A: AsRef<OsStr>,
    {
        let glob = glob.to_string();
        let bin = try_resolve_binary(Path::new(program.as_ref()))?;
        let args =
            args.into_iter().map(|a| a.as_ref().to_os_string()).collect();
        self.commands.push(DecompressionCommand { glob, bin, args });
        Ok(self)
    }
}

/// Матчер для определения способа распаковки файлов.
#[derive(Clone, Debug)]
pub struct DecompressionMatcher {
    /// Набор glob-шаблонов для сопоставления. Каждый glob имеет соответствующую
    /// запись в `commands`. Когда glob совпадает, соответствующая команда
    /// должна использоваться для выполнения распаковки во внешнем процессе.
    globs: GlobSet,
    /// Команды для каждого подходящего glob-шаблона.
    commands: Vec<DecompressionCommand>,
}

impl Default for DecompressionMatcher {
    fn default() -> DecompressionMatcher {
        DecompressionMatcher::new()
    }
}

impl DecompressionMatcher {
    /// Создает новый матчер с правилами по умолчанию.
    ///
    /// Чтобы добавить больше правил сопоставления, постройте матчер с
    /// [`DecompressionMatcherBuilder`].
    pub fn new() -> DecompressionMatcher {
        DecompressionMatcherBuilder::new()
            .build()
            .expect("встроенные правила сопоставления всегда должны компилироваться")
    }

    /// Возвращает предварительно собранную команду на основе данного пути к файлу,
    /// которая может распаковать его содержимое. Если такая команда распаковки
    /// неизвестна, то возвращается `None`.
    ///
    /// Если есть несколько возможных команд, соответствующих данному пути, то
    /// последняя добавленная команда имеет приоритет.
    pub fn command<P: AsRef<Path>>(&self, path: P) -> Option<Command> {
        if let Some(i) = self.globs.matches(path).into_iter().next_back() {
            let decomp_cmd = &self.commands[i];
            let mut cmd = Command::new(&decomp_cmd.bin);
            cmd.args(&decomp_cmd.args);
            return Some(cmd);
        }
        None
    }

    /// Возвращает true тогда и только тогда, когда данный путь к файлу имеет
    /// хотя бы одну соответствующую команду для выполнения распаковки.
    pub fn has_command<P: AsRef<Path>>(&self, path: P) -> bool {
        self.globs.is_match(path)
    }
}

/// Настраивает и строит потоковый читатель для распаковки данных.
#[derive(Clone, Debug, Default)]
pub struct DecompressionReaderBuilder {
    matcher: DecompressionMatcher,
    command_builder: CommandReaderBuilder,
}

impl DecompressionReaderBuilder {
    /// Создает новый построитель с конфигурацией по умолчанию.
    pub fn new() -> DecompressionReaderBuilder {
        DecompressionReaderBuilder::default()
    }

    /// Построить новый потоковый читатель для распаковки данных.
    ///
    /// Если распаковка выполняется во внешнем процессе и если возникла проблема
    /// при запуске процесса, то его ошибка логируется на уровне отладки и
    /// возвращается передающий читатель, который не выполняет распаковку.
    /// Это поведение обычно возникает, когда данный путь к файлу соответствует
    /// команде распаковки, но выполняется в среде, где команда распаковки
    /// недоступна.
    ///
    /// Если данный путь к файлу не может быть сопоставлен со стратегией
    /// распаковки, то возвращается передающий читатель, который не выполняет
    /// распаковку.
    pub fn build<P: AsRef<Path>>(
        &self,
        path: P,
    ) -> Result<DecompressionReader, CommandError> {
        let path = path.as_ref();
        let Some(mut cmd) = self.matcher.command(path) else {
            return DecompressionReader::new_passthru(path);
        };
        cmd.arg(path);

        match self.command_builder.build(&mut cmd) {
            Ok(cmd_reader) => Ok(DecompressionReader { rdr: Ok(cmd_reader) }),
            Err(err) => {
                log::debug!(
                    "{}: error spawning command '{:?}': {} \
                     (falling back to uncompressed reader)",
                    path.display(),
                    cmd,
                    err,
                );
                DecompressionReader::new_passthru(path)
            }
        }
    }

    /// Установить матчер для использования при поиске команды распаковки
    /// для каждого пути к файлу.
    ///
    /// По умолчанию включен набор разумных правил. Установка этого полностью
    /// заменяет текущие правила.
    pub fn matcher(
        &mut self,
        matcher: DecompressionMatcher,
    ) -> &mut DecompressionReaderBuilder {
        self.matcher = matcher;
        self
    }

    /// Получить базовый матчер, используемый в данный момент этим построителем.
    pub fn get_matcher(&self) -> &DecompressionMatcher {
        &self.matcher
    }

    /// Когда включено, читатель будет асинхронно читать содержимое вывода
    /// stderr команды. Когда выключено, stderr читается только после того,
    /// как поток stdout исчерпан (или если процесс завершается с кодом ошибки).
    ///
    /// Обратите внимание, что при включении это может потребовать запуска
    /// дополнительного потока для чтения stderr. Это делается для того, чтобы
    /// выполняемый процесс никогда не блокировался при записи в stdout или stderr.
    /// Если это отключено, то процесс может заполнить буфер stderr и вызвать
    /// взаимную блокировку.
    ///
    /// По умолчанию включено.
    pub fn async_stderr(
        &mut self,
        yes: bool,
    ) -> &mut DecompressionReaderBuilder {
        self.command_builder.async_stderr(yes);
        self
    }
}

/// Потоковый читатель для распаковки содержимого файла.
///
/// Назначение этого читателя — обеспечить простой способ распаковки
/// содержимого файла с использованием существующих инструментов в текущей
/// среде. Это должно быть альтернативой использованию библиотек распаковки
/// в пользу простоты и переносимости использования внешних команд, таких как
/// `gzip` и `xz`. Это накладывает некоторые накладные расходы на запуск
/// процесса, поэтому, если эти накладные расходы неприемлемы, следует искать
/// другие средства для выполнения распаковки.
///
/// Читатель распаковки поставляется с набором правил сопоставления по
/// умолчанию, которые предназначены для сопоставления путей к файлам с
/// соответствующей командой для их распаковки. Например, glob-шаблон `*.gz`
/// соответствует сжатым gzip файлам с командой `gzip -d -c`. Если путь к
/// файлу не соответствует ни одному из существующих правил или если он
/// соответствует правилу, команда которого не существует в текущей среде,
/// то читатель распаковки передает содержимое базового файла без выполнения
/// какой-либо распаковки.
///
/// Правила сопоставления по умолчанию, вероятно, подходят для большинства
/// случаев, и если они требуют пересмотра, приветствуются pull-запросы.
/// В случаях, когда их необходимо изменить или расширить, их можно
/// настроить с помощью [`DecompressionMatcherBuilder`] и
/// [`DecompressionReaderBuilder`].
///
/// По умолчанию этот читатель будет асинхронно читать stderr процессов.
/// Это предотвращает тонкие ошибки взаимной блокировки для шумных процессов,
/// которые много пишут в stderr. В настоящее время все содержимое stderr
/// читается в кучу.
///
/// # Пример
///
/// Этот пример показывает, как читать распакованное содержимое файла без
/// необходимости явно выбирать команду распаковки для запуска.
///
/// Обратите внимание, что если вам нужно распаковать несколько файлов,
/// лучше использовать `DecompressionReaderBuilder`, который амортизирует
/// стоимость компиляции матчера.
///
/// ```no_run
/// use std::{io::Read, process::Command};
///
/// use grep_cli::DecompressionReader;
///
/// let mut rdr = DecompressionReader::new("/usr/share/man/man1/ls.1.gz")?;
/// let mut contents = vec![];
/// rdr.read_to_end(&mut contents)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct DecompressionReader {
    rdr: Result<CommandReader, File>,
}

impl DecompressionReader {
    /// Построить новый потоковый читатель для распаковки данных.
    ///
    /// Если распаковка выполняется во внешнем процессе и если возникла проблема
    /// при запуске процесса, то возвращается его ошибка.
    ///
    /// Если данный путь к файлу не может быть сопоставлен со стратегией
    /// распаковки, то возвращается передающий читатель, который не выполняет
    /// распаковку.
    ///
    /// Используются правила сопоставления по умолчанию для определения способа
    /// распаковки данного файла. Чтобы изменить эти правила сопоставления,
    /// используйте [`DecompressionReaderBuilder`] и [`DecompressionMatcherBuilder`].
    ///
    /// При создании читателей для многих путей лучше использовать построитель,
    /// так как он амортизирует стоимость создания матчера.
    pub fn new<P: AsRef<Path>>(
        path: P,
    ) -> Result<DecompressionReader, CommandError> {
        DecompressionReaderBuilder::new().build(path)
    }

    /// Создает новый "передающий" читатель распаковки, который читает из файла,
    /// соответствующего данному пути, без выполнения распаковки и без запуска
    /// другого процесса.
    fn new_passthru(path: &Path) -> Result<DecompressionReader, CommandError> {
        let file = File::open(path)?;
        Ok(DecompressionReader { rdr: Err(file) })
    }

    /// Закрывает этот читатель, освобождая любые ресурсы, используемые его
    /// базовым дочерним процессом, если таковой использовался. Если дочерний
    /// процесс завершается с ненулевым кодом выхода, то возвращенное значение
    /// Err будет включать его stderr.
    ///
    /// `close` идемпотентен, что означает, что его можно безопасно вызывать
    /// несколько раз. Первый вызов закрывает CommandReader, а любые последующие
    /// вызовы ничего не делают.
    ///
    /// Этот метод следует вызывать после частичного чтения файла для
    /// предотвращения утечки ресурсов. Однако нет необходимости явно вызывать
    /// `close`, если ваш код всегда вызывает `read` до EOF, так как `read`
    /// заботится о вызове `close` в этом случае.
    ///
    /// `close` также вызывается в `drop` как последний рубеж защиты от
    /// утечки ресурсов. Любая ошибка от дочернего процесса затем печатается
    /// как предупреждение в stderr. Этого можно избежать, явно вызвав `close`
    /// перед тем, как CommandReader будет удален.
    pub fn close(&mut self) -> io::Result<()> {
        match self.rdr {
            Ok(ref mut rdr) => rdr.close(),
            Err(_) => Ok(()),
        }
    }
}

impl io::Read for DecompressionReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.rdr {
            Ok(ref mut rdr) => rdr.read(buf),
            Err(ref mut rdr) => rdr.read(buf),
        }
    }
}

/// Разрешает путь к программе в путь путем поиска программы в `PATH`.
///
/// Если программа не может быть разрешена, то возвращается ошибка.
///
/// Цель этого вместо передачи пути к программе напрямую в Command::new
/// заключается в том, что Command::new передает относительные пути в
/// CreateProcess в Windows, что неявно ищет исполняемый файл в текущем
/// рабочем каталоге. Это может быть нежелательно по соображениям
/// безопасности. Например, запуск ripgrep с флагом -z/--search-zip в
/// недоверенном дереве каталогов может привести к выполнению произвольных
/// программ в Windows.
///
/// Обратите внимание, что это все еще может вернуть относительный путь,
/// если PATH содержит относительный путь. Мы разрешаем это, поскольку
/// предполагается, что пользователь установил это явно и, следовательно,
/// желает такого поведения.
///
/// # Поведение платформы
///
/// В не-Windows это операция без действия.
pub fn resolve_binary<P: AsRef<Path>>(
    prog: P,
) -> Result<PathBuf, CommandError> {
    if !cfg!(windows) {
        return Ok(prog.as_ref().to_path_buf());
    }
    try_resolve_binary(prog)
}

/// Разрешает путь к программе в путь путем поиска программы в `PATH`.
///
/// Если программа не может быть разрешена, то возвращается ошибка.
///
/// Цель этого вместо передачи пути к программе напрямую в Command::new
/// заключается в том, что Command::new передает относительные пути в
/// CreateProcess в Windows, что неявно ищет исполняемый файл в текущем
/// рабочем каталоге. Это может быть нежелательно по соображениям
/// безопасности. Например, запуск ripgrep с флагом -z/--search-zip в
/// недоверенном дереве каталогов может привести к выполнению произвольных
/// программ в Windows.
///
/// Обратите внимание, что это все еще может вернуть относительный путь,
/// если PATH содержит относительный путь. Мы разрешаем это, поскольку
/// предполагается, что пользователь установил это явно и, следовательно,
/// желает такого поведения.
///
/// Если `check_exists` равен false или путь уже является абсолютным,
/// это вернется немедленно.
fn try_resolve_binary<P: AsRef<Path>>(
    prog: P,
) -> Result<PathBuf, CommandError> {
    use std::env;

    fn is_exe(path: &Path) -> bool {
        let Ok(md) = path.metadata() else { return false };
        !md.is_dir()
    }

    let prog = prog.as_ref();
    if prog.is_absolute() {
        return Ok(prog.to_path_buf());
    }
    let Some(syspaths) = env::var_os("PATH") else {
        let msg = "system PATH environment variable not found";
        return Err(CommandError::io(io::Error::new(
            io::ErrorKind::Other,
            msg,
        )));
    };
    for syspath in env::split_paths(&syspaths) {
        if syspath.as_os_str().is_empty() {
            continue;
        }
        let abs_prog = syspath.join(prog);
        if is_exe(&abs_prog) {
            return Ok(abs_prog.to_path_buf());
        }
        if abs_prog.extension().is_none() {
            for extension in ["com", "exe"] {
                let abs_prog = abs_prog.with_extension(extension);
                if is_exe(&abs_prog) {
                    return Ok(abs_prog.to_path_buf());
                }
            }
        }
    }
    let msg = format!("{}: could not find executable in PATH", prog.display());
    return Err(CommandError::io(io::Error::new(io::ErrorKind::Other, msg)));
}

fn default_decompression_commands() -> Vec<DecompressionCommand> {
    const ARGS_GZIP: &[&str] = &["gzip", "-d", "-c"];
    const ARGS_BZIP: &[&str] = &["bzip2", "-d", "-c"];
    const ARGS_XZ: &[&str] = &["xz", "-d", "-c"];
    const ARGS_LZ4: &[&str] = &["lz4", "-d", "-c"];
    const ARGS_LZMA: &[&str] = &["xz", "--format=lzma", "-d", "-c"];
    const ARGS_BROTLI: &[&str] = &["brotli", "-d", "-c"];
    const ARGS_ZSTD: &[&str] = &["zstd", "-q", "-d", "-c"];
    const ARGS_UNCOMPRESS: &[&str] = &["uncompress", "-c"];

    fn add(glob: &str, args: &[&str], cmds: &mut Vec<DecompressionCommand>) {
        let bin = match resolve_binary(Path::new(args[0])) {
            Ok(bin) => bin,
            Err(err) => {
                log::debug!("{}", err);
                return;
            }
        };
        cmds.push(DecompressionCommand {
            glob: glob.to_string(),
            bin,
            args: args
                .iter()
                .skip(1)
                .map(|s| OsStr::new(s).to_os_string())
                .collect(),
        });
    }
    let mut cmds = vec![];
    add("*.gz", ARGS_GZIP, &mut cmds);
    add("*.tgz", ARGS_GZIP, &mut cmds);
    add("*.bz2", ARGS_BZIP, &mut cmds);
    add("*.tbz2", ARGS_BZIP, &mut cmds);
    add("*.xz", ARGS_XZ, &mut cmds);
    add("*.txz", ARGS_XZ, &mut cmds);
    add("*.lz4", ARGS_LZ4, &mut cmds);
    add("*.lzma", ARGS_LZMA, &mut cmds);
    add("*.br", ARGS_BROTLI, &mut cmds);
    add("*.zst", ARGS_ZSTD, &mut cmds);
    add("*.zstd", ARGS_ZSTD, &mut cmds);
    add("*.Z", ARGS_UNCOMPRESS, &mut cmds);
    cmds
}
