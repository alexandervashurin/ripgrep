/*!
Определяет очень простой логгер, который работает с крейтом `log`.

Мы не делаем ничего сложного. Нам нужны только базовые уровни логов и
возможность вывода в stderr. Поэтому мы избегаем привлечения дополнительных
зависимостей только для этой функциональности.
*/

use log::Log;

/// Простейший логгер, который логирует в stderr.
///
/// Этот логгер не выполняет фильтрацию. Вместо этого он полагается на
/// фильтрацию крейта `log` через его глобальную настройку max_level.
#[derive(Debug)]
pub(crate) struct Logger(());

/// Одиночка, используемый как цель для реализации трейта `Log`.
const LOGGER: &'static Logger = &Logger(());

impl Logger {
    /// Создать новый логгер, который логирует в stderr, и инициализировать
    /// его как глобальный логгер. Если возникла проблема при установке
    /// логгера, то возвращается ошибка.
    pub(crate) fn init() -> Result<(), log::SetLoggerError> {
        log::set_logger(LOGGER)
    }
}

impl Log for Logger {
    fn enabled(&self, _: &log::Metadata<'_>) -> bool {
        // Мы устанавливаем уровень лога через log::set_max_level, поэтому
        // нам не нужно реализовывать фильтрацию здесь.
        true
    }

    fn log(&self, record: &log::Record<'_>) {
        match (record.file(), record.line()) {
            (Some(file), Some(line)) => {
                eprintln_locked!(
                    "{}|{}|{}:{}: {}",
                    record.level(),
                    record.target(),
                    file,
                    line,
                    record.args()
                );
            }
            (Some(file), None) => {
                eprintln_locked!(
                    "{}|{}|{}: {}",
                    record.level(),
                    record.target(),
                    file,
                    record.args()
                );
            }
            _ => {
                eprintln_locked!(
                    "{}|{}: {}",
                    record.level(),
                    record.target(),
                    record.args()
                );
            }
        }
    }

    fn flush(&self) {
        // Мы используем eprintln_locked!, который сбрасывается при каждом вызове.
    }
}
