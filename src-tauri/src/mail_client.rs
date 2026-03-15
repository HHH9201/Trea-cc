use std::time::Duration;

use reqwest::Client;
use uuid::Uuid;

const API_BASE: &str = "https://email.hhxyyq.online";

pub struct MailClient {
    client: Client,
    api_key: Option<String>,
}

impl MailClient {
    pub async fn new() -> anyhow::Result<Self> {
        Self::new_with_key(None).await
    }

    pub async fn new_with_key(api_key: Option<String>) -> anyhow::Result<Self> {
        let client = Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .timeout(Duration::from_secs(30))
            .danger_accept_invalid_certs(true)
            .build()?;

        Ok(Self { client, api_key })
    }

    /// 生成随机邮箱地址
    pub fn generate_email() -> String {
        let username = Uuid::new_v4().simple().to_string()[..12].to_string();
        format!("{}@hhxyyq.online", username)
    }

    /// 获取验证码 - API 返回格式: "验证码: 123456\n时间: ..."
    pub async fn get_code(&self) -> anyhow::Result<Option<String>> {
        let url = format!("{}/view", API_BASE);
        
        let mut request = self.client.get(&url);
        
        // 如果提供了 API 密钥，添加到请求头
        if let Some(ref key) = self.api_key {
            request = request.header("X-API-Key", key);
        }
        
        let resp = request.send().await?;
        let status = resp.status();

        if !status.is_success() {
            return Ok(None);
        }

        let content = resp.text().await?;
        
        // 尝试从格式 "验证码: 123456" 中提取
        if let Some(cap) = content.lines().next().and_then(|line| {
            let line = line.trim();
            // 匹配 "验证码: 123456" 或 "231042" 格式
            if line.starts_with("验证码:") {
                line.split(':').nth(1).map(|s| s.trim().to_string())
            } else {
                None
            }
        }) {
            if cap.len() == 6 && cap.chars().all(|c| c.is_ascii_digit()) {
                return Ok(Some(cap));
            }
        }
        
        // 备用：直接查找6位数字
        let digits: String = content.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.len() >= 6 {
            let code = &digits[..6];
            return Ok(Some(code.to_string()));
        }
        
        Ok(None)
    }
}

pub fn generate_password() -> String {
    let raw = Uuid::new_v4().simple().to_string();
    format!("A{}!{}", &raw[..6], &raw[6..12])
}

pub async fn wait_for_verification_code(client: &MailClient, timeout: Duration) -> anyhow::Result<String> {
    use std::time::Instant;

    let start = Instant::now();

    while start.elapsed() < timeout {
        match client.get_code().await {
            Ok(Some(code)) => {
                return Ok(code);
            }
            Ok(None) => {}
            Err(_) => {}
        }
        
        tokio::time::sleep(Duration::from_secs(5)).await;
    }

    Err(anyhow::anyhow!(
        "等待验证码超时 ({} 秒)\n\n可能原因:\n1. 邮件发送延迟\n2. 邮箱服务不可用\n3. 注册页未发送验证码",
        timeout.as_secs()
    ))
}
