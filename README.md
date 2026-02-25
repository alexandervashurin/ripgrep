ripgrep (rg)
------------
ripgrep — это ориентированный на поиск по строкам инструмент, который рекурсивно
ищет в текущем каталоге шаблон регулярного выражения. По умолчанию ripgrep
уважает правила gitignore и автоматически пропускает скрытые файлы/каталоги и
бинарные файлы. (Чтобы отключить всю автоматическую фильтрацию по умолчанию,
используйте `rg -uuu`.) ripgrep имеет первоклассную поддержку Windows, macOS и
Linux, с бинарными загрузками для [каждого
релиза](https://github.com/BurntSushi/ripgrep/releases). ripgrep похож на другие
популярные инструменты поиска, такие как The Silver Searcher, ack и grep.

[![Статус сборки](https://github.com/BurntSushi/ripgrep/workflows/ci/badge.svg)](https://github.com/BurntSushi/ripgrep/actions)
[![Crates.io](https://img.shields.io/crates/v/ripgrep.svg)](https://crates.io/crates/ripgrep)
[![Статус пакетирования](https://repology.org/badge/tiny-repos/ripgrep.svg)](https://repology.org/project/ripgrep/badges)

Двойное лицензирование под MIT или [UNLICENSE](https://unlicense.org).


### CHANGELOG

Пожалуйста, смотрите [CHANGELOG](CHANGELOG.md) для истории релизов.

### Быстрые ссылки на документацию

* [Установка](#installation)
* [Руководство пользователя](GUIDE.md)
* [Часто задаваемые вопросы](FAQ.md)
* [Синтаксис регулярных выражений](https://docs.rs/regex/1/regex/#syntax)
* [Файлы конфигурации](GUIDE.md#configuration-file)
* [Автодополнение оболочки](FAQ.md#complete)
* [Сборка](#building)
* [Переводы](#translations)


### Скриншот результатов поиска

[![Скриншот примера поиска с ripgrep](https://burntsushi.net/stuff/ripgrep1.png)](https://burntsushi.net/stuff/ripgrep1.png)


### Быстрые примеры сравнения инструментов

Этот пример ищет во всем
[исходном дереве ядра Linux](https://github.com/BurntSushi/linux)
(после запуска `make defconfig && make -j8`) шаблон `[A-Z]+_SUSPEND`, где
все совпадения должны быть словами. Замеры времени были собраны на системе с
Intel i9-12900K 5,2 ГГц.

Пожалуйста, помните, что одного бенчмарка никогда не достаточно! Смотрите мой
[пост в блоге о ripgrep](https://blog.burntsushi.net/ripgrep/)
для очень подробного сравнения с большим количеством бенчмарков и анализом.

| Инструмент | Команда | Количество строк | Время |
| ---- | ------- | ---------- | ---- |
| ripgrep (Unicode) | `rg -n -w '[A-Z]+_SUSPEND'` | 536 | **0,082с** (1,00x) |
| [hypergrep](https://github.com/p-ranav/hypergrep) | `hgrep -n -w '[A-Z]+_SUSPEND'` | 536 | 0,167с (2,04x) |
| [git grep](https://www.kernel.org/pub/software/scm/git/docs/git-grep.html) | `git grep -P -n -w '[A-Z]+_SUSPEND'` | 536 | 0,273с (3,34x) |
| [The Silver Searcher](https://github.com/ggreer/the_silver_searcher) | `ag -w '[A-Z]+_SUSPEND'` | 534 | 0,443с (5,43x) |
| [ugrep](https://github.com/Genivia/ugrep) | `ugrep -r --ignore-files --no-hidden -I -w '[A-Z]+_SUSPEND'` | 536 | 0,639с (7,82x) |
| [git grep](https://www.kernel.org/pub/software/scm/git/docs/git-grep.html) | `LC_ALL=C git grep -E -n -w '[A-Z]+_SUSPEND'` | 536 | 0,727с (8,91x) |
| [git grep (Unicode)](https://www.kernel.org/pub/software/scm/git/docs/git-grep.html) | `LC_ALL=en_US.UTF-8 git grep -E -n -w '[A-Z]+_SUSPEND'` | 536 | 2,670с (32,70x) |
| [ack](https://github.com/beyondgrep/ack3) | `ack -w '[A-Z]+_SUSPEND'` | 2677 | 2,935с (35,94x) |

Вот еще один бенчмарк на том же корпусе, что и выше, который игнорирует файлы
gitignore и вместо этого использует белый список. Корпус тот же, что и в
предыдущем бенчмарке, а флаги, переданные каждой команде, гарантируют, что они
выполняют эквивалентную работу:

| Инструмент | Команда | Количество строк | Время |
| ---- | ------- | ---------- | ---- |
| ripgrep | `rg -uuu -tc -n -w '[A-Z]+_SUSPEND'` | 447 | **0,063с** (1,00x) |
| [ugrep](https://github.com/Genivia/ugrep) | `ugrep -r -n --include='*.c' --include='*.h' -w '[A-Z]+_SUSPEND'` | 447 | 0,607с (9,62x) |
| [GNU grep](https://www.gnu.org/software/grep/) | `grep -E -r -n --include='*.c' --include='*.h' -w '[A-Z]+_SUSPEND'` | 447 | 0,674с (10,69x) |

Теперь перейдем к поиску в одном большом файле. Вот прямое сравнение между
ripgrep, ugrep и GNU grep на файле в кэше памяти (~13 ГБ,
[`OpenSubtitles.raw.en.gz`](http://opus.nlpl.eu/download.php?f=OpenSubtitles/v2018/mono/OpenSubtitles.raw.en.gz), распакованный):

| Инструмент | Команда | Количество строк | Время |
| ---- | ------- | ---------- | ---- |
| ripgrep (Unicode) | `rg -w 'Sherlock [A-Z]\w+'` | 7882 | **1,042с** (1,00x) |
| [ugrep](https://github.com/Genivia/ugrep) | `ugrep -w 'Sherlock [A-Z]\w+'` | 7882 | 1,339с (1,28x) |
| [GNU grep (Unicode)](https://www.gnu.org/software/grep/) | `LC_ALL=en_US.UTF-8 egrep -w 'Sherlock [A-Z]\w+'` | 7882 | 6,577с (6,31x) |

В приведенном выше бенчмарке передача флага `-n` (для отображения номеров строк)
увеличивает время до `1,664с` для ripgrep и `9,484с` для GNU grep. На времена
ugrep не влияет наличие или отсутствие `-n`.

Однако остерегайтесь падения производительности:

| Инструмент | Команда | Количество строк | Время |
| ---- | ------- | ---------- | ---- |
| ripgrep (Unicode) | `rg -w '[A-Z]\w+ Sherlock [A-Z]\w+'` | 485 | **1,053с** (1,00x) |
| [GNU grep (Unicode)](https://www.gnu.org/software/grep/) | `LC_ALL=en_US.UTF-8 grep -E -w '[A-Z]\w+ Sherlock [A-Z]\w+'` | 485 | 6,234с (5,92x) |
| [ugrep](https://github.com/Genivia/ugrep) | `ugrep -w '[A-Z]\w+ Sherlock [A-Z]\w+'` | 485 | 28,973с (27,51x) |

И производительность может резко упасть повсеместно при поиске в больших файлах
шаблонов без каких-либо возможностей для оптимизаций по буквальным совпадениям:

| Инструмент | Команда | Количество строк | Время |
| ---- | ------- | ---------- | ---- |
| ripgrep | `rg '[A-Za-z]{30}'` | 6749 | **15,569с** (1,00x) |
| [ugrep](https://github.com/Genivia/ugrep) | `ugrep -E '[A-Za-z]{30}'` | 6749 | 21,857с (1,40x) |
| [GNU grep](https://www.gnu.org/software/grep/) | `LC_ALL=C grep -E '[A-Za-z]{30}'` | 6749 | 32,409с (2,08x) |
| [GNU grep (Unicode)](https://www.gnu.org/software/grep/) | `LC_ALL=en_US.UTF-8 grep -E '[A-Za-z]{30}'` | 6795 | 8м30с (32,74x) |

Наконец, большое количество совпадений также имеет тенденцию снижать
производительность и сглаживать различия между инструментами (потому что
производительность в целом определяется тем, насколько быстро можно обработать
совпадение, а не алгоритмом, используемым для обнаружения совпадения):

| Инструмент | Команда | Количество строк | Время |
| ---- | ------- | ---------- | ---- |
| ripgrep | `rg the` | 83499915 | **6,948с** (1,00x) |
| [ugrep](https://github.com/Genivia/ugrep) | `ugrep the` | 83499915 | 11,721с (1,69x) |
| [GNU grep](https://www.gnu.org/software/grep/) | `LC_ALL=C grep the` | 83499915 | 15,217с (2,19x) |

### Почему я должен использовать ripgrep?

* Он может заменить множество сценариев использования других инструментов поиска,
  потому что содержит большинство их функций и, как правило, быстрее. (Смотрите
  [FAQ](FAQ.md#posix4ever) для получения более подробной информации о том, может
  ли ripgrep действительно заменить grep.)
* Как и другие инструменты, специализирующиеся на поиске кода, ripgrep по умолчанию
  выполняет [рекурсивный поиск](GUIDE.md#recursive-search) и применяет
  [автоматическую фильтрацию](GUIDE.md#automatic-filtering). А именно, ripgrep не
  ищет файлы, игнорируемые вашими `.gitignore`/`.ignore`/`.rgignore` файлами, не
  ищет скрытые файлы и не ищет бинарные файлы. Автоматическую фильтрацию можно
  отключить с помощью `rg -uuu`.
* ripgrep может [искать файлы определенных типов](GUIDE.md#manual-filtering-file-types).
  Например, `rg -tpy foo` ограничивает поиск файлами Python, а `rg -Tjs foo`
  исключает файлы JavaScript из поиска. ripgrep можно обучить новым типам файлов
  с помощью пользовательских правил сопоставления.
* ripgrep поддерживает множество функций, найденных в `grep`, таких как отображение
  контекста результатов поиска, поиск по нескольким шаблонам, подсветка совпадений
  цветом и полная поддержка Unicode. В отличие от GNU grep, ripgrep остается
  быстрым при поддержке Unicode (который всегда включен).
* ripgrep имеет опциональную поддержку переключения своего движка регулярных
  выражений на использование PCRE2. Среди прочего, это позволяет использовать
  просмотр вперед/назад и обратные ссылки в ваших шаблонах, которые не
  поддерживаются в движке регулярных выражений ripgrep по умолчанию. Поддержку
  PCRE2 можно включить с помощью `-P/--pcre2` (всегда использовать PCRE2) или
  `--auto-hybrid-regex` (использовать PCRE2 только при необходимости). Альтернативный
  синтаксис предоставляется через опцию `--engine (default|pcre2|auto)`.
* ripgrep имеет [ rudimentary поддержку замен](GUIDE.md#replacements), которые
  позволяют переписывать вывод на основе того, что было найдено.
* ripgrep поддерживает [поиск файлов в текстовых кодировках](GUIDE.md#file-encoding),
  отличных от UTF-8, таких как UTF-16, latin-1, GBK, EUC-JP, Shift_JIS и другие.
  (Некоторая поддержка автоматического определения UTF-16 предусмотрена. Другие
  текстовые кодировки должны быть указаны с помощью флага `-E/--encoding`.)
* ripgrep поддерживает поиск в сжатых файлах распространенных форматов (brotli,
  bzip2, gzip, lz4, lzma, xz или zstandard) с помощью флага `-z/--search-zip`.
* ripgrep поддерживает [произвольную предварительную обработку ввода](GUIDE.md#preprocessor),
  такую как извлечение текста из PDF, декомпрессия с поддержкой less, дешифрование,
  автоматическое определение кодировки и так далее.
* ripgrep можно настроить через [файл конфигурации](GUIDE.md#configuration-file).

Другими словами, используйте ripgrep, если вам нравятся скорость, фильтрация по
умолчанию, меньшее количество ошибок и поддержка Unicode.


### Почему я не должен использовать ripgrep?

Несмотря на первоначальное нежелание добавлять в ripgrep все функции под солнцем,
со временем ripgrep получил поддержку большинства функций, найденных в других
инструментах поиска файлов. Это включает поиск результатов, охватывающих несколько
строк, и опциональную поддержку PCRE2, которая обеспечивает поддержку просмотра
окружения и обратных ссылок.

На данный момент основные причины не использовать ripgrep, вероятно, состоят в
одном или нескольких из следующего:

* Вам нужен портативный и повсеместный инструмент. Хотя ripgrep работает в Windows,
  macOS и Linux, он не является повсеместным и не соответствует какому-либо
  стандарту, такому как POSIX. Лучший инструмент для этой работы — старый добрый grep.
* Существует какая-то другая функция (или ошибка), не указанная в этом README, на
  которую вы полагаетесь и которая есть в другом инструменте, но нет в ripgrep.
* Существует случай производительности, когда ripgrep работает плохо, а другой
  инструмент работает хорошо. (Пожалуйста, сообщите об ошибке!)
* ripgrep невозможно установить на вашей машине или он недоступен для вашей
  платформы. (Пожалуйста, сообщите об ошибке!)


### Действительно ли он быстрее всего остального?

Как правило, да. Большое количество бенчмарков с подробным анализом каждого
[доступно в моем блоге](https://blog.burntsushi.net/ripgrep/).

Обобщая, ripgrep быстр, потому что:

* Он построен на основе [движка регулярных выражений Rust](https://github.com/rust-lang/regex).
  Движок регулярных выражений Rust использует конечные автоматы, SIMD и агрессивные
  оптимизации буквенных совпадений, чтобы сделать поиск очень быстрым. (Поддержку
  PCRE2 можно включить с помощью флага `-P/--pcre2`.)
* Библиотека регулярных выражений Rust поддерживает производительность с полной
  поддержкой Unicode, встраивая декодирование UTF-8 непосредственно в свой движок
  детерминированных конечных автоматов.
* Он поддерживает поиск либо с помощью отображения памяти, либо с помощью
  инкрементального поиска с промежуточным буфером. Первый лучше для отдельных
  файлов, а второй — для больших каталогов. ripgrep автоматически выбирает для
  вас лучшую стратегию поиска.
* Применяет ваши шаблоны игнорирования в файлах `.gitignore` с помощью
  [`RegexSet`](https://docs.rs/regex/1/regex/struct.RegexSet.html). Это означает,
  что один путь к файлу может быть сопоставлен с несколькими шаблонами glob
  одновременно.
* Он использует параллельный рекурсивный обход каталогов без блокировок, благодаря
  [`crossbeam`](https://docs.rs/crossbeam) и [`ignore`](https://docs.rs/ignore).


### Сравнение функций

Энди Лестер, автор [ack](https://beyondgrep.com/), опубликовал отличную таблицу,
сравнивающую функции ack, ag, git-grep, GNU grep и ripgrep:
https://beyondgrep.com/feature-comparison/

Обратите внимание, что ripgrep получил несколько значительных новых функций,
которые недавно не были представлены в таблице Энди. Это включает, но не
ограничивается, файлы конфигурации, passthru, поддержку поиска в сжатых файлах,
многострочный поиск и опциональную поддержку сложных регулярных выражений через PCRE2.


### Площадка для тестирования

Если вы хотите попробовать ripgrep перед установкой, существует неофициальная
[площадка](https://codapi.org/ripgrep/) и [интерактивное руководство](https://codapi.org/try/ripgrep/).

Если у вас есть какие-либо вопросы по ним, пожалуйста, откройте issue в
[репозитории руководства](https://github.com/nalgeon/tryxinyminutes).


### Установка

Имя бинарного файла для ripgrep — `rg`.

**[Архивы предварительно скомпилированных бинарных файлов ripgrep доступны для
Windows, macOS и Linux.](https://github.com/BurntSushi/ripgrep/releases)**
Linux и Windows бинарники являются статическими исполняемыми файлами.
Пользователям платформ, явно не упомянутых ниже, рекомендуется загрузить один
из этих архивов.

Если вы пользователь **macOS Homebrew** или **Linuxbrew**, то можете установить
ripgrep из homebrew-core:

```
$ brew install ripgrep
```

Если вы пользователь **MacPorts**, то можете установить ripgrep из
[официальных портов](https://www.macports.org/ports.php?by=name&substr=ripgrep):

```
$ sudo port install ripgrep
```

Если вы пользователь **Windows Chocolatey**, то можете установить ripgrep из
[официального репозитория](https://chocolatey.org/packages/ripgrep):

```
$ choco install ripgrep
```

Если вы пользователь **Windows Scoop**, то можете установить ripgrep из
[официального бакета](https://github.com/ScoopInstaller/Main/blob/master/bucket/ripgrep.json):

```
$ scoop install ripgrep
```

Если вы пользователь **Windows Winget**, то можете установить ripgrep из
репозитория [winget-pkgs](https://github.com/microsoft/winget-pkgs/tree/master/manifests/b/BurntSushi/ripgrep):

```
$ winget install BurntSushi.ripgrep.MSVC
```

Если вы пользователь **Arch Linux**, то можете установить ripgrep из официальных репозиториев:

```
$ sudo pacman -S ripgrep
```

Если вы пользователь **Gentoo**, вы можете установить ripgrep из
[официального репозитория](https://packages.gentoo.org/packages/sys-apps/ripgrep):

```
$ sudo emerge sys-apps/ripgrep
```

Если вы пользователь **Fedora**, вы можете установить ripgrep из официальных
репозиториев.

```
$ sudo dnf install ripgrep
```

Если вы пользователь **openSUSE**, ripgrep включен в **openSUSE Tumbleweed** и
**openSUSE Leap** начиная с 15.1.

```
$ sudo zypper install ripgrep
```

Если вы пользователь **CentOS Stream 10**, вы можете установить ripgrep из
репозитория [EPEL](https://docs.fedoraproject.org/en-US/epel/getting-started/):

```
$ sudo dnf config-manager --set-enabled crb
$ sudo dnf install https://dl.fedoraproject.org/pub/epel/epel-release-latest-10.noarch.rpm
$ sudo dnf install ripgrep
```

Если вы пользователь **Red Hat 10**, вы можете установить ripgrep из
репозитория [EPEL](https://docs.fedoraproject.org/en-US/epel/getting-started/):

```
$ sudo subscription-manager repos --enable codeready-builder-for-rhel-10-$(arch)-rpms
$ sudo dnf install https://dl.fedoraproject.org/pub/epel/epel-release-latest-10.noarch.rpm
$ sudo dnf install ripgrep
```

Если вы пользователь **Rocky Linux 10**, вы можете установить ripgrep из
репозитория [EPEL](https://docs.fedoraproject.org/en-US/epel/getting-started/):

```
$ sudo dnf install https://dl.fedoraproject.org/pub/epel/epel-release-latest-10.noarch.rpm
$ sudo dnf install ripgrep
```

Если вы пользователь **Nix**, вы можете установить ripgrep из [nixpkgs](https://github.com/NixOS/nixpkgs/blob/master/pkgs/by-name/ri/ripgrep/package.nix):

```
$ nix-env --install ripgrep
```

Если вы пользователь **Flox**, вы можете установить ripgrep следующим образом:

```
$ flox install ripgrep
```

Если вы пользователь **Guix**, вы можете установить ripgrep из официальной
коллекции пакетов:

```
$ guix install ripgrep
```

Если вы пользователь **Debian** (или пользователь производного от Debian дистрибутива,
такого как **Ubuntu**), то ripgrep можно установить с помощью бинарного `.deb` файла,
предоставленного в каждом [релизе ripgrep](https://github.com/BurntSushi/ripgrep/releases).

```
$ curl -LO https://github.com/BurntSushi/ripgrep/releases/download/14.1.1/ripgrep_14.1.1-1_amd64.deb
$ sudo dpkg -i ripgrep_14.1.1-1_amd64.deb
```

Если вы используете Debian stable, ripgrep [официально поддерживается Debian](https://tracker.debian.org/pkg/rust-ripgrep),
хотя его версия может быть старше, чем `deb` пакет, доступный на предыдущем шаге.

```
$ sudo apt-get install ripgrep
```

Если вы пользователь **Ubuntu Cosmic (18.10)** (или новее), ripgrep [доступен](https://launchpad.net/ubuntu/+source/rust-ripgrep)
с использованием той же упаковки, что и Debian:

```
$ sudo apt-get install ripgrep
```

(N.B. Различные снапы для ripgrep на Ubuntu также доступны, но ни один из них,
похоже, не работает правильно и генерирует множество очень странных отчетов об
ошибках, которые я не знаю, как исправить и у меня нет времени исправлять.
Поэтому это больше не рекомендуемый вариант установки.)

Если вы пользователь **ALT**, вы можете установить ripgrep из
[официального репозитория](https://packages.altlinux.org/en/search?name=ripgrep):

```
$ sudo apt-get install ripgrep
```

Если вы пользователь **FreeBSD**, то можете установить ripgrep из
[официальных портов](https://www.freshports.org/textproc/ripgrep/):

```
$ sudo pkg install ripgrep
```

Если вы пользователь **OpenBSD**, то можете установить ripgrep из
[официальных портов](https://openports.se/textproc/ripgrep):

```
$ doas pkg_add ripgrep
```

Если вы пользователь **NetBSD**, то можете установить ripgrep из [pkgsrc](https://pkgsrc.se/textproc/ripgrep):

```
$ sudo pkgin install ripgrep
```

Если вы пользователь **Haiku x86_64**, то можете установить ripgrep из
[официальных портов](https://github.com/haikuports/haikuports/tree/master/sys-apps/ripgrep):

```
$ sudo pkgman install ripgrep
```

Если вы пользователь **Haiku x86_gcc2**, то можете установить ripgrep из того же
порта, что и Haiku x86_64, используя сборку вторичной архитектуры x86:

```
$ sudo pkgman install ripgrep_x86
```

Если вы пользователь **Void Linux**, то можете установить ripgrep из
[официального репозитория](https://voidlinux.org/packages/?arch=x86_64&q=ripgrep):

```
$ sudo xbps-install -Syv ripgrep
```

Если вы **Rust программист**, ripgrep можно установить с помощью `cargo`.

* Обратите внимание, что минимальная поддерживаемая версия Rust для ripgrep —
  **1.85.0**, хотя ripgrep может работать и с более старыми версиями.
* Обратите внимание, что бинарный файл может быть больше ожидаемого, потому что
  он содержит отладочные символы. Это сделано намеренно. Чтобы удалить отладочные
  символы и, следовательно, уменьшить размер файла, запустите `strip` на бинарном файле.

```
$ cargo install ripgrep
```

В качестве альтернативы можно использовать [`cargo binstall`](https://github.com/cargo-bins/cargo-binstall)
для установки бинарного файла ripgrep напрямую из GitHub:

```
$ cargo binstall ripgrep
```


### Сборка

ripgrep написан на Rust, поэтому вам понадобится [установка Rust](https://www.rust-lang.org/)
для его компиляции. ripgrep компилируется с Rust 1.85.0 (stable) или новее. В целом,
ripgrep отслеживает последний стабильный релиз компилятора Rust.

Для сборки ripgrep:

```
$ git clone https://github.com/BurntSushi/ripgrep
$ cd ripgrep
$ cargo build --release
$ ./target/release/rg --version
0.1.3
```

**ПРИМЕЧАНИЕ:** В прошлом ripgrep поддерживал функцию Cargo `simd-accel` при
использовании ночного компилятора Rust. Это приносило пользу только перекодированию
UTF-16. Поскольку для этого требовались нестабильные функции, этот режим сборки
был склонен к поломкам. Из-за этого поддержка была удалена. Если вы хотите SIMD
оптимизации для перекодирования UTF-16, то вам придется попросить проект
[`encoding_rs`](https://github.com/hsivonen/encoding_rs) использовать стабильные API.

Наконец, опциональная поддержка PCRE2 может быть собрана с ripgrep путем включения
функции `pcre2`:

```
$ cargo build --release --features 'pcre2'
```

Включение функции PCRE2 работает со стабильным компилятором Rust и попытается
автоматически найти и связаться с системной библиотекой PCRE2 вашей системы через
`pkg-config`. Если таковой не существует, то ripgrep соберет PCRE2 из исходного
кода, используя системный C компилятор, и затем статически свяжет его с финальным
исполняемым файлом. Статическую линковку можно принудительно включить, даже если
доступна системная библиотека PCRE2, либо собрав ripgrep с целевой платформой MUSL,
либо установив `PCRE2_SYS_STATIC=1`.

ripgrep может быть собран с целевой платформой MUSL на Linux, сначала установив
библиотеку MUSL в вашей системе (обратитесь к вашему дружелюбному пакетному менеджеру).
Затем вам просто нужно добавить поддержку MUSL в ваш инструментальный набор Rust и
пересобрать ripgrep, что даст полностью статический исполняемый файл:

```
$ rustup target add x86_64-unknown-linux-musl
$ cargo build --release --target x86_64-unknown-linux-musl
```

Применение флага `--features` из выше работает как ожидалось. Если вы хотите собрать
статический исполняемый файл с MUSL и с PCRE2, то вам понадобится иметь установленный
`musl-gcc`, который может быть в отдельном пакете от самой библиотеки MUSL, в
зависимости от вашего дистрибутива Linux.


### Запуск тестов

ripgrep относительно хорошо протестирован, включая как модульные тесты, так и
интеграционные тесты. Для запуска полного набора тестов используйте:

```
$ cargo test --all
```

из корня репозитория.


### Связанные инструменты

* [delta](https://github.com/dandavison/delta) — это синтаксически подсвечивающий
пейджер, который поддерживает формат вывода `rg --json`. Поэтому все, что вам нужно
сделать, чтобы заставить его работать, это `rg --json pattern | delta`. Смотрите
[раздел руководства delta о grep](https://dandavison.github.io/delta/grep.html) для
более подробной информации.


### Сообщение об уязвимостях

Для сообщения об уязвимости безопасности, пожалуйста,
[свяжитесь с Эндрю Галлантом](https://blog.burntsushi.net/about/).
На странице контактов есть мой адрес электронной почты и публичный ключ PGP, если
вы хотите отправить зашифрованное сообщение.


### Переводы

Ниже приведен список известных переводов документации ripgrep. Они неофициально
поддерживаются и могут быть не актуальны.

* [Китайский](https://github.com/chinanf-boy/ripgrep-zh#%E6%9B%B4%E6%96%B0-)
* [Испанский](https://github.com/UltiRequiem/traducciones/tree/master/ripgrep)
