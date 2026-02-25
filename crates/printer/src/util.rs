use std::{borrow::Cow, cell::OnceCell, fmt, io, path::Path, time};

use {
    bstr::ByteVec,
    grep_matcher::{Captures, LineTerminator, Match, Matcher},
    grep_searcher::{
        LineIter, Searcher, SinkContext, SinkContextKind, SinkError, SinkMatch,
    },
};

use crate::{MAX_LOOK_AHEAD, hyperlink::HyperlinkPath};

/// Тип для обработки замен с амортизацией выделения памяти.
pub(crate) struct Replacer<M: Matcher> {
    space: Option<Space<M>>,
}

struct Space<M: Matcher> {
    /// Место для хранения мест захвата.
    caps: M::Captures,
    /// Место для записи замены.
    dst: Vec<u8>,
    /// Место для хранения смещений совпадений в терминах `dst`.
    matches: Vec<Match>,
}

impl<M: Matcher> fmt::Debug for Replacer<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (dst, matches) = self.replacement().unwrap_or((&[], &[]));
        f.debug_struct("Replacer")
            .field("dst", &dst)
            .field("matches", &matches)
            .finish()
    }
}

impl<M: Matcher> Replacer<M> {
    /// Создаёт новую замену для использования с конкретным матчером.
    ///
    /// Этот конструктор не выделяет память. Вместо этого пространство для обработки
    /// замен выделяется лениво только при необходимости.
    pub(crate) fn new() -> Replacer<M> {
        Replacer { space: None }
    }

    /// Выполняет замену в данной строке haystack, заменяя все
    /// совпадения данной заменой. Для доступа к результату
    /// замены используйте метод `replacement`.
    ///
    /// Это может завершиться ошибкой, если нижележащий матчер сообщит об ошибке.
    pub(crate) fn replace_all<'a>(
        &'a mut self,
        searcher: &Searcher,
        matcher: &M,
        mut haystack: &[u8],
        range: std::ops::Range<usize>,
        replacement: &[u8],
    ) -> io::Result<()> {
        // См. огромный комментарий в 'find_iter_at_in_context' ниже, почему мы
        // делаем этот танец.
        let is_multi_line = searcher.multi_line_with_matcher(&matcher);
        // Получаем line_terminator, который был удалён (если есть), чтобы мы могли добавить его
        // обратно.
        let line_terminator = if is_multi_line {
            if haystack[range.end..].len() >= MAX_LOOK_AHEAD {
                haystack = &haystack[..range.end + MAX_LOOK_AHEAD];
            }
            &[]
        } else {
            // При поиске одной строки мы должны удалить терминатор строки.
            // В противном случае возможно, что regex (через look-around) обнаружит
            // терминатор строки и не совпадёт из-за этого.
            let mut m = Match::new(0, range.end);
            let line_terminator =
                trim_line_terminator(searcher, haystack, &mut m);
            haystack = &haystack[..m.end()];
            line_terminator
        };
        {
            let &mut Space { ref mut dst, ref mut caps, ref mut matches } =
                self.allocate(matcher)?;
            dst.clear();
            matches.clear();

            replace_with_captures_in_context(
                matcher,
                haystack,
                line_terminator,
                range.clone(),
                caps,
                dst,
                |caps, dst| {
                    let start = dst.len();
                    caps.interpolate(
                        |name| matcher.capture_index(name),
                        haystack,
                        replacement,
                        dst,
                    );
                    let end = dst.len();
                    matches.push(Match::new(start, end));
                    true
                },
            )
            .map_err(io::Error::error_message)?;
        }
        Ok(())
    }

    /// Возвращает результат предыдущей замены и смещения совпадений для
    /// всех вхождений замены в возвращённом буфере замены.
    ///
    /// Если замена не происходила, возвращается `None`.
    pub(crate) fn replacement<'a>(
        &'a self,
    ) -> Option<(&'a [u8], &'a [Match])> {
        match self.space {
            None => None,
            Some(ref space) => {
                if space.matches.is_empty() {
                    None
                } else {
                    Some((&space.dst, &space.matches))
                }
            }
        }
    }

    /// Очищает пространство, использованное для выполнения замены.
    ///
    /// Последующие вызовы `replacement` после вызова `clear` (но до
    /// выполнения другой замены) всегда будут возвращать `None`.
    pub(crate) fn clear(&mut self) {
        if let Some(ref mut space) = self.space {
            space.dst.clear();
            space.matches.clear();
        }
    }

    /// Выделяет пространство для замен при использовании с данным матчером и
    /// возвращает мутабельную ссылку на это пространство.
    ///
    /// Это может завершиться ошибкой, если выделение пространства для мест захвата
    /// от данного матчера не удастся.
    fn allocate(&mut self, matcher: &M) -> io::Result<&mut Space<M>> {
        if self.space.is_none() {
            let caps =
                matcher.new_captures().map_err(io::Error::error_message)?;
            self.space = Some(Space { caps, dst: vec![], matches: vec![] });
        }
        Ok(self.space.as_mut().unwrap())
    }
}

