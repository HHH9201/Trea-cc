//! 快速注册模块
//! 
//! 这个模块包含了 Trae 快速注册的完整逻辑：
//! 1. 生成临时邮箱地址
//! 2. 打开浏览器并自动填入邮箱
//! 3. 点击"Send Code"按钮发送验证码
//! 4. 从邮箱获取验证码
//! 5. 填入验证码和密码
//! 6. 点击注册并拦截 Token
//! 7. 保存账号到管理器

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::time::{Duration, Instant};

use anyhow::anyhow;
use reqwest::Url;
use tauri::{AppHandle, Manager, State};
use tokio::sync::oneshot;
use warp::Filter;

use crate::{
    emit_quick_register_notice,
    mail_client::{wait_for_verification_code, MailClient, generate_password},
    Account, AppState, ApiError,
};

/// 快速注册的主函数
/// 
/// # 流程
/// 1. 生成临时邮箱和密码
/// 2. 打开浏览器窗口
/// 3. 自动填入邮箱并点击"Send Code"
/// 4. 等待验证码
/// 5. 填入验证码和密码并注册
/// 6. 拦截 Token 并保存账号
pub async fn quick_register(
    app: AppHandle,
    show_window: bool,
    state: State<'_, AppState>,
) -> Result<Account, ApiError> {
    println!("\n========================================");
    println!("[quick-register] 🚀 开始快速注册流程");
    println!("========================================\n");

    // 检查是否已有浏览器登录在进行中
    if state.browser_login.lock().await.is_some() {
        println!("[quick-register] ❌ 浏览器登录正在进行中，退出");
        return Err(ApiError::from(anyhow!("浏览器登录正在进行中，请稍后再试")));
    }

    // 获取设置中的 API 密钥
    let api_key = {
        let settings = state.settings.lock().await;
        let key = settings.api_key.clone();
        if key.is_empty() {
            println!("[quick-register] ❌ 未配置 API 密钥");
            return Err(ApiError::from(anyhow!(
                "请先填写 API 密钥\n\n请在设置中填写 API 密钥后再使用快速注册功能。"
            )));
        } else {
            println!("[quick-register] 使用配置的 API 密钥");
            Some(key)
        }
    };

    // ========== 步骤 1: 初始化 ==========
    println!("[quick-register] 步骤 1/6: 初始化 MailClient...");
    let mail_client = MailClient::new(api_key).await.map_err(ApiError::from)?;

    println!("[quick-register] 步骤 2/6: 生成随机密码...");
    let password = generate_password();

    println!("[quick-register] 步骤 3/6: 生成随机邮箱地址...");
    let email = MailClient::generate_email();
    println!("[quick-register] 📧 邮箱: {}", email);
    println!("[quick-register] 🔑 密码: {}******", &password[..3]);

    emit_quick_register_notice(&app, "email_created", &format!("临时邮箱已创建: {}", email));

    // ========== 步骤 2: 启动本地回调服务器 ==========
    let (token_tx, token_rx) = oneshot::channel::<(String, String)>();
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    let token_sender = Arc::new(StdMutex::new(Some(token_tx)));
    let shutdown_sender = Arc::new(StdMutex::new(Some(shutdown_tx)));

    let token_sender_route = token_sender.clone();
    let shutdown_sender_route = shutdown_sender.clone();

    // 创建 warp 路由来接收浏览器回调
    let route = warp::path("callback")
        .and(warp::query::<HashMap<String, String>>())
        .map(move |query: HashMap<String, String>| {
            // 处理日志
            if let Some(msg) = query.get("log") {
                println!("[quick-register-js] {}", msg);
                return warp::reply::html("ok".to_string());
            }

            let token = query.get("token").cloned().unwrap_or_default();
            let url = query.get("url").cloned().unwrap_or_default();

            if !token.is_empty() {
                // 收到 Token，发送给主流程
                if let Some(tx) = token_sender_route.lock().unwrap().take() {
                    let _ = tx.send((token, url));
                }
                // 关闭服务器
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

    // ========== 步骤 3: 创建浏览器窗口 ==========
    let pending_completion: Arc<StdMutex<Option<(String, String)>>> = Arc::new(StdMutex::new(None));
    let pending_completion_onload = pending_completion.clone();
    
    let helper_script = build_register_helper_script(addr.port());
    let _helper_script_onload = helper_script.clone();
    let _helper_script_init = helper_script.clone();
    let email_onload = email.clone();

    // 关闭已存在的注册窗口
    if let Some(existing) = app.get_webview_window("trae-register") {
        let _ = existing.destroy();
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }
    // 再次检查确保窗口已关闭
    if app.get_webview_window("trae-register").is_some() {
        return Err(anyhow::anyhow!("无法关闭已存在的注册窗口，请重启应用后重试").into());
    }

    println!("[quick-register] 创建浏览器窗口 (show_window={})...", show_window);
    
    // 创建新的浏览器窗口 - 直接加载目标页面
    let webview = tauri::webview::WebviewWindowBuilder::new(
        &app,
        "trae-register",
        tauri::WebviewUrl::External("https://www.trae.ai/sign-up".parse().unwrap()),
    )
    .title("Trae 注册")
    .inner_size(1000.0, 720.0)
    .visible(show_window)
    .on_page_load(move |window, payload| {
        let url = payload.url();
        println!("[quick-register] 页面加载事件: {:?} - {}", payload.event(), url);
        
        if payload.event() == tauri::webview::PageLoadEvent::Finished {
            println!("[quick-register] 页面加载完成");
            
            if let Some((code, password)) = pending_completion_onload.lock().unwrap().clone() {
                // 页面加载完成，填入验证码和密码
                println!("[quick-register] 调用 complete 函数填入验证码和密码");
                let code_js = serde_json::to_string(&code).unwrap_or_else(|_| "\"\"".to_string());
                let password_js = serde_json::to_string(&password).unwrap_or_else(|_| "\"\"".to_string());
                let _ = window.eval(format!(
                    "if (window.__traeAutoRegister) {{ window.__traeAutoRegister.complete({}, {}); console.log('[TraeAuto] complete called'); }} else {{ console.error('[TraeAuto] __traeAutoRegister not found'); }}"
                    , code_js, password_js
                ));
            } else {
                // 页面加载完成，填入邮箱
                println!("[quick-register] 调用 start 函数填入邮箱");
                let email_js = serde_json::to_string(&email_onload).unwrap_or_else(|_| "\"\"".to_string());
                let _ = window.eval(format!(
                    "if (window.__traeAutoRegister) {{ window.__traeAutoRegister.start({}); console.log('[TraeAuto] start called from on_page_load'); }} else {{ console.error('[TraeAuto] __traeAutoRegister not found'); }}"
                    , email_js
                ));
            }
        }
    })
    .build()
    .map_err(|e| ApiError::from(anyhow!("无法打开注册窗口: {}", e)))?;
    
    println!("[quick-register] 浏览器窗口创建成功");

    if !show_window {
        emit_quick_register_notice(&app, "quick_register_init", "初始化完成，等待接收邮箱验证码");
    }

    // ========== 步骤 4: 等待页面加载并注入脚本 ==========
    println!("[quick-register] 步骤 4/6: 等待页面加载...");
    
    // 等待页面开始加载
    tokio::time::sleep(Duration::from_secs(2)).await;
    
    // 先测试一个简单的 JavaScript 是否能执行
    println!("[quick-register] 测试 JavaScript 执行...");
    let test_result = webview.eval("console.log('[TEST] JavaScript is working'); 'test_ok'".to_string());
    match test_result {
        Ok(_) => println!("[quick-register] ✅ JavaScript 测试执行成功"),
        Err(e) => println!("[quick-register] ❌ JavaScript 测试执行失败: {}", e),
    }
    
    // 注入脚本（在页面加载过程中注入，确保在页面完成前脚本已就绪）
    println!("[quick-register] 注入 JavaScript 脚本...");
    let script_result = webview.eval(helper_script.clone());
    match script_result {
        Ok(_) => println!("[quick-register] ✅ 脚本注入成功"),
        Err(e) => println!("[quick-register] ❌ 脚本注入失败: {}", e),
    }
    
    // 等待页面完全加载
    println!("[quick-register] 等待5秒让页面完全加载...");
    tokio::time::sleep(Duration::from_secs(5)).await;
    
    // 再次注入脚本（确保在页面加载完成后脚本仍然存在）
    println!("[quick-register] 再次注入脚本...");
    let _ = webview.eval(helper_script.clone());
    
    // 验证脚本是否已加载
    println!("[quick-register] 验证脚本是否已加载...");
    let _ = webview.eval("console.log('[TEST] __traeAutoRegister exists:', typeof window.__traeAutoRegister);".to_string());
    
    // 多次调用 start 函数，确保邮箱被填入并点击 Send Code
    println!("[quick-register] 开始填入邮箱并点击 Send Code...");
    let email_js = serde_json::to_string(&email).unwrap_or_else(|_| "\"\"".to_string());
    let mut countdown_started = false;
    let mut attempts = 0;
    let max_attempts = 30;
    
    while !countdown_started && attempts < max_attempts {
        attempts += 1;
        tokio::time::sleep(Duration::from_millis(500)).await;
        println!("[quick-register] 第 {} 次调用 start 函数...", attempts);
        
        // 调用 start 并检查返回值
        let result = webview.eval(format!(
            "(function() {{ 
                if (window.__traeAutoRegister) {{ 
                    var result = window.__traeAutoRegister.start({}); 
                    console.log('[TraeAuto] start returned:', result); 
                    return result;
                }} else {{ 
                    console.log('[TraeAuto] not found'); 
                    return 'not_found'; 
                }}
            }})()"
            , email_js
        ));
        
        // 检查是否已进入倒计时状态（通过全局变量）
        // 注意：eval 不返回值，我们通过检查页面元素来判断
        let _ = webview.eval(
            "if (window.__traeAutoRegisterState && window.__traeAutoRegisterState.countdownStarted) { 
                console.log('[TraeAuto] Countdown started!'); 
            }".to_string()
        );
        
        // 每5次尝试后额外等待一下
        if attempts % 5 == 0 {
            println!("[quick-register] 额外等待1秒...");
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
    
    if countdown_started {
        println!("[quick-register] ✅ Send Code 已进入倒计时，验证码已发送");
    } else {
        println!("[quick-register] ⚠️ 未检测到倒计时状态，继续尝试...");
    }

    // 等待3秒让邮件发送并到达
    println!("[quick-register] 等待3秒让邮件发送并到达...");
    tokio::time::sleep(Duration::from_secs(3)).await;

    // ========== 步骤 5: 等待验证码邮件 ==========
    println!("[quick-register] 步骤 5/6: 等待验证码邮件...");
    let code = match wait_for_verification_code(&mail_client, &email, Duration::from_secs(60)).await {
        Ok(code) => code,
        Err(err) => {
            let _ = webview.close();
            if !show_window {
                emit_quick_register_notice(
                    &app,
                    "quick_register_failed",
                    "快速注册失败，可在设置中开启快速注册显示浏览器查看失败原因。",
                );
            }
            return Err(ApiError::from(err));
        }
    };

    println!("[quick-register] ✅ 获取验证码成功: {}", code);
    if !show_window {
        emit_quick_register_notice(&app, "quick_register_code_ok", "邮箱验证码获取成功，正在登录");
    }

    // ========== 步骤 6: 填入验证码和密码并提交注册 ==========
    println!("[quick-register] 步骤 6/6: 填入验证码和密码并提交注册...");
    *pending_completion.lock().unwrap() = Some((code.clone(), password.clone()));
    let code_js = serde_json::to_string(&code).unwrap_or_else(|_| "\"\"".to_string());
    let password_js = serde_json::to_string(&password).unwrap_or_else(|_| "\"\"".to_string());

    println!("[quick-register] 📤 发送验证码到浏览器: {}", code);
    println!("[quick-register] 📤 发送密码到浏览器: {}******", &password[..3]);

    // 多次调用 complete 函数，确保注册按钮被点击
    println!("[quick-register] 开始调用 complete 函数...");
    for i in 1..=10 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        println!("[quick-register] 第 {} 次调用 complete 函数...", i);
        let _ = webview.eval(format!(
            "if (window.__traeAutoRegister) {{ window.__traeAutoRegister.complete({}, {}); }} else {{ console.log('[TraeAuto] complete not found'); }}"
            , code_js, password_js
        ));
    }
    
    // 额外等待让注册请求完成
    println!("[quick-register] 等待5秒让注册请求完成...");
    tokio::time::sleep(Duration::from_secs(5)).await;

    // ========== 等待注册完成并拦截 Token ==========
    println!("[quick-register] ⏳ 等待注册完成并拦截 Token...");
    let (token, url) = match token_rx.await {
        Ok((token, url)) => {
            println!("[quick-register] ✅ 成功拦截 Token: {}...", &token[..20.min(token.len())]);
            println!("[quick-register] 📍 Token 来源 URL: {}", url);
            (token, url)
        }
        Err(_) => {
            println!("[quick-register] ❌ Token 等待超时或通道已关闭");
            let _ = webview.close();
            if !show_window {
                emit_quick_register_notice(
                    &app,
                    "quick_register_failed",
                    "快速注册失败，可在设置中开启快速注册显示浏览器查看失败原因。",
                );
            }
            return Err(ApiError::from(anyhow!("等待 Token 超时或失败")));
        }
    };

    println!("[quick-register] ⏳ 正在获取 Cookies...");
    let cookies = match wait_for_request_cookies(&webview, &url, Duration::from_secs(6)).await {
        Ok(cookies) => {
            println!("[quick-register] ✅ 成功获取 Cookies: {}...", &cookies[..50.min(cookies.len())]);
            cookies
        }
        Err(err) => {
            println!("[quick-register] ❌ 获取 Cookies 失败: {}", err);
            let _ = webview.close();
            if !show_window {
                emit_quick_register_notice(
                    &app,
                    "quick_register_failed",
                    "获取登录 Cookie 失败，请重试。",
                );
            }
            return Err(ApiError::from(err));
        }
    };

    if !show_window {
        emit_quick_register_notice(&app, "quick_register_login_ok", "登录成功，正在导入账号");
    }

    println!("[quick-register] 🔒 关闭浏览器窗口...");
    let _ = webview.close();

    // ========== 保存账号到管理器 ==========
    println!("[quick-register] 💾 正在添加账号到管理器...");
    let mut manager = state.account_manager.lock().await;
    let mut account = manager
        .add_account_by_token(token, Some(cookies), Some(password))
        .await
        .map_err(ApiError::from)?;
    println!("[quick-register] ✅ 账号添加成功，ID: {}", account.id);

    // 检查是否需要更新邮箱
    let needs_email_override = account.email.trim().is_empty()
        || account.email.contains('*')
        || !account.email.contains('@');
    if needs_email_override {
        println!("[quick-register] 📝 更新账号邮箱为: {}", email);
        manager
            .update_account_email(&account.id, email.clone())
            .map_err(ApiError::from)?;
        account = manager.get_account(&account.id).map_err(ApiError::from)?;
        println!("[quick-register] ✅ 邮箱更新成功");
    }

    if !show_window {
        emit_quick_register_notice(&app, "quick_register_done", "导入成功");
    }

    println!("\n========================================");
    println!("[quick-register] ✅ 快速注册流程完成!");
    println!("[quick-register] 📧 邮箱: {}", account.email);
    println!("[quick-register] 🆔 账号ID: {}", account.id);
    println!("========================================\n");

    Ok(account)
}

