use std::io;

use bstr::ByteSlice;

/// Буфер ёмкостью по умолчанию, который мы используем для буфера строк.
pub(crate) const DEFAULT_BUFFER_CAPACITY: usize = 64 * (1 << 10); // 64 КБ

/// Поведение поисковика при работе с длинными строками и большими контекстами.
///
/// При инкрементальном поиске данных с использованием буфера фиксированного размера
/// это контролирует количество *дополнительной* памяти для выделения сверх размера буфера
/// для размещения строк (которые могут включать строки в окне контекста, когда
/// оно включено), которые не помещаются в буфере.
///
/// По умолчанию выполняется жадное выделение без ограничений.
#[derive(Clone, Copy, Debug)]
pub(crate) enum BufferAllocation {
    /// Пытаться расширить размер буфера до тех пор, пока либо хотя бы следующая
    /// строка не поместится в памяти, либо пока не будет исчерпана вся доступная память.
    ///
    /// Это значение по умолчанию.
    Eager,
    /// Ограничить количество дополнительной выделяемой памяти указанным размером. Если
    /// найдена строка, требующая больше памяти, чем разрешено здесь, то
    /// прекратить чтение и вернуть ошибку.
    Error(usize),
}

impl Default for BufferAllocation {
    fn default() -> BufferAllocation {
        BufferAllocation::Eager
    }
}

/// Создать новую ошибку для использования, когда достигнут предел выделения.
pub(crate) fn alloc_error(limit: usize) -> io::Error {
    let msg = format!("превышен предел выделения ({})", limit);
    io::Error::new(io::ErrorKind::Other, msg)
}

/// Поведение обнаружения двоичных данных в буфере строк.
///
/// Обнаружение двоичных данных — это процесс _эвристического_ определения того, является ли
/// данный фрагмент данных двоичным или нет, а затем принятие действия на основе
/// результата этой эвристики. Мотивация обнаружения двоичных данных
/// заключается в том, что двоичные данные часто указывают на данные, которые нежелательно искать
/// с помощью текстовых шаблонов. Конечно, есть много случаев, когда это неверно,
/// именно поэтому обнаружение двоичных данных отключено по умолчанию.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BinaryDetection {
    /// Обнаружение двоичных данных не выполняется. Данные, сообщаемые буфером строк, могут
    /// содержать произвольные байты.
    None,
    /// Указанный байт ищется во всём содержимом, прочитанном буфером строк. Если
    /// он встречается, то данные считаются двоичными, и буфер строк действует
    /// так, как если бы он достиг EOF. Буфер строк гарантирует, что этот байт никогда
    /// не будет наблюдаем вызывающим кодом.
    Quit(u8),
    /// Указанный байт ищется во всём содержимом, прочитанном буфером строк. Если
    /// он встречается, то он заменяется терминатором строки. Буфер строк
    /// гарантирует, что этот байт никогда не будет наблюдаем вызывающим кодом.
    Convert(u8),
}

impl Default for BinaryDetection {
    fn default() -> BinaryDetection {
        BinaryDetection::None
    }
}

impl BinaryDetection {
    /// Возвращает true, если и только если эвристика обнаружения требует,
    /// чтобы буфер строк прекратил чтение данных при обнаружении двоичных данных.
    fn is_quit(&self) -> bool {
        match *self {
            BinaryDetection::Quit(_) => true,
            _ => false,
        }
    }
}

/// Конфигурация буфера. Это содержит опции, которые фиксированы после
/// создания буфера.
#[derive(Clone, Copy, Debug)]
struct Config {
    /// Количество байтов для попытки чтения за раз.
    capacity: usize,
    /// Терминатор строки.
    lineterm: u8,
    /// Поведение для обработки длинных строк.
    buffer_alloc: BufferAllocation,
    /// Когда установлено, наличие указанного байта указывает на двоичное содержимое.
    binary: BinaryDetection,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            capacity: DEFAULT_BUFFER_CAPACITY,
            lineterm: b'\n',
            buffer_alloc: BufferAllocation::default(),
            binary: BinaryDetection::default(),
        }
    }
}

/// Билдер для создания буферов строк.
#[derive(Clone, Debug, Default)]
pub(crate) struct LineBufferBuilder {
    config: Config,
}

impl LineBufferBuilder {
    /// Создать новый билдер для буфера.
    pub(crate) fn new() -> LineBufferBuilder {
        LineBufferBuilder { config: Config::default() }
    }

    /// Создать новый буфер строк из конфигурации этого билдера.
    pub(crate) fn build(&self) -> LineBuffer {
        LineBuffer {
            config: self.config,
            buf: vec![0; self.config.capacity],
            pos: 0,
            last_lineterm: 0,
            end: 0,
            absolute_byte_offset: 0,
            binary_byte_offset: None,
        }
    }

