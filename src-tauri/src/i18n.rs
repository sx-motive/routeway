use std::fmt::Display;
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Ru,
    Zh,
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn parse(code: &str) -> Lang {
    match code {
        "ru" => Lang::Ru,
        "zh" => Lang::Zh,
        _ => Lang::En,
    }
}

pub fn set(code: &str) {
    CURRENT.store(parse(code) as u8, Ordering::Relaxed);
}

pub fn current() -> Lang {
    match CURRENT.load(Ordering::Relaxed) {
        1 => Lang::Ru,
        2 => Lang::Zh,
        _ => Lang::En,
    }
}

pub fn lookup(lang: Lang, key: &str) -> &'static str {
    TABLE
        .iter()
        .find(|(k, ..)| *k == key)
        .map(|(_, en, ru, zh)| match lang {
            Lang::En => *en,
            Lang::Ru => *ru,
            Lang::Zh => *zh,
        })
        .unwrap_or("")
}

pub fn tr(key: &str) -> &'static str {
    lookup(current(), key)
}

pub fn fill(template: &str, args: &[(&str, &dyn Display)]) -> String {
    let mut out = template.to_string();
    for (name, value) in args {
        out = out.replace(&format!("{{{name}}}"), &value.to_string());
    }
    out
}

#[macro_export]
macro_rules! t {
    ($key:literal) => {
        $crate::i18n::tr($key).to_string()
    };
    ($key:literal, $($name:ident = $value:expr),+ $(,)?) => {
        $crate::i18n::fill(
            $crate::i18n::tr($key),
            &[$((stringify!($name), &$value as &dyn std::fmt::Display)),+],
        )
    };
}

type Row = (&'static str, &'static str, &'static str, &'static str);