/// Простой слой абстракции над либо совпадением, либо контекстной строкой,
/// сообщённой поисковиком.
///
/// В частности, это предоставляет API, который объединяет типы `SinkMatch` и
/// `SinkContext`, одновременно предоставляя список всех индивидуальных мест
/// совпадений.
///
/// Хотя это служит удобным механизмом для абстракции над `SinkMatch`
/// и `SinkContext`, это также предоставляет способ абстракции над заменами.
/// А именно, после замены значение `Sunk` может быть создано с использованием
/// результатов замены вместо байтов, сообщённых непосредственно поисковиком.
#[derive(Debug)]
pub(crate) struct Sunk<'a> {
    bytes: &'a [u8],
    absolute_byte_offset: u64,
    line_number: Option<u64>,
    context_kind: Option<&'a SinkContextKind>,
    matches: &'a [Match],
    original_matches: &'a [Match],
}

impl<'a> Sunk<'a> {
    #[inline]
    pub(crate) fn empty() -> Sunk<'static> {
        Sunk {
            bytes: &[],
            absolute_byte_offset: 0,
            line_number: None,
            context_kind: None,
            matches: &[],
            original_matches: &[],
        }
    }

    #[inline]
    pub(crate) fn from_sink_match(
        sunk: &'a SinkMatch<'a>,
        original_matches: &'a [Match],
        replacement: Option<(&'a [u8], &'a [Match])>,
    ) -> Sunk<'a> {
        let (bytes, matches) =
            replacement.unwrap_or_else(|| (sunk.bytes(), original_matches));
        Sunk {
            bytes,
            absolute_byte_offset: sunk.absolute_byte_offset(),
            line_number: sunk.line_number(),
            context_kind: None,
            matches,
            original_matches,
        }
    }

    #[inline]
    pub(crate) fn from_sink_context(
        sunk: &'a SinkContext<'a>,
        original_matches: &'a [Match],
        replacement: Option<(&'a [u8], &'a [Match])>,
    ) -> Sunk<'a> {
        let (bytes, matches) =
            replacement.unwrap_or_else(|| (sunk.bytes(), original_matches));
        Sunk {
            bytes,
            absolute_byte_offset: sunk.absolute_byte_offset(),
            line_number: sunk.line_number(),
            context_kind: Some(sunk.kind()),
            matches,
            original_matches,
        }
    }

    #[inline]
    pub(crate) fn context_kind(&self) -> Option<&'a SinkContextKind> {
        self.context_kind
    }

    #[inline]
    pub(crate) fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    #[inline]
    pub(crate) fn matches(&self) -> &'a [Match] {
        self.matches
    }

    #[inline]
    pub(crate) fn original_matches(&self) -> &'a [Match] {
        self.original_matches
    }

    #[inline]
    pub(crate) fn lines(&self, line_term: u8) -> LineIter<'a> {
        LineIter::new(line_term, self.bytes())
    }

    #[inline]
    pub(crate) fn absolute_byte_offset(&self) -> u64 {
        self.absolute_byte_offset
    }

    #[inline]
    pub(crate) fn line_number(&self) -> Option<u64> {
        self.line_number
    }
}