    /// Установить ёмкость по умолчанию для использования в буфере.
    ///
    /// В общем, ёмкость буфера соответствует количеству данных
    /// для хранения в памяти и размеру чтений из нижележащего
    /// читателя.
    ///
    /// Это установлено на разумное значение по умолчанию и, вероятно, не должно изменяться,
    /// если нет конкретной причины для этого.
    pub(crate) fn capacity(
        &mut self,
        capacity: usize,
    ) -> &mut LineBufferBuilder {
        self.config.capacity = capacity;
        self
    }

    /// Установить терминатор строки для буфера.
    ///
    /// Каждый буфер имеет терминатор строки, и этот терминатор строки используется
    /// для определения того, как прокручивать буфер вперёд. Например, когда происходит чтение
    /// в буфер буфера, конец прочитанных данных, вероятно,
    /// соответствует неполной строке. Как буфер строк,
    /// вызывающие не должны получать доступ к этим данным, поскольку они неполные. Терминатор строки
    /// — это то, как буфер строк определяет ту часть чтения, которая
    /// является неполной.
    ///
    /// По умолчанию это установлено в `b'\n'`.
    pub(crate) fn line_terminator(
        &mut self,
        lineterm: u8,
    ) -> &mut LineBufferBuilder {
        self.config.lineterm = lineterm;
        self
    }

    /// Установить максимальный объём дополнительной памяти для выделения для длинных строк.
    ///
    /// Для включения построчного поиска фундаментальным требованием является
    /// то, что как минимум каждая строка должна помещаться в памяти. Эта
    /// настройка контролирует, какого размера эта строка может быть. По умолчанию это
    /// установлено в `BufferAllocation::Eager`, что означает, что буфер строк будет
    /// пытаться выделить как можно больше памяти для размещения строки и будет
    /// ограничен только доступной памятью.
    ///
    /// Обратите внимание, что эта настройка применяется только к количеству *дополнительной*
    /// памяти для выделения сверх ёмкости буфера. Это означает, что
    /// значение `0` имеет смысл и, в частности, гарантирует, что
    /// буфер строк никогда не выделит дополнительную память сверх своей начальной
    /// ёмкости.
    pub(crate) fn buffer_alloc(
        &mut self,
        behavior: BufferAllocation,
    ) -> &mut LineBufferBuilder {
        self.config.buffer_alloc = behavior;
        self
    }

    /// Следует ли включать обнаружение двоичных данных или нет. В зависимости от настройки,
    /// это может привести к тому, что буфер строк сообщит о EOF раньше или
    /// заставит буфер строк очистить данные.
    ///
    /// По умолчанию это отключено. В общем, обнаружение двоичных данных следует
    /// рассматривать как несовершенную эвристику.
    pub(crate) fn binary_detection(
        &mut self,
        detection: BinaryDetection,
    ) -> &mut LineBufferBuilder {
        self.config.binary = detection;
        self
    }
}

/// Чтение буфера строк эффективно читает строково-ориентированный буфер из
/// произвольного читателя.
#[derive(Debug)]
pub(crate) struct LineBufferReader<'b, R> {
    rdr: R,
    line_buffer: &'b mut LineBuffer,
}

