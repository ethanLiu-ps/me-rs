# Claude Code CLI 最佳使用指南（2026年4月）

---

## 一、安装

```bash
# macOS / Linux
curl -fsSL https://claude.ai/install.sh | bash

# Homebrew
brew install --cask claude-code

# Windows PowerShell
irm https://claude.ai/install.ps1 | iex
```

---

## 二、启动方式

| 命令 | 说明 |
|------|------|
| `claude` | 启动交互式会话 |
| `claude "任务描述"` | 带初始提示启动 |
| `claude -p "查询内容"` | 非交互式，执行一次后退出 |
| `claude -c` | 继续上次会话 |
| `claude -r` | 恢复指定历史会话 |

---

## 三、核心 CLI 参数

**权限模式**（最常用）：
```bash
claude --permission-mode default       # 每步都询问（默认）
claude --permission-mode acceptEdits   # 自动接受文件编辑
claude --permission-mode plan          # 先展示计划再执行
claude --permission-mode auto          # AI分类器自动判断（需Team/Enterprise）
```

**工具限制**：
```bash
claude --allowedTools "Bash,Edit,Read"         # 限定可用工具
claude --disallowedTools "Bash(rm *)"          # 禁止危险命令
```

**模型与计算量**：
```bash
claude --model claude-opus-4-6 --effort high   # Opus 4.6 + 高算力
claude --max-turns 10                           # 最多10轮自主操作
```

**输出格式**（脚本/CI用）：
```bash
claude -p "分析日志" --output-format json
cat error.log | claude -p "总结问题" --output-format stream-json
```

---

## 四、交互模式下的斜杠命令

在交互模式中输入 `/` 查看全部，常用命令：

| 命令 | 作用 |
|------|------|
| `/help` | 帮助 |
| `/config` | 打开设置界面 |
| `/clear` | 清空上下文 |
| `/rewind` | 回滚到某个检查点（代码+对话均可还原） |
| `/compact` | 压缩上下文窗口 |
| `/hooks` | 管理钩子 |
| `/mcp` | 管理 MCP 服务 |
| `/memory` | 查看/编辑 CLAUDE.md |
| `/loop 5m /command` | 定时循环执行命令 |

---

## 五、CLAUDE.md：持久化项目上下文

在项目根目录创建 `CLAUDE.md`，每次会话自动加载：

```markdown
# 项目简介
TypeScript + React 电商平台

# 代码规范
- 使用 ES modules，2空格缩进
- 提交前运行 npm run lint

# 关键模式
- API调用参考 @src/api/client.ts
- 表单组件参考 @src/components/Form.tsx

# 工作流
- 使用约定式提交：feat:, fix:, docs:
```

---

## 六、Hooks：确定性自动化

在 `.claude/settings.json` 中配置，在关键节点自动执行 shell 命令：

```json
{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Edit|Write",
        "hooks": [
          {
            "type": "command",
            "command": "jq -r '.tool_input.file_path' | xargs npx prettier --write"
          }
        ]
      }
    ],
    "Notification": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "osascript -e 'display notification \"Claude 需要你的输入\" with title \"Claude Code\"'"
          }
        ]
      }
    ]
  }
}
```

常见场景：编辑后自动格式化、保护敏感文件、运行 lint、发送通知。

---

## 七、MCP 服务：连接外部工具

```bash
claude mcp add github     # 添加 GitHub MCP
claude mcp list           # 查看已配置服务
```

或在 `settings.json` 中配置：
```json
{
  "mcpServers": {
    "github": {
      "command": "npx",
      "args": ["@anthropic-ai/mcp-server-github"],
      "env": { "GITHUB_PERSONAL_ACCESS_TOKEN": "$GITHUB_TOKEN" }
    }
  }
}
```

支持：GitHub、Slack、Notion、Linear、Jira、HubSpot 等。

---

## 八、IDE 集成

**VS Code**：安装 "Claude Code" 扩展
- `Cmd+Esc` — 编辑器与 Claude 间切换焦点
- `Option+K` — 插入文件 @引用（支持行号范围）

**JetBrains**：安装 "Claude Code Beta" 插件
- `Cmd+Esc` — 打开 Claude Code

---

## 九、最佳实践

**1. 给出验收标准**
```
实现 validateEmail()，并写测试。
测试用例：user@example.com → true，无效地址 → false
实现后自动运行测试，有失败则修复
```

**2. 先 Plan 后执行**
- 按 `Shift+Tab` 进入 Plan Mode
- 让 Claude 展示计划，确认后再执行

**3. 复杂探索用子代理**
```
"用子代理调查 auth 模块的 token 刷新逻辑"
```

**4. 用 @引用提供精准上下文**
```
"参考 @src/api/client.ts 的模式，实现 @src/payment.ts"
```

**5. 批量操作用非交互模式**
```bash
for file in *.py; do
  claude -p "为 $file 添加类型注解" --allowedTools "Edit"
done
```

**6. 及时用 `/clear` 切换任务**，避免不相关上下文干扰。

**7. 用 `/rewind` 快速回滚**，每个操作都有检查点，可随时恢复。

---

## 十、场景速查

| 场景 | 推荐方式 |
|------|---------|
| 一次性问题 | `claude -p "..."` |
| 探索陌生代码库 | 交互模式 + Plan Mode |
| 开发新功能 | 交互模式，复杂任务先 Plan |
| 修 Bug | 交互模式 + 复现步骤 + 测试 |
| CI/CD 自动化 | 非交互模式 `-p` + `--output-format json` |
| 定时任务 | Hooks + `/loop` |
| 安全敏感变更 | Plan Mode 审查后再执行 |
| IDE 内使用 | VS Code / JetBrains 插件 |
