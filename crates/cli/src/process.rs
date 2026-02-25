use std::{
    io::{self, Read},
    process,
};

/// Ошибка, которая может возникнуть при запуске команды и чтении ее вывода.
///
/// Эта ошибка может быть бесшовно преобразована в `io::Error` через
/// реализацию `From`.
#[derive(Debug)]
pub struct CommandError {
    kind: CommandErrorKind,
}

#[derive(Debug)]
enum CommandErrorKind {
    Io(io::Error),
    Stderr(Vec<u8>),
}

impl CommandError {
    /// Создать ошибку из ошибки I/O.
    pub(crate) fn io(ioerr: io::Error) -> CommandError {
        CommandError { kind: CommandErrorKind::Io(ioerr) }
    }

    /// Создать ошибку из содержимого stderr (которое может быть пустым).
    pub(crate) fn stderr(bytes: Vec<u8>) -> CommandError {
        CommandError { kind: CommandErrorKind::Stderr(bytes) }
    }

    /// Возвращает true тогда и только тогда, когда эта ошибка имеет пустые
    /// данные из stderr.
    pub(crate) fn is_empty(&self) -> bool {
        match self.kind {
            CommandErrorKind::Stderr(ref bytes) => bytes.is_empty(),
            _ => false,
        }
    }
}

impl std::error::Error for CommandError {}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            CommandErrorKind::Io(ref e) => e.fmt(f),
            CommandErrorKind::Stderr(ref bytes) => {
                let msg = String::from_utf8_lossy(bytes);
                if msg.trim().is_empty() {
                    write!(f, "<stderr is empty>")
                } else {
                    let div = "-".repeat(79);
                    write!(
                        f,
                        "\n{div}\n{msg}\n{div}",
                        div = div,
                        msg = msg.trim()
                    )
                }
            }
        }
    }
}

impl From<io::Error> for CommandError {
    fn from(ioerr: io::Error) -> CommandError {
        CommandError { kind: CommandErrorKind::Io(ioerr) }
    }
}

impl From<CommandError> for io::Error {
    fn from(cmderr: CommandError) -> io::Error {
        match cmderr.kind {
            CommandErrorKind::Io(ioerr) => ioerr,
            CommandErrorKind::Stderr(_) => {
                io::Error::new(io::ErrorKind::Other, cmderr)
            }
        }
    }
}

/// Настраивает и строит потоковый читатель для вывода процесса.
#[derive(Clone, Debug, Default)]
pub struct CommandReaderBuilder {
    async_stderr: bool,
}

impl CommandReaderBuilder {
    /// Создает новый построитель с конфигурацией по умолчанию.
    pub fn new() -> CommandReaderBuilder {
        CommandReaderBuilder::default()
    }

    /// Построить новый потоковый читатель для вывода данной команды.
    ///
    /// Вызывающий должен установить все, что требуется для данной команды,
    /// перед построением читателя, например, ее аргументы, окружение и
    /// текущий рабочий каталог. Настройки, такие как каналы stdout и stderr
    /// (но не stdin), будут переопределены, чтобы они могли управляться
    /// читателем.
    ///
    /// Если возникла проблема при запуске данной команды, то возвращается
    /// ее ошибка.
    pub fn build(
        &self,
        command: &mut process::Command,
    ) -> Result<CommandReader, CommandError> {
        let mut child = command
            .stdout(process::Stdio::piped())
            .stderr(process::Stdio::piped())
            .spawn()?;
        let stderr = if self.async_stderr {
            StderrReader::r#async(child.stderr.take().unwrap())
        } else {
            StderrReader::sync(child.stderr.take().unwrap())
        };
        Ok(CommandReader { child, stderr, eof: false })
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
    pub fn async_stderr(&mut self, yes: bool) -> &mut CommandReaderBuilder {
        self.async_stderr = yes;
        self
    }
}

/// Потоковый читатель для вывода команды.
///
/// Назначение этого читателя — предоставить простой способ выполнения
/// процессов, stdout которых читается потоковым способом, одновременно
/// делая stderr процессов доступным, когда процесс завершается с кодом
/// выхода. Это делает возможным выполнение процессов с отображением
/// основного режима отказа в случае ошибки.
///
/// Более того, по умолчанию этот читатель будет асинхронно читать stderr
/// процессов. Это предотвращает тонкие ошибки взаимной блокировки для
/// шумных процессов, которые много пишут в stderr. В настоящее время все
/// содержимое stderr читается в кучу.
///
/// # Пример
///
/// Этот пример показывает, как вызвать `gzip` для распаковки содержимого
/// файла. Если команда `gzip` сообщает о неудачном статусе выхода, то ее
/// stderr возвращается как ошибка.
///
/// ```no_run
/// use std::{io::Read, process::Command};
///
/// use grep_cli::CommandReader;
///
/// let mut cmd = Command::new("gzip");
/// cmd.arg("-d").arg("-c").arg("/usr/share/man/man1/ls.1.gz");
///
/// let mut rdr = CommandReader::new(&mut cmd)?;
/// let mut contents = vec![];
/// rdr.read_to_end(&mut contents)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct CommandReader {
    child: process::Child,
    stderr: StderrReader,
    /// Устанавливается в true, когда 'read' возвращает ноль байт. Когда это
    /// не установлено и мы закрываем читатель, то ожидаем ошибку канала
    /// при сборе дочернего процесса и заглушаем ее.
    eof: bool,
}

