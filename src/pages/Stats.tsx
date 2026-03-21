import { useEffect, useState } from "react";
import * as api from "../api";
import type { UsageSummary, UserStatisticData } from "../types";
import { DashboardWidgets } from "../components/DashboardWidgets";

interface StatsProps {
  accounts: Array<{
    id: string;
    name: string;
    email: string;
    usage?: UsageSummary | null;
    is_current?: boolean;
  }>;
  hasLoaded?: boolean;
}

export function Stats({ accounts, hasLoaded = true }: StatsProps) {
  const [userStats, setUserStats] = useState<UserStatisticData | null>(null);
  const [loadingStats, setLoadingStats] = useState(false);
  const [statsError, setStatsError] = useState<string | null>(null);

  const statsCacheKey = (accountId: string) => `trae_user_stats_${accountId}`;
  const loadStatsCache = (accountId: string) => {
    try {
      const raw = localStorage.getItem(statsCacheKey(accountId));
      if (!raw) return null;
      const parsed = JSON.parse(raw);
      // New format
      if (parsed && parsed.data && parsed.cachedAt) {
        return {
          data: parsed.data as UserStatisticData,
          cachedAt: new Date(parsed.cachedAt).getTime()
        };
      }
      // Legacy format (treat as expired)
      if (parsed && parsed.UserID) {
        return {
          data: parsed as UserStatisticData,
          cachedAt: 0 
        };
      }
      // Handle legacy format where data exists but structure might be different
      if (parsed && parsed.data) {
         return {
            data: parsed.data as UserStatisticData,
            cachedAt: 0
         };
      }
    } catch {
      return null;
    }
    return null;
  };
  const saveStatsCache = (accountId: string, data: UserStatisticData) => {
    try {
      localStorage.setItem(statsCacheKey(accountId), JSON.stringify({
        data,
        cachedAt: new Date().toISOString()
      }));
    } catch {
      // ignore cache write errors
    }
  };

  useEffect(() => {
    let cancelled = false;
    
    // 只获取当前登录账号的数据
    const currentAccount = accounts.find(a => a.is_current);
    
    if (!currentAccount) {
      setUserStats(null);
      setLoadingStats(false);
      setStatsError(null);
      return;
    }

    const now = new Date();
    const todayStart = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();

    // Load cache for current account only
    const cache = loadStatsCache(currentAccount.id);

    // Display valid cache immediately (even if stale)
    if (cache?.data) {
      setUserStats(cache.data);
      setLoadingStats(false); // We have data, so stop loading
    } else {
      setLoadingStats(true); // No data at all, show loading
    }
    setStatsError(null);

    // Check if cache is stale (older than today 00:00)
    const isStale = !cache || cache.cachedAt < todayStart;

    if (!isStale) {
      setLoadingStats(false);
      return; // Cache is fresh
    }

    // Fetch current account data
    (async () => {
      try {
        console.log(`[Stats] 正在获取当前账号 ${currentAccount.id} 的统计数据...`);
        const stats = await api.getUserStatistics(currentAccount.id);
        saveStatsCache(currentAccount.id, stats);
        
        if (cancelled) return;

        console.log(`[Stats] 成功获取当前账号统计数据:`, stats);
        setUserStats(stats);
        setStatsError(null);
      } catch (e: any) {
        if (cancelled) return;
        console.error(`[Stats] 获取当前账号统计数据失败:`, e);
        // Only show error if we have no cached data to show
        if (!cache?.data) {
          setStatsError(e.message || "获取统计数据失败");
        }
      } finally {
        if (!cancelled) {
          setLoadingStats(false);
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [accounts.find(a => a.is_current)?.id]); // Only re-run when current account changes

  return (
    <div className="dashboard">
      {/* 空状态 - 没有账号 */}
      {accounts.length === 0 && hasLoaded && (
        <div className="dashboard-empty" style={{
          textAlign: "center",
          padding: "60px 40px",
          background: "var(--bg-card)",
          borderRadius: "var(--radius-lg)",
          border: "1px solid var(--glass-border)",
          backdropFilter: "blur(16px)"
        }}>
          <div className="empty-icon" style={{ fontSize: "48px", marginBottom: "16px" }}>📊</div>
          <h3 style={{ fontSize: "20px", fontWeight: "600", marginBottom: "8px", color: "var(--text-primary)" }}>暂无账号数据</h3>
          <p style={{ color: "var(--text-muted)", marginBottom: "24px" }}>请先在"账号管理"中添加账号</p>
        </div>
      )}

      {/* 加载中 */}
      {loadingStats && (
        <div className="dashboard-widgets-section loading-placeholder" style={{
          marginBottom: "24px",
          textAlign: "center",
          padding: "60px 40px",
          background: "var(--bg-card)",
          borderRadius: "var(--radius-lg)",
          border: "1px solid var(--glass-border)",
          backdropFilter: "blur(16px)"
        }}>
          <div className="spinner" style={{
            margin: "0 auto 20px",
            width: "40px",
            height: "40px",
            border: "3px solid var(--border-light)",
            borderTopColor: "var(--accent)",
            borderRadius: "50%",
            animation: "spin 1s linear infinite"
          }}></div>
          <p style={{ color: "var(--text-muted)", fontSize: "15px" }}>正在加载统计数据...</p>
        </div>
      )}

      {/* 错误状态 */}
      {statsError && !userStats && accounts.length > 0 && (
        <div className="dashboard-widgets-section error-placeholder" style={{
          marginBottom: "24px",
          textAlign: "center",
          padding: "40px",
          background: "var(--danger-bg)",
          borderRadius: "var(--radius-lg)",
          color: "var(--danger)",
          border: "1px solid rgba(245, 101, 101, 0.2)"
        }}>
          <div style={{ fontSize: "40px", marginBottom: "12px" }}>⚠️</div>
          <p style={{ fontSize: "16px", marginBottom: "16px" }}>{statsError}</p>
          <button
            onClick={() => {
              window.location.reload();
            }}
            style={{
              padding: "10px 24px",
              background: "var(--danger)",
              border: "none",
              borderRadius: "var(--radius)",
              cursor: "pointer",
              color: "white",
              fontWeight: "500",
              transition: "all 0.2s"
            }}
          >
            重试
          </button>
        </div>
      )}

      {/* 数据显示 */}
      {userStats && (
        <div className="dashboard-widgets-section" style={{ marginBottom: "24px" }}>
          <DashboardWidgets data={userStats} />
        </div>
      )}
    </div>
  );
}
