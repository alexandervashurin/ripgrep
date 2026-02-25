use std::{ffi::OsString, io};

/// Возвращает имя хоста текущей системы.
///
/// Необычно, хотя технически возможно, чтобы эта функция возвращала
/// ошибку. Трудно перечислить условия ошибок, но одна из таких
/// возможностей — поддержка платформы.
///
/// # Специфичное для платформы поведение
///
/// В Windows это в настоящее время использует "физическое DNS-имя хоста"
/// компьютера. Это может измениться в будущем.
///
/// В Unix это возвращает результат функции `gethostname` из `libc`,
/// связанной с программой.
pub fn hostname() -> io::Result<OsString> {
    #[cfg(windows)]
    {
        use winapi_util::sysinfo::{ComputerNameKind, get_computer_name};
        get_computer_name(ComputerNameKind::PhysicalDnsHostname)
    }
    #[cfg(unix)]
    {
        gethostname()
    }
    #[cfg(not(any(windows, unix)))]
    {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "hostname could not be found on unsupported platform",
        ))
    }
}

#[cfg(unix)]
fn gethostname() -> io::Result<OsString> {
    use std::os::unix::ffi::OsStringExt;

    // БЕЗОПАСНОСТЬ: Похоже, нет никаких требований безопасности для вызова
    // sysconf.
    let limit = unsafe { libc::sysconf(libc::_SC_HOST_NAME_MAX) };
    if limit == -1 {
        // В теории возможно, что sysconf вернет -1 для лимита, но
        // *не* установит errno, в этом случае io::Error::last_os_error
        // не определен. Но распутывание этого чрезвычайно раздражает,
        // потому что std не предоставляет никаких unix-специфичных API
        // для проверки errno. (Мы могли бы сделать это сами, но это
        // просто не кажется стоящим?)
        return Err(io::Error::last_os_error());
    }
    let Ok(maxlen) = usize::try_from(limit) else {
        let msg = format!("максимальный лимит имени хоста ({}) переполнил usize", limit);
        return Err(io::Error::new(io::ErrorKind::Other, msg));
    };
    // maxlen здесь включает NUL-терминатор.
    let mut buf = vec![0; maxlen];
    // БЕЗОПАСНОСТЬ: Указатель, который мы даем, валиден, так как получен
    // напрямую из Vec. Аналогично, `maxlen` — длина нашего Vec, и поэтому
    // валиден для записи.
    let rc = unsafe {
        libc::gethostname(buf.as_mut_ptr().cast::<libc::c_char>(), maxlen)
    };
    if rc == -1 {
        return Err(io::Error::last_os_error());
    }
    // POSIX говорит, что если имя хоста больше `maxlen`, то оно может
    // записать обратно усеченное имя, которое не обязательно NUL-терминировано
    // (wtf, lol). Поэтому, если мы не можем найти NUL-терминатор, просто сдаемся.
    let Some(zeropos) = buf.iter().position(|&b| b == 0) else {
        let msg = "не удалось найти NUL-терминатор в имени хоста";
        return Err(io::Error::new(io::ErrorKind::Other, msg));
    };
    buf.truncate(zeropos);
    buf.shrink_to_fit();
    Ok(OsString::from_vec(buf))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_hostname() {
        println!("{:?}", hostname().unwrap());
    }
}
