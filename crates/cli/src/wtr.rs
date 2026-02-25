use std::io::{self, IsTerminal};

use termcolor::HyperlinkSpec;

/// Писатель, поддерживающий раскраску с построчной или блочной буферизацией.
#[derive(Debug)]
pub struct StandardStream(StandardStreamKind);

/// Возвращает возможно буферизированный писатель в stdout для данного
/// выбора цвета.
///
/// Возвращаемый писатель либо построчно буферизирован, либо блочно
/// буферизирован. Решение между ними принимается автоматически на основе
/// того, подключен ли tty к stdout или нет. Если tty подключен, то
/// используется построчная буферизация. В противном случае используется
/// блочная буферизация. В целом, блочная буферизация более эффективна,
/// но может увеличить время, необходимое конечному пользователю для
/// просмотра первых бит вывода.
///
/// Если вам нужен более тонкий контроль над режимом буферизации, то
/// используйте один из `stdout_buffered_line` или `stdout_buffered_block`.
///
/// Выбор цвета передается базовому писателю. Чтобы полностью отключить
/// цвета во всех случаях, используйте `ColorChoice::Never`.
pub fn stdout(color_choice: termcolor::ColorChoice) -> StandardStream {
    if std::io::stdout().is_terminal() {
        stdout_buffered_line(color_choice)
    } else {
        stdout_buffered_block(color_choice)
    }
}

/// Возвращает построчно буферизированный писатель в stdout для данного
/// выбора цвета.
///
/// Этот писатель полезен при выводе результатов напрямую в tty, чтобы
/// пользователи видели вывод, как только он записан. Недостатком этого
/// подхода является то, что он может быть медленнее, особенно когда
/// много вывода.
///
/// Вы можете рассмотреть использование [`stdout`] вместо этого, который
/// выбирает стратегию буферизации автоматически на основе того, подключен
/// ли stdout к tty.
pub fn stdout_buffered_line(
    color_choice: termcolor::ColorChoice,
) -> StandardStream {
    let out = termcolor::StandardStream::stdout(color_choice);
    StandardStream(StandardStreamKind::LineBuffered(out))
}

/// Возвращает блочно буферизированный писатель в stdout для данного
/// выбора цвета.
///
/// Этот писатель полезен при выводе результатов в файл, поскольку он
/// амортизирует стоимость записи данных. Недостатком этого подхода является
/// то, что он может увеличить задержку отображения вывода при записи в tty.
///
/// Вы можете рассмотреть использование [`stdout`] вместо этого, который
/// выбирает стратегию буферизации автоматически на основе того, подключен
/// ли stdout к tty.
pub fn stdout_buffered_block(
    color_choice: termcolor::ColorChoice,
) -> StandardStream {
    let out = termcolor::BufferedStandardStream::stdout(color_choice);
    StandardStream(StandardStreamKind::BlockBuffered(out))
}

#[derive(Debug)]
enum StandardStreamKind {
    LineBuffered(termcolor::StandardStream),
    BlockBuffered(termcolor::BufferedStandardStream),
}

impl io::Write for StandardStream {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        use self::StandardStreamKind::*;

        match self.0 {
            LineBuffered(ref mut w) => w.write(buf),
            BlockBuffered(ref mut w) => w.write(buf),
        }
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        use self::StandardStreamKind::*;

        match self.0 {
            LineBuffered(ref mut w) => w.flush(),
            BlockBuffered(ref mut w) => w.flush(),
        }
    }
}

impl termcolor::WriteColor for StandardStream {
    #[inline]
    fn supports_color(&self) -> bool {
        use self::StandardStreamKind::*;

        match self.0 {
            LineBuffered(ref w) => w.supports_color(),
            BlockBuffered(ref w) => w.supports_color(),
        }
    }

    #[inline]
    fn supports_hyperlinks(&self) -> bool {
        use self::StandardStreamKind::*;

        match self.0 {
            LineBuffered(ref w) => w.supports_hyperlinks(),
            BlockBuffered(ref w) => w.supports_hyperlinks(),
        }
    }

    #[inline]
    fn set_color(&mut self, spec: &termcolor::ColorSpec) -> io::Result<()> {
        use self::StandardStreamKind::*;

        match self.0 {
            LineBuffered(ref mut w) => w.set_color(spec),
            BlockBuffered(ref mut w) => w.set_color(spec),
        }
    }

    #[inline]
    fn set_hyperlink(&mut self, link: &HyperlinkSpec) -> io::Result<()> {
        use self::StandardStreamKind::*;

        match self.0 {
            LineBuffered(ref mut w) => w.set_hyperlink(link),
            BlockBuffered(ref mut w) => w.set_hyperlink(link),
        }
    }

    #[inline]
    fn reset(&mut self) -> io::Result<()> {
        use self::StandardStreamKind::*;

        match self.0 {
            LineBuffered(ref mut w) => w.reset(),
            BlockBuffered(ref mut w) => w.reset(),
        }
    }

    #[inline]
    fn is_synchronous(&self) -> bool {
        use self::StandardStreamKind::*;

        match self.0 {
            LineBuffered(ref w) => w.is_synchronous(),
            BlockBuffered(ref w) => w.is_synchronous(),
        }
    }
}
