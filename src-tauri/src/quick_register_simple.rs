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
    emit_quick_register_notice,
    mail_client::{wait_for_verification_code, MailClient, generate_password},
    api::{TraeApiClient, login_with_email},
    Account, AppState, ApiError,
};

pub async fn quick_register_simple(
    app: AppHandle,
    show_window: bool,
    state: State<'_, AppState>,
) -> Result<Account, ApiError> {
    println!("\n========================================");
    println!("[quick-register-simple] 🚀 开始快速注册流程");
    println!("========================================\n");

    // 检查是否已有浏览器登录在进行中
    if state.browser_login.lock().await.is_some() {
        return Err(ApiError::from(anyhow!("浏览器登录正在进行中，请稍后再试")));
    }

    // 获取设置中的 API 密钥
    let api_key = {
        let settings = state.settings.lock().await;
        let key = settings.api_key.clone();
        if key.is_empty() {
            println!("[quick-register-simple] ❌ 未配置 API 密钥");
            return Err(ApiError::from(anyhow!(
                "请先填写 API 密钥\n\n请在设置中填写 API 密钥后再使用快速注册功能。"
            )));
        } else {
            println!("[quick-register-simple] 使用配置的 API 密钥");
            Some(key)
        }
    };

    // 初始化
    println!("[quick-register-simple] 初始化 MailClient...");
    let mail_client = MailClient::new(api_key).await.map_err(ApiError::from)?;
    let password = generate_password();
    let email = MailClient::generate_email();
    println!("[quick-register-simple] 📧 邮箱: {}", email);
    println!("[quick-register-simple] 🔑 密码: {}******", &password[..3]);

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
            if let Some(msg) = query.get("log") {
                println!("[quick-register-js] {}", msg);
            }

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

    // 创建浏览器窗口，先关闭已存在的
    if let Some(existing) = app.get_webview_window("trae-register") {
        let _ = existing.destroy();
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
    // 再次检查确保窗口已关闭
    if app.get_webview_window("trae-register").is_some() {
        return Err(anyhow::anyhow!("无法关闭已存在的注册窗口，请重启应用后重试").into());
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
                console.log('[TokenIntercept] 捕获到 Token:', token.substring(0, 20) + '...');
                var fullUrl = callbackUrl + '?token=' + encodeURIComponent(token) + '&url=' + encodeURIComponent(url);
                if (navigator.sendBeacon) {{
                    navigator.sendBeacon(fullUrl);
                }} else {{
                    fetch(fullUrl, {{ mode: 'no-cors' }});
                }}
            }};
            
            var parseToken = function(data) {{
                if (!data) return null;
                var token = data.result?.token || data.result?.Token || data.data?.token || data.data?.Token || data.token || data.Token || null;
                // 只返回 JWT 格式的 Token（包含两个点号）
                if (token && token.split('.').length === 3) {{
                    return token;
                }}
                return null;
            }};
            
            // 拦截所有请求并记录
            var originalFetch = window.fetch;
            window.fetch = async function() {{
                var url = arguments[0];
                var urlStr = typeof url === 'string' ? url : (url.url || '');
                console.log('[TokenIntercept] Fetch请求:', urlStr.substring(0, 100));
                
                var response = await originalFetch.apply(this, arguments);
                
                // 检查是否包含 token 或 user 相关接口
                if (urlStr.includes('GetUserToken') || urlStr.includes('token') || urlStr.includes('user')) {{
                    console.log('[TokenIntercept] 捕获到可能的Token接口:', urlStr);
                    try {{
                        var cloned = response.clone();
                        var data = await cloned.json();
                        console.log('[TokenIntercept] 响应数据:', JSON.stringify(data).substring(0, 200));
                        var token = parseToken(data);
                        if (token) {{
                            console.log('[TokenIntercept] 成功提取Token');
                            sendToken(token, urlStr);
                        }}
                    }} catch (e) {{
                        console.log('[TokenIntercept] 解析失败:', e.message);
                    }}
                }}
                return response;
            }};
            
            // 拦截 XHR
            var originalOpen = XMLHttpRequest.prototype.open;
            var originalSend = XMLHttpRequest.prototype.send;
            XMLHttpRequest.prototype.open = function(method, url) {{
                this._url = url;
                console.log('[TokenIntercept] XHR请求:', (url || '').substring(0, 100));
                return originalOpen.apply(this, arguments);
            }};
            XMLHttpRequest.prototype.send = function() {{
                var xhr = this;
                var url = this._url || '';
                if (url.includes('GetUserToken') || url.includes('token') || url.includes('user')) {{
                    console.log('[TokenIntercept] 捕获到可能的Token XHR:', url);
                    this.addEventListener('load', function() {{
                        try {{
                            var data = JSON.parse(xhr.responseText);
                            console.log('[TokenIntercept] XHR响应:', JSON.stringify(data).substring(0, 200));
                            var token = parseToken(data);
                            if (token) {{
                                console.log('[TokenIntercept] 成功提取Token');
                                sendToken(token, url);
                            }}
                        }} catch (e) {{
                            console.log('[TokenIntercept] XHR解析失败:', e.message);
                        }}
                    }});
                }}
                return originalSend.apply(this, arguments);
            }};
            
            // 同时尝试从 localStorage 和 sessionStorage 获取
            var checkStorageForToken = function() {{
                console.log('[TokenIntercept] 检查 Storage...');
                var sources = [{{name: 'localStorage', storage: localStorage}}, {{name: 'sessionStorage', storage: sessionStorage}}];
                for (var i = 0; i < sources.length; i++) {{
                    var src = sources[i];
                    try {{
                        for (var key in src.storage) {{
                            var lowerKey = key.toLowerCase();
                            if (lowerKey.includes('token') || lowerKey.includes('jwt') || lowerKey.includes('auth')) {{
                                console.log('[TokenIntercept] 发现token key:', src.name, key);
                                var value = src.storage.getItem(key);
                                if (value && value.length > 20) {{
                                    console.log('[TokenIntercept] 尝试发送 Storage token');
                                    sendToken(value, src.name + ':' + key);
                                }}
                            }}
                        }}
                    }} catch(e) {{}}
                }}
            }};
            
            // 多次检查 Storage
            setTimeout(checkStorageForToken, 3000);
            setTimeout(checkStorageForToken, 8000);
            setTimeout(checkStorageForToken, 15000);
            
            // 定期检查是否有 token
            setInterval(function() {{
                if (window.__trae_last_token) return;
                checkStorageForToken();
            }}, 5000);
            
            console.log('[TokenIntercept] Token 拦截器已安装（initialization_script）');
        }})();
        "#,
        port
    );

    println!("[quick-register-simple] 创建浏览器窗口...");
    let webview = tauri::webview::WebviewWindowBuilder::new(
        &app,
        "trae-register",
        tauri::WebviewUrl::External("https://www.trae.ai/sign-up".parse().unwrap()),
    )
    .title("Trae 注册")
    .inner_size(1000.0, 720.0)
    .visible(show_window)
    .initialization_script(&init_script)  // 页面创建时就注入拦截器
    .build()
    .map_err(|e| ApiError::from(anyhow!("无法打开注册窗口: {}", e)))?;

    // 等待页面加载
    println!("[quick-register-simple] 等待页面加载...");
    tokio::time::sleep(Duration::from_secs(3)).await;

    // 填入邮箱并点击 Send Code
    println!("[quick-register-simple] 填入邮箱并点击 Send Code...");
    let email_escaped = email.replace("\"", "\\\"");
    
    for i in 1..=6 {
        tokio::time::sleep(Duration::from_millis(300)).await;
        
        // 填入邮箱
        let fill_email_script = format!(
            r#"(function() {{
                var input = document.querySelector('input[type="email"]') || document.querySelector('input[name="email"]');
                if (input && !input.value) {{
                    input.value = "{}";
                    input.dispatchEvent(new Event('input', {{ bubbles: true }}));
                    input.dispatchEvent(new Event('change', {{ bubbles: true }}));
                    console.log('[AutoFill] 邮箱已填入');
                }}
            }})()"#,
            email_escaped
        );
        let _ = webview.eval(fill_email_script);
        
        // 点击 Send Code
        if i == 3 {
            let click_script = r#"
                (function() {
                    var btn = document.querySelector('.right-part.send-code') || document.querySelector('.send-code');
                    if (btn) {
                        btn.click();
                        console.log('[AutoFill] Send Code 已点击');
                    }
                })()
            "#;
            let _ = webview.eval(click_script);
        }
    }

    // 等待验证码邮件（验证码有效期短，要尽快获取并填入）
    println!("[quick-register-simple] 等待验证码邮件...");
    
    let code = match wait_for_verification_code(&mail_client, &email, Duration::from_secs(60)).await {
        Ok(code) => code,
        Err(err) => {
            let _ = webview.close();
            return Err(ApiError::from(err));
        }
    };
    
    println!("[quick-register-simple] ✅ 获取验证码: {}", code);
    
    // ⚠️ 验证码有效期很短，立即填入并提交
    println!("[quick-register-simple] 🚀 立即填入验证码并提交...");
    let code_escaped = code.replace("\"", "\\\"");
    let password_escaped = password.replace("\"", "\\\"");
    
    // 一次性填入所有信息并点击注册
    let fill_and_submit_script = format!(
        r#"(function() {{
            // 填入验证码
            var codeInput = document.querySelector('input[placeholder*="Verification"]') || document.querySelector('input[maxlength="6"]');
            if (codeInput) {{
                codeInput.value = "{}";
                codeInput.dispatchEvent(new Event('input', {{ bubbles: true }}));
                codeInput.dispatchEvent(new Event('change', {{ bubbles: true }}));
                console.log('[AutoFill] 验证码已填入');
            }}
            
            // 填入密码
            var passInput = document.querySelector('input[type="password"]');
            if (passInput) {{
                passInput.value = "{}";
                passInput.dispatchEvent(new Event('input', {{ bubbles: true }}));
                passInput.dispatchEvent(new Event('change', {{ bubbles: true }}));
                console.log('[AutoFill] 密码已填入');
            }}
            
            // 延迟点击注册按钮，确保表单已更新
            setTimeout(function() {{
                var btn = document.querySelector('.btn-submit') || document.querySelector('.trae__btn');
                if (btn) {{
                    btn.click();
                    console.log('[AutoFill] Sign Up 已点击');
                }}
            }}, 300);
        }})()"#,
        code_escaped, password_escaped
    );
    let _ = webview.eval(fill_and_submit_script);

    // 等待注册完成
    println!("[quick-register-simple] 等待注册完成...");
    tokio::time::sleep(Duration::from_secs(4)).await;

    // 等待 Token（最多60秒）
    println!("[quick-register-simple] ⏳ 等待 Token（最多60秒）...");
    let token_result = tokio::time::timeout(Duration::from_secs(60), token_rx).await;
    
    let (token, _url, cookies) = match token_result {
        Ok(Ok((token, url))) => {
            println!("[quick-register-simple] ✅ 收到 Token");
            println!("[quick-register-simple] Token 前50字符: {}...", &token[..50.min(token.len())]);
            println!("[quick-register-simple] Token 长度: {}", token.len());
            // 获取 cookies
            let cookies = match wait_for_request_cookies(&webview, &url, Duration::from_secs(6)).await {
                Ok(cookies) => {
                    println!("[quick-register-simple] ✅ 获取到请求 cookies: {}...", &cookies[..50.min(cookies.len())]);
                    cookies
                }
                Err(e) => {
                    println!("[quick-register-simple] ⚠️ 获取请求 cookies 失败: {}", e);
                    // 尝试从页面获取 cookies
                    match webview.cookies() {
                        Ok(cookie_list) => {
                            let cookies_str = cookie_list
                                .into_iter()
                                .map(|c| format!("{}={}", c.name(), c.value()))
                                .collect::<Vec<_>>()
                                .join("; ");
                            println!("[quick-register-simple] 从页面获取到 cookies: {}...", &cookies_str[..50.min(cookies_str.len())]);
                            cookies_str
                        }
                        Err(e) => {
                            println!("[quick-register-simple] 获取页面 cookies 也失败: {}", e);
                            String::new()
                        }
                    }
                }
            };
            (Some(token), url, cookies)
        }
        _ => {
            println!("[quick-register-simple] ⚠️ 未收到 Token，尝试从页面获取 cookies...");
            // 尝试从当前页面获取 cookies
            let cookies = match webview.cookies() {
                Ok(cookie_list) => {
                    let cookies_str = cookie_list
                        .into_iter()
                        .map(|c| format!("{}={}", c.name(), c.value()))
                        .collect::<Vec<_>>()
                        .join("; ");
                    println!("[quick-register-simple] 从页面获取到 cookies: {}...", &cookies_str[..50.min(cookies_str.len())]);
                    cookies_str
                }
                Err(e) => {
                    println!("[quick-register-simple] 获取 cookies 失败: {}", e);
                    String::new()
                }
            };
            (None, String::new(), cookies)
        }
    };

    let _ = webview.close();

    // 保存账号
    println!("[quick-register-simple] 保存账号...");
    let mut manager = state.account_manager.lock().await;
    
    // 检查 Token 是否是 JWT 格式（包含两个点号）
    let is_jwt = token.as_ref().map(|t| t.split('.').count() == 3).unwrap_or(false);
    
    let mut account = if is_jwt {
        // 有 JWT Token，使用 Token 添加账号
        println!("[quick-register-simple] 使用 JWT Token 添加账号...");
        match manager.add_account_by_token(token.unwrap(), Some(cookies), Some(password.clone())).await {
            Ok(acc) => {
                println!("[quick-register-simple] ✅ 账号添加成功: user_id={}", acc.user_id);
                acc
            }
            Err(e) => {
                println!("[quick-register-simple] ❌ 使用 Token 添加账号失败: {}", e);
                return Err(ApiError::from(e));
            }
        }
    } else {
        // 没有 JWT Token，使用邮箱和密码登录获取 Token
        println!("[quick-register-simple] 浏览器未捕获 JWT Token，使用邮箱密码登录...");
        println!("[quick-register-simple] 邮箱: {}, 密码: {}******", email, &password[..3]);
        
        // 等待几秒确保账号已创建，最多重试3次
        let mut last_error = String::new();
        let mut account = None;
        
        for attempt in 0..3 {
            if attempt > 0 {
                println!("[quick-register-simple] 第 {} 次尝试登录...", attempt + 1);
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
            
            match login_with_email(&email, &password).await {
                Ok(login_result) => {
                    println!("[quick-register-simple] ✅ 邮箱登录成功，获取到 Token");
                    match manager.add_account_by_token(
                        login_result.token, 
                        Some(login_result.cookies), 
                        Some(password.clone())
                    ).await {
                        Ok(acc) => {
                            println!("[quick-register-simple] ✅ 账号添加成功: user_id={}", acc.user_id);
                            account = Some(acc);
                            break;
                        }
                        Err(e) => {
                            println!("[quick-register-simple] ❌ 使用 Token 添加账号失败: {}", e);
                            return Err(ApiError::from(e));
                        }
                    }
                }
                Err(e) => {
                    println!("[quick-register-simple] ❌ 第 {} 次邮箱登录失败: {}", attempt + 1, e);
                    last_error = e.to_string();
                    // 继续下一次重试
                }
            }
        }
        
        // 如果邮箱登录都失败了，尝试使用 Cookies
        if account.is_none() {
            if !cookies.is_empty() {
                println!("[quick-register-simple] 邮箱登录失败，尝试使用 Cookies 添加账号...");
                match manager.add_account(cookies, Some(password.clone())).await {
                    Ok(acc) => {
                        println!("[quick-register-simple] ✅ 使用 Cookies 添加账号成功: user_id={}", acc.user_id);
                        account = Some(acc);
                    }
                    Err(e2) => {
                        println!("[quick-register-simple] ❌ 使用 Cookies 也失败: {}", e2);
                        return Err(ApiError::from(anyhow!("登录失败: {}，且 Cookies 也无效: {}", last_error, e2)));
                    }
                }
            } else {
                return Err(ApiError::from(anyhow!("登录失败: {}", last_error)));
            }
        }
        
        account.unwrap()
    };

    // 更新邮箱
    if account.email.trim().is_empty() || account.email.contains('*') || !account.email.contains('@') {
        println!("[quick-register-simple] 更新账号邮箱为: {}", email);
        match manager.update_account_email(&account.id, email.clone()) {
            Ok(_) => {
                account = manager.get_account(&account.id).map_err(ApiError::from)?;
                println!("[quick-register-simple] ✅ 邮箱更新成功");
            }
            Err(e) => {
                println!("[quick-register-simple] ⚠️ 邮箱更新失败: {}", e);
            }
        }
    }

    println!("\n========================================");
    println!("[quick-register-simple] ✅ 快速注册完成!");
    println!("[quick-register-simple] 📧 邮箱: {}", account.email);
    println!("[quick-register-simple] 👤 用户名: {}", account.name);
    println!("========================================\n");

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