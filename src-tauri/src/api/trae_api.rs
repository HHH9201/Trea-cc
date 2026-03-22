use anyhow::{anyhow, Result};
use reqwest::{header, Client, Url};
use reqwest::cookie::{CookieStore, Jar};
use serde_json::json;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use std::sync::Arc;
use chrono::{Local, SecondsFormat, Utc};

use super::types::*;

const API_BASE_US: &str = "https://api-us-east.trae.ai";
const API_BASE_SG: &str = "https://api-sg-central.trae.ai";
const API_BASE_UG: &str = "https://ug-normal.trae.ai";

pub struct EmailLoginResult {
    pub token: String,
    pub user_id: String,
    pub tenant_id: String,
    pub cookies: String,
    pub expired_at: String,
}

pub struct TraeApiClient {
    client: Client,
    cookies: String,
    jwt_token: Option<String>,
    api_base: String,
}

impl TraeApiClient {
    pub fn new(cookies: &str) -> Result<Self> {
        let client = Client::builder().build()?;

        let cleaned_cookies = cookies
            .lines()
            .map(|line| line.trim())
            .collect::<Vec<_>>()
            .join("")
            .replace("  ", " ");

        let api_base = Self::detect_api_base_from_cookies(&cleaned_cookies);

        Ok(Self {
            client,
            cookies: cleaned_cookies,
            jwt_token: None,
            api_base,
        })
    }

    pub fn new_with_token(token: &str) -> Result<Self> {
        let client = Client::builder().build()?;

        Ok(Self {
            client,
            cookies: String::new(),
            jwt_token: Some(token.to_string()),
            api_base: API_BASE_SG.to_string(),
        })
    }

    pub fn new_with_token_and_cookies(token: &str, cookies: &str) -> Result<Self> {
        let mut client = Self::new(cookies)?;
        client.jwt_token = Some(token.to_string());
        Ok(client)
    }

    fn detect_api_base_from_cookies(cookies: &str) -> String {
        if cookies.contains("store-idc=useast") || cookies.contains("trae-target-idc=useast") {
            API_BASE_US.to_string()
        } else if cookies.contains("store-idc=alisg") || cookies.contains("trae-target-idc=alisg") {
            API_BASE_SG.to_string()
        } else if cookies.contains("store-country-code=us") {
            API_BASE_US.to_string()
        } else {
            API_BASE_SG.to_string()
        }
    }

    fn build_headers_token_only(&self) -> Result<header::HeaderMap> {
        let mut headers = header::HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, "application/json".parse()?);
        headers.insert(header::ACCEPT, "application/json, text/plain, */*".parse()?);
        headers.insert(header::ORIGIN, "https://www.trae.ai".parse()?);
        headers.insert(header::REFERER, "https://www.trae.ai/".parse()?);
        headers.insert(
            header::USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36".parse()?,
        );

        if let Some(token) = &self.jwt_token {
            let auth_value = header::HeaderValue::from_bytes(
                format!("Cloud-IDE-JWT {}", token).as_bytes()
            ).map_err(|e| anyhow!("Token 格式错误: {}", e))?;
            headers.insert(header::AUTHORIZATION, auth_value);
        }

