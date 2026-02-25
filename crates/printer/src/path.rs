use std::{io, path::Path};

use termcolor::WriteColor;

use crate::{
    color::ColorSpecs,
    hyperlink::{self, HyperlinkConfig},
    util::PrinterPath,
};

/// Конфигурация для описания того, как пути должны быть записаны.
#[derive(Clone, Debug)]
struct Config {
    colors: ColorSpecs,
    hyperlink: HyperlinkConfig,
    separator: Option<u8>,
    terminator: u8,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            colors: ColorSpecs::default(),
            hyperlink: HyperlinkConfig::default(),
            separator: None,
            terminator: b'\n',
        }
    }
}

/// Построитель для принтера, который выводит пути к файлам.
#[derive(Clone, Debug)]
pub struct PathPrinterBuilder {
    config: Config,
}

impl PathPrinterBuilder {
    /// Возвращает новый построитель путей к файлам с конфигурацией по умолчанию.
    pub fn new() -> PathPrinterBuilder {
        PathPrinterBuilder { config: Config::default() }
    }

    /// Создаёт новый принтер путей с текущей конфигурацией, который записывает
    /// пути в данный writer.
    pub fn build<W: WriteColor>(&self, wtr: W) -> PathPrinter<W> {
        let interpolator =
            hyperlink::Interpolator::new(&self.config.hyperlink);
        PathPrinter { config: self.config.clone(), wtr, interpolator }
    }

    /// Устанавливает спецификации пользовательских цветов для использования
    /// при раскраске в этом принтере.
    ///
    /// [`UserColorSpec`](crate::UserColorSpec) может быть создан из
    /// строки в соответствии с форматом спецификации цвета. Смотрите
    /// документацию типа `UserColorSpec` для получения более подробной информации
    /// о формате. Затем [`ColorSpecs`] может быть сгенерирован из нуля или более
    /// `UserColorSpec`.
    ///
    /// Независимо от предоставленных здесь спецификаций цвета, используется ли
    /// цвет на самом деле или нет, определяется реализацией
    /// `WriteColor`, предоставленной в `build`. Например, если `termcolor::NoColor`
    /// предоставлен в `build`, то никакой цвет никогда не будет напечатан, независимо
    /// от предоставленных здесь спецификаций цвета.
    ///
    /// Это полностью переопределяет любые предыдущие спецификации цвета. Это не
    /// добавляет к любым ранее предоставленным спецификациям цвета в этом
    /// построителе.
    ///
    /// Спецификации цвета по умолчанию не предоставляют стилизации.
    pub fn color_specs(
        &mut self,
        specs: ColorSpecs,
    ) -> &mut PathPrinterBuilder {
        self.config.colors = specs;
        self
    }

    /// Устанавливает конфигурацию для использования для гиперссылок, выводимых этим принтером.
    ///
    /// Независимо от предоставленного здесь формата гиперссылки, используются ли
    /// гиперссылки на самом деле или нет, определяется реализацией
    /// `WriteColor`, предоставленной в `build`. Например, если `termcolor::NoColor`
    /// предоставлен в `build`, то никакие гиперссылки никогда не будут напечатаны,
    /// независимо от предоставленного здесь формата.
    ///
    /// Это полностью переопределяет любой предыдущий формат гиперссылки.
    ///
    /// Конфигурация по умолчанию приводит к тому, что никакие гиперссылки не выводятся.
    pub fn hyperlink(
        &mut self,
        config: HyperlinkConfig,
    ) -> &mut PathPrinterBuilder {
        self.config.hyperlink = config;
        self
    }

    /// Устанавливает разделитель путей, используемый при выводе путей к файлам.
    ///
    /// Обычно вывод выполняется путём вывода пути к файлу как есть. Однако
    /// эта настройка предоставляет возможность использовать другой разделитель путей
    /// от того, что настроено в текущей среде.
    ///
    /// Типичное использование этой опции — позволить пользователям cygwin в Windows
    /// установить разделитель путей в `/` вместо использования системного `\` по умолчанию.
    ///
    /// Это отключено по умолчанию.
    pub fn separator(&mut self, sep: Option<u8>) -> &mut PathPrinterBuilder {
        self.config.separator = sep;
        self
    }

    /// Устанавливает терминатор путей, используемый.
    ///
    /// Терминатор путей — это байт, который выводится после каждого пути к файлу,
    /// выводимого этим принтером.
    ///
    /// Терминатор путей по умолчанию — `\n`.
    pub fn terminator(&mut self, terminator: u8) -> &mut PathPrinterBuilder {
        self.config.terminator = terminator;
        self
    }
}

/// Принтер путей к файлам с опциональной поддержкой цвета и гиперссылок.
///
/// Этот принтер очень похож на [`Summary`](crate::Summary) тем, что он
/// в основном выводит только пути к файлам. Основное различие заключается в том, что этот принтер
/// на самом деле не выполняет никакой поиск через реализацию `Sink`, а вместо этого
/// просто предоставляет способ вызывающей стороне выводить пути.
///
/// Вызывающая сторона могла бы просто выводить пути самостоятельно, но этот принтер обрабатывает
/// несколько деталей:
///
/// * Он может нормализовать разделители путей.
/// * Он позволяет настраивать терминатор.
/// * Он позволяет устанавливать конфигурацию цвета таким образом, который согласуется
/// с другими принтерами в этом крейте.
/// * Он позволяет устанавливать формат гиперссылки таким образом, который согласуется
/// с другими принтерами в этом крейте.
#[derive(Debug)]
pub struct PathPrinter<W> {
    config: Config,
    wtr: W,
    interpolator: hyperlink::Interpolator,
}

impl<W: WriteColor> PathPrinter<W> {
    /// Записывает данный путь в нижележащий writer.
    pub fn write(&mut self, path: &Path) -> io::Result<()> {
        let ppath = PrinterPath::new(path.as_ref())
            .with_separator(self.config.separator);
        if !self.wtr.supports_color() {
            self.wtr.write_all(ppath.as_bytes())?;
        } else {
            let status = self.start_hyperlink(&ppath)?;
            self.wtr.set_color(self.config.colors.path())?;
            self.wtr.write_all(ppath.as_bytes())?;
            self.wtr.reset()?;
            self.interpolator.finish(status, &mut self.wtr)?;
        }
        self.wtr.write_all(&[self.config.terminator])
    }

    /// Запускает span гиперссылки, когда применимо.
    fn start_hyperlink(
        &mut self,
        path: &PrinterPath,
    ) -> io::Result<hyperlink::InterpolatorStatus> {
        let Some(hyperpath) = path.as_hyperlink() else {
            return Ok(hyperlink::InterpolatorStatus::inactive());
        };
        let values = hyperlink::Values::new(hyperpath);
        self.interpolator.begin(&values, &mut self.wtr)
    }
}