impl<'b, R: io::Read> LineBufferReader<'b, R> {
    /// Создать новый буферизированный читатель, который читает из `rdr` и использует указанный
    /// `line_buffer` в качестве промежуточного буфера.
    ///
    /// Это не изменяет поведение обнаружения двоичных данных указанного буфера строк.
    pub(crate) fn new(
        rdr: R,
        line_buffer: &'b mut LineBuffer,
    ) -> LineBufferReader<'b, R> {
        line_buffer.clear();
        LineBufferReader { rdr, line_buffer }
    }

    /// Абсолютное смещение байта, которое соответствует начальным смещениям
    /// данных, возвращаемых `buffer`, относительно начала содержимого
    /// нижележащего читателя. Таким образом, это смещение обычно не
    /// соответствует смещению в памяти. Обычно оно используется для отчётности.
    /// Оно также может использоваться для подсчёта количества байтов, которые
    /// были найдены.
    pub(crate) fn absolute_byte_offset(&self) -> u64 {
        self.line_buffer.absolute_byte_offset()
    }

    /// Если двоичные данные были обнаружены, то это возвращает абсолютное смещение байта,
    /// при котором изначально были найдены двоичные данные.
    pub(crate) fn binary_byte_offset(&self) -> Option<u64> {
        self.line_buffer.binary_byte_offset()
    }

    /// Заполнить содержимое этого буфера, отбрасывая ту часть буфера,
    /// которая была потреблена. Свободное пространство, созданное отбрасыванием
    /// потреблённой части буфера, затем заполняется новыми данными от
    /// читателя.
    ///
    /// Если достигнут EOF, то возвращается `false`. В противном случае возвращается
    /// `true`. (Обратите внимание, что если обнаружение двоичных данных этого буфера строк установлено в
    /// `Quit`, то наличие двоичных данных приведёт к тому, что этот буфер
    /// будет вести себя так, как если бы он достиг EOF при первом появлении двоичных данных.)
    ///
    /// Это передаёт любые ошибки, возвращаемые нижележащим читателем, а также
    /// вернёт ошибку, если буфер должен быть расширен сверх предела выделения,
    /// в соответствии со стратегией выделения буфера.
    pub(crate) fn fill(&mut self) -> Result<bool, io::Error> {
        self.line_buffer.fill(&mut self.rdr)
    }

    /// Вернуть содержимое этого буфера.
    pub(crate) fn buffer(&self) -> &[u8] {
        self.line_buffer.buffer()
    }

    /// Вернуть буфер как BStr, используется для удобной проверки равенства
    /// только в тестах.
    #[cfg(test)]
    fn bstr(&self) -> &bstr::BStr {
        self.buffer().as_bstr()
    }

    /// Потребить указанное количество байтов. Это должно быть меньше или равно
    /// количеству байтов, возвращаемых `buffer`.
    pub(crate) fn consume(&mut self, amt: usize) {
        self.line_buffer.consume(amt);
    }

    /// Потребляет остаток буфера. Последующие вызовы `buffer`
    /// гарантированно вернут пустой срез, пока буфер не будет заполнен снова.
    ///
    /// Это удобная функция для `consume(buffer.len())`.
    #[cfg(test)]
    fn consume_all(&mut self) {
        self.line_buffer.consume_all();
    }
}

/// Буфер строк управляет (обычно фиксированным) буфером для хранения строк.
///
/// Вызывающие должны создавать буферы строк экономно и повторно использовать их, когда это возможно.
/// Буферы строк нельзя использовать напрямую, а вместо этого они должны использоваться через
/// LineBufferReader.
#[derive(Clone, Debug)]
pub(crate) struct LineBuffer {
    /// Конфигурация этого буфера.
    config: Config,
    /// Основной буфер для хранения данных.
    buf: Vec<u8>,
    /// Текущая позиция этого буфера. Это всегда допустимый индексируемый
    /// индекс в `buf`, и его максимальное значение — длина `buf`.
    pos: usize,
    /// Конечная позиция поискового содержимого в этом буфере. Это либо
    /// установлено сразу после последнего терминатора строки в буфере, либо
    /// сразу после последнего байта, выданного читателем, когда читатель
    /// был исчерпан.
    last_lineterm: usize,
    /// Конечная позиция буфера. Это всегда больше или равно
    /// last_lineterm. Байты между last_lineterm и end, если таковые имеются, всегда
    /// соответствуют частичной строке.
    end: usize,
    /// Абсолютное смещение байта, соответствующее `pos`. Чаще всего это
    /// не допустимый индекс в адресуемой памяти, а скорее смещение, которое
    /// относительно всех данных, проходящих через буфер строк (поскольку
    /// создание или с последнего вызова `clear`).
    ///
    /// Когда буфер строк достигает EOF, это устанавливается в позицию сразу
    /// после последнего байта, прочитанного из нижележащего читателя. То есть оно
    /// становится общим количеством байтов, которые были прочитаны.
    absolute_byte_offset: u64,
    /// Если двоичные данные были найдены, это записывает абсолютное смещение байта,
    /// при котором они были впервые обнаружены.
    binary_byte_offset: Option<u64>,
}

impl LineBuffer {
    /// Установить метод обнаружения двоичных данных, используемый в этом буфере строк.
    ///
    /// Это позволяет динамически изменять стратегию обнаружения двоичных данных в
    /// существующем буфере строк без необходимости создавать новый.
    pub(crate) fn set_binary_detection(&mut self, binary: BinaryDetection) {
        self.config.binary = binary;
    }

    /// Сбросить этот буфер, чтобы его можно было использовать с новым читателем.
    fn clear(&mut self) {
        self.pos = 0;
        self.last_lineterm = 0;
        self.end = 0;
        self.absolute_byte_offset = 0;
        self.binary_byte_offset = None;
    }

    /// Абсолютное смещение байта, которое соответствует начальным смещениям
    /// данных, возвращаемых `buffer`, относительно начала содержимого
    /// читателя. Таким образом, это смещение обычно не соответствует
    /// смещению в памяти. Обычно оно используется для отчётности,
    /// особенно в сообщениях об ошибках.
    ///
    /// Это сбрасывается в `0`, когда вызывается `clear`.
    fn absolute_byte_offset(&self) -> u64 {
        self.absolute_byte_offset
    }

