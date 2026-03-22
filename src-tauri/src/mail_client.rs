use std::time::Duration;

use reqwest::Client;
use uuid::Uuid;

const API_BASE: &str = "https://email.hhxyyq.online";

pub struct MailClient {
    client: Client,
    api_key: String,
}

impl MailClient {
    pub async fn new(api_key: Option<String>) -> anyhow::Result<Self> {
        println!("[MailClient] 初始化 HTTP 客户端...");
        let client = Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .timeout(Duration::from_secs(30))
            .danger_accept_invalid_certs(true)
            .build()?;

        let api_key = api_key.unwrap_or_default();
        println!("[MailClient] HTTP 客户端初始化成功");
        if !api_key.is_empty() {
            println!("[MailClient] 使用自定义 API 密钥");
        } else {
            println!("[MailClient] 使用默认服务（无 API 密钥）");
        }
        Ok(Self { client, api_key })
    }

    /// 生成随机邮箱地址
    pub fn generate_email() -> String {
        let username = Uuid::new_v4().simple().to_string()[..12].to_string();
        let email = format!("{}@hhxyyq.online", username);
        println!("[MailClient] 生成随机邮箱地址: {}", email);
        email
    }

    /// 获取验证码 - 需要指定邮箱地址
    pub async fn get_code(&self, email: &str) -> anyhow::Result<Option<String>> {
        // 使用 API 密钥和邮箱地址构建端点
        let url = if self.api_key.is_empty() {
            format!("{}/view", API_BASE)
        } else {
            format!("{}/api/get-code?key={}&email={}", API_BASE, self.api_key, email)
        };
        println!("[MailClient] 正在请求验证码: GET {}", url);
        
        let resp = match self.client.get(&url).send().await {
            Ok(resp) => resp,
            Err(e) => {
                println!("[MailClient] ❌ 请求失败: {}", e);
                if e.is_connect() {
                    println!("[MailClient]    连接错误，请检查网络或 API 地址");
                }
                if e.is_timeout() {
                    println!("[MailClient]    请求超时");
                }
                return Err(e.into());
            }
        };
        
        let status = resp.status();
        println!("[MailClient] API 响应状态: {}", status);

        if !status.is_success() {
            let error_text = resp.text().await.unwrap_or_default();
            println!("[MailClient] API 请求失败: {}", error_text);
            return Ok(None);
        }

        let content = resp.text().await?;
        println!("[MailClient] API 返回内容: '{}' (长度: {})", content, content.len());
        
        // 解析 JSON 格式 {"email":"xxx@xxx.com","code":"123456","time":"..."}
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
            // 检查是否有错误
            if let Some(error) = json.get("error").and_then(|v| v.as_str()) {
                println!("[MailClient] API 返回错误: {}", error);
                return Ok(None);
            }
            // 提取验证码
            if let Some(code) = json.get("code").and_then(|v| v.as_str()) {
                if code.len() == 6 && code.chars().all(|c| c.is_ascii_digit()) {
                    println!("[MailClient] 从 JSON 成功提取验证码: {}", code);
                    return Ok(Some(code.to_string()));
                }
            }
        }
        
        println!("[MailClient] 未找到有效验证码，等待中...");
        Ok(None)
    }
}

pub fn generate_password() -> String {
    let raw = Uuid::new_v4().simple().to_string();
    let password = format!("A{}!{}", &raw[..6], &raw[6..12]);
    println!("[MailClient] 生成随机密码: {}******", &password[..3]);
    password
}

pub async fn wait_for_verification_code(client: &MailClient, email: &str, timeout: Duration) -> anyhow::Result<String> {
    use std::time::Instant;

    println!("[MailClient] 开始等待验证码，邮箱: {}，超时时间: {} 秒", email, timeout.as_secs());
    let start = Instant::now();
    let mut check_count = 0;

    while start.elapsed() < timeout {
        check_count += 1;
        println!("[MailClient] 第 {} 次检查验证码...", check_count);
        
        match client.get_code(email).await {
            Ok(Some(code)) => {
                println!("[MailClient] ✅ 第 {} 次检查成功找到验证码: {}", check_count, code);
                println!("[MailClient] 总耗时: {} 秒", start.elapsed().as_secs());
                return Ok(code);
            }
            Ok(None) => {
                let elapsed = start.elapsed().as_secs();
                if check_count % 3 == 0 {
                    println!("[MailClient] ⏳ 等待验证码中... (已等待 {} 秒)", elapsed);
                }
            }
            Err(e) => {
                println!("[MailClient] ❌ 检查错误: {}", e);
            }
        }
        
        // 缩短等待时间，验证码只有1分钟有效期
        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    println!("[MailClient] ❌ 等待验证码超时 ({} 秒)", timeout.as_secs());
    Err(anyhow::anyhow!(
        "等待验证码超时 ({} 秒)\n\n可能原因:\n1. 邮件发送延迟\n2. 邮箱服务不可用\n3. 注册页未发送验证码\n4. 验证码已过期（超过1分钟）",
        timeout.as_secs()
    ))
}
