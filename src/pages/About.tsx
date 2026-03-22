import { useState } from "react";
import * as api from "../api";

interface AboutProps {
  onToast?: (type: "success" | "error" | "warning" | "info", message: string, duration?: number) => void;
}

export function About({ onToast }: AboutProps) {
  const [checkingUpdate, setCheckingUpdate] = useState(false);
  const [updateInfo, setUpdateInfo] = useState<{ version: string; current_version: string; body: string; date: string } | null>(null);
  const [showQrModal, setShowQrModal] = useState<string | null>(null);

  // 检查更新
  const handleCheckUpdate = async () => {
    setCheckingUpdate(true);
    try {
      const info = await api.checkUpdate();
      if (info) {
        setUpdateInfo(info);
        onToast?.("info", `发现新版本: ${info.version}`);
      } else {
        onToast?.("success", "当前已是最新版本");
      }
    } catch (err: any) {
      onToast?.("error", err.message || "检查更新失败");
    } finally {
      setCheckingUpdate(false);
    }
  };

  // 安装更新
  const handleInstallUpdate = async () => {
    try {
      onToast?.("info", "正在下载并安装更新...");
      await api.installUpdate();
    } catch (err: any) {
      onToast?.("error", err.message || "安装更新失败");
    }
  };

  // 处理赞助按钮点击
  const handleSponsorClick = (type: string) => {
    setShowQrModal(type);
  };

  // 关闭二维码弹窗
  const closeQrModal = () => {
    setShowQrModal(null);
  };

  // 复制 QQ 群号
  const handleCopyQQ = () => {
    navigator.clipboard.writeText("894356872").then(() => {
      onToast?.("success", "已复制 QQ 群号: 894356872", 2000);
    });
  };

  // 获取二维码图片路径
  const getQrImage = (type: string) => {
    switch (type) {
      case "wechat":
        return "./weixin.jpg";
      case "alipay":
        return "./zfb.jpg";
      case "qq":
        return "./qq.jpg";
      default:
        return "";
    }
  };

  // 获取二维码标题
  const getQrTitle = (type: string) => {
    switch (type) {
      case "wechat":
        return "微信支付";
      case "alipay":
        return "支付宝";
      case "qq":
        return "QQ 红包";
      default:
        return "";
    }
  };

  return (
    <div className="about-page">
      <div className="about-card">
        {/* 头部横排 */}
        <div className="about-header">
          <img src="./logo.png" alt="Trae账号管理" className="about-logo" />
          <div className="about-header-text">
            <div className="title-row">
              <h1 className="about-title">Trae账号管理</h1>
              <span className="version">v1.0.3</span>
            </div>
          </div>
        </div>
        
        {/* 说明和信息横向排列 */}
        <div className="about-intro-section">
          <p className="about-desc">
            这是一款专为 Trae IDE 用户打造的多账号高效管理工具。通过本工具，您可以轻松管理多个 Trae 账号，一键切换账号，实时查看使用量，让您的 Trae IDE 使用体验更加便捷！基于
            <a
              href="https://github.com/S-Trespassing/Trae账号管理"
              target="_blank"
              rel="noopener noreferrer"
              className="original-link"
            >
              原作者项目
            </a>
            进行二次开发，原作者已不再维护。
          </p>
          
          <div className="about-info">
            <a 
              href="https://github.com/HHH9201/Trae-CC.git" 
              target="_blank" 
              rel="noopener noreferrer"
              className="github-link"
            >
              <span className="label">GitHub</span>
              <span className="value">HHH9201/Trae-CC</span>
            </a>
          </div>
        </div>

        {/* 分割线 */}
        <div className="about-divider"></div>

        {/* 软件更新 */}
        <div className="about-update-section">
          <div className="about-update-header">
            <div className="about-update-icon">🔄</div>
            <div className="about-update-title">软件更新</div>
          </div>
          <div className="about-update-content">
            {updateInfo ? (
              <div className="about-update-info">
                <div className="about-update-version">
                  <span className="new-version">{updateInfo.version}</span>
                  <span className="current-version">当前: {updateInfo.current_version}</span>
                </div>
                {updateInfo.body && (
                  <div className="about-update-body">
                    {updateInfo.body}
                  </div>
                )}
              </div>
            ) : (
              <div className="about-update-desc">检查并安装最新版本</div>
            )}
          </div>
          <div className="about-update-actions">
            {!updateInfo ? (
              <button
                onClick={handleCheckUpdate}
                disabled={checkingUpdate}
                className="about-update-btn"
              >
                {checkingUpdate ? (
                  <>
                    <span className="btn-spinner"></span>
                    检查中...
                  </>
                ) : (
                  "检查更新"
                )}
              </button>
            ) : (
              <>
                <button
                  onClick={handleCheckUpdate}
                  disabled={checkingUpdate}
                  className="about-update-btn secondary"
                >
                  重新检查
                </button>
                <button
                  onClick={handleInstallUpdate}
                  className="about-update-btn primary"
                >
                  立即更新
                </button>
              </>
            )}
          </div>
        </div>

        {/* 售后支持 */}
        <div className="about-support-section">
          <div className="about-support-header">
            <div className="about-support-icon">💬</div>
            <div className="about-support-title">售后支持</div>
          </div>
          <div className="about-support-content">
            <p>遇到问题？加入 QQ 群 894356872 获取帮助</p>
          </div>
          <div className="about-support-actions">
            <button
              onClick={() => setShowQrModal("qqgroup")}
              className="about-support-btn copy"
              title="查看群二维码"
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <rect x="3" y="3" width="7" height="7"/>
                <rect x="14" y="3" width="7" height="7"/>
                <rect x="14" y="14" width="7" height="7"/>
                <rect x="3" y="14" width="7" height="7"/>
              </svg>
              复制群号
            </button>
            <a
              href="http://qm.qq.com/cgi-bin/qm/qr?_wv=1027&k=894356872"
              target="_blank"
              rel="noopener noreferrer"
              className="about-support-btn qq"
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
                <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-1 15h-2v-2h2v2zm0-4h-2V7h2v6zm4 4h-2v-2h2v2zm0-4h-2V7h2v6z"/>
              </svg>
              加入 QQ 群
            </a>
          </div>
        </div>

        {/* 赞助支持 */}
        <div className="about-sponsor-section">
          <div className="about-sponsor-header">
            <div className="about-sponsor-icon">☕</div>
            <div className="about-sponsor-title">赞助支持</div>
          </div>
          <div className="about-sponsor-content">
            <p>如果这个项目对你有帮助，可以考虑赞助支持开发者</p>
          </div>
          <div className="about-sponsor-actions">
            <button 
              className="about-sponsor-btn wechat"
              onClick={() => handleSponsorClick("wechat")}
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
                <path d="M8.5 13.5A1.5 1.5 0 1 0 7 12a1.5 1.5 0 0 0 1.5 1.5zm6.5 0a1.5 1.5 0 1 0-1.5-1.5 1.5 1.5 0 0 0 1.5 1.5z"/>
                <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 18c-4.41 0-8-3.59-8-8 0-2.21.9-4.21 2.35-5.65L12 12V4c4.41 0 8 3.59 8 8s-3.59 8-8 8z"/>
              </svg>
              微信支付
            </button>
            <button 
              className="about-sponsor-btn alipay"
              onClick={() => handleSponsorClick("alipay")}
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
                <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-1 15h-2v-2h2v2zm0-4h-2V7h2v6zm4 4h-2v-2h2v2zm0-4h-2V7h2v6z"/>
              </svg>
              支付宝
            </button>
            <button 
              className="about-sponsor-btn qq"
              onClick={() => handleSponsorClick("qq")}
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
                <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-1 15h-2v-2h2v2zm0-4h-2V7h2v6zm4 4h-2v-2h2v2zm0-4h-2V7h2v6z"/>
              </svg>
              QQ 红包
            </button>
          </div>
        </div>
        
        {/* 页脚 */}
        <div className="about-footer">
          Made with ❤️ by HJH · MIT License
        </div>
      </div>

      {/* 二维码弹窗 */}
      {showQrModal && (
        <div className="qr-modal-overlay" onClick={closeQrModal}>
          <div className="qr-modal-content" onClick={(e) => e.stopPropagation()}>
            <div className="qr-modal-header">
              <h3>{showQrModal === "qqgroup" ? "QQ 群: 894356872" : getQrTitle(showQrModal)}</h3>
              {showQrModal === "qqgroup" && (
                <button 
                  className="qr-modal-copy-btn"
                  onClick={handleCopyQQ}
                  title="复制群号"
                >
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                    <rect x="9" y="9" width="13" height="13" rx="2" ry="2"/>
                    <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
                  </svg>
                  复制群号
                </button>
              )}
              <button className="qr-modal-close" onClick={closeQrModal}>×</button>
            </div>
            <div className="qr-modal-body">
              <img 
                src={showQrModal === "qqgroup" ? "./qqq.jpg" : getQrImage(showQrModal)} 
                alt={showQrModal === "qqgroup" ? "QQ群二维码" : `${getQrTitle(showQrModal)}二维码`}
                className="qr-image"
              />
              <p className="qr-hint">
                {showQrModal === "qqgroup" ? "扫描二维码加入 QQ 群" : "扫描二维码进行支付"}
              </p>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
