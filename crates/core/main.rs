/*!
Точка входа в ripgrep.
*/

use std::{io::Write, process::ExitCode};

use ignore::WalkState;

use crate::flags::{HiArgs, SearchMode};

#[macro_use]
mod messages;

mod flags;
mod haystack;
mod logger;
mod search;

// Поскольку Rust больше не использует jemalloc по умолчанию, ripgrep будет,
// по умолчанию, использовать системный аллокатор. В Linux это обычно будет
// аллокатор glibc, который довольно хорош. В частности, ripgrep не является
// особенно тяжелым по выделениям рабочей нагрузкой, поэтому на самом деле
// нет большой разницы (для целей ripgrep) между аллокатором glibc и jemalloc.
//
// Однако, когда ripgrep собран с musl, это означает, что ripgrep будет
// использовать аллокатор musl, который, по-видимому, значительно хуже.
// (Цель musl — не иметь самую быструю версию всего. Его цель — быть
// маленьким и пригодным для статической компиляции.) Даже though ripgrep
// не особенно тяжел по выделениям, аллокатор musl, по-видимому, довольно
// сильно замедляет ripgrep. Поэтому при сборке с musl мы используем jemalloc.
//
// Мы не используем jemalloc безусловно, потому что может быть приятно
// использовать аллокатор по умолчанию системы по умолчанию. Более того,
// jemalloc, по-видимому, увеличивает время компиляции на немного.
//
// Более того, мы делаем это только на 64-битных системах, поскольку
// jemalloc не поддерживает i686.
#[cfg(all(target_env = "musl", target_pointer_width = "64"))]
#[global_allocator]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

/// Тогда, как было, так и будет.
fn main() -> ExitCode {
    match run(flags::parse()) {
        Ok(code) => code,
        Err(err) => {
            // Ищем ошибку разрыва канала. В этом случае мы обычно хотим
            // выйти «грациозно» с кодом выхода успеха. Это соответствует
            // существующему соглашению Unix. Нам нужно обрабатывать это
            // явно, поскольку среда выполнения Rust не запрашивает сигналы
            // PIPE, и поэтому мы получаем ошибку I/O вместо этого.
            // Традиционные C Unix-приложения завершаются, получая сигнал
            // PIPE, который они не обрабатывают, и поэтому необработанный
            // сигнал заставляет процесс церемонно завершиться.
            for cause in err.chain() {
                if let Some(ioerr) = cause.downcast_ref::<std::io::Error>() {
                    if ioerr.kind() == std::io::ErrorKind::BrokenPipe {
                        return ExitCode::from(0);
                    }
                }
            }
            eprintln_locked!("{:#}", err);
            ExitCode::from(2)
        }
    }
}

/// Основная точка входа для ripgrep.
///
/// Данный результат разбора определяет поведение ripgrep. Результат разбора
/// должен быть результатом разбора аргументов CLI в низкоуровневом
/// представлении, а затем последующего преобразования их в представление
/// более высокого уровня. Представление более высокого уровня имеет некоторые
/// более приятные абстракции, например, вместо представления флага
/// `-g/--glob` как `Vec<String>` (как в низкоуровневом представлении),
/// глобы преобразуются в единый матчер.
fn run(result: crate::flags::ParseResult<HiArgs>) -> anyhow::Result<ExitCode> {
    use crate::flags::{Mode, ParseResult};

    let args = match result {
        ParseResult::Err(err) => return Err(err),
        ParseResult::Special(mode) => return special(mode),
        ParseResult::Ok(args) => args,
    };
    let matched = match args.mode() {
        Mode::Search(_) if !args.matches_possible() => false,
        Mode::Search(mode) if args.threads() == 1 => search(&args, mode)?,
        Mode::Search(mode) => search_parallel(&args, mode)?,
        Mode::Files if args.threads() == 1 => files(&args)?,
        Mode::Files => files_parallel(&args)?,
        Mode::Types => return types(&args),
        Mode::Generate(mode) => return generate(mode),
    };
    Ok(if matched && (args.quiet() || !messages::errored()) {
        ExitCode::from(0)
    } else if messages::errored() {
        ExitCode::from(2)
    } else {
        ExitCode::from(1)
    })
}

