// 工具函数

pub fn generate_random_id() -> String {
    use rand::Rng;
    rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(8)
        .map(char::from)
        .collect()
}

/// 根据模型名称推测功能类型
// 注意：此函数已弃用，请改用 mappers::common_utils::resolve_request_config
pub fn _deprecated_infer_quota_group(model: &str) -> String {
    if model.to_lowercase().starts_with("claude") {
        "claude".to_string()
    } else {
        "gemini".to_string()
    }
}

/// 工具调用 ID 统一归一化处理。
///
/// 规范格式统一为以 `call_` 开头（如 `call_573077`）。
/// 部分客户端（如 Antigravity IDE、VS Code 插件或某些中间代理）在回传时会丢失下划线
/// 产生类似 `call573077` 的形式，导致与网关签名缓存与思考存储无法精确匹配。
///
/// 本函数将 `call<digits>` 或丢失下划线的 call ID 规范化为统一的 `call_<digits>` 格式。
/// 规范化 tool / functionCall id，满足 Claude Vertex 约束 `^[a-zA-Z0-9_-]+$`
pub fn normalize_tool_id(id: &str) -> std::borrow::Cow<'_, str> {
    let trimmed = id.trim();
    if trimmed.is_empty() {
        return std::borrow::Cow::Owned("call_unknown".to_string());
    }

    // 丢失下划线的 callXXXX → call_XXXX
    let with_call_underscore = if trimmed.starts_with("call")
        && !trimmed.starts_with("call_")
        && trimmed.len() > 4
    {
        let rest = &trimmed[4..];
        if rest.chars().next().is_some_and(|c| c.is_ascii_digit())
            || (rest.len() >= 6 && rest.chars().all(|c| c.is_ascii_hexdigit()))
        {
            Some(format!("call_{}", rest))
        } else {
            None
        }
    } else {
        None
    };

    let candidate = with_call_underscore
        .as_deref()
        .unwrap_or(trimmed);

    let needs_sanitize = candidate
        .chars()
        .any(|c| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'));

    if !needs_sanitize {
        return if with_call_underscore.is_some() {
            std::borrow::Cow::Owned(candidate.to_string())
        } else {
            std::borrow::Cow::Borrowed(id)
        };
    }

    let mut cleaned: String = candidate
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    while cleaned.contains("__") {
        cleaned = cleaned.replace("__", "_");
    }
    let cleaned = cleaned.trim_matches('_').to_string();
    if cleaned.is_empty() {
        return std::borrow::Cow::Owned("call_unknown".to_string());
    }
    if cleaned
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic())
    {
        std::borrow::Cow::Owned(cleaned)
    } else {
        std::borrow::Cow::Owned(format!("call_{cleaned}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_tool_id() {
        // 丢失下划线的客户端数字/哈希 ID 统一归一化为 call_
        assert_eq!(normalize_tool_id("call573077").as_ref(), "call_573077");
        assert_eq!(normalize_tool_id("call1599720").as_ref(), "call_1599720");
        assert_eq!(normalize_tool_id("call932369").as_ref(), "call_932369");
        assert_eq!(normalize_tool_id("call389691").as_ref(), "call_389691");
        assert_eq!(normalize_tool_id("call0123456").as_ref(), "call_0123456");
        assert_eq!(normalize_tool_id("calld4e5f6a1").as_ref(), "call_d4e5f6a1");

        // 已符合规范的保持原样
        assert_eq!(normalize_tool_id("call_573077").as_ref(), "call_573077");
        assert_eq!(
            normalize_tool_id("call_read_root_ada28549fe0e_0").as_ref(),
            "call_read_root_ada28549fe0e_0"
        );
        assert_eq!(normalize_tool_id("toolu_01Abc").as_ref(), "toolu_01Abc");

        // 普通词汇或短 ID 不误判
        assert_eq!(normalize_tool_id("call").as_ref(), "call");
        assert_eq!(normalize_tool_id("calling").as_ref(), "calling");
        assert_eq!(normalize_tool_id("callback").as_ref(), "callback");
        assert_eq!(normalize_tool_id("").as_ref(), "call_unknown");

        // Claude Vertex: 非法字符（冒号等）必须清洗
        assert_eq!(
            normalize_tool_id("call:default_api:read_file").as_ref(),
            "call_default_api_read_file"
        );
        assert_eq!(
            normalize_tool_id("functions.Shell:0").as_ref(),
            "functions_Shell_0"
        );
    }
}
