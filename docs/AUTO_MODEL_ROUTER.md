# Auto_08gg 模型路由

最后更新：2026-10-09  
实现：`src-tauri/src/proxy/common/auto_model_router.rs`  
模型目录参考：[ai.08gg.com/v1/models](https://ai.08gg.com/v1/models)

## 1. 客户端用法

```json
{
  "model": "Auto_08gg",
  "messages": [{ "role": "user", "content": "..." }]
}
```

兼容旧别名：`auto` / `auto-simple` / `auto-code` / `auto-complex`（大小写不敏感，`-`/`_` 等价）。

路径：`POST /v1/chat/completions` 与 `POST /v2/chat/completions` 同构。

## 2. 流程

```
提示词分级 → 档位降级链（高→低）→ 对照账号可用模型挑选 → resolve_model_route / 选号
```

提示词 = 全部 `system`/`developer` + **最后一条 user**。

## 3. 三档 + 优先高级、不可用再降级

| 档位 | 触发（提示词） | 降级链（节选，完整见代码） |
| --- | --- | --- |
| **complex** | 架构/系统设计/权衡/大规模重构… | `claude-opus-4-6-thinking` → `claude-opus-4-6` → `claude-sonnet-4-6-thinking` → … → Flash |
| **code** | 实现/修 bug/代码块/`tools`… | `claude-sonnet-4-6-thinking` → `claude-sonnet-4-6` → agent → `gemini-3.8-flash-high` → … |
| **simple** | 问答/翻译/介绍（默认） | `gemini-3.8-flash-high` → `…-medium` → `gemini-3.8-flash` → 3.7 → 3.6 → 3.5 → lite |

可用性来源：`TokenManager::get_all_collected_models()`。若池为空则取链首（最高级）。

未纳入自动链（专项）：`gemini-*-image*`、内部 `tab_jump_*` 等。

## 4. 观测日志

```
[AutoRouter] Auto_08gg -> Auto_08gg -> claude-sonnet-4-6 | prompt-classified code; picked from availability… | prompt="…"
```

## 5. 相关修复

Claude Vertex 要求 `tool_use.id ~= ^[a-zA-Z0-9_-]+$`。  
`normalize_tool_id` 会清洗冒号等非法字符；OpenAI→Gemini 转换时同步规范化 `functionCall.id`。
