#!/usr/bin/env -S uv run python
"""Extract a structured handoff summary from a Claude Code transcript.

Called by pre-compact.sh to automatically save session state before compaction.
Reads the transcript JSONL, extracts user prompts, files modified, and the
last assistant response to produce a compact summary for the next instance.
"""
import json
import sys


def extract_handoff(transcript_path, max_chars=3000):
    """Parse transcript and produce a structured handoff summary."""
    user_prompts = []
    assistant_texts = []
    tools_used = set()
    files_modified = set()

    with open(transcript_path) as f:
        for line in f:
            try:
                obj = json.loads(line)
            except json.JSONDecodeError:
                continue

            if "message" not in obj:
                continue

            msg = obj["message"]
            role = msg.get("role")
            content = msg.get("content", "")

            if role == "user":
                if isinstance(content, str) and content.strip():
                    text = content.strip()
                    if not text.startswith("<system-reminder>"):
                        user_prompts.append(text[:200])
                elif isinstance(content, list):
                    for block in content:
                        if isinstance(block, dict) and block.get("type") == "text":
                            text = block.get("text", "").strip()
                            if text and not text.startswith("<system-reminder>"):
                                user_prompts.append(text[:200])

            elif role == "assistant":
                if isinstance(content, list):
                    for block in content:
                        if not isinstance(block, dict):
                            continue
                        btype = block.get("type")
                        if btype == "text":
                            text = block.get("text", "").strip()
                            if text:
                                assistant_texts.append(text)
                        elif btype == "tool_use":
                            tool_name = block.get("name", "")
                            tools_used.add(tool_name)
                            tool_input = block.get("input", {})
                            if tool_name in ("Edit", "Write"):
                                fp = tool_input.get("file_path", "")
                                if fp:
                                    files_modified.add(fp)

    sections = []

    # What the user asked for
    if user_prompts:
        first = user_prompts[:3]
        last = user_prompts[-2:] if len(user_prompts) > 3 else []
        prompt_summary = "\n".join(f"- {p}" for p in first)
        if last:
            prompt_summary += "\n...\n" + "\n".join(f"- {p}" for p in last)
        sections.append(f"## User Requests\n\n{prompt_summary}")

    if files_modified:
        files_list = "\n".join(f"- {f}" for f in sorted(files_modified))
        sections.append(f"## Files Modified\n\n{files_list}")

    if tools_used:
        sections.append(f"## Tools Used\n\n{', '.join(sorted(tools_used))}")

    if assistant_texts:
        last_response = assistant_texts[-1]
        if len(last_response) > 800:
            last_response = last_response[:800] + "..."
        sections.append(f"## Last Response Summary\n\n{last_response}")

    result = "\n\n".join(sections)
    if len(result) > max_chars:
        result = result[:max_chars - 3] + "..."

    return result


if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: extract-handoff.py <transcript.jsonl>", file=sys.stderr)
        sys.exit(1)
    print(extract_handoff(sys.argv[1]))