        Ok(headers)
    }

    pub async fn get_user_info_by_token(&self) -> Result<TokenUserInfo> {
        let token = self.jwt_token.as_ref().ok_or_else(|| anyhow!("Token 不存在"))?;
        let jwt_data = Self::parse_jwt_token(token)?;

        let headers = self.build_headers_token_only()?;
        let endpoints = [&self.api_base, API_BASE_SG, API_BASE_US];

        let mut last_error = anyhow!("所有 API 端点都失败");

        for base in endpoints.iter() {
            let url = format!("{}/trae/api/v1/pay/user_current_entitlement_list", base);

            let response = self
                .client
                .post(&url)
                .headers(headers.clone())
                .json(&json!({"require_usage": true}))
                .send()
                .await;

            match response {
                Ok(resp) if resp.status().is_success() => {
                    match resp.json::<EntitlementListResponse>().await {
                        Ok(data) => {
                            let user_id_from_api = data.user_entitlement_pack_list
                                .first()
                                .map(|p| p.entitlement_base_info.user_id.clone())
                                .unwrap_or_else(|| jwt_data.user_id.clone());

                            let user_detail = self.get_user_info_with_token().await.ok();

                            return Ok(TokenUserInfo {
                                user_id: user_id_from_api,
                                tenant_id: jwt_data.tenant_id,
                                screen_name: user_detail.as_ref().map(|u| u.screen_name.clone()),
                                avatar_url: user_detail.as_ref().and_then(|u| {
                                    if u.avatar_url.is_empty() { None } else { Some(u.avatar_url.clone()) }
                                }),
                                email: user_detail.as_ref().and_then(|u| u.non_plain_text_email.clone()),
                            });
                        }
                        Err(e) => last_error = anyhow!("解析响应失败: {}", e),
                    }
                }
                Ok(resp) => last_error = anyhow!("API 返回错误: {}", resp.status()),
                Err(e) => last_error = anyhow!("请求失败: {}", e),
            }
        }

        Err(last_error)
    }

    async fn get_user_info_with_token(&self) -> Result<UserInfoResult> {
        let headers = self.build_headers_token_only()?;
        let endpoints = [API_BASE_UG, API_BASE_SG, API_BASE_US];
        
        let mut last_error = anyhow!("所有 GetUserInfo API 端点都失败");
        
        for base in endpoints.iter() {
            let url = format!("{}/cloudide/api/v3/trae/GetUserInfo", base);
            println!("[TraeApiClient] 尝试 GetUserInfo 端点: {}", base);

            match self
                .client
                .post(&url)
                .headers(headers.clone())
                .json(&json!({"IfWebPage": true}))
                .send()
                .await 
            {
                Ok(resp) if resp.status().is_success() => {
                    match resp.json::<GetUserInfoResponse>().await {
                        Ok(data) => return Ok(data.result),
                        Err(e) => {
                            println!("[TraeApiClient] 解析响应失败: {}", e);
                            last_error = anyhow!("解析响应失败: {}", e);
                        }
                    }
                }
                Ok(resp) => {
                    println!("[TraeApiClient] GetUserInfo API 返回错误: {}", resp.status());
                    last_error = anyhow!("API 返回错误: {}", resp.status());
                }
                Err(e) => {
                    println!("[TraeApiClient] GetUserInfo 请求失败: {}", e);
                    last_error = anyhow!("请求失败: {}", e);
                }
            }
        }

        // 所有 GetUserInfo API 端点都失败，尝试从 Token 解析基本信息
        println!("[TraeApiClient] 所有 GetUserInfo API 端点都失败，尝试从 Token 解析");
        
        if let Some(ref token) = self.jwt_token {
            if let Ok(jwt_data) = Self::parse_jwt_token(token) {
                return Ok(UserInfoResult {
                    screen_name: format!("User_{}", &jwt_data.user_id[..8.min(jwt_data.user_id.len())]),
                    gender: String::new(),
                    avatar_url: String::new(),
                    user_id: jwt_data.user_id,
                    description: String::new(),
                    tenant_id: jwt_data.tenant_id,
                    register_time: String::new(),
                    last_login_time: String::new(),
                    last_login_type: String::new(),
                    region: String::new(),
                    ai_region: Some(String::new()),
                    non_plain_text_email: None,
                    store_country: None,
                });
            }
        }

        Err(last_error)
    }

    fn parse_jwt_token(token: &str) -> Result<JwtPayload> {
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Err(anyhow!("无效的 JWT Token 格式"));
        }

        let payload_b64 = parts[1];
        let padding = (4 - payload_b64.len() % 4) % 4;
        let padded = format!("{}{}", payload_b64, "=".repeat(padding));
        let standard_b64 = padded.replace('-', "+").replace('_', "/");

        let payload_bytes = BASE64.decode(&standard_b64)
            .map_err(|e| anyhow!("解码 JWT payload 失败: {}", e))?;

        let payload_str = String::from_utf8(payload_bytes)
            .map_err(|e| anyhow!("JWT payload 不是有效的 UTF-8: {}", e))?;

        let payload: JwtPayloadRaw = serde_json::from_str(&payload_str)
            .map_err(|e| anyhow!("解析 JWT payload 失败: {}", e))?;

        Ok(JwtPayload {
            user_id: payload.data.id,
            tenant_id: payload.data.tenant_id,
        })
    }

    fn build_headers(&self, with_auth: bool) -> Result<header::HeaderMap> {
        let mut headers = header::HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, "application/json".parse()?);
        headers.insert(header::ACCEPT, "application/json, text/plain, */*".parse()?);

        if !self.cookies.trim().is_empty() {
            let cookie_value = header::HeaderValue::from_bytes(self.cookies.as_bytes())
                .map_err(|e| anyhow!("Cookie 格式错误: {}", e))?;
            headers.insert(header::COOKIE, cookie_value);
        }

        headers.insert(header::ORIGIN, "https://www.trae.ai".parse()?);
        headers.insert(header::REFERER, "https://www.trae.ai/".parse()?);
        headers.insert(
            header::USER_AGENT,
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".parse()?,
        );

        if with_auth {
            if let Some(token) = &self.jwt_token {
                let auth_value = header::HeaderValue::from_bytes(
                    format!("Cloud-IDE-JWT {}", token).as_bytes()
                ).map_err(|e| anyhow!("Token 格式错误: {}", e))?;
                headers.insert(header::AUTHORIZATION, auth_value);
            }
        }

        Ok(headers)
    }

    pub async fn get_user_token(&mut self) -> Result<UserTokenResult> {
        let mut headers = header::HeaderMap::new();
        headers.insert(header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".parse()?);
        headers.insert(header::CONTENT_TYPE, "application/json".parse()?);
        headers.insert(header::ACCEPT, "application/json, text/plain, */*".parse()?);
        headers.insert(header::ORIGIN, "https://www.trae.ai".parse()?);
        headers.insert(header::REFERER, "https://www.trae.ai/".parse()?);
        
        if !self.cookies.trim().is_empty() {
            let cookie_value = header::HeaderValue::from_bytes(self.cookies.as_bytes())
                .map_err(|e| anyhow!("Cookie 格式错误: {}", e))?;
            headers.insert(header::COOKIE, cookie_value);
        }

        // 所有端点按优先级排序
        let endpoints = vec![
            (self.api_base.as_str(), "Primary"),
            (API_BASE_SG, "SG"),
            (API_BASE_US, "US"),
            (API_BASE_UG, "UG"),
        ];
        
        let mut last_error = anyhow!("所有端点都失败");
        
        for (base, name) in endpoints {
            let url = format!("{}/cloudide/api/v3/common/GetUserToken", base);
            println!("[TraeApiClient] 尝试 GetUserToken 端点: {} ({})", name, base);
            
            // 每个端点重试2次
            for attempt in 0..2 {
                if attempt > 0 {
                    println!("[TraeApiClient] 端点 {} 第 {} 次重试...", name, attempt + 1);
                    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                }
                
                match self.client.post(&url).headers(headers.clone()).send().await {
                    Ok(resp) => {
                        let status = resp.status();
                        if status.is_success() {
                            match resp.json::<GetUserTokenResponse>().await {
                                Ok(data) => {
                                    println!("[TraeApiClient] ✅ GetUserToken 成功: {}", name);
                                    self.jwt_token = Some(data.result.token.clone());
                                    if base != self.api_base {
                                        self.api_base = base.to_string();
                                    }
                                    return Ok(data.result);
                                }
                                Err(e) => {
                                    println!("[TraeApiClient] 端点 {} 解析响应失败: {}", name, e);
                                    last_error = anyhow!("解析响应失败: {}", e);
                                }
                            }
                        } else {
                            println!("[TraeApiClient] 端点 {} 返回错误状态: {}", name, status);
                            last_error = anyhow!("HTTP 错误: {}", status);
                        }
                    }
                    Err(e) => {
                        println!("[TraeApiClient] 端点 {} 请求失败: {}", name, e);
                        if e.is_timeout() {
                            last_error = anyhow!("请求超时");
                        } else if e.is_connect() {
                            last_error = anyhow!("连接失败");
                        } else {
                            last_error = anyhow!("请求错误: {}", e);
                        }
                    }
                }
            }
        }

        Err(anyhow!("获取 Token 失败: {}", last_error))
    }

    pub async fn get_user_info(&self) -> Result<UserInfoResult> {
        let headers = self.build_headers(false)?;
        let endpoints = [API_BASE_UG, API_BASE_SG, API_BASE_US];
        
        let mut last_error = anyhow!("所有 GetUserInfo 端点都失败");
        
        for base in endpoints.iter() {
            let url = format!("{}/cloudide/api/v3/trae/GetUserInfo", base);
            println!("[TraeApiClient] 尝试 GetUserInfo 端点: {}", base);

            match self
                .client
                .post(&url)
                .headers(headers.clone())
                .json(&json!({"IfWebPage": true}))
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    match resp.json::<GetUserInfoResponse>().await {
                        Ok(data) => return Ok(data.result),
                        Err(e) => {
                            println!("[TraeApiClient] 端点 {} 解析失败: {}", base, e);
                            last_error = anyhow!("解析失败: {}", e);
                        }
                    }
                }
                Ok(resp) => {
                    println!("[TraeApiClient] 端点 {} 返回错误: {}", base, resp.status());
                    last_error = anyhow!("HTTP 错误: {}", resp.status());
                }
                Err(e) => {
                    println!("[TraeApiClient] 端点 {} 请求失败: {}", base, e);
                    last_error = anyhow!("请求失败: {}", e);
                }
            }
        }

        // 所有 GetUserInfo 端点都失败，尝试从 Token 解析基本信息
        println!("[TraeApiClient] 所有 GetUserInfo 端点都失败，尝试从 Token 解析用户信息");
        
        if let Some(ref token) = self.jwt_token {
            if let Ok(jwt_data) = Self::parse_jwt_token(token) {
                return Ok(UserInfoResult {
                    screen_name: format!("User_{}", &jwt_data.user_id[..8.min(jwt_data.user_id.len())]),
                    gender: String::new(),
                    avatar_url: String::new(),
                    user_id: jwt_data.user_id,
                    description: String::new(),
                    tenant_id: jwt_data.tenant_id,
                    register_time: String::new(),
                    last_login_time: String::new(),
                    last_login_type: String::new(),
                    region: String::new(),
                    ai_region: Some(String::new()),
                    non_plain_text_email: None,
                    store_country: None,
                });
            }
        }

        Err(last_error)
    }

    pub async fn get_entitlement_list(&self) -> Result<EntitlementListResponse> {
        let headers = self.build_headers(true)?;
        let endpoints = [self.api_base.as_str(), API_BASE_SG, API_BASE_US, API_BASE_UG];
        
        let mut last_error = anyhow!("所有 entitlement 端点都失败");
        
        for base in endpoints.iter() {
            let url = format!("{}/trae/api/v1/pay/user_current_entitlement_list", base);
            println!("[TraeApiClient] 尝试 entitlement 端点: {}", base);
            
            match self
                .client
                .post(&url)
                .headers(headers.clone())
                .json(&json!({"require_usage": true}))
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    match resp.json::<EntitlementListResponse>().await {
                        Ok(data) => {
                            println!("[TraeApiClient] ✅ entitlement 成功: {}", base);
                            return Ok(data);
                        }
                        Err(e) => {
                            println!("[TraeApiClient] 端点 {} 解析失败: {}", base, e);
                            last_error = anyhow!("解析失败: {}", e);
                        }
                    }
                }
                Ok(resp) => {
                    println!("[TraeApiClient] 端点 {} 返回错误: {}", base, resp.status());
                    last_error = anyhow!("HTTP 错误: {}", resp.status());
                }
                Err(e) => {
                    println!("[TraeApiClient] 端点 {} 请求失败: {}", base, e);
                    last_error = anyhow!("请求失败: {}", e);
                }
            }
        }
        
        Err(last_error)
    }

    pub async fn query_usage(
        &self,
        start_time: i64,
        end_time: i64,
        page_size: i32,
        page_num: i32,
    ) -> Result<UsageQueryResponse> {
        let url = format!("{}/trae/api/v1/pay/query_user_usage_group_by_session", self.api_base);
        let headers = self.build_headers(true)?;

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .json(&json!({
                "start_time": start_time,
                "end_time": end_time,
                "page_size": page_size,
                "page_num": page_num
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("查询使用记录失败: {}", response.status()));
        }

        let data: UsageQueryResponse = response.json().await?;
        Ok(data)
    }

    pub async fn get_usage_summary(&mut self) -> Result<UsageSummary> {
        if self.jwt_token.is_none() {
            self.get_user_token().await?;
        }

        let entitlements = self.get_entitlement_list().await?;
        Self::parse_entitlements_to_summary(entitlements)
    }

    pub async fn get_usage_summary_by_token(&self) -> Result<UsageSummary> {
        let headers = self.build_headers_token_only()?;
        let endpoints = [&self.api_base, API_BASE_SG, API_BASE_US];

        let mut last_error = anyhow!("所有 API 端点都失败");

        for base in endpoints.iter() {
            let url = format!("{}/trae/api/v1/pay/user_current_entitlement_list", base);
            println!("[TraeApiClient] 尝试获取额度信息: {}", base);

            let response = self
                .client
                .post(&url)
                .headers(headers.clone())
                .json(&json!({"require_usage": true}))
                .send()
                .await;

            match response {
                Ok(resp) if resp.status().is_success() => {
                    let response_text = resp.text().await?;
                    println!("[TraeApiClient] 端点 {} 响应长度: {}", base, response_text.len());

                    if !response_text.contains("user_entitlement_pack_list") {
                        println!("[TraeApiClient] 端点 {} 响应不包含 user_entitlement_pack_list", base);
                        last_error = anyhow!("响应不包含额度信息");
                        continue;
                    }

                    match serde_json::from_str::<EntitlementListResponse>(&response_text) {
                        Ok(entitlements) => {
                            println!("[TraeApiClient] ✅ 成功获取额度信息: {}", base);
                            return Self::parse_entitlements_to_summary(entitlements);
                        }
                        Err(e) => {
                            println!("[TraeApiClient] 解析响应失败: {}", e);
                            last_error = anyhow!("解析响应失败: {}", e);
                        }
                    }
                }
                Ok(resp) => {
                    let status = resp.status();
                    println!("[TraeApiClient] 端点 {} 返回错误: {}", base, status);
                    last_error = anyhow!("API 返回错误: {}", status);
                }
                Err(e) => {
                    println!("[TraeApiClient] 端点 {} 请求失败: {}", base, e);
                    last_error = anyhow!("请求失败: {}", e);
                }
            }
        }

        Err(last_error)
    }

    fn parse_entitlements_to_summary(entitlements: EntitlementListResponse) -> Result<UsageSummary> {
        let mut summary = UsageSummary::default();
        summary.is_dollar_billing = entitlements.is_dollar_usage_billing;

        for pack in entitlements.user_entitlement_pack_list {
            let base = &pack.entitlement_base_info;
            let usage = &pack.usage;
            let quota = &base.quota;

            if base.product_type == 2 {
                summary.extra_fast_request_limit = quota.premium_model_fast_request_limit;
                summary.extra_fast_request_used = usage.premium_model_fast_amount;
                summary.extra_fast_request_left = summary.extra_fast_request_limit as f64 - summary.extra_fast_request_used;
                summary.extra_expire_time = base.end_time;

                if let Some(pkg_extra) = &base.product_extra.package_extra {
                    if pkg_extra.package_source_type == 6 {
                        summary.extra_package_name = "2026 Anniversary Treat".to_string();
                    }
                }
            } else {
                summary.plan_type = if base.product_id == 0 { "Free".to_string() } else { "Pro".to_string() };
                summary.reset_time = base.end_time;

                summary.fast_request_limit = quota.premium_model_fast_request_limit;
                summary.fast_request_used = usage.premium_model_fast_amount;
                summary.fast_request_left = summary.fast_request_limit as f64 - summary.fast_request_used;

                let basic_limit = quota.basic_usage_limit as f64;
                let basic_used = usage.basic_usage_amount;
                summary.basic_dollar_limit = basic_limit;
                summary.basic_dollar_used = basic_used;
                summary.basic_dollar_left = basic_limit - basic_used;

                let bonus_limit = quota.bonus_usage_limit as f64;
                let bonus_used = usage.bonus_usage_amount;
                summary.bonus_dollar_limit = bonus_limit;
                summary.bonus_dollar_used = bonus_used;
                summary.bonus_dollar_left = bonus_limit - bonus_used;

                summary.fast_dollar_limit = basic_limit + bonus_limit;
                summary.fast_dollar_used = basic_used + bonus_used;
                summary.fast_dollar_left = summary.fast_dollar_limit - summary.fast_dollar_used;

                summary.slow_request_limit = quota.premium_model_slow_request_limit;
                summary.slow_request_used = usage.premium_model_slow_amount;
                summary.slow_request_left = summary.slow_request_limit as f64 - summary.slow_request_used;

                summary.advanced_model_limit = quota.advanced_model_request_limit;
                summary.advanced_model_used = usage.advanced_model_amount;
                summary.advanced_model_left = summary.advanced_model_limit as f64 - summary.advanced_model_used;

                summary.autocomplete_limit = quota.auto_completion_limit;
                summary.autocomplete_used = usage.auto_completion_amount;
                summary.autocomplete_left = summary.autocomplete_limit as f64 - summary.autocomplete_used;
            }
        }

        Ok(summary)
    }

    pub async fn get_user_statistic_data(&self) -> Result<UserStatisticResult> {
        let headers = self.build_headers(true)?;
        let endpoints = [API_BASE_UG, API_BASE_SG, API_BASE_US];

        let local = Local::now();
        let offset = local.offset().local_minus_utc();
        let offset_minutes = offset.div_euclid(60);
        let offset_hours = offset.div_euclid(3600);

        let payload = json!({
            "LocalTime": Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
            "Offset": offset_hours,
            "OffsetMinutes": offset_minutes
        });

        let mut last_error = anyhow!("所有 GetUserStasticData 端点都失败");

        for base in endpoints.iter() {
            let url = format!("{}/cloudide/api/v3/trae/GetUserStasticData", base);
            println!("[TraeApiClient] 尝试 GetUserStasticData 端点: {}", base);

            match self
                .client
                .post(&url)
                .headers(headers.clone())
                .json(&payload)
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    match resp.json::<GetUserStatisticResponse>().await {
                        Ok(data) => return Ok(data.result),
                        Err(e) => {
                            println!("[TraeApiClient] 端点 {} 解析失败: {}", base, e);
                            last_error = anyhow!("解析失败: {}", e);
                        }
                    }
                }
                Ok(resp) => {
                    println!("[TraeApiClient] 端点 {} 返回错误: {}", base, resp.status());
                    last_error = anyhow!("HTTP 错误: {}", resp.status());
                }
                Err(e) => {
                    println!("[TraeApiClient] 端点 {} 请求失败: {}", base, e);
                    last_error = anyhow!("请求失败: {}", e);
                }
            }
        }

        Err(last_error)
    }

    pub async fn query_birthday_bonus(&self) -> Result<bool> {
        let url = format!("{}/trae/api/v1/pay/query_birthday_bonus", self.api_base);
        let headers = self.build_headers(true)?;

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .json(&json!({}))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("查询生日奖励失败: {}", response.status()));
        }

        let data: serde_json::Value = response.json().await?;
        let claimed = data.get("result")
            .and_then(|r| r.get("claimed"))
            .and_then(|c| c.as_bool())
            .unwrap_or(false);
        
        Ok(claimed)
    }

    pub async fn claim_birthday_bonus(&self) -> Result<()> {
        let url = format!("{}/trae/api/v1/pay/claim_birthday_bonus", self.api_base);
        let headers = self.build_headers(true)?;

        let response = self
            .client
            .post(&url)
            .headers(headers)
            .json(&json!({}))
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("领取生日奖励失败: {}", response.status()));
        }

        let data: serde_json::Value = response.json().await?;
        let success = data.get("result")
            .and_then(|r| r.get("success"))
            .and_then(|s| s.as_bool())
            .unwrap_or(false);
        
        if !success {
            let msg = data.get("result")
                .and_then(|r| r.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("领取失败");
            return Err(anyhow!("{}", msg));
        }

        Ok(())
    }
}

