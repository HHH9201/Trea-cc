// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod api;
mod account;
mod autostart;
mod machine;
mod privacy;
mod mail_client;
mod quick_register;
mod quick_register_simple;

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use reqwest::Client;
use tokio::io::AsyncWriteExt;
use tokio::sync::{oneshot, Mutex};
use tauri::{AppHandle, Emitter, Manager, State, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent};
use tauri::webview::PageLoadEvent;
use tauri::tray::TrayIconBuilder;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri_plugin_updater::UpdaterExt;
use uuid::Uuid;
use warp::Filter;

use account::{AccountBrief, AccountManager, Account};
use api::{TraeApiClient, UsageSummary, UsageQueryResponse, UserStatisticResult};
use quick_register::wait_for_request_cookies;

#[cfg(target_os = "windows")]
fn hide_console_window() {
    use windows_sys::Win32::System::Console::GetConsoleWindow;
    use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};
    unsafe {
        let hwnd = GetConsoleWindow();
        if !hwnd.is_null() {
            ShowWindow(hwnd, SW_HIDE);
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub quick_register_show_window: bool,
    pub auto_refresh_enabled: bool,
    pub privacy_auto_enable: bool,
    pub auto_update_check: bool,
    pub auto_start_enabled: bool,
    pub api_key: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            quick_register_show_window: false,
            auto_refresh_enabled: true,
            privacy_auto_enable: true,
            auto_update_check: true,
            auto_start_enabled: false,
            api_key: "9201".to_string(),
        }
    }
}

fn get_settings_path() -> anyhow::Result<PathBuf> {
    let proj_dirs = directories::ProjectDirs::from("com", "hhj", "trae-cc")
        .ok_or_else(|| anyhow::anyhow!("无法获取应用配置目录"))?;
    let config_dir = proj_dirs.config_dir();
    fs::create_dir_all(config_dir)?;
    Ok(config_dir.join("settings.json"))
}

fn load_settings_from_disk() -> anyhow::Result<AppSettings> {
    let path = get_settings_path()?;
    if !path.exists() {
        return Ok(AppSettings::default());
    }
    let content = fs::read_to_string(&path)?;
    if content.trim().is_empty() {
        return Ok(AppSettings::default());
    }
    let settings = serde_json::from_str(&content)
        .unwrap_or_else(|_| AppSettings::default());
    Ok(settings)
}

fn save_settings_to_disk(settings: &AppSettings) -> anyhow::Result<()> {
    let path = get_settings_path()?;
    let content = serde_json::to_string_pretty(settings)?;
    fs::write(path, content)?;
    Ok(())
}

/// 应用状态
pub struct AppState {
    pub account_manager: Mutex<AccountManager>,
    browser_login: Mutex<Option<BrowserLoginSession>>,
    browser_login_cancel: Mutex<Option<oneshot::Sender<()>>>,
    settings: Mutex<AppSettings>,
}

struct BrowserLoginSession {
    receiver: oneshot::Receiver<(String, String)>,
    shutdown: Arc<StdMutex<Option<oneshot::Sender<()>>>>,
    cancel: oneshot::Receiver<()>,
    window_close: oneshot::Receiver<()>,
    webview: WebviewWindow,
    credentials: Arc<StdMutex<BrowserLoginCredentials>>,
}

#[derive(Debug, Default, Clone)]
struct BrowserLoginCredentials {
    email: Option<String>,
    password: Option<String>,
}

/// 错误类型
#[derive(Debug, serde::Serialize)]
pub struct ApiError {
    pub message: String,
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        Self {
            message: err.to_string(),
        }
    }
}

type Result<T> = std::result::Result<T, ApiError>;

// ============ Tauri 命令 ============

#[derive(Debug, Clone, serde::Serialize)]
struct QuickRegisterNotice {
    id: String,
    message: String,
}

fn emit_quick_register_notice(app: &AppHandle, id: &str, message: &str) {
    let payload = QuickRegisterNotice {
        id: id.to_string(),
        message: message.to_string(),
    };
    let _ = app.emit("quick_register_notice", payload);
}

/// 添加账号（通过 Token，可选 Cookies）
#[tauri::command]
async fn add_account_by_token(token: String, cookies: Option<String>, state: State<'_, AppState>) -> Result<Account> {
    let mut manager = state.account_manager.lock().await;
    manager.add_account_by_token(token, cookies, None).await.map_err(ApiError::from)
}

/// 添加账号（通过邮箱密码登录）
#[tauri::command]
async fn add_account_by_email(email: String, password: String, state: State<'_, AppState>) -> Result<Account> {
    let mut manager = state.account_manager.lock().await;
    manager.add_account_by_email(email, password).await.map_err(ApiError::from)
}

#[tauri::command]
async fn get_settings(state: State<'_, AppState>) -> Result<AppSettings> {
    let settings = state.settings.lock().await;
    Ok(settings.clone())
}

#[tauri::command]
async fn update_settings(settings: AppSettings, state: State<'_, AppState>) -> Result<AppSettings> {
    if let Err(err) = autostart::set_auto_start(settings.auto_start_enabled) {
        return Err(ApiError::from(err));
    }
    {
        let mut current = state.settings.lock().await;
        *current = settings.clone();
    }
    save_settings_to_disk(&settings).map_err(ApiError::from)?;
    Ok(settings)
}

/// 下载并运行更新安装包（Windows: .msi）
#[tauri::command]
async fn download_and_run_installer(url: String) -> Result<String> {
    let url = url.trim().to_string();
    if url.is_empty() {
        return Err(anyhow::anyhow!("安装包链接为空").into());
    }
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(anyhow::anyhow!("安装包链接无效").into());
    }

    // Prefer keeping the original filename, but avoid collisions.
    let raw_filename = url
        .split('/')
        .last()
        .unwrap_or("Trae账号管理Update.msi")
        .split('?')
        .next()
        .unwrap_or("Trae账号管理Update.msi")
        .trim();
    let filename = if raw_filename.is_empty() {
        "Trae账号管理Update.msi"
    } else {
        raw_filename
    };

    let mut dest_path = std::env::temp_dir();
    dest_path.push(format!(
        "Trae账号管理-update-{}-{}",
        Uuid::new_v4(),
        filename
    ));

    let client = Client::builder()
        .user_agent("Trae账号管理 @ Updater")
        .timeout(Duration::from_secs(60 * 30))
        .build()
        .map_err(|e| ApiError::from(anyhow::Error::new(e)))?;

    let mut response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| ApiError::from(anyhow::Error::new(e)))?;
    if !response.status().is_success() {
        return Err(anyhow::anyhow!("下载失败: {}", response.status()).into());
    }

    let mut file = tokio::fs::File::create(&dest_path)
        .await
        .map_err(|e| ApiError::from(anyhow::Error::new(e)))?;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| ApiError::from(anyhow::Error::new(e)))?
    {
        file.write_all(&chunk)
            .await
            .map_err(|e| ApiError::from(anyhow::Error::new(e)))?;
    }
    file.flush()
        .await
        .map_err(|e| ApiError::from(anyhow::Error::new(e)))?;
    drop(file);

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("msiexec")
            .arg("/i")
            .arg(dest_path.to_string_lossy().to_string())
            .spawn()
            .map_err(|e| anyhow::anyhow!("无法启动安装程序: {}", e))?;
    }

    #[cfg(not(target_os = "windows"))]
    {
        open::that(&dest_path).map_err(|e| anyhow::anyhow!("无法打开安装程序: {}", e))?;
    }

    Ok(dest_path.to_string_lossy().to_string())
}