/// Простая инкапсуляция пути к файлу, используемого принтером.
///
/// Это представляет любые преобразования, которые мы можем захотеть выполнить над путём,
/// такие как преобразование его в валидный UTF-8 и/или замена его разделителя на
/// что-то другое. Это позволяет нам амортизировать работу, если мы выводим
/// путь к файлу для каждого совпадения.
///
/// В обычном случае преобразование не требуется, что позволяет нам избежать
/// выделения памяти. Обычно только Windows требует преобразования, поскольку
/// проблематично получить доступ к сырым байтам пути напрямую и сначала нужно
/// потерянно преобразовать в UTF-8. Windows также обычно является местом, где используется
/// замена разделителя путей, например, в средах cygwin для использования `/`
/// вместо `\`.
///
/// Пользователи этого типа должны создавать его из обычного `Path`,
/// найденного в стандартной библиотеке. Затем он может быть записан в любую реализацию
/// `io::Write` с использованием метода `as_bytes`. Это достигает платформенной
/// переносимости с небольшой стоимостью: на Windows пути, которые не являются валидным UTF-16,
/// не будут воспроизведены корректно.
#[derive(Clone, Debug)]
pub(crate) struct PrinterPath<'a> {
    // На Unix мы можем повторно материализовать `Path` из нашего `Cow<'a, [u8]>` с
    // нулевой стоимостью, поэтому нет смысла хранить его. На момент написания
    // OsStr::as_os_str_bytes (и его соответствующий конструктор) ещё не
    // стабилизированы. Они позволили бы нам достичь того же конца переносимо. (До тех пор
    // пока мы сохраняем наше требование UTF-8 на Windows.)
    #[cfg(not(unix))]
    path: &'a Path,
    bytes: Cow<'a, [u8]>,
    hyperlink: OnceCell<Option<HyperlinkPath>>,
}

impl<'a> PrinterPath<'a> {
    /// Создаёт новый путь, пригодный для вывода.
    pub(crate) fn new(path: &'a Path) -> PrinterPath<'a> {
        PrinterPath {
            #[cfg(not(unix))]
            path,
            // N.B. This is zero-cost on Unix and requires at least a UTF-8
            // check on Windows. This doesn't allocate on Windows unless the
            // path is invalid UTF-8 (which is exceptionally rare).
            bytes: Vec::from_path_lossy(path),
            hyperlink: OnceCell::new(),
        }
    }

    /// Устанавливает разделитель на этом пути.
    ///
    /// Когда установлено, `PrinterPath::as_bytes` вернёт предоставленный путь, но
    /// с его разделителем, заменённым на данный.
    pub(crate) fn with_separator(
        mut self,
        sep: Option<u8>,
    ) -> PrinterPath<'a> {
        /// Заменяет разделитель пути в этом пути данным разделителем
        /// и делает это на месте. На Windows оба `/` и `\` обрабатываются как
        /// разделители путей, которые оба заменяются на `new_sep`. Во всех других
        /// средах только `/` обрабатывается как разделитель путей.
        fn replace_separator(bytes: &[u8], sep: u8) -> Vec<u8> {
            let mut bytes = bytes.to_vec();
            for b in bytes.iter_mut() {
                if *b == b'/' || (cfg!(windows) && *b == b'\\') {
                    *b = sep;
                }
            }
            bytes
        }
        let Some(sep) = sep else { return self };
        self.bytes = Cow::Owned(replace_separator(self.as_bytes(), sep));
        self
    }

    /// Возвращает сырые байты для этого пути.
    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Возвращает этот путь как гиперссылку.
    ///
    /// Обратите внимание, что гиперссылка может не быть создана из пути.
    /// А именно, вычисление гиперссылки может потребовать касания файловой системы
    /// (например, для каноникализации пути), и это может завершиться ошибкой. Эта ошибка
    /// молчалива, но логируется.
    pub(crate) fn as_hyperlink(&self) -> Option<&HyperlinkPath> {
        self.hyperlink
            .get_or_init(|| HyperlinkPath::from_path(self.as_path()))
            .as_ref()
    }

    /// Возвращает этот путь как фактический тип `Path`.
    pub(crate) fn as_path(&self) -> &Path {
        #[cfg(unix)]
        fn imp<'p>(p: &'p PrinterPath<'_>) -> &'p Path {
            use std::{ffi::OsStr, os::unix::ffi::OsStrExt};
            Path::new(OsStr::from_bytes(p.as_bytes()))
        }
        #[cfg(not(unix))]
        fn imp<'p>(p: &'p PrinterPath<'_>) -> &'p Path {
            p.path
        }
        imp(self)
    }
}

