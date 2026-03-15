use anyhow::{anyhow, Result};
use rusqlite::{Connection, OptionalExtension};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

/// 在指定数据库路径启用隐私模式，并在需要时重启 Trae IDE
pub fn enable_privacy_mode_at_path_with_restart<F>(db_path: PathBuf, restart_fn: F) -> Result<()>
where
    F: FnOnce() -> Result<()>,
{
    // 1. 等待数据库文件存在
    let start = Instant::now();
    let timeout = Duration::from_secs(30);
    while !db_path.exists() {
        if start.elapsed() > timeout {
            return Err(anyhow!("等待 Trae 数据库超时，文件不存在: {:?}", db_path));
        }
        thread::sleep(Duration::from_millis(200));
    }

    // 2. 等待数据库可以被打开
    thread::sleep(Duration::from_millis(500));

    // 3. 读取当前值
    let current_value = read_privacy_mode(&db_path)?;
    println!("[privacy] 当前隐私模式值: {:?}", current_value);

    // 4. 如果已经是隐私模式，直接返回
    if current_value == Some(true) {
        println!("[privacy] 隐私模式已启用");
        return Ok(());
    }

    // 5. 写入隐私模式
    write_privacy_mode(&db_path, true)?;
    println!("[privacy] 已写入隐私模式设置");

    // 6. 验证写入
    thread::sleep(Duration::from_millis(200));
    let new_value = read_privacy_mode(&db_path)?;
    if new_value != Some(true) {
        return Err(anyhow!("写入隐私模式后验证失败，当前值: {:?}", new_value));
    }
    println!("[privacy] 隐私模式写入验证成功");

    // 7. 执行重启
    println!("[privacy] 执行 Trae IDE 重启...");
    restart_fn()?;

    Ok(())
}

/// 从 state.vscdb 读取隐私模式设置
fn read_privacy_mode(db_path: &PathBuf) -> Result<Option<bool>> {
    let conn = Connection::open(db_path)
        .map_err(|e| anyhow!("无法打开数据库: {}", e))?;

    let result: Option<String> = conn
        .query_row(
            "SELECT value FROM ItemTable WHERE key = 'icube.privacy.mode'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| anyhow!("查询隐私模式失败: {}", e))?;

    match result {
        Some(value) => {
            // 解析 JSON 值
            if value.trim().eq_ignore_ascii_case("true") || value == "1" {
                Ok(Some(true))
            } else if value.trim().eq_ignore_ascii_case("false") || value == "0" {
                Ok(Some(false))
            } else {
                // 尝试解析 JSON 字符串
                match serde_json::from_str::<bool>(&value) {
                    Ok(b) => Ok(Some(b)),
                    Err(_) => Ok(None),
                }
            }
        }
        None => Ok(None),
    }
}

/// 向 state.vscdb 写入隐私模式设置
fn write_privacy_mode(db_path: &PathBuf, enabled: bool) -> Result<()> {
    let conn = Connection::open(db_path)
        .map_err(|e| anyhow!("无法打开数据库: {}", e))?;

    let value = if enabled { "true" } else { "false" };

    // 使用 INSERT OR REPLACE 语法
    conn.execute(
        "INSERT OR REPLACE INTO ItemTable (key, value) VALUES ('icube.privacy.mode', ?)",
        [value],
    )
    .map_err(|e| anyhow!("写入隐私模式失败: {}", e))?;

    Ok(())
}
