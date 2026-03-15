import { useEffect, useRef } from "react";

interface ContextMenuProps {
  x: number;
  y: number;
  onClose: () => void;
  onRelogin: () => void;
  onViewDetail: () => void;
  onRefresh: () => void;
  onUpdateToken: () => void;
  onCopyToken: () => void;
  onSwitchAccount: () => void;
  onBuyPro: () => void;
  onDelete: () => void;
  isCurrent?: boolean; // 是否是当前使用的账号
}

export function ContextMenu({
  x,
  y,
  onClose,
  onRelogin,
  onViewDetail,
  onRefresh,
  onUpdateToken,
  onCopyToken,
  onSwitchAccount,
  onBuyPro,
  onDelete,
  isCurrent = false,
}: ContextMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    // 调整菜单位置，防止超出屏幕
    if (menuRef.current) {
      const menu = menuRef.current;
      const rect = menu.getBoundingClientRect();

      if (rect.right > window.innerWidth) {
        menu.style.left = `${x - rect.width}px`;
      }
      if (rect.bottom > window.innerHeight) {
        menu.style.top = `${y - rect.height}px`;
      }
    }
  }, [x, y]);

  return (
    <>
      <div className="context-menu-overlay" onClick={onClose} />
      <div
        ref={menuRef}
        className="context-menu"
        style={{ left: x, top: y }}
      >
        {isCurrent ? (
          <div className="context-menu-item" onClick={onRelogin}>
            <span className="icon">🔁</span>
            重新登录
          </div>
        ) : (
          <div className="context-menu-item" onClick={onSwitchAccount}>
            <span className="icon">🔀</span>
            切换账号
          </div>
        )}
        <div className="context-menu-item" onClick={onRefresh}>
          <span className="icon">🔄</span>
          刷新数据
        </div>
        <div className="context-menu-item" onClick={onViewDetail}>
          <span className="icon">👁</span>
          查看详情
        </div>
        <div className="context-menu-item" onClick={onUpdateToken}>
          <span className="icon">🔐</span>
          更新 Token
        </div>
        <div className="context-menu-item" onClick={onCopyToken}>
          <span className="icon">🔑</span>
          复制 Token
        </div>
        <div className="context-menu-item" onClick={onBuyPro}>
          <span className="icon">🛒</span>
          购买 Pro
        </div>
        <div className="context-menu-divider" />
        <div className="context-menu-item danger" onClick={onDelete}>
          <span className="icon">🗑</span>
          删除账号
        </div>
      </div>
    </>
  );
}
