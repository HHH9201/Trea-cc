export function About() {
  return (
    <div className="about-page">
      <div className="about-card">
        {/* 头部横排 */}
        <div className="about-header">
          <img src="/logo.png" alt="Trae账号管理" className="about-logo" />
          <div className="about-header-text">
            <div className="title-row">
              <h1 className="about-title">Trae账号管理</h1>
              <span className="version">v1.0.2</span>
            </div>
          </div>
        </div>
        
        {/* 说明 */}
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
        
        {/* 信息 */}
        <div className="about-info">
          <div className="info-item">
            <span className="label">开发者</span>
            <span className="value">HJH</span>
          </div>
          <div className="info-item">
            <span className="label">GitHub</span>
            <a 
              href="https://github.com/HHH9201/Trae-CC.git" 
              target="_blank" 
              rel="noopener noreferrer"
              className="value link"
            >
              HHH9201/Trae-CC
            </a>
          </div>
        </div>
        
        {/* 页脚 */}
        <div className="about-footer">
          Made with ❤️ by HJH · MIT License
        </div>
      </div>
    </div>
  );
}