    /// Если двоичные данные были обнаружены, то это возвращает абсолютное смещение байта,
    /// при котором изначально были найдены двоичные данные.
    fn binary_byte_offset(&self) -> Option<u64> {
        self.binary_byte_offset
    }

    /// Вернуть содержимое этого буфера.
    fn buffer(&self) -> &[u8] {
        &self.buf[self.pos..self.last_lineterm]
    }

    /// Вернуть содержимое свободного пространства за концом буфера как
    /// изменяемый срез.
    fn free_buffer(&mut self) -> &mut [u8] {
        &mut self.buf[self.end..]
    }

    /// Потребить указанное количество байтов. Это должно быть меньше или равно
    /// количеству байтов, возвращаемых `buffer`.
    fn consume(&mut self, amt: usize) {
        assert!(amt <= self.buffer().len());
        self.pos += amt;
        self.absolute_byte_offset += amt as u64;
    }

    /// Потребляет остаток буфера. Последующие вызовы `buffer`
    /// гарантированно вернут пустой срез, пока буфер не будет заполнен снова.
    ///
    /// Это удобная функция для `consume(buffer.len())`.
    #[cfg(test)]
    fn consume_all(&mut self) {
        let amt = self.buffer().len();
        self.consume(amt);
    }

    /// Заполнить содержимое этого буфера, отбрасывая ту часть буфера,
    /// которая была потреблена. Свободное пространство, созданное отбрасыванием
    /// потреблённой части буфера, затем заполняется новыми данными от указанного
    /// читателя.
    ///
    /// Вызывающие должны предоставлять одного и того же читателя этому буферу строк в
    /// последующих вызовах fill. Другой читатель может быть использован только
    /// сразу после вызова `clear`.
    ///
    /// Если достигнут EOF, то возвращается `false`. В противном случае возвращается
    /// `true`. (Обратите внимание, что если обнаружение двоичных данных этого буфера строк установлено в
    /// `Quit`, то наличие двоичных данных приведёт к тому, что этот буфер
    /// будет вести себя так, как если бы он достиг EOF.)
    ///
    /// Это передаёт любые ошибки, возвращаемые `rdr`, а также вернёт ошибку,
    /// если буфер должен быть расширен сверх предела выделения, в соответствии
    /// со стратегией выделения буфера.
    fn fill<R: io::Read>(&mut self, mut rdr: R) -> Result<bool, io::Error> {
        // Если эвристика обнаружения двоичных данных говорит нам прекратить, как только
        // были обнаружены двоичные данные, то мы больше не читаем новые данные и достигаем EOF
        // после того, как текущий буфер был потреблён.
        if self.config.binary.is_quit() && self.binary_byte_offset.is_some() {
            return Ok(!self.buffer().is_empty());
        }

        self.roll();
        assert_eq!(self.pos, 0);
        loop {
            self.ensure_capacity()?;
            let readlen = rdr.read(self.free_buffer().as_bytes_mut())?;
            if readlen == 0 {
                // Мы закончили чтение навсегда только после того, как вызывающий
                // потребил всё.
                self.last_lineterm = self.end;
                return Ok(!self.buffer().is_empty());
            }

            // Получить изменяемое представление байтов, которые мы только что прочитали. Это
            // байты, на которых мы выполняем обнаружение двоичных данных, а также байты, которые мы
            // ищем для нахождения последнего терминатора строки. Нам нужен изменяемый срез
            // в случае преобразования двоичных данных.
            let oldend = self.end;
            self.end += readlen;
            let newbytes = &mut self.buf[oldend..self.end];

            // Обнаружение двоичных данных.
            match self.config.binary {
                BinaryDetection::None => {} // ничего не делать
                BinaryDetection::Quit(byte) => {
                    if let Some(i) = newbytes.find_byte(byte) {
                        self.end = oldend + i;
                        self.last_lineterm = self.end;
                        self.binary_byte_offset =
                            Some(self.absolute_byte_offset + self.end as u64);
                        // Если первый байт в нашем буфере является двоичным байтом,
                        // то наш буфер пуст, и мы должны сообщить об этом
                        // вызывающему.
                        return Ok(self.pos < self.end);
                    }
                }
                BinaryDetection::Convert(byte) => {
                    if let Some(i) =
                        replace_bytes(newbytes, byte, self.config.lineterm)
                    {
                        // Записать только первое смещение двоичных данных.
                        if self.binary_byte_offset.is_none() {
                            self.binary_byte_offset = Some(
                                self.absolute_byte_offset
                                    + (oldend + i) as u64,
                            );
                        }
                    }
                }
            }

            // Обновить наши позиции `last_lineterm`, если мы прочитали один.
            if let Some(i) = newbytes.rfind_byte(self.config.lineterm) {
                self.last_lineterm = oldend + i + 1;
                return Ok(true);
            }
            // На этом этапе, если мы не смогли найти терминатор строки, то у нас
            // нет полной строки. Поэтому мы пытаемся прочитать больше!
        }
    }