/// Тип, который предоставляет "более приятные" реализации Display и Serialize для
/// std::time::Duration. Формат сериализации должен быть фактически совместим с
/// реализацией Deserialize для std::time::Duration, поскольку этот тип только
/// добавляет новые поля.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct NiceDuration(pub time::Duration);

impl fmt::Display for NiceDuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:0.6}s", self.fractional_seconds())
    }
}

impl NiceDuration {
    /// Возвращает количество секунд в этой длительности в дробной форме.
    /// Число слева от десятичной точки — это количество секунд,
    /// а число справа — это количество миллисекунд.
    fn fractional_seconds(&self) -> f64 {
        let fractional = (self.0.subsec_nanos() as f64) / 1_000_000_000.0;
        self.0.as_secs() as f64 + fractional
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for NiceDuration {
    fn serialize<S: serde::Serializer>(
        &self,
        ser: S,
    ) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;

        let mut state = ser.serialize_struct("Duration", 3)?;
        state.serialize_field("secs", &self.0.as_secs())?;
        state.serialize_field("nanos", &self.0.subsec_nanos())?;
        state.serialize_field("human", &format!("{}", self))?;
        state.end()
    }
}

/// Простой форматтер для преобразования значений `u64` в ASCII байтовые строки.
///
/// Это позволяет избежать использования machinery форматирования, которое, кажется,
/// существенно замедляет вещи.
///
/// Крейт `itoa` делает то же самое, что и этот форматтер, но немного
/// быстрее. Мы реализуем свой собственный, который немного медленнее, но даёт нам
/// достаточный выигрыш, чтобы быть удовлетворёнными, и с чистым безопасным кодом.
#[derive(Debug)]
pub(crate) struct DecimalFormatter {
    buf: [u8; Self::MAX_U64_LEN],
    start: usize,
}

impl DecimalFormatter {
    /// Обнаружено через `u64::MAX.to_string().len()`.
    const MAX_U64_LEN: usize = 20;

    /// Создаёт новый десятичный форматтер для данного 64-битного беззнакового целого числа.
    pub(crate) fn new(mut n: u64) -> DecimalFormatter {
        let mut buf = [0; Self::MAX_U64_LEN];
        let mut i = buf.len();
        loop {
            i -= 1;

            let digit = u8::try_from(n % 10).unwrap();
            n /= 10;
            buf[i] = b'0' + digit;
            if n == 0 {
                break;
            }
        }
        DecimalFormatter { buf, start: i }
    }

    /// Возвращает десятичное число, отформатированное как ASCII байтовая строка.
    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.buf[self.start..]
    }
}

/// Обрезает префиксные ASCII пробелы из данного слайса и возвращает соответствующий
/// диапазон.
///
/// Это прекращает обрезку префикса, как только видит не пробел или терминатор строки.
pub(crate) fn trim_ascii_prefix(
    line_term: LineTerminator,
    slice: &[u8],
    range: Match,
) -> Match {
    fn is_space(b: u8) -> bool {
        match b {
            b'\t' | b'\n' | b'\x0B' | b'\x0C' | b'\r' | b' ' => true,
            _ => false,
        }
    }

    let count = slice[range]
        .iter()
        .take_while(|&&b| -> bool {
            is_space(b) && !line_term.as_bytes().contains(&b)
        })
        .count();
    range.with_start(range.start() + count)
}

