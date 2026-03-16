import { useRef, useState, useEffect } from "react";
import * as api from "../api";
import type { Account } from "../types";

interface AddAccountModalProps {
  isOpen: boolean;
  onClose: () => void;
  onToast?: (type: "success" | "error" | "warning" | "info", message: string) => void;
  onAccountAdded?: (account: Account) => void;
  quickRegisterShowWindow?: boolean;
}

type AddMode = "trae-ide" | "browser" | "register";

// 注册进度步骤
const REGISTER_STEPS = [
  { percent: 5, message: "正在初始化..." },
  { percent: 15, message: "生成临时邮箱..." },
  { percent: 25, message: "打开注册页面..." },
  { percent: 40, message: "填写注册信息..." },
  { percent: 55, message: "等待验证码..." },
  { percent: 70, message: "验证邮箱..." },
  { percent: 85, message: "获取账号 Token..." },
  { percent: 95, message: "保存账号信息..." },
  { percent: 100, message: "注册完成!" },
];

export function AddAccountModal({
  isOpen,
  onClose,
  onToast,
  onAccountAdded,
  quickRegisterShowWindow = true,
}: AddAccountModalProps) {
  const [mode, setMode] = useState<AddMode>("trae-ide");
  const [browserStarted, setBrowserStarted] = useState(false);
  const [browserWaiting, setBrowserWaiting] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const browserRunRef = useRef(0);
  
  // 快速注册进度状态
  const [registerProgress, setRegisterProgress] = useState(0);
  const [registerStatus, setRegisterStatus] = useState("");
  const progressIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // 清理进度定时器
  useEffect(() => {
    return () => {
      if (progressIntervalRef.current) {
        clearInterval(progressIntervalRef.current);
      }
    };
  }, []);

  if (!isOpen) return null;

  // 模拟进度更新
  const startProgressSimulation = () => {
    let currentStep = 0;
    setRegisterProgress(0);
    setRegisterStatus(REGISTER_STEPS[0].message);
    
    progressIntervalRef.current = setInterval(() => {
      currentStep++;
      if (currentStep < REGISTER_STEPS.length) {
        const step = REGISTER_STEPS[currentStep];
        setRegisterProgress(step.percent);
        setRegisterStatus(step.message);
      } else {
        if (progressIntervalRef.current) {
          clearInterval(progressIntervalRef.current);
        }
      }
    }, 3000); // 每3秒更新一次进度
  };

  // 停止进度模拟
  const stopProgressSimulation = () => {
    if (progressIntervalRef.current) {
      clearInterval(progressIntervalRef.current);
      progressIntervalRef.current = null;
    }
    setRegisterProgress(0);
    setRegisterStatus("");
  };

  const handleReadTraeAccount = async () => {
    setLoading(true);
    setError("");

    try {
      const account = await api.readTraeAccount();
      if (account) {
        onToast?.("success", `成功读取 Trae IDE 账号: ${account.email}`);
        onAccountAdded?.(account);
        handleClose();
      } else {
        setError("未找到 Trae IDE 登录账号或账号已存在");
      }
    } catch (err: any) {
      setError(err.message || "读取 Trae IDE 账号失败");
    } finally {
      setLoading(false);
    }
  };

  const handleBrowserStart = async () => {
    setLoading(true);
    setError("");
    const runId = browserRunRef.current + 1;
    browserRunRef.current = runId;

    try {
      await api.startBrowserLogin();
      setBrowserStarted(true);
      setBrowserWaiting(true);
      onToast?.("info", "已打开登录窗口，完成登录后将自动导入。");
      void (async () => {
        try {
          const account = await api.finishBrowserLogin();
          if (browserRunRef.current !== runId) return;
          onToast?.("success", `成功添加账号: ${account.email}`);
          onAccountAdded?.(account);
          handleClose();
        } catch (err: any) {
          if (browserRunRef.current !== runId) return;
          
          if (err.message && err.message.includes("浏览器被主动关闭")) {
            setBrowserStarted(false);
            setBrowserWaiting(false);
            onToast?.("error", "导入失败,浏览器被主动关闭");
            return;
          }

          setError(err.message || "等待浏览器登录失败");
        } finally {
          if (browserRunRef.current === runId) {
            setBrowserWaiting(false);
          }
        }
      })();
    } catch (err: any) {
      if (browserRunRef.current === runId) {
        setError(err.message || "打开浏览器登录窗口失败");
        setBrowserStarted(false);
        setBrowserWaiting(false);
      }
    } finally {
      setLoading(false);
    }
  };

  const handleBrowserCancel = async () => {
    browserRunRef.current += 1;
    try {
      await api.cancelBrowserLogin();
    } catch {} finally {
      setBrowserStarted(false);
      setBrowserWaiting(false);
    }
  };

  const handleQuickRegister = async () => {
    setLoading(true);
    setError("");
    startProgressSimulation();

    try {
      const account = await api.quickRegister(quickRegisterShowWindow);
      // 完成进度
      setRegisterProgress(100);
      setRegisterStatus("注册完成!");
      
      // 先通知父组件添加账号
      onAccountAdded?.(account);
      
      // 延迟关闭弹窗，让用户看到完成状态
      setTimeout(() => {
        handleClose();
        // 显示成功提示
        onToast?.("success", `注册成功，已导入账号: ${account.email}`);
      }, 800);
    } catch (err: any) {
      setError(err.message || "快速注册失败");
      setLoading(false);
      stopProgressSimulation();
    }
  };

  const handleClose = () => {
    browserRunRef.current += 1;
    setError("");
    setBrowserStarted(false);
    setBrowserWaiting(false);
    setMode("trae-ide");
    stopProgressSimulation();
    void api.cancelBrowserLogin();
    onClose();
  };

  return (
    <div className="modal-overlay" onClick={handleClose}>
      <div className="modal-content add-account-modal" onClick={(e) => e.stopPropagation()}>
        <h2>添加账号</h2>

        <div className="add-mode-tabs">
          <button
            className={`mode-tab ${mode === "trae-ide" ? "active" : ""}`}
            onClick={() => setMode("trae-ide")}
            disabled={loading}
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z" />
              <polyline points="3.27 6.96 12 12.01 20.73 6.96" />
              <line x1="12" y1="22.08" x2="12" y2="12" />
            </svg>
            从 Trae 读取
          </button>
          <button
            className={`mode-tab ${mode === "browser" ? "active" : ""}`}
            onClick={() => setMode("browser")}
            disabled={loading}
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <circle cx="12" cy="12" r="10" />
              <line x1="2" y1="12" x2="22" y2="12" />
              <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" />
            </svg>
            浏览器登录
          </button>
          <button
            className={`mode-tab ${mode === "register" ? "active" : ""}`}
            onClick={() => setMode("register")}
            disabled={loading}
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M12 5v14" />
              <path d="M5 12h14" />
            </svg>
            快速注册
          </button>
        </div>

        {mode === "trae-ide" ? (
          <div className="trae-ide-mode">
            <div className="mode-description">
              <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z" />
                <polyline points="3.27 6.96 12 12.01 20.73 6.96" />
                <line x1="12" y1="22.08" x2="12" y2="12" />
              </svg>
              <h3>自动检测本地 Trae IDE 账号</h3>
              <p>系统将自动读取本地 Trae IDE 客户端当前登录的账号信息</p>
            </div>

            {error && <div className="error-message">{error}</div>}

            <div className="modal-actions">
              <button type="button" onClick={handleClose} disabled={loading}>
                取消
              </button>
              <button
                type="button"
                className="primary"
                onClick={handleReadTraeAccount}
                disabled={loading}
              >
                {loading ? "读取中..." : "读取本地账号"}
              </button>
            </div>
          </div>
        ) : mode === "browser" ? (
          <div className="trae-ide-mode">
            <div className="mode-description">
              <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                <circle cx="12" cy="12" r="10" />
                <line x1="2" y1="12" x2="22" y2="12" />
                <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" />
              </svg>
              <h3>使用浏览器登录并自动导入</h3>
              <p>在新窗口完成登录后，系统会自动获取账号信息</p>
            </div>

            {error && <div className="error-message">{error}</div>}

            <div className="modal-actions">
              <button type="button" onClick={handleClose} disabled={loading}>
                取消
              </button>
              {!browserStarted ? (
                <button type="button" className="primary" onClick={handleBrowserStart} disabled={loading}>
                  {loading ? "正在打开..." : "打开登录窗口"}
                </button>
              ) : (
                <>
                  <button type="button" onClick={handleBrowserCancel} disabled={loading}>
                    重新开始
                  </button>
                  <button type="button" className="primary" disabled>
                    {browserWaiting ? "自动导入中..." : "等待登录完成"}
                  </button>
                </>
              )}
            </div>
          </div>
        ) : (
          <div className="trae-ide-mode">
            <div className="mode-description">
              <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5">
                <path d="M12 5v14" />
                <path d="M5 12h14" />
              </svg>
              <h3>快速注册并自动导入</h3>
              <p>系统自动生成邮箱完成注册，并导入到列表</p>
            </div>

            {/* 进度条区域 */}
            {loading && (
              <div className={`register-progress-container ${registerProgress >= 100 ? 'complete' : ''}`}>
                <div className="register-progress-status">
                  {registerProgress >= 100 ? '✓ ' : ''}{registerStatus}
                </div>
                <div className="register-progress-bar">
                  <div 
                    className="register-progress-fill" 
                    style={{ width: `${registerProgress}%` }}
                  />
                </div>
                <div className="register-progress-percent">{registerProgress}%</div>
              </div>
            )}

            {error && <div className="error-message">{error}</div>}

            <div className="modal-actions">
              <button type="button" onClick={handleClose} disabled={loading}>
                取消
              </button>
              <button type="button" className="primary" onClick={handleQuickRegister} disabled={loading}>
                {loading ? "注册中..." : "快速注册并导入"}
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
