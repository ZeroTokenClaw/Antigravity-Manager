//! Auto_08gg 模型路由（协议无关前置解析）
//!
//! 规范：`docs/AUTO_MODEL_ROUTER.md`
//! 目录对齐：https://ai.08gg.com/v1/models
//!
//! `model=Auto_08gg`：按提示词分级，在对应档位内**优先高级模型**，不可用时沿链降级。

use serde_json::Value;
use std::collections::HashSet;

/// 提示词分级规则（观测 / 后续 LLM 分类器复用）
pub const ROUTING_PROMPT_RUBRIC: &str = r#"
Classify the chat task into exactly one tier based on the user prompt:
- complex: architecture, system design, large refactors, deep multi-module analysis
- code: implement/fix/debug code, agent tool use, code blocks for development
- simple: short Q&A, translation, summary, formatting, casual text
"#;

/// 主入口别名（大小写不敏感）
pub const AUTO_08GG_ALIAS: &str = "Auto_08gg";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoTier {
    Simple,
    Code,
    Complex,
}

impl AutoTier {
    pub fn as_alias(self) -> &'static str {
        match self {
            Self::Simple => "auto-simple",
            Self::Code => "auto-code",
            Self::Complex => "auto-complex",
        }
    }

    /// 该档位内的降级链：索引越小优先级越高
    pub fn fallback_chain(self) -> &'static [&'static str] {
        match self {
            Self::Complex => COMPLEX_FALLBACK,
            Self::Code => CODE_FALLBACK,
            Self::Simple => SIMPLE_FALLBACK,
        }
    }
}

/// Complex：优先 Opus thinking → Opus → Sonnet thinking → Pro high …
const COMPLEX_FALLBACK: &[&str] = &[
    "claude-opus-4-6-thinking",
    "claude-opus-4-6",
    "claude-sonnet-4-6-thinking",
    "claude-sonnet-4-6",
    "gemini-3.1-pro-high",
    "gemini-3.1-pro",
    "gemini-3.1-pro-low",
    "gemini-pro-agent",
    "gemini-3.8-flash-high",
    "gemini-3.8-flash-medium",
    "gemini-3.8-flash",
    "gemini-3.7-flash-high",
    "gemini-3.7-flash",
    "gemini-3.6-flash-high",
    "gemini-3.6-flash",
    "gpt-oss-120b-medium",
    "gemini-3.5-flash-extra",
    "gemini-3.5-flash",
    "gemini-3-flash",
];

/// Code：优先 Sonnet → Agent → 新版 Flash high …
const CODE_FALLBACK: &[&str] = &[
    "claude-sonnet-4-6-thinking",
    "claude-sonnet-4-6",
    "claude-opus-4-6-thinking",
    "claude-opus-4-6",
    "gemini-3-flash-agent",
    "gemini-pro-agent",
    "gemini-3.8-flash-high",
    "gemini-3.8-flash-medium",
    "gemini-3.8-flash",
    "gemini-3.7-flash-high",
    "gemini-3.7-flash-medium",
    "gemini-3.7-flash",
    "gemini-3.6-flash-high",
    "gemini-3.6-flash-medium",
    "gemini-3.6-flash",
    "gpt-oss-120b-medium",
    "gemini-3.5-flash-extra",
    "gemini-3.5-flash",
    "gemini-3.1-flash-lite",
    "gemini-3-flash",
];

