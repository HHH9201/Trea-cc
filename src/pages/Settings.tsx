import { useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import * as api from "../api";
import type { AppSettings } from "../types";

interface SettingsProps {
  onToast?: (type: "success" | "error" | "warning" | "info", message: string, duration?: number) => void;
  settings?: AppSettings | null;
  onSettingsChange?: (settings: AppSettings) => void;
}

export function Settings({
  onToast,
  settings,
  onSettingsChange,
}: SettingsProps) {
  const [traeMachineId, setTraeMachineId] = useState<string>("");
  const [traeRefreshing, setTraeRefreshing] = useState(false);
  const [clearingTrae, setClearingTrae] = useState(false);
  const [traePath, setTraePath] = useState<string>("");
  const [traePathLoading, setTraePathLoading] = useState(false);
  const [scanning, setScanning] = useState(false);
  const defaultSettings = useMemo<AppSettings>(
    () => ({
      quick_register_show_window: false,
      auto_refresh_enabled: true,
      privacy_auto_enable: true,
      auto_start_enabled: false,
      api_key: "",
    }),
    []
  );
  const [appSettings, setAppSettings] = useState<AppSettings | null>(settings ?? null);

  // 加载 Trae IDE 机器码
  const loadTraeMachineId = async () => {
    setTraeRefreshing(true);
    try {
      const id = await api.getTraeMachineId();
      setTraeMachineId(id);
    } catch (err: any) {
      console.error("获取 Trae IDE 机器码失败:", err);
      setTraeMachineId("未找到");
    } finally {
      setTraeRefreshing(false);
    }
  };

  // 加载 Trae IDE 路径
  const loadTraePath = async () => {
    setTraePathLoading(true);
    try {
      const path = await api.getTraePath();
      setTraePath(path);
    } catch (err: any) {
      console.error("获取 Trae IDE 路径失败:", err);
      setTraePath("");
    } finally {
      setTraePathLoading(false);
    }
  };

  useEffect(() => {
    loadTraeMachineId();
    loadTraePath();
  }, []);

  useEffect(() => {
    if (settings) {
      setAppSettings(settings);
    }
  }, [settings]);

  useEffect(() => {
    if (appSettings) return;
    api.getSettings()
      .then((value) => setAppSettings(value))
      .catch(() => setAppSettings(defaultSettings));
  }, [appSettings, defaultSettings]);

  // 复制 Trae IDE 机器码
  const handleCopyTraeMachineId = async () => {
    try {
      await navigator.clipboard.writeText(traeMachineId);
      onToast?.("success", "Trae IDE 机器码已复制到剪贴板");
    } catch {
      onToast?.("error", "复制失败");
    }
  };

  // 清除 Trae IDE 登录状态
  const handleClearTraeLoginState = async () => {
    if (!confirm("确定要清除 Trae IDE 登录状态吗？\n\n这将：\n• 重置 Trae IDE 机器码\n• 清除所有登录信息\n• 删除本地缓存数据\n\n操作后 Trae IDE 将变成全新安装状态，需要重新登录。\n\n请确保 Trae IDE 已关闭！")) {
      return;
    }

    setClearingTrae(true);
    try {
      await api.clearTraeLoginState();
      await loadTraeMachineId(); // 重新加载新的机器码
      onToast?.("success", "Trae IDE 登录状态已清除，请重新打开 Trae IDE 登录");
    } catch (err: any) {
      onToast?.("error", err.message || "清除失败");
    } finally {
      setClearingTrae(false);
    }
  };

  // 自动扫描 Trae IDE 路径
  const handleScanTraePath = async () => {
    setScanning(true);
    try {
      const path = await api.scanTraePath();
      setTraePath(path);
      onToast?.("success", "已找到 Trae IDE: " + path);
    } catch (err: any) {
      onToast?.("error", err.message || "未找到 Trae IDE，请手动设置路径");
    } finally {
      setScanning(false);
    }
  };

  // 手动设置 Trae IDE 路径
  const handleSetTraePath = async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [{
          name: "Trae IDE",
          extensions: ["exe"]
        }],
        title: "选择 Trae.exe 文件"
      });

      if (selected) {
        const path = selected as string;
        await api.setTraePath(path);
        setTraePath(path);
        onToast?.("success", "Trae IDE 路径已保存");
      }
    } catch (err: any) {
      onToast?.("error", err.message || "选择文件失败");
    }
  };

  const updateSettings = async (updates: Partial<AppSettings>, successMessage: string) => {
    const base = appSettings ?? defaultSettings;
    const next = { ...base, ...updates };
    try {
      const saved = await api.updateSettings(next);
      setAppSettings(saved);
      onSettingsChange?.(saved);
      onToast?.("success", successMessage, 1000);
    } catch (err: any) {
      onToast?.("error", err.message || "更新设置失败");
    }
  };

  const currentSettings = appSettings ?? defaultSettings;
  const settingsDisabled = !appSettings;
  const handlePrivacyHelp = () => {
    const message =
      "启用隐私模式后，TRAE不会存储或使用您的任何聊天交互内容（包括相关代码片段）用于分析、产品改进或模型训练。";
    if (onToast) {
      onToast("info", message, 4000);
    } else {
      alert(message);
    }
  };

  return (
    <div className="settings-page">
      {/* Trae IDE 设置 */}
      <div className="settings-section">
        <h3>Trae IDE 配置</h3>
        
        {/* Machine ID */}
        <div className="setting-item" style={{ alignItems: 'flex-start' }}>
          <div className="setting-info" style={{ flex: 1, overflow: 'hidden' }}>
            <div className="setting-label">
              Machine ID
              <span style={{ 
                fontSize: '11px', 
                padding: '2px 6px', 
                background: 'var(--bg-hover)', 
                borderRadius: '4px', 
                marginLeft: '8px', 
                color: 'var(--text-muted)',
                fontWeight: 'normal'
              }}>客户端唯一标识</span>
            </div>
             <div style={{ position: 'relative', marginTop: '8px' }}>
              <div style={{ 
                padding: '10px 12px', 
                paddingRight: '80px',
                background: 'var(--bg-secondary)', 
                borderRadius: '8px', 
                fontFamily: 'Consolas, Monaco, "Courier New", monospace',
                fontSize: '14px',
                fontWeight: '500',
                border: '1px solid var(--border)',
                width: '100%',
                wordBreak: 'break-all',
                color: 'var(--text-primary)',
                minHeight: '42px',
                display: 'flex',
                alignItems: 'center'
              }}>
                {traeRefreshing ? "加载中..." : traeMachineId}
              </div>
              <div style={{ position: 'absolute', right: '4px', top: '4px', display: 'flex', gap: '4px' }}>
                 <button
                  onClick={loadTraeMachineId}
                  disabled={traeRefreshing}
                  title="刷新"
                  style={{
                    padding: '6px',
                    border: 'none',
                    background: 'transparent',
                    color: 'var(--text-secondary)',
                    cursor: 'pointer',
                    borderRadius: '6px',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    transition: 'all 0.2s'
                  }}
                  onMouseOver={(e) => e.currentTarget.style.backgroundColor = 'var(--bg-hover)'}
                  onMouseOut={(e) => e.currentTarget.style.backgroundColor = 'transparent'}
                >
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M23 4v6h-6M1 20v-6h6M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/></svg>
                </button>
                <button
                  onClick={handleCopyTraeMachineId}
                  disabled={!traeMachineId || traeRefreshing || traeMachineId === "未找到"}
                  title="复制"
                  style={{
                    padding: '6px',
                    border: 'none',
                    background: 'transparent',
                    color: 'var(--text-secondary)',
                    cursor: 'pointer',
                    borderRadius: '6px',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    transition: 'all 0.2s'
                  }}
                  onMouseOver={(e) => e.currentTarget.style.backgroundColor = 'var(--bg-hover)'}
                  onMouseOut={(e) => e.currentTarget.style.backgroundColor = 'transparent'}
                >
                  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>
                </button>
              </div>
            </div>
            <div className="setting-desc" style={{ marginTop: '8px', color: 'var(--warning)', display: 'flex', alignItems: 'center', gap: '4px' }}>
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12" y2="16"/></svg>
              <span>清除登录状态会重置机器码，需重新登录 Trae IDE</span>
            </div>
          </div>
          <div className="setting-action" style={{ display: 'flex', alignItems: 'flex-start', paddingTop: '32px', marginLeft: '16px' }}>
            <button
              className="setting-btn danger"
              onClick={handleClearTraeLoginState}
              disabled={clearingTrae || traeRefreshing}
              style={{ whiteSpace: 'nowrap' }}
            >
              {clearingTrae ? "清除中..." : "清除登录状态"}
            </button>
          </div>
        </div>

        {/* Trae IDE 路径 */}
        <div className="setting-item" style={{ alignItems: 'flex-start' }}>
          <div className="setting-info" style={{ flex: 1 }}>
            <div className="setting-label">安装路径</div>
             <div style={{ position: 'relative', marginTop: '8px' }}>
               <div style={{ 
                padding: '10px 12px', 
                paddingRight: '40px', 
                background: 'var(--bg-secondary)', 
                borderRadius: '8px', 
                fontFamily: 'Consolas, Monaco, "Courier New", monospace',
                fontSize: '14px',
                fontWeight: '500',
                border: '1px solid var(--border)',
                color: 'var(--text-primary)',
                wordBreak: 'break-all',
                minHeight: '42px',
                display: 'flex',
                alignItems: 'center'
              }}>
                {traePathLoading ? "加载中..." : (traePath || "未设置")}
              </div>
               <div style={{ position: 'absolute', right: '4px', top: '4px', display: 'flex', gap: '4px' }}>
                <button
                  onClick={handleSetTraePath}
                  title="手动设置"
                  style={{
                    padding: '6px',
                    border: 'none',
                    background: 'transparent',
                    color: 'var(--text-secondary)',
                    cursor: 'pointer',
                    borderRadius: '6px',
                    display: 'flex',
                    alignItems: 'center',
                    justifyContent: 'center',
                    transition: 'all 0.2s'
                  }}
                  onMouseOver={(e) => e.currentTarget.style.backgroundColor = 'var(--bg-hover)'}
                  onMouseOut={(e) => e.currentTarget.style.backgroundColor = 'transparent'}
                >
                   <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>
                </button>
              </div>
            </div>
            <div className="setting-desc" style={{ marginTop: '4px' }}>
              切换账号后会自动打开 Trae IDE
            </div>
          </div>
          <div className="setting-action" style={{ display: 'flex', alignItems: 'flex-start', paddingTop: '32px', marginLeft: '16px' }}>
             <button
               className="setting-btn"
               onClick={handleScanTraePath}
               disabled={scanning}
               style={{ whiteSpace: 'nowrap' }}
             >
               {scanning ? "扫描中..." : "自动扫描"}
             </button>
          </div>
        </div>

        {/* 自动开启隐私模式 */}
        <div className="setting-item">
          <div className="setting-info">
            <div className="setting-label">
              自动开启隐私模式
              <button type="button" className="setting-help" onClick={handlePrivacyHelp} style={{ marginLeft: '6px' }}>
                ?
              </button>
            </div>
            <div className="setting-desc">切换账号后自动开启 Trae 隐私模式（需重启生效）</div>
          </div>
          <div className="setting-action">
            <button
              type="button"
              className={`pill-toggle ${currentSettings.privacy_auto_enable ? "on" : ""}`}
              onClick={() =>
                updateSettings(
                  { privacy_auto_enable: !currentSettings.privacy_auto_enable },
                  "已更新隐私模式设置"
                )
              }
              disabled={settingsDisabled}
              role="switch"
              aria-checked={currentSettings.privacy_auto_enable}
            >
              <span className="pill-track"></span>
              <span className="pill-thumb"></span>
            </button>
          </div>
        </div>
      </div>

      <div className="settings-section">
        <h3>通用设置</h3>
        <div className="setting-item">
          <div className="setting-info">
            <div className="setting-label">快速注册显示浏览器窗口</div>
            <div className="setting-desc">关闭后在后台完成注册并通过通知提示进度</div>
          </div>
          <div className="setting-action">
            <button
              type="button"
              className={`pill-toggle ${currentSettings.quick_register_show_window ? "on" : ""}`}
              onClick={() =>
                updateSettings(
                  { quick_register_show_window: !currentSettings.quick_register_show_window },
                  "已更新快速注册显示设置"
                )
              }
              disabled={settingsDisabled}
              role="switch"
              aria-checked={currentSettings.quick_register_show_window}
            >
              <span className="pill-track"></span>
              <span className="pill-thumb"></span>
            </button>
          </div>
        </div>

        <div className="setting-item">
          <div className="setting-info">
            <div className="setting-label">自动刷新</div>
            <div className="setting-desc">定时自动刷新账号使用量数据</div>
          </div>
          <div className="setting-action">
            <button
              type="button"
              className={`pill-toggle ${currentSettings.auto_refresh_enabled ? "on" : ""}`}
              onClick={() =>
                updateSettings(
                  { auto_refresh_enabled: !currentSettings.auto_refresh_enabled },
                  "已更新自动刷新设置"
                )
              }
              disabled={settingsDisabled}
              role="switch"
              aria-checked={currentSettings.auto_refresh_enabled}
            >
              <span className="pill-track"></span>
              <span className="pill-thumb"></span>
            </button>
          </div>
        </div>





        <div className="setting-item">
          <div className="setting-info">
            <div className="setting-label">
              开机静默自动刷新 Token
              <span style={{ 
                fontSize: '11px', 
                padding: '2px 6px', 
                background: 'var(--success-bg)', 
                color: 'var(--success)', 
                borderRadius: '4px', 
                marginLeft: '8px',
                fontWeight: 'normal',
                border: '1px solid var(--success-border)'
              }}>强烈推荐</span>
            </div>
            <div className="setting-desc">
              开机时在后台静默启动，自动刷新所有账号 Token 并同步到 Trae IDE，确保打开 IDE 时 Token 始终有效且无需手动刷新。
            </div>
          </div>
          <div className="setting-action">
            <button
              type="button"
              className={`pill-toggle ${currentSettings.auto_start_enabled ? "on" : ""}`}
              onClick={() =>
                updateSettings(
                  { auto_start_enabled: !currentSettings.auto_start_enabled },
                  "已更新开机静默刷新设置"
                )
              }
              disabled={settingsDisabled}
              role="switch"
              aria-checked={currentSettings.auto_start_enabled}
            >
              <span className="pill-track"></span>
              <span className="pill-thumb"></span>
            </button>
          </div>
        </div>

        <div className="setting-item">
          <div className="setting-info">
            <div className="setting-label">刷新间隔</div>
            <div className="setting-desc">自动刷新的时间间隔（分钟）</div>
          </div>
          <div className="setting-action">
            <select className="setting-select" disabled={settingsDisabled}>
              <option value="5">5 分钟</option>
              <option value="10">10 分钟</option>
              <option value="30">30 分钟</option>
              <option value="60">60 分钟</option>
            </select>
          </div>
        </div>

        {/* API 密钥设置 */}
        <div className="setting-item" style={{ alignItems: 'flex-start' }}>
          <div className="setting-info" style={{ flex: 1 }}>
            <div className="setting-label">
              API 密钥
              <span style={{ 
                fontSize: '11px', 
                padding: '2px 6px', 
                background: 'var(--bg-hover)', 
                borderRadius: '4px', 
                marginLeft: '8px', 
                color: 'var(--text-muted)',
                fontWeight: 'normal'
              }}>可选</span>
            </div>
            <div className="setting-desc">用于访问验证码获取服务，留空则使用默认服务</div>
            <div style={{ marginTop: '8px' }}>
              <input
                type="password"
                value={currentSettings.api_key || ''}
                onChange={(e) => updateSettings({ api_key: e.target.value }, 'API 密钥已更新')}
                placeholder="请输入 API 密钥"
                disabled={settingsDisabled}
                style={{
                  width: '100%',
                  padding: '10px 12px',
                  background: 'var(--bg-secondary)',
                  border: '1px solid var(--border)',
                  borderRadius: '8px',
                  fontSize: '14px',
                  color: 'var(--text-primary)',
                  outline: 'none',
                }}
              />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
