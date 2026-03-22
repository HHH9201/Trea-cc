import { useState } from "react";
import type { UsageSummary } from "../types";

interface AccountCardProps {
  account: {
    id: string;
    name: string;
    email: string;
    avatar_url: string;
    plan_type: string;
    created_at: number;
    is_current?: boolean;
  };
  usage: UsageSummary | null;
  selected: boolean;
  onSelect: (id: string) => void;
  onContextMenu: (e: React.MouseEvent, id: string) => void;
  onToast?: (type: "success" | "error" | "warning" | "info", message: string, duration?: number) => void;
}

export function AccountCard({ account, usage, selected, onSelect, onContextMenu, onToast }: AccountCardProps) {
  const [copied, setCopied] = useState(false);
  const getUsageLevel = (used: number, limit: number) => {
    if (limit === 0) return "low";
    const percent = (used / limit) * 100;
    if (percent >= 80) return "high";
    if (percent >= 50) return "medium";
    return "low";
  };

  const formatDate = (timestamp: number) => {
    if (!timestamp) return "-";
    const date = new Date(timestamp * 1000);
    const year = date.getFullYear();
    const month = date.getMonth() + 1;
    const day = date.getDate();
    return `${year}/${month}/${day}`;
  };

  const formatCreatedDate = (timestamp: number) => {
    if (!timestamp) return "-";
    const date = new Date(timestamp * 1000);
    const year = date.getFullYear();
    const month = date.getMonth() + 1;
    const day = date.getDate();
    return `${year}/${month}/${day}`;
  };

  const hasUsage = !!usage;
  // 根据是否是美元计费模式显示不同的额度
  const isDollarBilling = usage?.is_dollar_billing ?? false;
  const totalUsed = isDollarBilling
    ? (usage?.fast_dollar_used ?? 0)
    : (usage ? usage.fast_request_used + usage.extra_fast_request_used : 0);
  const totalLimit = isDollarBilling
    ? (usage?.fast_dollar_limit ?? 3)
    : (usage ? usage.fast_request_limit + usage.extra_fast_request_limit : 0);
  const totalLeft = isDollarBilling
    ? (usage?.fast_dollar_left ?? 3)
    : (usage ? usage.fast_request_left + usage.extra_fast_request_left : 0);
  const usagePercent = totalLimit > 0 ? Math.round((totalUsed / totalLimit) * 100) : 0;
  const usageLevel = getUsageLevel(totalUsed, totalLimit);

  const isTokenExpired = false; // TODO: 根据实际 token 过期时间判断

  const handleCopy = (e: React.MouseEvent) => {
    e.stopPropagation();
    const textToCopy = account.name || account.email;
    navigator.clipboard.writeText(textToCopy).then(() => {
      onToast?.("success", `已复制用户名: ${textToCopy}`, 2000);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  };

  return (
    <div
      className={`account-card ${selected ? "selected" : ""} ${account.is_current ? "current" : ""}`}
      onClick={(e) => {
        if (e.button !== 0) return;
        onSelect(account.id);
      }}
      onContextMenu={(e) => {
        e.preventDefault();
        e.stopPropagation();
        onContextMenu(e, account.id);
      }}
    >
      <div className="card-header">
        <div className="card-checkbox" onClick={(e) => e.stopPropagation()}>
          <input
            type="checkbox"
            checked={selected}
            onChange={() => onSelect(account.id)}
          />
        </div>

        <div className="card-avatar">
          {account.avatar_url ? (
            <img src={account.avatar_url} alt={account.name} />
          ) : (
            <div className="avatar-placeholder">
              {(account.email || account.name).charAt(0).toUpperCase()}
            </div>
          )}
        </div>

        <div className="card-info">
          <div className="card-email">
            <span className="email-text">{account.name || account.email}</span>
            <button
              className="copy-btn"
              onClick={handleCopy}
              title={copied ? "复制成功" : "复制用户名"}
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <rect x="9" y="9" width="13" height="13" rx="2" ry="2"/>
                <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
              </svg>
            </button>
          </div>
          <div className="card-badges">
            {(usage?.plan_type || account.plan_type) !== "Free" && (
              <span className="badge pro">PRO</span>
            )}
            {account.is_current && (
              <span className="badge current">
                <svg width="10" height="10" viewBox="0 0 24 24" fill="currentColor">
                  <path d="M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z"/>
                </svg>
                当前使用
              </span>
            )}
          </div>
        </div>

        <div className={`card-status ${isTokenExpired ? "expired" : "normal"}`}>
          <span className="status-indicator"></span>
          {isTokenExpired ? "已过期" : "正常"}
        </div>
      </div>

      {isDollarBilling && usage ? (
        // 美元计费模式 - 紧凑显示
        <div className="card-usage-dollar">
          {/* 总额度 - 一行显示 */}
          <div className="usage-compact-header">
            <span className="usage-compact-label">额度</span>
            <span className="usage-compact-percent">{usagePercent}%</span>
          </div>

          {/* 进度条 */}
          <div className="usage-bar">
            <div
              className={`usage-bar-fill ${usageLevel}`}
              style={{ width: `${Math.min(usagePercent, 100)}%` }}
            />
          </div>

          {/* 总额度数值 - 显示格式: $0.00 / 6.0 或 $0.00 / 3.0+3.0 */}
          <div className="usage-compact-total">
            <span className="usage-compact-used">${totalUsed.toFixed(2)}</span>
            <span className="usage-compact-limit">
              {" / "}
              {usage.bonus_dollar_limit > 0 ? (
                <>
                  <span className="limit-basic">{usage.basic_dollar_limit.toFixed(1)}</span>
                  <span className="limit-plus">+</span>
                  <span className="limit-bonus">{usage.bonus_dollar_limit.toFixed(1)}</span>
                </>
              ) : (
                <span className="limit-basic">{usage.fast_dollar_limit.toFixed(1)}</span>
              )}
            </span>
            <span className={`usage-compact-left ${totalLeft < 0 ? 'negative' : ''}`}>
              {totalLeft < 0 ? '超支' : '剩'} ${Math.abs(totalLeft).toFixed(2)}
            </span>
          </div>

          {/* 只在有 Bonus 额度时显示赠送详情 */}
          {usage.bonus_dollar_limit > 0 && (
            <div className="usage-compact-details">
              <div className="usage-compact-item bonus-only">
                <div className="compact-item-header">
                  <span className="compact-icon">🎁</span>
                  <span className="compact-name">赠送</span>
                  <span className="compact-value">
                    ${usage.bonus_dollar_used.toFixed(2)} / ${usage.bonus_dollar_limit.toFixed(2)}
                  </span>
                </div>
                <div className="compact-bar">
                  <div
                    className="compact-bar-fill bonus"
                    style={{
                      width: `${usage.bonus_dollar_limit > 0 ? Math.min((usage.bonus_dollar_used / usage.bonus_dollar_limit) * 100, 100) : 0}%`
                    }}
                  />
                </div>
              </div>
            </div>
          )}
        </div>
      ) : (
        // 普通模式 - 显示 Fast Requests
        <div className="card-usage">
          <div className="usage-header">
            <span className="usage-label">Fast Requests</span>
            <span className={`usage-percent ${usageLevel}`}>{usagePercent}%</span>
          </div>
          <div className="usage-bar">
            <div
              className={`usage-bar-fill ${usageLevel}`}
              style={{ width: `${Math.min(usagePercent, 100)}%` }}
            />
          </div>
          <div className="usage-numbers">
            <span className="usage-used">
              <strong>{hasUsage ? Math.round(totalUsed) : "-"}</strong>
              <span style={{ opacity: 0.6, marginLeft: '4px' }}>/ {hasUsage ? totalLimit : "-"}</span>
            </span>
            <span className="usage-left">剩余 {hasUsage ? Math.round(totalLeft) : "-"}</span>
          </div>
        </div>
      )}

      <div className="card-meta">
        <span className="meta-item">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <rect x="3" y="4" width="18" height="18" rx="2" ry="2"/>
            <line x1="16" y1="2" x2="16" y2="6"/>
            <line x1="8" y1="2" x2="8" y2="6"/>
            <line x1="3" y1="10" x2="21" y2="10"/>
          </svg>
          添加于 {formatCreatedDate(account.created_at)}
        </span>
        <span className="meta-item">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M23 4v6h-6M1 20v-6h6M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/>
          </svg>
          重置 {usage ? formatDate(usage.reset_time) : "-"}
        </span>
        {usage && usage.extra_expire_time > 0 && (
          <span className="meta-item warning">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M20 12v6a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2v-6M12 2v10M8 6l4-4 4 4"/>
            </svg>
            礼包到期 {formatDate(usage.extra_expire_time)}
          </span>
        )}
      </div>

      <div className="card-footer">
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
          <circle cx="12" cy="12" r="1"/><circle cx="19" cy="12" r="1"/><circle cx="5" cy="12" r="1"/>
        </svg>
        右键查看更多操作
      </div>
    </div>
  );
}