#[tauri::command]
async fn quick_register(app: AppHandle, show_window: bool, state: State<'_, AppState>) -> Result<Account> {
    quick_register_simple::quick_register_simple(app, show_window, state).await.map_err(|e| e.into())
}

fn build_browser_login_script(port: u16) -> String {
    let script = r#"(function() {
  if (window.__traeAutoInjected) return;
  window.__traeAutoInjected = true;

  const callback = "http://127.0.0.1:__PORT__/callback";
  let loginTriggered = false;
  const normalize = (text) => (text || "").toLowerCase();
  const STORAGE_EMAIL_KEY = "__trae_login_email";
  const STORAGE_PASSWORD_KEY = "__trae_login_password";
  let capturedEmail = "";
  let capturedPassword = "";
  let lastSentEmail = "";
  let lastSentPassword = "";
  const boundInputs = new WeakSet();
  try {
    capturedEmail = sessionStorage.getItem(STORAGE_EMAIL_KEY) || "";
    capturedPassword = sessionStorage.getItem(STORAGE_PASSWORD_KEY) || "";
  } catch {}
  const captureEmail = (value) => {
    const next = (value || "").trim();
    if (next) {
      capturedEmail = next;
      try {
        sessionStorage.setItem(STORAGE_EMAIL_KEY, capturedEmail);
      } catch {}
    }
  };
  const capturePassword = (value) => {
    const next = (value || "").toString();
    if (next) {
      capturedPassword = next;
      try {
        sessionStorage.setItem(STORAGE_PASSWORD_KEY, capturedPassword);
      } catch {}
    }
  };
  const maybeCapture = (el) => {
    if (!el || !el.getAttribute) return;
    const type = normalize(el.getAttribute("type") || "");
    const name = normalize(el.getAttribute("name") || "");
    const autocomplete = normalize(el.getAttribute("autocomplete") || "");
    const placeholder = normalize(el.getAttribute("placeholder") || "");
    const value = typeof el.value === "string" ? el.value : "";
    const trimmedValue = value.trim();
    if (type === "password" || name.includes("password") || autocomplete.includes("password") || placeholder.includes("password")) {
      capturePassword(value);
    }
    if (
      type === "email" ||
      name.includes("email") ||
      name.includes("account") ||
      autocomplete.includes("email") ||
      placeholder.includes("email") ||
      placeholder.includes("邮箱")
    ) {
      captureEmail(value);
    } else if (!capturedEmail && trimmedValue.includes("@")) {
      captureEmail(trimmedValue);
    }
  };
  const bindInput = (input) => {
    if (!input || boundInputs.has(input) || !input.addEventListener) return;
    boundInputs.add(input);
    const handler = () => {
      maybeCapture(input);
      syncCredentials();
    };
    input.addEventListener("input", handler);
    input.addEventListener("change", handler);
    input.addEventListener("blur", handler);
  };
  const applyCredentialField = (key, value) => {
    if (typeof value !== "string") return;
    const lower = normalize(key);
    if (lower.includes("email")) {
      captureEmail(value);
    }
    if (
      lower.includes("password") ||
      lower.includes("passwd") ||
      lower === "pwd" ||
      lower.endsWith("password")
    ) {
      capturePassword(value);
    }
  };
  const extractCredentialsFromBody = (body) => {
    if (!body) return;
    try {
      if (typeof body === "string") {
        const trimmed = body.trim();
        if (!trimmed) return;
        if (trimmed.startsWith("{") || trimmed.startsWith("[")) {
          const data = JSON.parse(trimmed);
          if (data && typeof data === "object") {
            Object.keys(data).forEach((key) => applyCredentialField(key, data[key]));
          }
        } else {
          const params = new URLSearchParams(trimmed);
          params.forEach((value, key) => applyCredentialField(key, value));
        }
        syncCredentials();
        return;
      }
      if (body instanceof URLSearchParams) {
        body.forEach((value, key) => applyCredentialField(key, value));
        syncCredentials();
        return;
      }
      if (typeof FormData !== "undefined" && body instanceof FormData) {
        body.forEach((value, key) => {
          if (typeof value === "string") {
            applyCredentialField(key, value);
          }
        });
        syncCredentials();
        return;
      }
    } catch {}
  };
  const hookValueSetter = () => {
    try {
      if (window.__traeValueHooked) return;
      if (!window.HTMLInputElement) return;
      const proto = HTMLInputElement.prototype;
      const desc = Object.getOwnPropertyDescriptor(proto, "value");
      if (!desc || !desc.set || !desc.get) return;
      Object.defineProperty(proto, "value", {
        get: function() {
          return desc.get.call(this);
        },
        set: function(val) {
          desc.set.call(this, val);
          try {
            maybeCapture(this);
            syncCredentials();
          } catch {}
        }
      });
      window.__traeValueHooked = true;
    } catch {}
  };
  const getInputFromEvent = (event) => {
    const path = typeof event.composedPath === "function" ? event.composedPath() : (event.path || []);
    if (path && path.length) {
      for (const node of path) {
        if (node && node.tagName && node.tagName.toLowerCase() === "input") {
          return node;
        }
      }
    }
    return event.target;
  };
  const scanRoot = (root) => {
    if (!root) return;
    try {
      const inputs = root.querySelectorAll ? root.querySelectorAll("input") : [];
      if (inputs && inputs.length) {
        inputs.forEach((input) => {
          maybeCapture(input);
          bindInput(input);
        });
      }
      const elements = root.querySelectorAll ? root.querySelectorAll("*") : [];
      if (elements && elements.length) {
        elements.forEach((el) => {
          if (el && el.shadowRoot) {
            scanRoot(el.shadowRoot);
          }
          if (el && el.tagName && el.tagName.toLowerCase() === "iframe") {
            try {
              scanRoot(el.contentDocument || (el.contentWindow && el.contentWindow.document));
            } catch {}
          }
        });
      }
    } catch {}
  };
  const scanInputs = () => {
    scanRoot(document);
    syncCredentials();
  };
  const tryAcceptCookies = () => {
    const cookieSelectors = [
      'button.cm__btn',
      '.cm__btn[role=\"button\"]',
      '.cm__btn'
    ];
    for (const selector of cookieSelectors) {
      const btn = document.querySelector(selector);
      if (btn) {
        btn.click();
        return true;
      }
    }
    const candidates = Array.from(
      document.querySelectorAll("button, [role='button'], input[type='button'], input[type='submit'], a")
    );
    const matchText = (text) => {
      const val = (text || "").toLowerCase();
      return (
        val.includes("got it") ||
        val.includes("accept") ||
        val.includes("agree") ||
        val.includes("允许") ||
        val.includes("同意")
      );
    };
    for (const el of candidates) {
      const text = el.innerText || el.textContent || "";
      if (matchText(text)) {
        el.click();
        return true;
      }
    }
    const wrapper = document.querySelector(".cm-wrapper, .cc__wrapper, .cookie-banner, .cookie-consent");
    if (wrapper) {
      wrapper.remove();
      return true;
    }
    return false;
  };
  const sendPayload = (payload) => {
    const params = new URLSearchParams();
    Object.keys(payload || {}).forEach((key) => {
      const value = payload[key];
      if (value === undefined || value === null || value === "") return;
      params.append(key, value);
    });
    if (capturedEmail) params.append("email", capturedEmail);
    if (capturedPassword) params.append("password", capturedPassword);
    const url = callback + "?" + params.toString();
    if (navigator.sendBeacon) {
      navigator.sendBeacon(url);
    } else {
      fetch(url, { mode: "no-cors" });
    }
  };
  const syncCredentials = () => {
    if (!capturedEmail && !capturedPassword) return;
    if (capturedEmail === lastSentEmail && capturedPassword === lastSentPassword) return;
    lastSentEmail = capturedEmail;
    lastSentPassword = capturedPassword;
    sendPayload({ state: "credentials" });
  };
  const normalizeUrl = (raw) => {
    if (!raw) return "";
    try {
      return new URL(raw, location.href).toString();
    } catch {
      return String(raw);
    }
  };

  const sendToken = (token, url) => {
    if (!token) return;
    loginTriggered = true;
    sendPayload({ token, url: normalizeUrl(url) });
  };
  const sendState = (state, href) => {
    if (!state) return;
    loginTriggered = true;
    sendPayload({ state, href: href || "" });
  };
  const isLoginCompleteUrl = (href) => {
    if (!href) return false;
    const lower = href.toLowerCase();
    if (lower.includes("/login")) return false;
    if (lower.includes("passport")) return false;
    if (lower.includes("sign-up") || lower.includes("signup") || lower.includes("register")) return false;
    if (lower.includes("terms") || lower.includes("privacy")) return false;
    return true;
  };
  const parseToken = (data) => {
    if (!data) return null;
    return (
      data.result?.token ||
      data.result?.Token ||
      data.Result?.token ||
      data.Result?.Token ||
      null
    );
  };

  const markLoginTriggered = () => {
    loginTriggered = true;
  };
  const tryFetch = async () => {
    const endpoints = [
      "https://api-sg-central.trae.ai/cloudide/api/v3/common/GetUserToken",
      "https://api-us-east.trae.ai/cloudide/api/v3/common/GetUserToken"
    ];
    const headers = {
      "content-type": "application/json",
      "accept": "application/json, text/plain, */*",
      "origin": "https://www.trae.ai",
      "referer": "https://www.trae.ai/"
    };
    for (const endpoint of endpoints) {
      try {
        const res = await fetch(endpoint, {
          method: "POST",
          credentials: "include",
          headers,
          body: "{}"
        });
        const data = await res.json();
        const token = parseToken(data);
        if (token) {
          sendToken(token, res.url);
          return;
        }
      } catch {}
    }
  };

  const hookFetch = () => {
    const orig = window.fetch;
    window.fetch = async (...args) => {
      try {
        const input = args[0];
        const init = args[1];
        if (init && init.body) {
          extractCredentialsFromBody(init.body);
        } else if (input && typeof input === "object" && typeof input.clone === "function") {
          input.clone().text().then((text) => extractCredentialsFromBody(text)).catch(() => {});
        }
      } catch {}
      const res = await orig(...args);
      try {
        if (typeof res.url === "string" && res.url.includes("GetUserToken")) {
          const data = await res.clone().json();
          const token = parseToken(data);
          if (token) sendToken(token, res.url);
        }
      } catch {}
      return res;
    };
  };

  const hookXHR = () => {
    const origOpen = XMLHttpRequest.prototype.open;
    const origSend = XMLHttpRequest.prototype.send;
    XMLHttpRequest.prototype.open = function(method, url, ...rest) {
      this.__trae_url = url;
      return origOpen.apply(this, [method, url, ...rest]);
    };
    XMLHttpRequest.prototype.send = function(body) {
      try {
        extractCredentialsFromBody(body);
      } catch {}
      this.addEventListener("load", function() {
        try {
          if ((this.__trae_url || "").includes("GetUserToken")) {
            const data = JSON.parse(this.responseText);
            const token = parseToken(data);
            if (token) sendToken(token, this.__trae_url);
          }
        } catch {}
      });
      return origSend.apply(this, arguments);
    };
  };

  hookFetch();
  hookXHR();
  hookValueSetter();
  tryFetch();
  tryAcceptCookies();
  scanInputs();
  setInterval(tryFetch, 3000);
  setInterval(tryAcceptCookies, 1500);
  setInterval(scanInputs, 2000);
  try {
    const observer = new MutationObserver(() => scanInputs());
    const target = document.documentElement || document;
    observer.observe(target, { childList: true, subtree: true });
  } catch {}
  document.addEventListener("submit", () => {
    scanInputs();
    markLoginTriggered();
  }, true);
  syncCredentials();
  document.addEventListener("click", (event) => {
    const target = event.target;
    if (!target || !target.closest) return;
    scanInputs();
    const button = target.closest("button, [role='button'], a, input[type='button'], input[type='submit']");
    if (!button) return;
    const text = normalize(button.innerText || button.textContent || button.getAttribute("aria-label"));
    if (
      text.includes("log in") ||
      text.includes("login") ||
      text.includes("sign in") ||
      text.includes("sign-in") ||
      text.includes("github") ||
      text.includes("google") ||
      text.includes("continue") ||
      text.includes("登录") ||
      text.includes("继续") ||
      text.includes("授权")
    ) {
      markLoginTriggered();
    }
  }, true);
  document.addEventListener("input", (event) => {
    const target = getInputFromEvent(event);
    if (!target) return;
    maybeCapture(target);
    syncCredentials();
    const targetType = target.getAttribute ? normalize(target.getAttribute("type") || "") : "";
    if (targetType === "password") markLoginTriggered();
  }, true);
  let lastHref = location.href;
  let stateSent = false;
  const checkHref = () => {
    const href = location.href;
    if (href !== lastHref) {
      lastHref = href;
      if (!stateSent && isLoginCompleteUrl(href)) {
        stateSent = true;
        sendState("logged_in", href);
        tryFetch();
      }
    }
  };
  setInterval(checkHref, 1000);
  if (isLoginCompleteUrl(location.href)) {
    stateSent = true;
    sendState("logged_in", location.href);
    tryFetch();
  }
})();"#;
    script.replace("__PORT__", &port.to_string())
}