    /// Прокрутить не потреблённые части буфера вперёд.
    ///
    /// Эта операция идемпотентна.
    ///
    /// После прокрутки `last_lineterm` и `end` указывают на одно и то же место,
    /// и `pos` всегда устанавливается в `0`.
    fn roll(&mut self) {
        if self.pos == self.end {
            self.pos = 0;
            self.last_lineterm = 0;
            self.end = 0;
            return;
        }

        let roll_len = self.end - self.pos;
        self.buf.copy_within(self.pos..self.end, 0);
        self.pos = 0;
        self.last_lineterm = roll_len;
        self.end = roll_len;
    }

    /// Гарантирует, что внутренний буфер имеет ненулевое количество свободного пространства
    /// для чтения большего количества данных. Если свободного пространства нет, то выделяется больше.
    /// Если выделение должно превысить настроенный предел, то
    /// это возвращает ошибку.
    fn ensure_capacity(&mut self) -> Result<(), io::Error> {
        if !self.free_buffer().is_empty() {
            return Ok(());
        }
        // `len` используется для вычисления следующего размера выделения. Ёмкость
        // разрешено начинать с `0`, поэтому мы убеждаемся, что она хотя бы `1`.
        let len = std::cmp::max(1, self.buf.len());
        let additional = match self.config.buffer_alloc {
            BufferAllocation::Eager => len * 2,
            BufferAllocation::Error(limit) => {
                let used = self.buf.len() - self.config.capacity;
                let n = std::cmp::min(len * 2, limit - used);
                if n == 0 {
                    return Err(alloc_error(self.config.capacity + limit));
                }
                n
            }
        };
        assert!(additional > 0);
        let newlen = self.buf.len() + additional;
        self.buf.resize(newlen, 0);
        assert!(!self.free_buffer().is_empty());
        Ok(())
    }
}

/// Заменяет `src` на `replacement` в байтах и возвращает смещение
/// первой замены, если таковая существует.
fn replace_bytes(
    mut bytes: &mut [u8],
    src: u8,
    replacement: u8,
) -> Option<usize> {
    if src == replacement {
        return None;
    }
    let first_pos = bytes.find_byte(src)?;
    bytes[first_pos] = replacement;
    bytes = &mut bytes[first_pos + 1..];
    while let Some(i) = bytes.find_byte(src) {
        bytes[i] = replacement;
        bytes = &mut bytes[i + 1..];

        // Для поиска смежных байтов `src` мы используем другую стратегию.
        // Поскольку двоичные данные склонны иметь длинные последовательности терминаторов NUL,
        // быстрее сравнивать по одному байту за раз, чем останавливаться и запускать
        // memchr (через `find_byte`) для каждого байта в последовательности.
        while bytes.get(0) == Some(&src) {
            bytes[0] = replacement;
            bytes = &mut bytes[1..];
        }
    }
    Some(first_pos)
}

#[cfg(test)]
mod tests {
    use bstr::ByteVec;

    use super::*;

    const SHERLOCK: &'static str = "\
For the Doctor Watsons of this world, as opposed to the Sherlock
Holmeses, success in the province of detective work must always
be, to a very large extent, the result of luck. Sherlock Holmes
can extract a clew from a wisp of straw or a flake of cigar ash;
but Doctor Watson has to have it taken out for him and dusted,
and exhibited clearly, with a label attached.\
";

    fn s(slice: &str) -> String {
        slice.to_string()
    }

    fn replace_str(
        slice: &str,
        src: u8,
        replacement: u8,
    ) -> (String, Option<usize>) {
        let mut dst = Vec::from(slice);
        let result = replace_bytes(&mut dst, src, replacement);
        (dst.into_string().unwrap(), result)
    }

    #[test]
    fn replace() {
        assert_eq!(replace_str("", b'b', b'z'), (s(""), None));
        assert_eq!(replace_str("a", b'a', b'a'), (s("a"), None));
        assert_eq!(replace_str("a", b'b', b'z'), (s("a"), None));
        assert_eq!(replace_str("abc", b'b', b'z'), (s("azc"), Some(1)));
        assert_eq!(replace_str("abb", b'b', b'z'), (s("azz"), Some(1)));
        assert_eq!(replace_str("aba", b'a', b'z'), (s("zbz"), Some(0)));
        assert_eq!(replace_str("bbb", b'b', b'z'), (s("zzz"), Some(0)));
        assert_eq!(replace_str("bac", b'b', b'z'), (s("zac"), Some(0)));
    }