/// Simple：优先最新 Flash 高档 → 中档 → 默认 → 旧版 → lite
const SIMPLE_FALLBACK: &[&str] = &[
    "gemini-3.8-flash-high",
    "gemini-3.8-flash-medium",
    "gemini-3.8-flash",
    "gemini-3.8-flash-low",
    "gemini-3.8-flash-tiered",
    "gemini-3.7-flash-high",
    "gemini-3.7-flash-medium",
    "gemini-3.7-flash",
    "gemini-3.7-flash-low",
    "gemini-3.7-flash-tiered",
    "gemini-3.6-flash-high",
    "gemini-3.6-flash-medium",
    "gemini-3.6-flash",
    "gemini-3.6-flash-low",
    "gemini-3.6-flash-tiered",
    "gemini-3.5-flash-extra",
    "gemini-3.5-flash",
    "gemini-3.5-flash-low",
    "gemini-3.5-flash-extra-low",
    "gemini-3.5-flash-lite",
    "gemini-3.1-flash-lite",
    "gemini-3-flash",
    "tab_flash_lite_preview",
    "gpt-oss-120b-medium",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedAutoRoute {
    pub requested: String,
    pub alias: String,
    pub target_model: String,
    pub tier: AutoTier,
    pub reason: String,
    pub prompt_excerpt: String,
    /// 本档完整候选链（观测用）
    pub candidates: Vec<String>,
}

pub fn is_auto_model(model: &str) -> bool {
    matches!(
        normalize_alias(model).as_str(),
        "auto_08gg"
            | "auto08gg"
            | "auto"
            | "auto_simple"
            | "auto_code"
            | "auto_complex"
            | "auto-simple"
            | "auto-code"
            | "auto-complex"
    )
}

fn normalize_alias(model: &str) -> String {
    model
        .trim()
        .to_ascii_lowercase()
        .replace('-', "_")
}

fn tier_from_explicit_alias(model: &str) -> Option<AutoTier> {
    match normalize_alias(model).as_str() {
        "auto_simple" | "auto-simple" => Some(AutoTier::Simple),
        "auto_code" | "auto-code" => Some(AutoTier::Code),
        "auto_complex" | "auto-complex" => Some(AutoTier::Complex),
        _ => None,
    }
}

/// 从 messages 提取路由提示词：system/developer + 最后一条 user
pub fn extract_routing_prompt(messages: &[Value]) -> String {
    let mut system_parts = Vec::new();
    let mut last_user = String::new();

    for msg in messages {
        let role = msg
            .get("role")
            .and_then(|r| r.as_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let text = message_text(msg);
        if text.trim().is_empty() {
            continue;
        }
        match role.as_str() {
            "system" | "developer" => system_parts.push(text),
            "user" => last_user = text,
            _ => {}
        }
    }

    let mut out = String::new();
    if !system_parts.is_empty() {
        out.push_str(&system_parts.join("\n"));
        out.push_str("\n\n");
    }
    out.push_str(&last_user);
    out
}

fn message_text(msg: &Value) -> String {
    if let Some(s) = msg.get("content").and_then(|c| c.as_str()) {
        return s.to_string();
    }
    if let Some(arr) = msg.get("content").and_then(|c| c.as_array()) {
        let mut parts = Vec::new();
        for part in arr {
            if let Some(t) = part.get("text").and_then(|t| t.as_str()) {
                parts.push(t.to_string());
            } else if let Some(t) = part.as_str() {
                parts.push(t.to_string());
            }
        }
        return parts.join("\n");
    }
    String::new()
}

pub fn classify_task(messages: &[Value], tools: Option<&Value>) -> (AutoTier, String, String) {
    let prompt = extract_routing_prompt(messages);
    let lower = prompt.to_ascii_lowercase();
    let has_tools = tools_present(tools) || messages_have_tool_calls(messages);
    let code_fences = lower.matches("```").count();
    let char_len = prompt.chars().count();

    let mut complex_score = 0i32;
    let mut code_score = 0i32;

    for p in COMPLEX_PATTERNS {
        if lower.contains(p) {
            complex_score += 3;
        }
    }
    for p in CODE_INTENT_PATTERNS {
        if lower.contains(p) {
            code_score += 2;
        }
    }
    if code_fences >= 1 {
        code_score += 3;
    }
    if code_fences >= 4 {
        complex_score += 2;
    }
    if char_len > 6000 {
        complex_score += 2;
    }
    if has_tools {
        code_score += 4;
    }
    if SIMPLE_QA_PATTERNS.iter().any(|p| lower.contains(p)) && code_fences == 0 && !has_tools {
        code_score = code_score.saturating_sub(3);
    }

    let (tier, reason) = if complex_score >= 3 && complex_score >= code_score {
        (
            AutoTier::Complex,
            format!(
                "prompt-classified complex (score={complex_score}, code={code_score}, fences={code_fences})"
            ),
        )
    } else if code_score >= 2 || has_tools || code_fences >= 1 {
        (
            AutoTier::Code,
            format!(
                "prompt-classified code (score={code_score}, complex={complex_score}, tools={has_tools}, fences={code_fences})"
            ),
        )
    } else {
        (
            AutoTier::Simple,
            format!(
                "prompt-classified simple (score_code={code_score}, score_complex={complex_score}, chars={char_len})"
            ),
        )
    };

    (tier, reason, prompt)
}

const COMPLEX_PATTERNS: &[&str] = &[
    "architecture",
    "system design",
    "系统设计",
    "架构设计",
    "架构评审",
    "架构方案",
    "大规模重构",
    "multi-module",
    "微服务拆分",
    "trade-off",
    "权衡对比",
    "deep analysis",
    "深度分析",
    "end-to-end design",
    "技术方案选型",
    "设计文档",
    "rfc",
];

const CODE_INTENT_PATTERNS: &[&str] = &[
    "写代码",
    "改代码",
    "实现一个",
    "实现功能",
    "修复bug",
    "修 bug",
    "fix bug",
    "fix the",
    "compile error",
    "编译错误",
    "单元测试",
    "write a function",
    "implement ",
    "refactor ",
    "pull request",
    "stack trace",
    "报错",
    "debug",
    "调试",
    "pr ",
    "code review",
    "类型错误",
    "lsp",
    "cargo build",
    "npm run",
    "pnpm ",
];

const SIMPLE_QA_PATTERNS: &[&str] = &[
    "介绍",
    "是什么",
    "什么是",
    "翻译",
    "总结",
    "摘要",
    "一句话",
    "explain",
    "what is",
    "summarize",
    "translate",
];

fn tools_present(tools: Option<&Value>) -> bool {
    match tools {
        Some(Value::Array(arr)) => !arr.is_empty(),
        Some(Value::Object(map)) => !map.is_empty(),
        _ => false,
    }
}

fn messages_have_tool_calls(messages: &[Value]) -> bool {
    messages.iter().any(|m| {
        m.get("tool_calls").is_some()
            || m.get("function_call").is_some()
            || m.get("role")
                .and_then(|r| r.as_str())
                .is_some_and(|r| r.eq_ignore_ascii_case("tool"))
    })
}

fn excerpt(s: &str, max: usize) -> String {
    let trimmed = s.trim();
    if trimmed.chars().count() <= max {
        return trimmed.to_string();
    }
    trimmed.chars().take(max).collect::<String>() + "…"
}

/// 在降级链中挑选第一个「可用」模型；无可用性信息时取链首（最高级）
pub fn pick_from_fallback_chain(
    chain: &[&str],
    available: Option<&HashSet<String>>,
) -> (String, String) {
    let Some(avail) = available.filter(|s| !s.is_empty()) else {
        let top = chain.first().copied().unwrap_or("gemini-3.8-flash");
        return (top.to_string(), "no-availability-data, prefer top".into());
    };

    let avail_lower: HashSet<String> = avail.iter().map(|s| s.to_ascii_lowercase()).collect();

    for &cand in chain {
        if avail_lower.contains(&cand.to_ascii_lowercase()) {
            // 尽量保留账号侧原始大小写
            let exact = avail
                .iter()
                .find(|a| a.eq_ignore_ascii_case(cand))
                .cloned()
                .unwrap_or_else(|| cand.to_string());
            return (
                exact,
                format!("picked from availability chain (preferred={cand})"),
            );
        }
    }

    let top = chain.first().copied().unwrap_or("gemini-3.8-flash");
    (
        top.to_string(),
        format!("no chain member in availability, fallback top={top}"),
    )
}

/// 解析 Auto_08gg / 兼容旧 auto-* 别名
pub fn resolve_auto_model(
    requested_model: &str,
    messages: &[Value],
    tools: Option<&Value>,
    available_models: Option<&HashSet<String>>,
) -> Option<ResolvedAutoRoute> {
    let trimmed = requested_model.trim();
    if !is_auto_model(trimmed) {
        return None;
    }

    let (tier, class_reason, prompt) = if let Some(t) = tier_from_explicit_alias(trimmed) {
        (
            t,
            format!("explicit alias {trimmed}"),
            extract_routing_prompt(messages),
        )
    } else {
        classify_task(messages, tools)
    };

    let chain = tier.fallback_chain();
    let (target, pick_reason) = pick_from_fallback_chain(chain, available_models);

    Some(ResolvedAutoRoute {
        requested: trimmed.to_string(),
        alias: if normalize_alias(trimmed) == "auto_08gg" || normalize_alias(trimmed) == "auto08gg"
        {
            AUTO_08GG_ALIAS.to_string()
        } else {
            tier.as_alias().to_string()
        },
        target_model: target,
        tier,
        reason: format!("{class_reason}; {pick_reason}"),
        prompt_excerpt: excerpt(&prompt, 160),
        candidates: chain.iter().map(|s| (*s).to_string()).collect(),
    })
}

/// 暴露给 /v1/models 的别名（主入口在前）
pub fn list_auto_model_ids() -> &'static [&'static str] {
    &[
        AUTO_08GG_ALIAS,
        "auto",
        "auto-simple",
        "auto-code",
        "auto-complex",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn auto_08gg_alias_recognized() {
        assert!(is_auto_model("Auto_08gg"));
        assert!(is_auto_model("auto_08gg"));
        assert!(is_auto_model("AUTO-08GG"));
    }

    #[test]
    fn prefers_high_when_available() {
        let mut avail = HashSet::new();
        avail.insert("gemini-3.8-flash".into());
        avail.insert("gemini-3.8-flash-high".into());
        avail.insert("gemini-3.5-flash".into());
        let (picked, _) = pick_from_fallback_chain(SIMPLE_FALLBACK, Some(&avail));
        assert_eq!(picked, "gemini-3.8-flash-high");
    }

    #[test]
    fn degrades_when_high_missing() {
        let mut avail = HashSet::new();
        avail.insert("gemini-3.5-flash".into());
        avail.insert("gemini-3-flash".into());
        let (picked, _) = pick_from_fallback_chain(SIMPLE_FALLBACK, Some(&avail));
        assert_eq!(picked, "gemini-3.5-flash");
    }

    #[test]
    fn complex_prefers_opus_thinking() {
        let mut avail = HashSet::new();
        avail.insert("claude-sonnet-4-6".into());
        avail.insert("claude-opus-4-6-thinking".into());
        let messages = vec![json!({
            "role":"user",
            "content":"请做一次系统架构设计，给出微服务拆分与权衡对比"
        })];
        let r = resolve_auto_model("Auto_08gg", &messages, None, Some(&avail)).unwrap();
        assert_eq!(r.tier, AutoTier::Complex);
        assert_eq!(r.target_model, "claude-opus-4-6-thinking");
    }

    #[test]
    fn code_degrades_to_sonnet_when_thinking_missing() {
        let mut avail = HashSet::new();
        avail.insert("claude-sonnet-4-6".into());
        avail.insert("gemini-3.8-flash".into());
        let messages = vec![json!({
            "role":"user",
            "content":"请修复这段代码的编译错误：\n```rs\nfn main(){}\n```"
        })];
        let r = resolve_auto_model("Auto_08gg", &messages, None, Some(&avail)).unwrap();
        assert_eq!(r.tier, AutoTier::Code);
        assert_eq!(r.target_model, "claude-sonnet-4-6");
    }

    #[test]
    fn simple_qa_uses_prompt() {
        let messages = vec![json!({"role":"user","content":"用一句话介绍 Rust"})];
        let r = resolve_auto_model("Auto_08gg", &messages, None, None).unwrap();
        assert_eq!(r.tier, AutoTier::Simple);
        assert_eq!(r.target_model, "gemini-3.8-flash-high");
    }
}