pub(crate) fn find_iter_at_in_context<M, F>(
    searcher: &Searcher,
    matcher: M,
    mut bytes: &[u8],
    range: std::ops::Range<usize>,
    mut matched: F,
) -> io::Result<()>
where
    M: Matcher,
    F: FnMut(Match) -> bool,
{
    // Эта странная пляска нужна для учёта возможности look-ahead в
    // regex. Проблема здесь в том, что mat.bytes() не включает
    // строки за пределами границ совпадения в многострочном режиме, что означает, что
    // когда мы пытаемся заново обнаружить полный набор совпадений здесь, regex может больше не
    // совпасть, если он требовал некоторого look-ahead за пределами совпадающих строк.
    //
    // PCRE2 (и интерфейсы grep-matcher) не предоставляет способа указания конечной
    // границы поиска. Поэтому мы используем костыль и позволяем движку regex искать
    // остальную часть буфера... Но чтобы избежать слишком безумных вещей, мы ограничиваем
    // буфер.
    //
    // Если бы не многострочный режим, то ничего этого не было бы нужно.
    // Альтернативно, если бы мы рефакторили интерфейсы grep для передачи
    // полного набора совпадений (если доступно) от поисковика, то это также
    // могло бы помочь здесь. Но это влечёт за собой неизбежную предварительную стоимость для
    // случая, когда совпадения не нужно подсчитывать. Поэтому тогда вам придётся
    // ввести способ передачи совпадений условно, только когда это нужно. Ой.
    //
    // Возможно, более общая вещь здесь в том, что поисковик должен быть
    // ответственен за поиск совпадений, когда это необходимо, а принтер
    // не должен быть вовлечён в это дело в первую очередь. Вздох. Живи и учись.
    // Границы абстракций сложны.
    let is_multi_line = searcher.multi_line_with_matcher(&matcher);
    if is_multi_line {
        if bytes[range.end..].len() >= MAX_LOOK_AHEAD {
            bytes = &bytes[..range.end + MAX_LOOK_AHEAD];
        }
    } else {
        // При поиске одной строки мы должны удалить терминатор строки.
        // В противном случае возможно, что regex (через look-around) обнаружит
        // терминатор строки и не совпадёт из-за этого.
        let mut m = Match::new(0, range.end);
        // Нет нужды запоминать терминатор строки, так как мы не делаем здесь
        // замену.
        trim_line_terminator(searcher, bytes, &mut m);
        bytes = &bytes[..m.end()];
    }
    matcher
        .find_iter_at(bytes, range.start, |m| {
            if m.start() >= range.end {
                return false;
            }
            matched(m)
        })
        .map_err(io::Error::error_message)
}

/// Учитывая buf и некоторые границы, если в конце
/// данных границ в buf есть терминатор строки, то границы обрезаются для удаления терминатора
/// строки, возвращая слайс удалённого терминатора строки (если есть).
pub(crate) fn trim_line_terminator<'b>(
    searcher: &Searcher,
    buf: &'b [u8],
    line: &mut Match,
) -> &'b [u8] {
    let lineterm = searcher.line_terminator();
    if lineterm.is_suffix(&buf[*line]) {
        let mut end = line.end() - 1;
        if lineterm.is_crlf() && end > 0 && buf.get(end - 1) == Some(&b'\r') {
            end -= 1;
        }
        let orig_end = line.end();
        *line = line.with_end(end);
        &buf[end..orig_end]
    } else {
        &[]
    }
}

/// Как `Matcher::replace_with_captures_at`, но принимает конечную границу.
///
/// Смотрите также: `find_iter_at_in_context` для того, почему нам это нужно.
fn replace_with_captures_in_context<M, F>(
    matcher: M,
    bytes: &[u8],
    line_terminator: &[u8],
    range: std::ops::Range<usize>,
    caps: &mut M::Captures,
    dst: &mut Vec<u8>,
    mut append: F,
) -> Result<(), M::Error>
where
    M: Matcher,
    F: FnMut(&M::Captures, &mut Vec<u8>) -> bool,
{
    let mut last_match = range.start;
    matcher.captures_iter_at(bytes, range.start, caps, |caps| {
        let m = caps.get(0).unwrap();
        if m.start() >= range.end {
            return false;
        }
        dst.extend(&bytes[last_match..m.start()]);
        last_match = m.end();
        append(caps, dst)
    })?;
    let end = if last_match > range.end {
        bytes.len()
    } else {
        std::cmp::min(bytes.len(), range.end)
    };
    dst.extend(&bytes[last_match..end]);
    // Add back any line terminator.
    dst.extend(line_terminator);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_decimal_format() {
        let fmt = |n: u64| {
            let bytes = DecimalFormatter::new(n).as_bytes().to_vec();
            String::from_utf8(bytes).unwrap()
        };
        let std = |n: u64| n.to_string();

        let ints = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 20, 100, 123, u64::MAX];
        for n in ints {
            assert_eq!(std(n), fmt(n));
        }
    }
}