    #[test]
    fn buffer_basics1() {
        let bytes = "homer\nlisa\nmaggie";
        let mut linebuf = LineBufferBuilder::new().build();
        let mut rdr = LineBufferReader::new(bytes.as_bytes(), &mut linebuf);

        assert!(rdr.buffer().is_empty());

        assert!(rdr.fill().unwrap());
        assert_eq!(rdr.bstr(), "homer\nlisa\n");
        assert_eq!(rdr.absolute_byte_offset(), 0);
        rdr.consume(5);
        assert_eq!(rdr.absolute_byte_offset(), 5);
        rdr.consume_all();
        assert_eq!(rdr.absolute_byte_offset(), 11);

        assert!(rdr.fill().unwrap());
        assert_eq!(rdr.bstr(), "maggie");
        rdr.consume_all();

        assert!(!rdr.fill().unwrap());
        assert_eq!(rdr.absolute_byte_offset(), bytes.len() as u64);
        assert_eq!(rdr.binary_byte_offset(), None);
    }

    #[test]
    fn buffer_basics2() {
        let bytes = "homer\nlisa\nmaggie\n";
        let mut linebuf = LineBufferBuilder::new().build();
        let mut rdr = LineBufferReader::new(bytes.as_bytes(), &mut linebuf);

        assert!(rdr.fill().unwrap());
        assert_eq!(rdr.bstr(), "homer\nlisa\nmaggie\n");
        rdr.consume_all();

        assert!(!rdr.fill().unwrap());
        assert_eq!(rdr.absolute_byte_offset(), bytes.len() as u64);
        assert_eq!(rdr.binary_byte_offset(), None);
    }

    #[test]
    fn buffer_basics3() {
        let bytes = "\n";
        let mut linebuf = LineBufferBuilder::new().build();
        let mut rdr = LineBufferReader::new(bytes.as_bytes(), &mut linebuf);

        assert!(rdr.fill().unwrap());
        assert_eq!(rdr.bstr(), "\n");
        rdr.consume_all();

        assert!(!rdr.fill().unwrap());
        assert_eq!(rdr.absolute_byte_offset(), bytes.len() as u64);
        assert_eq!(rdr.binary_byte_offset(), None);
    }

    #[test]
    fn buffer_basics4() {
        let bytes = "\n\n";
        let mut linebuf = LineBufferBuilder::new().build();
        let mut rdr = LineBufferReader::new(bytes.as_bytes(), &mut linebuf);

        assert!(rdr.fill().unwrap());
        assert_eq!(rdr.bstr(), "\n\n");
        rdr.consume_all();

        assert!(!rdr.fill().unwrap());
        assert_eq!(rdr.absolute_byte_offset(), bytes.len() as u64);
        assert_eq!(rdr.binary_byte_offset(), None);
    }

    #[test]
    fn buffer_empty() {
        let bytes = "";
        let mut linebuf = LineBufferBuilder::new().build();
        let mut rdr = LineBufferReader::new(bytes.as_bytes(), &mut linebuf);

        assert!(!rdr.fill().unwrap());
        assert_eq!(rdr.absolute_byte_offset(), bytes.len() as u64);
        assert_eq!(rdr.binary_byte_offset(), None);
    }

    #[test]
    fn buffer_zero_capacity() {
        let bytes = "homer\nlisa\nmaggie";
        let mut linebuf = LineBufferBuilder::new().capacity(0).build();
        let mut rdr = LineBufferReader::new(bytes.as_bytes(), &mut linebuf);

        while rdr.fill().unwrap() {
            rdr.consume_all();
        }
        assert_eq!(rdr.absolute_byte_offset(), bytes.len() as u64);
        assert_eq!(rdr.binary_byte_offset(), None);
    }

    #[test]
    fn buffer_small_capacity() {
        let bytes = "homer\nlisa\nmaggie";
        let mut linebuf = LineBufferBuilder::new().capacity(1).build();
        let mut rdr = LineBufferReader::new(bytes.as_bytes(), &mut linebuf);

        let mut got = vec![];
        while rdr.fill().unwrap() {
            got.push_str(rdr.buffer());
            rdr.consume_all();
        }
        assert_eq!(bytes, got.as_bstr());
        assert_eq!(rdr.absolute_byte_offset(), bytes.len() as u64);
        assert_eq!(rdr.binary_byte_offset(), None);
    }

    #[test]
    fn buffer_limited_capacity1() {
        let bytes = "homer\nlisa\nmaggie";
        let mut linebuf = LineBufferBuilder::new()
            .capacity(1)
            .buffer_alloc(BufferAllocation::Error(5))
            .build();
        let mut rdr = LineBufferReader::new(bytes.as_bytes(), &mut linebuf);

        assert!(rdr.fill().unwrap());
        assert_eq!(rdr.bstr(), "homer\n");
        rdr.consume_all();

        assert!(rdr.fill().unwrap());
        assert_eq!(rdr.bstr(), "lisa\n");
        rdr.consume_all();

        // This returns an error because while we have just enough room to
        // store maggie in the buffer, we *don't* have enough room to read one
        // more byte, so we don't know whether we're at EOF or not, and
        // therefore must give up.
        assert!(rdr.fill().is_err());

        // We can mush on though!
        assert_eq!(rdr.bstr(), "m");
        rdr.consume_all();

        assert!(rdr.fill().unwrap());
        assert_eq!(rdr.bstr(), "aggie");
        rdr.consume_all();

        assert!(!rdr.fill().unwrap());
    }