const TABLE: &[Row] = &[
    ("recovery.checking", "Connection check failed; confirming the outage.", "Проверка соединения не прошла; проверяем повторно.", "连接检查失败，正在确认故障。"),
    ("recovery.unavailable", "The server is unavailable. Recovery attempts are rate limited; monitoring continues.", "Сервер недоступен. Частота попыток восстановления ограничена; проверки продолжаются.", "服务器不可用。恢复尝试频率受限，监控继续。"),
    ("recovery.network_wait", "Direct network checks also failed; waiting without restarting the core.", "Прямые проверки сети тоже не прошли; ожидаем без перезапуска ядра.", "直连网络检查也失败，等待网络恢复，不重启核心。"),
    ("recovery.tun_manual", "TUN host stopped. Reconnect manually to authorize its launch.", "TUN-хост остановлен. Подключитесь вручную, чтобы разрешить его запуск.", "TUN 主机已停止，请手动重新连接以授权启动。"),
    ("recovery.restarting", "Restoring connection: restarting the core.", "Восстановление соединения: перезапуск ядра.", "正在恢复连接：重启核心。"),
    ("recovery.started", "Core restarted. Checking the server connection.", "Ядро перезапущено. Проверяем соединение с сервером.", "核心已重启，正在检查服务器连接。"),
    ("recovery.failed", "Automatic recovery failed: {err}", "Автоматическое восстановление не удалось: {err}", "自动恢复失败：{err}"),
    ("recovery.restored", "Server connection restored.", "Соединение с сервером восстановлено.", "服务器连接已恢复。"),
    (
        "err.tunnel_unavailable",
        "The server connection check failed. {err}",
        "Проверка соединения через сервер не прошла. {err}",
        "服务器连接检查失败。{err}",
    ),
    (
        "tray.show",
        "Open Routeway",
        "Открыть Routeway",
        "打开 Routeway",
    ),
    ("tray.quit", "Quit", "Выйти", "退出"),
    (
        "err.config_dir",
        "could not determine the settings folder: {err}",
        "не удалось определить каталог настроек: {err}",
        "无法确定设置目录：{err}",
    ),
    (
        "err.config_dir_plain",
        "could not determine the settings folder",
        "не удалось определить каталог настроек",
        "无法确定设置目录",
    ),
    (
        "err.create_dir",
        "could not create {path}: {err}",
        "не удалось создать {path}: {err}",
        "无法创建 {path}：{err}",
    ),
    (
        "err.serialize_config",
        "could not serialize the config: {err}",
        "не удалось сериализовать конфиг: {err}",
        "无法序列化配置：{err}",
    ),
    (
        "err.write_config",
        "could not write the config: {err}",
        "не удалось записать конфиг: {err}",
        "无法写入配置：{err}",
    ),
    (
        "err.port_busy",
        "port {port} is used by another process — pick a different port in settings",
        "порт {port} занят другим процессом — выберите другой порт в настройках",
        "端口 {port} 已被其他进程占用 — 请在设置中选择其他端口",
    ),
    (
        "err.wait_core",
        "could not wait for the core to start: {err}",
        "не удалось дождаться запуска ядра: {err}",
        "等待核心启动失败：{err}",
    ),
    (
        "err.port_not_opened",
        "port {port} did not open",
        "порт {port} не открылся",
        "端口 {port} 未能打开",
    ),
    (
        "err.core_failed",
        "the core did not start: {detail}",
        "ядро не запустилось: {detail}",
        "核心未能启动：{detail}",
    ),
    (
        "err.core_restart",
        "could not restart the core: {err}",
        "не удалось перезапустить ядро: {err}",
        "无法重启核心：{err}",
    ),
    (
        "err.core_no_reload",
        "the core did not respond to the restart",
        "ядро не ответило на перезапуск",
        "核心未响应重启请求",
    ),
    (
        "err.config_path_chars",
        "the config path contains unsupported characters",
        "путь к конфигу содержит недопустимые символы",
        "配置路径包含不支持的字符",
    ),
    (
        "err.core_not_found",
        "could not find the sing-box core: {err}",
        "не удалось найти ядро sing-box: {err}",
        "找不到 sing-box 核心：{err}",
    ),
    (
        "err.core_spawn",
        "could not start the core: {err}",
        "не удалось запустить ядро: {err}",
        "无法启动核心：{err}",
    ),
    (
        "core.exited_code",
        "the core stopped (code {code})",
        "ядро остановилось (код {code})",
        "核心已停止（代码 {code}）",
    ),
    (
        "core.exited",
        "the core stopped",
        "ядро остановилось",
        "核心已停止",
    ),
    (
        "err.tun_failed",
        "the core did not start in TUN mode",
        "ядро в режиме TUN не запустилось",
        "核心未能以 TUN 模式启动",
    ),
    (
        "err.tun_failed_detail",
        "the core did not start in TUN mode: {detail}",
        "ядро в режиме TUN не запустилось: {detail}",
        "核心未能以 TUN 模式启动：{detail}",
    ),
    (
        "err.self_exe",
        "could not locate the app executable: {err}",
        "не удалось найти исполняемый файл приложения: {err}",
        "找不到应用程序可执行文件：{err}",
    ),
    (
        "err.unsupported_path",
        "unsupported path: {path}",
        "неподдерживаемый путь: {path}",
        "不支持的路径：{path}",
    ),
    (
        "err.cancelled",
        "cancelled by the user",
        "отменено пользователем",
        "用户已取消",
    ),
    (
        "err.elevation_code",
        "could not get administrator rights (code {code})",
        "не удалось получить права администратора (код {code})",
        "无法获取管理员权限（代码 {code}）",
    ),
    (
        "err.elevation",
        "could not get administrator rights: {err}",
        "не удалось получить права администратора: {err}",
        "无法获取管理员权限：{err}",
    ),
    (
        "err.osascript",
        "could not run osascript: {err}",
        "не удалось запустить osascript: {err}",
        "无法运行 osascript：{err}",
    ),
    (
        "tun.prompt",
        "Routeway is turning on TUN mode",
        "Routeway включает режим TUN",
        "Routeway 正在开启 TUN 模式",
    ),
    (
        "err.apply_proxy",
        "could not apply the system proxy: {err}",
        "не удалось применить системный прокси: {err}",
        "无法应用系统代理：{err}",
    ),
    (
        "err.bad_proxy",
        "invalid proxy address: {err}",
        "неверный адрес прокси: {err}",
        "代理地址无效：{err}",
    ),
    (
        "err.http_client",
        "could not create the HTTP client: {err}",
        "не удалось создать HTTP-клиент: {err}",
        "无法创建 HTTP 客户端：{err}",
    ),
    (
        "err.request",
        "request failed: {err}",
        "запрос не удался: {err}",
        "请求失败：{err}",
    ),
    (
        "err.read_response",
        "could not read the response: {err}",
        "не удалось прочитать ответ: {err}",
        "无法读取响应：{err}",
    ),
    (
        "err.open_dir",
        "could not open the folder: {err}",
        "не удалось открыть каталог: {err}",
        "无法打开目录：{err}",
    ),
    (
        "err.save_proxy_backup",
        "could not save the proxy snapshot: {err}",
        "не удалось сохранить снимок прокси: {err}",
        "无法保存代理快照：{err}",
    ),
    (
        "err.serialize_settings",
        "could not serialize settings: {err}",
        "не удалось сериализовать настройки: {err}",
        "无法序列化设置：{err}",
    ),
    (
        "err.write_file",
        "could not write {path}: {err}",
        "не удалось записать {path}: {err}",
        "无法写入 {path}：{err}",
    ),
    (
        "err.no_server",
        "no server configured",
        "сервер не настроен",
        "尚未配置服务器",
    ),
    (
        "err.no_address",
        "server address is missing",
        "не указан адрес сервера",
        "未填写服务器地址",
    ),
    (
        "err.no_uuid",
        "UUID is missing",
        "не указан UUID",
        "未填写 UUID",
    ),
    (
        "err.read_snapshot",
        "could not read the settings snapshot: {err}",
        "не удалось прочитать снимок настроек: {err}",
        "无法读取设置快照：{err}",
    ),
    (
        "err.network_service_name",
        "unsupported network service name: {name}",
        "неподдерживаемое имя сетевого сервиса: {name}",
        "不支持的网络服务名称：{name}",
    ),
    (
        "err.no_network",
        "no active network connection",
        "нет активного сетевого подключения",
        "没有活动的网络连接",
    ),
    (
        "err.network_service",
        "could not determine the network service name",
        "не удалось определить имя сетевого сервиса",
        "无法确定网络服务名称",
    ),
    (
        "err.scutil",
        "scutil failed: {err}",
        "ошибка scutil: {err}",
        "scutil 出错：{err}",
    ),
    (
        "err.change_proxy",
        "could not change the system proxy: {err}",
        "не удалось изменить системный прокси: {err}",
        "无法更改系统代理：{err}",
    ),
    (
        "err.open_inet_settings",
        "could not open internet settings: {err}",
        "не удалось открыть настройки интернета: {err}",
        "无法打开 Internet 设置：{err}",
    ),
    (
        "err.registry_write",
        "could not write {name}: {err}",
        "не удалось записать {name}: {err}",
        "无法写入 {name}：{err}",
    ),
    (
        "err.rules_json",
        "invalid JSON at line {line}, column {column}: {err}",
        "ошибка JSON в строке {line}, столбец {column}: {err}",
        "JSON 错误，第 {line} 行第 {column} 列：{err}",
    ),
    (
        "err.rules_domain",
        "not a domain: {values}",
        "не похоже на домен: {values}",
        "不是有效的域名：{values}",
    ),
    (
        "err.proxy_unsupported",
        "the system proxy is only supported on macOS and Windows",
        "системный прокси поддерживается только на macOS и Windows",
        "系统代理仅支持 macOS 和 Windows",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn placeholders(text: &str) -> Vec<String> {
        let mut found: Vec<String> = text
            .split('{')
            .skip(1)
            .filter_map(|part| part.split_once('}').map(|(name, _)| name.to_string()))
            .collect();
        found.sort();
        found
    }

    #[test]
    fn keys_are_unique() {
        for (i, (key, ..)) in TABLE.iter().enumerate() {
            assert!(
                TABLE[i + 1..].iter().all(|(other, ..)| other != key),
                "duplicate key {key}"
            );
        }
    }

    #[test]
    fn every_language_has_the_same_placeholders() {
        for (key, en, ru, zh) in TABLE {
            assert!(!en.is_empty() && !ru.is_empty() && !zh.is_empty(), "{key}");
            assert_eq!(placeholders(en), placeholders(ru), "{key}");
            assert_eq!(placeholders(en), placeholders(zh), "{key}");
        }
    }

    #[test]
    fn every_key_used_in_code_exists() {
        let sources = [
            include_str!("core.rs"),
            include_str!("lib.rs"),
            include_str!("settings.rs"),
            include_str!("singbox.rs"),
            include_str!("sysproxy.rs"),
            include_str!("tunhost.rs"),
        ];
        for source in sources {
            let parts: Vec<&str> = source.split("t!(\"").collect();
            for pair in parts.windows(2) {
                let follows_identifier = pair[0]
                    .chars()
                    .last()
                    .is_some_and(|c| c.is_alphanumeric() || c == '_');
                if follows_identifier {
                    continue;
                }
                let key = pair[1].split('"').next().unwrap();
                assert!(TABLE.iter().any(|(k, ..)| *k == key), "missing key {key}");
            }
        }
    }

    #[test]
    fn fill_substitutes_named_values() {
        let text = fill(lookup(Lang::En, "err.port_busy"), &[("port", &2080)]);
        assert!(text.starts_with("port 2080 is used"));
    }
}
