// 账号简要信息
export interface AccountBrief {
  id: string;
  name: string;
  email: string;
  avatar_url: string;
  plan_type: string;
  is_active: boolean;
  created_at: number;
  machine_id: string | null;
  is_current: boolean; // 是否是当前 Trae IDE 正在使用的账号
}

// 完整账号信息
export interface Account {
  id: string;
  name: string;
  email: string;
  avatar_url: string;
  cookies: string;
  jwt_token: string | null;
  token_expired_at: string | null;
  password?: string | null;
  user_id: string;
  tenant_id: string;
  region: string;
  plan_type: string;
  created_at: number;
  updated_at: number;
  is_active: boolean;
  machine_id: string | null;
}

// 使用量汇总
export interface UsageSummary {
  plan_type: string;
  reset_time: number;

  // Fast Request - 请求次数
  fast_request_used: number;
  fast_request_limit: number;
  fast_request_left: number;

  // Fast Request - 美元额度 (新账号 3 美元额度显示用)
  fast_dollar_used: number;
  fast_dollar_limit: number;
  fast_dollar_left: number;

  // Basic 额度 (基础 $3)
  basic_dollar_limit: number;
  basic_dollar_used: number;
  basic_dollar_left: number;

  // Bonus 额度 (奖励 $3)
  bonus_dollar_limit: number;
  bonus_dollar_used: number;
  bonus_dollar_left: number;

  // Extra Package
  extra_fast_request_used: number;
  extra_fast_request_limit: number;
  extra_fast_request_left: number;
  extra_expire_time: number;
  extra_package_name: string;

  // Slow Request
  slow_request_used: number;
  slow_request_limit: number;
  slow_request_left: number;

  // Advanced Model
  advanced_model_used: number;
  advanced_model_limit: number;
  advanced_model_left: number;

  // Autocomplete
  autocomplete_used: number;
  autocomplete_limit: number;
  autocomplete_left: number;

  // 是否是美元计费模式 (新账号)
  is_dollar_billing: boolean;
}

// 使用事件
export interface UsageEvent {
  session_id: string;
  usage_time: number;
  mode: string;
  model_name: string;
  amount_float: number;
  cost_money_float: number;
  use_max_mode: boolean;
  product_type_list: number[];
  extra_info: {
    cache_read_token: number;
    cache_write_token: number;
    input_token: number;
    output_token: number;
  };
}

// 使用事件响应
export interface UsageEventsResponse {
  total: number;
  user_usage_group_by_sessions: UsageEvent[];
}

// API 错误
export interface ApiError {
  message: string;
}

export interface AppSettings {
  quick_register_show_window: boolean;
  auto_refresh_enabled: boolean;
  privacy_auto_enable: boolean;
  auto_start_enabled: boolean;
  api_key: string; // 用于访问验证码获取服务
}

// 用户统计数据
export interface UserStatisticData {
  UserID: string;
  RegisterDays: number;
  AiCnt365d: Record<string, number>;
  CodeAiAcceptCnt7d: number;
  CodeAiAcceptDiffLanguageCnt7d: Record<string, number>;
  CodeCompCnt7d: number;
  CodeCompDiffAgentCnt7d: Record<string, number>;
  CodeCompDiffModelCnt7d: Record<string, number>;
  IdeActiveDiffHourCnt7d: Record<string, number>;
  DataDate: string;
  IsIde: boolean;
}