/// Точка входа верхнего уровня для однопоточного поиска.
///
/// Это рекурсивно проходит через список файлов (каталог по умолчанию)
/// и ищет каждый файл последовательно.
fn search(args: &HiArgs, mode: SearchMode) -> anyhow::Result<bool> {
    let started_at = std::time::Instant::now();
    let haystack_builder = args.haystack_builder();
    let unsorted = args
        .walk_builder()?
        .build()
        .filter_map(|result| haystack_builder.build_from_result(result));
    let haystacks = args.sort(unsorted);

    let mut matched = false;
    let mut searched = false;
    let mut stats = args.stats();
    let mut searcher = args.search_worker(
        args.matcher()?,
        args.searcher()?,
        args.printer(mode, args.stdout()),
    )?;
    for haystack in haystacks {
        searched = true;
        let search_result = match searcher.search(&haystack) {
            Ok(search_result) => search_result,
            // Разрыв канала означает грациозное завершение.
            Err(err) if err.kind() == std::io::ErrorKind::BrokenPipe => break,
            Err(err) => {
                err_message!("{}: {}", haystack.path().display(), err);
                continue;
            }
        };
        matched = matched || search_result.has_match();
        if let Some(ref mut stats) = stats {
            *stats += search_result.stats().unwrap();
        }
        if matched && args.quit_after_match() {
            break;
        }
    }
    if args.has_implicit_path() && !searched {
        eprint_nothing_searched();
    }
    if let Some(ref stats) = stats {
        let wtr = searcher.printer().get_mut();
        let _ = print_stats(mode, stats, started_at, wtr);
    }
    Ok(matched)
}

/// Точка входа верхнего уровня для многопоточного поиска.
///
/// Параллелизм сам по себе достигается рекурсивным обходом каталога.
/// Все, что нам нужно сделать, — это передать ему рабочего для выполнения
/// поиска по каждому файлу.
///
/// Запрос отсортированного вывода от ripgrep (например, с `--sort path`)
/// автоматически отключит параллелизм, и поэтому сортировка не обрабатывается
/// здесь.
fn search_parallel(args: &HiArgs, mode: SearchMode) -> anyhow::Result<bool> {
    use std::sync::atomic::{AtomicBool, Ordering};

    let started_at = std::time::Instant::now();
    let haystack_builder = args.haystack_builder();
    let bufwtr = args.buffer_writer();
    let stats = args.stats().map(std::sync::Mutex::new);
    let matched = AtomicBool::new(false);
    let searched = AtomicBool::new(false);

    let mut searcher = args.search_worker(
        args.matcher()?,
        args.searcher()?,
        args.printer(mode, bufwtr.buffer()),
    )?;
    args.walk_builder()?.build_parallel().run(|| {
        let bufwtr = &bufwtr;
        let stats = &stats;
        let matched = &matched;
        let searched = &searched;
        let haystack_builder = &haystack_builder;
        let mut searcher = searcher.clone();

        Box::new(move |result| {
            let haystack = match haystack_builder.build_from_result(result) {
                Some(haystack) => haystack,
                None => return WalkState::Continue,
            };
            searched.store(true, Ordering::SeqCst);
            searcher.printer().get_mut().clear();
            let search_result = match searcher.search(&haystack) {
                Ok(search_result) => search_result,
                Err(err) => {
                    err_message!("{}: {}", haystack.path().display(), err);
                    return WalkState::Continue;
                }
            };
            if search_result.has_match() {
                matched.store(true, Ordering::SeqCst);
            }
            if let Some(ref locked_stats) = *stats {
                let mut stats = locked_stats.lock().unwrap();
                *stats += search_result.stats().unwrap();
            }
            if let Err(err) = bufwtr.print(searcher.printer().get_mut()) {
                // Разрыв канала означает грациозное завершение.
                if err.kind() == std::io::ErrorKind::BrokenPipe {
                    return WalkState::Quit;
                }
                // В противном случае мы продолжаем свой путь.
                err_message!("{}: {}", haystack.path().display(), err);
            }
            if matched.load(Ordering::SeqCst) && args.quit_after_match() {
                WalkState::Quit
            } else {
                WalkState::Continue
            }
        })
    });
    if args.has_implicit_path() && !searched.load(Ordering::SeqCst) {
        eprint_nothing_searched();
    }
    if let Some(ref locked_stats) = stats {
        let stats = locked_stats.lock().unwrap();
        let mut wtr = searcher.printer().get_mut();
        let _ = print_stats(mode, &stats, started_at, &mut wtr);
        let _ = bufwtr.print(&mut wtr);
    }
    Ok(matched.load(Ordering::SeqCst))
}

