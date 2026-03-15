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
  const aggregateStats = (statsList: UserStatisticData[]): UserStatisticData => {
    const merged: UserStatisticData = {
      UserID: "ALL",
      RegisterDays: 0,
      AiCnt365d: {},
      CodeAiAcceptCnt7d: 0,
      CodeAiAcceptDiffLanguageCnt7d: {},
      CodeCompCnt7d: 0,
      CodeCompDiffAgentCnt7d: {},
      CodeCompDiffModelCnt7d: {},
      IdeActiveDiffHourCnt7d: {},
      DataDate: "",
      IsIde: false
    };
    for (const stats of statsList) {
      if (!stats) continue;
      merged.RegisterDays = Math.max(merged.RegisterDays, stats.RegisterDays || 0);
      merged.CodeAiAcceptCnt7d += stats.CodeAiAcceptCnt7d || 0;
      merged.CodeCompCnt7d += stats.CodeCompCnt7d || 0;
      merged.IsIde = merged.IsIde || !!stats.IsIde;
      if (stats.DataDate && stats.DataDate > merged.DataDate) {
        merged.DataDate = stats.DataDate;
      }
      const mergeMap = (target: Record<string, number>, source?: Record<string, number>) => {
        if (!source) return;
        Object.entries(source).forEach(([key, value]) => {
          target[key] = (target[key] || 0) + (value || 0);
        });
      };
      mergeMap(merged.AiCnt365d, stats.AiCnt365d);
      mergeMap(merged.CodeAiAcceptDiffLanguageCnt7d, stats.CodeAiAcceptDiffLanguageCnt7d);
      mergeMap(merged.CodeCompDiffAgentCnt7d, stats.CodeCompDiffAgentCnt7d);
      mergeMap(merged.CodeCompDiffModelCnt7d, stats.CodeCompDiffModelCnt7d);
      mergeMap(merged.IdeActiveDiffHourCnt7d, stats.IdeActiveDiffHourCnt7d);
    }
    return merged;
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
    if (!accounts.length) {
      setUserStats(null);
      setLoadingStats(false);
      setStatsError(null);
      return;
    }

    const now = new Date();
    const todayStart = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();

    // Load all caches
    const cachedResults = accounts.map(account => {
      const cache = loadStatsCache(account.id);
      return { id: account.id, cache };
    });

    // Display valid cache immediately (even if stale)
    const validCacheData = cachedResults
      .map(r => r.cache?.data)
      .filter(Boolean) as UserStatisticData[];

    if (validCacheData.length > 0) {
      setUserStats(aggregateStats(validCacheData));
      setLoadingStats(false); // We have data, so stop loading
    } else {
      setLoadingStats(true); // No data at all, show loading
    }
    setStatsError(null);

    // Identify stale accounts (no cache or cache older than today 00:00)
    const accountsToFetch = accounts.filter(account => {
      const result = cachedResults.find(r => r.id === account.id);
      if (!result?.cache) return true; // No cache
      return result.cache.cachedAt < todayStart; // Stale cache
    });

    if (accountsToFetch.length === 0) {
      setLoadingStats(false);
      return; // All fresh
    }

    // Fetch stale accounts in background
    (async () => {
      try {
        const results = await Promise.allSettled(
          accountsToFetch.map(async (account) => {
            const stats = await api.getUserStatistics(account.id);
            saveStatsCache(account.id, stats);
            return { id: account.id, stats };
          })
        );
        
        if (cancelled) return;

        const freshStatsMap = new Map<string, UserStatisticData>();
        results.forEach(res => {
          if (res.status === "fulfilled") {
            freshStatsMap.set(res.value.id, res.value.stats);
          }
        });

        // Merge fresh data with existing fresh cache
        const finalStatsList = accounts.map(account => {
          if (freshStatsMap.has(account.id)) {
            return freshStatsMap.get(account.id)!;
          }
          const cache = cachedResults.find(r => r.id === account.id)?.cache?.data;
          return cache;
        }).filter(Boolean) as UserStatisticData[];

        if (finalStatsList.length > 0) {
          setUserStats(aggregateStats(finalStatsList));
          setStatsError(null);
        } else {
          // Only show error if we still have no data
          if (!userStats) {
             setStatsError("获取统计数据失败");
          }
        }
      } catch (e: any) {
        if (cancelled) return;
        // Only show error if we have no cached data to show
        if (!validCacheData.length) {
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
  }, [accounts.map(a => a.id).join("|")]);

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
