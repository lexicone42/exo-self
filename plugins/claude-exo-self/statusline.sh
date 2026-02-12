#!/bin/bash

# Claude Code Custom Status Line
# v4.0.0 - Two-line compact layout
# Line 1: Exo-self indicator | Model | Repo:Branch | git status | lines changed
# Line 2: Context bar | percentage | duration | cost

# Read JSON from stdin
input=$(cat)

# Parse Claude data
model=$(echo "$input" | jq -r '.model.display_name // "Claude"' | sed 's/Claude //')
current_dir=$(echo "$input" | jq -r '.workspace.current_dir // env.PWD')
lines_added=$(echo "$input" | jq -r '.cost.total_lines_added // 0')
lines_removed=$(echo "$input" | jq -r '.cost.total_lines_removed // 0')
session_id=$(echo "$input" | jq -r '.session_id // ""')

# Get git information (change to workspace directory)
cd "$current_dir" 2>/dev/null || cd "$HOME"

# Check if we're in a git repo
if git rev-parse --git-dir > /dev/null 2>&1; then
    repo_name=$(basename "$(git rev-parse --show-toplevel 2>/dev/null)" || echo "")
    branch=$(git branch --show-current 2>/dev/null || echo "detached")

    # Git status indicators
    git_status=""
    if [[ -n $(git status --porcelain 2>/dev/null) ]]; then
        git_status="*"
    fi

    # Check ahead/behind remote
    upstream=$(git rev-parse --abbrev-ref --symbolic-full-name @{u} 2>/dev/null)
    if [[ -n "$upstream" ]]; then
        ahead=$(git rev-list --count "$upstream"..HEAD 2>/dev/null || echo "0")
        behind=$(git rev-list --count HEAD.."$upstream" 2>/dev/null || echo "0")
        [[ "$ahead" -gt 0 ]] && git_status="${git_status}↑${ahead}"
        [[ "$behind" -gt 0 ]] && git_status="${git_status}↓${behind}"
    fi
else
    repo_name=""
    branch=""
    git_status=""
fi

# Check if exo-self is active for this session
exo_indicator=""
if [[ -n "$session_id" && -f "$HOME/.claude/exo-self/sessions/state-${session_id}.json" ]]; then
    exo_indicator="\033[1;35m◈\033[0m "
fi

# Build Line 1: Exo + Model + Repo:Branch + Status + Changes
line1="${exo_indicator}\033[1;36m[$model]\033[0m "

if [[ -n "$repo_name" ]]; then
    line1+="\033[1;32m$repo_name\033[0m"
    if [[ -n "$branch" ]]; then
        line1+=":\033[1;34m$branch\033[0m"
    fi
fi

if [[ -n "$git_status" ]]; then
    line1+=" \033[1;31m$git_status\033[0m"
fi

if [[ "$lines_added" -gt 0 || "$lines_removed" -gt 0 ]]; then
    line1+=" | \033[0;32m+$lines_added\033[0m/\033[0;31m-$lines_removed\033[0m"
fi

# Build Line 2: Context bar + percentage + duration + cost
duration_ms=$(echo "$input" | jq -r '.cost.total_duration_ms // 0')
duration_hours=$((duration_ms / 3600000))
duration_min=$(((duration_ms % 3600000) / 60000))

cost_usd=$(echo "$input" | jq -r '.cost.total_cost_usd // 0')

total_tokens=$(echo "$input" | jq -r '.context_window.context_window_size // 200000')

# Try new percentage fields first (Claude Code 2.1.6+)
used_pct_raw=$(echo "$input" | jq -r '.context_window.used_percentage // null')
remaining_pct_raw=$(echo "$input" | jq -r '.context_window.remaining_percentage // null')

if [[ "$used_pct_raw" != "null" && -n "$used_pct_raw" ]]; then
    usage_pct=${used_pct_raw%.*}
    remaining_pct=${remaining_pct_raw%.*}
    used_tokens=$(( (total_tokens * usage_pct) / 100 ))
    free_tokens=$(( (total_tokens * remaining_pct) / 100 ))
else
    # Fallback: Calculate from current_usage
    current_usage=$(echo "$input" | jq -r '.context_window.current_usage // null')
    if [[ "$current_usage" != "null" ]]; then
        input_tokens=$(echo "$current_usage" | jq -r '.input_tokens // 0')
        cache_creation=$(echo "$current_usage" | jq -r '.cache_creation_input_tokens // 0')
        cache_read=$(echo "$current_usage" | jq -r '.cache_read_input_tokens // 0')
        used_tokens=$((input_tokens + cache_creation + cache_read))
    else
        used_tokens=0
    fi
    free_tokens=$((total_tokens - used_tokens))
    if [[ $total_tokens -gt 0 ]]; then
        usage_pct=$(( (used_tokens * 100) / total_tokens ))
    else
        usage_pct=0
    fi
fi

# Generate brick visualization (20 bricks)
total_bricks=20
if [[ $total_tokens -gt 0 ]]; then
    used_bricks=$(( (used_tokens * total_bricks) / total_tokens ))
else
    used_bricks=0
fi
free_bricks=$((total_bricks - used_bricks))

brick_line="["
for ((i=0; i<used_bricks; i++)); do
    brick_line+="\033[0;36m■\033[0m"
done
for ((i=0; i<free_bricks; i++)); do
    brick_line+="\033[2;37m□\033[0m"
done
brick_line+="]"

brick_line+=" \033[1m${usage_pct}%\033[0m"
brick_line+=" | ${duration_hours}h${duration_min}m"

# Add cost only if non-zero
if command -v bc &> /dev/null; then
    if (( $(echo "$cost_usd > 0" | bc -l 2>/dev/null || echo "0") )); then
        cost_formatted=$(printf "%.2f" "$cost_usd" 2>/dev/null || echo "0.00")
        brick_line+=" | \033[0;33m\$${cost_formatted}\033[0m"
    fi
else
    if [[ "$cost_usd" != "0" && "$cost_usd" != "0.0" && "$cost_usd" != "0.00" && -n "$cost_usd" ]]; then
        brick_line+=" | \033[0;33m\$${cost_usd}\033[0m"
    fi
fi

# Write context data to shared file for exo-self hooks to read
EXO_CONTEXT_FILE="$HOME/.claude/exo-self/.context-window.json"
echo "$input" | jq -c "{
  used_percentage: (.context_window.used_percentage // null),
  remaining_percentage: (.context_window.remaining_percentage // null),
  context_window_size: (.context_window.context_window_size // 200000),
  exceeds_200k_tokens: (.exceeds_200k_tokens // false),
  used_tokens: $used_tokens,
  free_tokens: $free_tokens,
  usage_pct: $usage_pct,
  session_id: \"$session_id\",
  updated_at: $(date +%s)
}" > "$EXO_CONTEXT_FILE" 2>/dev/null

# Output
echo -e "$line1"
echo -e "$brick_line"