impl CommandReader {
    /// Создать потоковый читатель для данной команды с использованием
    /// конфигурации по умолчанию.
    ///
    /// Вызывающий должен установить все, что требуется для данной команды,
    /// перед построением читателя, например, ее аргументы, окружение и
    /// текущий рабочий каталог. Настройки, такие как каналы stdout и stderr
    /// (но не stdin), будут переопределены, чтобы они могли управляться
    /// читателем.
    ///
    /// Если возникла проблема при запуске данной команды, то возвращается
    /// ее ошибка.
    ///
    /// Если вызывающему требуется дополнительная настройка для возвращаемого
    /// читателя, то используйте [`CommandReaderBuilder`].
    pub fn new(
        cmd: &mut process::Command,
    ) -> Result<CommandReader, CommandError> {
        CommandReaderBuilder::new().build(cmd)
    }

    /// Закрывает CommandReader, освобождая любые ресурсы, используемые его
    /// базовым дочерним процессом. Если дочерний процесс завершается с
    /// ненулевым кодом выхода, то возвращенное значение Err будет включать
    /// его stderr.
    ///
    /// `close` идемпотентен, что означает, что его можно безопасно вызывать
    /// несколько раз. Первый вызов закрывает CommandReader, а любые
    /// последующие вызовы ничего не делают.
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
        // Закрытие stdout закрывает базовый файловый дескриптор, что должно
        // заставить хорошо ведущий себя дочерний процесс выйти. Если
        // child.stdout равен None, мы предполагаем, что close() уже был
        // вызван, и ничего не делаем.
        let stdout = match self.child.stdout.take() {
            None => return Ok(()),
            Some(stdout) => stdout,
        };
        drop(stdout);
        if self.child.wait()?.success() {
            Ok(())
        } else {
            let err = self.stderr.read_to_end();
            // В конкретном случае, когда мы не потребили все данные от
            // дочернего процесса, то закрытие stdout выше приводит к
            // сигналу разрыва канала в большинстве случаев. Но я не думаю,
            // что есть какой-либо надежный и переносимый способ обнаружить
            // это. Вместо этого, если мы знаем, что не достигли EOF (так что
            // ожидаем ошибку разрыва канала) и если stderr в противном случае
            // не имеет ничего на нем, то мы предполагаем полный успех.
            if !self.eof && err.is_empty() {
                return Ok(());
            }
            Err(io::Error::from(err))
        }
    }
}

impl Drop for CommandReader {
    fn drop(&mut self) {
        if let Err(error) = self.close() {
            log::warn!("{}", error);
        }
    }
}

impl io::Read for CommandReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let stdout = match self.child.stdout {
            None => return Ok(0),
            Some(ref mut stdout) => stdout,
        };
        let nread = stdout.read(buf)?;
        if nread == 0 {
            self.eof = true;
            self.close().map(|_| 0)
        } else {
            Ok(nread)
        }
    }
}

/// Читатель, который инкапсулирует асинхронное или синхронное чтение stderr.
#[derive(Debug)]
enum StderrReader {
    Async(Option<std::thread::JoinHandle<CommandError>>),
    Sync(process::ChildStderr),
}

impl StderrReader {
    /// Создать читатель для stderr, который читает содержимое асинхронно.
    fn r#async(mut stderr: process::ChildStderr) -> StderrReader {
        let handle =
            std::thread::spawn(move || stderr_to_command_error(&mut stderr));
        StderrReader::Async(Some(handle))
    }

    /// Создать читатель для stderr, который читает содержимое синхронно.
    fn sync(stderr: process::ChildStderr) -> StderrReader {
        StderrReader::Sync(stderr)
    }

    /// Потребляет все содержимое stderr в кучу и возвращает его как ошибку.
    ///
    /// Если возникла проблема при чтении самого stderr, то возвращается
    /// ошибка I/O команды.
    fn read_to_end(&mut self) -> CommandError {
        match *self {
            StderrReader::Async(ref mut handle) => {
                let handle = handle
                    .take()
                    .expect("read_to_end cannot be called more than once");
                handle.join().expect("stderr reading thread does not panic")
            }
            StderrReader::Sync(ref mut stderr) => {
                stderr_to_command_error(stderr)
            }
        }
    }
}

fn stderr_to_command_error(stderr: &mut process::ChildStderr) -> CommandError {
    let mut bytes = vec![];
    match stderr.read_to_end(&mut bytes) {
        Ok(_) => CommandError::stderr(bytes),
        Err(err) => CommandError::io(err),
    }
}
