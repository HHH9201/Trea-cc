import { ThemeSwitcher } from "./ThemeSwitcher";

interface SidebarProps {
  currentPage: string;
  onNavigate: (page: string) => void;
}

const menuItems = [
  { id: "accounts", label: "账号管理", icon: "👥" },
  { id: "stats", label: "统计数据", icon: "📈" },
  { id: "settings", label: "设置", icon: "⚙️" },
  { id: "about", label: "关于", icon: "ℹ️" },
];

export function Sidebar({ currentPage, onNavigate }: SidebarProps) {
  return (
    <aside className="sidebar">
      <nav className="sidebar-nav">
        {menuItems.map((item) => (
          <div
            key={item.id}
            className={`sidebar-item ${currentPage === item.id ? "active" : ""}`}
            onClick={() => onNavigate(item.id)}
          >
            {/* <span className="sidebar-icon">{item.icon}</span> */}
            <span className="sidebar-label">{item.label}</span>
          </div>
        ))}
      </nav>

      <div className="sidebar-footer">
        <ThemeSwitcher />
      </div>
    </aside>
  );
}