pub async fn login_with_email(email: &str, password: &str) -> Result<EmailLoginResult> {
    fn encode_xor_hex(input: &str) -> String {
        input
            .as_bytes()
            .iter()
            .map(|b| format!("{:02x}", b ^ 0x05))
            .collect::<Vec<_>>()
            .join("")
    }

    let cookie_jar = Arc::new(Jar::default());
    let client = Client::builder()
        .cookie_store(true)
        .cookie_provider(cookie_jar.clone())
        .build()?;

    let init_url = "https://www.trae.ai/login";
    let _ = client
        .get(init_url)
        .header(header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .send()
        .await?;

    let login_url = "https://ug-normal.trae.ai/passport/web/email/login/";
    let login_params = [
        ("aid", "677332"),
        ("account_sdk_source", "web"),
        ("sdk_version", "2.1.10-tiktok"),
        ("language", "en"),
    ];

    let encoded_email = encode_xor_hex(email);
    let encoded_password = encode_xor_hex(password);
    let login_body = [
        ("mix_mode", "1"),
        ("fixed_mix_mode", "1"),
        ("email", encoded_email.as_str()),
        ("password", encoded_password.as_str()),
    ];

    let login_response = client
        .post(login_url)
        .header(header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .header(header::ORIGIN, "https://www.trae.ai")
        .header(header::REFERER, "https://www.trae.ai/")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .query(&login_params)
        .form(&login_body)
        .send()
        .await?;

    if !login_response.status().is_success() {
        return Err(anyhow!("登录请求失败: {}", login_response.status()));
    }

    let login_result: serde_json::Value = login_response.json().await?;

    let error_code = login_result.get("error_code")
        .and_then(|v| v.as_i64())
        .unwrap_or_else(|| {
            let ok = login_result.get("message")
                .and_then(|v| v.as_str())
                .map(|m| m.eq_ignore_ascii_case("success"))
                .unwrap_or(false);
            if ok { 0 } else { -1 }
        });

    if error_code != 0 {
        let description = login_result.get("description")
            .and_then(|v| v.as_str())
            .or_else(|| login_result.get("message").and_then(|v| v.as_str()))
            .unwrap_or("未知错误");
        println!("[login_with_email] ❌ 登录失败，响应: {:?}", login_result);
        return Err(anyhow!("登录失败: {}", description));
    }

    let trae_login_url = "https://ug-normal.trae.ai/cloudide/api/v3/trae/Login?type=email";

    let trae_login_response = client
        .post(trae_login_url)
        .header(header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .header(header::ORIGIN, "https://www.trae.ai")
        .header(header::REFERER, "https://www.trae.ai/")
        .header(header::CONTENT_TYPE, "application/json")
        .send()
        .await?;

    if !trae_login_response.status().is_success() {
        return Err(anyhow!("Trae 登录失败: {}", trae_login_response.status()));
    }

    let check_url = Url::parse("https://www.trae.ai")?;
    let cookies_str = cookie_jar.cookies(&check_url)
        .map(|v| v.to_str().unwrap_or_default().to_string())
        .unwrap_or_default();
    let api_base = TraeApiClient::detect_api_base_from_cookies(&cookies_str);

    let token_url = format!("{}/cloudide/api/v3/common/GetUserToken", api_base);

    let token_response = client
        .post(&token_url)
        .header(header::USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .header(header::ORIGIN, "https://www.trae.ai")
        .header(header::REFERER, "https://www.trae.ai/")
        .header(header::CONTENT_TYPE, "application/json")
        .send()
        .await?;

    if !token_response.status().is_success() {
        return Err(anyhow!("获取 Token 失败: {}", token_response.status()));
    }

    let token_data: GetUserTokenResponse = token_response.json().await?;

    let token_url_parsed = Url::parse(&token_url)?;
    let mut cookies = cookie_jar
        .cookies(&token_url_parsed)
        .map(|v| v.to_str().unwrap_or_default().to_string())
        .unwrap_or_default();
    if !cookies.is_empty() && !cookies.contains("store-idc=") && !cookies.contains("trae-target-idc=") {
        cookies = format!("{cookies}; store-idc=alisg");
    }

    Ok(EmailLoginResult {
        token: token_data.result.token,
        user_id: token_data.result.user_id,
        tenant_id: token_data.result.tenant_id,
        cookies,
        expired_at: token_data.result.expired_at,
    })
}