/// 构建注册辅助脚本
/// 
/// 这个脚本会被注入到浏览器中，用于：
/// - 自动填入邮箱
/// - 点击"Send Code"按钮
/// - 填入验证码和密码
/// - 点击注册按钮
/// - 拦截 Token 并发送回 Rust
fn build_register_helper_script(port: u16) -> String {
    // 读取 register_simple.js 文件内容（简化版用于测试）
    let script = include_str!("../scripts/register_simple.js");
    script.replace("__PORT__", &port.to_string())
}

/// 等待请求 Cookies
/// 
/// 在指定时间内轮询获取指定 URL 的 Cookies
pub async fn wait_for_request_cookies(
    webview: &tauri::webview::WebviewWindow,
    request_url: &str,
    timeout: Duration,
) -> anyhow::Result<String> {
    let parsed_url = normalize_request_url(request_url)
        .ok_or_else(|| anyhow!("GetUserToken URL 无效: {}", request_url))?;
    let start = Instant::now();
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
    Err(anyhow!("未能获取 GetUserToken 请求 Cookie"))
}

/// 规范化请求 URL
fn normalize_request_url(url: &str) -> Option<Url> {
    let trimmed = url.split('?').next().unwrap_or(url);
    Url::parse("https://www.trae.ai/")
        .ok()?
        .join(trimmed)
        .ok()
}