/// Точка входа верхнего уровня для вывода списка файлов без поиска.
///
/// Это рекурсивно проходит через список файлов (каталог по умолчанию)
/// и печатает каждый путь последовательно с использованием одного потока.
fn files(args: &HiArgs) -> anyhow::Result<bool> {
    let haystack_builder = args.haystack_builder();
    let unsorted = args
        .walk_builder()?
        .build()
        .filter_map(|result| haystack_builder.build_from_result(result));
    let haystacks = args.sort(unsorted);

    let mut matched = false;
    let mut path_printer = args.path_printer_builder().build(args.stdout());
    for haystack in haystacks {
        matched = true;
        if args.quit_after_match() {
            break;
        }
        if let Err(err) = path_printer.write(haystack.path()) {
            // Разрыв канала означает грациозное завершение.
            if err.kind() == std::io::ErrorKind::BrokenPipe {
                break;
            }
            // В противном случае у нас есть какая-то другая ошибка, которая
            // мешает нам записывать в stdout, поэтому мы должны поднять ее.
            return Err(err.into());
        }
    }
    Ok(matched)
}

/// Точка входа верхнего уровня для многопоточного вывода списка файлов без
/// поиска.
///
/// Это рекурсивно проходит через список файлов (каталог по умолчанию)
/// и печатает каждый путь последовательно с использованием нескольких потоков.
///
/// Запрос отсортированного вывода от ripgrep (например, с `--sort path`)
/// автоматически отключит параллелизм, и поэтому сортировка не обрабатывается
/// здесь.
fn files_parallel(args: &HiArgs) -> anyhow::Result<bool> {
    use std::{
        sync::{
            atomic::{AtomicBool, Ordering},
            mpsc,
        },
        thread,
    };

    let haystack_builder = args.haystack_builder();
    let mut path_printer = args.path_printer_builder().build(args.stdout());
    let matched = AtomicBool::new(false);
    let (tx, rx) = mpsc::channel::<crate::haystack::Haystack>();

    // Мы порождаем единственный поток печати, чтобы убедиться, что мы не
    // разрываем записи. Мы используем канал здесь под предположением, что
    // это, вероятно, быстрее, чем использование мьютекса в рабочих потоках
    // ниже, но это никогда серьезно не обсуждалось.
    let print_thread = thread::spawn(move || -> std::io::Result<()> {
        for haystack in rx.iter() {
            path_printer.write(haystack.path())?;
        }
        Ok(())
    });
    args.walk_builder()?.build_parallel().run(|| {
        let haystack_builder = &haystack_builder;
        let matched = &matched;
        let tx = tx.clone();

        Box::new(move |result| {
            let haystack = match haystack_builder.build_from_result(result) {
                Some(haystack) => haystack,
                None => return WalkState::Continue,
            };
            matched.store(true, Ordering::SeqCst);
            if args.quit_after_match() {
                WalkState::Quit
            } else {
                match tx.send(haystack) {
                    Ok(_) => WalkState::Continue,
                    Err(_) => WalkState::Quit,
                }
            }
        })
    });
    drop(tx);
    if let Err(err) = print_thread.join().unwrap() {
        // Разрыв канала означает грациозное завершение, поэтому переходим.
        // В противном случае произошло что-то плохое при записи в stdout,
        // поэтому поднимаем ошибку.
        if err.kind() != std::io::ErrorKind::BrokenPipe {
            return Err(err.into());
        }
    }
    Ok(matched.load(Ordering::SeqCst))
}

/// Точка входа верхнего уровня для `--type-list`.
fn types(args: &HiArgs) -> anyhow::Result<ExitCode> {
    let mut count = 0;
    let mut stdout = args.stdout();
    for def in args.types().definitions() {
        count += 1;
        stdout.write_all(def.name().as_bytes())?;
        stdout.write_all(b": ")?;

        let mut first = true;
        for glob in def.globs() {
            if !first {
                stdout.write_all(b", ")?;
            }
            stdout.write_all(glob.as_bytes())?;
            first = false;
        }
        stdout.write_all(b"\n")?;
    }
    Ok(ExitCode::from(if count == 0 { 1 } else { 0 }))
}

/// Реализует режимы «генерации» ripgrep.
///
/// Эти режимы соответствуют генерации каких-либо вспомогательных данных,
/// связанных с ripgrep. В настоящее время это включает страницу руководства
/// ripgrep (в формате roff) и поддерживаемые автодополнения оболочки.
fn generate(mode: crate::flags::GenerateMode) -> anyhow::Result<ExitCode> {
    use crate::flags::GenerateMode;

    let output = match mode {
        GenerateMode::Man => flags::generate_man_page(),
        GenerateMode::CompleteBash => flags::generate_complete_bash(),
        GenerateMode::CompleteZsh => flags::generate_complete_zsh(),
        GenerateMode::CompleteFish => flags::generate_complete_fish(),
        GenerateMode::CompletePowerShell => {
            flags::generate_complete_powershell()
        }
    };
    writeln!(std::io::stdout(), "{}", output.trim_end())?;
    Ok(ExitCode::from(0))
}

