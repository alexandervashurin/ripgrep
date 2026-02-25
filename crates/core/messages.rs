/*!
Этот модуль определяет некоторые макросы и некоторое общее изменяемое состояние.

Это состояние отвечает за отслеживание того, должны ли мы выводить определенные
виды сообщений пользователю (например, ошибки), которые отличаются от
стандартных отладочных сообщений или сообщений трассировки. Это состояние
конкретно устанавливается во время запуска при разборе аргументов CLI и затем
никогда не изменяется.

Другое состояние, отслеживаемое здесь, — это то, испытала ли ripgrep условие
ошибки. Помимо ошибок, связанных с невалидными аргументами CLI, ripgrep обычно
не прерывается при возникновении ошибки (например, если чтение файла не
удалось). Но когда ошибка все же возникает, она изменит статус выхода ripgrep.
Таким образом, когда сообщение об ошибке выводится через `err_message`, то
переключается глобальный флаг, указывающий, что произошла хотя бы одна ошибка.
Когда ripgrep завершается, этот флаг проверяется для определения того, каким
должен быть статус выхода.
*/

use std::sync::atomic::{AtomicBool, Ordering};

/// Когда false, "сообщения" не будут выводиться.
static MESSAGES: AtomicBool = AtomicBool::new(false);
/// Когда false, сообщения, связанные с правилами игнорирования, не будут выводиться.
static IGNORE_MESSAGES: AtomicBool = AtomicBool::new(false);
/// Переключается на true, когда выводится сообщение об ошибке.
static ERRORED: AtomicBool = AtomicBool::new(false);

/// Как eprintln, но блокирует stdout для предотвращения перемешивания строк.
///
/// Это блокирует stdout, а не stderr, хотя это выводит в stderr. Это
/// избегает появления перемешанного вывода, когда stdout и stderr оба
/// соответствуют tty.
#[macro_export]
macro_rules! eprintln_locked {
    ($($tt:tt)*) => {{
        {
            use std::io::Write;

            // Это в некотором роде нарушение абстракции, потому что мы явно
            // блокируем stdout перед выводом в stderr. Это избегает перемешивания
            // строк внутри ripgrep, потому что `search_parallel` использует
            // `termcolor`, который обращается к той же блокировке stdout при
            // записи строк.
            let stdout = std::io::stdout().lock();
            let mut stderr = std::io::stderr().lock();
            // Мы специально игнорируем любые ошибки здесь. Одна правдоподобная
            // ошибка, которую мы можем получить в некоторых случаях, — это ошибка
            // разрыва канала. И когда это происходит, мы должны выйти gracefully.
            // В противном случае просто прерываем с кодом ошибки, потому что
            // мы не можем сделать much else.
            //
            // См.: https://github.com/BurntSushi/ripgrep/issues/1966
            if let Err(err) = write!(stderr, "rg: ") {
                if err.kind() == std::io::ErrorKind::BrokenPipe {
                    std::process::exit(0);
                } else {
                    std::process::exit(2);
                }
            }
            if let Err(err) = writeln!(stderr, $($tt)*) {
                if err.kind() == std::io::ErrorKind::BrokenPipe {
                    std::process::exit(0);
                } else {
                    std::process::exit(2);
                }
            }
            drop(stdout);
        }
    }}
}

/// Выводит неустранимое сообщение об ошибке, если только сообщения не были отключены.
#[macro_export]
macro_rules! message {
    ($($tt:tt)*) => {
        if crate::messages::messages() {
            eprintln_locked!($($tt)*);
        }
    }
}

/// Как message, но устанавливает флаг "errored" ripgrep, который управляет
/// статусом выхода.
#[macro_export]
macro_rules! err_message {
    ($($tt:tt)*) => {
        crate::messages::set_errored();
        message!($($tt)*);
    }
}

/// Выводит связанное с игнорированием неустранимое сообщение об ошибке
/// (например, ошибку разбора), если только сообщения об игнорировании не
/// были отключены.
#[macro_export]
macro_rules! ignore_message {
    ($($tt:tt)*) => {
        if crate::messages::messages() && crate::messages::ignore_messages() {
            eprintln_locked!($($tt)*);
        }
    }
}

/// Возвращает true тогда и только тогда, когда сообщения должны отображаться.
pub(crate) fn messages() -> bool {
    MESSAGES.load(Ordering::Relaxed)
}

/// Установить, должны ли сообщения отображаться или нет.
///
/// По умолчанию они не отображаются.
pub(crate) fn set_messages(yes: bool) {
    MESSAGES.store(yes, Ordering::Relaxed)
}

/// Возвращает true тогда и только тогда, когда сообщения, связанные с
/// "игнорированием", должны отображаться.
pub(crate) fn ignore_messages() -> bool {
    IGNORE_MESSAGES.load(Ordering::Relaxed)
}

/// Установить, должны ли сообщения, связанные с "игнорированием",
/// отображаться или нет.
///
/// По умолчанию они не отображаются.
///
/// Обратите внимание, что это переопределяется, если `messages` отключен.
/// А именно, если `messages` отключен, то сообщения об "игнорировании"
/// никогда не отображаются, независимо от этой настройки.
pub(crate) fn set_ignore_messages(yes: bool) {
    IGNORE_MESSAGES.store(yes, Ordering::Relaxed)
}

/// Возвращает true тогда и только тогда, когда ripgrep столкнулся с
/// неустранимой ошибкой.
pub(crate) fn errored() -> bool {
    ERRORED.load(Ordering::Relaxed)
}

/// Указать, что ripgrep столкнулся с неустранимой ошибкой.
///
/// Вызывающие не должны использовать это напрямую. Вместо этого это
/// вызывается автоматически через макрос `err_message`.
pub(crate) fn set_errored() {
    ERRORED.store(true, Ordering::Relaxed);
}
