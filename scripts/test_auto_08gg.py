#!/usr/bin/env python3
"""Verify Auto_08gg routing against a Chat Completions endpoint.

Usage:
  set ANTIGRAVITY_BASE_URL=http://127.0.0.1:8045/v2
  set ANTIGRAVITY_API_KEY=sk-xxx
  python scripts/test_auto_08gg.py

  # or against remote:
  set ANTIGRAVITY_BASE_URL=https://ai.08gg.com/v2
  python scripts/test_auto_08gg.py
"""

from __future__ import annotations

import os
import sys
import time

try:
    from openai import OpenAI
except ImportError:
    print("[FAIL] missing dependency: pip install openai")
    sys.exit(1)


BASE_URL = os.environ.get("ANTIGRAVITY_BASE_URL", "https://ai.08gg.com/v2").rstrip("/")
API_KEY = os.environ.get("ANTIGRAVITY_API_KEY") or os.environ.get("OPENAI_API_KEY") or ""
MODEL = os.environ.get("ANTIGRAVITY_MODEL", "Auto_08gg")
TIMEOUT = float(os.environ.get("ANTIGRAVITY_TIMEOUT", "180"))

# Prompt samples expected to land in different tiers
CASES = [
    {
        "name": "simple",
        "content": "用一句话介绍 Rust",
        "expect_tier_hint": "simple / flash",
        "expect_model_substr": ("flash",),
        "timeout": 60.0,
    },
    {
        "name": "code",
        "content": "请修复这段代码的编译错误：\n```rs\nfn main() { let x = 1 }\n```",
        "expect_tier_hint": "code / sonnet",
        "expect_model_substr": ("sonnet", "flash-agent", "agent"),
        "timeout": 90.0,
    },
    {
        "name": "complex",
        "content": "请做一次系统架构设计，用三条要点说明微服务拆分与权衡",
        "expect_tier_hint": "complex / opus",
        "expect_model_substr": ("opus", "sonnet", "pro"),
        "timeout": TIMEOUT,
    },
]


def main() -> int:
    if not API_KEY:
        print("[FAIL] set ANTIGRAVITY_API_KEY or OPENAI_API_KEY")
        return 2

    print("=== Auto_08gg smoke test ===", flush=True)
    print(f"base_url : {BASE_URL}", flush=True)
    print(f"model    : {MODEL}", flush=True)
    print(flush=True)

    client = OpenAI(
        base_url=BASE_URL,
        api_key=API_KEY,
        timeout=TIMEOUT,
        max_retries=1,
    )

    # 1) models list — prefer /v1/models when base is /v2
    try:
        print("[step] models.list ...", flush=True)
        models_base = BASE_URL[:-3] + "/v1" if BASE_URL.endswith("/v2") else BASE_URL
        models_client = OpenAI(
            base_url=models_base, api_key=API_KEY, timeout=30.0, max_retries=0
        )
        models = models_client.models.list()
        ids = [m.id for m in models.data]
        has_auto = any(
            i.lower().replace("-", "_") in ("auto_08gg", "auto") for i in ids
        )
        print(
            f"[models] base={models_base} count={len(ids)} Auto_08gg/auto present={has_auto}",
            flush=True,
        )
    except Exception as e:
        print(f"[WARN] models.list failed: {e}", flush=True)

    # 2) chat cases
    failed = 0
    for case in CASES:
        print("-" * 60, flush=True)
        print(
            f"[case] {case['name']} (expect ~ {case['expect_tier_hint']})",
            flush=True,
        )
        case_timeout = float(case.get("timeout", TIMEOUT))
        t0 = time.time()
        try:
            resp = client.chat.completions.create(
                model=MODEL,
                messages=[{"role": "user", "content": case["content"]}],
                temperature=0,
                max_tokens=512,
                timeout=case_timeout,
            )
            elapsed = time.time() - t0
            choice = resp.choices[0]
            content = (choice.message.content or "").strip()
            used_model = (getattr(resp, "model", None) or MODEL).lower()
            print(f"[ok]   latency={elapsed:.2f}s response_model={used_model}", flush=True)
            print(f"       finish={choice.finish_reason}", flush=True)
            preview = content.replace("\n", " ")[:180]
            print(f"       content={preview!r}", flush=True)
            if not content:
                print("[FAIL] empty content", flush=True)
                failed += 1
                continue
            hints = case.get("expect_model_substr") or ()
            if hints and not any(h in used_model for h in hints):
                print(
                    f"[WARN] response_model={used_model} not in expected {hints} "
                    "(may be degraded by availability)",
                    flush=True,
                )
        except Exception as e:
            elapsed = time.time() - t0
            print(f"[FAIL] latency={elapsed:.2f}s error={e}", flush=True)
            failed += 1

    print("-" * 60, flush=True)
    if failed:
        print(f"[RESULT] FAILED ({failed}/{len(CASES)} cases)", flush=True)
        return 1
    print(f"[RESULT] OK ({len(CASES)}/{len(CASES)} cases)", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