/// Реализует «специальные» режимы ripgrep.
///
/// Специальный режим — это режим, который обычно коротко замыкает большую
/// часть (не всю) логику инициализации ripgrep и переходит сразу к этой
/// процедуре. Специальные режимы в основном состоят из вывода помощи и
/// версии. Идея короткого замыкания заключается в том, чтобы обеспечить
/// как можно меньше (в разумных пределах), что помешало бы ripgrep вывести
/// справку.
///
/// Например, частью логики инициализации, которая пропускается (среди
/// прочего), является доступ к текущему рабочему каталогу. Если это не
/// удается, ripgrep выдает ошибку. Мы не хотим выдавать ошибку, если это
/// не удается, и пользователь запросил информацию о версии или помощи.
fn special(mode: crate::flags::SpecialMode) -> anyhow::Result<ExitCode> {
    use crate::flags::SpecialMode;

    let mut exit = ExitCode::from(0);
    let output = match mode {
        SpecialMode::HelpShort => flags::generate_help_short(),
        SpecialMode::HelpLong => flags::generate_help_long(),
        SpecialMode::VersionShort => flags::generate_version_short(),
        SpecialMode::VersionLong => flags::generate_version_long(),
        // --pcre2-version is a little special because it emits an error
        // exit code if this build of ripgrep doesn't support PCRE2.
        SpecialMode::VersionPCRE2 => {
            let (output, available) = flags::generate_version_pcre2();
            if !available {
                exit = ExitCode::from(1);
            }
            output
        }
    };
    writeln!(std::io::stdout(), "{}", output.trim_end())?;
    Ok(exit)
}

/// Печатает эвристическое сообщение об ошибке, когда ничего не найдено.
///
/// Это может произойти, если применимый файл игнорирования имеет одно или
/// несколько правил, которые слишком широки и заставляют ripgrep игнорировать
/// все.
///
/// Мы показываем это сообщение об ошибке только тогда, когда пользователь
/// *не* предоставляет явный путь для поиска. Это потому, что сообщение в
/// противном случае может быть шумным, например, когда предполагается, что
/// искать нечего.
fn eprint_nothing_searched() {
    err_message!(
        "No files were searched, which means ripgrep probably \
         applied a filter you didn't expect.\n\
         Running with --debug will show why files are being skipped."
    );
}

/// Печатает данную статистику в данный писатель.
///
/// Данный режим поиска определяет, должна ли статистика печататься в
/// формате простого текста или в формате JSON.
///
/// Время `started` должно быть временем, в которое ripgrep начал работу.
///
/// Если возникает ошибка при записи, то запись останавливается и ошибка
/// возвращается. Обратите внимание, что вызывающие, вероятно, должны
/// игнорировать эту ошибку, поскольку то, не удается ли напечатать
/// статистику или нет, обычно не должно вызывать переход ripgrep в
/// состояние «ошибки». И обычно единственный способ для этого не
/// удасться — если сама запись в stdout не удается.
fn print_stats<W: Write>(
    mode: SearchMode,
    stats: &grep::printer::Stats,
    started: std::time::Instant,
    mut wtr: W,
) -> std::io::Result<()> {
    let elapsed = std::time::Instant::now().duration_since(started);
    if matches!(mode, SearchMode::JSON) {
        // Мы специально сопоставляем формат, изложенный JSON принтером в
        // крейте grep-printer. Мы просто «расширяем» его типом сообщения
        // 'summary'.
        serde_json::to_writer(
            &mut wtr,
            &serde_json::json!({
                "type": "summary",
                "data": {
                    "stats": stats,
                    "elapsed_total": {
                        "secs": elapsed.as_secs(),
                        "nanos": elapsed.subsec_nanos(),
                        "human": format!("{:0.6}s", elapsed.as_secs_f64()),
                    },
                }
            }),
        )?;
        write!(wtr, "\n")
    } else {
        write!(
            wtr,
            "
{matches} matches
{lines} matched lines
{searches_with_match} files contained matches
{searches} files searched
{bytes_printed} bytes printed
{bytes_searched} bytes searched
{search_time:0.6} seconds spent searching
{process_time:0.6} seconds total
",
            matches = stats.matches(),
            lines = stats.matched_lines(),
            searches_with_match = stats.searches_with_match(),
            searches = stats.searches(),
            bytes_printed = stats.bytes_printed(),
            bytes_searched = stats.bytes_searched(),
            search_time = stats.elapsed().as_secs_f64(),
            process_time = elapsed.as_secs_f64(),
        )
    }
}