    #[test]
    fn buffer_limited_capacity2() {
        let bytes = "homer\nlisa\nmaggie";
        let mut linebuf = LineBufferBuilder::new()
            .capacity(1)
            .buffer_alloc(BufferAllocation::Error(6))
            .build();
        let mut rdr = LineBufferReader::new(bytes.as_bytes(), &mut linebuf);

        assert!(rdr.fill().unwrap());
        assert_eq!(rdr.bstr(), "homer\n");
        rdr.consume_all();

        assert!(rdr.fill().unwrap());
        assert_eq!(rdr.bstr(), "lisa\n");
        rdr.consume_all();

        // We have just enough space.
        assert!(rdr.fill().unwrap());
        assert_eq!(rdr.bstr(), "maggie");
        rdr.consume_all();

        assert!(!rdr.fill().unwrap());
    }

    #[test]
    fn buffer_limited_capacity3() {
        let bytes = "homer\nlisa\nmaggie";
        let mut linebuf = LineBufferBuilder::new()
            .capacity(1)
            .buffer_alloc(BufferAllocation::Error(0))
            .build();
        let mut rdr = LineBufferReader::new(bytes.as_bytes(), &mut linebuf);

        assert!(rdr.fill().is_err());
        assert_eq!(rdr.bstr(), "");
    }

    #[test]
    fn buffer_binary_none() {
        let bytes = "homer\nli\x00sa\nmaggie\n";
        let mut linebuf = LineBufferBuilder::new().build();
        let mut rdr = LineBufferReader::new(bytes.as_bytes(), &mut linebuf);

        assert!(rdr.buffer().is_empty());

        assert!(rdr.fill().unwrap());
        assert_eq!(rdr.bstr(), "homer\nli\x00sa\nmaggie\n");
        rdr.consume_all();

        assert!(!rdr.fill().unwrap());
        assert_eq!(rdr.absolute_byte_offset(), bytes.len() as u64);
        assert_eq!(rdr.binary_byte_offset(), None);
    }

    #[test]
    fn buffer_binary_quit1() {
        let bytes = "homer\nli\x00sa\nmaggie\n";
        let mut linebuf = LineBufferBuilder::new()
            .binary_detection(BinaryDetection::Quit(b'\x00'))
            .build();
        let mut rdr = LineBufferReader::new(bytes.as_bytes(), &mut linebuf);

        assert!(rdr.buffer().is_empty());

        assert!(rdr.fill().unwrap());
        assert_eq!(rdr.bstr(), "homer\nli");
        rdr.consume_all();

        assert!(!rdr.fill().unwrap());
        assert_eq!(rdr.absolute_byte_offset(), 8);
        assert_eq!(rdr.binary_byte_offset(), Some(8));
    }

    #[test]
    fn buffer_binary_quit2() {
        let bytes = "\x00homer\nlisa\nmaggie\n";
        let mut linebuf = LineBufferBuilder::new()
            .binary_detection(BinaryDetection::Quit(b'\x00'))
            .build();
        let mut rdr = LineBufferReader::new(bytes.as_bytes(), &mut linebuf);

        assert!(!rdr.fill().unwrap());
        assert_eq!(rdr.bstr(), "");
        assert_eq!(rdr.absolute_byte_offset(), 0);
        assert_eq!(rdr.binary_byte_offset(), Some(0));
    }

    #[test]
    fn buffer_binary_quit3() {
        let bytes = "homer\nlisa\nmaggie\n\x00";
        let mut linebuf = LineBufferBuilder::new()
            .binary_detection(BinaryDetection::Quit(b'\x00'))
            .build();
        let mut rdr = LineBufferReader::new(bytes.as_bytes(), &mut linebuf);

        assert!(rdr.buffer().is_empty());

        assert!(rdr.fill().unwrap());
        assert_eq!(rdr.bstr(), "homer\nlisa\nmaggie\n");
        rdr.consume_all();

        assert!(!rdr.fill().unwrap());
        assert_eq!(rdr.absolute_byte_offset(), bytes.len() as u64 - 1);
        assert_eq!(rdr.binary_byte_offset(), Some(bytes.len() as u64 - 1));
    }