#[allow(dead_code)]
fn collect_trae_cookies(webview: &WebviewWindow, extra_url: Option<&str>) -> String {
    let mut cookie_map: HashMap<String, String> = HashMap::new();
    let mut urls = vec![
        "https://www.trae.ai/".to_string(),
        "https://api-sg-central.trae.ai/".to_string(),
        "https://ug-normal.trae.ai/".to_string(),
    ];
    
    if let Some(url) = extra_url {
        if !url.is_empty() {
             // 尝试提取 base url (e.g. https://api-us-east.trae.ai)
             if let Ok(parsed) = Url::parse(url) {
                 let base = format!("{}://{}/", parsed.scheme(), parsed.host_str().unwrap_or_default());
                 urls.push(base);
             }
             urls.push(url.to_string());
        }
    }

    for raw_url in urls {
        if let Ok(url) = Url::parse(&raw_url) {
            if let Ok(cookies) = webview.cookies_for_url(url) {
                for cookie in cookies {
                    cookie_map
                        .entry(cookie.name().to_string())
                        .or_insert(cookie.value().to_string());
                }
            }
        }
    }

    let mut cookies = cookie_map
        .into_iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join("; ");
    if !cookies.is_empty()
        && !cookies.contains("store-idc=")
        && !cookies.contains("trae-target-idc=")
    {
        cookies.push_str("; store-idc=alisg");
    }
    cookies
}
#[tauri::command]
async fn start_browser_login(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    let mut browser_login = state.browser_login.lock().await;
    if browser_login.is_some() {
        return Err(anyhow::anyhow!("浏览器登录已在进行中").into());
    }
    println!("[browser-login] start_browser_login: launching login window");

    let (token_tx, token_rx) = oneshot::channel::<(String, String)>();
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let (cancel_tx, cancel_rx) = oneshot::channel::<()>();
    let (window_close_tx, window_close_rx) = oneshot::channel::<()>();
    let token_sender = Arc::new(StdMutex::new(Some(token_tx)));
    let shutdown_sender = Arc::new(StdMutex::new(Some(shutdown_tx)));
    let window_close_sender = Arc::new(StdMutex::new(Some(window_close_tx)));
    let credentials = Arc::new(StdMutex::new(BrowserLoginCredentials::default()));

    let token_sender_route = token_sender.clone();
    let shutdown_sender_route = shutdown_sender.clone();
    let credentials_route = credentials.clone();
    let route = warp::path("callback")
        .and(warp::query::<HashMap<String, String>>())
        .map(move |query: HashMap<String, String>| {
            let mut log_query = query.clone();
            if log_query.contains_key("password") {
                log_query.insert("password".to_string(), "***".to_string());
            }
            println!("[browser-login] callback query: {:?}", log_query);
            let token = query.get("token").cloned().unwrap_or_default();
            let state = query.get("state").cloned().unwrap_or_default();
            let href = query.get("href").cloned().unwrap_or_default();
            let url = query.get("url").cloned().unwrap_or_default();
            let email = query.get("email").cloned().unwrap_or_default();
            let password = query.get("password").cloned().unwrap_or_default();

            if !email.trim().is_empty() || !password.is_empty() {
                let mut creds = credentials_route.lock().unwrap();
                if !email.trim().is_empty() {
                    creds.email = Some(email.trim().to_string());
                }
                if !password.is_empty() {
                    creds.password = Some(password);
                }
            }
            if !token.is_empty() {
                if let Some(tx) = token_sender_route.lock().unwrap().take() {
                    let _ = tx.send((token, url));
                }
                if let Some(tx) = shutdown_sender_route.lock().unwrap().take() {
                    let _ = tx.send(());
                }
                warp::reply::html("已收到 Token，可以关闭此页面并返回应用。".to_string())
            } else if state == "logged_in" {
                warp::reply::html(format!("检测到登录完成，等待获取 Token。{href}"))
            } else {
                warp::reply::html("未收到 Token，请重试。".to_string())
            }
        });

    let (addr, server): (std::net::SocketAddr, _) = warp::serve(route)
        .bind_with_graceful_shutdown(([127, 0, 0, 1], 0), async move {
            let _ = shutdown_rx.await;
        });

    tokio::spawn(server);

    let script = build_browser_login_script(addr.port());
    let script_init = script.clone();
    let script_onload = script.clone();

    // 关闭已存在的登录窗口
    if let Some(existing) = app.get_webview_window("trae-login") {
        let _ = existing.destroy();
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
    // 再次检查确保窗口已关闭
    if app.get_webview_window("trae-login").is_some() {
        return Err(anyhow::anyhow!("无法关闭已存在的登录窗口，请重启应用后重试").into());
    }

    let webview = WebviewWindowBuilder::new(&app, "trae-login", WebviewUrl::External("about:blank".parse().unwrap()))
        .title("Trae 登录")
        .inner_size(1000.0, 720.0)
        .initialization_script(&script_init)
        .on_page_load(move |window, payload| {
            if payload.event() == PageLoadEvent::Finished {
                println!("[browser-login] page load finished, injecting script");
                let _ = window.eval(script_onload.clone());
            }
        })
        .build()
        .map_err(|e| anyhow::anyhow!("无法打开登录窗口: {}", e))?;

    let window_close_sender_clone = window_close_sender.clone();
    webview.on_window_event(move |event| {
        if let tauri::WindowEvent::Destroyed = event {
            if let Some(tx) = window_close_sender_clone.lock().unwrap().take() {
                let _ = tx.send(());
            }
        }
    });

    if let Err(e) = webview.clear_all_browsing_data() {
        println!("[browser-login] clear browsing data failed: {}", e);
    } else {
        println!("[browser-login] cleared browsing data");
    }
    let _ = webview.navigate(Url::parse("https://www.trae.ai/login").unwrap());

    let _ = webview.set_focus();
    let _ = webview.eval(script);

    *browser_login = Some(BrowserLoginSession {
        receiver: token_rx,
        shutdown: shutdown_sender,
        cancel: cancel_rx,
        window_close: window_close_rx,
        webview,
        credentials,
    });
    *state.browser_login_cancel.lock().await = Some(cancel_tx);

    Ok(())
}

#[tauri::command]
async fn finish_browser_login(state: State<'_, AppState>) -> Result<Account> {
    println!("[browser-login] finish_browser_login: waiting for token");
    let session = {
        let mut browser_login = state.browser_login.lock().await;
        browser_login.take().ok_or_else(|| anyhow::anyhow!("浏览器登录未开始"))?
    };

    let (token, url) = tokio::select! {
        res = session.receiver => {
            match res {
                Ok(token) => token,
                Err(_) => {
                    let _ = state.browser_login_cancel.lock().await.take();
                    if let Some(tx) = session.shutdown.lock().unwrap().take() {
                        let _ = tx.send(());
                    }
                    let _ = session.webview.close();
                    return Err(anyhow::anyhow!("浏览器登录已取消").into());
                }
            }
        }
        _ = session.cancel => {
            let _ = state.browser_login_cancel.lock().await.take();
            if let Some(tx) = session.shutdown.lock().unwrap().take() {
                let _ = tx.send(());
            }
            let _ = session.webview.close();
            return Err(anyhow::anyhow!("浏览器登录已取消").into());
        }
        _ = session.window_close => {
            let _ = state.browser_login_cancel.lock().await.take();
            if let Some(tx) = session.shutdown.lock().unwrap().take() {
                let _ = tx.send(());
            }
            return Err(anyhow::anyhow!("浏览器被主动关闭").into());
        }
        _ = tokio::time::sleep(Duration::from_secs(300)) => {
            let _ = state.browser_login_cancel.lock().await.take();
            if let Some(tx) = session.shutdown.lock().unwrap().take() {
                let _ = tx.send(());
            }
            let _ = session.webview.close();
            return Err(anyhow::anyhow!("等待浏览器登录超时").into());
        }
    };

    if let Some(tx) = session.shutdown.lock().unwrap().take() {
        let _ = tx.send(());
    }
    let _ = state.browser_login_cancel.lock().await.take();

    let cookies = match wait_for_request_cookies(&session.webview, &url, Duration::from_secs(6)).await {
        Ok(cookies) => {
            println!("[browser-login] captured cookies for {}: {}", url, cookies);
            cookies
        }
        Err(err) => {
            let _ = session.webview.close();
            return Err(ApiError::from(err));
        }
    };

    let mut credentials = session.credentials.lock().unwrap().clone();
    if credentials.email.as_deref().unwrap_or("").trim().is_empty()
        && credentials.password.as_deref().unwrap_or("").is_empty()
    {
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            let snapshot = session.credentials.lock().unwrap().clone();
            if !snapshot.email.as_deref().unwrap_or("").trim().is_empty()
                || !snapshot.password.as_deref().unwrap_or("").is_empty()
            {
                credentials = snapshot;
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    let _ = session.webview.close();
    let cookies = if cookies.is_empty() { None } else { Some(cookies) };

    let mut manager = state.account_manager.lock().await;
    let mut account = manager
        .upsert_account_by_token(token, cookies, None)
        .await
        .map_err(ApiError::from)?;

    let email = credentials.email.unwrap_or_default();
    let password = credentials.password.unwrap_or_default();
    let has_email = !email.trim().is_empty();
    let has_password = !password.is_empty();
    if has_email || has_password {
        account = manager
            .update_account_profile(
                &account.id,
                if has_email { Some(email) } else { None },
                if has_password { Some(password) } else { None },
            )
            .map_err(ApiError::from)?;
    }

    Ok(account)
}

#[tauri::command]
async fn cancel_browser_login(app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    if let Some(tx) = state.browser_login_cancel.lock().await.take() {
        let _ = tx.send(());
    }
    let session = {
        let mut browser_login = state.browser_login.lock().await;
        browser_login.take()
    };
    if let Some(session) = session {
        if let Some(tx) = session.shutdown.lock().unwrap().take() {
            let _ = tx.send(());
        }
        let _ = session.webview.destroy();
    } else if let Some(window) = app.get_webview_window("trae-login") {
        let _ = window.destroy();
    }
    Ok(())
}

#[tauri::command]
async fn remove_account(account_id: String, state: State<'_, AppState>) -> Result<()> {
    let mut manager = state.account_manager.lock().await;
    manager.remove_account(&account_id).map_err(ApiError::from)
}

/// 获取所有账号
#[tauri::command]
async fn get_accounts(state: State<'_, AppState>) -> Result<Vec<AccountBrief>> {
    let manager = state.account_manager.lock().await;
    Ok(manager.get_accounts())
}

/// 获取单个账号详情
#[tauri::command]
async fn get_account(account_id: String, state: State<'_, AppState>) -> Result<Account> {
    let manager = state.account_manager.lock().await;
    manager.get_account(&account_id).map_err(ApiError::from)
}

/// 切换账号（设置活跃账号并更新机器码）
#[tauri::command]
async fn switch_account(account_id: String, force: Option<bool>, state: State<'_, AppState>) -> Result<()> {
    {
        let mut manager = state.account_manager.lock().await;
        let force = force.unwrap_or(false);
        manager.switch_account(&account_id, force).await.map_err(ApiError::from)?;
    }

    let settings = state.settings.lock().await.clone();
    if settings.privacy_auto_enable {
        println!("[INFO] 等待 Trae IDE 启动后写入隐私模式设置");
        let db_path = match machine::get_trae_state_db_path() {
            Ok(path) => path,
            Err(err) => {
                println!("[ERROR] 查找 Trae 数据库失败: {}", err);
                // 即使查找数据库失败，也尝试启动 Trae
                let _ = tokio::task::spawn_blocking(|| {
                    let _ = machine::open_trae();
                }).await;
                return Ok(());
            }
        };
        
        // 先启动 Trae，等待数据库文件生成
        let db_path_clone = db_path.clone();
        let start_result = tokio::task::spawn_blocking(move || {
            // 启动 Trae
            if let Err(e) = machine::open_trae() {
                println!("[ERROR] 启动 Trae IDE 失败: {}", e);
                return Err(e);
            }
            
            // 等待数据库文件存在（最多等待 30 秒）
            let start = std::time::Instant::now();
            let timeout = std::time::Duration::from_secs(30);
            while !db_path_clone.exists() {
                if start.elapsed() > timeout {
                    return Err(anyhow::anyhow!("等待 Trae 数据库超时"));
                }
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
            
            Ok(())
        }).await;
        
        match start_result {
            Ok(Ok(())) => {
                // Trae 启动成功，数据库文件已生成，现在写入隐私模式
                let result = tokio::task::spawn_blocking(move || {
                    privacy::enable_privacy_mode_at_path_with_restart(db_path, || {
                        println!("[INFO] 正在重启 Trae IDE 以应用隐私模式...");
                        machine::kill_trae()?;
                        machine::open_trae()
                    })
                }).await;

                match result {
                    Ok(Ok(_)) => {}
                    Ok(Err(err)) => {
                        println!("[ERROR] 自动开启隐私模式失败: {}", err);
                    }
                    Err(err) => {
                        println!("[ERROR] 自动开启隐私模式任务失败: {}", err);
                    }
                }
            }
            Ok(Err(err)) => {
                println!("[ERROR] 启动 Trae IDE 失败: {}", err);
            }
            Err(err) => {
                println!("[ERROR] 启动 Trae IDE 任务失败: {}", err);
            }
        }
    } else {
        // 隐私模式未启用，直接启动 Trae
        let _ = tokio::task::spawn_blocking(|| {
            if let Err(e) = machine::open_trae() {
                println!("[WARN] 自动打开 Trae IDE 失败: {}", e);
            }
        }).await;
    }

    Ok(())
}

/// 获取账号使用量
#[tauri::command]
async fn get_account_usage(account_id: String, state: State<'_, AppState>) -> Result<UsageSummary> {
    // 1. 获取账号信息（持有锁的时间极短）
    let account = {
        let manager = state.account_manager.lock().await;
        manager.get_account(&account_id).map_err(ApiError::from)?
    };

    // 2. 执行网络请求（不持有锁，可并行）
    let (summary, new_token) = fetch_usage_for_account(&account).await.map_err(ApiError::from)?;

    // 3. 更新账号信息（持有锁的时间极短）
    {
        let mut manager = state.account_manager.lock().await;
        // 忽略更新错误（可能账号已被删除），但不影响返回结果
        let _ = manager.update_account_info_after_usage_check(
            &account_id,
            summary.plan_type.clone(),
            new_token,
        );
    }

    Ok(summary)
}

async fn fetch_usage_for_account(account: &Account) -> anyhow::Result<(UsageSummary, Option<(String, String)>)> {
    let mut new_token_info = None;

    let summary = if let Some(token) = &account.jwt_token {
        // 优先使用 Token
        let client = TraeApiClient::new_with_token(token)?;
        match client.get_usage_summary_by_token().await {
            Ok(summary) => summary,
            Err(e) => {
                let error_msg = e.to_string();
                // 如果是 401 错误且有 Cookies，尝试刷新 Token
                if error_msg.contains("401") && !account.cookies.is_empty() {
                    println!("[INFO] Token 已过期，尝试使用 Cookies 刷新...");
                    // 使用 Cookies 刷新 Token
                    let mut cookie_client = TraeApiClient::new(&account.cookies)?;
                    let token_result = cookie_client.get_user_token().await?;
                    
                    new_token_info = Some((token_result.token.clone(), token_result.expired_at.clone()));

                    // 使用新 Token 重新获取使用量
                    let new_client = TraeApiClient::new_with_token(&token_result.token)?;
                    new_client.get_usage_summary_by_token().await?
                } else if error_msg.contains("401") {
                    return Err(anyhow::anyhow!("Token 已过期，请更新 Token 或 Cookies"));
                } else {
                    return Err(e);
                }
            }
        }
    } else if !account.cookies.is_empty() {
        // 使用 Cookies
        let mut client = TraeApiClient::new(&account.cookies)?;
        // 先获取 token 以便保存
        let token_result = client.get_user_token().await?;
        new_token_info = Some((token_result.token.clone(), token_result.expired_at.clone()));
        
        client.get_usage_summary().await?
    } else {
        return Err(anyhow::anyhow!("账号没有有效的 Token 或 Cookies"));
    };

    Ok((summary, new_token_info))
}

/// 更新账号 Token
#[tauri::command]
async fn update_account_token(account_id: String, token: String, state: State<'_, AppState>) -> Result<UsageSummary> {
    let mut manager = state.account_manager.lock().await;
    manager.update_account_token(&account_id, token).await.map_err(ApiError::from)
}

/// 刷新 Token（使用 Cookies）
#[tauri::command]
async fn refresh_token(account_id: String, state: State<'_, AppState>) -> Result<()> {
    let mut manager = state.account_manager.lock().await;
    manager.refresh_token(&account_id).await.map_err(ApiError::from)
}

/// 使用密码刷新 Token/Cookies
#[tauri::command]
async fn refresh_token_with_password(
    account_id: String,
    password: String,
    state: State<'_, AppState>,
) -> Result<()> {
    let mut manager = state.account_manager.lock().await;
    manager
        .refresh_token_with_password(&account_id, &password)
        .await
        .map_err(ApiError::from)
}

/// 使用邮箱密码重新登录并更新账号
#[tauri::command]
async fn login_account_with_email(
    account_id: String,
    email: String,
    password: String,
    state: State<'_, AppState>,
) -> Result<UsageSummary> {
    let mut manager = state.account_manager.lock().await;
    manager
        .login_account_with_email(&account_id, email, password)
        .await
        .map_err(ApiError::from)
}

/// 更新账号邮箱/密码
#[tauri::command]
async fn update_account_profile(
    account_id: String,
    email: Option<String>,
    password: Option<String>,
    state: State<'_, AppState>,
) -> Result<Account> {
    let mut manager = state.account_manager.lock().await;
    manager
        .update_account_profile(&account_id, email, password)
        .map_err(ApiError::from)
}

/// 清空账号数据
#[tauri::command]
async fn clear_accounts(state: State<'_, AppState>) -> Result<usize> {
    let mut manager = state.account_manager.lock().await;
    manager.clear_accounts().map_err(ApiError::from)
}

/// 导出账号到指定路径
#[tauri::command]
async fn export_accounts_to_path(path: String, state: State<'_, AppState>) -> Result<()> {
    let manager = state.account_manager.lock().await;
    let content = manager.export_accounts().map_err(ApiError::from)?;
    fs::write(&path, content)
        .map_err(|err| ApiError::from(anyhow::Error::from(err)))?;
    Ok(())
}

/// 导出账号
#[tauri::command]
async fn export_accounts(state: State<'_, AppState>) -> Result<String> {
    let manager = state.account_manager.lock().await;
    manager.export_accounts().map_err(ApiError::from)
}

/// 导入账号
#[tauri::command]
async fn import_accounts(data: String, state: State<'_, AppState>) -> Result<usize> {
    let mut manager = state.account_manager.lock().await;
    manager.import_accounts(&data).await.map_err(ApiError::from)
}

/// 获取使用事件
#[tauri::command]
async fn get_usage_events(
    account_id: String,
    start_time: i64,
    end_time: i64,
    page_num: i32,
    page_size: i32,
    state: State<'_, AppState>
) -> Result<UsageQueryResponse> {
    let mut manager = state.account_manager.lock().await;
    manager.get_usage_events(&account_id, start_time, end_time, page_num, page_size)
        .await
        .map_err(ApiError::from)
}

/// 从 Trae IDE 读取账号
#[tauri::command]
async fn read_trae_account(state: State<'_, AppState>) -> Result<Option<Account>> {
    let mut manager = state.account_manager.lock().await;
    manager.read_trae_ide_account().await.map_err(ApiError::from)
}

/// 获取当前系统机器码
#[tauri::command]
async fn get_machine_id() -> Result<String> {
    machine::get_machine_guid().map_err(ApiError::from)
}

/// 重置系统机器码（生成新的随机机器码）
#[tauri::command]
async fn reset_machine_id() -> Result<String> {
    machine::reset_machine_guid().map_err(ApiError::from)
}

/// 设置系统机器码为指定值
#[tauri::command]
async fn set_machine_id(machine_id: String) -> Result<()> {
    machine::set_machine_guid(&machine_id).map_err(ApiError::from)
}

/// 绑定账号机器码（保存当前系统机器码到账号）
#[tauri::command]
async fn bind_account_machine_id(account_id: String, state: State<'_, AppState>) -> Result<String> {
    let mut manager = state.account_manager.lock().await;
    manager.bind_machine_id(&account_id).map_err(ApiError::from)
}

/// 获取 Trae IDE 的机器码
#[tauri::command]
async fn get_trae_machine_id() -> Result<String> {
    machine::get_trae_machine_id().map_err(ApiError::from)
}

/// 设置 Trae IDE 的机器码
#[tauri::command]
async fn set_trae_machine_id(machine_id: String) -> Result<()> {
    machine::set_trae_machine_id(&machine_id).map_err(ApiError::from)
}

/// 清除 Trae IDE 登录状态（让 IDE 变成全新安装状态）
#[tauri::command]
async fn clear_trae_login_state() -> Result<()> {
    machine::clear_trae_login_state().map_err(ApiError::from)
}

/// 获取保存的 Trae IDE 路径
#[tauri::command]
async fn get_trae_path() -> Result<String> {
    machine::get_saved_trae_path().map_err(ApiError::from)
}

/// 设置 Trae IDE 路径
#[tauri::command]
async fn set_trae_path(path: String) -> Result<()> {
    machine::save_trae_path(&path).map_err(ApiError::from)
}

/// 自动扫描 Trae IDE 路径
#[tauri::command]
async fn scan_trae_path() -> Result<String> {
    machine::scan_trae_path().map_err(ApiError::from)
}

/// 检查更新
#[tauri::command]
async fn check_update(app: AppHandle) -> Result<Option<serde_json::Value>> {
    let updater = app.updater().map_err(|e| {
        println!("[ERROR] 获取更新器失败: {}", e);
        ApiError::from(anyhow::anyhow!("获取更新器失败: {}", e))
    })?;
    
    println!("[INFO] 正在检查更新...");
    
    match updater.check().await {
        Ok(Some(update)) => {
            println!("[INFO] 发现新版本: {}", update.version);
            let info = serde_json::json!({
                "version": update.version,
                "current_version": update.current_version,
                "body": update.body,
                "date": update.date.map(|d| d.to_string())
            });
            Ok(Some(info))
        }
        Ok(None) => {
            println!("[INFO] 当前已是最新版本");
            Ok(None)
        }
        Err(e) => {
            println!("[ERROR] 检查更新失败: {}", e);
            Err(ApiError::from(anyhow::anyhow!("检查更新失败: {}", e)))
        }
    }
}

/// 下载并安装更新
#[tauri::command]
async fn install_update(app: AppHandle) -> Result<()> {
    let updater = app.updater().map_err(|e| ApiError::from(anyhow::anyhow!("获取更新器失败: {}", e)))?;
    
    if let Some(update) = updater.check().await.map_err(|e| ApiError::from(anyhow::anyhow!("检查更新失败: {}", e)))? {
        update.download_and_install(|_, _| {}, || {}).await.map_err(|e| ApiError::from(anyhow::anyhow!("下载安装失败: {}", e)))?;
    }
    
    Ok(())
}

/// 打开购买页面（内置浏览器，携带账号 Cookies）
#[tauri::command]
async fn open_pricing(account_id: String, app: AppHandle, state: State<'_, AppState>) -> Result<()> {
    let account = {
        let manager = state.account_manager.lock().await;
        manager.get_account(&account_id).map_err(ApiError::from)?
    };

    // 如果窗口已存在，先关闭它
    if let Some(existing) = app.get_webview_window("trae-pricing") {
        // 使用 destroy 强制销毁窗口
        let _ = existing.destroy();
        // 等待窗口完全销毁
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    }
    // 再次检查确保窗口已关闭
    if app.get_webview_window("trae-pricing").is_some() {
        return Err(anyhow::anyhow!("无法关闭已存在的购买窗口，请重启应用后重试").into());
    }

    let cookies = account.cookies.clone();
    let cookies_for_js = cookies.replace('\\', "\\\\").replace('`', "\\`");
    let js_onload = format!(
        r#"
(() => {{
  try {{
    // 只在 trae.ai 域名下执行
    if (!location.hostname.endsWith('trae.ai')) return;

    // 如果已经在 pricing 页面且已注入过，就不再执行
    if (location.href.includes('/pricing') && sessionStorage.getItem('trae_auth_injected')) return;

    console.log('[pricing] Starting auth injection...');

    // 1. 尽力清除旧数据 (JS 能访问到的)
    try {{
        localStorage.clear();
        sessionStorage.clear();
        const oldCookies = document.cookie.split(";");
        for (let i = 0; i < oldCookies.length; i++) {{
            const cookie = oldCookies[i];
            const eqPos = cookie.indexOf("=");
            const name = eqPos > -1 ? cookie.substr(0, eqPos).trim() : cookie.trim();
            document.cookie = name + "=;expires=Thu, 01 Jan 1970 00:00:00 GMT;path=/;domain=.trae.ai";
            document.cookie = name + "=;expires=Thu, 01 Jan 1970 00:00:00 GMT;path=/;domain=www.trae.ai";
            document.cookie = name + "=;expires=Thu, 01 Jan 1970 00:00:00 GMT;path=/";
        }}
    }} catch (e) {{
        console.warn('[pricing] Clear old data failed', e);
    }}

    // 2. 注入新 Cookie
    const raw = `{cookies}`;
    const parts = raw ? raw.split(';').map(s => s.trim()).filter(Boolean) : [];
    const seen = new Set();
    for (const kv of parts) {{
      const idx = kv.indexOf('=');
      if (idx <= 0) continue;
      const name = kv.slice(0, idx);
      const value = kv.slice(idx + 1);
      if (seen.has(name)) continue;
      seen.add(name);
      document.cookie = `${{name}}=${{value}}; path=/; domain=.trae.ai; secure; samesite=lax`;
    }}
    // 补全 IDC cookie
    if (!raw.includes('store-idc=') && !raw.includes('trae-target-idc=')) {{
      document.cookie = `store-idc=alisg; path=/; domain=.trae.ai; secure; samesite=lax`;
    }}
    
    // 3. 标记并跳转
    sessionStorage.setItem('trae_auth_injected', 'true');
    
    if (!location.href.includes('/pricing')) {{
        console.log('[pricing] Redirecting to pricing...');
        window.location.href = "https://www.trae.ai/pricing";
    }} else {{
        console.log('[pricing] Reloading to apply cookies...');
        location.reload();
    }}
  }} catch (e) {{
    console.error('[pricing] cookie inject error', e);
  }}
}})();
"#,
        cookies = cookies_for_js
    );

    let script_onload = js_onload.clone();
    let webview = WebviewWindowBuilder::new(
        &app,
        "trae-pricing",
        WebviewUrl::External("about:blank".parse().unwrap()),
    )
    .title("Trae 购买 Pro")
    .inner_size(1000.0, 720.0)
    .on_page_load(move |window, payload| {
        if payload.event() == PageLoadEvent::Finished {
            let _ = window.eval(script_onload.clone());
        }
    })
    .build()
    .map_err(|e| anyhow::anyhow!("无法打开购买窗口: {}", e))?;

    // 强制清理数据
    if let Err(e) = webview.clear_all_browsing_data() {
        println!("[pricing] clear browsing data failed: {}", e);
    } else {
        println!("[pricing] cleared browsing data");
    }

    // 先导航到一个轻量页(404)来建立域上下文并执行注入，然后再由脚本跳转到 pricing
    // 这样可以确保 Cookie 在请求 pricing 之前就已经准备好
    let _ = webview.navigate(Url::parse("https://www.trae.ai/404_auth_init").unwrap());
    let _ = webview.set_focus();
    Ok(())
}

/// 获取用户统计数据
#[tauri::command]
async fn get_user_statistics(account_id: String, state: State<'_, AppState>) -> Result<UserStatisticResult> {
    let manager = state.account_manager.lock().await;
    manager.get_account_statistics(&account_id).await.map_err(ApiError::from)
}

async fn handle_silent_start() -> anyhow::Result<()> {
    let mut manager = AccountManager::new()?;
    
    // 1. Refresh all accounts
    let account_ids: Vec<String> = manager.get_accounts().into_iter().map(|a| a.id).collect();
    for id in account_ids {
        if let Err(e) = manager.refresh_token(&id).await {
            println!("[Silent] Failed to refresh account {}: {}", id, e);
        } else {
            println!("[Silent] Refreshed account {}", id);
        }
    }

    // 2. Sync with Trae IDE if it's not running
    if !machine::is_trae_running() {
        let accounts = manager.get_accounts();
        if let Some(current) = accounts.iter().find(|a| a.is_current) {
             if let Ok(account) = manager.get_account(&current.id) {
                if let Some(token) = account.jwt_token {
                     let login_info = machine::TraeLoginInfo {
                        token,
                        refresh_token: None,
                        user_id: account.user_id,
                        email: account.email,
                        username: account.name,
                        avatar_url: account.avatar_url,
                        host: String::new(),
                        region: if account.region.is_empty() { "SG".to_string() } else { account.region },
                    };
                    if let Err(e) = machine::write_trae_login_info(&login_info) {
                        println!("[Silent] Failed to write Trae login info: {}", e);
                    } else {
                        println!("[Silent] Synced token to Trae IDE for account {}", current.email);
                    }
                }
             }
        }
    } else {
        println!("[Silent] Trae IDE is running, skipping sync");
    }

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Check for silent flag
    let args: Vec<String> = std::env::args().collect();
    if args.contains(&"--silent".to_string()) {
        #[cfg(target_os = "windows")]
        hide_console_window();
        let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
        rt.block_on(async {
            if let Err(e) = handle_silent_start().await {
                eprintln!("[Silent] Error: {}", e);
            }
        });
        std::process::exit(0);
    }

    let account_manager = AccountManager::new().expect("无法初始化账号管理器");
    let settings = load_settings_from_disk().unwrap_or_else(|err| {
        println!("[WARN] 读取设置失败，使用默认值: {}", err);
        AppSettings::default()
    });
    if let Err(err) = autostart::set_auto_start(settings.auto_start_enabled) {
        println!("[WARN] 设置开机自启动失败: {}", err);
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_positioner::init())
        .manage(AppState {
            account_manager: Mutex::new(account_manager),
            browser_login: Mutex::new(None),
            browser_login_cancel: Mutex::new(None),
            settings: Mutex::new(settings),
        })
        .invoke_handler(tauri::generate_handler![
            add_account_by_token,
            add_account_by_email,
            get_settings,
            update_settings,
            download_and_run_installer,
            quick_register,
            start_browser_login,
            finish_browser_login,
            cancel_browser_login,
            remove_account,
            get_accounts,
            get_account,
            switch_account,
            get_account_usage,
            update_account_token,
            refresh_token,
            refresh_token_with_password,
            login_account_with_email,
            update_account_profile,
            export_accounts,
            export_accounts_to_path,
            import_accounts,
            clear_accounts,
            get_usage_events,
            read_trae_account,
            get_machine_id,
            reset_machine_id,
            set_machine_id,
            bind_account_machine_id,
            get_trae_machine_id,
            set_trae_machine_id,
            clear_trae_login_state,
            get_trae_path,
            set_trae_path,
            scan_trae_path,
            get_user_statistics,
            open_pricing,
            check_update,
            install_update,
        ])
        .setup(|app| {
            // 创建托盘菜单
            let show_item = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
            let hide_item = MenuItem::with_id(app, "hide", "隐藏窗口", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let separator = PredefinedMenuItem::separator(app)?;
            
            let tray_menu = Menu::with_items(app, &[&show_item, &hide_item, &separator, &quit_item])?;

            // 设置托盘图标
            if let Some(icon) = app.default_window_icon() {
                let tray = TrayIconBuilder::new()
                    .icon(icon.clone())
                    .tooltip("Trae账号管理")
                    .menu(&tray_menu)
                    .on_menu_event(|app, event| {
                        match event.id().as_ref() {
                            "show" => {
                                if let Some(window) = app.get_webview_window("main") {
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                }
                            }
                            "hide" => {
                                if let Some(window) = app.get_webview_window("main") {
                                    let _ = window.hide();
                                }
                            }
                            "quit" => {
                                std::process::exit(0);
                            }
                            _ => {}
                        }
                    })
                    .build(app);
                
                if let Err(e) = tray {
                    println!("[ERROR] 创建托盘图标失败: {}", e);
                }
            }

            // 获取主窗口并显示
            if let Some(window) = app.get_webview_window("main") {
                window.show().unwrap();
                window.set_focus().unwrap();
            }
            Ok(())
        })
        .on_window_event(|window, event| match event {
            // 拦截关闭事件，改为隐藏窗口
            WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                window.hide().unwrap();
            }
            _ => {}
        })

        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
