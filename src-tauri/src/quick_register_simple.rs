//! 简化版快速注册模块 - 直接使用 eval 执行 DOM 操作

use std::time::Duration;
use anyhow::anyhow;
use reqwest::Url;
use tauri::{AppHandle, Manager, State};
use tokio::sync::oneshot;
use warp::Filter;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use crate::{
    mail_client::{wait_for_verification_code, MailClient, generate_password},
    Account, AppState, ApiError,
};

pub async fn quick_register_simple(
    app: AppHandle,
    _show_window: bool,
    state: State<'_, AppState>,
) -> Result<Account, ApiError> {
    // 检查是否已有浏览器登录在进行中
    if state.browser_login.lock().await.is_some() {
        return Err(ApiError::from(anyhow!("浏览器登录正在进行中，请稍后再试")));
    }

    // 读取设置获取 API 密钥
    let settings = state.settings.lock().await.clone();
    let api_key = if settings.api_key.is_empty() {
        None
    } else {
        Some(settings.api_key.clone())
    };

    let mail_client = MailClient::new_with_key(api_key).await.map_err(ApiError::from)?;
    let password = generate_password();
    let email = MailClient::generate_email();

    // 启动本地回调服务器
    let (token_tx, token_rx) = oneshot::channel::<(String, String)>();
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let token_sender = Arc::new(StdMutex::new(Some(token_tx)));
    let shutdown_sender = Arc::new(StdMutex::new(Some(shutdown_tx)));

    let token_sender_route = token_sender.clone();
    let shutdown_sender_route = shutdown_sender.clone();

    let route = warp::path("callback")
        .and(warp::query::<HashMap<String, String>>())
        .map(move |query: HashMap<String, String>| {
            let token = query.get("token").cloned().unwrap_or_default();
            let url = query.get("url").cloned().unwrap_or_default();

            if !token.is_empty() {
                if let Some(tx) = token_sender_route.lock().unwrap().take() {
                    let _ = tx.send((token, url));
                }
                if let Some(tx) = shutdown_sender_route.lock().unwrap().take() {
                    let _ = tx.send(());
                }
                warp::reply::html("已收到 Token，注册成功。".to_string())
            } else {
                warp::reply::html("未收到 Token".to_string())
            }
        });

    let (addr, server) = warp::serve(route)
        .bind_with_graceful_shutdown(([127, 0, 0, 1], 0), async move {
            let _ = shutdown_rx.await;
        });
    tokio::spawn(server);

    // 创建浏览器窗口
    if let Some(existing) = app.get_webview_window("trae-register") {
        let _ = existing.close();
    }

    // 准备 Token 拦截脚本（在页面创建时就注入）
    let port = addr.port();
    let init_script = format!(
        r#"
        (function() {{
            if (window.__tokenInterceptorInstalled) return;
            window.__tokenInterceptorInstalled = true;
            
            var callbackUrl = 'http://127.0.0.1:{}/callback';
            
            var sendToken = function(token, url) {{
                if (!token) return;
                var fullUrl = callbackUrl + '?token=' + encodeURIComponent(token) + '&url=' + encodeURIComponent(url);
                if (navigator.sendBeacon) {{
                    navigator.sendBeacon(fullUrl);
                }} else {{
                    fetch(fullUrl, {{ mode: 'no-cors' }});
                }}
            }};
            
            var parseToken = function(data) {{
                if (!data) return null;
                return data.result?.token || data.result?.Token || data.data?.token || data.data?.Token || data.token || data.Token || null;
            }};
            
            // 拦截所有请求并记录
            var originalFetch = window.fetch;
            window.fetch = async function() {{
                var url = arguments[0];
                var urlStr = typeof url === 'string' ? url : (url.url || '');
                
                var response = await originalFetch.apply(this, arguments);
                
                // 检查是否包含 token 或 user 相关接口
                if (urlStr.includes('GetUserToken') || urlStr.includes('token') || urlStr.includes('user')) {{
                    try {{
                        var cloned = response.clone();
                        var data = await cloned.json();
                        var token = parseToken(data);
                        if (token) {{
                            sendToken(token, urlStr);
                        }}
                    }} catch (e) {{}}
                }}
                return response;
            }};
            
            // 拦截 XHR
            var originalOpen = XMLHttpRequest.prototype.open;
            var originalSend = XMLHttpRequest.prototype.send;
            XMLHttpRequest.prototype.open = function(method, url) {{
                this._url = url;
                return originalOpen.apply(this, arguments);
            }};
            XMLHttpRequest.prototype.send = function() {{
                var xhr = this;
                var url = this._url || '';
                if (url.includes('GetUserToken') || url.includes('token') || url.includes('user')) {{
                    this.addEventListener('load', function() {{
                        try {{
                            var data = JSON.parse(xhr.responseText);
                            var token = parseToken(data);
                            if (token) {{
                                sendToken(token, url);
                            }}
                        }} catch (e) {{}}
                    }});
                }}
                return originalSend.apply(this, arguments);
            }};
        }})();
        "#,
        port
    );

    let webview = tauri::webview::WebviewWindowBuilder::new(
        &app,
        "trae-register",
        tauri::WebviewUrl::External("https://www.trae.ai/sign-up".parse().unwrap()),
    )
    .title("Trae 注册")
    .inner_size(1000.0, 720.0)
    .visible(true)
    .initialization_script(&init_script)
    .build()
    .map_err(|e| ApiError::from(anyhow!("无法打开注册窗口: {}", e)))?;

    // 等待页面加载
    tokio::time::sleep(Duration::from_secs(5)).await;

    // 填入邮箱并点击 Send Code
    let email_escaped = email.replace("\"", "\\\"");
    
    for i in 1..=10 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // 填入邮箱
        let fill_email_script = format!(
            r#"(function() {{
                var input = document.querySelector('input[type="email"]') || document.querySelector('input[name="email"]');
                if (input && !input.value) {{
                    input.value = "{}";
                    input.dispatchEvent(new Event('input', {{ bubbles: true }}));
                    input.dispatchEvent(new Event('change', {{ bubbles: true }}));
                }}
            }})()"#,
            email_escaped
        );
        let _ = webview.eval(fill_email_script);
        
        // 点击 Send Code
        if i == 5 {
            let click_script = r#"
                (function() {
                    var btn = document.querySelector('.right-part.send-code') || document.querySelector('.send-code');
                    if (btn) {
                        btn.click();
                    }
                })()
            "#;
            let _ = webview.eval(click_script);
        }
    }

    // 等待验证码邮件
    tokio::time::sleep(Duration::from_secs(5)).await;
    
    let code = match wait_for_verification_code(&mail_client, Duration::from_secs(60)).await {
        Ok(code) => code,
        Err(err) => {
            let _ = webview.close();
            return Err(ApiError::from(err));
        }
    };

    // 填入验证码、密码并点击注册
    let code_escaped = code.replace("\"", "\\\"");
    let password_escaped = password.replace("\"", "\\\"");
    
    for i in 1..=10 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // 填入验证码
        let fill_code_script = format!(
            r#"(function() {{
                var input = document.querySelector('input[placeholder*="Verification"]') || document.querySelector('input[maxlength="6"]');
                if (input) {{
                    input.value = "{}";
                    input.dispatchEvent(new Event('input', {{ bubbles: true }}));
                    input.dispatchEvent(new Event('change', {{ bubbles: true }}));
                }}
            }})()"#,
            code_escaped
        );
        let _ = webview.eval(fill_code_script);
        
        // 填入密码
        let fill_pass_script = format!(
            r#"(function() {{
                var input = document.querySelector('input[type="password"]');
                if (input) {{
                    input.value = "{}";
                    input.dispatchEvent(new Event('input', {{ bubbles: true }}));
                    input.dispatchEvent(new Event('change', {{ bubbles: true }}));
                }}
            }})()"#,
            password_escaped
        );
        let _ = webview.eval(fill_pass_script);
        
        // 点击注册按钮
        if i >= 5 {
            let click_script = r#"
                (function() {
                    var btn = document.querySelector('.btn-submit') || document.querySelector('.trae__btn');
                    if (btn) {
                        btn.click();
                    }
                })()
            "#;
            let _ = webview.eval(click_script);
        }
    }

    // 等待注册完成
    tokio::time::sleep(Duration::from_secs(5)).await;

    // 等待 Token（最多30秒）
    let token_result = tokio::time::timeout(Duration::from_secs(30), token_rx).await;
    
    let (token, _url, cookies) = match token_result {
        Ok(Ok((token, url))) => {
            // 获取 cookies
            let cookies = match wait_for_request_cookies(&webview, &url, Duration::from_secs(6)).await {
                Ok(cookies) => cookies,
                Err(_) => String::new(),
            };
            (Some(token), url, cookies)
        }
        _ => {
            // 尝试从当前页面获取 cookies
            let cookies = match webview.cookies() {
                Ok(cookie_list) => {
                    cookie_list
                        .into_iter()
                        .map(|c| format!("{}={}", c.name(), c.value()))
                        .collect::<Vec<_>>()
                        .join("; ")
                }
                Err(_) => String::new(),
            };
            (None, String::new(), cookies)
        }
    };

    let _ = webview.close();

    // 保存账号
    let mut manager = state.account_manager.lock().await;
    
    let mut account = if let Some(token) = token {
        // 有 Token，使用 Token 添加账号
        manager
            .add_account_by_token(token, Some(cookies), Some(password))
            .await
            .map_err(ApiError::from)?
    } else {
        // 没有 Token，但有 Cookies，尝试使用 add_account
        if cookies.is_empty() {
            return Err(ApiError::from(anyhow!("未能获取 Token 或 Cookies")));
        }
        
        // 使用 add_account 方法（它会通过 cookies 获取 token 和用户信息）
        manager.add_account(cookies, Some(password)).await.map_err(ApiError::from)?
    };

    // 更新邮箱
    if account.email.trim().is_empty() || account.email.contains('*') || !account.email.contains('@') {
        manager.update_account_email(&account.id, email.clone()).map_err(ApiError::from)?;
        account = manager.get_account(&account.id).map_err(ApiError::from)?;
    }

    Ok(account)
}

async fn wait_for_request_cookies(
    webview: &tauri::webview::WebviewWindow,
    request_url: &str,
    timeout: Duration,
) -> anyhow::Result<String> {
    let parsed_url = normalize_request_url(request_url)
        .ok_or_else(|| anyhow!("URL 无效: {}", request_url))?;
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if let Ok(cookie_list) = webview.cookies_for_url(parsed_url.clone()) {
            let cookies = cookie_list
                .into_iter()
                .map(|c| format!("{}={}", c.name(), c.value()))
                .collect::<Vec<_>>()
                .join("; ");
            if !cookies.is_empty() {
                return Ok(cookies);
            }
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    Err(anyhow!("未能获取 Cookie"))
}

fn normalize_request_url(url: &str) -> Option<Url> {
    let trimmed = url.split('?').next().unwrap_or(url);
    Url::parse("https://www.trae.ai/")
        .ok()?
        .join(trimmed)
        .ok()
}
