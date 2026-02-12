---
description: Show estimated context usage and exo-self session stats
allowed-tools: ["Bash", "Read"]
---

# /context-budget — Context Usage Estimate

Show how much context has been used and what thresholds have been hit.

## Steps

1. **Get context usage data** — prefer token-accurate statusline data, fall back to transcript size:
   ```bash
   uv run python -c "
   import os, glob, json, time

   exo_dir = os.path.expanduser('~/.claude/exo-self')
   ctx_file = os.path.join(exo_dir, '.context-window.json')

   source = 'none'
   usage_pct = 0
   extra = ''

   # Priority 1: Statusline token data (written by statusline.sh)
   if os.path.exists(ctx_file):
       try:
           with open(ctx_file) as f:
               ctx = json.load(f)
           updated_at = ctx.get('updated_at', 0)
           age = time.time() - updated_at
           if age < 120:
               pct = ctx.get('used_percentage') or ctx.get('usage_pct')
               if pct is not None:
                   usage_pct = int(float(pct))
                   source = 'tokens'
                   used_k = ctx.get('used_tokens', 0) // 1000
                   free_k = ctx.get('free_tokens', 0) // 1000
                   total_k = ctx.get('context_window_size', 200000) // 1000
                   extra = f'used={used_k}k free={free_k}k total={total_k}k'
       except Exception:
           pass

   # Priority 2: Transcript file size (rough approximation)
   if source == 'none':
       projects_dir = os.path.expanduser('~/.claude/projects')
       cwd = os.getcwd()
       candidates = [
           cwd.replace('/', '-'),
           cwd.replace('/', '-').replace('_', '-'),
           cwd.replace('/', '-').replace('_', '-').replace('.', '-'),
       ]
       project_dir = None
       for mangled in candidates:
           candidate = os.path.join(projects_dir, mangled)
           if os.path.isdir(candidate):
               project_dir = candidate
               break
       if project_dir is None:
           basename = os.path.basename(cwd)
           for d in os.listdir(projects_dir):
               if d.endswith('-' + basename) or d.endswith('-' + basename.replace('_', '-')):
                   candidate = os.path.join(projects_dir, d)
                   if os.path.isdir(candidate):
                       project_dir = candidate
                       break
       if project_dir:
           jsonl_files = glob.glob(os.path.join(project_dir, '*.jsonl'))
           if jsonl_files:
               transcript = max(jsonl_files, key=os.path.getmtime)
               size = os.path.getsize(transcript)
               max_chars = 800000
               try:
                   cfg_path = os.path.join(exo_dir, 'config.json')
                   with open(cfg_path) as f:
                       max_chars = json.load(f).get('estimated_max_chars', max_chars)
               except Exception:
                   pass
               usage_pct = int((size / max_chars) * 100)
               source = 'filesize'
               extra = f'transcript={size // 1024}KB est_max={max_chars // 1024}KB'

   print(f'source={source}')
   print(f'usage_pct={usage_pct}')
   print(f'extra={extra}')
   "
   ```

2. **Read config** from `~/.claude/exo-self/config.json` for thresholds:
   - `estimated_max_chars` (default: 800,000)
   - `checkin_threshold` (default: 0.50)
   - `reserve_threshold` (default: 0.80)

3. **Read monitor state** from `~/.claude/exo-self/.context-monitor-state.json`:
   - Whether check-in has fired (and whether Claude responded)
   - Whether reserve reminder has fired
   - At what ratio each triggered
   - Session ID for this session
   - Data source used (tokens vs filesize)

4. **Read session stats** from `~/.claude/exo-self/meta.json`

5. **Present a clear budget report**:
   ```
   ## Context Budget

   Usage: ~XX% (source: token-accurate / transcript estimate)
   Details: used=XXk free=XXk total=XXXk (or transcript=XXkB est_max=XXXkB)
   Session ID: XXXXXXXXXXXX

   ### Thresholds
   - [x] 25% nudge: Fired (or [ ] Not yet reached)
   - [x] 50% check-in: Fired at XX% (source: tokens/filesize) (or [ ] Not yet reached)
     - [x] Responded (journal updated) (or [ ] Not yet responded)
   - [ ] 80% reserve: Not yet reached (or [x] Fired at XX%)

   ### Data Sources
   - Primary: statusline token data (~/.claude/exo-self/.context-window.json)
   - Fallback: transcript file size / estimated_max_chars
   - Currently using: [tokens | filesize | none]

   ### Session Stats
   - Session #N overall
   - X check-ins completed across all sessions
   - Last session: YYYY-MM-DD

   ### Configuration
   - Nudge threshold: 25%
   - Check-in threshold: 50%
   - Reserve threshold: 80%
   - Estimated max chars: 800,000 (filesize fallback only)
   - Config: ~/.claude/exo-self/config.json
   ```

## Notes

- **Token-accurate data** comes from the statusline writing to `.context-window.json` — this uses the actual `used_percentage` from Claude Code's API
- **Transcript file size** is a rough fallback when statusline data is unavailable or stale (>120s old)
- The token source is significantly more accurate than file size estimation
- If you're seeing this near 80%, consider wrapping up or saving state