    #[test]
    fn buffer_binary_quit4() {
        let bytes = "homer\nlisa\nmaggie\x00\n";
        let mut linebuf = LineBufferBuilder::new()
            .binary_detection(BinaryDetection::Quit(b'\x00'))
            .build();
        let mut rdr = LineBufferReader::new(bytes.as_bytes(), &mut linebuf);

        assert!(rdr.buffer().is_empty());

        assert!(rdr.fill().unwrap());
        assert_eq!(rdr.bstr(), "homer\nlisa\nmaggie");
        rdr.consume_all();

        assert!(!rdr.fill().unwrap());
        assert_eq!(rdr.absolute_byte_offset(), bytes.len() as u64 - 2);
        assert_eq!(rdr.binary_byte_offset(), Some(bytes.len() as u64 - 2));
    }

    #[test]
    fn buffer_binary_quit5() {
        let mut linebuf = LineBufferBuilder::new()
            .binary_detection(BinaryDetection::Quit(b'u'))
            .build();
        let mut rdr = LineBufferReader::new(SHERLOCK.as_bytes(), &mut linebuf);

        assert!(rdr.buffer().is_empty());

        assert!(rdr.fill().unwrap());
        assert_eq!(
            rdr.bstr(),
            "\
For the Doctor Watsons of this world, as opposed to the Sherlock
Holmeses, s\
"
        );
        rdr.consume_all();

        assert!(!rdr.fill().unwrap());
        assert_eq!(rdr.absolute_byte_offset(), 76);
        assert_eq!(rdr.binary_byte_offset(), Some(76));
        assert_eq!(SHERLOCK.as_bytes()[76], b'u');
    }

    #[test]
    fn buffer_binary_convert1() {
        let bytes = "homer\nli\x00sa\nmaggie\n";
        let mut linebuf = LineBufferBuilder::new()
            .binary_detection(BinaryDetection::Convert(b'\x00'))
            .build();
        let mut rdr = LineBufferReader::new(bytes.as_bytes(), &mut linebuf);

        assert!(rdr.buffer().is_empty());

        assert!(rdr.fill().unwrap());
        assert_eq!(rdr.bstr(), "homer\nli\nsa\nmaggie\n");
        rdr.consume_all();

        assert!(!rdr.fill().unwrap());
        assert_eq!(rdr.absolute_byte_offset(), bytes.len() as u64);
        assert_eq!(rdr.binary_byte_offset(), Some(8));
    }

    #[test]
    fn buffer_binary_convert2() {
        let bytes = "\x00homer\nlisa\nmaggie\n";
        let mut linebuf = LineBufferBuilder::new()
            .binary_detection(BinaryDetection::Convert(b'\x00'))
            .build();
        let mut rdr = LineBufferReader::new(bytes.as_bytes(), &mut linebuf);

        assert!(rdr.buffer().is_empty());

        assert!(rdr.fill().unwrap());
        assert_eq!(rdr.bstr(), "\nhomer\nlisa\nmaggie\n");
        rdr.consume_all();

        assert!(!rdr.fill().unwrap());
        assert_eq!(rdr.absolute_byte_offset(), bytes.len() as u64);
        assert_eq!(rdr.binary_byte_offset(), Some(0));
    }

    #[test]
    fn buffer_binary_convert3() {
        let bytes = "homer\nlisa\nmaggie\n\x00";
        let mut linebuf = LineBufferBuilder::new()
            .binary_detection(BinaryDetection::Convert(b'\x00'))
            .build();
        let mut rdr = LineBufferReader::new(bytes.as_bytes(), &mut linebuf);

        assert!(rdr.buffer().is_empty());

        assert!(rdr.fill().unwrap());
        assert_eq!(rdr.bstr(), "homer\nlisa\nmaggie\n\n");
        rdr.consume_all();

        assert!(!rdr.fill().unwrap());
        assert_eq!(rdr.absolute_byte_offset(), bytes.len() as u64);
        assert_eq!(rdr.binary_byte_offset(), Some(bytes.len() as u64 - 1));
    }

    #[test]
    fn buffer_binary_convert4() {
        let bytes = "homer\nlisa\nmaggie\x00\n";
        let mut linebuf = LineBufferBuilder::new()
            .binary_detection(BinaryDetection::Convert(b'\x00'))
            .build();
        let mut rdr = LineBufferReader::new(bytes.as_bytes(), &mut linebuf);

        assert!(rdr.buffer().is_empty());

        assert!(rdr.fill().unwrap());
        assert_eq!(rdr.bstr(), "homer\nlisa\nmaggie\n\n");
        rdr.consume_all();

        assert!(!rdr.fill().unwrap());
        assert_eq!(rdr.absolute_byte_offset(), bytes.len() as u64);
        assert_eq!(rdr.binary_byte_offset(), Some(bytes.len() as u64 - 2));
    }
}
