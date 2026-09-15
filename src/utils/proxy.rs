//! System proxy detection.
//!
//! Strategy: explicit environment proxy first, then the enabled Windows user proxy.
//! This keeps CLI/portable overrides authoritative while allowing the desktop app to
//! follow the proxy configured by the operating system (for example 127.0.0.1:2081).
//! Users can configure NO_PROXY / no_proxy env var to bypass the proxy for domains.

/// Detect a proxy URL from environment variables, then Windows Internet Settings.
/// Environment variables take precedence so CI/portable launches remain deterministic.
pub fn detect_proxy_url() -> Option<String> {
    if let Some(proxy) = env_proxy_url() {
        tracing::info!("[PROXY] 环境变量代理: {}", proxy);
        return Some(proxy);
    }

    #[cfg(windows)]
    if let Some(proxy) = windows_system_proxy() {
        tracing::info!("[PROXY] Windows 系统代理: {}", proxy);
        return Some(proxy);
    }

    None
}

fn env_proxy_url() -> Option<String> {
    let proxy = std::env::var("HTTPS_PROXY")
        .or_else(|_| std::env::var("https_proxy"))
        .or_else(|_| std::env::var("HTTP_PROXY"))
        .or_else(|_| std::env::var("http_proxy"))
        .ok()?;
    normalize_proxy_url(&proxy)
}

fn normalize_proxy_url(proxy: &str) -> Option<String> {
    let proxy = proxy.trim();
    if proxy.is_empty() {
        return None;
    }
    Some(if proxy.contains("://") {
        proxy.to_string()
    } else {
        format!("http://{}", proxy)
    })
}

#[cfg(windows)]
fn windows_registry_string(key: windows::Win32::System::Registry::HKEY, name: &str) -> Option<String> {
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::{RegGetValueW, RRF_RT_REG_SZ};

    let name_w: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let mut bytes = vec![0u8; 4096];
    let mut len = bytes.len() as u32;
    let result = unsafe {
        RegGetValueW(
            key,
            PCWSTR::null(),
            PCWSTR(name_w.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(bytes.as_mut_ptr().cast()),
            Some(&mut len),
        )
    };
    if result.is_err() {
        return None;
    }
    let units: Vec<u16> = bytes[..len as usize]
        .chunks_exact(2)
        .map(|pair| u16::from_ne_bytes([pair[0], pair[1]]))
        .take_while(|c| *c != 0)
        .collect();
    let value = String::from_utf16_lossy(&units);
    (!value.trim().is_empty()).then_some(value)
}

#[cfg(windows)]
fn windows_registry_dword(
    key: windows::Win32::System::Registry::HKEY,
    name: &str,
) -> Option<u32> {
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::{RegGetValueW, RRF_RT_REG_DWORD};

    let name_w: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let mut value = 0u32;
    let mut len = std::mem::size_of::<u32>() as u32;
    let result = unsafe {
        RegGetValueW(
            key,
            PCWSTR::null(),
            PCWSTR(name_w.as_ptr()),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut value as *mut u32).cast()),
            Some(&mut len),
        )
    };
    if result.is_err() {
        return None;
    }
    Some(value)
}

#[cfg(windows)]
fn windows_system_proxy() -> Option<String> {
    use windows::core::PCWSTR;
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, HKEY_CURRENT_USER, KEY_READ,
    };

    let path: Vec<u16> = r"Software\Microsoft\Windows\CurrentVersion\Internet Settings"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut key = Default::default();
    let result = unsafe {
        RegOpenKeyExW(HKEY_CURRENT_USER, PCWSTR(path.as_ptr()), 0, KEY_READ, &mut key)
    };
    if result.is_err() {
        return None;
    }

    let enabled = windows_registry_dword(key, "ProxyEnable") == Some(1);
    let server = windows_registry_string(key, "ProxyServer");
    unsafe { let _ = RegCloseKey(key); }
    if !enabled {
        return None;
    }
    let server = server?;
    let selected = server
        .split(';')
        .find_map(|part| part.strip_prefix("https="))
        .or_else(|| server.split(';').find_map(|part| part.strip_prefix("http=")))
        .unwrap_or(server.as_str());
    normalize_proxy_url(selected)
}

/// Returns the list of domains that should bypass the proxy.
/// Reads from NO_PROXY env var only. Always includes localhost/127.0.0.1.
pub fn get_no_proxy_domains() -> Vec<String> {
    let env_no_proxy = std::env::var("NO_PROXY")
        .or_else(|_| std::env::var("no_proxy"))
        .unwrap_or_default();

    let mut domains: Vec<String> = env_no_proxy
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    // Always bypass localhost
    for &d in &["localhost", "127.0.0.1"] {
        if !domains.contains(&d.to_string()) {
            domains.push(d.to_string());
        }
    }

    domains
}

/// Check whether a host should bypass the proxy based on NO_PROXY rules.
pub fn should_bypass_proxy(host: &str) -> bool {
    let domains = get_no_proxy_domains();
    let host = host
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    domains
        .iter()
        .any(|d| host == d.as_str() || host.ends_with(&format!(".{}", d)))
}

/// Build a `reqwest::Proxy` with NO_PROXY support (reads env var only).
pub fn build_reqwest_proxy(proxy_url: &str) -> Option<reqwest::Proxy> {
    let mut proxy = reqwest::Proxy::all(proxy_url).ok()?;
    let no_proxy_str = std::env::var("NO_PROXY")
        .or_else(|_| std::env::var("no_proxy"))
        .unwrap_or_default();
    if !no_proxy_str.is_empty() {
        if let Some(no_proxy) = reqwest::NoProxy::from_string(&no_proxy_str) {
            proxy = proxy.no_proxy(Some(no_proxy));
        }
    }
    Some(proxy)
}
